// ---------------------------------------------------------------------------
// Ported from OpenDA (open data assimilation toolbox).
//
//   Upstream project: OpenDA, OpenDA Association
//   Upstream repo:    https://github.com/OpenDA-Association/OpenDA
//   Upstream files:   algorithms/java/src/org/openda/algorithms/kalmanFilter/EnSR.java (analysis)
//                     .../kalmanFilter/ParticleFilter.java (analysis, residualResampling,
//                       drawFromDiscreteDistribution)
//                     .../kalmanFilter/SteadyStateFilter.java (analysis)
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
// Translation notes: as for enkf.rs, each analysis has a vector-level core
// and a model-level wrapper. Ported faithfully: EnSR's I + psi'psi (the code,
// not its comment's minus); the particle filter's residual resampling only,
// with index 0 returned when a uniform exceeds every cumulative weight; the
// steady-state filter's per-observation gain columns and skip factor. Gain
// columns are matched by the observation's `index` (OpenDA matches
// "id:offset-seconds"; offsets other than 0 are not ported). Not ported: the
// steady-state filter's missing-observation HK compensation, gain
// replacement in time, and reading gains from files.
// ---------------------------------------------------------------------------

//! **OpenDA's ensemble square-root filter, particle filter and steady-state
//! filter** (gh:#1192).

use super::enkf::move_main_to_mean;
use super::linalg::{self, axpy, ensemble_average, remove_average};
use super::models::{Observations, OdaStochModel};
use super::noise::DrawSource;
use crate::{RafflesError, Result};

/// EnSR (Sakov 2006, symmetric transform) on vectors: OpenDA's increment
/// `xChange = x̄_a − x_i + √(q−1)·(L_f·T)_i` for each member.
///
/// # Errors
/// Shapes, or the SVDs'.
pub fn ensr_update(states: &[Vec<f64>], preds: &[Vec<f64>], obs: &Observations) -> Result<Vec<Vec<f64>>> {
    let q = states.len();
    let m = obs.len();
    if q < 2 || preds.len() != q || preds.iter().any(|p| p.len() != m) {
        return Err(RafflesError::DimensionMismatch { expected: q.max(2), found: preds.len() });
    }
    let mut xi = states.to_vec();
    let mut pred = preds.to_vec();
    let xi_avg = ensemble_average(&xi);
    remove_average(&mut xi, &xi_avg);
    let pred_avg = ensemble_average(&pred);
    remove_average(&mut pred, &pred_avg);
    let fac = (1.0 / (q as f64 - 1.0)).sqrt();
    let mut pred_mat = linalg::from_columns(&pred);
    linalg::scale_matrix(&mut pred_mat, fac);
    let mut phi = linalg::zeros(m, m);
    linalg::multiply(&mut phi, 1.0, &pred_mat, &pred_mat, 0.0, false, true)?;
    let sqrt_r = linalg::diag_matrix(&obs.std);
    linalg::multiply(&mut phi, 1.0, &sqrt_r, &sqrt_r, 1.0, false, true)?;
    let mut w = linalg::zeros(q, m);
    let inverse_phi = linalg::inverse(&phi)?;
    linalg::multiply(&mut w, 1.0, &pred_mat, &inverse_phi, 0.0, true, false)?;
    let mut lf = linalg::from_columns(&xi);
    linalg::scale_matrix(&mut lf, fac);
    let n = xi_avg.len();
    let mut k = linalg::zeros(n, m);
    linalg::multiply(&mut k, 1.0, &lf, &w, 0.0, false, false)?;
    let mut innovation = obs.value.clone();
    axpy(&mut innovation, -1.0, &pred_avg);
    let mut xi_new_avg = xi_avg.clone();
    linalg::right_multiply(&k, 1.0, &innovation, 1.0, &mut xi_new_avg);
    let pred_norm: Vec<Vec<f64>> = pred.iter().map(|p| p.iter().zip(&obs.std).map(|(a, s)| a / s).collect()).collect();
    let mut psi = linalg::from_columns(&pred_norm);
    linalg::scale_matrix(&mut psi, fac);
    let mut chi = linalg::eye(q);
    linalg::multiply(&mut chi, 1.0, &psi, &psi, 1.0, true, false)?;
    let (u, d, v) = linalg::svd(&chi)?;
    let inv_sqrt_d: Vec<f64> = linalg::diag(&d).iter().map(|x| (1.0 / x).sqrt()).collect();
    let ts = linalg::mult(&linalg::mult(&u, &linalg::diag_matrix(&inv_sqrt_d), false, false)?, &super::jama::transpose(&v), false, false)?;
    let l_a = linalg::columns(&linalg::mult(&lf, &ts, false, false)?);
    let sq = (q as f64 - 1.0).sqrt();
    Ok((0..q)
        .map(|i| {
            let mut change = xi_new_avg.clone();
            axpy(&mut change, -1.0, &states[i]);
            axpy(&mut change, sq, &l_a[i]);
            change
        })
        .collect())
}

