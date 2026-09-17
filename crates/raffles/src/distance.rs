//! Statistical distances between two *sample sets*.
//!
//! # What these are for
//!
//! Classical Bayesian model updating needs a likelihood: a closed-form
//! statement of how probable the observed data are under a parameter vector.
//! Plenty of engineering problems cannot supply one — the model is a black box,
//! the data are a handful of scattered measurements, or the quantity being
//! matched is itself a *distribution* rather than a point. What can always be
//! done is to run the model, get a sample of its output, and ask **how far that
//! sample is from the measured one**.
//!
//! That is what this module provides, and it is the engine of
//! [Approximate Bayesian Computation](crate::abc) and of the distance-based
//! stochastic model updating frameworks: pick a distance, and a small distance
//! becomes a large likelihood.
//!
//! # Choosing one
//!
//! The choice is not cosmetic. Lye, Ferson and Xiao (2024) compare exactly
//! these functions on the same problems and find they disagree about which
//! parameters are identifiable — so the metric is a modelling decision that
//! belongs in the write-up, not a default to be taken silently.
//!
//! | Metric | Sees | True metric? | Bounded? |
//! |---|---|---|---|
//! | [`DistanceMetric::Euclidean`] | first two moments only | only with `standardise: false` | no |
//! | [`DistanceMetric::Bhattacharyya`] | the whole (binned) shape | no (fails the triangle inequality) | no — infinite for disjoint samples |
//! | [`DistanceMetric::Hellinger`] | the whole (binned) shape | yes | yes, `[0, 1]` |
//! | [`DistanceMetric::JensenShannon`] | the whole (binned) shape | its square root is | yes, `[0, ln 2]` nats |
//! | [`DistanceMetric::BrayCurtis`] | the whole (binned) shape | no | yes, `[0, 1]` |
//! | [`DistanceMetric::Wasserstein1`] | the whole CDF, unbinned | yes | no |
//!
//! Two practical consequences of that table:
//!
//! - **[`Bhattacharyya`](DistanceMetric::Bhattacharyya) goes infinite** when
//!   the two binned samples share no occupied bin. In an ABC likelihood that
//!   is a hard zero, which is a legitimate answer but will stall a sampler
//!   whose entire initial population is disjoint from the data. The bounded
//!   metrics degrade gracefully instead.
//! - **[`Wasserstein1`](DistanceMetric::Wasserstein1) needs no binning**, so it
//!   has no bin-count parameter to get wrong, and it is the only one here that
//!   keeps working sensibly when the samples are tiny. It is the "area metric"
//!   of the probability-bounds literature.
//!
//! # Binning
//!
//! Four of the six compare histograms, which means a bin count and a shared
//! grid. Both are decided from the **pooled** sample so that neither set gets
//! a grid that flatters it, and the default count follows the square-root rule
//! (`ceil(sqrt(n_pooled))`, clamped to `[2, 128]`) unless the caller names one.
//!
//! In more than one dimension the grid is the Cartesian product of the
//! per-dimension bins, so the bin count grows as `bins^d`. That is the
//! curse of dimensionality and it is not hidden: past three or four
//! dimensions nearly every bin is empty and the binned metrics stop
//! discriminating. Use [`Wasserstein1`](DistanceMetric::Wasserstein1) or
//! [`Euclidean`](DistanceMetric::Euclidean) there, or reduce to summary
//! statistics first. [`DistanceMetric::bin_budget`] reports the grid size a
//! configuration would build so a caller can check before paying for it.
//!
//! # References
//!
//! - A. Lye, S. Ferson and S. Xiao (2024). Comparison between distance
//!   functions for Approximate Bayesian Computation towards stochastic model
//!   updating and model validation under limited data. *ASCE-ASME Journal of
//!   Risk and Uncertainty in Engineering Systems Part A: Civil Engineering,
//!   10*, 03124001. doi:
//!   [10.1061/AJRUA6.RUENG-1223](https://doi.org/10.1061/AJRUA6.RUENG-1223)
//! - S. Bi, M. Broggi and M. Beer (2019). The role of the Bhattacharyya
//!   distance in stochastic model updating. *Mechanical Systems and Signal
//!   Processing, 117*, 437–452. doi:
//!   [10.1016/j.ymssp.2018.08.017](https://doi.org/10.1016/j.ymssp.2018.08.017)
//! - J. Lin (1991). Divergence measures based on the Shannon entropy. *IEEE
//!   Transactions on Information Theory, 37*(1), 145–151. doi:
//!   [10.1109/18.61115](https://doi.org/10.1109/18.61115)
//! - S. Ferson, W. L. Oberkampf and L. Ginzburg (2008). Model validation and
//!   predictive capability for the thermal challenge problem. *Computer Methods
//!   in Applied Mechanics and Engineering, 197*(29–32), 2408–2430. doi:
//!   [10.1016/j.cma.2007.07.030](https://doi.org/10.1016/j.cma.2007.07.030)
//!   — the area metric.
//!
//! Independent implementations from the published definitions; see the
//! provenance note in [`crate::bayesian`] for why that is stated explicitly.

use crate::{RafflesError, Result};

/// Smallest and largest bin counts the automatic rule will choose.
const MIN_BINS: usize = 2;
const MAX_BINS: usize = 128;

