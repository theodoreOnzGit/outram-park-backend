// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// NOT ported from RAVEN. Added 2026-10-03 for GitHub issue #496 (epic #493) at
// the WORKSPACE MAINTAINER's direction; the crate owner's (Adolphus Lye's)
// review is outstanding — see this crate's CLAUDE.md, "Scope boundaries".

//! **Stationarity diagnostics for a trace** — has a sequence stopped
//! drifting, and from where on?
//!
//! Generic output-analysis tools for the warm-up ("initial transient")
//! problem. A Monte Carlo power iteration is the motivating case — its
//! Shannon-entropy and `k` traces relax from the initial guess before they
//! fluctuate about a fixed mode — but nothing here knows that: it takes a
//! slice of numbers. Deciding *which* trace, and what to do with the answer,
//! stays with the caller.
//!
//! # Three independent pieces of evidence
//!
//! - **MSER-m truncation** (White 1997; White, Cobb & Spratt 2000). Batch the
//!   trace into means of `m` (default `m = 5`, the published MSER-5), and pick
//!   the truncation `d` minimising
//!   `MSER(d) = Σ_{j>d} (Z_j − Z̄_d)² / (k − d)²`. It balances the bias of
//!   keeping transient values against the variance of discarding good ones.
//!   **If the minimum lies in the second half of the trace** the rule has
//!   found no stationary tail long enough to trust, and the trace is flagged
//!   as not converged — the published rule's own failure signal.
//! - **Geweke's two-window test** (Geweke 1992). Compare the mean of the first
//!   10 % of a (truncated) trace with the last 50 %, as a z-score, with each
//!   window's standard error from **batch means** ([`super::autocorrelation`])
//!   rather than the naive `s/√n` — the windows are autocorrelated, and the
//!   naive error would turn ordinary correlated wander into "drift". Geweke's
//!   original uses a spectral density at zero; batch means estimates the same
//!   quantity. `|z| > 1.96` is drift at 95 %, fixed in advance.
//! - **A single change-point in the mean**, Bayesian. On the batch means
//!   (batching suppresses the autocorrelation the model ignores), with flat
//!   priors on the two segment means and a Jeffreys prior on a common
//!   variance, the posterior over the change location `τ` is exact:
//!   `p(τ | Z) ∝ (n₁ n₂)^{-1/2} · S_τ^{-(n−2)/2}`, `S_τ` the within-segment
//!   sum of squares. Whether there is a change at all is judged by BIC
//!   (`ln Z ≈ ln L̂ − (p/2) ln n`) between "one mean" and "two means plus a
//!   location", and reported on the Kass–Raftery scale through
//!   [`crate::model_selection`].
//!
//! # What none of these can do
//!
//! They cannot see a transient longer than the trace, and a slowly drifting
//! trace with a long correlation time can look stationary to every one of
//! them. Passing is evidence, not proof; the reference for an inactive-cycle
//! count is still a much longer run.
//!
//! # References
//!
//! - K. P. White Jr. (1997). An effective truncation heuristic for bias
//!   reduction in simulation output. *Simulation, 69*(6), 323–334.
//! - K. P. White Jr., M. J. Cobb and S. C. Spratt (2000). A comparison of five
//!   steady-state truncation heuristics for simulation. *Proc. 2000 Winter
//!   Simulation Conference*, 755–760. doi:10.1109/WSC.2000.899870
//! - J. Geweke (1992). Evaluating the accuracy of sampling-based approaches to
//!   the calculation of posterior moments. In *Bayesian Statistics 4*, Oxford
//!   University Press, 169–193.
//! - R. E. Kass and A. E. Raftery (1995). Bayes factors. *JASA, 90*(430),
//!   773–795.

use crate::model_selection::{ln_bayes_factor, EvidenceStrength, ModelEvidence};

/// MSER batch size, fixed at the published MSER-5.
pub const MSER_BATCH: usize = 5;
/// Geweke window fractions (first, last), fixed at Geweke's 10 % / 50 %.
pub const GEWEKE_WINDOWS: (f64, f64) = (0.1, 0.5);
/// Two-sided 95 % critical `|z|` for the Geweke test.
pub const GEWEKE_Z_CRIT: f64 = 1.959_963_984_540_054;

