// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// NOT ported from RAVEN. Added 2026-11-04 for GitHub issue #1194 (epic #1177)
// at the WORKSPACE MAINTAINER's direction; the crate owner's (Adolphus Lye's)
// review is outstanding — see this crate's CLAUDE.md, "Scope boundaries".

//! The surrogate routes behind one typed interface: [`SurrogateSpec`] says
//! what to fit, [`Surrogate`] is the fitted model, [`Prediction`] what it
//! returns, and [`ValidationReport`] how wrong it is on the held-out rows.

use core::marker::PhantomData;

use super::dataset::Dataset;
use super::quantities::{Channel, Quantities};
use crate::surrogate::polynomial::MultiOutputPolynomial;
use crate::surrogate::validation::LeaveOneOut;
use crate::{RafflesError, Result};

/// The `burn` backend the [`Surrogate::Neural`] route trains and predicts
/// on: `ndarray` (pure Rust, Android- and wasm-clean) with autodiff, `f32`.
#[cfg(feature = "burn")]
pub type NeuralBackend = burn::backend::Autodiff<burn::backend::NdArray<f32>>;

/// What to fit.
///
/// Routes still to come, each added **with its port** rather than as an
/// empty variant now (this crate's convention: an unimplemented part has no
/// public entry point, `RafflesError`'s docs): the SMT kriging route and
/// `CoKriging` (gh:#1190). ~~`PolynomialChaos` (the RAVEN port, gh:#1193)~~
/// **landed 2026-11-04**, with `Kriging { route: RavenSklearn }`.
#[derive(Debug, Clone, PartialEq)]
pub enum SurrogateSpec {
    /// One least-squares polynomial per output
    /// ([`MultiOutputPolynomial`]), at total degree `degree` with Tikhonov
    /// ridge `ridge` ([`crate::surrogate::PolynomialSurrogate::fit`]).
    Polynomial {
        /// Total degree.
        degree: usize,
        /// Ridge; `0.0` for plain least squares.
        ridge: f64,
    },
    /// A `burn` feed-forward network ([`crate::surrogate::neural`]), all
    /// outputs at once. Needs the crate's `burn` feature.
    #[cfg(feature = "burn")]
    Neural {
        /// Width, depth, epochs, learning rate, seed.
        config: crate::surrogate::neural::TrainingConfig,
    },
    /// RAVEN's polynomial chaos ([`crate::surrogate::PolynomialChaos`],
    /// gh:#1193), all outputs at once. A projection on RAVEN's quadrature
    /// grid: the training rows must contain every point of `spec.grid()`
    /// (within 1e-9, RAVEN's rule); other training rows are ignored.
    PolynomialChaos {
        /// Inputs (in SI, the order of `I`'s channels), index set, order, grid.
        spec: crate::surrogate::PolynomialChaosSpec,
    },
    /// Gaussian-process regression ("kriging"), one GP per output, as
    /// RAVEN's default `<multioutput>`. Its prediction carries a standard
    /// deviation.
    Kriging {
        /// Kernel, alpha, target normalisation, optimiser.
        config: crate::surrogate::GpConfig,
        /// Which implementation.
        route: KrigingRoute,
    },
}

/// Which Gaussian-process implementation [`SurrogateSpec::Kriging`] uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KrigingRoute {
    /// RAVEN's `GaussianProcessRegressor` interface over scikit-learn's
    /// numerics ([`crate::surrogate::gaussian_process`], gh:#1193). The SMT
    /// route (gh:#1190) joins as a second variant when it lands.
    RavenSklearn,
}