/// A statistical distance between two sample sets.
///
/// Every variant is evaluated by [`DistanceMetric::distance`] on two sample
/// sets given as rows of equal width: `a[i][j]` is the `j`-th coordinate of the
/// `i`-th sample. One-dimensional data are rows of length 1.
///
/// Enum dispatch rather than a trait object, per the workspace design rules.
/// Here it also buys exhaustiveness where it matters: adding a metric forces
/// every `match` — including the one that decides whether a metric is bounded —
/// to say what the new one does.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DistanceMetric {
    /// Euclidean distance between the two samples' summary statistics: the
    /// per-dimension means and standard deviations, stacked into one vector.
    ///
    /// The cheapest and the blindest. Two samples with the same mean and
    /// variance but opposite skew are at distance zero from each other, so this
    /// metric cannot see a bimodality that the data do show. It is included
    /// because it is the classical ABC choice and the baseline the others are
    /// compared against in the literature.
    ///
    /// # `standardise` is a real choice, not a convenience
    ///
    /// With `standardise: true` each coordinate's contribution is divided by
    /// the pooled standard deviation **of the pair being compared**, so a
    /// parameter in metres does not drown out one in millimetres. That is what
    /// you want for a multi-output ABC problem — and it costs the triangle
    /// inequality, because a distance whose scaling depends on which two
    /// samples are in front of it is not a metric. This is not a subtlety that
    /// was reasoned about in advance: the property test in this module caught
    /// it, measuring `d(a, c) = 3.000899` against
    /// `d(a, b) + d(b, c) = 2.999664` on three normal samples at means 0, 1
    /// and 3.
    ///
    /// With `standardise: false` the raw summary vectors are compared, which
    /// *is* a true metric — and is the right choice when the outputs already
    /// share a unit, or when the metric property is being relied on.
    ///
    /// [`DistanceMetric::is_true_metric`] reports which of the two you have.
    Euclidean {
        /// Divide each coordinate by the pooled standard deviation of the pair
        /// (unit-robust, not a metric) or compare raw summaries (a metric).
        standardise: bool,
    },

    /// Bhattacharyya distance `-ln(sum_i sqrt(p_i q_i))` over the binned
    /// histograms.
    ///
    /// The distance-based stochastic-model-updating workhorse (Bi, Broggi and
    /// Beer, 2019). Sensitive to the whole shape, not just the moments.
    ///
    /// **It is not a metric** — it violates the triangle inequality — and it is
    /// **unbounded**: when the two binned samples share no occupied bin the
    /// Bhattacharyya coefficient is 0 and the distance is `+inf`. Both facts
    /// are properties of the definition, not of this implementation, and both
    /// are tested.
    Bhattacharyya {
        /// Bins per dimension, or `None` for the square-root rule.
        bins: Option<usize>,
    },

    /// Hellinger distance `sqrt(1 - sum_i sqrt(p_i q_i))` over the binned
    /// histograms.
    ///
    /// The bounded, true-metric relative of [`Bhattacharyya`](Self::Bhattacharyya):
    /// same Bhattacharyya coefficient underneath, but mapped into `[0, 1]` and
    /// satisfying the triangle inequality. 0 means the histograms coincide,
    /// 1 means they are disjoint. This is the distance function of the
    /// Hellinger-distance stochastic model updating framework.
    Hellinger {
        /// Bins per dimension, or `None` for the square-root rule.
        bins: Option<usize>,
    },

    /// Jensen–Shannon divergence in nats over the binned histograms:
    /// `0.5 KL(P || M) + 0.5 KL(Q || M)` with `M = (P + Q) / 2`.
    ///
    /// Bounded above by `ln 2` (0.693147 nats), attained exactly when the two
    /// samples are disjoint — which is what makes it well-behaved as an ABC
    /// distance where the Bhattacharyya distance blows up. Its **square root**
    /// is a true metric (the Jensen–Shannon distance); the divergence itself is
    /// not, and this variant returns the divergence, as the papers using it do.
    ///
    /// This is the distance function of the entropy-based affine-invariant
    /// stochastic model updating framework.
    JensenShannon {
        /// Bins per dimension, or `None` for the square-root rule.
        bins: Option<usize>,
    },

    /// Bray–Curtis dissimilarity over the binned histograms:
    /// `sum_i |p_i - q_i| / sum_i (p_i + q_i)`, which for normalised
    /// histograms is `0.5 * sum_i |p_i - q_i|` — the total variation distance.
    ///
    /// Bounded in `[0, 1]`, cheap, and interpretable as the fraction of
    /// probability mass that would have to be moved. Not a metric in the strict
    /// sense used here only because it is a dissimilarity on compositions; on
    /// normalised histograms it coincides with total variation, which is.
    BrayCurtis {
        /// Bins per dimension, or `None` for the square-root rule.
        bins: Option<usize>,
    },

    /// 1-Wasserstein distance, summed over dimensions — the **area metric**.
    ///
    /// For each coordinate, the area between the two empirical CDFs, computed
    /// exactly from the order statistics with no binning at all. That makes it
    /// the right choice for very small samples, where any histogram is mostly
    /// noise, and it is why the probability-bounds literature (Ferson,
    /// Oberkampf and Ginzburg, 2008) uses it for model validation.
    ///
    /// Multivariate data are handled by summing the per-coordinate distances,
    /// i.e. it compares the **marginals** and is blind to the dependence
    /// structure between them. That is the standard "area metric" convention
    /// in this literature, and it is stated here because the alternative — a
    /// genuine multivariate optimal-transport distance — is a different and far
    /// more expensive object.
    Wasserstein1,
}

