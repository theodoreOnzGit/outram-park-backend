// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// NOT ported from RAVEN. Added 2026-10-03 for GitHub issue #494 (epic #493) at
// the WORKSPACE MAINTAINER's direction; the crate owner's (Adolphus Lye's)
// review is outstanding — see this crate's CLAUDE.md, "Scope boundaries".

//! **Seed-ensemble consistency** — is the scatter of `N` independent Monte
//! Carlo runs what each run's own internal `σ` says it should be?
//!
//! # What this answers
//!
//! A Monte Carlo code reports, for one run, a value `x_i` and an internal
//! standard error `σ_i`. Run the same case with `N` independent seeds and you
//! can check that `σ_i` is honest: if it is, the values scatter about their
//! common mean with exactly that spread, and
//!
//! ```text
//! χ² = Σ (x_i − x̄_w)² / σ_i²        x̄_w = Σ(x_i/σ_i²) / Σ(1/σ_i²)
//! ```
//!
//! is distributed as chi-square with `N − 1` degrees of freedom. So
//! `χ²/dof ≈ 1` is consistency; `χ²/dof` above its upper 97.5 % point means
//! the runs scatter **more** than their internal `σ` admits (the classic cause
//! in power iteration: cycle-to-cycle correlation makes the per-run `σ` too
//! small — GitHub #495); below the lower 2.5 % point means they scatter
//! **less**, which points at seeds that are not independent (the `op-rbo`
//! stream-correlation defect is the precedent).
//!
//! # The band is fixed in advance
//!
//! Two-sided 95 %: `[χ²_{0.025}(dof), χ²_{0.975}(dof)] / dof`, from the exact
//! chi-square quantiles (the regularised incomplete gamma in
//! [`crate::distributions`]), not from a normal approximation. Nothing here
//! takes a band from the caller, so the criterion cannot be moved after a
//! result is seen.
//!
//! # Outlier seeds
//!
//! Each seed is compared with the inverse-variance mean of the **others**
//! (leave-one-out, so a wild seed cannot pull the reference towards itself):
//!
//! ```text
//! z_i = (x_i − x̄_{−i}) / sqrt(σ_i² + 1/Σ_{j≠i} σ_j⁻²)
//! ```
//!
//! and flagged when `|z_i|` exceeds the two-sided Bonferroni threshold
//! `Φ⁻¹(1 − 0.025/N)` — a 5 % family-wise false-alarm rate over all `N` seeds,
//! again fixed in advance. A flag is a prompt to look at that seed, **never a
//! licence to drop it**: dropping a seed after seeing it is choosing the data.
//!
//! # References
//!
//! - Particle Data Group, *Review of Particle Physics*, "Statistics" chapter
//!   (weighted averages and the `χ²/dof` scale-factor test).
//! - C. E. Bonferroni (1936); any text on multiple comparisons.

use crate::distributions::special;
use crate::{RafflesError, Result};

/// Direction of a seed-ensemble inconsistency, judged against the fixed
/// two-sided 95 % `χ²/dof` band.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConsistencyVerdict {
    /// `χ²/dof` inside its 95 % band: the seed-to-seed scatter agrees with the
    /// runs' own internal `σ`.
    Consistent,
    /// Above the band: the runs scatter **more** than their internal `σ`
    /// says. The internal `σ` under-states the uncertainty (in power
    /// iteration, usually inter-cycle correlation — GitHub #495).
    OverDispersed,
    /// Below the band: the runs scatter **less** than their internal `σ`
    /// says — seeds that are not independent, or a `σ` that over-states.
    UnderDispersed,
}

/// One seed flagged by the leave-one-out outlier test.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SeedOutlier {
    /// Position of the seed in the input slice.
    pub index: usize,
    /// The seed's value, in the caller's unit.
    pub value: f64,
    /// Its leave-one-out z-score (dimensionless).
    pub z: f64,
}

