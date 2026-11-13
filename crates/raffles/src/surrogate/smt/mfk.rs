// ---------------------------------------------------------------------------
// Ported from SMT, the Surrogate Modeling Toolbox.
//
//   Upstream repo:    https://github.com/SMTorg/smt
//   Upstream file:    smt/applications/mfk.py  (MFK: _initialize defaults,
//                       _new_train, _new_train_init, _new_train_iteration,
//                       _predict_intermediate_values,
//                       predict_variances_all_levels, _check_param)
//   Upstream commit:  151a3ff3c13733d168b0ca8844ff0a4ff072252a  (master)
//   Accessed:         2026-11-04
//   Authors (file):   Mostafa Meliani; adapted 2021 by Andres Lopez-Lopera
//
//   Copyright (c) 2017, SMT developers. All rights reserved.
//   BSD 3-Clause License; the full text is this crate's LICENSE-BSD-SMT, and
//   NOTICE records it. Redistributions of source code must retain the above
//   copyright notice, this list of conditions and the disclaimer in
//   LICENSE-BSD-SMT.
//
// This Rust translation is part of RAFFLES / Outram Park and is distributed
// under AGPL-3.0-only. BSD-3-Clause -> (A)GPLv3 is ONE-WAY: this file may NOT
// be contributed back to SMT or redistributed under BSD terms.
//
// Translation notes:
//   * Le Gratiet's recursive formulation of the Kennedy-O'Hagan AR(1) model,
//     any number of levels, one output; the per-level fit shares `core`'s
//     likelihood and optimiser with KRG.
//   * Kept faithfully: `predict_variances_all_levels` builds level i's
//     regression vector from `mu[:, i - 1]`, but only `mu[:, 0]` is ever
//     filled in that function, so for three or more levels the variance of
//     level >= 2 uses mu = 0 (V&V record, finding 6). With two levels, the
//     only case tested, it is exact.
//   * Not ported: MFKPLS / MFKPLSK, noise estimation (`eval_noise`,
//     `optim_var`, `_reinterpolate`), derivatives, mixed-integer inputs,
//     NestedLHS. A theta0 outside the bounds is an error (SMT re-draws).
// ---------------------------------------------------------------------------

//! **SMT's MFK co-kriging** (gh:#1190, fulfilling gh:#1181's machinery):
//! the recursive autoregressive multi-fidelity model of Le Gratiet (2013),
//! level by level from the lowest fidelity.

use super::core::{Level, LevelPar, Pairs, Projection, SmtCorrelation, SmtOptimizer, SmtRegression, SmtStarts};
use super::krg::{broadcast, check_in_bounds, fit_level, normalise};
use super::linalg::{backward_upper, columns, dot, forward, forward_ut, inverse};
use crate::{RafflesError, Result};

/// SMT's MFK options.
#[derive(Debug, Clone, PartialEq)]
pub struct MfkConfig {
    /// `corr` (default `squar_exp`).
    pub corr: SmtCorrelation,
    /// `poly` (default `constant`).
    pub poly: SmtRegression,
    /// `rho_regr` (default `constant`).
    pub rho_regr: SmtRegression,
    /// θ0 per level (one row, broadcast to every level, or one per level;
    /// each row length 1 or `nx`); default `[[1e-2]]`.
    pub theta0: Vec<Vec<f64>>,
    /// `theta_bounds` (default `[1e-6, 20]`).
    pub theta_bounds: (f64, f64),
    /// `nugget` (default `100 ε`).
    pub nugget: f64,
    /// Optimise or keep θ0.
    pub optimizer: SmtOptimizer,
    /// Starts per level (one entry, used for every level with the seed
    /// offset by the level index, or one per level).
    pub starts: Vec<SmtStarts>,
    /// `propagate_uncertainty` (default true).
    pub propagate_uncertainty: bool,
}

impl MfkConfig {
    /// SMT's `MFK()` defaults.
    #[must_use]
    pub fn smt_defaults() -> Self {
        Self {
            corr: SmtCorrelation::SquarExp,
            poly: SmtRegression::Constant,
            rho_regr: SmtRegression::Constant,
            theta0: vec![vec![1e-2]],
            theta_bounds: (1e-6, 20.0),
            nugget: 100.0 * f64::EPSILON,
            optimizer: SmtOptimizer::MaximiseLikelihood,
            starts: vec![SmtStarts::smt_default()],
            propagate_uncertainty: true,
        }
    }

