// ---------------------------------------------------------------------------
// Ported from OpenDA (open data assimilation toolbox).
//
//   Upstream project: OpenDA, OpenDA Association
//   Upstream repo:    https://github.com/OpenDA-Association/OpenDA
//   Upstream files:   algorithms/java/src/org/openda/algorithms/kalmanFilter/EnKF.java
//                       (getEnsembleVectorsState/Prediction, computeGainMatrix,
//                        applyLocalizationToGain, localizationHamill,
//                        localizationZhang, updateModelWithGain, updateMainModel,
//                        analysis)
//                     .../kalmanFilter/EnsembleVectors.java
//                     .../kalmanFilter/AutoLocalizationZhang2011.java
//                     .../kalmanFilter/AbstractSequentialEnsembleAlgorithm.java
//                       (LocalizationMethodType)
//   Upstream commit:  e51565c2c8def7d35f8b378a9ac118edbd98a94d
//   Accessed:         2026-11-04
//
//   Copyright (c) 2012 OpenDA Association. All rights reserved.
//   OpenDA is free software: you can redistribute it and/or modify it under
//   the terms of the GNU Lesser General Public License as published by the
//   Free Software Foundation, either version 3 of the License, or (at your
//   option) any later version. OpenDA is distributed in the hope that it will
//   be useful, but WITHOUT ANY WARRANTY; without even the implied warranty of
//   MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU Lesser
//   General Public License for more details (LICENSE-LGPL-3.0-OPENDA).
//
// This Rust translation is part of RAFFLES / Outram Park, conveyed under
// AGPL-3.0-only as a combined work (GPLv3 section 13 / AGPLv3 section 13).
//
// Translation notes: the analysis is split into `enkf_update`, which works on
// plain state and prediction vectors (so a caller's model needs no wrapper),
// and `enkf_analysis`, which gathers them from `OdaStochModel` members and
// applies the result as OpenDA does. Ported faithfully, NOT fixed (gh:#1192
// record section 3): the in-place 1/sqrt(q-1) ... sqrt(q-1) round trip of the
// state deviations; the Zhang bootstrap members being TWICE the deviation;
// `EnsembleVectors` removing the mean from member 1 onward only. Not ported:
// result writing, timers, gain storage to file, the time smoother of the gain
// (`timeRegularisationPerDay`), and computeHK / pred_a_linear (output only).
// ---------------------------------------------------------------------------

//! **OpenDA's ensemble Kalman filter** with its localisation options
//! (gh:#1192).

use super::jama::Rows;
use super::linalg::{self, axpy, ensemble_average, remove_average};
use super::models::{Observations, OdaStochModel};
use super::noise::DrawSource;
use crate::{RafflesError, Result};

/// OpenDA's `localization` options for the EnKF.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Localization {
    /// No localisation.
    None,
    /// Hamill et al. (2001): Schur product of each gain column with the
    /// model's mask ([`OdaStochModel::observed_localization`]).
    Hamill,
    /// Zhang & Oliver (2011): weights from bootstrap gains.
    AutoZhang {
        /// Bootstrap gains (OpenDA: 50).
        n_bootstrap: usize,
        /// σ²_α (OpenDA: 0.36, the paper's optimum).
        sigma_alpha2: f64,
    },
}

impl Localization {
    /// OpenDA's `autoZhang` settings.
    #[must_use]
    pub fn auto_zhang() -> Self {
        Self::AutoZhang { n_bootstrap: 50, sigma_alpha2: 0.36 }
    }
}

/// The localisation an [`enkf_update`] applies, with its input.
#[derive(Debug, Clone, PartialEq)]
pub enum LocalizationInput {
    /// None.
    None,
    /// Hamill: one mask per observation, each the state's length.
    Hamill(Vec<Vec<f64>>),
    /// Zhang & Oliver, with OpenDA's settings.
    AutoZhang {
        /// Bootstrap gains.
        n_bootstrap: usize,
        /// σ²_α.
        sigma_alpha2: f64,
    },
}

/// OpenDA's `EnsembleVectors`: deviations and mean.
#[derive(Debug, Clone, PartialEq)]
pub struct EnsembleVectors {
    /// Members with the mean removed.
    pub ensemble: Vec<Vec<f64>>,
    /// The mean.
    pub mean: Vec<f64>,
}

impl EnsembleVectors {
    /// `getEnsembleVectorsState`/`Prediction`: `ensembleAverage`, then
    /// `removeAverage` from **every** member.
    #[must_use]
    pub fn from_members(mut members: Vec<Vec<f64>>) -> Self {
        let mean = ensemble_average(&members);
        remove_average(&mut members, &mean);
        Self { ensemble: members, mean }
    }