/// Everything [`seed_consistency`] measures about an ensemble.
///
/// Values carry the caller's unit (pcm, `k`, a tally unit); the ratios and
/// `χ²` are dimensionless.
#[derive(Debug, Clone, PartialEq)]
pub struct SeedConsistency {
    /// Number of seeds.
    pub n: usize,
    /// Unweighted pooled mean, sample sd and standard error, exactly
    /// [`super::pooled`] — the numbers this workspace's ensembles have always
    /// quoted.
    pub mean: f64,
    /// Seed-to-seed sample standard deviation (what ONE run scatters by).
    pub sd: f64,
    /// `sd / √n`, the uncertainty on [`Self::mean`].
    pub sem: f64,
    /// Inverse-variance weighted mean `x̄_w`, the reference for `χ²`.
    pub weighted_mean: f64,
    /// `1/sqrt(Σ σ_i⁻²)`: the uncertainty on `x̄_w` *if* the internal `σ`
    /// are honest. Compare with [`Self::sem`].
    pub weighted_sem: f64,
    /// Root-mean-square internal `σ`, `sqrt(mean σ_i²)`.
    pub rms_internal_sigma: f64,
    /// `sd / rms_internal_sigma` — `≈ 1` when the internal `σ` is honest.
    pub spread_ratio: f64,
    /// `Σ (x_i − x̄_w)² / σ_i²`.
    pub chi2: f64,
    /// `n − 1`.
    pub dof: usize,
    /// `chi2 / dof`.
    pub chi2_per_dof: f64,
    /// The fixed two-sided 95 % band on `χ²/dof` for this `dof`.
    pub band_95: (f64, f64),
    /// Upper-tail probability `P(χ²_dof ≥ chi2)`.
    pub p_value_upper: f64,
    /// Where `chi2_per_dof` falls relative to `band_95`.
    pub verdict: ConsistencyVerdict,
    /// The two-sided Bonferroni `|z|` threshold used for outliers.
    pub outlier_threshold: f64,
    /// Seeds whose leave-one-out `|z|` exceeds `outlier_threshold`.
    pub outliers: Vec<SeedOutlier>,
}

/// Check an ensemble of independent runs against their own internal `σ`.
///
/// `values[i]` and `sigmas[i]` are seed `i`'s estimate and its internal 1σ
/// standard error, in one unit. See the module docs for the statistic, the
/// fixed 95 % band and the outlier rule.
///
/// # Errors
///
/// - [`RafflesError::DimensionMismatch`] if the slices differ in length.
/// - [`RafflesError::InvalidParameter`] for fewer than two seeds, a
///   non-finite value, or a `σ` that is not strictly positive and finite (a
///   zero `σ` would give that seed infinite weight and an infinite `χ²`).
///
/// ```
/// use raffles::estimators::seed_consistency::{seed_consistency, ConsistencyVerdict};
/// let v = [0.0, 1.0, -1.0, 0.5, -0.5];
/// let s = [1.0; 5];
/// let c = seed_consistency(&v, &s).unwrap();
/// assert_eq!(c.dof, 4);
/// assert!((c.chi2 - 2.5).abs() < 1e-12);
/// assert_eq!(c.verdict, ConsistencyVerdict::Consistent);
/// ```
pub fn seed_consistency(values: &[f64], sigmas: &[f64]) -> Result<SeedConsistency> {
    if values.len() != sigmas.len() {
        return Err(RafflesError::DimensionMismatch {
            expected: values.len(),
            found: sigmas.len(),
        });
    }
    let n = values.len();
    if n < 2 {
        return Err(RafflesError::InvalidParameter {
            parameter: "values".into(),
            value: n as f64,
            reason: "a consistency check needs at least two seeds".into(),
        });
    }
    for (&v, &s) in values.iter().zip(sigmas) {
        if !v.is_finite() {
            return Err(RafflesError::InvalidParameter {
                parameter: "values".into(),
                value: v,
                reason: "every seed value must be finite".into(),
            });
        }
        if !(s > 0.0) || !s.is_finite() {
            return Err(RafflesError::InvalidParameter {
                parameter: "sigmas".into(),
                value: s,
                reason: "every internal sigma must be strictly positive and finite".into(),
            });
        }
    }

    let (mean, sd, sem) = super::pooled(values);
    let w: Vec<f64> = sigmas.iter().map(|s| 1.0 / (s * s)).collect();
    let w_sum: f64 = w.iter().sum();
    let weighted_mean = values.iter().zip(&w).map(|(x, wi)| x * wi).sum::<f64>() / w_sum;
    let weighted_sem = (1.0 / w_sum).sqrt();
    let rms_internal_sigma = (sigmas.iter().map(|s| s * s).sum::<f64>() / n as f64).sqrt();
    let chi2: f64 = values
        .iter()
        .zip(&w)
        .map(|(x, wi)| (x - weighted_mean).powi(2) * wi)
        .sum();
    let dof = n - 1;
    let d = dof as f64;
    let band_95 = chi2_per_dof_band(dof, 0.95);
    let chi2_per_dof = chi2 / d;
    let p_value_upper = special::gamma_q(0.5 * d, 0.5 * chi2);
    let verdict = if chi2_per_dof > band_95.1 {
        ConsistencyVerdict::OverDispersed
    } else if chi2_per_dof < band_95.0 {
        ConsistencyVerdict::UnderDispersed
    } else {
        ConsistencyVerdict::Consistent
    };

    let outlier_threshold = special::norm_ppf_std(1.0 - 0.025 / n as f64);
    let mut outliers = Vec::new();
    for i in 0..n {
        let (z, _) = leave_one_out_z(values, sigmas, &w, w_sum, i);
        if z.abs() > outlier_threshold {
            outliers.push(SeedOutlier {
                index: i,
                value: values[i],
                z,
            });
        }
    }

    Ok(SeedConsistency {
        n,
        mean,
        sd,
        sem,
        weighted_mean,
        weighted_sem,
        rms_internal_sigma,
        spread_ratio: sd / rms_internal_sigma,
        chi2,
        dof,
        chi2_per_dof,
        band_95,
        p_value_upper,
        verdict,
        outlier_threshold,
        outliers,
    })
}