/// A fitted surrogate from inputs `I` to outputs `O`.
///
/// Typed at the API ([`predict`](Self::predict) takes an `I` and returns a
/// [`Prediction<O>`]); the fitted models inside work on SI `f64`. The
/// `PhantomData` carries the types; there is no `dyn` and no `Box`.
#[derive(Debug, Clone)]
pub enum Surrogate<I: Quantities, O: Quantities> {
    /// [`SurrogateSpec::Polynomial`].
    Polynomial(MultiOutputPolynomial, PhantomData<fn(I) -> O>),
    /// [`SurrogateSpec::Neural`].
    #[cfg(feature = "burn")]
    Neural(
        crate::surrogate::neural::NeuralSurrogate<NeuralBackend>,
        PhantomData<fn(I) -> O>,
    ),
    /// [`SurrogateSpec::PolynomialChaos`].
    PolynomialChaos(crate::surrogate::PolynomialChaos, PhantomData<fn(I) -> O>),
    /// [`SurrogateSpec::Kriging`]: one fitted GP per output channel.
    Kriging(Vec<crate::surrogate::GaussianProcess>, PhantomData<fn(I) -> O>),
}

/// A surrogate's answer at one input.
#[derive(Debug, Clone, PartialEq)]
pub struct Prediction<O> {
    /// The predicted output.
    pub mean: O,
    /// Its standard deviation, **in the output's own units**, when the route
    /// has one (a Gaussian process will; the polynomial and neural routes do
    /// not, and report `None` rather than a made-up spread). For an absolute
    /// `ThermodynamicTemperature` channel the stored kelvin value is the
    /// spread.
    pub std_dev: Option<O>,
}

impl<I: Quantities, O: Quantities> Surrogate<I, O> {
    /// Fits `spec` to the **training rows** of `data`. Held-out rows are
    /// never read ([`Dataset::training_si`]).
    ///
    /// # Errors
    ///
    /// The route's own fitting errors (e.g. a non-finite training value, or
    /// a design that cannot identify the polynomial).
    pub fn fit(spec: &SurrogateSpec, data: &Dataset<I, O>) -> Result<Self> {
        let (x, y) = data.training_si();
        Self::fit_si(spec, &x, &y)
    }

    /// Fits `spec` to SI rows.
    fn fit_si(spec: &SurrogateSpec, x: &[Vec<f64>], y: &[Vec<f64>]) -> Result<Self> {
        match spec {
            SurrogateSpec::Polynomial { degree, ridge } => Ok(Self::Polynomial(
                MultiOutputPolynomial::fit(x, y, *degree, *ridge)?,
                PhantomData,
            )),
            #[cfg(feature = "burn")]
            SurrogateSpec::Neural { config } => {
                let device = Default::default();
                let (net, _report) =
                    crate::surrogate::neural::train::<NeuralBackend>(x, y, config, &device)?;
                Ok(Self::Neural(net, PhantomData))
            }
            SurrogateSpec::PolynomialChaos { spec } => Ok(Self::PolynomialChaos(
                crate::surrogate::PolynomialChaos::fit(spec, x, y)?,
                PhantomData,
            )),
            SurrogateSpec::Kriging { config, route } => match route {
                KrigingRoute::RavenSklearn => Ok(Self::Kriging(
                    fit_kriging(config, x, y)?,
                    PhantomData,
                )),
            },
        }
    }

    /// The polynomial fits, if this is the polynomial route.
    #[must_use]
    pub fn as_polynomial(&self) -> Option<&MultiOutputPolynomial> {
        match self {
            Self::Polynomial(p, _) => Some(p),
            #[cfg(feature = "burn")]
            Self::Neural(..) => None,
            Self::PolynomialChaos(..) | Self::Kriging(..) => None,
        }
    }

    /// Prediction at `x`.
    ///
    /// # Errors
    ///
    /// The route's prediction errors.
    pub fn predict(&self, x: &I) -> Result<Prediction<O>> {
        let (mean, std_dev) = self.predict_si(&x.to_si())?;
        Ok(Prediction {
            mean: O::from_si(&mean)?,
            std_dev: std_dev.map(|s| O::from_si(&s)).transpose()?,
        })
    }