    /// `new EnsembleVectors(IVector[])`: the mean of all members, removed
    /// from member **1** onward only (OpenDA's loops start at 1).
    #[must_use]
    pub fn as_openda_constructor(mut members: Vec<Vec<f64>>) -> Self {
        let mean = ensemble_average(&members);
        for m in members.iter_mut().skip(1) {
            axpy(m, -1.0, &mean);
        }
        Self { ensemble: members, mean }
    }
}

/// One EnKF analysis on vectors.
#[derive(Debug, Clone, PartialEq)]
pub struct EnkfUpdate {
    /// The increment to add to each member (`axpyOnState(1.0, ·)`).
    pub deltas: Vec<Vec<f64>>,
    /// Gain columns, one per observation, before localisation.
    pub gain_pre: Vec<Vec<f64>>,
    /// Gain columns after localisation.
    pub gain_post: Vec<Vec<f64>>,
    /// `D = PRED·PREDᵀ/(q−1) + R`.
    pub innovation_covariance: Rows,
    /// The forecast mean of the predictions.
    pub predicted_mean: Vec<f64>,
}

/// `computeGainMatrix`: the gain columns `K_i = Σ_j E(j,i)·X_j`, with
/// `E = PREDᵀ·inv(D)`, the state deviations scaled by `1/√(q−1)` and
/// back by `√(q−1)` **in place**, as OpenDA does.
///
/// # Errors
/// Shapes, or the SVD inverse's.
pub fn compute_gain(obs_std: &[f64], preds: &EnsembleVectors, states: &mut EnsembleVectors) -> Result<(Vec<Vec<f64>>, Rows)> {
    let m = obs_std.len();
    let q = states.ensemble.len();
    if preds.ensemble.len() != q || preds.ensemble.iter().any(|p| p.len() != m) {
        return Err(RafflesError::DimensionMismatch { expected: m, found: preds.ensemble.first().map_or(0, Vec::len) });
    }
    if q < 2 {
        return Err(RafflesError::InvalidParameter { parameter: "ensemble size".into(), value: q as f64, reason: "must be at least 2".into() });
    }
    let sqrt_qmin1 = (q as f64 - 1.0).sqrt();
    let mut pred_mat = linalg::from_columns(&preds.ensemble);
    linalg::scale_matrix(&mut pred_mat, (1.0 / (q as f64 - 1.0)).sqrt());
    let mut d = linalg::zeros(m, m);
    linalg::multiply(&mut d, 1.0, &pred_mat, &pred_mat, 0.0, false, true)?;
    let sqrt_r = linalg::diag_matrix(obs_std);
    linalg::multiply(&mut d, 1.0, &sqrt_r, &sqrt_r, 1.0, false, true)?;
    for x in &mut states.ensemble {
        linalg::scale(x, 1.0 / sqrt_qmin1);
    }
    let inverse_d = linalg::inverse(&d)?;
    let mut e = linalg::zeros(q, m);
    linalg::multiply(&mut e, 1.0, &pred_mat, &inverse_d, 0.0, true, false)?;
    let mut k = Vec::with_capacity(m);
    for i in 0..m {
        let mut ki: Vec<f64> = states.ensemble[0].iter().map(|v| 0.0 * v).collect();
        for j in 0..q {
            axpy(&mut ki, e[j][i], &states.ensemble[j]);
        }
        k.push(ki);
    }
    for x in &mut states.ensemble {
        linalg::scale(x, sqrt_qmin1);
    }
    Ok((k, d))
}

