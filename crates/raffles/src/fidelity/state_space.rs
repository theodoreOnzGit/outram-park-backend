// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// NOT ported from FilterPy or RAVEN. Added 2026-11-04 for GitHub issue #1191
// (epic #1177), the sibling trait gh:#1194 planned, at the WORKSPACE
// MAINTAINER's direction; the crate owner's review is outstanding.

//! **State-space models and the typed route to the filters** (gh:#1191).
//!
//! [`StateSpaceModel`] is the third caller-implemented seam beside
//! [`HighFidelityModel`](super::HighFidelityModel) and
//! [`ParametricModel`](super::ParametricModel): a process model
//! `x_{k+1} = f(x_k, k)` and an observation operator `z_k = h(x_k, k)`, with
//! the state and the observation as `uom` records ([`Quantities`]). It is
//! consumed generically (`M: StateSpaceModel`), never as `dyn`.
//!
//! [`Assimilator`] is the closed set of filters RAFFLES provides (FilterPy's
//! EKF, UKF and EnKF, [`crate::filtering`]), run on such a model: typed in
//! and out, SI `f64` inside. Noise covariances and the prior covariance are
//! SI matrices in the state's channel order.
//!
//! **Written here, not FilterPy:** the EKF's Jacobians are central
//! differences of the model (FilterPy takes the caller's Jacobian), with the
//! step `h_j = relative_step · max(|x_j|, √P_jj)`; the NIS of each step is
//! computed by [`crate::filtering::nis`].
//!
//! **Synthetic data only** (`RESPONSIBLE_USE.md`).

use super::quantities::Quantities;
use crate::filtering::{
    nis, EnsembleKalmanFilter, ExtendedKalmanFilter, Matrix, MerweScaledSigmaPoints, UnscentedKalmanFilter,
};
use crate::openda::{enkf_update, DrawSource, LocalizationInput, Observations as OdaObservations, StochVector};
use crate::{RafflesError, Result};

/// A caller's state-space model, typed.
pub trait StateSpaceModel {
    /// The state record `x` (may include parameters being estimated).
    type State: Quantities;
    /// The observation record `z`.
    type Observation: Quantities;
    /// The noise-free process model from step `step` to `step + 1`.
    ///
    /// # Errors
    /// The model's own failure, as [`RafflesError::ModelEvaluation`].
    fn propagate(&self, x: &Self::State, step: usize) -> Result<Self::State>;
    /// The noise-free observation at step `step`.
    ///
    /// # Errors
    /// The model's own failure, as [`RafflesError::ModelEvaluation`].
    fn observe(&self, x: &Self::State, step: usize) -> Result<Self::Observation>;
}

/// A Gaussian state estimate: mean and standard deviation in the state's
/// own units, and the full SI covariance.
#[derive(Debug, Clone, PartialEq)]
pub struct Gaussian<S> {
    /// Mean.
    pub mean: S,
    /// Marginal standard deviations (√ of the diagonal), in the state's units.
    pub std_dev: S,
    /// The SI covariance, in channel order.
    pub covariance_si: Matrix,
}

impl<S: Quantities> Gaussian<S> {
    fn from_si(mean: &[f64], p: &Matrix) -> Result<Self> {
        Ok(Self { mean: S::from_si(mean)?, std_dev: S::from_si(&p.diag().iter().map(|v| v.sqrt()).collect::<Vec<f64>>())?, covariance_si: p.clone() })
    }
}

/// One predict-and-update cycle.
#[derive(Debug, Clone, PartialEq)]
pub struct AssimilationStep<S, O> {
    /// The step index passed to the model.
    pub step: usize,
    /// After predict.
    pub prior: Gaussian<S>,
    /// After update.
    pub posterior: Gaussian<S>,
    /// The filter's predicted observation.
    pub predicted_observation: O,
    /// Innovation `z − ẑ` (SI).
    pub innovation_si: Vec<f64>,
    /// Innovation covariance `S` (SI).
    pub innovation_covariance_si: Matrix,
    /// Normalised innovation squared `yᵀ S⁻¹ y`.
    pub nis: f64,
}

