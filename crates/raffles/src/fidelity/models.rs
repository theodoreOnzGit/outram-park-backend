// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// NOT ported from RAVEN. Added 2026-11-04 for GitHub issue #1194 (epic #1177)
// at the WORKSPACE MAINTAINER's direction; the crate owner's (Adolphus Lye's)
// review is outstanding — see this crate's CLAUDE.md, "Scope boundaries".

//! The caller-implemented seams, [`HighFidelityModel`] and
//! [`ParametricModel`], and the corrected low-fidelity model
//! `y(x) = y_LF(x, θ) + δ(x)` ([`CorrectedModel`]).
//!
//! Both traits are implemented in the physics crate and consumed here
//! **generically** (`M: HighFidelityModel`), never as `dyn` (root
//! `AGENTS.md`, "Crate-level interfaces may follow SOLID principles"): RAFFLES
//! owns the implementer by value, so no lifetime is needed.

use super::dataset::{Dataset, Split};
use super::quantities::Quantities;
use super::space::ParameterSpace;
use super::surrogate::{Prediction, Surrogate};
use crate::Result;

/// An expensive model `y = f(x)`, typed: CFD, Monte Carlo transport, or a
/// toy function for a test.
pub trait HighFidelityModel {
    /// The input record `x`.
    type Input: Quantities;
    /// The output record `y`.
    type Output: Quantities;

    /// Bounds or priors on `x`, which a design is mapped through.
    fn input_space(&self) -> ParameterSpace<Self::Input>;

    /// Runs the model at `x`.
    ///
    /// # Errors
    ///
    /// The implementer's failure, typically as
    /// [`crate::RafflesError::ModelEvaluation`].
    fn evaluate(&self, x: &Self::Input) -> Result<Self::Output>;
}

/// A cheaper model with tunable parameters, `y = f_LF(x, θ)`: the object of
/// a Kennedy–O'Hagan calibration (gh:#1180) or a DUD fit (gh:#1192).
pub trait ParametricModel {
    /// The input record `x`.
    type Input: Quantities;
    /// The parameter record `θ`.
    type Theta: Quantities;
    /// The output record `y`.
    type Output: Quantities;

    /// Bounds or priors on `θ`.
    fn parameter_space(&self) -> ParameterSpace<Self::Theta>;

    /// Runs the model at `(x, θ)`.
    ///
    /// # Errors
    ///
    /// As [`HighFidelityModel::evaluate`].
    fn evaluate(&self, x: &Self::Input, theta: &Self::Theta) -> Result<Self::Output>;
}

/// What is known about `θ`: a point, or ~~posterior samples (from TMCMC,
/// gh:#1180) as the planned second variant~~ (landed 2026-11-04, gh:#1180:
/// [`ThetaEstimate::Posterior`], from [`crate::fidelity::koh`]).
#[derive(Debug, Clone, PartialEq)]
pub enum ThetaEstimate<T: Quantities> {
    /// One value of `θ` (e.g. the nominal, uncalibrated one, or a MAP).
    Point(T),
    /// Posterior samples of `θ` and their channel-wise mean (in SI), the
    /// value [`point`](Self::point) gives a [`CorrectedModel`].
    Posterior {
        /// The samples.
        samples: Vec<T>,
        /// The mean of each channel's SI value over the samples.
        mean: T,
    },
}

impl<T: Quantities> ThetaEstimate<T> {
    /// A posterior estimate from samples; `mean` is the channel-wise SI mean.
    ///
    /// # Errors
    /// [`crate::RafflesError::InvalidParameter`] for no samples.
    pub fn posterior(samples: Vec<T>) -> Result<Self> {
        let Some(first) = samples.first() else {
            return Err(crate::RafflesError::InvalidParameter {
                parameter: "samples".to_string(),
                value: 0.0,
                reason: "a posterior needs at least one sample".to_string(),
            });
        };
        let mut sum = vec![0.0; first.to_si().len()];
        for s in &samples {
            for (a, v) in sum.iter_mut().zip(s.to_si()) {
                *a += v;
            }
        }
        let n = samples.len() as f64;
        let mean = T::from_si(&sum.iter().map(|a| a / n).collect::<Vec<f64>>())?;
        Ok(Self::Posterior { samples, mean })
    }

    /// The value used for a prediction: the point itself, or the posterior
    /// mean.
    #[must_use]
    pub fn point(&self) -> &T {
        match self {
            Self::Point(t) => t,
            Self::Posterior { mean, .. } => mean,
        }
    }

