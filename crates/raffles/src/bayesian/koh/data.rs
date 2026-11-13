// ---------------------------------------------------------------------------
// Ported from SEPIA (Simulation-Enabled Prediction, Inference, and Analysis).
//
//   Upstream project: SEPIA — Los Alamos National Laboratory
//   Upstream repo:    https://github.com/lanl/SEPIA
//   Upstream file:    sepia/SepiaData.py (transform_xt, standardize_y)
//   Upstream commit:  309890423a121919dc09a678c65a21748bbefb4b  (master)
//   Accessed:         2026-11-04
//
//   (c) 2020. Triad National Security, LLC. All rights reserved.
//   Licensed under the BSD 3-Clause License; the full text is preserved
//   verbatim in crates/raffles/LICENSE-BSD-SEPIA.
//
// This Rust translation is part of RAFFLES / Outram Park and is distributed
// under AGPL-3.0-only. BSD-3 -> AGPLv3 is ONE-WAY: this file may not be
// contributed back to SEPIA or redistributed under BSD terms.
//
// Translation notes: scalar output only (no K/D bases, no ragged or
// separable data, no categorical inputs, no user ranges). numpy's `sum`,
// `mean` and `std(ddof = 1)` are reproduced with numpy's own pairwise
// summation (`pairwise_sum`), so the transforms are bit-identical.
// ---------------------------------------------------------------------------

//! The data of a scalar-output SEPIA model: simulator runs (x, t, y),
//! optional observations (x, y), and SEPIA's transforms (x and t to the unit
//! cube over the simulator's range, y standardised by the simulator's mean
//! and standard deviation).

use crate::{RafflesError, Result};

/// numpy's `pairwise_sum` (`numpy/core/src/umath/loops_utils.h.src`): a
/// plain loop below 8 elements, eight accumulators up to 128, and a split
/// (`n/2` rounded down to a multiple of 8) above. Starts from 0 (checked
/// against `np.sum` on 1 800 random arrays of 3 to 300 elements, numpy
/// 1.26.4: identical bits).
#[must_use]
pub fn pairwise_sum(a: &[f64]) -> f64 {
    let n = a.len();
    if n < 8 {
        let mut r = 0.0;
        for v in a {
            r += *v;
        }
        r
    } else if n <= 128 {
        let mut r = [a[0], a[1], a[2], a[3], a[4], a[5], a[6], a[7]];
        let mut i = 8;
        while i < n - (n % 8) {
            for (j, rj) in r.iter_mut().enumerate() {
                *rj += a[i + j];
            }
            i += 8;
        }
        let mut res = ((r[0] + r[1]) + (r[2] + r[3])) + ((r[4] + r[5]) + (r[6] + r[7]));
        while i < n {
            res += a[i];
            i += 1;
        }
        res
    } else {
        let mut n2 = n / 2;
        n2 -= n2 % 8;
        pairwise_sum(&a[..n2]) + pairwise_sum(&a[n2..])
    }
}

/// The transformed data of a scalar-output model.
#[derive(Debug, Clone, PartialEq)]
pub struct KohData {
    /// Simulator inputs `[x_trans, t_trans]` per run, the emulator's `zt`.
    pub zt: Vec<Vec<f64>>,
    /// Standardised simulator outputs, SEPIA's `w`.
    pub w: Vec<f64>,
    /// Observation inputs on the unit scale (`obs_data.x_trans`); empty for
    /// a simulation-only model.
    pub x_obs: Vec<Vec<f64>>,
    /// Standardised observations, SEPIA's `u`.
    pub u: Vec<f64>,
    /// `p`, the controllable-input count.
    pub p: usize,
    /// `q`, the calibration-input count.
    pub q: usize,
    /// Simulator x minimum per column (0 where SEPIA does not transform).
    pub x_min: Vec<f64>,
    /// Simulator x maximum per column (1 where SEPIA does not transform).
    pub x_max: Vec<f64>,
    /// Simulator t minimum per column.
    pub t_min: Vec<f64>,
    /// Simulator t maximum per column.
    pub t_max: Vec<f64>,
    /// Simulator output mean.
    pub y_mean: f64,
    /// Simulator output standard deviation (`ddof = 1`).
    pub y_sd: f64,
}

fn range(cols: &[Vec<f64>]) -> (Vec<f64>, Vec<f64>) {
    let mut lo: Vec<f64> = cols.iter().map(|c| c.iter().copied().fold(f64::INFINITY, f64::min)).collect();
    let mut hi: Vec<f64> = cols.iter().map(|c| c.iter().copied().fold(f64::NEG_INFINITY, f64::max)).collect();
    // SEPIA: a column whose min equals its max is not transformed.
    for k in 0..lo.len() {
        if hi[k] - lo[k] == 0.0 {
            lo[k] = 0.0;
            hi[k] = 1.0;
        }
    }
    (lo, hi)
}