    fn starts_for(&self, lvl: usize) -> SmtStarts {
        match self.starts.get(lvl) {
            Some(s) if self.starts.len() > 1 => s.clone(),
            _ => match self.starts.first() {
                Some(SmtStarts::Default { n_start, seed }) => SmtStarts::Default {
                    n_start: *n_start,
                    seed: seed.wrapping_add(1000 * lvl as i64),
                },
                Some(s) => s.clone(),
                None => SmtStarts::smt_default(),
            },
        }
    }
}

/// One fitted level.
#[derive(Debug, Clone, PartialEq)]
struct FittedLevel {
    x_norma: Vec<Vec<f64>>,
    y_norma: Vec<f64>,
    f: Vec<Vec<f64>>,
    q: usize,
    theta: Vec<f64>,
    rlf: f64,
    par: LevelPar,
}

/// A fitted MFK model of one output.
#[derive(Debug, Clone, PartialEq)]
pub struct Mfk {
    config: MfkConfig,
    x_offset: Vec<f64>,
    x_scale: Vec<f64>,
    y_mean: f64,
    y_std: f64,
    levels: Vec<FittedLevel>,
}

impl Mfk {
    /// Fits the levels `(x_l, y_l)`, lowest fidelity first, as SMT's MFK
    /// `train()`.
    ///
    /// # Errors
    /// No level, mismatched data, a non-finite value, too few rows at a
    /// level for its regression (`n > p + q`), an invalid θ0, or a
    /// likelihood that is `−inf` everywhere.
    pub fn fit(config: &MfkConfig, levels: &[(Vec<Vec<f64>>, Vec<f64>)]) -> Result<Self> {
        let Some(first) = levels.first() else {
            return Err(RafflesError::InvalidParameter {
                parameter: "levels".to_string(),
                value: 0.0,
                reason: "at least one level".to_string(),
            });
        };
        let nx = first.0.first().map_or(0, Vec::len);
        for (x, y) in levels {
            if x.is_empty() || x.len() != y.len() || x.iter().any(|r| r.len() != nx) {
                return Err(RafflesError::DimensionMismatch { expected: x.len(), found: y.len() });
            }
            if x.iter().flatten().chain(y).any(|v| !v.is_finite()) {
                return Err(RafflesError::InvalidParameter {
                    parameter: "training data".to_string(),
                    value: f64::NAN,
                    reason: "non-finite value".to_string(),
                });
            }
        }
        let nlvl = levels.len();
        let all_x: Vec<Vec<f64>> = levels.iter().flat_map(|l| l.0.iter().cloned()).collect();
        let all_y: Vec<f64> = levels.iter().flat_map(|l| l.1.iter().copied()).collect();
        let (x_offset, x_scale, y_mean, y_std) = super::core::standardization(&all_x, &all_y);
        let theta0: Vec<Vec<f64>> = match config.theta0.len() {
            1 => vec![broadcast(&config.theta0[0], nx)?; nlvl],
            l if l == nlvl => config.theta0.iter().map(|t| broadcast(t, nx)).collect::<Result<_>>()?,
            l => return Err(RafflesError::DimensionMismatch { expected: nlvl, found: l }),
        };
        let mut model = Self {
            config: config.clone(),
            x_offset,
            x_scale,
            y_mean,
            y_std,
            levels: Vec::with_capacity(nlvl),
        };
        for (lvl, (x, y)) in levels.iter().enumerate() {
            let x_norma: Vec<Vec<f64>> = x.iter().map(|r| normalise(r, &model.x_offset, &model.x_scale)).collect();
            let y_norma: Vec<f64> = y.iter().map(|v| (v - model.y_mean) / model.y_std).collect();
            let mut f: Vec<Vec<f64>> = x_norma.iter().map(|r| config.poly.basis(r)).collect();
            let p = f[0].len();
            let mut q = 0;
            if lvl > 0 {
                let mut fr = Vec::with_capacity(f.len());
                for (row, xn) in f.iter().zip(&x_norma) {
                    let mu = model.mean_normalised(xn, lvl);
                    let g = config.rho_regr.basis(xn);
                    q = g.len();
                    let mut full: Vec<f64> = g.iter().map(|gv| gv * mu).collect();
                    full.extend_from_slice(row);
                    fr.push(full);
                }
                f = fr;
            }
            if p + q >= x.len() {
                return Err(RafflesError::InvalidParameter {
                    parameter: "levels".to_string(),
                    value: x.len() as f64,
                    reason: format!("ordinary least squares is undetermined at level {lvl}: n_samples = {} must be greater than p + q = {}", x.len(), p + q),
                });
            }
            check_in_bounds(&theta0[lvl], config.theta_bounds)?;
            let level = Level {
                pairs: Pairs::new(&x_norma, config.corr, &Projection::Full),
                f: f.clone(),
                y: y_norma.clone(),
                p,
                q,
                y_std: model.y_std,
                nugget: config.nugget,
                corr: config.corr,
            };
            let (theta, rlf, par) = fit_level(&level, &theta0[lvl], config.theta_bounds, config.optimizer, &config.starts_for(lvl), true)?;
            model.levels.push(FittedLevel { x_norma, y_norma, f, q, theta, rlf, par });
        }
        Ok(model)
    }