    /// Prediction at an SI row: the mean and, when the route has one, the
    /// standard deviation, both SI.
    ///
    /// # Errors
    ///
    /// As [`predict`](Self::predict).
    pub fn predict_si(&self, x: &[f64]) -> Result<(Vec<f64>, Option<Vec<f64>>)> {
        match self {
            Self::Polynomial(p, _) => Ok((p.predict(x)?, None)),
            #[cfg(feature = "burn")]
            Self::Neural(n, _) => Ok((n.predict(x)?, None)),
            Self::PolynomialChaos(p, _) => Ok((p.predict(x)?, None)),
            Self::Kriging(gps, _) => {
                let mut m = Vec::with_capacity(gps.len());
                let mut s = Vec::with_capacity(gps.len());
                for gp in gps {
                    let (a, b) = gp.predict(x)?;
                    m.push(a);
                    s.push(b);
                }
                Ok((m, Some(s)))
            }
        }
    }
}

/// One GP per output column (RAVEN's `MultiOutputRegressor` default).
fn fit_kriging(
    config: &crate::surrogate::GpConfig,
    x: &[Vec<f64>],
    y: &[Vec<f64>],
) -> Result<Vec<crate::surrogate::GaussianProcess>> {
    let w = y.first().map_or(0, Vec::len);
    (0..w)
        .map(|k| {
            let col: Vec<f64> = y.iter().map(|r| r[k]).collect();
            crate::surrogate::GaussianProcess::fit(config, x, &col)
        })
        .collect()
}

/// How wrong a fitted surrogate is, **on rows it was not fitted to**.
///
/// Every error is per output channel, in that channel's units (an SI value
/// in a typed `O`; for an absolute-temperature channel the kelvin value is
/// the error, not a temperature).
#[derive(Debug, Clone, PartialEq)]
pub struct ValidationReport<O: Quantities> {
    /// Output channels, in order.
    pub channels: Vec<Channel>,
    /// Training and held-out row counts.
    pub counts: (usize, usize),
    /// Root-mean-square error over the held-out rows (`None` if there are
    /// none).
    pub held_out_rmse: Option<O>,
    /// Largest absolute error over the held-out rows.
    pub held_out_max_error: Option<O>,
    /// Root-mean-square leave-one-out residual over the **training** rows,
    /// each a genuine prediction of a row its model did not see. `None` when
    /// not requested, or for a route without a LOO implementation (the
    /// neural route: `n` trainings).
    pub loo_rmse: Option<O>,
    /// Fraction of held-out rows whose truth lies within `mean ± 2 std_dev`,
    /// per channel (0.954 for calibrated Gaussian errors). `None` unless the
    /// route reports a standard deviation.
    pub coverage_2sigma: Option<Vec<f64>>,
}

impl<O: Quantities> ValidationReport<O> {
    /// Validates `surrogate`, fitted with `spec` to `data`'s training rows,
    /// on `data`'s held-out rows; and, if `leave_one_out`, by leave-one-out
    /// refits on the training rows.
    ///
    /// # Errors
    ///
    /// Prediction or refit errors.
    pub fn of<I: Quantities>(
        surrogate: &Surrogate<I, O>,
        spec: &SurrogateSpec,
        data: &Dataset<I, O>,
        leave_one_out: bool,
    ) -> Result<Self> {
        let (hx, hy) = data.held_out_si();
        let mut means = Vec::with_capacity(hx.len());
        let mut sds = Vec::with_capacity(hx.len());
        for x in &hx {
            let (m, s) = surrogate.predict_si(x)?;
            means.push(m);
            sds.push(s);
        }
        let errors = held_out_errors(&hy, &means)?;
        let (held_out_rmse, held_out_max_error) = match errors {
            Some((r, m)) => (Some(O::from_si(&r)?), Some(O::from_si(&m)?)),
            None => (None, None),
        };
        let sd_rows: Option<Vec<Vec<f64>>> = sds.into_iter().collect();
        let coverage_2sigma = match sd_rows {
            Some(s) if !hy.is_empty() => Some(interval_coverage(&hy, &means, &s, 2.0)?),
            _ => None,
        };
        let loo_rmse = if leave_one_out {
            let (tx, ty) = data.training_si();
            loo_rmse_si(spec, &tx, &ty)?
                .map(|r| O::from_si(&r))
                .transpose()?
        } else {
            None
        };
        Ok(Self {
            channels: O::channels(),
            counts: data.counts(),
            held_out_rmse,
            held_out_max_error,
            loo_rmse,
            coverage_2sigma,
        })
    }
}

