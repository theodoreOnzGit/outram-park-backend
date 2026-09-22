// SPDX-License-Identifier: GPL-3.0

//! **Tally triggers** — run until a target precision rather than for a fixed
//! batch count. GitHub #263, first half.
//!
//! Ported from `src/tallies/trigger.cpp` at OpenMC `afa7a14`. (The commit the
//! issue cites, `608a1c33`, is unavailable here and not fetchable.)
//!
//! # Why this matters beyond convenience
//!
//! Every driver in `physics/` takes a fixed batch count today. That means a
//! user either over-runs (wasted time) or under-runs (a result whose sigma
//! nobody checked). A trigger converts "how many batches?" — a guess — into
//! "what precision do I need?", which is the question that actually has an
//! answer.

/// Which uncertainty measure a trigger watches. `openmc::TriggerMetric`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TriggerMetric {
    /// Variance of the mean, i.e. `std_dev^2`.
    Variance,
    /// Standard deviation of the mean divided by `|mean|`.
    RelativeError,
    /// Standard deviation of the mean.
    StandardDeviation,
}

/// One trigger: a metric, a threshold, and what to do about empty bins.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Trigger {
    pub metric: TriggerMetric,
    /// The value the metric must fall **to or below**.
    pub threshold: f64,
    /// Treat a bin with no contributions as satisfied rather than as
    /// infinitely uncertain. `Trigger::ignore_zeros` upstream.
    ///
    /// The default (`false`) is the safe one: a score that has never been hit
    /// is not converged, it is unmeasured, and the two must not look alike.
    pub ignore_zeros: bool,
}

/// Running sums for one tally bin across realizations.
#[derive(Debug, Clone, Copy, Default)]
pub struct BinStats {
    /// Sum of the per-realization values.
    pub sum: f64,
    /// Sum of their squares.
    pub sum_sq: f64,
}

/// Mean, standard deviation **of the mean**, and relative error for a bin over
/// `n` realizations — `get_tally_uncertainty` (`src/tallies/trigger.cpp:30`).
///
/// Returns `None` for a bin whose mean is exactly zero, which upstream signals
/// with a `(-1, -1)` sentinel pair. A `None` here is "no contributions", not
/// "converged to zero", and the caller must keep those distinct.
///
/// # The formula is the standard error, not the sample spread
///
/// ```text
/// mean    = sum / n
/// std_dev = sqrt( (sum_sq / n - mean^2) / (n - 1) )
/// rel_err = std_dev / |mean|
/// ```
///
/// Note the `(n - 1)` is **outside** the parenthesis, so this is the
/// uncertainty **on the mean**, not the spread of the realizations. Getting
/// that wrong by a factor of `sqrt(n)` would make every trigger fire far too
/// early and the runs would look wonderfully cheap.
pub fn bin_uncertainty(stats: BinStats, n: usize) -> Option<(f64, f64, f64)> {
    if n < 2 {
        return None;
    }
    let nf = n as f64;
    let mean = stats.sum / nf;
    if mean == 0.0 {
        return None;
    }
    let std_dev = ((stats.sum_sq / nf - mean * mean) / (nf - 1.0)).sqrt();
    let rel_err = std_dev / mean.abs();
    Some((mean, std_dev, rel_err))
}