impl DistanceMetric {
    /// Whether the metric's value is bounded above.
    ///
    /// Worth asking before using one as an ABC likelihood: an unbounded metric
    /// can return `+inf`, which is a hard zero likelihood that a sampler cannot
    /// climb out of if its whole population is there.
    pub fn is_bounded(&self) -> bool {
        match self {
            Self::Euclidean { .. } | Self::Bhattacharyya { .. } | Self::Wasserstein1 => false,
            Self::Hellinger { .. } | Self::JensenShannon { .. } | Self::BrayCurtis { .. } => true,
        }
    }

    /// Whether the metric satisfies the triangle inequality, and is therefore a
    /// metric in the mathematical sense rather than merely a divergence.
    ///
    /// Not a pedantic distinction: a divergence used where a metric is assumed
    /// (for example to define a tolerance ball in ABC, or to argue that a
    /// posterior concentrates) can give conclusions that do not follow.
    pub fn is_true_metric(&self) -> bool {
        match self {
            Self::Hellinger { .. } | Self::Wasserstein1 => true,
            // Pair-dependent rescaling breaks the triangle inequality; the
            // raw-summary form does not. Measured, not assumed - see the
            // variant docs.
            Self::Euclidean { standardise } => !standardise,
            Self::Bhattacharyya { .. } | Self::JensenShannon { .. } => false,
            // On normalised histograms Bray-Curtis is the total variation
            // distance, which is a metric.
            Self::BrayCurtis { .. } => true,
        }
    }

    /// Bins per dimension this metric would use for a pooled sample of size
    /// `n_pooled`, or `None` for the unbinned [`Wasserstein1`](Self::Wasserstein1)
    /// and [`Euclidean`](Self::Euclidean).
    pub fn bins_for(&self, n_pooled: usize) -> Option<usize> {
        let requested = match self {
            Self::Bhattacharyya { bins }
            | Self::Hellinger { bins }
            | Self::JensenShannon { bins }
            | Self::BrayCurtis { bins } => *bins,
            Self::Euclidean { .. } | Self::Wasserstein1 => return None,
        };
        Some(requested.unwrap_or_else(|| default_bins(n_pooled)))
    }

    /// Total number of histogram cells this metric would build for `n_pooled`
    /// samples in `dimension` dimensions: `bins^dimension`.
    ///
    /// Returns `None` for the unbinned metrics. Saturates rather than
    /// overflowing, so a caller checking the budget before a large run gets
    /// `usize::MAX` rather than a wrapped, reassuringly small number.
    pub fn bin_budget(&self, n_pooled: usize, dimension: usize) -> Option<usize> {
        let bins = self.bins_for(n_pooled)?;
        let mut total: usize = 1;
        for _ in 0..dimension {
            total = total.saturating_mul(bins);
        }
        Some(total)
    }

    /// Distance between two sample sets.
    ///
    /// `a` and `b` are rows of equal width; they need **not** have the same
    /// number of rows, which is the usual situation (a handful of measurements
    /// against thousands of model runs).
    ///
    /// # Errors
    ///
    /// - [`RafflesError::InvalidParameter`] if either sample set is empty, or
    ///   if a metric that needs spread gets a single row.
    /// - [`RafflesError::DimensionMismatch`] if the two sets disagree on width,
    ///   or a row within a set does.
    pub fn distance(&self, a: &[Vec<f64>], b: &[Vec<f64>]) -> Result<f64> {
        let d = validate_pair(a, b)?;
        match self {
            Self::Euclidean { standardise } => euclidean_summary_distance(a, b, d, *standardise),
            Self::Bhattacharyya { .. } => {
                let bc = bhattacharyya_coefficient(self, a, b, d)?;
                if bc <= 0.0 {
                    Ok(f64::INFINITY)
                } else {
                    Ok(-bc.min(1.0).ln())
                }
            }
            Self::Hellinger { .. } => {
                let bc = bhattacharyya_coefficient(self, a, b, d)?;
                Ok((1.0 - bc.clamp(0.0, 1.0)).max(0.0).sqrt())
            }
            Self::JensenShannon { .. } => {
                let (p, q) = histogram_pair(self, a, b, d)?;
                Ok(jensen_shannon(&p, &q))
            }
            Self::BrayCurtis { .. } => {
                let (p, q) = histogram_pair(self, a, b, d)?;
                Ok(0.5 * p.iter().zip(q.iter()).map(|(x, y)| (x - y).abs()).sum::<f64>())
            }
            Self::Wasserstein1 => Ok((0..d)
                .map(|j| {
                    let mut x: Vec<f64> = a.iter().map(|row| row[j]).collect();
                    let mut y: Vec<f64> = b.iter().map(|row| row[j]).collect();
                    x.sort_by(|p, q| p.partial_cmp(q).unwrap_or(core::cmp::Ordering::Equal));
                    y.sort_by(|p, q| p.partial_cmp(q).unwrap_or(core::cmp::Ordering::Equal));
                    wasserstein1_sorted(&x, &y)
                })
                .sum()),
        }
    }
}

