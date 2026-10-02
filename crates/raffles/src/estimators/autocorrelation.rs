// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// NOT ported from RAVEN. Added 2026-10-03 for GitHub issue #495 (epic #493) at
// the WORKSPACE MAINTAINER's direction; the crate owner's (Adolphus Lye's)
// review is outstanding — see this crate's CLAUDE.md, "Scope boundaries".

//! **Standard errors for correlated sequences** — batch means, the integrated
//! autocorrelation time and the effective sample size.
//!
//! # The problem
//!
//! The textbook standard error `s/√n` assumes the `n` values are independent.
//! A Markov chain's successive states are not, and neither are successive
//! generations of a Monte Carlo power iteration: generation `g + 1`'s fission
//! source *is* generation `g`'s fission sites. Positive correlation makes the
//! true uncertainty of the mean larger than `s/√n` by `sqrt(τ_int)`, where
//!
//! ```text
//! τ_int = 1 + 2 Σ_{k≥1} ρ_k          (ρ_k: lag-k autocorrelation)
//! ```
//!
//! In a loosely coupled core (a pebble bed, a large lattice) `τ_int` can be
//! several, and the naive `σ` under-states the truth by that square root.
//!
//! # Two estimators, reported side by side with the naive one
//!
//! - **Non-overlapping batch means.** Cut the sequence into `a` consecutive
//!   batches of `b`, take each batch's mean, and quote `sd(batch means)/√a`.
//!   If `b` is long compared with the correlation length the batch means are
//!   nearly independent, so this is honest without estimating any `ρ_k`. The
//!   default batch size is `b = ⌊√n⌋` (Flegal & Jones 2010, the standard
//!   consistent choice), **fixed in advance** — choosing `b` after seeing
//!   which one gives the flattering `σ` is exactly the tuning the workspace
//!   forbids. When `n` is not a multiple of `b`, the **leading** remainder is
//!   dropped: the earliest values are the ones most likely to still carry a
//!   start-up transient.
//! - **Integrated autocorrelation time** with Sokal's automatic window:
//!   `τ(M) = 1 + 2 Σ_{k=1..M} ρ_k`, taking the smallest `M ≥ c·τ(M)`, with
//!   `c = 5` (Sokal 1997), again fixed. Then `ESS = n/τ` and
//!   `σ_ESS = s/√ESS`.
//!
//! **Neither replaces the naive `σ`.** [`CorrelatedMean`] carries all three,
//! so a caller cannot report the corrected one while the naive one silently
//! vanishes, or vice versa.
//!
//! # Limits stated up front
//!
//! Both corrected estimators are themselves noisy: a batch-means `σ` from `a`
//! batches has a relative uncertainty of about `1/sqrt(2(a − 1))` — `±21 %`
//! at the 12 batches of a 120-generation run. With too few values neither can
//! see a correlation longer than the run, so a short run can report "no
//! correlation" for a source that simply has not had time to show it.
//!
//! # References
//!
//! - J. M. Flegal and G. L. Jones (2010). Batch means and spectral variance
//!   estimators in Markov chain Monte Carlo. *Annals of Statistics, 38*(2),
//!   1034–1070. doi:10.1214/09-AOS735
//! - A. D. Sokal (1997). Monte Carlo methods in statistical mechanics:
//!   foundations and new algorithms. In *Functional Integration*, Springer,
//!   131–192. doi:10.1007/978-1-4899-0319-8_6
//! - T. Ueki, F. B. Brown, D. K. Parsons and J. S. Warsa (2004).
//!   Autocorrelation and dominance ratio in Monte Carlo criticality
//!   calculations. *Nuclear Science and Engineering, 145*, 279–290 — the
//!   power-iteration motivation.

/// Sokal's window constant `c`: the window `M` is the smallest with
/// `M ≥ c·τ(M)`. Fixed in advance at Sokal's recommended `5`.
pub const SOKAL_WINDOW_C: f64 = 5.0;

/// Lag-`k` sample autocorrelations `ρ_0 = 1, ρ_1, …, ρ_{max_lag}`.
///
/// The standard biased estimator (divide every lag's sum by `n`, not
/// `n − k`), which keeps the sequence positive semi-definite. Returns an
/// empty vector for fewer than two values or a constant sequence (zero
/// variance has no autocorrelation to speak of). `max_lag` is clamped to
/// `n − 1`.
pub fn autocorrelation(x: &[f64], max_lag: usize) -> Vec<f64> {
    let n = x.len();
    if n < 2 {
        return Vec::new();
    }
    let mean = x.iter().sum::<f64>() / n as f64;
    let c0: f64 = x.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / n as f64;
    if !(c0 > 0.0) {
        return Vec::new();
    }
    let max_lag = max_lag.min(n - 1);
    (0..=max_lag)
        .map(|k| {
            let ck: f64 = (0..n - k).map(|i| (x[i] - mean) * (x[i + k] - mean)).sum::<f64>()
                / n as f64;
            ck / c0
        })
        .collect()
}

