// ---------------------------------------------------------------------------
// Ported from SCRAM (a probabilistic risk analysis tool).
//
//   Upstream project: SCRAM — Olzhas Rakhimov
//   Upstream repo:    https://github.com/rakhimov/scram
//   Upstream file:    src/uncertainty_analysis.{h,cc}
//   Upstream commit:  b85b78940de38996eeffec54d946824bd4280a1c  (2019-07-03)
//   Accessed:         2026-09-22
//
//   Copyright (C) 2014-2018 Olzhas Rakhimov
//   Licensed under the GNU General Public License, version 3 or later.
//
// Same-licence port: SCRAM is GPL-3.0-or-later, RAFFLES is GPL-3.0-only.
//
// Translation notes: the statistics are ported formula for formula, including
// the `num_trials / (num_trials - 1)` correction on the variance and the
// `exp(1.96 * sigma)` error factor. Upstream accumulates through
// `boost::accumulators` with `extended_p_square_quantile`, an ONLINE quantile
// estimator; this keeps the samples and sorts them, which is exact for the
// sample. That is a deliberate difference and is stated in the module doc,
// because an online estimator and an exact order statistic do not agree to
// machine precision and pretending otherwise would hide it.
//
// The random stream differs too: upstream uses one static `std::mt19937`,
// this uses `outram_mc_libs::rng::lcg`, the workspace's generator. Two Monte
// Carlo runs from different streams agree statistically and never exactly,
// which is why the verification is statistical.
// ---------------------------------------------------------------------------

//! Uncertainty analysis — what the top-event probability's **distribution**
//! looks like, not just its mean.
//!
//! A basic event defined by a `<lognormal-deviate>` is not a number, it is a
//! distribution. Evaluating it at its mean — which is what every other part of
//! this module does, and what an ordinary SCRAM run does — gives one number
//! and says nothing about the spread. This runs the analysis many times,
//! drawing every deviate afresh each round, and reports the distribution of
//! the answers.
//!
//! # The loop
//!
//! Upstream's `UncertaintyAnalysis::Analyze`, in three steps:
//!
//! 1. gather the basic events whose expression [`Expression::is_deviate`];
//! 2. per trial, `Reset()` and `Sample()` each of them, **clamping to
//!    `[0, 1]`** — upstream's `prob > 1 ? 1 : prob < 0 ? 0 : prob`, which
//!    matters because a normal deviate reaches outside the unit interval;
//! 3. quantify the top event with those probabilities, and collect.
//!
//! **Parameters are sampled once per trial, not once per use.** Upstream
//! memoises a draw per expression object per round, so a parameter feeding
//! several basic events contributes the same draw to all of them. Here a
//! parameter is a name in a table, so the table is sampled first and every
//! event evaluated against it — the same thing by construction rather than by
//! bookkeeping.
//!
//! # What this can and cannot be checked against
//!
//! SCRAM's `--uncertainty` reports the mean, sigma, error factor, confidence
//! interval and quantiles. Those are the oracle. But **the random streams
//! differ** — upstream draws from one static `std::mt19937`, this from
//! `outram_mc_libs::rng::lcg` — so the two runs cannot agree sample for
//! sample, only in distribution. The verification is therefore statistical,
//! and `tests/scram_uncertainty.rs` says at what confidence.
//!
//! The quantiles differ for a second reason: upstream estimates them
//! **online** with `boost::accumulators`' `extended_p_square_quantile`, which
//! is an approximation that never stores the samples, while this sorts the
//! samples and takes the exact order statistic. Both are defensible; they are
//! not the same number.

use super::expression::{Expression, Parameters};
use super::fault_tree::FaultTreeModel;
use crate::{RafflesError, Result};

/// Upstream `Settings::num_trials_`.
pub const DEFAULT_TRIALS: usize = 1_000;

/// Upstream `Settings::num_quantiles_`.
pub const DEFAULT_QUANTILES: usize = 20;

/// Upstream `Settings::num_bins_`.
pub const DEFAULT_BINS: usize = 20;

/// Upstream `Settings::seed_`.
pub const DEFAULT_SEED: u64 = 0;

/// How a Monte Carlo run is configured — upstream's `Settings` fields that
/// uncertainty analysis reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UncertaintySettings {
    /// How many trials to run.
    pub trials: usize,
    /// How many quantiles to report.
    pub quantiles: usize,
    /// How many histogram bins to report.
    pub bins: usize,
    /// The generator's seed.
    pub seed: u64,
}