/// Square-root rule for the bin count, clamped to a usable range.
fn default_bins(n_pooled: usize) -> usize {
    ((n_pooled as f64).sqrt().ceil() as usize).clamp(MIN_BINS, MAX_BINS)
}

/// Checks two sample sets are non-empty, rectangular and mutually conformable,
/// returning their shared width.
fn validate_pair(a: &[Vec<f64>], b: &[Vec<f64>]) -> Result<usize> {
    if a.is_empty() || b.is_empty() {
        return Err(RafflesError::InvalidParameter {
            parameter: "sample set".to_string(),
            value: 0.0,
            reason: "both sample sets must hold at least one sample".to_string(),
        });
    }
    let d = a[0].len();
    if d == 0 {
        return Err(RafflesError::InvalidParameter {
            parameter: "sample set".to_string(),
            value: 0.0,
            reason: "samples must have at least one coordinate".to_string(),
        });
    }
    for set in [a, b] {
        for row in set {
            if row.len() != d {
                return Err(RafflesError::DimensionMismatch {
                    expected: d,
                    found: row.len(),
                });
            }
            for value in row {
                if !value.is_finite() {
                    return Err(RafflesError::InvalidParameter {
                        parameter: "sample".to_string(),
                        value: *value,
                        reason: "sample coordinates must be finite".to_string(),
                    });
                }
            }
        }
    }
    Ok(d)
}

/// Per-dimension mean and standard deviation of a sample set.
fn summary(set: &[Vec<f64>], d: usize) -> (Vec<f64>, Vec<f64>) {
    let n = set.len() as f64;
    let mut mean = vec![0.0; d];
    for row in set {
        for j in 0..d {
            mean[j] += row[j];
        }
    }
    for m in mean.iter_mut() {
        *m /= n;
    }
    let mut sd = vec![0.0; d];
    if set.len() > 1 {
        for row in set {
            for j in 0..d {
                sd[j] += (row[j] - mean[j]).powi(2);
            }
        }
        for s in sd.iter_mut() {
            *s = (*s / (n - 1.0)).sqrt();
        }
    }
    (mean, sd)
}

/// Euclidean distance between the two sets' (mean, sd) summary vectors,
/// each coordinate scaled by the pooled spread of that coordinate.
fn euclidean_summary_distance(
    a: &[Vec<f64>],
    b: &[Vec<f64>],
    d: usize,
    standardise: bool,
) -> Result<f64> {
    let (mean_a, sd_a) = summary(a, d);
    let (mean_b, sd_b) = summary(b, d);
    let mut acc = 0.0;
    for j in 0..d {
        // With `standardise`, scale so that a coordinate in metres and one in
        // millimetres contribute comparably — at the cost of the triangle
        // inequality, since the scale depends on the pair. Falls back to the
        // raw difference when both samples are degenerate in that coordinate.
        let scale = if standardise {
            let pooled = 0.5 * (sd_a[j] + sd_b[j]);
            if pooled > 0.0 {
                pooled
            } else {
                1.0
            }
        } else {
            1.0
        };
        acc += ((mean_a[j] - mean_b[j]) / scale).powi(2);
        acc += ((sd_a[j] - sd_b[j]) / scale).powi(2);
    }
    Ok(acc.sqrt())
}

/// Normalised histograms of two sample sets over a shared product grid.
fn histogram_pair(
    metric: &DistanceMetric,
    a: &[Vec<f64>],
    b: &[Vec<f64>],
    d: usize,
) -> Result<(Vec<f64>, Vec<f64>)> {
    let n_pooled = a.len() + b.len();
    let bins = metric
        .bins_for(n_pooled)
        .expect("histogram_pair called for an unbinned metric");

    // Shared edges from the pooled range, so neither set gets a flattering
    // grid. A degenerate coordinate (all values equal) is widened so that the
    // bin index is well defined.
    let mut lower = vec![f64::INFINITY; d];
    let mut upper = vec![f64::NEG_INFINITY; d];
    for set in [a, b] {
        for row in set {
            for j in 0..d {
                lower[j] = lower[j].min(row[j]);
                upper[j] = upper[j].max(row[j]);
            }
        }
    }
    for j in 0..d {
        if !(upper[j] > lower[j]) {
            let pad = if lower[j].abs() > 0.0 {
                lower[j].abs() * 1.0e-6
            } else {
                1.0e-6
            };
            lower[j] -= pad;
            upper[j] += pad;
        }
    }

    let cells = metric
        .bin_budget(n_pooled, d)
        .expect("bin_budget called for an unbinned metric");
    if cells == usize::MAX || cells > 1 << 24 {
        return Err(RafflesError::InvalidParameter {
            parameter: "bins".to_string(),
            value: cells as f64,
            reason: format!(
                "a {bins}-bin grid in {d} dimensions would need {cells} cells; reduce the bin \
                 count or use an unbinned metric such as Wasserstein1"
            ),
        });
    }

    let mut p = vec![0.0; cells];
    let mut q = vec![0.0; cells];
    for (set, histogram) in [(a, &mut p), (b, &mut q)] {
        for row in set {
            let mut index = 0usize;
            for j in 0..d {
                let t = (row[j] - lower[j]) / (upper[j] - lower[j]);
                let bin = ((t * bins as f64) as usize).min(bins - 1);
                index = index * bins + bin;
            }
            histogram[index] += 1.0;
        }
        let total: f64 = set.len() as f64;
        for value in histogram.iter_mut() {
            *value /= total;
        }
    }
    Ok((p, q))
}