    /// The samples: the one point, or the posterior's.
    #[must_use]
    pub fn samples(&self) -> &[T] {
        match self {
            Self::Point(t) => core::slice::from_ref(t),
            Self::Posterior { samples, .. } => samples,
        }
    }
}

/// `y(x) = y_LF(x, θ) + δ(x)`: a low-fidelity model at an estimated `θ`,
/// plus an optional discrepancy surrogate `δ` fitted to `y_HF − y_LF`.
///
/// With no `δ`, [`predict`](Self::predict) returns the low-fidelity output
/// itself, untouched (no SI round trip), so it is bit-identical to calling
/// the model.
#[derive(Debug, Clone)]
pub struct CorrectedModel<L: ParametricModel> {
    lf: L,
    theta: ThetaEstimate<L::Theta>,
    delta: Option<Surrogate<L::Input, L::Output>>,
}

impl<L: ParametricModel> CorrectedModel<L> {
    /// A corrected model from its parts.
    #[must_use]
    pub fn new(
        lf: L,
        theta: ThetaEstimate<L::Theta>,
        delta: Option<Surrogate<L::Input, L::Output>>,
    ) -> Self {
        Self { lf, theta, delta }
    }

    /// The low-fidelity model.
    #[must_use]
    pub fn lf(&self) -> &L {
        &self.lf
    }

    /// The `θ` estimate.
    #[must_use]
    pub fn theta(&self) -> &ThetaEstimate<L::Theta> {
        &self.theta
    }

    /// The discrepancy surrogate, if any.
    #[must_use]
    pub fn delta(&self) -> Option<&Surrogate<L::Input, L::Output>> {
        self.delta.as_ref()
    }

    /// `y_LF(x, θ) + δ(x)`; the standard deviation is `δ`'s (none for the
    /// low-fidelity model alone).
    ///
    /// # Errors
    ///
    /// The model's or the surrogate's.
    pub fn predict(&self, x: &L::Input) -> Result<Prediction<L::Output>> {
        let y = self.lf.evaluate(x, self.theta.point())?;
        let Some(delta) = &self.delta else {
            return Ok(Prediction {
                mean: y,
                std_dev: None,
            });
        };
        let (d, s) = delta.predict_si(&x.to_si())?;
        let sum: Vec<f64> = y.to_si().iter().zip(d.iter()).map(|(a, b)| a + b).collect();
        Ok(Prediction {
            mean: L::Output::from_si(&sum)?,
            std_dev: s.map(|s| L::Output::from_si(&s)).transpose()?,
        })
    }
}