/// Integrated autocorrelation time by Sokal's automatic window.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AutocorrelationTime {
    /// `τ_int = 1 + 2 Σ_{k=1..window} ρ_k`.
    pub tau_int: f64,
    /// The window `M` the sum was truncated at.
    pub window: usize,
    /// `false` when no `M < n/2` satisfied `M ≥ c·τ(M)`: the correlation
    /// length is comparable to the run, and `tau_int` is a **lower bound**.
    pub window_converged: bool,
    /// `n / τ_int`.
    pub effective_sample_size: f64,
}

/// See [`AutocorrelationTime`]. `None` for fewer than four values or a
/// constant sequence.
///
/// `τ_int` below 1 (anti-correlation) is reported as measured, not clamped:
/// it means the naive `σ` *over*-states, and hiding that would be choosing
/// the instrument after the result.
pub fn integrated_autocorrelation_time(x: &[f64]) -> Option<AutocorrelationTime> {
    let n = x.len();
    if n < 4 {
        return None;
    }
    let max_lag = n / 2;
    let rho = autocorrelation(x, max_lag);
    if rho.is_empty() {
        return None;
    }
    let mut tau = 1.0;
    for m in 1..rho.len() {
        tau += 2.0 * rho[m];
        if m as f64 >= SOKAL_WINDOW_C * tau {
            return Some(AutocorrelationTime {
                tau_int: tau,
                window: m,
                window_converged: true,
                effective_sample_size: n as f64 / tau,
            });
        }
    }
    Some(AutocorrelationTime {
        tau_int: tau,
        window: rho.len() - 1,
        window_converged: false,
        effective_sample_size: n as f64 / tau,
    })
}

/// Non-overlapping batch means.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BatchMeans {
    /// Values per batch, `b`.
    pub batch_size: usize,
    /// Number of batches, `a`.
    pub n_batches: usize,
    /// Leading values dropped so that `a·b` values remain.
    pub dropped_leading: usize,
    /// Mean of the values used (equal to the mean of the batch means).
    pub mean: f64,
    /// `sd(batch means) / √a`.
    pub sem: f64,
    /// Approximate relative 1σ on `sem` itself, `1/sqrt(2(a − 1))`.
    pub sem_relative_uncertainty: f64,
}

/// See [`BatchMeans`]. `None` when `batch_size` is zero or fewer than two
/// batches fit.
pub fn batch_means(x: &[f64], batch_size: usize) -> Option<BatchMeans> {
    if batch_size == 0 {
        return None;
    }
    let n_batches = x.len() / batch_size;
    if n_batches < 2 {
        return None;
    }
    let dropped_leading = x.len() - n_batches * batch_size;
    let used = &x[dropped_leading..];
    let means: Vec<f64> = used
        .chunks(batch_size)
        .map(|c| c.iter().sum::<f64>() / batch_size as f64)
        .collect();
    let (mean, _, sem) = super::pooled(&means);
    Some(BatchMeans {
        batch_size,
        n_batches,
        dropped_leading,
        mean,
        sem,
        sem_relative_uncertainty: 1.0 / (2.0 * (n_batches as f64 - 1.0)).sqrt(),
    })
}

/// The default batch size `⌊√n⌋` (Flegal & Jones 2010), fixed in advance.
pub fn default_batch_size(n: usize) -> usize {
    (n as f64).sqrt().floor() as usize
}

/// The mean of a correlated sequence with its naive and corrected standard
/// errors, side by side.
///
/// `naive_sem` is [`super::mean_and_stderr`] exactly — the number every
/// existing report in this workspace quotes. The two corrected ones are
/// additions; nothing here replaces it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CorrelatedMean {
    /// Number of values.
    pub n: usize,
    /// Plain mean of all values.
    pub mean: f64,
    /// Textbook `s/√n` (assumes independence).
    pub naive_sem: f64,
    /// Batch-means estimate at the default batch size, if at least two
    /// batches fit.
    pub batch: Option<BatchMeans>,
    /// Autocorrelation-time estimate, if computable.
    pub tau: Option<AutocorrelationTime>,
    /// `s / sqrt(ESS)`, the `τ`-corrected standard error.
    pub ess_sem: Option<f64>,
}

impl CorrelatedMean {
    /// Estimate everything from `x`, with the default batch size.
    pub fn estimate(x: &[f64]) -> Self {
        Self::estimate_with_batch_size(x, default_batch_size(x.len()))
    }