/// Bhattacharyya coefficient `sum_i sqrt(p_i q_i)` of two binned samples.
fn bhattacharyya_coefficient(
    metric: &DistanceMetric,
    a: &[Vec<f64>],
    b: &[Vec<f64>],
    d: usize,
) -> Result<f64> {
    let (p, q) = histogram_pair(metric, a, b, d)?;
    Ok(p.iter().zip(q.iter()).map(|(x, y)| (x * y).sqrt()).sum())
}

/// Jensen–Shannon divergence in nats between two normalised histograms.
fn jensen_shannon(p: &[f64], q: &[f64]) -> f64 {
    let mut divergence = 0.0;
    for (x, y) in p.iter().zip(q.iter()) {
        let m = 0.5 * (x + y);
        if *x > 0.0 {
            divergence += 0.5 * x * (x / m).ln();
        }
        if *y > 0.0 {
            divergence += 0.5 * y * (y / m).ln();
        }
    }
    // Clamp away the last-ulp excursions past the analytic bound so that a
    // caller comparing against ln 2 does not see 0.6931471805599454 + 1e-16.
    divergence.clamp(0.0, core::f64::consts::LN_2)
}

/// 1-Wasserstein distance between two *sorted* one-dimensional samples.
///
/// Computed as the area between the empirical CDFs, which for sorted samples is
/// a single merged sweep: at each breakpoint the two step functions have known
/// heights, and the contribution is the height gap times the width to the next
/// breakpoint. Exact, `O(n + m)` after sorting, and correct for unequal sample
/// sizes — which a naive "mean absolute difference of paired order statistics"
/// shortcut is not.
fn wasserstein1_sorted(x: &[f64], y: &[f64]) -> f64 {
    let (n, m) = (x.len(), y.len());
    let mut all: Vec<f64> = Vec::with_capacity(n + m);
    all.extend_from_slice(x);
    all.extend_from_slice(y);
    all.sort_by(|p, q| p.partial_cmp(q).unwrap_or(core::cmp::Ordering::Equal));

    let mut area = 0.0;
    let (mut i, mut j) = (0usize, 0usize);
    for w in 0..all.len().saturating_sub(1) {
        let value = all[w];
        while i < n && x[i] <= value {
            i += 1;
        }
        while j < m && y[j] <= value {
            j += 1;
        }
        let width = all[w + 1] - value;
        if width > 0.0 {
            let fx = i as f64 / n as f64;
            let fy = j as f64 / m as f64;
            area += (fx - fy).abs() * width;
        }
    }
    area
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::distributions::{ContinuousDistribution1D, Normal};
    use crate::samplers::stream_seed;
    use outram_mc_libs::rng::lcg::prn;

    /// Draws `n` one-dimensional samples from `N(mu, sigma^2)`.
    fn normal_sample(n: usize, mu: f64, sigma: f64, seed: &mut u64) -> Vec<Vec<f64>> {
        let dist = Normal::new(mu, sigma).unwrap();
        (0..n)
            .map(|_| {
                let u = prn(seed).clamp(1e-12, 1.0 - 1e-12);
                vec![dist.ppf(u).unwrap()]
            })
            .collect()
    }

    /// Closed-form Hellinger distance between two univariate normals.
    fn hellinger_normals(mu1: f64, s1: f64, mu2: f64, s2: f64) -> f64 {
        let bc = (2.0 * s1 * s2 / (s1 * s1 + s2 * s2)).sqrt()
            * (-0.25 * (mu1 - mu2).powi(2) / (s1 * s1 + s2 * s2)).exp();
        (1.0 - bc).sqrt()
    }

    /// Closed-form Bhattacharyya distance between two univariate normals.
    fn bhattacharyya_normals(mu1: f64, s1: f64, mu2: f64, s2: f64) -> f64 {
        0.25 * (0.25 * (s1 * s1 / (s2 * s2) + s2 * s2 / (s1 * s1) + 2.0)).ln()
            + 0.25 * (mu1 - mu2).powi(2) / (s1 * s1 + s2 * s2)
    }

    /// **Methodology.** Every distance must be zero between a sample and
    /// itself, and non-negative in general. Checked for all six metrics on a
    /// 500-point normal sample against itself.
    ///
    /// **Result** (2026-09-16): all six return exactly 0.0.
    #[test]
    fn every_metric_is_zero_against_itself() {
        let mut seed = stream_seed(1, 0);
        let s = normal_sample(500, 0.0, 1.0, &mut seed);
        for metric in [
            DistanceMetric::Euclidean { standardise: true },
            DistanceMetric::Bhattacharyya { bins: None },
            DistanceMetric::Hellinger { bins: None },
            DistanceMetric::JensenShannon { bins: None },
            DistanceMetric::BrayCurtis { bins: None },
            DistanceMetric::Wasserstein1,
        ] {
            let d = metric.distance(&s, &s).unwrap();
            assert!(d.abs() < 1e-12, "{metric:?} gave {d} against itself");
        }
    }

    /// **Methodology.** Every distance here is symmetric by definition, so
    /// `d(a, b)` must equal `d(b, a)` exactly — not approximately, since the
    /// implementations treat the two arguments differently (different sample
    /// sizes, different histogram passes). Two normal samples of different
    /// sizes, all six metrics.
    ///
    /// **Result** (2026-09-16): all six agree to within 1e-15.
    #[test]
    fn every_metric_is_symmetric() {
        let mut seed = stream_seed(2, 0);
        let a = normal_sample(300, 0.0, 1.0, &mut seed);
        let b = normal_sample(700, 0.6, 1.4, &mut seed);
        for metric in [
            DistanceMetric::Euclidean { standardise: true },
            DistanceMetric::Bhattacharyya { bins: Some(20) },
            DistanceMetric::Hellinger { bins: Some(20) },
            DistanceMetric::JensenShannon { bins: Some(20) },
            DistanceMetric::BrayCurtis { bins: Some(20) },
            DistanceMetric::Wasserstein1,
        ] {
            let ab = metric.distance(&a, &b).unwrap();
            let ba = metric.distance(&b, &a).unwrap();
            assert!((ab - ba).abs() < 1e-15, "{metric:?}: {ab} vs {ba}");
        }
    }

    /// **Methodology — against closed forms.** The Hellinger and Bhattacharyya
    /// distances between two univariate normals have exact expressions. Large
    /// samples (200 000 points each) from `N(0, 1)` and `N(1, 1.5^2)` are
    /// compared with 64 bins, and the binned estimates are checked against
    /// those closed forms. Binning biases both estimates downwards (a
    /// histogram cannot resolve structure below its bin width), so the pass
    /// criterion is 15 % relative agreement rather than machine precision;
    /// what is being verified is that the estimator converges to the right
    /// number, not that binning is exact.
    ///
    /// **Result** (2026-09-16, `--release`, seed 20260916): Hellinger 0.330768
    /// against the exact 0.332213 (-0.4 %); Bhattacharyya 0.115868 against the
    /// exact 0.116944 (-0.9 %). Both low by the expected binning bias, and both
    /// well inside the 15 % criterion.
    #[test]
    fn binned_estimates_match_the_gaussian_closed_forms() {
        let mut seed = stream_seed(20_260_916, 0);
        let a = normal_sample(200_000, 0.0, 1.0, &mut seed);
        let b = normal_sample(200_000, 1.0, 1.5, &mut seed);

        let h = DistanceMetric::Hellinger { bins: Some(64) }
            .distance(&a, &b)
            .unwrap();
        let h_ref = hellinger_normals(0.0, 1.0, 1.0, 1.5);
        let bd = DistanceMetric::Bhattacharyya { bins: Some(64) }
            .distance(&a, &b)
            .unwrap();
        let bd_ref = bhattacharyya_normals(0.0, 1.0, 1.0, 1.5);
        println!(
            "Gaussian closed forms: Hellinger {h:.6} (exact {h_ref:.6}), \
             Bhattacharyya {bd:.6} (exact {bd_ref:.6})"
        );

        assert!(
            (h - h_ref).abs() < 0.15 * h_ref,
            "Hellinger {h} vs exact {h_ref}"
        );
        assert!(
            (bd - bd_ref).abs() < 0.15 * bd_ref,
            "Bhattacharyya {bd} vs exact {bd_ref}"
        );
    }

    /// **Methodology — against a closed form with no binning.** For two normals
    /// with equal standard deviation the 1-Wasserstein distance is exactly the
    /// difference of their means. 200 000 points each from `N(0, 1)` and
    /// `N(2, 1)`; the estimate must reach 2.0. This also exercises the unequal
    /// sample-size path (150 000 against 200 000), which the naive paired
    /// order-statistic shortcut would get wrong.
    ///
    /// **Result** (2026-09-16, `--release`, seed 5): 1.996847 with equal sample
    /// sizes and 1.995260 with unequal ones, against the exact 2.0 — a 0.16 %
    /// and 0.24 % sampling error respectively, with no binning bias because
    /// there is no binning.
    #[test]
    fn wasserstein_matches_the_mean_shift_of_equal_variance_normals() {
        let mut seed = stream_seed(5, 0);
        let a = normal_sample(200_000, 0.0, 1.0, &mut seed);
        let b = normal_sample(200_000, 2.0, 1.0, &mut seed);
        let w = DistanceMetric::Wasserstein1.distance(&a, &b).unwrap();

        let c = normal_sample(150_000, 2.0, 1.0, &mut seed);
        let w_uneven = DistanceMetric::Wasserstein1.distance(&a, &c).unwrap();
        println!("Wasserstein1: equal sizes {w:.6}, unequal sizes {w_uneven:.6}, exact 2.0");

        assert!((w - 2.0).abs() < 0.02, "W1 {w} vs exact 2.0");
        assert!(
            (w_uneven - 2.0).abs() < 0.02,
            "W1 with unequal sample sizes {w_uneven} vs exact 2.0"
        );
    }

    /// **Methodology — the exact bounds.** Two completely disjoint samples must
    /// drive each bounded metric to its maximum and the Bhattacharyya distance
    /// to infinity. Samples on `[0, 1]` and `[100, 101]`.
    ///
    /// **Result** (2026-09-16): Hellinger 1.0, Jensen–Shannon ln 2 =
    /// 0.693147, Bray–Curtis 1.0, Bhattacharyya `+inf`.
    #[test]
    fn disjoint_samples_hit_the_exact_bounds() {
        let a: Vec<Vec<f64>> = (0..100).map(|i| vec![i as f64 / 100.0]).collect();
        let b: Vec<Vec<f64>> = (0..100).map(|i| vec![100.0 + i as f64 / 100.0]).collect();

        let h = DistanceMetric::Hellinger { bins: Some(16) }
            .distance(&a, &b)
            .unwrap();
        let js = DistanceMetric::JensenShannon { bins: Some(16) }
            .distance(&a, &b)
            .unwrap();
        let bc = DistanceMetric::BrayCurtis { bins: Some(16) }
            .distance(&a, &b)
            .unwrap();
        let bd = DistanceMetric::Bhattacharyya { bins: Some(16) }
            .distance(&a, &b)
            .unwrap();

        assert!((h - 1.0).abs() < 1e-12, "Hellinger {h}");
        assert!(
            (js - core::f64::consts::LN_2).abs() < 1e-12,
            "Jensen-Shannon {js}"
        );
        assert!((bc - 1.0).abs() < 1e-12, "Bray-Curtis {bc}");
        assert!(bd.is_infinite(), "Bhattacharyya {bd}");
    }

    /// **Methodology — the blind spot, tested rather than asserted in prose.**
    /// The Euclidean summary metric sees only the first two moments, so it must
    /// return (near) zero between a symmetric bimodal sample and a normal
    /// sample constructed to share its mean and variance — while a
    /// shape-sensitive metric must not. This is the reason the module's table
    /// warns against it, so it is worth a test rather than a sentence.
    ///
    /// **Result** (2026-09-16, `--release`, seed 808): Euclidean 0.003672 —
    /// effectively zero, the two samples are indistinguishable to it — while
    /// the Hellinger distance over the same pair is 0.540137. The blind spot
    /// is real, and is 150 times wider than this metric's own noise floor.
    #[test]
    fn euclidean_is_blind_to_a_shape_the_others_see() {
        let mut seed = stream_seed(808, 0);
        // Bimodal: half at -1.5, half at +1.5, each with sd 0.3.
        let mut bimodal = normal_sample(20_000, -1.5, 0.3, &mut seed);
        bimodal.extend(normal_sample(20_000, 1.5, 0.3, &mut seed));
        // A normal with the same mean (0) and the same variance
        // (1.5^2 + 0.3^2 = 2.34).
        let unimodal = normal_sample(40_000, 0.0, 2.34_f64.sqrt(), &mut seed);

        let euclidean = DistanceMetric::Euclidean { standardise: true }
            .distance(&bimodal, &unimodal)
            .unwrap();
        let hellinger = DistanceMetric::Hellinger { bins: Some(40) }
            .distance(&bimodal, &unimodal)
            .unwrap();
        println!(
            "bimodal vs moment-matched normal: Euclidean {euclidean:.6}, Hellinger {hellinger:.6}"
        );

        assert!(
            euclidean < 0.05,
            "Euclidean {euclidean} should be near zero for moment-matched samples"
        );
        assert!(
            hellinger > 0.3,
            "Hellinger {hellinger} should clearly separate these shapes"
        );
    }

    /// **Methodology — the triangle inequality, where it is claimed.** For each
    /// metric that [`DistanceMetric::is_true_metric`] reports as a metric,
    /// `d(a, c) <= d(a, b) + d(b, c)` must hold. Three normal samples at means
    /// 0, 1 and 3. The Bhattacharyya distance is *expected* to be able to
    /// violate it and is excluded from the assertion, which is the honest way
    /// to encode the claim.
    ///
    /// **Result** (2026-09-16, `--release`, seed 64): raw Euclidean and
    /// Wasserstein1 both give d(a,c) 3.0032 against d(a,b) + d(b,c) = 3.0032
    /// (equality, as expected for samples on a line); Hellinger 0.8209 against
    /// 0.9676; Bray-Curtis 0.8668 against 1.0605. All four hold.
    /// This test earned its place on the first run: the standardised Euclidean
    /// variant failed it at `d(a, c) = 3.000899` against
    /// `d(a, b) + d(b, c) = 2.999664`, because its per-pair rescaling makes the
    /// scale depend on which two samples are being compared. The fix was to
    /// split the variant into a standardised (not a metric) and a raw (metric)
    /// form rather than to weaken the test — see [`DistanceMetric::Euclidean`].
    #[test]
    fn true_metrics_satisfy_the_triangle_inequality() {
        let mut seed = stream_seed(64, 0);
        let a = normal_sample(50_000, 0.0, 1.0, &mut seed);
        let b = normal_sample(50_000, 1.0, 1.0, &mut seed);
        let c = normal_sample(50_000, 3.0, 1.0, &mut seed);

        for metric in [
            DistanceMetric::Euclidean {
                standardise: false,
            },
            DistanceMetric::Hellinger { bins: Some(32) },
            DistanceMetric::BrayCurtis { bins: Some(32) },
            DistanceMetric::Wasserstein1,
        ] {
            assert!(metric.is_true_metric(), "{metric:?} should claim to be one");
            let ab = metric.distance(&a, &b).unwrap();
            let bc = metric.distance(&b, &c).unwrap();
            let ac = metric.distance(&a, &c).unwrap();
            println!("{metric:?}: d(a,c) {ac:.6} <= d(a,b) {ab:.6} + d(b,c) {bc:.6}");
            assert!(
                ac <= ab + bc + 1e-9,
                "{metric:?} violated the triangle inequality"
            );
        }
    }

    /// **Methodology.** Monotonicity is what makes a distance usable as a
    /// likelihood: moving the second sample further away must not decrease the
    /// distance. Each metric evaluated against `N(shift, 1)` for shift in
    /// 0, 0.5, 1, 2, 4.
    ///
    /// **Result** (2026-09-16, `--release`): every metric increases
    /// monotonically over that sequence, except that the bounded ones saturate
    /// at their maximum once the samples are disjoint — which is not a
    /// decrease. Printed under `--nocapture`.
    #[test]
    fn distances_increase_as_the_samples_separate() {
        let mut seed = stream_seed(303, 0);
        let a = normal_sample(20_000, 0.0, 1.0, &mut seed);
        for metric in [
            DistanceMetric::Euclidean { standardise: true },
            DistanceMetric::Bhattacharyya { bins: Some(32) },
            DistanceMetric::Hellinger { bins: Some(32) },
            DistanceMetric::JensenShannon { bins: Some(32) },
            DistanceMetric::BrayCurtis { bins: Some(32) },
            DistanceMetric::Wasserstein1,
        ] {
            let mut previous = -1.0;
            let mut row = Vec::new();
            for shift in [0.0, 0.5, 1.0, 2.0, 4.0] {
                let b = normal_sample(20_000, shift, 1.0, &mut seed);
                let d = metric.distance(&a, &b).unwrap();
                row.push(d);
                assert!(
                    d >= previous - 1e-9,
                    "{metric:?} decreased at shift {shift}: {d} after {previous}"
                );
                previous = d;
            }
            println!("{metric:?} across shifts 0, 0.5, 1, 2, 4: {row:?}");
        }
    }

    /// **Methodology.** Two-dimensional data must work, and the product-grid
    /// bin budget must be reported honestly. A 2-D sample against a shifted
    /// copy, and the budget for 32 bins in 3 dimensions.
    ///
    /// **Result** (2026-09-16): the 2-D distance is positive and finite;
    /// `bin_budget(_, 3)` for 32 bins reports 32768 cells.
    #[test]
    fn multivariate_samples_and_the_bin_budget() {
        let mut seed = stream_seed(12, 0);
        let a: Vec<Vec<f64>> = (0..2_000)
            .map(|_| {
                let x = normal_sample(1, 0.0, 1.0, &mut seed)[0][0];
                let y = normal_sample(1, 0.0, 1.0, &mut seed)[0][0];
                vec![x, y]
            })
            .collect();
        let b: Vec<Vec<f64>> = a
            .iter()
            .map(|row| vec![row[0] + 1.0, row[1] - 0.5])
            .collect();

        let d = DistanceMetric::Hellinger { bins: Some(12) }
            .distance(&a, &b)
            .unwrap();
        assert!(d > 0.0 && d <= 1.0, "2-D Hellinger {d}");

        let metric = DistanceMetric::JensenShannon { bins: Some(32) };
        assert_eq!(metric.bin_budget(100, 3), Some(32 * 32 * 32));
        assert_eq!(DistanceMetric::Wasserstein1.bin_budget(100, 3), None);
    }

    /// **Methodology.** A grid too large to build must be refused with a
    /// message that says what to do, not attempted. 64 bins in 8 dimensions is
    /// 2.8e14 cells.
    ///
    /// **Result.** `InvalidParameter` naming the cell count (2026-09-16).
    #[test]
    fn an_unaffordable_grid_is_refused() {
        let a: Vec<Vec<f64>> = (0..10).map(|i| vec![i as f64; 8]).collect();
        let b: Vec<Vec<f64>> = (0..10).map(|i| vec![i as f64 + 0.5; 8]).collect();
        let err = DistanceMetric::Hellinger { bins: Some(64) }
            .distance(&a, &b)
            .unwrap_err();
        assert!(matches!(err, RafflesError::InvalidParameter { .. }));
    }

    /// **Methodology.** Malformed input must be refused rather than silently
    /// producing a number: empty sets, ragged rows, non-finite coordinates.
    ///
    /// **Result.** All three rejected (2026-09-16).
    #[test]
    fn malformed_input_is_refused() {
        let good = vec![vec![1.0], vec![2.0]];
        let empty: Vec<Vec<f64>> = Vec::new();
        assert!(DistanceMetric::Wasserstein1
            .distance(&good, &empty)
            .is_err());

        let ragged = vec![vec![1.0, 2.0], vec![3.0]];
        assert!(DistanceMetric::Wasserstein1
            .distance(&ragged, &ragged)
            .is_err());

        let infinite = vec![vec![f64::INFINITY], vec![1.0]];
        assert!(DistanceMetric::Wasserstein1
            .distance(&good, &infinite)
            .is_err());
    }
}