/// Result of the MSER-m truncation rule.
#[derive(Debug, Clone, PartialEq)]
pub struct MserTruncation {
    /// Batch size `m` used.
    pub batch_size: usize,
    /// Optimal truncation in **original samples** (`d* · m`).
    pub truncation: usize,
    /// Optimal truncation in batches, `d*`.
    pub truncation_batches: usize,
    /// Number of batches `k`.
    pub n_batches: usize,
    /// `MSER(d)` for every `d` evaluated (`0..=k−2`).
    pub statistic: Vec<f64>,
    /// `true` when `d* ≤ k/2`: a stationary tail at least half the trace long
    /// was found. `false` is the rule's own "not converged" signal.
    pub in_first_half: bool,
}

/// MSER-`batch_size` on `x`. Batches from the **start** (the trailing
/// remainder is dropped), so a truncation counts samples from the first
/// value. `None` with fewer than four batches.
pub fn mser_truncation(x: &[f64], batch_size: usize) -> Option<MserTruncation> {
    if batch_size == 0 {
        return None;
    }
    let k = x.len() / batch_size;
    if k < 4 {
        return None;
    }
    let z: Vec<f64> = x[..k * batch_size]
        .chunks(batch_size)
        .map(|c| c.iter().sum::<f64>() / batch_size as f64)
        .collect();
    let mut statistic = Vec::with_capacity(k - 1);
    for d in 0..=k - 2 {
        let tail = &z[d..];
        let m = tail.len() as f64;
        let mean = tail.iter().sum::<f64>() / m;
        let ss: f64 = tail.iter().map(|v| (v - mean).powi(2)).sum();
        statistic.push(ss / (m * m));
    }
    let mut best = 0;
    for (d, s) in statistic.iter().enumerate() {
        if *s < statistic[best] {
            best = d;
        }
    }
    Some(MserTruncation {
        batch_size,
        truncation: best * batch_size,
        truncation_batches: best,
        n_batches: k,
        statistic,
        in_first_half: best <= k / 2,
    })
}

/// Result of Geweke's two-window test.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GewekeTest {
    /// Mean of the first window.
    pub mean_first: f64,
    /// Mean of the last window.
    pub mean_last: f64,
    /// Standard error of each window's mean (batch means where at least two
    /// batches fit, otherwise naive — see `batch_errors`).
    pub sem_first: f64,
    /// See `sem_first`.
    pub sem_last: f64,
    /// `(mean_first − mean_last) / sqrt(sem_first² + sem_last²)`.
    pub z: f64,
    /// `true` when both errors came from batch means. `false` means at least
    /// one window was too short for that and fell back to the naive `s/√n`,
    /// which under-states the error of a correlated window — read `z` as an
    /// upper bound on its true size then.
    pub batch_errors: bool,
    /// `|z| > GEWEKE_Z_CRIT`.
    pub drift: bool,
}

/// Geweke's test on `x` with the fixed [`GEWEKE_WINDOWS`]. `None` when either
/// window has fewer than two values.
pub fn geweke(x: &[f64]) -> Option<GewekeTest> {
    let n = x.len();
    let na = (GEWEKE_WINDOWS.0 * n as f64).floor() as usize;
    let nb = (GEWEKE_WINDOWS.1 * n as f64).floor() as usize;
    if na < 2 || nb < 2 {
        return None;
    }
    let a = &x[..na];
    let b = &x[n - nb..];
    let err = |w: &[f64]| -> (f64, f64, bool) {
        let c = super::autocorrelation::CorrelatedMean::estimate(w);
        match c.batch {
            Some(bm) => (c.mean, bm.sem, true),
            None => (c.mean, c.naive_sem, false),
        }
    };
    let (ma, sa, ba) = err(a);
    let (mb, sb, bb) = err(b);
    let denom = (sa * sa + sb * sb).sqrt();
    let z = if denom > 0.0 {
        (ma - mb) / denom
    } else if ma == mb {
        0.0
    } else {
        f64::INFINITY
    };
    Some(GewekeTest {
        mean_first: ma,
        mean_last: mb,
        sem_first: sa,
        sem_last: sb,
        z,
        batch_errors: ba && bb,
        drift: z.abs() > GEWEKE_Z_CRIT,
    })
}