impl Default for UncertaintySettings {
    fn default() -> Self {
        UncertaintySettings {
            trials: DEFAULT_TRIALS,
            quantiles: DEFAULT_QUANTILES,
            bins: DEFAULT_BINS,
            seed: DEFAULT_SEED,
        }
    }
}

/// The result of a Monte Carlo run — upstream's `UncertaintyAnalysis`
/// accessors.
#[derive(Debug, Clone)]
pub struct UncertaintyResult {
    /// Mean of the sampled top-event probabilities.
    pub mean: f64,
    /// Standard deviation, with upstream's `n / (n - 1)` correction.
    pub sigma: f64,
    /// `exp(1.96 * sigma)` — upstream's 95 % error factor.
    ///
    /// Note this is upstream's formula as written. It is the error factor of a
    /// *log-normal* with that log-scale, so it is only the usual ratio measure
    /// when the distribution is log-normal; on any other shape it is a number
    /// computed from sigma rather than a property of the samples. Ported as
    /// upstream wrote it.
    pub error_factor: f64,
    /// 95 % confidence interval **of the mean**: `mean ± 1.96 sigma / sqrt(n)`.
    pub confidence_interval: (f64, f64),
    /// Quantiles at `1/q, 2/q, …, 1`.
    pub quantiles: Vec<f64>,
    /// The histogram, as `(lower bound, fraction of samples)` pairs.
    ///
    /// **This is not upstream's binning.** Upstream reports the same
    /// *quantity* — its `<bin value=…>` is a fraction, and the twenty of them
    /// sum to 1 — but the edges come from `boost::accumulators`' density
    /// accumulator, whose range is its own and is neither `[min, max]` nor
    /// documented as anything reproducible. On `SmallTree` its first edge is
    /// `0.0021956` and its width `0.0100408`, which is not
    /// `(max - min) / 20`. These bins are equal-width over `[min, max]`, and
    /// `scram_uncertainty` compares the mean, sigma and quantiles rather than
    /// this.
    pub distribution: Vec<(f64, f64)>,
    /// Every sampled top-event probability, in trial order.
    pub samples: Vec<f64>,
}

/// Runs the Monte Carlo — upstream's `UncertaintyAnalysis::Analyze`.
///
/// `quantify` turns one trial's basic-event probabilities into a top-event
/// probability. Which quantification it uses is the caller's choice, exactly
/// as it is upstream: the BDD, inclusion-exclusion over cut sets, or an
/// approximation. It is called once per trial, so its cost sets the run's.
///
/// `parameters` are the model's parameter **expressions**, sampled once per
/// trial before the basic events are — upstream's memoisation, by
/// construction.
///
/// # Errors
///
/// [`RafflesError::InvalidParameter`] if `settings.trials` is below 2 (the
/// variance correction divides by `n - 1`), if a sample or a quantification
/// fails, or if the model has no deviate at all — a Monte Carlo over a
/// deterministic model reports a spread of zero and is almost always a
/// mistake, so it is refused rather than answered.
pub fn analyse<Q>(
    model: &FaultTreeModel,
    expressions: &[Expression],
    parameters: &std::collections::HashMap<String, Expression>,
    mission_time: f64,
    settings: UncertaintySettings,
    mut quantify: Q,
) -> Result<UncertaintyResult>
where
    Q: FnMut(&[f64]) -> Result<f64>,
{
    let bad = |reason: &str, value: f64| RafflesError::InvalidParameter {
        parameter: "uncertainty".to_string(),
        value,
        reason: reason.to_string(),
    };
    if settings.trials < 2 {
        return Err(bad(
            "a Monte Carlo needs at least 2 trials; upstream's variance divides by n - 1",
            settings.trials as f64,
        ));
    }
    if expressions.len() != model.probabilities().len() {
        return Err(bad(
            "one expression per basic event of the tree is needed",
            expressions.len() as f64,
        ));
    }
    let deviate: Vec<usize> = expressions
        .iter()
        .enumerate()
        .filter(|(_, e)| e.is_deviate())
        .map(|(i, _)| i)
        .collect();
    let deviate_parameters: Vec<&String> = parameters
        .iter()
        .filter(|(_, e)| e.is_deviate())
        .map(|(name, _)| name)
        .collect();
    if deviate.is_empty() && deviate_parameters.is_empty() {
        return Err(bad(
            "no basic event and no parameter of this model is a random deviate, so every \
             trial would draw the same numbers; there is no uncertainty to analyse",
            0.0,
        ));
    }

    let mut seed = settings.seed.wrapping_add(1);
    let mut samples = Vec::with_capacity(settings.trials);
    let mut probabilities = model.probabilities().to_vec();
    for _ in 0..settings.trials {
        // Parameters first: one draw per parameter per trial, shared by every
        // event that names it. This IS upstream's per-object memoisation.
        let mut round: Parameters = Parameters::new();
        for _ in 0..parameters.len().max(1) {
            let mut progressed = false;
            for (name, expression) in parameters {
                if round.contains_key(name) {
                    continue;
                }
                if let Ok(v) = expression.sample(&round, mission_time, &mut seed) {
                    round.insert(name.clone(), v);
                    progressed = true;
                }
            }
            if !progressed {
                break;
            }
        }
        for &i in &deviate {
            // Upstream: `prob > 1 ? 1 : prob < 0 ? 0 : prob`. A normal deviate
            // reaches outside [0, 1] and this is what keeps it a probability.
            probabilities[i] = expressions[i]
                .sample(&round, mission_time, &mut seed)?
                .clamp(0.0, 1.0);
        }
        // A non-deviate event whose expression names a sampled parameter also
        // moves, so every event is re-evaluated against this trial's table.
        for (i, expression) in expressions.iter().enumerate() {
            if !deviate.contains(&i) {
                probabilities[i] = expression.evaluate(&round, mission_time)?.clamp(0.0, 1.0);
            }
        }
        samples.push(quantify(&probabilities)?);
    }

    Ok(statistics(samples, settings))
}