/// The filters RAFFLES runs on a [`StateSpaceModel`].
#[derive(Debug, Clone, PartialEq)]
pub enum Assimilator {
    /// FilterPy's EKF with central-difference Jacobians.
    Extended {
        /// The filter.
        filter: ExtendedKalmanFilter,
        /// Relative finite-difference step.
        relative_step: f64,
    },
    /// FilterPy's UKF with Merwe sigma points.
    Unscented(UnscentedKalmanFilter),
    /// FilterPy's EnKF with the LCG's own draws.
    Ensemble {
        /// The filter.
        filter: EnsembleKalmanFilter,
        /// The LCG state for the draws.
        seed: u64,
    },
    /// OpenDA's EnKF (gh:#1192, [`crate::openda::enkf_update`]): members
    /// propagated by the model plus a realisation of `N(0, Q)` (OpenDA's
    /// system noise), perturbed observations, no localisation. Its estimate
    /// is the ensemble mean and the ensemble covariance (1/(N − 1)); OpenDA
    /// itself reports the mean and the standard deviations.
    OpenDaEnsemble {
        /// Member states, SI.
        members: Vec<Vec<f64>>,
        /// The process noise per step.
        process_noise: StochVector,
        /// Observation standard deviations (R must be diagonal: OpenDA's
        /// observer is independent).
        obs_std: Vec<f64>,
        /// The draws.
        draws: DrawSource,
    },
}

/// `N(0, C)` as OpenDA builds it: independent when `C` is diagonal,
/// otherwise correlated with OpenDA's SVD square root.
fn zero_mean(c: &Matrix) -> Result<StochVector> {
    let n = c.rows();
    let diagonal = (0..n).all(|i| (0..n).all(|j| i == j || c.get(i, j) == 0.0));
    if diagonal {
        StochVector::independent(vec![0.0; n], c.diag().iter().map(|v| v.sqrt()).collect())
    } else {
        StochVector::correlated(vec![0.0; n], c.to_rows(), false)
    }
}

/// The ensemble mean and the 1/(N − 1) covariance.
fn ensemble_gaussian<S: Quantities>(members: &[Vec<f64>]) -> Result<Gaussian<S>> {
    let n = members.len() as f64;
    let mean = crate::openda::linalg::ensemble_average(members);
    let d = mean.len();
    let mut p = Matrix::zeros(d, d);
    for i in 0..d {
        for j in 0..d {
            p.set(i, j, members.iter().map(|x| (x[i] - mean[i]) * (x[j] - mean[j])).sum::<f64>() / (n - 1.0));
        }
    }
    Gaussian::from_si(&mean, &p)
}

fn check_square(m: &Matrix, n: usize) -> Result<()> {
    if m.rows() != n || m.cols() != n {
        return Err(RafflesError::DimensionMismatch { expected: n, found: m.rows() });
    }
    Ok(())
}

fn model_fx<M: StateSpaceModel>(model: &M, step: usize) -> impl Fn(&[f64]) -> Result<Vec<f64>> + '_ {
    move |x: &[f64]| Ok(model.propagate(&M::State::from_si(x)?, step)?.to_si())
}

fn model_hx<M: StateSpaceModel>(model: &M, step: usize) -> impl Fn(&[f64]) -> Result<Vec<f64>> + '_ {
    move |x: &[f64]| Ok(model.observe(&M::State::from_si(x)?, step)?.to_si())
}

fn jacobian<G: Fn(&[f64]) -> Result<Vec<f64>>>(g: G, x: &[f64], p: &Matrix, relative_step: f64) -> Result<Matrix> {
    let mut cols: Vec<Vec<f64>> = Vec::with_capacity(x.len());
    for j in 0..x.len() {
        let h = relative_step * x[j].abs().max(p.get(j, j).sqrt());
        let (mut a, mut b) = (x.to_vec(), x.to_vec());
        a[j] += h;
        b[j] -= h;
        let (ga, gb) = (g(&a)?, g(&b)?);
        cols.push(ga.iter().zip(&gb).map(|(u, v)| (u - v) / (2.0 * h)).collect());
    }
    let rows = cols.first().map_or(0, Vec::len);
    let mut m = Matrix::zeros(rows, x.len());
    for (j, c) in cols.iter().enumerate() {
        for (i, v) in c.iter().enumerate() {
            m.set(i, j, *v);
        }
    }
    Ok(m)
}

