// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// NOT a port. The typed route from a `ParametricModel` and a `Dataset` into
// the SEPIA translation (`crate::bayesian::koh`), written for gh:#1180.

//! **Kennedy–O'Hagan calibration of a [`ParametricModel`] against a
//! [`Dataset`]** (gh:#1180): `y_HF(x) = y_LF(x, θ) + δ(x) + ε` with a GP
//! emulator of `y_LF` and a GP prior on δ, through SEPIA's scalar model
//! ([`crate::bayesian::koh`]).
//!
//! [`KohCalibration::calibrate`]:
//!
//! 1. **Simulator design:** at each caller-given design x, `runs_per_input`
//!    points of a seeded Latin hypercube over the calibrated θ coordinates,
//!    each mapped to a θ record (uncalibrated channels at
//!    [`KohSpec::theta_fixed`]) and run through the low-fidelity model.
//!    Running the low-fidelity model at a held-out x reads no held-out
//!    output.
//! 2. **Field data:** the dataset's **training rows only**.
//! 3. **Emulator hyperparameters** by the highest-posterior particle of a
//!    TMCMC run on SEPIA's simulation-only posterior, then
//! 4. **the KOH posterior** over (θ, β_V, λ_Vz, λ_Os) by TMCMC with the
//!    emulator held there (KOH 2001 §4.5's plug-in).
//!
//! Each coordinate may be taken on a log scale ([`Scale`]); the output must
//! be a single-channel record. Predictions ([`KohCalibration::predict`])
//! are mixtures over posterior samples of SEPIA's conditional Gaussians, with
//! δ's part reported beside the total.

use petir::mathf::RealMath;
use super::dataset::{Dataset, Split};
use super::models::{ParametricModel, ThetaEstimate};
use super::quantities::Quantities;
use crate::bayesian::koh::{KohData, KohForm, KohModel, KohParams, KohPosterior, KohPriors, KohSampleConfig, ParamPrior, PriorDist};
use crate::distributions::{ContinuousDistribution1D, Normal};
use crate::samplers::{LatinHypercube, SamplingDesign};
use crate::{RafflesError, Result};

/// The coordinate a channel is calibrated, emulated or modelled on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scale {
    /// The SI value itself.
    Linear,
    /// The natural log of the SI value (which must be positive).
    Log,
}

impl Scale {
    /// SI value to the coordinate.
    #[must_use]
    pub fn to_coord(self, si: f64) -> f64 {
        match self {
            Self::Linear => si,
            Self::Log => si.r_ln(),
        }
    }

    /// The coordinate back to an SI value.
    #[must_use]
    pub fn to_si(self, c: f64) -> f64 {
        match self {
            Self::Linear => c,
            Self::Log => c.r_exp(),
        }
    }
}

/// A prior on a calibrated θ coordinate, truncated to the design range.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ThetaPrior {
    /// Flat on the range (SEPIA's default N(0.5, 10²) on the unit scale).
    Uniform,
    /// Normal on the coordinate (ln of the SI value for [`Scale::Log`], the
    /// SI value for [`Scale::Linear`]).
    Normal {
        /// Mean, in coordinate units.
        mean: f64,
        /// Standard deviation, in coordinate units.
        sd: f64,
    },
}

/// One calibrated θ channel.
#[derive(Debug, Clone, PartialEq)]
pub struct CalibratedParameter {
    /// The θ channel's name.
    pub name: String,
    /// Its coordinate.
    pub scale: Scale,
    /// Its prior on that coordinate.
    pub prior: ThetaPrior,
}

/// A Gamma prior on the observation precision, stated as the error it
/// implies: `λ_Os ~ Gamma(shape, mean 1/σ²)`, σ the observation error on
/// the output coordinate divided by SEPIA's output standard deviation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ObservationError {
    /// The error's standard deviation on the output coordinate.
    pub sd: f64,
    /// The Gamma shape.
    pub shape: f64,
}