/// Per-channel RMSE and maximum absolute error of `pred` against `truth`;
/// `None` for no rows.
fn held_out_errors(truth: &[Vec<f64>], pred: &[Vec<f64>]) -> Result<Option<(Vec<f64>, Vec<f64>)>> {
    if truth.len() != pred.len() {
        return Err(RafflesError::DimensionMismatch {
            expected: truth.len(),
            found: pred.len(),
        });
    }
    let Some(first) = truth.first() else {
        return Ok(None);
    };
    let w = first.len();
    let mut sum = vec![0.0; w];
    let mut max = vec![0.0_f64; w];
    for (t, p) in truth.iter().zip(pred.iter()) {
        if t.len() != w || p.len() != w {
            return Err(RafflesError::DimensionMismatch {
                expected: w,
                found: p.len().min(t.len()),
            });
        }
        for k in 0..w {
            let e = t[k] - p[k];
            sum[k] += e * e;
            // NaN must propagate into the report, so no `f64::max` (which
            // drops it): once NaN, the maximum stays NaN.
            let a = e.abs();
            if !max[k].is_nan() && (a.is_nan() || a > max[k]) {
                max[k] = a;
            }
        }
    }
    let n = truth.len() as f64;
    Ok(Some((sum.iter().map(|s| (s / n).sqrt()).collect(), max)))
}

/// Per-channel fraction of rows with `|truth − mean| <= k std_dev`.
fn interval_coverage(
    truth: &[Vec<f64>],
    mean: &[Vec<f64>],
    std_dev: &[Vec<f64>],
    k: f64,
) -> Result<Vec<f64>> {
    if truth.len() != mean.len() || truth.len() != std_dev.len() || truth.is_empty() {
        return Err(RafflesError::DimensionMismatch {
            expected: truth.len(),
            found: mean.len().min(std_dev.len()),
        });
    }
    let w = truth[0].len();
    let mut inside = vec![0usize; w];
    for i in 0..truth.len() {
        for c in 0..w {
            if (truth[i][c] - mean[i][c]).abs() <= k * std_dev[i][c] {
                inside[c] += 1;
            }
        }
    }
    Ok(inside
        .iter()
        .map(|n| *n as f64 / truth.len() as f64)
        .collect())
}