impl Assimilator {
    /// An EKF from a typed prior mean and SI covariances.
    ///
    /// # Errors
    /// On a shape mismatch, or a non-positive `relative_step`.
    pub fn extended<S: Quantities, O: Quantities>(x0: &S, p0: Matrix, q: Matrix, r: Matrix, relative_step: f64) -> Result<Self> {
        let (n, m) = (S::width(), O::width());
        check_square(&p0, n)?;
        check_square(&q, n)?;
        check_square(&r, m)?;
        if !(relative_step > 0.0) {
            return Err(RafflesError::InvalidParameter { parameter: "relative_step".into(), value: relative_step, reason: "must be positive".into() });
        }
        let mut f = ExtendedKalmanFilter::new(n, m)?;
        f.x = Matrix::column(&x0.to_si());
        f.p = p0;
        f.q = q;
        f.r = r;
        Ok(Self::Extended { filter: f, relative_step })
    }

    /// A UKF with Merwe points `(α, β, κ)`.
    ///
    /// # Errors
    /// On a shape mismatch.
    pub fn unscented<S: Quantities, O: Quantities>(x0: &S, p0: Matrix, q: Matrix, r: Matrix, alpha: f64, beta: f64, kappa: f64) -> Result<Self> {
        let (n, m) = (S::width(), O::width());
        check_square(&p0, n)?;
        check_square(&q, n)?;
        check_square(&r, m)?;
        let mut f = UnscentedKalmanFilter::new(n, m, 1.0, MerweScaledSigmaPoints::new(n, alpha, beta, kappa))?;
        f.x = x0.to_si();
        f.p = p0;
        f.q = q;
        f.r = r;
        Ok(Self::Unscented(f))
    }

    /// An EnKF of `members` members, its initial ensemble drawn from
    /// `N(x0, p0)` on the LCG at `seed`.
    ///
    /// # Errors
    /// On a shape mismatch or fewer than 2 members.
    pub fn ensemble<S: Quantities, O: Quantities>(x0: &S, p0: Matrix, q: Matrix, r: Matrix, members: usize, seed: u64) -> Result<Self> {
        let (n, m) = (S::width(), O::width());
        check_square(&p0, n)?;
        check_square(&q, n)?;
        check_square(&r, m)?;
        if members < 2 {
            return Err(RafflesError::InvalidParameter { parameter: "members".into(), value: members as f64, reason: "an ensemble needs at least 2".into() });
        }
        let mut seed = seed;
        let mut f = EnsembleKalmanFilter::new_drawn(&x0.to_si(), &p0, m, 1.0, members, &mut seed)?;
        f.q = q;
        f.r = r;
        Ok(Self::Ensemble { filter: f, seed })
    }

    /// OpenDA's EnKF of `members` members (gh:#1192): each member is `x₀` plus
    /// a realisation of `N(0, P₀)` (OpenDA's `stochInit`), drawn on the
    /// workspace LCG at `seed`.
    ///
    /// # Errors
    /// On a shape mismatch, a non-diagonal R, or fewer than 2 members.
    pub fn openda_ensemble<S: Quantities, O: Quantities>(x0: &S, p0: Matrix, q: Matrix, r: Matrix, members: usize, seed: u64) -> Result<Self> {
        let (n, m) = (S::width(), O::width());
        check_square(&p0, n)?;
        check_square(&q, n)?;
        check_square(&r, m)?;
        if members < 2 {
            return Err(RafflesError::InvalidParameter { parameter: "members".into(), value: members as f64, reason: "an ensemble needs at least 2".into() });
        }
        if (0..m).any(|i| (0..m).any(|j| i != j && r.get(i, j) != 0.0)) {
            return Err(RafflesError::InvalidParameter { parameter: "R".into(), value: 0.0, reason: "OpenDA's observer is independent: R must be diagonal".into() });
        }
        let mut draws = DrawSource::lcg(seed);
        let init = zero_mean(&p0)?;
        let x = x0.to_si();
        let mut ens = Vec::with_capacity(members);
        for _ in 0..members {
            let mut xi = x.clone();
            crate::openda::linalg::axpy(&mut xi, 1.0, &init.realization(&mut draws)?);
            ens.push(xi);
        }
        Ok(Self::OpenDaEnsemble { members: ens, process_noise: zero_mean(&q)?, obs_std: r.diag().iter().map(|v| v.sqrt()).collect(), draws })
    }