/// OpenDA's `EnSR.analysis` on model members.
///
/// # Errors
/// From the models or [`ensr_update`].
pub fn ensr_analysis<M: OdaStochModel>(members: &mut [M], main: &mut M, obs: &Observations) -> Result<()> {
    let states: Vec<Vec<f64>> = members.iter().map(OdaStochModel::state).collect();
    let preds = members.iter().map(|m| m.observed_values(obs)).collect::<Result<Vec<_>>>()?;
    let changes = ensr_update(&states, &preds, obs)?;
    for (m, c) in members.iter_mut().zip(&changes) {
        m.axpy_on_state(1.0, c);
    }
    move_main_to_mean(members, main);
    Ok(())
}

/// The particle filter's normalised weights and residual-resampling index
/// (`index[i]` is the member member `i` becomes a copy of).
#[derive(Debug, Clone, PartialEq)]
pub struct Resampling {
    /// Normalised weights.
    pub weights: Vec<f64>,
    /// Parent of each new member.
    pub index: Vec<usize>,
}

/// Residual resampling (`ParticleFilter.residualResampling`): `⌊N w_i⌋`
/// copies each, the rest drawn from the residual weights.
///
/// # Errors
/// The draws'.
pub fn residual_resampling(weights: &[f64], draws: &mut DrawSource) -> Result<Vec<usize>> {
    let n = weights.len();
    let mut residual = vec![0.0; n];
    let mut sons = vec![0usize; n];
    let mut first = 0;
    for i in 0..n {
        sons[i] = (n as f64 * weights[i]).floor() as usize;
        first += sons[i];
        residual[i] = weights[i] - (sons[i] as f64 / n as f64);
    }
    let next = n - first;
    if next > 0 {
        let mut cum_sum = 0.0;
        let mut cum = vec![0.0; n];
        for i in 0..n {
            residual[i] = residual[i] * n as f64 / next as f64;
            cum[i] = cum_sum + residual[i];
            cum_sum += residual[i];
        }
        for _ in 0..next {
            let u = draws.uniform()?;
            let pick = cum.iter().position(|c| u < *c).unwrap_or(0);
            sons[pick] += 1;
        }
    }
    let mut index = Vec::with_capacity(n);
    for (j, s) in sons.iter().enumerate() {
        index.extend(std::iter::repeat_n(j, *s));
    }
    Ok(index)
}

/// The particle filter's weights (`evaluatePDF` of each prediction,
/// normalised by their sum) and resampling.
///
/// # Errors
/// Shapes or the draws'.
pub fn particle_filter_update(preds: &[Vec<f64>], obs: &Observations, draws: &mut DrawSource) -> Result<Resampling> {
    let sv = obs.uncertainty()?;
    let raw = preds.iter().map(|p| sv.evaluate_pdf(p)).collect::<Result<Vec<f64>>>()?;
    let mut sum = 0.0;
    for w in &raw {
        sum += w;
    }
    let weights: Vec<f64> = raw.iter().map(|w| w / sum).collect();
    let index = residual_resampling(&weights, draws)?;
    Ok(Resampling { weights, index })
}

/// OpenDA's `ParticleFilter.analysis` on model members: members are
/// replaced by copies of their parents (`restoreInternalState`).
///
/// # Errors
/// From the models or [`particle_filter_update`].
pub fn particle_filter_analysis<M: OdaStochModel>(members: &mut [M], main: &mut M, obs: &Observations, draws: &mut DrawSource) -> Result<Resampling> {
    let saved = members.to_vec();
    let preds = members.iter().map(|m| m.observed_values(obs)).collect::<Result<Vec<_>>>()?;
    let r = particle_filter_update(&preds, obs, draws)?;
    for (m, &p) in members.iter_mut().zip(&r.index) {
        m.restore_from(&saved[p]);
    }
    move_main_to_mean(members, main);
    Ok(r)
}

/// A stored gain for the steady-state filter.
#[derive(Debug, Clone, PartialEq)]
pub struct SteadyStateGain {
    /// `(observation index, gain column)`.
    pub columns: Vec<(f64, Vec<f64>)>,
    /// `skipAssimilationStandardDeviationFactor` (OpenDA: +∞, none skipped).
    pub skip_factor: f64,
}