    /// Number of levels.
    #[must_use]
    pub fn level_count(&self) -> usize {
        self.levels.len()
    }

    /// Level `lvl`'s θ, reduced log10 likelihood, σ² (output units), β, γ.
    #[must_use]
    pub fn level(&self, lvl: usize) -> Option<(&[f64], f64, f64, &[f64], &[f64])> {
        self.levels.get(lvl).map(|l| (l.theta.as_slice(), l.rlf, l.par.sigma2, l.par.beta.as_slice(), l.par.gamma.as_slice()))
    }

    /// `(X_offset, X_scale, y_mean, y_std)` over all levels concatenated.
    #[must_use]
    pub fn standardisation(&self) -> (&[f64], &[f64], f64, f64) {
        (&self.x_offset, &self.x_scale, self.y_mean, self.y_std)
    }

    fn correlations(&self, lvl: usize, xn: &[f64]) -> Vec<f64> {
        let l = &self.levels[lvl];
        l.x_norma
            .iter()
            .map(|t| {
                let dx: Vec<f64> = xn.iter().zip(t).map(|(a, b)| a - b).collect();
                self.config.corr.value(&l.theta, &Projection::Full.distances(self.config.corr, &dx))
            })
            .collect()
    }

    /// `_predict_intermediate_values(x, upto, descale=False)`: the
    /// standardised mean of level `upto − 1` at a standardised point.
    fn mean_normalised(&self, xn: &[f64], upto: usize) -> f64 {
        let f0 = self.config.poly.basis(xn);
        let l0 = &self.levels[0];
        let mut mu = dot(&f0, &l0.par.beta) + dot(&self.correlations(0, xn), &l0.par.gamma);
        for i in 1..upto {
            let g = self.config.rho_regr.basis(xn);
            let mut f: Vec<f64> = g.iter().map(|v| v * mu).collect();
            f.extend_from_slice(&f0);
            let li = &self.levels[i];
            mu = dot(&f, &li.par.beta) + dot(&self.correlations(i, xn), &li.par.gamma);
        }
        mu
    }

    /// The predictive mean at the highest level, and the variance of every
    /// level (`predict_variances_all_levels`, in output units), with the
    /// `σ²_ρ` of each level above the first.
    ///
    /// # Errors
    /// A wrong-length `x`, or a singular `GᵀG` (not reachable for a fitted
    /// model, whose `G` passed the conditioning check).
    pub fn predict_all_levels(&self, x: &[f64]) -> Result<(f64, Vec<f64>, Vec<f64>)> {
        if x.len() != self.x_offset.len() {
            return Err(RafflesError::DimensionMismatch { expected: self.x_offset.len(), found: x.len() });
        }
        let xn = normalise(x, &self.x_offset, &self.x_scale);
        let mean = self.mean_normalised(&xn, self.levels.len()) * self.y_std + self.y_mean;
        let f0 = self.config.poly.basis(&xn);
        let ys2 = self.y_std * self.y_std;
        let mut mse = Vec::with_capacity(self.levels.len());
        let mut rhos = Vec::new();
        let l0 = &self.levels[0];
        let r0 = self.correlations(0, &xn);
        let mu0 = dot(&f0, &l0.par.beta) + dot(&r0, &l0.par.gamma);
        let level_term = |l: &FittedLevel, r: &[f64], f: &[f64]| -> f64 {
            let rt = forward(&l.par.c, r);
            let ftc = columns(&l.par.ft);
            let rhs: Vec<f64> = f.iter().zip(&ftc).map(|(fv, col)| fv - dot(col, &rt)).collect();
            let u = forward_ut(&l.par.g, &rhs);
            let mut rt2 = 0.0;
            for v in &rt {
                rt2 += v * v;
            }
            let mut u2 = 0.0;
            for v in &u {
                u2 += v * v;
            }
            (l.par.sigma2 / ys2) * (1.0 - rt2 + u2)
        };
        mse.push(level_term(l0, &r0, &f0));
        for i in 1..self.levels.len() {
            let li = &self.levels[i];
            let g = self.config.rho_regr.basis(&xn);
            // Finding 6: mu[:, i - 1] is only ever filled for i - 1 = 0.
            let mu_prev = if i == 1 { mu0 } else { 0.0 };
            let mut f: Vec<f64> = g.iter().map(|v| v * mu_prev).collect();
            f.extend_from_slice(&f0);
            let r = self.correlations(i, &xn);
            let mut m = level_term(li, &r, &f);
            // σ²_ρ = gᵀ (σ² (GᵀG)⁻¹[:q,:q] + β_q β_qᵀ) g.
            let q = li.q;
            let ginv = invert_upper(&li.par.g)?;
            let pq = ginv.len();
            let s = li.par.sigma2 / ys2;
            let mut sig = 0.0;
            for a in 0..q {
                let mut row = 0.0;
                for b in 0..q {
                    let gtg_inv: f64 = (0..pq).map(|k| ginv[a][k] * ginv[b][k]).sum();
                    row += (s * gtg_inv + li.par.beta[a] * li.par.beta[b]) * g[b];
                }
                sig += row * g[a];
            }
            rhos.push(sig);
            if self.config.propagate_uncertainty {
                m += sig * mse[i - 1];
            }
            mse.push(m);
        }
        Ok((mean, mse.iter().map(|v| v * ys2).collect(), rhos))
    }