    /// The current estimate.
    ///
    /// # Errors
    /// If the state cannot be rebuilt from SI.
    pub fn estimate<S: Quantities>(&self) -> Result<Gaussian<S>> {
        match self {
            Self::Extended { filter, .. } => Gaussian::from_si(&filter.x.col(0), &filter.p),
            Self::Unscented(f) => Gaussian::from_si(&f.x, &f.p),
            Self::Ensemble { filter, .. } => Gaussian::from_si(&filter.x, &filter.p),
            Self::OpenDaEnsemble { members, .. } => ensemble_gaussian(members),
        }
    }

    /// Predict from `step − 1` to `step` with `model.propagate(·, step − 1)`
    /// (no predict at `step == 0`), then update on `z` with
    /// `model.observe(·, step)`.
    ///
    /// # Errors
    /// From the model or the filter.
    pub fn step<M: StateSpaceModel>(&mut self, model: &M, step: usize, z: &M::Observation) -> Result<AssimilationStep<M::State, M::Observation>> {
        let zs = z.to_si();
        let predict = step > 0;
        let prev = step.saturating_sub(1);
        let (prior, zp, y, s, posterior) = match self {
            Self::Extended { filter, relative_step } => {
                if predict {
                    filter.f = jacobian(model_fx(model, prev), &filter.x.col(0), &filter.p, *relative_step)?;
                    filter.predict_x_with(model_fx(model, prev))?;
                }
                let prior = Gaussian::from_si(&filter.x.col(0), &filter.p)?;
                let (rs, p) = (*relative_step, filter.p.clone());
                filter.update(&zs, |x| jacobian(model_hx(model, step), x, &p, rs), model_hx(model, step))?;
                let y = filter.y.col(0);
                let zp: Vec<f64> = zs.iter().zip(&y).map(|(a, b)| a - b).collect();
                (prior, zp, y, filter.s.clone(), Gaussian::from_si(&filter.x.col(0), &filter.p)?)
            }
            Self::Unscented(f) => {
                if predict {
                    let fx = model_fx(model, prev);
                    f.predict(|x, _| fx(x))?;
                } else {
                    f.sigmas_f = f.points.sigma_points(&f.x, &f.p)?;
                }
                let prior = Gaussian::from_si(&f.x, &f.p)?;
                f.update(&zs, model_hx(model, step))?;
                let zp: Vec<f64> = zs.iter().zip(&f.y).map(|(a, b)| a - b).collect();
                (prior, zp, f.y.clone(), f.s.clone(), Gaussian::from_si(&f.x, &f.p)?)
            }
            Self::Ensemble { filter, seed } => {
                if predict {
                    let fx = model_fx(model, prev);
                    filter.predict(|x, _| fx(x), seed)?;
                }
                let prior = Gaussian::from_si(&filter.x, &filter.p)?;
                let hx = model_hx(model, step);
                let mut hs = Matrix::zeros(filter.n, zs.len());
                for i in 0..filter.n {
                    hs.set_row(i, &hx(filter.sigmas.row(i))?)?;
                }
                let zp = crate::filtering::numpy_ops::mean_rows(&hs)?;
                filter.update(&zs, model_hx(model, step), seed)?;
                let y: Vec<f64> = zs.iter().zip(&zp).map(|(a, b)| a - b).collect();
                (prior, zp, y, filter.s.clone(), Gaussian::from_si(&filter.x, &filter.p)?)
            }
            Self::OpenDaEnsemble { members, process_noise, obs_std, draws } => {
                if predict {
                    let fx = model_fx(model, prev);
                    for xi in members.iter_mut() {
                        let mut x = fx(xi)?;
                        crate::openda::linalg::axpy(&mut x, 1.0, &process_noise.realization(draws)?);
                        *xi = x;
                    }
                }
                let prior = ensemble_gaussian(members)?;
                let hx = model_hx(model, step);
                let preds = members.iter().map(|x| hx(x)).collect::<Result<Vec<_>>>()?;
                let m = zs.len();
                let obs = OdaObservations::new(vec![0.0; m], (0..m).map(|i| i as f64).collect(), zs.clone(), obs_std.clone(), None)?;
                let up = enkf_update(members.clone(), preds, &obs, &LocalizationInput::None, draws)?;
                for (xi, d) in members.iter_mut().zip(&up.deltas) {
                    crate::openda::linalg::axpy(xi, 1.0, d);
                }
                let y: Vec<f64> = zs.iter().zip(&up.predicted_mean).map(|(a, b)| a - b).collect();
                (prior, up.predicted_mean.clone(), y, Matrix::from_rows(&up.innovation_covariance)?, ensemble_gaussian(members)?)
            }
        };
        Ok(AssimilationStep {
            step,
            prior,
            posterior,
            predicted_observation: M::Observation::from_si(&zp)?,
            nis: nis(&y, &s)?,
            innovation_si: y,
            innovation_covariance_si: s,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uom::si::f64::{Length, Velocity};
    use uom::si::length::meter;
    use uom::si::velocity::meter_per_second;

    crate::quantities! {
        /// Position and velocity.
        #[derive(Debug, Clone, Copy, PartialEq)]
        pub struct Kinematic { pub pos: Length, pub vel: Velocity }
    }
    crate::quantities! {
        /// A position fix.
        #[derive(Debug, Clone, Copy, PartialEq)]
        pub struct Fix { pub pos: Length }
    }

    /// Constant velocity at dt = 0.1 s, position measured.
    struct Cv;
    impl StateSpaceModel for Cv {
        type State = Kinematic;
        type Observation = Fix;
        fn propagate(&self, x: &Kinematic, _: usize) -> Result<Kinematic> {
            Ok(Kinematic { pos: x.pos + x.vel * uom::si::f64::Time::new::<uom::si::time::second>(0.1), vel: x.vel })
        }
        fn observe(&self, x: &Kinematic, _: usize) -> Result<Fix> {
            Ok(Fix { pos: x.pos })
        }
    }

    /// On a linear model the typed EKF (finite-difference Jacobians, exact
    /// for a linear map up to round-off) and the typed UKF equal FilterPy's
    /// KalmanFilter on the same matrices, to 1e-9 relative, and the EnKF
    /// (N 400) to within its sampling error. Units come back as uom.
    #[test]
    fn typed_filters_on_a_linear_model_agree_with_the_kf() {
        let q = Matrix::from_rows(&[vec![2.5e-6, 5e-5], vec![5e-5, 1e-3]]).unwrap();
        let r = Matrix::diagonal(&[0.25]);
        let p0 = Matrix::diagonal(&[4.0, 1.0]);
        let x0 = Kinematic { pos: Length::new::<meter>(0.0), vel: Velocity::new::<meter_per_second>(1.0) };
        let mut kf = crate::filtering::KalmanFilter::new(2, 1).unwrap();
        kf.f = Matrix::from_rows(&[vec![1.0, 0.1], vec![0.0, 1.0]]).unwrap();
        kf.h = Matrix::from_rows(&[vec![1.0, 0.0]]).unwrap();
        kf.q = q.clone();
        kf.r = r.clone();
        kf.p = p0.clone();
        kf.x = Matrix::column(&[0.0, 1.0]);
        let mut ekf = Assimilator::extended::<Kinematic, Fix>(&x0, p0.clone(), q.clone(), r.clone(), 1e-3).unwrap();
        let mut ukf = Assimilator::unscented::<Kinematic, Fix>(&x0, p0.clone(), q.clone(), r.clone(), 0.5, 2.0, 1.0).unwrap();
        let mut enkf = Assimilator::ensemble::<Kinematic, Fix>(&x0, p0.clone(), q.clone(), r.clone(), 400, 7).unwrap();
        let mut oda = Assimilator::openda_ensemble::<Kinematic, Fix>(&x0, p0.clone(), q.clone(), r.clone(), 400, 7).unwrap();
        let mut nis_sum = 0.0;
        for k in 0..30usize {
            let z = Fix { pos: Length::new::<meter>(0.1 * k as f64 + 0.3 * ((k as f64 * 0.77) % 1.0 - 0.5)) };
            if k > 0 {
                kf.predict(None).unwrap();
            }
            kf.update(&z.to_si()).unwrap();
            let se = ekf.step(&Cv, k, &z).unwrap();
            let su = ukf.step(&Cv, k, &z).unwrap();
            let sn = enkf.step(&Cv, k, &z).unwrap();
            let so = oda.step(&Cv, k, &z).unwrap();
            let mo = so.posterior.mean.to_si();
            assert!((mo[0] - kf.x.get(0, 0)).abs() < 4.0 * kf.p.get(0, 0).sqrt() / 20.0 + 0.02, "openda enkf k{k}");
            assert!(so.nis.is_finite() && so.posterior.covariance_si.get(0, 0) > 0.0);
            nis_sum += su.nis;
            for (est, tol) in [(&se.posterior, 1e-9), (&su.posterior, 1e-9)] {
                let m = est.mean.to_si();
                for i in 0..2 {
                    assert!((m[i] - kf.x.get(i, 0)).abs() <= tol * (1.0 + kf.x.get(i, 0).abs()), "k{k} {m:?} {:?}", kf.x);
                    for j in 0..2 {
                        assert!((est.covariance_si.get(i, j) - kf.p.get(i, j)).abs() <= tol * (1.0 + kf.p.get(i, j).abs()));
                    }
                }
            }
            let m = sn.posterior.mean.to_si();
            assert!((m[0] - kf.x.get(0, 0)).abs() < 4.0 * kf.p.get(0, 0).sqrt() / 20.0 + 0.02, "enkf k{k}");
            assert!(se.nis.is_finite() && sn.nis.is_finite());
            assert_eq!(su.predicted_observation.pos.get::<meter>() + su.innovation_si[0], z.pos.get::<meter>());
        }
        assert!(nis_sum / 30.0 < 5.0);
        let e: Gaussian<Kinematic> = ukf.estimate().unwrap();
        assert!(e.std_dev.pos.get::<meter>() > 0.0 && e.std_dev.pos.get::<meter>() < 0.5);
        let _: Gaussian<Kinematic> = ekf.estimate().unwrap();
        let _: Gaussian<Kinematic> = enkf.estimate().unwrap();
        let _: Gaussian<Kinematic> = oda.estimate().unwrap();
        assert!(Assimilator::openda_ensemble::<Kinematic, Fix>(&x0, p0.clone(), q.clone(), r.clone(), 1, 7).is_err());
        let rr = Matrix::from_rows(&[vec![0.25]]).unwrap();
        assert!(Assimilator::openda_ensemble::<Kinematic, Fix>(&x0, p0.clone(), q.clone(), rr, 10, 7).is_ok());
        assert!(Assimilator::extended::<Kinematic, Fix>(&x0, p0.clone(), q.clone(), r.clone(), 0.0).is_err());
        assert!(Assimilator::unscented::<Kinematic, Fix>(&x0, r.clone(), q.clone(), r.clone(), 0.5, 2.0, 1.0).is_err());
        assert!(Assimilator::ensemble::<Kinematic, Fix>(&x0, p0, q, r, 1, 7).is_err());
    }
}