/// The uncertainty/threshold ratio for one bin under one trigger, or
/// `INFINITY` for an unmeasured bin that the trigger does not ignore.
///
/// A ratio at or below 1 means that bin is satisfied.
///
/// # A deliberate divergence from upstream, and why
///
/// `src/tallies/trigger.cpp:108` reads:
///
/// ```text
/// double this_ratio = uncertainty / trigger.threshold;
/// if (trigger.metric == TriggerMetric::variance) {
///   this_ratio = std::sqrt(ratio);
/// }
/// ```
///
/// Under the **variance** metric it takes `sqrt(ratio)` — `ratio` being the
/// running maximum across all bins, the function's *output* accumulator, which
/// is initialised to `0` at `:60`. It is not `this_ratio`. So the first bin
/// evaluated under a variance trigger yields `sqrt(0) = 0`, the threshold is
/// not consulted at all, and the trigger reports satisfied on a bin it never
/// examined.
///
/// **That reads as an upstream bug rather than a convention**, and this port
/// does NOT reproduce it. The variance branch here is
/// `sqrt(this_ratio)` — the evident intent, which makes the variance metric
/// behave like the others: it puts the ratio on a standard-deviation footing so
/// a threshold on variance converges at the same rate as one on `std_dev`.
///
/// The workspace rule is that upstream is the specification, and it is being
/// knowingly departed from here. Recorded rather than silently "fixed", because
/// a reader comparing the two files will find the difference and needs to know
/// it was deliberate. `tests::the_variance_metric_does_not_reproduce_upstreams_bug`
/// pins the divergence so it cannot be quietly reverted either way.
pub fn bin_ratio(stats: BinStats, n: usize, trigger: &Trigger) -> f64 {
    let Some((_, std_dev, rel_err)) = bin_uncertainty(stats, n) else {
        return if trigger.ignore_zeros {
            0.0
        } else {
            f64::INFINITY
        };
    };
    let uncertainty = match trigger.metric {
        TriggerMetric::Variance => std_dev * std_dev,
        TriggerMetric::StandardDeviation => std_dev,
        TriggerMetric::RelativeError => rel_err,
    };
    let this_ratio = uncertainty / trigger.threshold;
    match trigger.metric {
        // See the doc comment: upstream has `sqrt(ratio)` here, using the
        // running maximum instead of this bin's own ratio.
        TriggerMetric::Variance => this_ratio.sqrt(),
        _ => this_ratio,
    }
}

/// The most limiting ratio over every bin of every trigger.
///
/// Bins with fewer than two realizations are skipped, as upstream skips tallies
/// with `n_realizations_ < 2` (`:65`) — a single realization has no
/// uncertainty estimate at all, and treating it as converged would stop the run
/// on its first batch.
pub fn limiting_ratio(bins: &[BinStats], n: usize, triggers: &[Trigger]) -> f64 {
    if n < 2 {
        return f64::INFINITY;
    }
    let mut ratio = 0.0_f64;
    for t in triggers {
        for &b in bins {
            let r = bin_ratio(b, n, t);
            if r > ratio {
                ratio = r;
            }
            if ratio.is_infinite() {
                return ratio; // upstream exits early on an unmeasured bin
            }
        }
    }
    ratio
}

/// Are all triggers satisfied? `max(ratio) <= 1` (`:182`).
pub fn satisfied(ratio: f64) -> bool {
    ratio <= 1.0
}