/// Leave-one-out z-score of seed `i`: `(x_i − x̄_{−i}) / sqrt(σ_i² + var(x̄_{−i}))`.
/// Returns `(z, x̄_{−i})`.
fn leave_one_out_z(values: &[f64], sigmas: &[f64], w: &[f64], w_sum: f64, i: usize) -> (f64, f64) {
    let w_rest = w_sum - w[i];
    let num: f64 = values
        .iter()
        .zip(w)
        .enumerate()
        .filter(|(j, _)| *j != i)
        .map(|(_, (x, wj))| x * wj)
        .sum();
    let mean_rest = num / w_rest;
    let var = sigmas[i] * sigmas[i] + 1.0 / w_rest;
    ((values[i] - mean_rest) / var.sqrt(), mean_rest)
}

/// Two-sided band on `χ²/dof` at confidence `level` (e.g. `0.95`), from the
/// exact chi-square quantiles: `[χ²_{(1−level)/2}, χ²_{(1+level)/2}] / dof`.
///
/// `dof = 0` returns `(NaN, NaN)`; there is no distribution.
pub fn chi2_per_dof_band(dof: usize, level: f64) -> (f64, f64) {
    if dof == 0 {
        return (f64::NAN, f64::NAN);
    }
    let a = 0.5 * dof as f64;
    let tail = 0.5 * (1.0 - level);
    // chi2_p(nu) = 2 * GammaQuantile(nu/2, p) for a unit-scale gamma.
    let lo = 2.0 * special::gamma_ppf_std(a, tail) / dof as f64;
    let hi = 2.0 * special::gamma_ppf_std(a, 1.0 - tail) / dof as f64;
    (lo, hi)
}

/// Scatter of **disjoint group means** against the prediction `sd/√m`.
///
/// The `1/√N` law is not something a single ensemble can show by quoting
/// `sd/√N` — that is the formula, not a measurement. What *can* be measured
/// is whether means of `m` seeds really scatter by `sd/√m`: split the seeds
/// (in order) into `n_groups = ⌊N/m⌋` disjoint groups of `m`, take each
/// group's mean, and compare the sample sd of those means with `sd/√m`. If
/// the seeds are independent the ratio is `1` within
/// `ratio_sigma ≈ 1/sqrt(2(n_groups − 1))` (the large-sample standard error of
/// a sample sd, relative). Seeds that share structure push it away from 1.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GroupMeanScatter {
    /// Seeds per group, `m`.
    pub group_size: usize,
    /// Number of disjoint groups used (leftover seeds at the end are unused).
    pub n_groups: usize,
    /// Sample sd of the group means.
    pub observed: f64,
    /// `sd / √m` with `sd` the seed-to-seed sd over the seeds used.
    pub predicted: f64,
    /// `observed / predicted`.
    pub ratio: f64,
    /// Approximate 1σ on `ratio`, `1/sqrt(2(n_groups − 1))`.
    pub ratio_sigma: f64,
}