/// Posterior over a single change-point in the mean, plus a BIC verdict on
/// whether there is one.
#[derive(Debug, Clone, PartialEq)]
pub struct ChangePoint {
    /// Batch size the trace was reduced by before the analysis.
    pub batch_size: usize,
    /// `posterior[i]` is `p(τ = first_tau + i | Z)`: the first `τ` batch
    /// means form segment one. Sums to 1.
    pub posterior: Vec<f64>,
    /// Smallest `τ` considered (each segment needs two batch means).
    pub first_tau: usize,
    /// Posterior mode, in batch means.
    pub map_tau: usize,
    /// `map_tau` in original samples.
    pub map_sample: usize,
    /// Mean of segment one and segment two at the mode.
    pub segment_means: (f64, f64),
    /// `ln B` of "change" against "no change" by BIC. Positive favours a change.
    pub ln_bayes_factor_change: f64,
    /// Kass–Raftery reading of `|ln B|`.
    pub strength: EvidenceStrength,
}

/// Single change-point analysis of `x`, reduced to batch means of
/// `batch_size` first. `None` with fewer than six batch means, or a trace
/// with no spread at all.
pub fn change_point(x: &[f64], batch_size: usize) -> Option<ChangePoint> {
    if batch_size == 0 {
        return None;
    }
    let k = x.len() / batch_size;
    if k < 6 {
        return None;
    }
    let z: Vec<f64> = x[..k * batch_size]
        .chunks(batch_size)
        .map(|c| c.iter().sum::<f64>() / batch_size as f64)
        .collect();
    let n = k as f64;
    let ss = |s: &[f64]| -> (f64, f64) {
        let m = s.iter().sum::<f64>() / s.len() as f64;
        (m, s.iter().map(|v| (v - m).powi(2)).sum())
    };
    let (_, s0) = ss(&z);
    if !(s0 > 0.0) {
        return None;
    }
    let first_tau = 2;
    let mut logp = Vec::new();
    let mut s_min = f64::INFINITY;
    for tau in first_tau..=k - 2 {
        let (_, s1) = ss(&z[..tau]);
        let (_, s2) = ss(&z[tau..]);
        let s = (s1 + s2).max(f64::MIN_POSITIVE);
        s_min = s_min.min(s);
        let (n1, n2) = (tau as f64, (k - tau) as f64);
        logp.push(-0.5 * (n1 * n2).ln() - 0.5 * (n - 2.0) * s.ln());
    }
    let lmax = logp.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let w: Vec<f64> = logp.iter().map(|l| (l - lmax).exp()).collect();
    let tot: f64 = w.iter().sum();
    let posterior: Vec<f64> = w.iter().map(|v| v / tot).collect();
    let mut imax = 0;
    for (i, p) in posterior.iter().enumerate() {
        if *p > posterior[imax] {
            imax = i;
        }
    }
    let map_tau = first_tau + imax;
    let (m1, _) = ss(&z[..map_tau]);
    let (m2, _) = ss(&z[map_tau..]);

    // BIC: Gaussian max log-likelihood -n/2 (ln(2π S/n) + 1); p = 2 (μ, σ)
    // without a change, p = 4 (μ₁, μ₂, σ, τ) with one.
    let ln_l = |s: f64| -0.5 * n * ((2.0 * std::f64::consts::PI * s / n).ln() + 1.0);
    let ln_z0 = ln_l(s0) - 1.0 * n.ln();
    let ln_z1 = ln_l(s_min) - 2.0 * n.ln();
    let (e0, e1) = match (
        ModelEvidence::new("one mean", ln_z0),
        ModelEvidence::new("change in mean", ln_z1),
    ) {
        (Ok(a), Ok(b)) => (a, b),
        _ => return None,
    };
    let lnb = ln_bayes_factor(&e1, &e0);
    Some(ChangePoint {
        batch_size,
        posterior,
        first_tau,
        map_tau,
        map_sample: map_tau * batch_size,
        segment_means: (m1, m2),
        ln_bayes_factor_change: lnb,
        strength: EvidenceStrength::from_ln_bayes_factor(lnb),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// White noise from the crate's seeded sampler, plus an optional
    /// exponential transient `amp · exp(−t/scale)`.
    fn trace(n: usize, amp: f64, scale: f64, seed: u64) -> Vec<f64> {
        let mut s = seed;
        (0..n)
            .map(|t| {
                amp * (-(t as f64) / scale).exp()
                    + 0.1 * crate::distributions::seeded::sample_normal(&mut s)
            })
            .collect()
    }

    /// **Methodology.** A transient of amplitude 50 noise-σ decaying with
    /// scale 20 samples, in 1000 samples. The transient falls below 0.1 σ
    /// after `20 ln 500 ≈ 124` samples, so MSER-5 must truncate between 40
    /// and 250 samples (a band fixed in advance around the analytic decay,
    /// wide because MSER trades bias against variance), and inside the first
    /// half. Geweke on the untruncated trace must flag drift; on the trace
    /// after the MSER truncation it must not (`|z| ≤ 1.96` — this can fail
    /// 5 % of the time for a fixed seed; the seed is fixed so the test is
    /// deterministic, and a failure is a prompt to inspect, not to reseed).
    ///
    /// **Result:** NOT YET MEASURED (testing deferred by maintainer,
    /// 2026-10-03).
    #[test]
    fn transient_is_found_and_tail_passes() {
        let x = trace(1000, 5.0, 20.0, 496);
        let m = mser_truncation(&x, MSER_BATCH).unwrap();
        assert!(m.in_first_half);
        assert!(
            (40..=250).contains(&m.truncation),
            "truncation {}",
            m.truncation
        );
        assert!(geweke(&x).unwrap().drift);
        let tail = &x[m.truncation..];
        let g = geweke(tail).unwrap();
        assert!(!g.drift, "z {}", g.z);
    }

    /// **Methodology.** A step of 10 noise-σ at sample 300 of 600: the change
    /// point's posterior mode must sit within one batch of batch 60, and the
    /// BIC verdict must be "very strong". On pure noise the BIC must NOT
    /// report a change with strong or very strong evidence.
    ///
    /// **Result:** NOT YET MEASURED (testing deferred by maintainer,
    /// 2026-10-03).
    #[test]
    fn step_change_is_located() {
        let mut x = trace(600, 0.0, 1.0, 11);
        for v in &mut x[300..] {
            *v += 1.0;
        }
        let c = change_point(&x, MSER_BATCH).unwrap();
        assert!((c.map_tau as i64 - 60).abs() <= 1, "map {}", c.map_tau);
        assert_eq!(c.strength, EvidenceStrength::VeryStrong);
        assert!(c.ln_bayes_factor_change > 0.0);
        let s: f64 = c.posterior.iter().sum();
        assert!((s - 1.0).abs() < 1e-12);

        let flat = trace(600, 0.0, 1.0, 12);
        let c = change_point(&flat, MSER_BATCH).unwrap();
        assert!(
            c.ln_bayes_factor_change < 0.0
                || c.strength < EvidenceStrength::Strong,
            "noise read as a change: ln B {}",
            c.ln_bayes_factor_change
        );
    }

    /// Too-short inputs give `None`, never a verdict.
    #[test]
    fn short_traces_are_refused() {
        assert!(mser_truncation(&[1.0; 15], 5).is_none());
        assert!(geweke(&[1.0; 10]).is_none());
        assert!(change_point(&[1.0; 25], 5).is_none());
        assert!(change_point(&[1.0; 60], 5).is_none());
    }
}