    /// The highest level's predictive mean and variance (SMT's
    /// `predict_values`, `predict_variances`).
    ///
    /// # Errors
    /// As [`predict_all_levels`](Self::predict_all_levels).
    pub fn predict(&self, x: &[f64]) -> Result<(f64, f64)> {
        let (m, v, _) = self.predict_all_levels(x)?;
        Ok((m, v[v.len() - 1]))
    }
}

/// `G⁻¹` for upper-triangular `G` (so `(GᵀG)⁻¹ = G⁻¹ G⁻ᵀ`); SMT forms
/// `linalg.inv(GᵀG)` instead, equal to round-off.
fn invert_upper(g: &[Vec<f64>]) -> Result<Vec<Vec<f64>>> {
    let n = g.len();
    if (0..n).any(|i| g[i][i] == 0.0) {
        return inverse(g).ok_or(RafflesError::InvalidParameter {
            parameter: "G".to_string(),
            value: 0.0,
            reason: "singular".to_string(),
        });
    }
    let cols: Vec<Vec<f64>> = (0..n)
        .map(|c| {
            let mut e = vec![0.0; n];
            e[c] = 1.0;
            backward_upper(g, &e)
        })
        .collect();
    Ok((0..n).map(|r| (0..n).map(|c| cols[c][r]).collect()).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// One level is KRG up to finding 7 (V&V record): at the same θ the
    /// means agree, and MFK's σ² and variance are KRG's times n/(n − 1)
    /// (MFK divides by n − p, KRG by n); too few high-fidelity rows for
    /// p + q is refused.
    #[test]
    fn one_level_is_krg_and_undetermined_levels_are_refused() {
        let x: Vec<Vec<f64>> = (0..6).map(|i| vec![f64::from(i) / 5.0]).collect();
        let y: Vec<f64> = x.iter().map(|r| (6.0 * r[0]).sin()).collect();
        let mut cfg = MfkConfig::smt_defaults();
        cfg.theta0 = vec![vec![0.7]];
        cfg.optimizer = SmtOptimizer::None;
        let m = Mfk::fit(&cfg, &[(x.clone(), y.clone())]).unwrap();
        let k = super::super::SmtKriging::fit(&super::super::SmtKrgConfig::at_theta(vec![0.7]), &x, &y).unwrap();
        let ratio = 6.0 / 5.0;
        assert!((m.level(0).unwrap().2 / k.gls().0 - ratio).abs() < 1e-14);
        let (a, b) = (m.predict(&[0.33]).unwrap(), k.predict(&[0.33]).unwrap());
        assert!((a.0 - b.0).abs() < 1e-14 && (a.1 / b.1 - ratio).abs() < 1e-12);
        assert_eq!(m.level_count(), 1);
        let hf = (vec![vec![0.0], vec![1.0]], vec![0.0, 1.0]);
        assert!(Mfk::fit(&cfg, &[(x, y), hf]).is_err());
        assert!(Mfk::fit(&cfg, &[]).is_err());
        let inv = invert_upper(&[vec![2.0, 1.0], vec![0.0, 4.0]]).unwrap();
        assert_eq!(inv, vec![vec![0.5, -0.125], vec![0.0, 0.25]]);
    }
}