/// Per-channel leave-one-out RMSE on the training rows, by the route's own
/// LOO machinery; `None` for a route without one.
fn loo_rmse_si(spec: &SurrogateSpec, x: &[Vec<f64>], y: &[Vec<f64>]) -> Result<Option<Vec<f64>>> {
    match spec {
        SurrogateSpec::Polynomial { degree, ridge } => {
            let w = y.first().map_or(0, Vec::len);
            let mut out = Vec::with_capacity(w);
            for k in 0..w {
                let column: Vec<f64> = y.iter().map(|r| r[k]).collect();
                out.push(LeaveOneOut::fit(x, &column, *degree, *ridge)?.rmse());
            }
            Ok(Some(out))
        }
        #[cfg(feature = "burn")]
        SurrogateSpec::Neural { .. } => Ok(None),
        // A projection on a fixed quadrature grid: dropping a node breaks the
        // rule, so leave-one-out is not defined for it.
        SurrogateSpec::PolynomialChaos { .. } => Ok(None),
        // Closed-form LOO at the hyperparameters fitted on all training rows
        // (Rasmussen and Williams 2006, eq. 5.12).
        SurrogateSpec::Kriging { config, .. } => {
            let gps = fit_kriging(config, x, y)?;
            Ok(Some(
                gps.iter()
                    .map(|gp| {
                        let r = gp.loo_residuals();
                        (r.iter().map(|e| e * e).sum::<f64>() / r.len() as f64).sqrt()
                    })
                    .collect(),
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fidelity::{Fidelity, Split};
    use uom::si::f64::{Length, ThermodynamicTemperature};
    use uom::si::length::meter;
    use uom::si::thermodynamic_temperature::kelvin;

    crate::quantities! {
        #[derive(Debug, Clone, Copy, PartialEq)]
        struct X { a: Length }
    }
    crate::quantities! {
        #[derive(Debug, Clone, Copy, PartialEq)]
        struct Y { t: ThermodynamicTemperature }
    }

    /// An exact quadratic, held-out rows included: the polynomial route
    /// reproduces it, the report says ~0 everywhere, and the prediction is
    /// typed.
    #[test]
    fn the_polynomial_route_is_typed_end_to_end() {
        let f = |a: f64| 300.0 + 2.0 * a - 0.5 * a * a;
        let x: Vec<X> = (0..10)
            .map(|i| X {
                a: Length::new::<meter>(f64::from(i) * 0.4),
            })
            .collect();
        let y: Vec<Y> = x
            .iter()
            .map(|x| Y {
                t: ThermodynamicTemperature::new::<kelvin>(f(x.a.value)),
            })
            .collect();
        let split: Vec<Split> = (0..10)
            .map(|i| {
                if i % 3 == 1 {
                    Split::HeldOut
                } else {
                    Split::Training
                }
            })
            .collect();
        let data = Dataset::new(
            &x,
            &y,
            &split,
            Fidelity::new(1, "exact quadratic"),
            "unit test",
        )
        .unwrap();
        let spec = SurrogateSpec::Polynomial {
            degree: 2,
            ridge: 0.0,
        };
        let s = Surrogate::fit(&spec, &data).unwrap();
        let p = s
            .predict(&X {
                a: Length::new::<meter>(1.3),
            })
            .unwrap();
        assert!((p.mean.t.get::<kelvin>() - f(1.3)).abs() < 1e-10);
        assert!(p.std_dev.is_none());
        let r = ValidationReport::of(&s, &spec, &data, true).unwrap();
        assert_eq!(r.counts, (7, 3));
        assert_eq!(r.channels[0].unit, "K");
        assert!(r.held_out_rmse.unwrap().t.value < 1e-10);
        assert!(r.held_out_max_error.unwrap().t.value < 1e-10);
        assert!(r.loo_rmse.unwrap().t.value < 1e-10);
        assert!(r.coverage_2sigma.is_none());
        let r = ValidationReport::of(&s, &spec, &data, false).unwrap();
        assert!(r.loo_rmse.is_none());
    }

    /// The two RAVEN routes (gh:#1193) through the typed interface: polynomial
    /// chaos reproduces the same quadratic the polynomial route does, with no
    /// σ and no LOO; kriging returns a σ in the output's units, 2σ coverage on
    /// the held-out rows, and a LOO RMSE; both give the same bits as calling
    /// the route directly.
    #[test]
    fn the_raven_routes_are_typed_end_to_end() {
        use crate::surrogate::polynomial_chaos::{PceVariable, PolynomialChaosSpec};
        use crate::surrogate::{GaussianProcess, GpConfig, GpKernel, RavenKernel};
        let f = |a: f64| 300.0 + 2.0 * a - 0.5 * a * a;
        let pspec = PolynomialChaosSpec::total_degree(
            vec![PceVariable::Uniform { lower: 0.0, upper: 4.0 }],
            2,
        );
        let mut xs: Vec<f64> = pspec.grid().unwrap().points.iter().map(|p| p[0]).collect();
        xs.extend([0.5, 1.3, 2.7, 3.9]);
        let x: Vec<X> = xs.iter().map(|v| X { a: Length::new::<meter>(*v) }).collect();
        let y: Vec<Y> = xs
            .iter()
            .map(|v| Y { t: ThermodynamicTemperature::new::<kelvin>(f(*v)) })
            .collect();
        let split: Vec<Split> = (0..xs.len())
            .map(|i| if i < 3 { Split::Training } else { Split::HeldOut })
            .collect();
        let data = Dataset::new(&x, &y, &split, Fidelity::new(1, "quadratic"), "unit test").unwrap();
        let spec = SurrogateSpec::PolynomialChaos { spec: pspec.clone() };
        let s = Surrogate::fit(&spec, &data).unwrap();
        assert!(s.as_polynomial().is_none());
        let p = s.predict(&X { a: Length::new::<meter>(1.3) }).unwrap();
        assert!((p.mean.t.get::<kelvin>() - f(1.3)).abs() < 1e-10);
        assert!(p.std_dev.is_none());
        let r = ValidationReport::of(&s, &spec, &data, true).unwrap();
        assert!(r.held_out_max_error.unwrap().t.value < 1e-10);
        assert!(r.loo_rmse.is_none() && r.coverage_2sigma.is_none());
        let (tx, ty) = data.training_si();
        let direct = crate::surrogate::PolynomialChaos::fit(&pspec, &tx, &ty).unwrap();
        assert_eq!(s.predict_si(&[1.3]).unwrap().0, direct.predict(&[1.3]).unwrap());

        // Kriging on all but the held-out rows (here: the first 6 train).
        let split: Vec<Split> = (0..xs.len())
            .map(|i| if i % 2 == 0 || i < 2 { Split::Training } else { Split::HeldOut })
            .collect();
        let data = Dataset::new(&x, &y, &split, Fidelity::new(1, "quadratic"), "unit test").unwrap();
        let config = GpConfig::raven_with_kernel(GpKernel::raven(RavenKernel::ConstantRbf, 1, false));
        let spec = SurrogateSpec::Kriging { config: config.clone(), route: KrigingRoute::RavenSklearn };
        let s = Surrogate::fit(&spec, &data).unwrap();
        let p = s.predict(&X { a: Length::new::<meter>(1.3) }).unwrap();
        let (tx, ty) = data.training_si();
        let col: Vec<f64> = ty.iter().map(|r| r[0]).collect();
        let gp = GaussianProcess::fit(&config, &tx, &col).unwrap();
        let (m, sd) = gp.predict(&[1.3]).unwrap();
        assert_eq!(p.mean.t.get::<kelvin>().to_bits(), m.to_bits());
        assert_eq!(p.std_dev.unwrap().t.value.to_bits(), sd.to_bits());
        let r = ValidationReport::of(&s, &spec, &data, true).unwrap();
        assert!(r.coverage_2sigma.is_some() && r.loo_rmse.is_some());
        eprintln!("kriging typed: held-out RMSE {:?} K, LOO RMSE {:?} K, 2σ coverage {:?}",
            r.held_out_rmse.unwrap().t.value, r.loo_rmse.unwrap().t.value, r.coverage_2sigma);
    }

    /// The error and coverage helpers on hand-computed rows; NaN propagates
    /// into the maximum rather than being dropped.
    #[test]
    fn error_and_coverage_helpers() {
        let truth = vec![
            vec![1.0, 0.0],
            vec![3.0, 0.0],
            vec![0.0, 0.0],
            vec![0.0, 0.0],
        ];
        let pred = vec![
            vec![0.0, 0.0],
            vec![0.0, 1.0],
            vec![0.0, 0.0],
            vec![0.0, 0.0],
        ];
        let (rmse, max) = held_out_errors(&truth, &pred).unwrap().unwrap();
        assert_eq!(rmse, vec![(10.0_f64 / 4.0).sqrt(), 0.5]);
        assert_eq!(max, vec![3.0, 1.0]);
        assert!(held_out_errors(&[], &[]).unwrap().is_none());
        assert!(held_out_errors(&truth, &pred[..1]).is_err());
        let nan = held_out_errors(&[vec![f64::NAN], vec![1.0]], &[vec![0.0], vec![0.0]])
            .unwrap()
            .unwrap();
        assert!(nan.0[0].is_nan() && nan.1[0].is_nan());
        let sd = vec![vec![1.0, 1.0]; 4];
        // |e| = 1, 3, 0, 0 against 2σ = 2: three of four inside; channel 2 all.
        assert_eq!(
            interval_coverage(&truth, &pred, &sd, 2.0).unwrap(),
            vec![0.75, 1.0]
        );
        assert!(interval_coverage(&[], &[], &[], 2.0).is_err());
    }
}