/// Zhang & Oliver's weights (`AutoLocalizationZhang2011`), ported as
/// written: each bootstrap member is the drawn deviation **added to
/// itself**, and the bootstrap ensemble is built with
/// [`EnsembleVectors::as_openda_constructor`].
///
/// # Errors
/// From [`compute_gain`], the draws, or a negative C² (OpenDA throws).
pub fn zhang_weights(obs_std: &[f64], preds: &EnsembleVectors, states: &EnsembleVectors, n_bootstrap: usize, sigma_alpha2: f64, draws: &mut DrawSource) -> Result<Vec<Vec<f64>>> {
    let n = states.ensemble.len();
    let mut gains: Vec<Vec<Vec<f64>>> = Vec::with_capacity(n_bootstrap);
    for _ in 0..n_bootstrap {
        let mut bv = Vec::with_capacity(n);
        let mut bp = Vec::with_capacity(n);
        for _ in 0..n {
            let r = draws.int_below(n)?;
            let mut v = states.ensemble[r].clone();
            axpy(&mut v, 1.0, &states.ensemble[r]);
            let mut p = preds.ensemble[r].clone();
            axpy(&mut p, 1.0, &preds.ensemble[r]);
            bv.push(v);
            bp.push(p);
        }
        let mut be = EnsembleVectors::as_openda_constructor(bv);
        let bpred = EnsembleVectors::as_openda_constructor(bp);
        gains.push(compute_gain(obs_std, &bpred, &mut be)?.0);
    }
    let alpha = 1.0 / n_bootstrap as f64;
    let n_obs = gains[0].len();
    let zero: Vec<f64> = gains[0][0].iter().map(|_| 0.0).collect();
    let mut mean = vec![zero.clone(); n_obs];
    for g in &gains {
        for (o, mo) in mean.iter_mut().enumerate() {
            axpy(mo, alpha, &g[o]);
        }
    }
    for g in &mut gains {
        for (o, go) in g.iter_mut().enumerate() {
            axpy(go, -1.0, &mean[o]);
            let c = go.clone();
            linalg::pointwise_multiply(go, &c);
        }
    }
    for mo in &mut mean {
        let c = mo.clone();
        linalg::pointwise_multiply(mo, &c);
    }
    let mut sigma2 = vec![zero.clone(); n_obs];
    for g in &gains {
        for (o, so) in sigma2.iter_mut().enumerate() {
            axpy(so, alpha, &g[o]);
        }
    }
    let wrk = vec![1.0e-16; zero.len()];
    for (o, so) in sigma2.iter_mut().enumerate() {
        axpy(&mut mean[o], 1.0, &wrk);
        linalg::pointwise_divide(so, &mean[o]);
    }
    if let Some(v) = sigma2.iter().flatten().find(|v| **v < 0.0) {
        return Err(RafflesError::InvalidParameter { parameter: "Zhang C²".into(), value: *v, reason: "negative (OpenDA throws)".into() });
    }
    let beta = 1.0 + 1.0 / sigma_alpha2;
    let mut rho = Vec::with_capacity(n_obs);
    for mut c2 in sigma2 {
        linalg::scale(&mut c2, beta);
        let ones = vec![1.0; c2.len()];
        axpy(&mut c2, 1.0, &ones);
        let mut r = ones;
        linalg::pointwise_divide(&mut r, &c2);
        rho.push(r);
    }
    Ok(rho)
}

/// OpenDA's EnKF analysis on vectors: `states` and `preds` are the
/// members' forecast states and predictions of `obs`. Gain, localisation,
/// then each member's increment from perturbed observations (one draw per
/// observation per member, in member order).
///
/// # Errors
/// Shapes, the inverse, or the draws.
pub fn enkf_update(states: Vec<Vec<f64>>, preds: Vec<Vec<f64>>, obs: &Observations, localization: &LocalizationInput, draws: &mut DrawSource) -> Result<EnkfUpdate> {
    let mut sv = EnsembleVectors::from_members(states);
    let pv = EnsembleVectors::from_members(preds);
    let (gain_pre, d) = compute_gain(&obs.std, &pv, &mut sv)?;
    let mut k = gain_pre.clone();
    match localization {
        LocalizationInput::None => {}
        LocalizationInput::Hamill(rho) => {
            if rho.len() != k.len() {
                return Err(RafflesError::DimensionMismatch { expected: k.len(), found: rho.len() });
            }
            for (ki, ri) in k.iter_mut().zip(rho) {
                linalg::pointwise_multiply(ki, ri);
            }
        }
        LocalizationInput::AutoZhang { n_bootstrap, sigma_alpha2 } => {
            let rho = zhang_weights(&obs.std, &pv, &sv, *n_bootstrap, *sigma_alpha2, draws)?;
            for (ki, ri) in k.iter_mut().zip(&rho) {
                linalg::pointwise_multiply(ki, ri);
            }
        }
    }
    let mut deltas = Vec::with_capacity(sv.ensemble.len());
    for i in 0..sv.ensemble.len() {
        let mut innovation = obs.realization(draws)?;
        axpy(&mut innovation, -1.0, &pv.ensemble[i]);
        axpy(&mut innovation, -1.0, &pv.mean);
        let mut delta: Vec<f64> = sv.ensemble[0].iter().map(|v| 0.0 * v).collect();
        for (j, kj) in k.iter().enumerate() {
            axpy(&mut delta, innovation[j], kj);
        }
        deltas.push(delta);
    }
    Ok(EnkfUpdate { deltas, gain_pre, gain_post: k, innovation_covariance: d, predicted_mean: pv.mean })
}