/// What to calibrate, and how.
#[derive(Debug, Clone, PartialEq)]
pub struct KohSpec<T: Quantities> {
    /// The calibrated θ channels.
    pub calibrated: Vec<CalibratedParameter>,
    /// The design range's lower end (only calibrated channels are read).
    pub theta_lower: T,
    /// The design range's upper end.
    pub theta_upper: T,
    /// Every channel's value when not calibrated, and the **derived value**
    /// an ablation resets a calibrated channel to.
    pub theta_fixed: T,
    /// The coordinate of each input channel.
    pub input_scales: Vec<Scale>,
    /// The output coordinate.
    pub output_scale: Scale,
    /// Low-fidelity runs per design input.
    pub runs_per_input: usize,
    /// The design's master seed (input `i` uses `seed + i`).
    pub design_seed: i64,
    /// The observation-precision prior; `None` keeps SEPIA's Gamma(1, 1e-3).
    pub observation_error: Option<ObservationError>,
    /// With or without δ.
    pub form: KohForm,
    /// TMCMC for the emulator's hyperparameters (all free, simulation only).
    pub emulator_sampler: KohSampleConfig,
    /// TMCMC for the calibration (`free` is overridden to `CalibrationOnly`).
    pub calibration_sampler: KohSampleConfig,
}

/// The predictive distribution at one x.
#[derive(Debug, Clone, PartialEq)]
pub struct KohPrediction<O: Quantities> {
    /// The mixture median, in output units.
    pub median: O,
    /// The 2.5 % quantile.
    pub lower95: O,
    /// The 97.5 % quantile.
    pub upper95: O,
    /// Mixture mean on the output coordinate.
    pub mean: f64,
    /// Mixture standard deviation on the output coordinate.
    pub sd: f64,
    /// δ's posterior mean on the output coordinate (0 without δ).
    pub delta_mean: f64,
    /// δ's posterior standard deviation on the output coordinate.
    pub delta_sd: f64,
}

/// A finished calibration.
#[derive(Debug, Clone)]
pub struct KohCalibration<L: ParametricModel> {
    lf: L,
    spec: KohSpec<L::Theta>,
    model: KohModel,
    emulator: KohParams,
    posterior: KohPosterior,
    calibrated_index: Vec<usize>,
}

fn channel_index<T: Quantities>(name: &str) -> Result<usize> {
    T::channels().iter().position(|c| c.name == name).ok_or_else(|| RafflesError::InvalidParameter { parameter: name.to_string(), value: f64::NAN, reason: "no θ channel of this name".to_string() })
}

fn scaled(v: &[f64], s: &[Scale]) -> Vec<f64> {
    v.iter().zip(s).map(|(a, sc)| sc.to_coord(*a)).collect()
}