/// `SteadyStateFilter.analysis`: `main += Σ_i (y_i − h_i(main))·K_i`,
/// skipping observations with `|innovation| > skip·σ`.
///
/// # Errors
/// An observation without a gain column (OpenDA throws), or the model's.
pub fn steady_state_analysis<M: OdaStochModel>(gain: &SteadyStateGain, main: &mut M, obs: &Observations) -> Result<()> {
    let pred = main.observed_values(obs)?;
    let mut innovation = obs.value.clone();
    axpy(&mut innovation, -1.0, &pred);
    let mut delta: Vec<f64> = main.state().iter().map(|v| 0.0 * v).collect();
    for i in 0..obs.len() {
        let (_, k) = gain.columns.iter().find(|(ix, _)| *ix == obs.index[i]).ok_or_else(|| RafflesError::InvalidParameter {
            parameter: "steady-state gain".into(),
            value: obs.index[i],
            reason: "no gain column for this observation".into(),
        })?;
        if k.len() != delta.len() {
            return Err(RafflesError::DimensionMismatch { expected: delta.len(), found: k.len() });
        }
        if gain.skip_factor.is_finite() && innovation[i].abs() > gain.skip_factor * obs.std[i] {
            continue;
        }
        axpy(&mut delta, innovation[i], k);
    }
    main.axpy_on_state(1.0, &delta);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::openda::models::{Dynamics, SimpleModelConfig, SimpleStochModel};

    /// EnSR keeps the analysis mean at the KF mean for a scalar and makes
    /// the spread match `P_a`; residual resampling copies by weight; the
    /// steady-state filter applies its gain and skips; errors.
    #[test]
    fn ensr_pf_steady() {
        let states = vec![vec![1.0], vec![2.0], vec![4.0]];
        let obs = Observations::new(vec![0.0], vec![0.0], vec![3.0], vec![1.0], None).unwrap();
        let new: Vec<Vec<f64>> = ensr_update(&states, &states, &obs).unwrap().iter().zip(&states).map(|(c, s)| vec![s[0] + c[0]]).collect();
        let (m0, p0) = (7.0 / 3.0, states.iter().map(|s| (s[0] - 7.0 / 3.0).powi(2)).sum::<f64>() / 2.0);
        let k = p0 / (p0 + 1.0);
        let ma = new.iter().map(|s| s[0]).sum::<f64>() / 3.0;
        let pa = new.iter().map(|s| (s[0] - ma).powi(2)).sum::<f64>() / 2.0;
        assert!((ma - (m0 + k * (3.0 - m0))).abs() < 1e-12 && (pa - (1.0 - k) * p0).abs() < 1e-12);
        assert!(ensr_update(&states, &states[..2], &obs).is_err());
        // floor(3w) = (1, 0, 0); residual cumulative (0.25, 0.7, 1.0).
        let mut d = DrawSource::injected(vec![], vec![0.9, 0.1], vec![]);
        assert_eq!(residual_resampling(&[0.5, 0.3, 0.2], &mut d).unwrap(), vec![0, 0, 2]);
        let mut d = DrawSource::injected(vec![], vec![1.5, 1.5], vec![]);
        assert_eq!(residual_resampling(&[0.5, 0.3, 0.2], &mut d).unwrap(), vec![0, 0, 0], "a uniform past every cumulative weight picks index 0, as OpenDA");
        assert!(residual_resampling(&[0.5, 0.3, 0.2], &mut DrawSource::injected(vec![], vec![], vec![])).is_err());
        let r = particle_filter_update(&[vec![3.0], vec![10.0]], &obs, &mut DrawSource::injected(vec![], vec![0.1], vec![])).unwrap();
        assert!(r.weights[0] > 0.99 && r.index == vec![0, 0]);
        let mut main = SimpleStochModel::new(Dynamics::oscillator(), &SimpleModelConfig::default()).unwrap();
        let g = SteadyStateGain { columns: vec![(0.0, vec![0.5, 0.25])], skip_factor: f64::INFINITY };
        steady_state_analysis(&g, &mut main, &Observations::new(vec![0.0], vec![0.0], vec![1.0], vec![0.1], None).unwrap()).unwrap();
        assert!((main.state()[0] - 0.9).abs() < 1e-15 && (main.state()[1] - 0.05).abs() < 1e-15);
        let skip = SteadyStateGain { skip_factor: 1.0, ..g.clone() };
        let before = main.state();
        steady_state_analysis(&skip, &mut main, &Observations::new(vec![0.0], vec![0.0], vec![5.0], vec![0.1], None).unwrap()).unwrap();
        assert_eq!(main.state(), before);
        assert!(steady_state_analysis(&g, &mut main, &Observations::new(vec![0.0], vec![1.0], vec![5.0], vec![0.1], None).unwrap()).is_err());
        let bad = SteadyStateGain { columns: vec![(0.0, vec![0.5])], skip_factor: f64::INFINITY };
        assert!(steady_state_analysis(&bad, &mut main, &Observations::new(vec![0.0], vec![0.0], vec![5.0], vec![0.1], None).unwrap()).is_err());
    }
}