/// See [`GroupMeanScatter`]. Returns `None` unless `group_size ≥ 1` and at
/// least three groups fit (two groups give a ratio with no usable error).
pub fn group_mean_scatter(values: &[f64], group_size: usize) -> Option<GroupMeanScatter> {
    if group_size == 0 {
        return None;
    }
    let n_groups = values.len() / group_size;
    if n_groups < 3 {
        return None;
    }
    let used = &values[..n_groups * group_size];
    let (_, sd, _) = super::pooled(used);
    let means: Vec<f64> = used
        .chunks(group_size)
        .map(|c| c.iter().sum::<f64>() / group_size as f64)
        .collect();
    let (_, observed, _) = super::pooled(&means);
    let predicted = sd / (group_size as f64).sqrt();
    Some(GroupMeanScatter {
        group_size,
        n_groups,
        observed,
        predicted,
        ratio: observed / predicted,
        ratio_sigma: 1.0 / (2.0 * (n_groups as f64 - 1.0)).sqrt(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **Methodology.** Hand-computed `χ²` on a symmetric five-seed set with
    /// unit `σ`: deviations `{0, 1, −1, 0.5, −0.5}` from a weighted mean of 0
    /// give `χ² = 2.5`, `dof = 4`.
    ///
    /// **Result:** NOT YET MEASURED (testing deferred by maintainer,
    /// 2026-10-03).
    #[test]
    fn hand_computed_chi2() {
        let c = seed_consistency(&[0.0, 1.0, -1.0, 0.5, -0.5], &[1.0; 5]).unwrap();
        assert!(c.weighted_mean.abs() < 1e-15);
        assert!((c.chi2 - 2.5).abs() < 1e-12);
        assert_eq!(c.dof, 4);
        assert!(c.outliers.is_empty());
    }

    /// **Methodology.** The 95 % band against published chi-square quantiles
    /// (NIST/SEMATECH e-Handbook, table 1.3.6.7.4): for `dof = 10`,
    /// `χ²_{0.025} = 3.247` and `χ²_{0.975} = 20.483`; for `dof = 31`,
    /// `17.539` and `48.232`. Tolerance `2e-3` absolute on `χ²`, set by the
    /// table's three decimals.
    ///
    /// **Result:** NOT YET MEASURED (testing deferred by maintainer,
    /// 2026-10-03).
    #[test]
    fn band_matches_published_quantiles() {
        let (lo, hi) = chi2_per_dof_band(10, 0.95);
        assert!((lo * 10.0 - 3.247).abs() < 2e-3, "lo {}", lo * 10.0);
        assert!((hi * 10.0 - 20.483).abs() < 2e-3, "hi {}", hi * 10.0);
        let (lo, hi) = chi2_per_dof_band(31, 0.95);
        assert!((lo * 31.0 - 17.539).abs() < 2e-3, "lo {}", lo * 31.0);
        assert!((hi * 31.0 - 48.232).abs() < 2e-3, "hi {}", hi * 31.0);
    }

    /// **Methodology.** Internal `σ` deliberately ten times too small for the
    /// scatter: the verdict must be over-dispersed. Ten times too large:
    /// under-dispersed. One seed placed 20σ away: flagged, and only it.
    ///
    /// **Result:** NOT YET MEASURED (testing deferred by maintainer,
    /// 2026-10-03).
    #[test]
    fn verdicts_and_outliers() {
        let v: Vec<f64> = (0..32).map(|i| ((i * 7919) % 13) as f64 - 6.0).collect();
        let (_, sd, _) = super::super::pooled(&v);
        let small = vec![sd / 10.0; v.len()];
        let large = vec![sd * 10.0; v.len()];
        assert_eq!(
            seed_consistency(&v, &small).unwrap().verdict,
            ConsistencyVerdict::OverDispersed
        );
        assert_eq!(
            seed_consistency(&v, &large).unwrap().verdict,
            ConsistencyVerdict::UnderDispersed
        );
        let mut w = vec![0.0; 16];
        for (i, x) in w.iter_mut().enumerate() {
            *x = if i % 2 == 0 { 0.5 } else { -0.5 };
        }
        w[5] = 20.0;
        let c = seed_consistency(&w, &[1.0; 16]).unwrap();
        assert_eq!(c.outliers.len(), 1);
        assert_eq!(c.outliers[0].index, 5);
    }

    /// Degenerate inputs are refused, never silently scored.
    #[test]
    fn degenerate_inputs_are_refused() {
        assert!(seed_consistency(&[1.0], &[1.0]).is_err());
        assert!(seed_consistency(&[1.0, 2.0], &[1.0]).is_err());
        assert!(seed_consistency(&[1.0, 2.0], &[1.0, 0.0]).is_err());
        assert!(seed_consistency(&[1.0, f64::NAN], &[1.0, 1.0]).is_err());
        assert!(group_mean_scatter(&[1.0, 2.0, 3.0, 4.0], 2).is_none());
    }

    /// **Methodology.** Group means of a sequence built so every group of 4
    /// has the same mean: observed scatter is zero, ratio zero. This checks
    /// the bookkeeping (grouping in order, leftover dropped), not statistics.
    ///
    /// **Result:** NOT YET MEASURED (testing deferred by maintainer,
    /// 2026-10-03).
    #[test]
    fn group_bookkeeping() {
        let v: Vec<f64> = (0..18).map(|i| [1.0, -1.0, 2.0, -2.0][i % 4]).collect();
        let g = group_mean_scatter(&v, 4).unwrap();
        assert_eq!(g.n_groups, 4);
        assert!(g.observed.abs() < 1e-15);
        assert_eq!(g.ratio, 0.0);
    }
}