/// Upstream's `CalculateStatistics`, over an already-collected sample.
///
/// Separated so that the statistics can be checked against closed forms
/// without running a Monte Carlo, which is what
/// `scram_uncertainty::the_statistics_are_upstreams_formulas` does.
///
/// # Panics
///
/// Never: `analyse` has already refused fewer than two trials, and this is
/// only reachable through it or through a test that supplies its own samples
/// with at least two.
pub fn statistics(samples: Vec<f64>, settings: UncertaintySettings) -> UncertaintyResult {
    let n = samples.len();
    let mean = samples.iter().sum::<f64>() / n as f64;
    // Upstream: `sqrt(num_trials * variance(acc) / (num_trials - 1))`, where
    // boost's `variance` is the POPULATION variance. The correction turns it
    // into the sample standard deviation.
    let population_variance = samples.iter().map(|s| (s - mean).powi(2)).sum::<f64>() / n as f64;
    let sigma = (n as f64 * population_variance / (n as f64 - 1.0)).sqrt();
    let error_factor = (1.96 * sigma).exp();
    let half_width = sigma * 1.96 / (n as f64).sqrt();

    let mut sorted = samples.clone();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let quantiles = (1..=settings.quantiles)
        .map(|i| {
            let p = i as f64 / settings.quantiles as f64;
            // The exact order statistic of the sample. Upstream estimates the
            // same quantity online and approximately -- see the module doc.
            let rank = ((p * n as f64).ceil() as usize).clamp(1, n);
            sorted[rank - 1]
        })
        .collect();

    // Equal-width bins over [min, max], reported as (lower bound, fraction) --
    // the same quantity upstream reports, on edges that are this port's. See
    // the note on `UncertaintyResult::distribution`.
    let (low, high) = (sorted[0], sorted[n - 1]);
    let width = if high > low {
        (high - low) / settings.bins as f64
    } else {
        0.0
    };
    let mut distribution = Vec::with_capacity(settings.bins);
    for b in 0..settings.bins {
        let lower = low + width * b as f64;
        let upper = if b + 1 == settings.bins {
            f64::INFINITY
        } else {
            lower + width
        };
        let count = sorted
            .iter()
            .filter(|s| **s >= lower && (**s < upper || upper.is_infinite()))
            .count();
        distribution.push((lower, count as f64 / n as f64));
    }

    UncertaintyResult {
        mean,
        sigma,
        error_factor,
        confidence_interval: (mean - half_width, mean + half_width),
        quantiles,
        distribution,
        samples,
    }
}