/// `updateMainModel` (also EnSR's and the particle filter's): `main +=
/// 1.0·(mean − main)`, the mean of the members' current states.
pub fn move_main_to_mean<M: OdaStochModel>(members: &[M], main: &mut M) {
    let states: Vec<Vec<f64>> = members.iter().map(OdaStochModel::state).collect();
    let mut delta = ensemble_average(&states);
    axpy(&mut delta, -1.0, &main.state());
    main.axpy_on_state(1.0, &delta);
}

/// OpenDA's `EnKF.analysis` on model members: gathers states and
/// predictions, runs [`enkf_update`] (Hamill's mask from member 0),
/// applies the increments, and moves `main` to the analysis mean.
///
/// # Errors
/// From the models or [`enkf_update`].
pub fn enkf_analysis<M: OdaStochModel>(members: &mut [M], main: &mut M, obs: &Observations, localization: Localization, draws: &mut DrawSource) -> Result<EnkfUpdate> {
    let states: Vec<Vec<f64>> = members.iter().map(OdaStochModel::state).collect();
    let preds = members.iter().map(|m| m.observed_values(obs)).collect::<Result<Vec<_>>>()?;
    let loc = match localization {
        Localization::None => LocalizationInput::None,
        Localization::Hamill => LocalizationInput::Hamill(members[0].observed_localization(obs)),
        Localization::AutoZhang { n_bootstrap, sigma_alpha2 } => LocalizationInput::AutoZhang { n_bootstrap, sigma_alpha2 },
    };
    let up = enkf_update(states, preds, obs, &loc, draws)?;
    for (m, d) in members.iter_mut().zip(&up.deltas) {
        m.axpy_on_state(1.0, d);
    }
    move_main_to_mean(members, main);
    Ok(up)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// With one observation of a scalar state, the gain is the sample
    /// covariance over (sample variance + R); Hamill scales it; Zhang's
    /// weights lie in (0, 1]; the bootstrap constructor leaves member 0
    /// un-centred; shape and size errors.
    #[test]
    fn gain_and_localisation() {
        let states = vec![vec![1.0, 10.0], vec![2.0, 20.0], vec![4.0, 40.0]];
        let preds: Vec<Vec<f64>> = states.iter().map(|s| vec![s[0]]).collect();
        let obs = Observations::new(vec![0.0], vec![0.0], vec![3.0], vec![1.0], None).unwrap();
        let mut d = DrawSource::injected(vec![0.0; 3], vec![], vec![]);
        let up = enkf_update(states.clone(), preds.clone(), &obs, &LocalizationInput::None, &mut d).unwrap();
        let mean = 7.0 / 3.0;
        let s2: f64 = states.iter().map(|s| (s[0] - mean) * (s[0] - mean)).sum::<f64>() / 2.0;
        assert!((up.gain_pre[0][0] - s2 / (s2 + 1.0)).abs() < 1e-12 && (up.gain_pre[0][1] - 10.0 * s2 / (s2 + 1.0)).abs() < 1e-11);
        assert!((up.innovation_covariance[0][0] - (s2 + 1.0)).abs() < 1e-12);
        let mut d = DrawSource::injected(vec![0.0; 3], vec![], vec![]);
        let h = enkf_update(states.clone(), preds.clone(), &obs, &LocalizationInput::Hamill(vec![vec![1.0, 0.5]]), &mut d).unwrap();
        assert_eq!(h.gain_post[0][1], h.gain_pre[0][1] * 0.5);
        assert!(enkf_update(states.clone(), preds.clone(), &obs, &LocalizationInput::Hamill(vec![]), &mut d).is_err());
        let mut z = DrawSource::lcg(5);
        let zh = enkf_update(states.clone(), preds.clone(), &obs, &LocalizationInput::AutoZhang { n_bootstrap: 20, sigma_alpha2: 0.36 }, &mut z).unwrap();
        for (a, b) in zh.gain_post[0].iter().zip(&zh.gain_pre[0]) {
            let r = a / b;
            assert!(r > 0.0 && r <= 1.0 + 1e-12, "{r}");
        }
        let c = EnsembleVectors::as_openda_constructor(vec![vec![1.0], vec![3.0]]);
        assert_eq!(c.ensemble, vec![vec![1.0], vec![1.0]]);
        assert!(enkf_update(vec![vec![1.0]], vec![vec![1.0]], &obs, &LocalizationInput::None, &mut d).is_err());
        assert!(enkf_update(states, vec![vec![1.0, 2.0]; 3], &obs, &LocalizationInput::None, &mut d).is_err());
        assert_eq!(Localization::auto_zhang(), Localization::AutoZhang { n_bootstrap: 50, sigma_alpha2: 0.36 });
    }
}