    /// As [`Self::estimate`] with an explicit batch size — for a study of the
    /// batch-size dependence, never for picking the size that flatters.
    pub fn estimate_with_batch_size(x: &[f64], batch_size: usize) -> Self {
        let (mean, naive_sem) = super::mean_and_stderr(x);
        let batch = batch_means(x, batch_size);
        let tau = integrated_autocorrelation_time(x);
        let ess_sem = tau.and_then(|t| {
            if x.len() < 2 || !(t.effective_sample_size > 0.0) {
                return None;
            }
            let (_, sd, _) = super::pooled(x);
            Some(sd / t.effective_sample_size.sqrt())
        });
        Self {
            n: x.len(),
            mean,
            naive_sem,
            batch,
            tau,
            ess_sem,
        }
    }

    /// `batch.sem / naive_sem` — how much the naive `σ` under-states, by the
    /// batch-means estimate. `None` when either is unavailable or zero.
    pub fn batch_inflation(&self) -> Option<f64> {
        let b = self.batch?;
        if self.naive_sem > 0.0 {
            Some(b.sem / self.naive_sem)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A deterministic AR(1) sequence `x_{t+1} = φ x_t + e_t` with `e_t` from
    /// the crate's seeded normal sampler — the textbook correlated process,
    /// whose exact `τ_int = (1 + φ)/(1 − φ)`.
    fn ar1(phi: f64, n: usize, seed: u64) -> Vec<f64> {
        let mut s = seed;
        let mut x = 0.0;
        let mut out = Vec::with_capacity(n);
        for _ in 0..n {
            x = phi * x + crate::distributions::seeded::sample_normal(&mut s);
            out.push(x);
        }
        out
    }

    /// **Methodology.** AR(1) with `φ = 0.8`, `n = 20 000`: exact
    /// `τ_int = 9`. The Sokal estimate must land within `±25 %` (its own
    /// statistical error at this `n` is a few percent; 25 % is a bookkeeping
    /// gate, fixed in advance, that a missing factor of 2 in the sum fails).
    /// The batch-means `σ` must exceed the naive one by about `√9 = 3`
    /// (gate: between 2 and 4).
    ///
    /// **Result:** NOT YET MEASURED (testing deferred by maintainer,
    /// 2026-10-03).
    #[test]
    fn ar1_recovers_the_exact_autocorrelation_time() {
        let x = ar1(0.8, 20_000, 20261003);
        let t = integrated_autocorrelation_time(&x).unwrap();
        assert!(t.window_converged);
        assert!((t.tau_int - 9.0).abs() < 0.25 * 9.0, "tau {}", t.tau_int);
        let c = CorrelatedMean::estimate(&x);
        let r = c.batch_inflation().unwrap();
        assert!(r > 2.0 && r < 4.0, "batch inflation {r}");
    }

    /// **Methodology.** White noise (`φ = 0`): `τ_int ≈ 1` (gate `0.8..1.25`)
    /// and the batch-means `σ` within 40 % of the naive one (141 batches give
    /// it a ~6 % relative error; 40 % is a bookkeeping gate).
    ///
    /// **Result:** NOT YET MEASURED (testing deferred by maintainer,
    /// 2026-10-03).
    #[test]
    fn white_noise_is_not_inflated() {
        let x = ar1(0.0, 20_000, 7);
        let t = integrated_autocorrelation_time(&x).unwrap();
        assert!(t.tau_int > 0.8 && t.tau_int < 1.25, "tau {}", t.tau_int);
        let r = CorrelatedMean::estimate(&x).batch_inflation().unwrap();
        assert!((r - 1.0).abs() < 0.4, "inflation {r}");
    }

    /// The naive `σ` is reported unchanged, bit for bit, alongside the
    /// corrected ones; batch bookkeeping drops the LEADING remainder.
    #[test]
    fn naive_sigma_is_kept_and_bookkeeping_is_exact() {
        let x: Vec<f64> = (0..11).map(|i| i as f64).collect();
        let c = CorrelatedMean::estimate(&x);
        assert_eq!((c.mean, c.naive_sem), super::super::mean_and_stderr(&x));
        let b = batch_means(&x, 3).unwrap();
        assert_eq!(b.n_batches, 3);
        assert_eq!(b.dropped_leading, 2);
        assert_eq!(b.mean, (2..11).sum::<i32>() as f64 / 9.0);
        assert!(batch_means(&x, 6).is_none());
        assert!(autocorrelation(&[1.0, 1.0, 1.0], 2).is_empty());
        assert_eq!(default_batch_size(120), 10);
    }
}