fn unit(v: &[f64], lo: &[f64], hi: &[f64]) -> Vec<f64> {
    v.iter().zip(lo.iter().zip(hi)).map(|(a, (l, h))| (a - l) / (h - l)).collect()
}

fn columns(rows: &[Vec<f64>], width: usize) -> Vec<Vec<f64>> {
    (0..width).map(|k| rows.iter().map(|r| r[k]).collect()).collect()
}

impl KohData {
    /// SEPIA's `SepiaData(...)`, `transform_xt()`, `standardize_y()` for
    /// scalar output. `x_obs`/`y_obs` empty means a simulation-only model.
    ///
    /// # Errors
    /// [`RafflesError::DimensionMismatch`] for ragged rows or lengths that do
    /// not agree; [`RafflesError::InvalidParameter`] for fewer than two
    /// simulator runs or no calibration input.
    pub fn new(x_sim: &[Vec<f64>], t_sim: &[Vec<f64>], y_sim: &[f64], x_obs: &[Vec<f64>], y_obs: &[f64]) -> Result<Self> {
        let m = y_sim.len();
        if m < 2 || t_sim.is_empty() || t_sim[0].is_empty() {
            return Err(RafflesError::InvalidParameter {
                parameter: "simulator runs".to_string(),
                value: m as f64,
                reason: "a scalar SEPIA model needs at least two runs and one calibration input".to_string(),
            });
        }
        let p = x_sim.first().map_or(0, Vec::len);
        let q = t_sim[0].len();
        let bad = |e: usize, f: usize| RafflesError::DimensionMismatch { expected: e, found: f };
        if x_sim.len() != m {
            return Err(bad(m, x_sim.len()));
        }
        if t_sim.len() != m {
            return Err(bad(m, t_sim.len()));
        }
        if x_obs.len() != y_obs.len() {
            return Err(bad(y_obs.len(), x_obs.len()));
        }
        for r in x_sim.iter().chain(x_obs) {
            if r.len() != p {
                return Err(bad(p, r.len()));
            }
        }
        for r in t_sim {
            if r.len() != q {
                return Err(bad(q, r.len()));
            }
        }
        let (x_min, x_max) = range(&columns(x_sim, p));
        let (t_min, t_max) = range(&columns(t_sim, q));
        let zt = x_sim.iter().zip(t_sim).map(|(x, t)| [unit(x, &x_min, &x_max), unit(t, &t_min, &t_max)].concat()).collect();
        let x_obs_t = x_obs.iter().map(|x| unit(x, &x_min, &x_max)).collect();
        // standardize_y(center=True, scale='scalar'): np.mean(y, 0), then
        // np.std(y - mean, ddof=1), as numpy computes them.
        let y_mean = pairwise_sum(y_sim) / m as f64;
        let y_dm: Vec<f64> = y_sim.iter().map(|y| y - y_mean).collect();
        let dm_mean = pairwise_sum(&y_dm) / m as f64;
        let sq: Vec<f64> = y_dm.iter().map(|v| (v - dm_mean) * (v - dm_mean)).collect();
        let y_sd = (pairwise_sum(&sq) / (m as f64 - 1.0)).sqrt();
        let w = y_dm.iter().map(|v| v / y_sd).collect();
        let u = y_obs.iter().map(|y| (y - y_mean) / y_sd).collect();
        Ok(Self { zt, w, x_obs: x_obs_t, u, p, q, x_min, x_max, t_min, t_max, y_mean, y_sd })
    }

    /// Simulator run count `m`.
    #[must_use]
    pub fn m(&self) -> usize {
        self.w.len()
    }

    /// Observation count `n` (0 for a simulation-only model).
    #[must_use]
    pub fn n(&self) -> usize {
        self.u.len()
    }

    /// A native-scale x onto the unit scale (`transform_xt(x=...)`).
    #[must_use]
    pub fn unit_x(&self, x: &[f64]) -> Vec<f64> {
        unit(x, &self.x_min, &self.x_max)
    }

    /// A native-scale t onto the unit scale.
    #[must_use]
    pub fn unit_t(&self, t: &[f64]) -> Vec<f64> {
        unit(t, &self.t_min, &self.t_max)
    }

    /// A unit-scale t back to native (`transform_xt(native=True)`).
    #[must_use]
    pub fn native_t(&self, t_unit: &[f64]) -> Vec<f64> {
        t_unit.iter().zip(self.t_min.iter().zip(&self.t_max)).map(|(u, (l, h))| u * (h - l) + l).collect()
    }
}