/// The discrepancy data `δ_i = y_i − y_LF(x_i, θ)` at every row of `data`,
/// **keeping each row's split**, so a `δ` fitted to it ([`Surrogate::fit`])
/// reads only the training rows and is validated on the held-out ones.
///
/// # Errors
///
/// The model's.
pub fn discrepancy_dataset<L: ParametricModel>(
    lf: &L,
    theta: &ThetaEstimate<L::Theta>,
    data: &Dataset<L::Input, L::Output>,
) -> Result<Dataset<L::Input, L::Output>> {
    let mut x = Vec::with_capacity(data.len());
    let mut d = Vec::with_capacity(data.len());
    let mut split: Vec<Split> = Vec::with_capacity(data.len());
    for i in 0..data.len() {
        let (xi, yi, si) = data.row(i)?;
        let lf_y = lf.evaluate(&xi, theta.point())?;
        d.push(
            yi.to_si()
                .iter()
                .zip(lf_y.to_si().iter())
                .map(|(a, b)| a - b)
                .collect(),
        );
        x.push(xi.to_si());
        split.push(si);
    }
    let provenance = format!("discrepancy y - y_LF(x, theta) of: {}", data.provenance());
    Dataset::from_si(x, d, split, data.fidelity().clone(), &provenance)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fidelity::{Fidelity, SurrogateSpec, ValidationReport};
    use crate::RafflesError;
    use uom::si::f64::{Length, Ratio};
    use uom::si::length::meter;
    use uom::si::ratio::ratio;

    crate::quantities! {
        #[derive(Debug, Clone, Copy, PartialEq)]
        struct X { a: Length }
    }
    crate::quantities! {
        #[derive(Debug, Clone, Copy, PartialEq)]
        struct Th { slope: Ratio }
    }
    crate::quantities! {
        #[derive(Debug, Clone, Copy, PartialEq)]
        struct Y { b: Length }
    }

    /// `y_LF = θ a`; fails for `a < 0`.
    #[derive(Debug, Clone)]
    struct Linear;

    impl ParametricModel for Linear {
        type Input = X;
        type Theta = Th;
        type Output = Y;
        fn parameter_space(&self) -> ParameterSpace<Th> {
            let t = |v: f64| Th {
                slope: Ratio::new::<ratio>(v),
            };
            match ParameterSpace::bounded(&t(0.0), &t(4.0)) {
                Ok(s) => s,
                Err(e) => panic!("{e}"),
            }
        }
        fn evaluate(&self, x: &X, theta: &Th) -> Result<Y> {
            if x.a.value < 0.0 {
                return Err(RafflesError::ModelEvaluation {
                    model: "Linear".into(),
                    reason: "a < 0".into(),
                });
            }
            Ok(Y {
                b: Length::new::<meter>(x.a.value * theta.slope.value),
            })
        }
    }

    /// **Methodology.** HF = `2 a + a²`, LF = `θ a` at θ = 2: the
    /// discrepancy is exactly `a²`, so a degree-2 `δ` fitted to the training
    /// residuals makes the corrected model exact at the held-out points;
    /// without `δ` the corrected model is the LF model bit for bit.
    ///
    /// **Prediction:** exact to round-off. **Result (2026-11-04):** held-out error below 1e-12 m
    /// at both held-out rows; LF-only path bit-identical (`==`).
    #[test]
    fn a_discrepancy_surrogate_corrects_the_low_fidelity_model() {
        let xs: Vec<X> = (0..9)
            .map(|i| X {
                a: Length::new::<meter>(f64::from(i) * 0.5),
            })
            .collect();
        let ys: Vec<Y> = xs
            .iter()
            .map(|x| Y {
                b: Length::new::<meter>(2.0 * x.a.value + x.a.value * x.a.value),
            })
            .collect();
        let split: Vec<Split> = (0..9)
            .map(|i| {
                if i % 4 == 2 {
                    Split::HeldOut
                } else {
                    Split::Training
                }
            })
            .collect();
        let hf = Dataset::new(&xs, &ys, &split, Fidelity::new(1, "toy HF"), "unit test").unwrap();
        let theta = ThetaEstimate::Point(Th {
            slope: Ratio::new::<ratio>(2.0),
        });
        assert_eq!(theta.point().slope.value, 2.0);
        assert_eq!(theta.samples().len(), 1);
        let post = ThetaEstimate::posterior(vec![
            Th { slope: Ratio::new::<ratio>(1.0) },
            Th { slope: Ratio::new::<ratio>(3.0) },
        ])
        .unwrap();
        assert_eq!(post.point().slope.value, 2.0);
        assert_eq!(post.samples().len(), 2);
        assert!(ThetaEstimate::<Th>::posterior(Vec::new()).is_err());

        let lf_only = CorrectedModel::new(Linear, theta.clone(), None);
        let x = X {
            a: Length::new::<meter>(1.7),
        };
        assert_eq!(
            lf_only.predict(&x).unwrap().mean,
            Linear.evaluate(&x, theta.point()).unwrap()
        );
        assert!(lf_only.delta().is_none());

        let resid = discrepancy_dataset(&Linear, &theta, &hf).unwrap();
        assert_eq!(resid.split(), hf.split());
        assert!(resid.provenance().starts_with("discrepancy"));
        let spec = SurrogateSpec::Polynomial {
            degree: 2,
            ridge: 0.0,
        };
        let delta = Surrogate::fit(&spec, &resid).unwrap();
        let report = ValidationReport::of(&delta, &spec, &resid, false).unwrap();
        assert!(report.held_out_rmse.unwrap().b.value < 1e-12);
        let corrected = CorrectedModel::new(Linear, theta, Some(delta));
        assert!(corrected.delta().is_some());
        assert_eq!(corrected.theta().point().slope.value, 2.0);
        for i in [2usize, 6] {
            let (x, y, s) = hf.row(i).unwrap();
            assert_eq!(s, Split::HeldOut);
            let p = corrected.predict(&x).unwrap();
            assert!((p.mean.b.value - y.b.value).abs() < 1e-12, "{i}");
            assert!(p.std_dev.is_none());
        }
        // The model's own failure is passed through.
        assert!(corrected
            .predict(&X {
                a: Length::new::<meter>(-1.0)
            })
            .is_err());
        assert_eq!(corrected.lf().parameter_space().channels()[0].name, "slope");
    }
}