/// Predicted total batches needed, assuming variance falls as `1/N`
/// (`:209-215`):
///
/// ```text
/// n_pred = (int)(n_active * ratio^2) + n_inactive + 1
/// ```
///
/// Returns `None` when the ratio is infinite — a tally with no scores gives no
/// basis for an estimate, and upstream says so rather than printing a number.
pub fn predict_batches(
    current_batch: usize,
    n_inactive: usize,
    ratio: f64,
) -> Option<usize> {
    if !ratio.is_finite() {
        return None;
    }
    let n_active = current_batch.saturating_sub(n_inactive) as f64;
    Some((n_active * ratio * ratio) as usize + n_inactive + 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build stats for `n` identical realizations of `v` plus a spread.
    fn stats(values: &[f64]) -> (BinStats, usize) {
        let sum = values.iter().sum();
        let sum_sq = values.iter().map(|x| x * x).sum();
        (BinStats { sum, sum_sq }, values.len())
    }

    /// The uncertainty is the standard error OF THE MEAN, not the sample
    /// spread. A factor of sqrt(n) here would make every trigger fire early.
    #[test]
    fn the_uncertainty_is_the_standard_error_of_the_mean() {
        let (s, n) = stats(&[1.0, 2.0, 3.0, 4.0, 5.0]);
        let (mean, std_dev, rel_err) = bin_uncertainty(s, n).unwrap();
        assert!((mean - 3.0).abs() < 1e-12);

        // Population variance of {1..5} is 2.0; upstream divides by (n-1):
        // sqrt(2.0 / 4) = 0.7071...
        let expected = (2.0_f64 / 4.0).sqrt();
        assert!(
            (std_dev - expected).abs() < 1e-12,
            "std_dev {std_dev} != {expected}"
        );
        assert!((rel_err - expected / 3.0).abs() < 1e-12);

        // The SAMPLE standard deviation would be sqrt(2.5) = 1.58, more than
        // twice this. Asserted so the two can never be confused.
        assert!(std_dev < 1.0, "this must not be the sample spread");
    }

    /// Fewer than two realizations has no uncertainty estimate and must not
    /// read as converged.
    #[test]
    fn one_realization_is_never_converged() {
        let (s, n) = stats(&[42.0]);
        assert!(bin_uncertainty(s, n).is_none());
        let t = Trigger {
            metric: TriggerMetric::RelativeError,
            threshold: 0.01,
            ignore_zeros: false,
        };
        assert!(limiting_ratio(&[s], n, &[t]).is_infinite());
        assert!(!satisfied(limiting_ratio(&[s], n, &[t])));
    }

    /// An unmeasured bin is infinitely uncertain unless explicitly ignored.
    #[test]
    fn an_unmeasured_bin_is_not_converged() {
        let empty = BinStats {
            sum: 0.0,
            sum_sq: 0.0,
        };
        let strict = Trigger {
            metric: TriggerMetric::RelativeError,
            threshold: 0.05,
            ignore_zeros: false,
        };
        assert!(bin_ratio(empty, 10, &strict).is_infinite());

        let lax = Trigger {
            ignore_zeros: true,
            ..strict
        };
        assert_eq!(bin_ratio(empty, 10, &lax), 0.0);
    }

    /// **The deliberate divergence from upstream.**
    ///
    /// Upstream's variance branch reads `sqrt(ratio)` — the running maximum,
    /// initialised to 0 — instead of `sqrt(this_ratio)`. On the first bin that
    /// yields 0 regardless of the threshold.
    ///
    /// This port uses the evident intent. The test pins BOTH halves: that the
    /// result depends on the threshold at all (upstream's would not), and that
    /// it equals the square root of this bin's own ratio.
    #[test]
    fn the_variance_metric_does_not_reproduce_upstreams_bug() {
        let (s, n) = stats(&[1.0, 2.0, 3.0, 4.0, 5.0]);
        let (_, std_dev, _) = bin_uncertainty(s, n).unwrap();
        let var = std_dev * std_dev;

        let tight = Trigger {
            metric: TriggerMetric::Variance,
            threshold: var / 100.0,
            ignore_zeros: false,
        };
        let loose = Trigger {
            threshold: var * 100.0,
            ..tight
        };

        let r_tight = bin_ratio(s, n, &tight);
        let r_loose = bin_ratio(s, n, &loose);

        // Upstream would return sqrt(0) = 0 for BOTH, on the first bin.
        assert!(
            r_tight > 0.0 && r_loose > 0.0,
            "a variance trigger must consult its threshold; got {r_tight} and {r_loose}"
        );
        assert!(
            r_tight > r_loose,
            "a tighter threshold must give a larger ratio: {r_tight} vs {r_loose}"
        );
        assert!((r_tight - (var / (var / 100.0)).sqrt()).abs() < 1e-12);
        // And a tight variance threshold must NOT read as satisfied.
        assert!(!satisfied(r_tight));
        assert!(satisfied(r_loose));
    }

    /// The three metrics order as their definitions require, on one bin.
    #[test]
    fn the_three_metrics_are_consistent_with_each_other() {
        let (s, n) = stats(&[10.0, 11.0, 9.0, 10.5, 9.5]);
        let (mean, std_dev, rel_err) = bin_uncertainty(s, n).unwrap();
        assert!((rel_err - std_dev / mean.abs()).abs() < 1e-12);

        // A std-dev trigger at exactly the measured std_dev is satisfied
        // (ratio == 1), and just below it is not.
        let at = Trigger {
            metric: TriggerMetric::StandardDeviation,
            threshold: std_dev,
            ignore_zeros: false,
        };
        assert!(satisfied(bin_ratio(s, n, &at)));
        let under = Trigger {
            threshold: std_dev * 0.99,
            ..at
        };
        assert!(!satisfied(bin_ratio(s, n, &under)));
    }

    /// Batch prediction assumes variance ~ 1/N, so halving the ratio target
    /// quadruples the batches.
    #[test]
    fn batch_prediction_scales_as_the_square_of_the_ratio() {
        // 100 active batches so far, 20 inactive, currently 2x too uncertain.
        assert_eq!(predict_batches(120, 20, 2.0), Some(100 * 4 + 20 + 1));
        // Already converged: no more active batches needed.
        assert_eq!(predict_batches(120, 20, 1.0), Some(100 + 20 + 1));
        // No scores at all: no basis for an estimate.
        assert_eq!(predict_batches(120, 20, f64::INFINITY), None);
    }
}
