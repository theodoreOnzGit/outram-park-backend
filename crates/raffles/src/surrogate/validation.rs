// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// NOT ported from RAVEN. Added 2026-10-03 for GitHub issue #499 (epic #493).
// This fills the "cross-validation machinery" this module's own docs list as
// in scope (src/surrogate/mod.rs, "Scope — what belongs here"), so it is
// ordinary contribution under the crate's ownership rule; the crate owner
// (Adolphus Lye) has not reviewed it.

//! **Leave-one-out cross-validation and jackknife+ prediction intervals** for
//! [`PolynomialSurrogate`].
//!
//! # Why held-out error, not `R²` on the fit
//!
//! A surrogate's error on the points it was fitted to measures how well it
//! *memorised* them. The question a sweep needs answered is how wrong it is
//! **between** them. Leave-one-out (LOO) refits `n` times, each time without
//! one point, and records the error at the point left out — every residual is
//! a genuine prediction.
//!
//! # Prediction intervals: jackknife+
//!
//! For a new input `x`, with `μ_{−i}` the model fitted without point `i` and
//! `R_i = |y_i − μ_{−i}(x_i)|` its LOO residual, the jackknife+ interval at
//! miscoverage `α` is
//!
//! ```text
//! [ q⁻_α { μ_{−i}(x) − R_i },  q⁺_α { μ_{−i}(x) + R_i } ]
//! ```
//!
//! where `q⁺_α` is the `⌈(1 − α)(n + 1)⌉`-th smallest value and `q⁻_α` the
//! `⌊α(n + 1)⌋`-th smallest. Barber, Candès, Ramdas and Tibshirani (2021)
//! prove coverage of at least `1 − 2α` for exchangeable data **with no
//! assumption on the model** — which is the point: the guarantee does not
//! depend on the polynomial being the right shape. When `n` is too small for
//! the order statistic to exist the bound is infinite, and is returned as
//! such rather than invented.
//!
//! # What the interval covers
//!
//! The data `y_i` here are Monte Carlo results, each carrying its own
//! statistical noise. The interval therefore covers **a new noisy run** at
//! `x`, not the noise-free truth, and it can be no narrower than the run-to-
//! run scatter. Callers that need a statement about the noise-free mean must
//! say so and handle it themselves.
//!
//! # Reference
//!
//! R. F. Barber, E. J. Candès, A. Ramdas and R. J. Tibshirani (2021).
//! Predictive inference with the jackknife+. *Annals of Statistics, 49*(1),
//! 486–507. doi:10.1214/20-AOS1965

use super::PolynomialSurrogate;
use crate::{RafflesError, Result};

/// Leave-one-out models and residuals for a polynomial surrogate.
#[derive(Debug, Clone, PartialEq)]
pub struct LeaveOneOut {
    /// The fit to all points — the model to evaluate for a point estimate.
    pub full: PolynomialSurrogate,
    /// `models[i]` was fitted without point `i`.
    pub models: Vec<PolynomialSurrogate>,
    /// `residuals[i] = y_i − models[i](x_i)` (signed).
    pub residuals: Vec<f64>,
}

impl LeaveOneOut {
    /// Fit the full model and the `n` leave-one-out models.
    ///
    /// `degree` and `ridge` are as in [`PolynomialSurrogate::fit`] and are the
    /// **same** for every refit — choosing them per fold would leak the
    /// held-out point into the model.
    ///
    /// # Errors
    ///
    /// As [`PolynomialSurrogate::fit`] for any of the `n + 1` fits, plus
    /// [`RafflesError::InvalidParameter`] for fewer than three points.
    pub fn fit(inputs: &[Vec<f64>], outputs: &[f64], degree: usize, ridge: f64) -> Result<Self> {
        let n = inputs.len();
        if n < 3 {
            return Err(RafflesError::InvalidParameter {
                parameter: "inputs".into(),
                value: n as f64,
                reason: "leave-one-out needs at least three points".into(),
            });
        }
        if outputs.len() != n {
            return Err(RafflesError::DimensionMismatch {
                expected: n,
                found: outputs.len(),
            });
        }
        let full = PolynomialSurrogate::fit(inputs, outputs, degree, ridge)?;
        let mut models = Vec::with_capacity(n);
        let mut residuals = Vec::with_capacity(n);
        for i in 0..n {
            let xi: Vec<Vec<f64>> = inputs
                .iter()
                .enumerate()
                .filter(|(j, _)| *j != i)
                .map(|(_, r)| r.clone())
                .collect();
            let yi: Vec<f64> = outputs
                .iter()
                .enumerate()
                .filter(|(j, _)| *j != i)
                .map(|(_, y)| *y)
                .collect();
            let m = PolynomialSurrogate::fit(&xi, &yi, degree, ridge)?;
            residuals.push(outputs[i] - m.predict(&inputs[i])?);
            models.push(m);
        }
        Ok(Self {
            full,
            models,
            residuals,
        })
    }

    /// Root-mean-square LOO residual.
    pub fn rmse(&self) -> f64 {
        let n = self.residuals.len() as f64;
        (self.residuals.iter().map(|r| r * r).sum::<f64>() / n).sqrt()
    }