/// The mixture CDF's inverse at `p` by bisection.
fn mixture_quantile(comp: &[(f64, f64)], p: f64) -> Result<f64> {
    let std = Normal::new(0.0, 1.0)?;
    let cdf = |y: f64| comp.iter().map(|(m, s)| std.cdf((y - m) / s)).sum::<f64>() / comp.len() as f64;
    let mut lo = comp.iter().map(|(m, s)| m - 10.0 * s).fold(f64::INFINITY, f64::min);
    let mut hi = comp.iter().map(|(m, s)| m + 10.0 * s).fold(f64::NEG_INFINITY, f64::max);
    for _ in 0..200 {
        let mid = 0.5 * (lo + hi);
        if cdf(mid) < p {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    Ok(0.5 * (lo + hi))
}

impl<L: ParametricModel> KohCalibration<L> {
    /// Runs the calibration (module docs).
    ///
    /// # Errors
    /// A multi-channel output, an unknown θ channel, a scale/width mismatch,
    /// the low-fidelity model's failure, a non-positive value on a log
    /// coordinate, or the samplers'.
    pub fn calibrate(lf: L, data: &Dataset<L::Input, L::Output>, design: &[L::Input], spec: KohSpec<L::Theta>) -> Result<Self> {
        if L::Output::channels().len() != 1 {
            return Err(RafflesError::DimensionMismatch { expected: 1, found: L::Output::channels().len() });
        }
        if spec.input_scales.len() != L::Input::channels().len() {
            return Err(RafflesError::DimensionMismatch { expected: L::Input::channels().len(), found: spec.input_scales.len() });
        }
        let idx: Vec<usize> = spec.calibrated.iter().map(|c| channel_index::<L::Theta>(&c.name)).collect::<Result<_>>()?;
        let q = idx.len();
        let (lo, hi, fixed) = (spec.theta_lower.to_si(), spec.theta_upper.to_si(), spec.theta_fixed.to_si());
        let (mut x_sim, mut t_sim, mut y_sim) = (Vec::new(), Vec::new(), Vec::new());
        let lhs = LatinHypercube::new(spec.runs_per_input, q)?;
        for (i, x) in design.iter().enumerate() {
            let xs = scaled(&x.to_si(), &spec.input_scales);
            for u in lhs.generate(spec.design_seed + i as i64) {
                let mut th = fixed.clone();
                let mut t = Vec::with_capacity(q);
                for (k, c) in spec.calibrated.iter().enumerate() {
                    let (a, b) = (c.scale.to_coord(lo[idx[k]]), c.scale.to_coord(hi[idx[k]]));
                    let tk = a + u[k] * (b - a);
                    th[idx[k]] = c.scale.to_si(tk);
                    t.push(tk);
                }
                let y = lf.evaluate(x, &L::Theta::from_si(&th)?)?.to_si()[0];
                x_sim.push(xs.clone());
                t_sim.push(t);
                y_sim.push(spec.output_scale.to_coord(y));
            }
        }
        let (mut x_obs, mut y_obs) = (Vec::new(), Vec::new());
        for i in 0..data.len() {
            let (x, y, s) = data.row(i)?;
            if s == Split::Training {
                x_obs.push(scaled(&x.to_si(), &spec.input_scales));
                y_obs.push(spec.output_scale.to_coord(y.to_si()[0]));
            }
        }
        if y_sim.iter().chain(&y_obs).chain(x_sim.iter().flatten()).any(|v| !v.is_finite()) {
            return Err(RafflesError::InvalidParameter { parameter: "data".to_string(), value: f64::NAN, reason: "a value is not finite on its coordinate (log of a non-positive value?)".to_string() });
        }
        let sim = KohData::new(&x_sim, &t_sim, &y_sim, &[], &[])?;
        let full = KohData::new(&x_sim, &t_sim, &y_sim, &x_obs, &y_obs)?;
        // θ priors on SEPIA's unit scale, which spans the design's own range.
        let mut priors = KohPriors::sepia_defaults(q);
        let (mut mean, mut sd) = (vec![0.5; q], vec![10.0; q]);
        for (k, c) in spec.calibrated.iter().enumerate() {
            if let ThetaPrior::Normal { mean: m, sd: s } = c.prior {
                let w = full.t_max[k] - full.t_min[k];
                mean[k] = (m - full.t_min[k]) / w;
                sd[k] = s / w;
            }
        }
        priors.theta = ParamPrior { dist: PriorDist::Normal { mean, sd }, lower: 0.0, upper: 1.0 };
        if let Some(e) = spec.observation_error {
            let lam_mean = (full.y_sd / e.sd).r_powi(2);
            priors.lam_os = ParamPrior { dist: PriorDist::Gamma { a: e.shape, b: e.shape / lam_mean }, lower: 0.0, upper: f64::INFINITY };
        }
        let p = full.p;
        let sim_model = KohModel::new(sim, KohForm::SimulationOnly, priors.clone())?;
        let emu_post = sim_model.sample(&KohParams::sepia_init(KohForm::SimulationOnly, p, q), &spec.emulator_sampler)?;
        let mut emulator = KohParams::sepia_init(spec.form, p, q);
        let best = emu_post.best();
        emulator.beta_u = best.beta_u.clone();
        emulator.lam_uz = best.lam_uz;
        emulator.lam_ws = best.lam_ws;
        emulator.lam_wos = best.lam_wos;
        Self::with_emulator(lf, full, priors, emulator, spec, idx)
    }

    fn with_emulator(lf: L, data: KohData, priors: KohPriors, emulator: KohParams, spec: KohSpec<L::Theta>, calibrated_index: Vec<usize>) -> Result<Self> {
        let model = KohModel::new(data, spec.form, priors)?;
        let mut cfg = spec.calibration_sampler;
        cfg.free = crate::bayesian::koh::KohFree::CalibrationOnly;
        let posterior = model.sample(&emulator, &cfg)?;
        Ok(Self { lf, spec, model, emulator, posterior, calibrated_index })
    }

    /// The same data and emulator, calibrated again as `form` (e.g. without
    /// δ, the identifiability contrast).
    ///
    /// # Errors
    /// The sampler's.
    pub fn recalibrate(&self, form: KohForm) -> Result<Self>
    where
        L: Clone,
    {
        let mut spec = self.spec.clone();
        spec.form = form;
        let mut emulator = KohParams::sepia_init(form, self.model.data.p, self.model.data.q);
        emulator.beta_u = self.emulator.beta_u.clone();
        emulator.lam_uz = self.emulator.lam_uz;
        emulator.lam_ws = self.emulator.lam_ws;
        emulator.lam_wos = self.emulator.lam_wos;
        Self::with_emulator(self.lf.clone(), self.model.data.clone(), self.model.priors.clone(), emulator, spec, self.calibrated_index.clone())
    }

    /// The SEPIA model (data, priors).
    #[must_use]
    pub fn model(&self) -> &KohModel {
        &self.model
    }

    /// The emulator's plug-in hyperparameters.
    #[must_use]
    pub fn emulator(&self) -> &KohParams {
        &self.emulator
    }

    /// The raw posterior.
    #[must_use]
    pub fn posterior(&self) -> &KohPosterior {
        &self.posterior
    }

    /// The low-fidelity model.
    #[must_use]
    pub fn lf(&self) -> &L {
        &self.lf
    }

    /// Posterior samples of the calibrated θ coordinates, one row each.
    #[must_use]
    pub fn theta_coordinates(&self) -> Vec<Vec<f64>> {
        self.posterior.samples.iter().map(|p| self.model.data.native_t(&p.theta)).collect()
    }

    /// The typed posterior of θ (uncalibrated channels at their fixed
    /// values), for a [`super::CorrectedModel`].
    ///
    /// # Errors
    /// [`ThetaEstimate::posterior`]'s, or a record that does not build.
    pub fn theta_estimate(&self) -> Result<ThetaEstimate<L::Theta>> {
        let fixed = self.spec.theta_fixed.to_si();
        let samples = self
            .theta_coordinates()
            .iter()
            .map(|t| {
                let mut th = fixed.clone();
                for (k, c) in self.spec.calibrated.iter().enumerate() {
                    th[self.calibrated_index[k]] = c.scale.to_si(t[k]);
                }
                L::Theta::from_si(&th)
            })
            .collect::<Result<Vec<_>>>()?;
        ThetaEstimate::posterior(samples)
    }

    /// The predictive at each x, a mixture over every `stride`-th posterior
    /// sample. `observation_error` adds 1/λ_Os (for comparing against a
    /// new observation). `reset` sets one calibrated channel to its
    /// [`KohSpec::theta_fixed`] value in every sample (the ablation).
    ///
    /// # Errors
    /// An unknown `reset` channel, a zero `stride`, or SEPIA's predictor.
    pub fn predict(&self, xs: &[L::Input], stride: usize, observation_error: bool, reset: Option<&str>) -> Result<Vec<KohPrediction<L::Output>>> {
        if stride == 0 {
            return Err(RafflesError::InvalidParameter { parameter: "stride".to_string(), value: 0.0, reason: "must be at least 1".to_string() });
        }
        let d = &self.model.data;
        let reset_at = match reset {
            None => None,
            Some(name) => {
                let k = self.spec.calibrated.iter().position(|c| c.name == name).ok_or_else(|| RafflesError::InvalidParameter { parameter: name.to_string(), value: f64::NAN, reason: "not a calibrated channel".to_string() })?;
                let c = &self.spec.calibrated[k];
                let t = c.scale.to_coord(self.spec.theta_fixed.to_si()[self.calibrated_index[k]]);
                Some((k, (t - d.t_min[k]) / (d.t_max[k] - d.t_min[k])))
            }
        };
        let xu: Vec<Vec<f64>> = xs.iter().map(|x| d.unit_x(&scaled(&x.to_si(), &self.spec.input_scales))).collect();
        let np = xs.len();
        let with_d = self.model.form == KohForm::WithDiscrepancy;
        let mut comps: Vec<Vec<(f64, f64)>> = vec![Vec::new(); np];
        let mut dcomps: Vec<Vec<(f64, f64)>> = vec![Vec::new(); np];
        for p in self.posterior.samples.iter().step_by(stride) {
            let mut p = p.clone();
            if let Some((k, u)) = reset_at {
                p.theta[k] = u;
            }
            let pr = self.model.predict(&p, &xu)?;
            let off = if with_d { np } else { 0 };
            for i in 0..np {
                let (mu_u, var_u) = (pr.mean[off + i], pr.cov[off + i][off + i]);
                let (mu_v, var_v, cvu) = if with_d { (pr.mean[i], pr.cov[i][i], pr.cov[i][off + i]) } else { (0.0, 0.0, 0.0) };
                let mut var = var_u + var_v + 2.0 * cvu;
                if observation_error {
                    var += 1.0 / (1.0 + 1e-8) / p.lam_os;
                }
                comps[i].push((d.y_mean + d.y_sd * (mu_u + mu_v), d.y_sd * var.max(0.0).sqrt()));
                dcomps[i].push((d.y_sd * mu_v, d.y_sd * var_v.max(0.0).sqrt()));
            }
        }
        let typed = |c: f64| L::Output::from_si(&[self.spec.output_scale.to_si(c)]);
        (0..np)
            .map(|i| {
                let c = &comps[i];
                let s = c.len() as f64;
                let mean = c.iter().map(|(m, _)| m).sum::<f64>() / s;
                let var = c.iter().map(|(m, sd)| sd * sd + (m - mean) * (m - mean)).sum::<f64>() / s;
                let dm = dcomps[i].iter().map(|(m, _)| m).sum::<f64>() / s;
                let dv = dcomps[i].iter().map(|(m, sd)| sd * sd + (m - dm) * (m - dm)).sum::<f64>() / s;
                Ok(KohPrediction {
                    median: typed(mixture_quantile(c, 0.5)?)?,
                    lower95: typed(mixture_quantile(c, 0.025)?)?,
                    upper95: typed(mixture_quantile(c, 0.975)?)?,
                    mean,
                    sd: var.sqrt(),
                    delta_mean: dm,
                    delta_sd: dv.sqrt(),
                })
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bayesian::koh::KohFree;
    use crate::fidelity::{CorrectedModel, Fidelity};
    use uom::si::f64::{Length, Ratio};
    use uom::si::length::meter;
    use uom::si::ratio::ratio;

    crate::quantities! {
        #[derive(Debug, Clone, Copy, PartialEq)]
        struct X { a: Length }
    }
    crate::quantities! {
        #[derive(Debug, Clone, Copy, PartialEq)]
        struct Th { k: Ratio, unused: Ratio }
    }
    crate::quantities! {
        #[derive(Debug, Clone, Copy, PartialEq)]
        struct Y { b: Length }
    }
    crate::quantities! {
        #[derive(Debug, Clone, Copy, PartialEq)]
        struct Y2 { b: Length, c: Length }
    }

    /// `y_LF = 1 + k a` (always positive on a ∈ [0, 1], k ∈ [0.5, 2]).
    #[derive(Debug, Clone)]
    struct Line;
    impl ParametricModel for Line {
        type Input = X;
        type Theta = Th;
        type Output = Y;
        fn parameter_space(&self) -> crate::fidelity::ParameterSpace<Th> {
            let t = |v: f64| Th { k: Ratio::new::<ratio>(v), unused: Ratio::new::<ratio>(v) };
            crate::fidelity::ParameterSpace::bounded(&t(0.5), &t(2.0)).unwrap()
        }
        fn evaluate(&self, x: &X, th: &Th) -> Result<Y> {
            Ok(Y { b: Length::new::<meter>(1.0 + th.k.value * x.a.value) })
        }
    }
    #[derive(Debug, Clone)]
    struct Two;
    impl ParametricModel for Two {
        type Input = X;
        type Theta = Th;
        type Output = Y2;
        fn parameter_space(&self) -> crate::fidelity::ParameterSpace<Th> {
            Line.parameter_space()
        }
        fn evaluate(&self, x: &X, _: &Th) -> Result<Y2> {
            Ok(Y2 { b: x.a, c: x.a })
        }
    }

    fn th(k: f64) -> Th {
        Th { k: Ratio::new::<ratio>(k), unused: Ratio::new::<ratio>(7.0) }
    }

    fn spec(form: KohForm) -> KohSpec<Th> {
        let s = |population, chain_length, seed| KohSampleConfig { free: KohFree::All, population, chain_length, seed };
        KohSpec {
            calibrated: vec![CalibratedParameter { name: "k".to_string(), scale: Scale::Log, prior: ThetaPrior::Normal { mean: 0.0, sd: 0.5 } }],
            theta_lower: th(0.5),
            theta_upper: th(2.0),
            theta_fixed: th(1.0),
            input_scales: vec![Scale::Linear],
            output_scale: Scale::Log,
            runs_per_input: 5,
            design_seed: 11,
            observation_error: Some(ObservationError { sd: 0.01, shape: 2.0 }),
            form,
            emulator_sampler: s(300, 3, 1),
            calibration_sampler: s(400, 5, 2),
        }
    }

    /// **Methodology.** The typed route end to end on an analytic pair: LF
    /// `1 + k a`, HF `1 + 1.4 a + 0.05 a²` (truth k ≈ 1.4 plus a small
    /// curvature δ), 9 training rows, 2 held out, log output, a log-scale k
    /// with prior N(0, 0.5²) on ln k. Checks: the held-out predictive
    /// interval brackets the HF value, the posterior k is above the derived
    /// 1, the no-δ recalibration runs on the same emulator, resetting k to
    /// its derived value moves the prediction away from the HF, the typed θ
    /// posterior feeds a `CorrectedModel`, and malformed specs are refused.
    ///
    /// **Prediction:** posterior median k in [1.2, 1.6]; both held-out HF
    /// values inside the 95 % interval. **Result (2026-11-04):** met. Median
    /// k 1.4376; HF 1.42450 in [1.40595, 1.45658] (median 1.43102), HF
    /// 2.15200 in [2.11358, 2.19203] (median 2.15189).
    #[test]
    fn the_typed_route_calibrates_an_analytic_pair() {
        let hf = |a: f64| 1.0 + 1.4 * a + 0.05 * a * a;
        let xs: Vec<X> = (0..11).map(|i| X { a: Length::new::<meter>(f64::from(i) / 10.0) }).collect();
        let ys: Vec<Y> = xs.iter().map(|x| Y { b: Length::new::<meter>(hf(x.a.value)) }).collect();
        let split: Vec<Split> = (0..11).map(|i| if i == 3 || i == 8 { Split::HeldOut } else { Split::Training }).collect();
        let data = Dataset::new(&xs, &ys, &split, Fidelity::new(2, "toy"), "unit test").unwrap();
        let cal = KohCalibration::calibrate(Line, &data, &xs, spec(KohForm::WithDiscrepancy)).unwrap();
        assert_eq!(cal.model().data.n(), 9);
        assert_eq!(cal.model().data.m(), 55);
        let ks: Vec<f64> = cal.theta_coordinates().iter().map(|t| t[0].exp()).collect();
        let mut sorted = ks.clone();
        sorted.sort_by(f64::total_cmp);
        let med = sorted[sorted.len() / 2];
        println!("posterior median k {med:.4}; emulator lam_uz {:.3}", cal.emulator().lam_uz);
        assert!((1.2..=1.6).contains(&med), "{med}");
        let held = [xs[3], xs[8]];
        let pred = cal.predict(&held, 4, true, None).unwrap();
        for (p, x) in pred.iter().zip(&held) {
            let y = hf(x.a.value);
            println!("a {:.1}: HF {y:.5}, median {:.5} [{:.5}, {:.5}], delta {:.2e} ± {:.2e}", x.a.value, p.median.b.value, p.lower95.b.value, p.upper95.b.value, p.delta_mean, p.delta_sd);
            assert!(p.lower95.b.value <= y && y <= p.upper95.b.value);
            assert!(p.sd > 0.0);
        }
        let reset = cal.predict(&held[1..], 4, false, Some("k")).unwrap();
        assert!(reset[0].median.b.value < pred[1].median.b.value);
        assert!(cal.predict(&held, 4, false, Some("unused")).is_err());
        assert!(cal.predict(&held, 0, false, None).is_err());
        let est = cal.theta_estimate().unwrap();
        assert_eq!(est.samples().len(), cal.posterior().samples.len());
        assert_eq!(est.point().unused.value, 7.0);
        let corrected = CorrectedModel::new(cal.lf().clone(), est, None);
        assert!(corrected.predict(&held[0]).unwrap().mean.b.value > 1.0);
        let nod = cal.recalibrate(KohForm::NoDiscrepancy).unwrap();
        assert_eq!(nod.emulator().beta_u, cal.emulator().beta_u);
        let p0 = nod.predict(&held, 4, true, None).unwrap();
        assert_eq!(p0[0].delta_mean, 0.0);
        // Refusals.
        assert!(KohCalibration::calibrate(Two, &Dataset::new(&xs, &xs.iter().map(|x| Y2 { b: x.a, c: x.a }).collect::<Vec<_>>(), &split, Fidelity::new(2, "t"), "t").unwrap(), &xs, spec(KohForm::WithDiscrepancy)).is_err());
        let mut bad = spec(KohForm::WithDiscrepancy);
        bad.calibrated[0].name = "nope".to_string();
        assert!(KohCalibration::calibrate(Line, &data, &xs, bad).is_err());
        let mut bad = spec(KohForm::WithDiscrepancy);
        bad.input_scales = vec![];
        assert!(KohCalibration::calibrate(Line, &data, &xs, bad).is_err());
        let mut bad = spec(KohForm::WithDiscrepancy);
        bad.input_scales = vec![Scale::Log]; // ln 0 at a = 0
        assert!(KohCalibration::calibrate(Line, &data, &xs, bad).is_err());
        assert_eq!(Scale::Linear.to_si(Scale::Linear.to_coord(2.0)), 2.0);
    }
}