    /// `Σ (r_i / σ_i)² / n` — the LOO residuals in units of each point's own
    /// statistical `σ`. About `1` or more is expected even for a perfect
    /// surrogate, because each residual contains the held-out point's own
    /// noise plus the refit's error; see the module docs.
    ///
    /// # Errors
    ///
    /// [`RafflesError::DimensionMismatch`] if `sigmas` has the wrong length;
    /// [`RafflesError::InvalidParameter`] for a non-positive `σ`.
    pub fn chi2_per_point(&self, sigmas: &[f64]) -> Result<f64> {
        if sigmas.len() != self.residuals.len() {
            return Err(RafflesError::DimensionMismatch {
                expected: self.residuals.len(),
                found: sigmas.len(),
            });
        }
        let mut s = 0.0;
        for (r, sg) in self.residuals.iter().zip(sigmas) {
            if !(*sg > 0.0) {
                return Err(RafflesError::InvalidParameter {
                    parameter: "sigmas".into(),
                    value: *sg,
                    reason: "every sigma must be strictly positive".into(),
                });
            }
            s += (r / sg).powi(2);
        }
        Ok(s / sigmas.len() as f64)
    }

    /// Jackknife+ interval at `x` with miscoverage `alpha` (coverage at least
    /// `1 − 2α`). Bounds are `±∞` when `n` is too small for the order
    /// statistic at this `alpha`.
    ///
    /// # Errors
    ///
    /// [`RafflesError::InvalidParameter`] for `alpha` outside `(0, 0.5)`;
    /// prediction errors as [`PolynomialSurrogate::predict`].
    pub fn jackknife_plus(&self, x: &[f64], alpha: f64) -> Result<(f64, f64)> {
        if !(alpha > 0.0 && alpha < 0.5) {
            return Err(RafflesError::InvalidParameter {
                parameter: "alpha".into(),
                value: alpha,
                reason: "miscoverage must lie in (0, 0.5)".into(),
            });
        }
        let n = self.models.len();
        let mut lo = Vec::with_capacity(n);
        let mut hi = Vec::with_capacity(n);
        for (m, r) in self.models.iter().zip(&self.residuals) {
            let p = m.predict(x)?;
            lo.push(p - r.abs());
            hi.push(p + r.abs());
        }
        lo.sort_by(f64::total_cmp);
        hi.sort_by(f64::total_cmp);
        let np1 = (n + 1) as f64;
        let k_hi = ((1.0 - alpha) * np1).ceil() as usize; // 1-based
        let k_lo = (alpha * np1).floor() as usize; // 1-based
        let upper = if k_hi >= 1 && k_hi <= n {
            hi[k_hi - 1]
        } else {
            f64::INFINITY
        };
        let lower = if k_lo >= 1 && k_lo <= n {
            lo[k_lo - 1]
        } else {
            f64::NEG_INFINITY
        };
        Ok((lower, upper))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **Methodology.** A quadratic sampled exactly at 8 points, fitted at
    /// degree 2: every LOO model reproduces the quadratic, so every LOO
    /// residual is zero to round-off (gate `1e-9`) and the jackknife+
    /// interval at `α = 0.1` collapses onto the true value — except that at
    /// `n = 8`, `⌈0.9·9⌉ = 9 > 8`, so the upper bound must be `+∞`: the
    /// sample is too small for a 80 %-coverage guarantee and the code must
    /// say so. At `α = 0.2` both bounds exist and equal the truth.
    ///
    /// **Result:** NOT YET MEASURED (testing deferred by maintainer,
    /// 2026-10-03).
    #[test]
    fn exact_polynomial_has_zero_loo_error() {
        let xs: Vec<Vec<f64>> = (0..8).map(|i| vec![i as f64]).collect();
        let ys: Vec<f64> = xs.iter().map(|x| 1.0 + 2.0 * x[0] - 0.5 * x[0] * x[0]).collect();
        let l = LeaveOneOut::fit(&xs, &ys, 2, 0.0).unwrap();
        assert!(l.rmse() < 1e-9, "rmse {}", l.rmse());
        let (_, hi) = l.jackknife_plus(&[3.5], 0.1).unwrap();
        assert!(hi.is_infinite());
        let truth = 1.0 + 7.0 - 0.5 * 12.25;
        let (lo, hi) = l.jackknife_plus(&[3.5], 0.2).unwrap();
        assert!((lo - truth).abs() < 1e-8 && (hi - truth).abs() < 1e-8);
    }

    /// **Methodology.** A degree-1 fit to a quadratic must show LOO error
    /// clearly above the degree-2 fit's (which is zero) — the instrument can
    /// see a wrong model.
    ///
    /// **Result:** NOT YET MEASURED (testing deferred by maintainer,
    /// 2026-10-03).
    #[test]
    fn wrong_degree_shows_up_in_loo_error() {
        let xs: Vec<Vec<f64>> = (0..10).map(|i| vec![i as f64]).collect();
        let ys: Vec<f64> = xs.iter().map(|x| x[0] * x[0]).collect();
        let l1 = LeaveOneOut::fit(&xs, &ys, 1, 0.0).unwrap();
        assert!(l1.rmse() > 1.0, "linear fit to x^2 rmse {}", l1.rmse());
        assert!(l1.chi2_per_point(&[1.0; 10]).unwrap() > 1.0);
        assert!(l1.chi2_per_point(&[1.0; 9]).is_err());
        assert!(l1.jackknife_plus(&[1.0], 0.6).is_err());
    }
}
