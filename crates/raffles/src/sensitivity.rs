//! Sensitivity analysis — Sobol variance decomposition and correlation measures.
//!
//! Importance measures computed **from an existing sample set**: a matrix of
//! input points and the corresponding model outputs. Nothing here evaluates a
//! model, generates a random design, or knows what the numbers mean physically.
//! The caller runs their own model and hands RAFFLES arrays of `f64`.
//!
//! ## What is implemented
//!
//! - [`sobol_indices`] — first-order `S_i` and total-effect `S_Ti` variance
//!   indices, estimated from a Saltelli-style A / B / A_B^(i) sample.
//! - [`SobolSampleLayout`] — the sample layout that estimator requires: how
//!   many model evaluations `k` inputs and `n` base samples cost, where each
//!   block sits in the output vector, and [`SobolSampleLayout::build_design`]
//!   to assemble the design matrix from two independent base matrices.
//! - [`pearson_correlation`], [`spearman_correlation`],
//!   [`input_output_correlations`], [`CorrelationKind`] — cheap linear and rank
//!   correlation measures, useful alongside the variance-based indices.
//! - [`sample_mean`], [`sample_variance`], [`average_ranks`] — the ensemble
//!   statistics the measures above are built on, exposed because they are
//!   useful on their own.
//!
//! Everything returns [`crate::Result`]; no function in this module panics on
//! caller-supplied data.
//!
//! ## What does NOT belong here
//!
//! - **Generating the design.** [`crate::samplers`] does that. This module
//!   consumes an already-evaluated sample. The one exception is
//!   [`SobolSampleLayout::build_design`], because the A-B-A_B^(i) construction
//!   is part of the *estimator*, not a general-purpose sampling strategy.
//! - **Surrogate construction.** Sobol indices read analytically off the
//!   coefficients of a polynomial-chaos expansion are a [`crate::surrogate`]
//!   capability that this module may later consume; the surrogate itself is not
//!   built here.
//! - **Plotting, reporting, file output.**
//!
//! ## Design
//!
//! Estimators are free functions over slices returning owned results. The one
//! variant family — which correlation coefficient to compute — is the
//! [`CorrelationKind`] enum, dispatched by `match`, never
//! `Box<dyn SensitivityMeasure>`. No type here carries a lifetime parameter.
//!
//! ## The Sobol estimator, stated explicitly
//!
//! For a model `f` of `k` independent inputs, let `A` and `B` be two
//! independent `n x k` sample matrices drawn from the input distribution, and
//! let `A_B^(i)` be `A` with its `i`-th column replaced by the `i`-th column of
//! `B`. Write `y_A = f(A)`, `y_B = f(B)`, `y_AB_i = f(A_B^(i))`, each of length
//! `n`. This module uses:
//!
//! - **Total variance** — the unbiased sample variance of the `2n` values
//!   `{y_A, y_B}` pooled:
//!
//!   `V = (1 / (2n - 1)) * sum over the pooled sample of (y - ybar)^2`
//!
//! - **First-order index** (Saltelli's form of the Sobol'/Homma–Saltelli
//!   estimator):
//!
//!   `V_i = (1/n) * sum_j y_B[j] * (y_AB_i[j] - y_A[j])`,  `S_i = V_i / V`
//!
//! - **Total-effect index** (Jansen's estimator, the one Saltelli et al. (2010)
//!   recommend for `S_Ti`):
//!
//!   `V_Ti = (1/(2n)) * sum_j (y_A[j] - y_AB_i[j])^2`,  `S_Ti = V_Ti / V`
//!
//! Both indices are dimensionless. In exact arithmetic `S_i` lies in `[0, 1]`,
//! `S_Ti` lies in `[0, 1]`, `S_i <= S_Ti`, the `S_i` sum to at most 1 (equality
//! only for a purely additive model), and the `S_Ti` sum to at least 1. **A
//! finite-sample estimate can violate all of these**, and a small negative
//! `S_i` is the normal signature of an index that is truly zero. This module
//! deliberately does **not** clamp the estimates — a clamped index hides
//! exactly the "my sample is too small" signal the caller needs to see.
//!
//! References for the estimator formulas:
//!
//! - I. M. Sobol', *Global sensitivity indices for nonlinear mathematical
//!   models and their Monte Carlo estimates*, Mathematics and Computers in
//!   Simulation **55** (2001) 271–280.
//! - T. Homma and A. Saltelli, *Importance measures in global sensitivity
//!   analysis of nonlinear models*, Reliability Engineering and System Safety
//!   **52** (1996) 1–17.
//! - M. J. W. Jansen, *Analysis of variance designs for model output*,
//!   Computer Physics Communications **117** (1999) 35–43.
//! - A. Saltelli, P. Annoni, I. Azzini, F. Campolongo, M. Ratto and
//!   S. Tarantola, *Variance based sensitivity analysis of model output. Design
//!   and estimator for the total sensitivity index*, Computer Physics
//!   Communications **181** (2010) 259–270.
//!
//! These bibliographic details are given as they are conventionally cited in
//! the sensitivity-analysis literature; they have **not** been checked against
//! the publications themselves and must be verified before appearing in any
//! published V&V write-up.
//!
//! ## Verification — status
//!
//! The estimator is checked against closed-form indices in the `tests` module
//! at the bottom of this file. Measured 2026-08-06; see each test's doc comment
//! for methodology and the numbers actually produced.
//!
//! | Gate | Reference | Achieved |
//! |---|---|---|
//! | Sudret polynomial, `N = 3` | `S_i = 25/91`, `S_Ti = 36/91` exactly | max abs error `2.681e-4` on `S_i`, `3.266e-4` on `S_Ti` at `n = 65536` |
//! | Ishigami function | `S = (0.313905, 0.442411, 0)`, `S_T = (0.557589, 0.442411, 0.243684)` | max abs error `3.723e-4` on `S`, `5.105e-5` on `S_T` at `n = 65536` |
//! | Additive linear model | `S_i = S_Ti = c_i^2 / sum(c^2)`, `sum S_i = 1` | max abs error `2.493e-4`; `sum S_i = 0.999441` |
//! | Pearson / Spearman | exact constructions (`+1`, `-1`, `0`, known `r = 0.6`) | machine precision |
//!
//! **Still open, not claimed:** the Sobol g-function gate named in the crate
//! `CLAUDE.md` verification table is *not* implemented here. It is an
//! 8-input case, so the deterministic 16-dimensional Halton design the other
//! gates use degrades badly (high-index Halton dimensions correlate), and a
//! flaky or quietly-wrong gate is worse than a missing one. It needs a proper
//! low-discrepancy or scrambled sequence, which is [`crate::samplers`]' job.
//!
//! No part of this module has been through **human** review, and nothing here
//! is validated — these are verification gates only ("is it implemented
//! correctly?"), not evidence that any of it represents physical reality.
//!
//! ## Provenance — read before adding to this file
//!
//! **No RAVEN code has been ported into this module.** It is an independent
//! implementation of published algorithms, so per the crate `CLAUDE.md` it
//! carries no upstream attribution header. Checked against RAVEN `devel` at
//! commit `01216937967c38ee287859270c035c8eca906dc6` (accessed 2026-08-06):
//!
//! - RAVEN has **no** Saltelli-style Monte Carlo Sobol estimator. Its Sobol
//!   indices are computed *analytically* from polynomial-chaos coefficients in
//!   `ravenframework/SupervisedLearning/GaussPolynomialRom.py`
//!   (`getSensitivities`, line 613). That is a [`crate::surrogate`] capability,
//!   not this one. The estimator above comes from the papers cited earlier.
//! - RAVEN's Pearson and Spearman counterparts live in
//!   `ravenframework/Models/PostProcessors/BasicStatistics.py` (`corrCoeff`,
//!   line 1401; `spearmanCorrelation`, line 1518). Those are *probability-
//!   weighted* estimators built on `numpy`/`xarray`. This module implements the
//!   unweighted textbook definitions directly and is not a translation of them.
//!   Weighted variants, if wanted later, would be the port.
//!
//! **LICENCE HAZARD — keep this warning in place.** The upstream area adjacent
//! to sensitivity analysis is where RAVEN vendors third-party **BSD** code that
//! is *not* covered by RAVEN's Apache-2.0 grant:
//!
//! - **AMSC** — Copyright 2014 University of Utah, Scientific Computing and
//!   Imaging Institute (3-clause BSD).
//! - **NGL** — Copyright 2012 Carlos D. Correa (2-clause BSD).
//!
//! They sit in `src/AMSC/` and reach the framework through
//! `Models/PostProcessors/TopologicalDecomposition.py`,
//! `SupervisedLearning/MSR.py` and — note the name —
//! `ravenframework/UI/SensitivityView.py`. None of those was read or used here,
//! and nothing in this file derives from them. **Anything derived from AMSC or
//! NGL needs the BSD attribution header, not the Apache-2.0 one.** If a file
//! you are about to port traces back to either, stop and ask rather than
//! guessing at the header. See the crate `NOTICE` and `NOTICE-RAVEN`.

use crate::{RafflesError, Result};
use core::ops::Range;

// ---------------------------------------------------------------------------
// Basic ensemble statistics
// ---------------------------------------------------------------------------

/// Arithmetic mean of a sample.
///
/// `x` is a sample of a scalar quantity in whatever units the caller's model
/// uses; RAFFLES never interprets them. The result carries those same units.
///
/// # Errors
///
/// [`RafflesError::DimensionMismatch`] if `x` is empty (at least one sample is
/// required).
pub fn sample_mean(x: &[f64]) -> Result<f64> {
    if x.is_empty() {
        return Err(RafflesError::DimensionMismatch {
            expected: 1,
            found: 0,
        });
    }
    Ok(x.iter().sum::<f64>() / x.len() as f64)
}

/// Unbiased (Bessel-corrected, `n - 1` denominator) sample variance.
///
/// Non-negative, in the square of the caller's output units. Use this rather
/// than the biased `n` form when the sample is being used to *estimate* a
/// population variance, which is what every measure in this module does.
///
/// # Errors
///
/// [`RafflesError::DimensionMismatch`] if `x` has fewer than two elements — the
/// unbiased variance of a single point is undefined.
pub fn sample_variance(x: &[f64]) -> Result<f64> {
    if x.len() < 2 {
        return Err(RafflesError::DimensionMismatch {
            expected: 2,
            found: x.len(),
        });
    }
    let mean = x.iter().sum::<f64>() / x.len() as f64;
    let ss: f64 = x.iter().map(|v| (v - mean) * (v - mean)).sum();
    Ok(ss / (x.len() as f64 - 1.0))
}

/// Ranks of `x` in ascending order, `1`-based, with **ties resolved by
/// averaging** — the convention Spearman's rank correlation assumes.
///
/// The returned vector has the same length and ordering as `x`: element `j` is
/// the rank of `x[j]`. Ranks lie in `[1, n]` and always sum to `n(n + 1) / 2`.
/// Three tied values occupying ranks 4, 5 and 6 each receive `5.0`.
///
/// Non-finite inputs are not rejected: `NaN` sorts to the end (via
/// [`f64::total_cmp`], which cannot panic) and compares unequal to itself, so
/// each `NaN` receives its own rank. Results in the presence of `NaN` are
/// well-defined but not statistically meaningful.
///
/// # Errors
///
/// [`RafflesError::DimensionMismatch`] if `x` is empty.
pub fn average_ranks(x: &[f64]) -> Result<Vec<f64>> {
    if x.is_empty() {
        return Err(RafflesError::DimensionMismatch {
            expected: 1,
            found: 0,
        });
    }
    let n = x.len();
    let mut order: Vec<usize> = (0..n).collect();
    order.sort_by(|&a, &b| x[a].total_cmp(&x[b]));

    let mut ranks = vec![0.0_f64; n];
    let mut start = 0_usize;
    while start < n {
        let mut end = start + 1;
        while end < n && x[order[end]] == x[order[start]] {
            end += 1;
        }
        // Ranks start+1 ..= end, averaged over the tied group.
        let average = (start + 1 + end) as f64 / 2.0;
        for slot in &order[start..end] {
            ranks[*slot] = average;
        }
        start = end;
    }
    Ok(ranks)
}

// ---------------------------------------------------------------------------
// Correlation measures
// ---------------------------------------------------------------------------

/// Which correlation coefficient to compute.
///
/// Enum dispatch, per the workspace design rules — never a trait object. Both
/// variants produce a dimensionless coefficient in `[-1, 1]`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CorrelationKind {
    /// Pearson product-moment correlation: measures **linear** association.
    /// `+1`/`-1` only for an exactly affine relationship, and it is *not*
    /// preserved by a non-linear monotone transform of either variable.
    Pearson,
    /// Spearman rank correlation: Pearson's coefficient applied to the average
    /// ranks of each variable. Measures **monotone** association, so it is
    /// exactly preserved by any strictly monotone transform, linear or not.
    Spearman,
}

impl CorrelationKind {
    /// Computes whichever coefficient this variant names, for the paired
    /// samples `x` and `y`.
    ///
    /// # Errors
    ///
    /// Same as [`pearson_correlation`] / [`spearman_correlation`].
    pub fn correlation(&self, x: &[f64], y: &[f64]) -> Result<f64> {
        match self {
            Self::Pearson => pearson_correlation(x, y),
            Self::Spearman => spearman_correlation(x, y),
        }
    }
}

/// Pearson product-moment correlation coefficient between two paired samples.
///
/// `x[j]` and `y[j]` are the two quantities observed at sample `j`. The result
/// is dimensionless and lies in `[-1, 1]`: `+1` for a perfectly increasing
/// affine relationship, `-1` for a perfectly decreasing one, `0` for no
/// *linear* association (which is not the same as independence — see the
/// symmetric-parabola case in this module's tests).
///
/// Computed as `cov(x, y) / (sd(x) * sd(y))` with the unbiased `n - 1`
/// denominator throughout; the correction cancels, so the biased form gives the
/// identical coefficient.
///
/// # Errors
///
/// - [`RafflesError::DimensionMismatch`] if the two samples differ in length,
///   or if fewer than two points are supplied.
/// - [`RafflesError::InvalidParameter`] if either sample has zero variance —
///   a constant variable has no correlation with anything, and returning `NaN`
///   silently would hide that.
pub fn pearson_correlation(x: &[f64], y: &[f64]) -> Result<f64> {
    if x.len() != y.len() {
        return Err(RafflesError::DimensionMismatch {
            expected: x.len(),
            found: y.len(),
        });
    }
    if x.len() < 2 {
        return Err(RafflesError::DimensionMismatch {
            expected: 2,
            found: x.len(),
        });
    }
    let n = x.len() as f64;
    let mean_x = x.iter().sum::<f64>() / n;
    let mean_y = y.iter().sum::<f64>() / n;

    let mut sxy = 0.0;
    let mut sxx = 0.0;
    let mut syy = 0.0;
    for (a, b) in x.iter().zip(y.iter()) {
        let dx = a - mean_x;
        let dy = b - mean_y;
        sxy += dx * dy;
        sxx += dx * dx;
        syy += dy * dy;
    }
    if sxx <= 0.0 {
        return Err(RafflesError::InvalidParameter {
            parameter: "x".to_string(),
            value: 0.0,
            reason: "sample has zero variance; correlation with a constant is undefined"
                .to_string(),
        });
    }
    if syy <= 0.0 {
        return Err(RafflesError::InvalidParameter {
            parameter: "y".to_string(),
            value: 0.0,
            reason: "sample has zero variance; correlation with a constant is undefined"
                .to_string(),
        });
    }
    Ok(sxy / (sxx * syy).sqrt())
}

/// Spearman rank correlation coefficient between two paired samples.
///
/// Pearson's coefficient applied to the average ranks (see [`average_ranks`]),
/// so it is dimensionless, lies in `[-1, 1]`, and is **invariant under any
/// strictly monotone transform** of either variable — `+1` for any increasing
/// relationship whether or not it is linear.
///
/// # Errors
///
/// - [`RafflesError::DimensionMismatch`] if the two samples differ in length,
///   or if fewer than two points are supplied.
/// - [`RafflesError::InvalidParameter`] if either sample is entirely tied (all
///   ranks equal), which makes the coefficient undefined.
pub fn spearman_correlation(x: &[f64], y: &[f64]) -> Result<f64> {
    if x.len() != y.len() {
        return Err(RafflesError::DimensionMismatch {
            expected: x.len(),
            found: y.len(),
        });
    }
    let rank_x = average_ranks(x)?;
    let rank_y = average_ranks(y)?;
    pearson_correlation(&rank_x, &rank_y)
}

/// Correlation of every input column against a single scalar output.
///
/// A cheap first look at which inputs matter, and the natural companion to
/// [`sobol_indices`]: it costs one already-evaluated sample rather than the
/// `n * (k + 2)` evaluations the Sobol estimator needs, but it only sees
/// linear ([`CorrelationKind::Pearson`]) or monotone
/// ([`CorrelationKind::Spearman`]) association, and is blind to interactions.
///
/// - `inputs_row_major` — the `n x k` input sample, **row-major**: sample `j`'s
///   value for input `i` is at `inputs_row_major[j * k + i]`.
/// - `inputs` — `k`, the number of input variables.
/// - `outputs` — the `n` model outputs, `outputs[j]` matching sample row `j`.
///
/// Returns `k` coefficients, each in `[-1, 1]`, in input order.
///
/// # Errors
///
/// - [`RafflesError::InvalidParameter`] if `inputs` is zero.
/// - [`RafflesError::DimensionMismatch`] if `inputs_row_major.len()` is not
///   `outputs.len() * inputs`, or if fewer than two samples are supplied.
/// - Whatever [`CorrelationKind::correlation`] returns for a degenerate column.
pub fn input_output_correlations(
    inputs_row_major: &[f64],
    inputs: usize,
    outputs: &[f64],
    kind: CorrelationKind,
) -> Result<Vec<f64>> {
    if inputs == 0 {
        return Err(RafflesError::InvalidParameter {
            parameter: "inputs".to_string(),
            value: 0.0,
            reason: "at least one input variable is required".to_string(),
        });
    }
    let n = outputs.len();
    if n < 2 {
        return Err(RafflesError::DimensionMismatch {
            expected: 2,
            found: n,
        });
    }
    if inputs_row_major.len() != n * inputs {
        return Err(RafflesError::DimensionMismatch {
            expected: n * inputs,
            found: inputs_row_major.len(),
        });
    }

    let mut column = vec![0.0_f64; n];
    let mut out = Vec::with_capacity(inputs);
    for i in 0..inputs {
        for (j, slot) in column.iter_mut().enumerate() {
            *slot = inputs_row_major[j * inputs + i];
        }
        out.push(kind.correlation(&column, outputs)?);
    }
    Ok(out)
}

// ---------------------------------------------------------------------------
// Sobol sample layout
// ---------------------------------------------------------------------------

/// The sample layout the Sobol estimator requires — **get this wrong and the
/// indices are silently wrong**, which is why it is a type rather than a
/// convention in a doc comment.
///
/// For `k` inputs and `n` base samples the estimator needs two independent
/// `n x k` sample matrices `A` and `B`, plus the `k` mixed matrices `A_B^(i)`
/// (`A` with its `i`-th column replaced by `B`'s `i`-th column). That is
/// **`n * (k + 2)` model evaluations**, stacked in this fixed block order:
///
/// | Block | Rows | Accessor |
/// |---|---|---|
/// | `A` | `0 .. n` | [`block_a`](Self::block_a) |
/// | `B` | `n .. 2n` | [`block_b`](Self::block_b) |
/// | `A_B^(0)` | `2n .. 3n` | [`block_ab`](Self::block_ab) |
/// | … | … | … |
/// | `A_B^(k-1)` | `(k+1)n .. (k+2)n` | [`block_ab`](Self::block_ab) |
///
/// Both the design matrix built by [`build_design`](Self::build_design) and the
/// output vector consumed by [`sobol_indices`] use exactly this order.
///
/// ```
/// use raffles::sensitivity::SobolSampleLayout;
///
/// // 3 inputs, 1024 base samples
/// let layout = SobolSampleLayout::new(3, 1024).unwrap();
/// assert_eq!(layout.model_evaluations(), 1024 * 5);
/// assert_eq!(layout.block_a(), 0..1024);
/// assert_eq!(layout.block_b(), 1024..2048);
/// assert_eq!(layout.block_ab(0).unwrap(), 2048..3072);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SobolSampleLayout {
    inputs: usize,
    base_samples: usize,
}

impl SobolSampleLayout {
    /// Describes a Sobol design over `inputs` input variables with
    /// `base_samples` base samples per matrix.
    ///
    /// `base_samples` is the `n` of the formulas in the module docs, **not**
    /// the total evaluation count — see [`model_evaluations`](Self::model_evaluations).
    ///
    /// # Errors
    ///
    /// [`RafflesError::InvalidParameter`] if `inputs` is zero, or if
    /// `base_samples` is below 2 (the pooled variance needs at least two
    /// points per matrix to be estimable).
    pub fn new(inputs: usize, base_samples: usize) -> Result<Self> {
        if inputs == 0 {
            return Err(RafflesError::InvalidParameter {
                parameter: "inputs".to_string(),
                value: 0.0,
                reason: "at least one input variable is required".to_string(),
            });
        }
        if base_samples < 2 {
            return Err(RafflesError::InvalidParameter {
                parameter: "base_samples".to_string(),
                value: base_samples as f64,
                reason: "at least 2 base samples are required to estimate a variance".to_string(),
            });
        }
        Ok(Self {
            inputs,
            base_samples,
        })
    }

    /// Number of input variables, `k`.
    pub fn inputs(&self) -> usize {
        self.inputs
    }

    /// Number of base samples per matrix, `n`.
    pub fn base_samples(&self) -> usize {
        self.base_samples
    }

    /// Total model evaluations this design costs: `n * (k + 2)`.
    ///
    /// This is the number every Sobol study is budgeted by, and the classic
    /// mistake is to read `n` as the total. For 3 inputs and 65536 base
    /// samples it is 327680 evaluations, not 65536.
    pub fn model_evaluations(&self) -> usize {
        self.base_samples * (self.inputs + 2)
    }

    /// Row range of the `A` block within the stacked design / output vector.
    pub fn block_a(&self) -> Range<usize> {
        0..self.base_samples
    }

    /// Row range of the `B` block within the stacked design / output vector.
    pub fn block_b(&self) -> Range<usize> {
        self.base_samples..2 * self.base_samples
    }

    /// Row range of the `A_B^(i)` block — `A` with column `i` taken from `B`.
    ///
    /// # Errors
    ///
    /// [`RafflesError::DimensionMismatch`] if `i` is not a valid input index.
    pub fn block_ab(&self, i: usize) -> Result<Range<usize>> {
        if i >= self.inputs {
            return Err(RafflesError::DimensionMismatch {
                expected: self.inputs,
                found: i,
            });
        }
        let start = (2 + i) * self.base_samples;
        Ok(start..start + self.base_samples)
    }

    /// Assembles the full stacked design matrix from two independent base
    /// matrices.
    ///
    /// - `a`, `b` — the `n x k` base samples, **row-major** (`a[j * k + i]` is
    ///   sample `j`'s value for input `i`). They must be independent draws from
    ///   the same input distribution; reusing one matrix for both makes every
    ///   index meaningless. Values are in the caller's own units — this routine
    ///   only shuffles columns and never interprets them.
    ///
    /// Returns `n * (k + 2)` rows of `k` values, row-major, in the block order
    /// documented on this type. Evaluate the model once per row and pass the
    /// resulting `n * (k + 2)` outputs to [`sobol_indices`].
    ///
    /// # Errors
    ///
    /// [`RafflesError::DimensionMismatch`] if either matrix is not exactly
    /// `n * k` long for this layout's `n` and `k`.
    pub fn build_design(&self, a: &[f64], b: &[f64]) -> Result<Vec<f64>> {
        let k = self.inputs;
        let n = self.base_samples;
        let expected = n * k;
        if a.len() != expected {
            return Err(RafflesError::DimensionMismatch {
                expected,
                found: a.len(),
            });
        }
        if b.len() != expected {
            return Err(RafflesError::DimensionMismatch {
                expected,
                found: b.len(),
            });
        }

        let mut design = Vec::with_capacity(self.model_evaluations() * k);
        design.extend_from_slice(a);
        design.extend_from_slice(b);
        for i in 0..k {
            for j in 0..n {
                let row = &a[j * k..j * k + k];
                design.extend_from_slice(row);
                let last = design.len() - k + i;
                design[last] = b[j * k + i];
            }
        }
        Ok(design)
    }
}

// ---------------------------------------------------------------------------
// Sobol indices
// ---------------------------------------------------------------------------

/// Variance-based sensitivity indices estimated from one Sobol sample.
///
/// All indices are dimensionless fractions of the output variance. See the
/// module docs for the exact estimators and for why the values are **not**
/// clamped to `[0, 1]`.
#[derive(Debug, Clone, PartialEq)]
pub struct SobolIndices {
    /// First-order index `S_i` per input, in input order: the fraction of
    /// output variance explained by input `i` **alone**, averaging over all
    /// the others. Exact range `[0, 1]`; the estimates sum to at most 1, with
    /// equality only for a purely additive model.
    pub first_order: Vec<f64>,
    /// Total-effect index `S_Ti` per input, in input order: the fraction of
    /// output variance explained by input `i` alone **plus every interaction
    /// it takes part in**. Exact range `[0, 1]`, with `S_Ti >= S_i`; the
    /// estimates sum to at least 1. `S_Ti` near zero is the criterion for
    /// fixing an input at a nominal value.
    pub total_effect: Vec<f64>,
    /// Sample mean of the pooled `A` and `B` outputs (`2n` values), in the
    /// caller's output units.
    pub mean: f64,
    /// Unbiased sample variance of the pooled `A` and `B` outputs, the `V`
    /// every index above is divided by. Squared output units, non-negative.
    pub total_variance: f64,
    /// The layout the estimate was computed against — carried so a caller can
    /// report `n` and the evaluation count alongside the numbers.
    pub layout: SobolSampleLayout,
}

/// Estimates first-order and total-effect Sobol indices from an evaluated
/// Saltelli-style sample.
///
/// `outputs` holds the scalar model output for every row of the design
/// described by `layout`, **in that layout's block order** — build it with
/// [`SobolSampleLayout::build_design`] and evaluate row by row, or lay it out
/// yourself using [`SobolSampleLayout::block_a`],
/// [`SobolSampleLayout::block_b`] and [`SobolSampleLayout::block_ab`]. Its
/// length must be exactly [`SobolSampleLayout::model_evaluations`].
///
/// The estimator assumes the `k` inputs are **mutually independent**; the
/// variance decomposition it inverts does not hold for correlated inputs, and
/// this function cannot detect the violation.
///
/// # Errors
///
/// - [`RafflesError::DimensionMismatch`] if `outputs.len()` does not match the
///   layout.
/// - [`RafflesError::InvalidParameter`] if the pooled output variance is zero
///   (a constant model has no sensitivity structure to report, and dividing by
///   it would return `NaN` indices that look like real answers).
pub fn sobol_indices(layout: SobolSampleLayout, outputs: &[f64]) -> Result<SobolIndices> {
    let expected = layout.model_evaluations();
    if outputs.len() != expected {
        return Err(RafflesError::DimensionMismatch {
            expected,
            found: outputs.len(),
        });
    }

    let n = layout.base_samples();
    let k = layout.inputs();
    let y_a = &outputs[layout.block_a()];
    let y_b = &outputs[layout.block_b()];

    // Total variance from the pooled A and B outputs (2n values), unbiased.
    let pooled = 2 * n;
    let mean = (y_a.iter().sum::<f64>() + y_b.iter().sum::<f64>()) / pooled as f64;
    let ss: f64 = y_a
        .iter()
        .chain(y_b.iter())
        .map(|v| (v - mean) * (v - mean))
        .sum();
    let total_variance = ss / (pooled as f64 - 1.0);
    if total_variance <= 0.0 {
        return Err(RafflesError::InvalidParameter {
            parameter: "outputs".to_string(),
            value: total_variance,
            reason: "pooled output variance is zero; a constant model has no sensitivity indices"
                .to_string(),
        });
    }

    let mut first_order = Vec::with_capacity(k);
    let mut total_effect = Vec::with_capacity(k);
    for i in 0..k {
        let y_ab = &outputs[layout.block_ab(i)?];

        // Saltelli's form of the Sobol'/Homma-Saltelli first-order estimator.
        let mut v_i = 0.0;
        // Jansen's total-effect estimator.
        let mut v_ti = 0.0;
        for j in 0..n {
            v_i += y_b[j] * (y_ab[j] - y_a[j]);
            let d = y_a[j] - y_ab[j];
            v_ti += d * d;
        }
        first_order.push(v_i / (n as f64) / total_variance);
        total_effect.push(v_ti / (2.0 * n as f64) / total_variance);
    }

    Ok(SobolIndices {
        first_order,
        total_effect,
        mean,
        total_variance,
        layout,
    })
}

// ---------------------------------------------------------------------------
// Verification
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use core::f64::consts::PI;

    /// Base samples per matrix used by every Sobol gate below. With `k = 3`
    /// that is `65536 * 5 = 327680` model evaluations per gate.
    const N: usize = 65536;

    /// First six primes, the Halton bases for the 6-dimensional design that
    /// supplies `A` (dimensions 0-2) and `B` (dimensions 3-5) for a 3-input
    /// problem.
    const HALTON_BASES: [usize; 6] = [2, 3, 5, 7, 11, 13];

    /// Radical inverse of `index` in `base` — the `index`-th term of the van
    /// der Corput sequence, in `[0, 1)`.
    ///
    /// Deterministic and dependency-free, which is the point: the gates below
    /// use no RNG, so they cannot flake. Index 0 is skipped by the caller
    /// because it yields exactly 0 in every base.
    fn radical_inverse(mut index: usize, base: usize) -> f64 {
        let b = base as f64;
        let mut f = 1.0_f64;
        let mut r = 0.0_f64;
        while index > 0 {
            f /= b;
            r += f * ((index % base) as f64);
            index /= base;
        }
        r
    }

    /// A deterministic 6-dimensional Halton design of `n` points on the unit
    /// hypercube, split into two independent-in-the-low-discrepancy-sense
    /// 3-column halves `(a, b)`, each row-major `n x 3`.
    fn halton_ab(n: usize) -> (Vec<f64>, Vec<f64>) {
        let mut a = Vec::with_capacity(n * 3);
        let mut b = Vec::with_capacity(n * 3);
        for j in 0..n {
            let index = j + 1; // skip the all-zero point
            for d in 0..3 {
                a.push(radical_inverse(index, HALTON_BASES[d]));
            }
            for d in 3..6 {
                b.push(radical_inverse(index, HALTON_BASES[d]));
            }
        }
        (a, b)
    }

    /// Evaluates `model` over every row of a row-major `rows x 3` design.
    fn evaluate(design: &[f64], model: impl Fn(f64, f64, f64) -> f64) -> Vec<f64> {
        design
            .chunks_exact(3)
            .map(|r| model(r[0], r[1], r[2]))
            .collect()
    }

    fn max_abs_error(measured: &[f64], reference: &[f64]) -> f64 {
        measured
            .iter()
            .zip(reference.iter())
            .map(|(m, r)| (m - r).abs())
            .fold(0.0_f64, f64::max)
    }

    // -- Sudret polynomial ---------------------------------------------------

    /// **Sudret's polynomial, `N = 3` — Sobol indices as exact rationals.**
    ///
    /// *Methodology.* Model
    /// `u(y) = (1 / 2^N) * prod_{n=1..N} (1 + 3 y_n^2)` with every `y_n`
    /// uniform on `[0, 1]`, from Sudret, *Global sensitivity analysis using
    /// polynomial chaos expansions*, Reliability Engineering and System Safety
    /// **93** (2008) 964–979; it is the model RAVEN ships as
    /// `tests/framework/AnalyticModels/sudret_sobol_poly.py` and documents in
    /// `doc/tests/sobol_sens.tex`. Because each factor `g_n = (1 + 3 y_n^2)/2`
    /// is independent with `E[g_n] = 1` and `var[g_n] = 1/5`, the closed form
    /// follows directly: `E[u] = 1`, `var[u] = (6/5)^N - 1 = 91/125 = 0.728`,
    /// partial variance of any subset `S` is `(1/5)^|S| / var`, hence for
    /// `N = 3` the **exact rationals** `S_i = 25/91`, `S_ij = 5/91`,
    /// `S_123 = 1/91`, and total-effect `S_Ti = 36/91`. Sudret's own indices
    /// are reproduced from RAVEN's comment block and were re-derived here
    /// independently; they agree.
    ///
    /// *Inputs.* `n = 65536` base samples, `k = 3`, so `327680` model
    /// evaluations. The design is a deterministic 6-dimensional Halton
    /// sequence (bases 2, 3, 5 for `A`; 7, 11, 13 for `B`; index 0 skipped) —
    /// no RNG, so the test is bit-reproducible and cannot flake. Estimators:
    /// Saltelli first-order and Jansen total-effect, as in the module docs.
    ///
    /// *Pass criterion.* Max absolute error below `2e-3` on both `S_i` and
    /// `S_Ti`, and mean/variance within `1e-3` of `1.0` / `0.728`.
    ///
    /// *Results, measured 2026-08-06* (release build, x86_64 Linux):
    ///
    /// - `S = (0.2747259, 0.2747998, 0.2744571)` against `25/91 = 0.2747253`;
    ///   max abs error **`2.681e-4`**.
    /// - `S_T = (0.3957005, 0.3955792, 0.3959310)` against
    ///   `36/91 = 0.3956044`; max abs error **`3.266e-4`**.
    /// - `mean = 0.999756` against `1.0`; `variance = 0.727560` against
    ///   `0.728`.
    ///
    /// *Interpretation.* The estimator recovers exactly-known rational indices
    /// to ~3e-4 at 3.3e5 evaluations, i.e. roughly 0.1% of the index value —
    /// consistent with a correct Saltelli/Jansen implementation on a
    /// quasi-random design. The residual is estimator sampling error, not a
    /// bias: the mean and variance are recovered to `2.4e-4` and `4.4e-4` on
    /// the same sample. This verifies the implementation only; it says nothing
    /// about validity for any physical problem.
    #[test]
    fn sudret_polynomial_matches_exact_rational_indices() {
        let layout = SobolSampleLayout::new(3, N).unwrap();
        let (a, b) = halton_ab(N);
        let design = layout.build_design(&a, &b).unwrap();
        let outputs = evaluate(&design, |y1, y2, y3| {
            (1.0 + 3.0 * y1 * y1) * (1.0 + 3.0 * y2 * y2) * (1.0 + 3.0 * y3 * y3) / 8.0
        });
        let s = sobol_indices(layout, &outputs).unwrap();

        let exact_first = [25.0 / 91.0; 3];
        let exact_total = [36.0 / 91.0; 3];
        let err_first = max_abs_error(&s.first_order, &exact_first);
        let err_total = max_abs_error(&s.total_effect, &exact_total);

        println!(
            "sudret n={} S={:?} maxErrS={:.3e} ST={:?} maxErrST={:.3e} mean={:.6} var={:.6}",
            N, s.first_order, err_first, s.total_effect, err_total, s.mean, s.total_variance
        );

        assert!(err_first < 2e-3, "first-order error {err_first:e}");
        assert!(err_total < 2e-3, "total-effect error {err_total:e}");
        assert!((s.mean - 1.0).abs() < 1e-3, "mean {}", s.mean);
        assert!(
            (s.total_variance - 0.728).abs() < 1e-3,
            "variance {}",
            s.total_variance
        );
    }

    // -- Ishigami function ---------------------------------------------------

    /// **Ishigami function — the standard Sobol benchmark, closed-form
    /// partial variances.**
    ///
    /// *Methodology.* Model
    /// `u(x) = sin(x1) + a sin^2(x2) + b x3^4 sin(x1)` with `a = 7`,
    /// `b = 0.1` and `x1, x2, x3` uniform on `[-pi, pi]`, from Ishigami and
    /// Homma (1990), restated in Sudret (2008); RAVEN ships it as
    /// `tests/framework/AnalyticModels/ishigami.py`. The closed-form
    /// decomposition, re-derived here and cross-checked against RAVEN's
    /// comment block, is
    /// `D = a^2/8 + b pi^4/5 + b^2 pi^8/18 + 1/2 = 13.844588`,
    /// `D1 = b pi^4/5 + b^2 pi^8/50 + 1/2 = 4.345888`, `D2 = a^2/8 = 6.125`,
    /// `D3 = 0`, `D13 = 8 b^2 pi^8/225 = 3.373700`, all other terms zero
    /// (`D1 + D2 + D13 = D` identically, which is itself a check on the
    /// reference values). Hence `S = (D1, D2, 0)/D` and
    /// `S_T = (D1 + D13, D2, D13)/D`.
    ///
    /// The `S_3 = 0` entry is the sharpest part of the gate: `x3` has **no**
    /// first-order effect at all yet a large total effect (0.2437) through its
    /// interaction with `x1`, so an estimator that confuses the two, or that
    /// clamps negative first-order estimates, fails visibly here.
    ///
    /// *Inputs.* Same design as the Sudret gate — `n = 65536`, `k = 3`,
    /// `327680` evaluations, deterministic 6-dimensional Halton, unit-cube
    /// points mapped to `[-pi, pi]` by `x = -pi + 2 pi u`.
    ///
    /// *Pass criterion.* Max absolute error below `2e-3` on `S` and on `S_T`;
    /// total variance within `2e-3` of `13.844588`; the true-zero `S_3`
    /// within `2e-3` of zero.
    ///
    /// *Results, measured 2026-08-06* (release build, x86_64 Linux):
    ///
    /// - `S = (0.3137574, 0.4422647, -0.0003723)` against
    ///   `(0.3139052, 0.4424111, 0)`; max abs error **`3.723e-4`**.
    /// - `S_T = (0.5575378, 0.4424477, 0.2437088)` against
    ///   `(0.5575889, 0.4424111, 0.2436837)`; max abs error **`5.105e-5`**.
    /// - `variance = 13.844014` against `13.844588`.
    ///
    /// *Interpretation.* `S_3` comes out at `-3.72e-4` — negative, as a
    /// finite-sample estimate of a true zero should be, and three orders of
    /// magnitude below `S_1` and `S_2`. `S_T3 = 0.2437088` recovers the pure
    /// `x1`-`x3` interaction to `2.5e-5`. Both are what a correct
    /// Saltelli/Jansen pair does on this function; verification only, no
    /// validity claim.
    #[test]
    fn ishigami_matches_analytic_indices() {
        const A: f64 = 7.0;
        const B: f64 = 0.1;

        let d = A * A / 8.0 + B * PI.powi(4) / 5.0 + B * B * PI.powi(8) / 18.0 + 0.5;
        let d1 = B * PI.powi(4) / 5.0 + B * B * PI.powi(8) / 50.0 + 0.5;
        let d2 = A * A / 8.0;
        let d13 = 8.0 * B * B * PI.powi(8) / 225.0;
        // The decomposition must be exact; a slip in the reference values
        // themselves would show up here before the estimator is even run.
        assert!((d1 + d2 + d13 - d).abs() < 1e-9);

        let exact_first = [d1 / d, d2 / d, 0.0];
        let exact_total = [(d1 + d13) / d, d2 / d, d13 / d];

        let layout = SobolSampleLayout::new(3, N).unwrap();
        let (a, b) = halton_ab(N);
        let design = layout.build_design(&a, &b).unwrap();
        let outputs = evaluate(&design, |u1, u2, u3| {
            let x1 = -PI + 2.0 * PI * u1;
            let x2 = -PI + 2.0 * PI * u2;
            let x3 = -PI + 2.0 * PI * u3;
            x1.sin() + A * x2.sin().powi(2) + B * x3.powi(4) * x1.sin()
        });
        let s = sobol_indices(layout, &outputs).unwrap();

        let err_first = max_abs_error(&s.first_order, &exact_first);
        let err_total = max_abs_error(&s.total_effect, &exact_total);

        println!(
            "ishigami n={} S={:?} maxErrS={:.3e} ST={:?} maxErrST={:.3e} var={:.6} (exact D={:.6})",
            N, s.first_order, err_first, s.total_effect, err_total, s.total_variance, d
        );

        assert!(err_first < 2e-3, "first-order error {err_first:e}");
        assert!(err_total < 2e-3, "total-effect error {err_total:e}");
        assert!(
            (s.total_variance - d).abs() < 2e-3,
            "variance {} vs {}",
            s.total_variance,
            d
        );
        // S_3 is exactly zero; the estimate must be numerically small, and is
        // deliberately allowed to be negative.
        assert!(s.first_order[2].abs() < 2e-3, "S_3 {}", s.first_order[2]);
    }

    // -- Additive linear model ----------------------------------------------

    /// **Purely additive linear model — first-order indices sum to 1 and equal
    /// the total indices.**
    ///
    /// *Methodology.* `u(y) = c1 y1 + c2 y2 + c3 y3` with `c = (1, 2, 3)` and
    /// `y_n` uniform on `[0, 1]`. Each `var[c_n y_n] = c_n^2 / 12`, the inputs
    /// are independent and there are no interaction terms, so
    /// `S_i = S_Ti = c_i^2 / sum(c^2)` exactly:
    /// `(1/14, 4/14, 9/14) = (0.0714286, 0.2857143, 0.6428571)`, with
    /// `sum S_i = 1` and total variance `14/12 = 1.1666667`. This is the gate
    /// that would catch a first-order/total-effect mix-up, since the two must
    /// coincide.
    ///
    /// *Inputs.* Same deterministic design: `n = 65536`, `k = 3`, `327680`
    /// evaluations, 6-dimensional Halton.
    ///
    /// *Pass criterion.* Max abs error below `5e-3` on `S` and on `S_T`;
    /// `|sum S_i - 1| < 5e-3`; total variance within `5e-3` of `1.1666667`.
    ///
    /// *Results, measured 2026-08-06* (release build, x86_64 Linux):
    ///
    /// - `S = (0.0713310, 0.2854650, 0.6426454)` against
    ///   `(0.0714286, 0.2857143, 0.6428571)`; max abs error **`2.493e-4`**.
    /// - `S_T = (0.0714363, 0.2857442, 0.6429543)`; max abs error
    ///   **`9.715e-5`**.
    /// - `sum S_i = 0.999441`; `variance = 1.166564` against `1.1666667`.
    ///
    /// *Interpretation.* First-order and total indices agree to `3.1e-4` of
    /// each other, and the first-order indices sum to 1 within `5.6e-4` — the
    /// additivity signature is reproduced. Verification only.
    #[test]
    fn additive_model_has_no_interactions() {
        let c = [1.0_f64, 2.0, 3.0];
        let denom: f64 = c.iter().map(|v| v * v).sum();
        let exact: Vec<f64> = c.iter().map(|v| v * v / denom).collect();

        let layout = SobolSampleLayout::new(3, N).unwrap();
        let (a, b) = halton_ab(N);
        let design = layout.build_design(&a, &b).unwrap();
        let outputs = evaluate(&design, |y1, y2, y3| c[0] * y1 + c[1] * y2 + c[2] * y3);
        let s = sobol_indices(layout, &outputs).unwrap();

        let err_first = max_abs_error(&s.first_order, &exact);
        let err_total = max_abs_error(&s.total_effect, &exact);
        let sum_first: f64 = s.first_order.iter().sum();

        println!(
            "additive n={} S={:?} maxErrS={:.3e} ST={:?} maxErrST={:.3e} sumS={:.6} var={:.6}",
            N, s.first_order, err_first, s.total_effect, err_total, sum_first, s.total_variance
        );

        assert!(err_first < 5e-3, "first-order error {err_first:e}");
        assert!(err_total < 5e-3, "total-effect error {err_total:e}");
        assert!((sum_first - 1.0).abs() < 5e-3, "sum of S_i = {sum_first}");
        assert!(
            (s.total_variance - denom / 12.0).abs() < 5e-3,
            "variance {}",
            s.total_variance
        );
    }

    // -- Layout arithmetic ---------------------------------------------------

    /// **Sample-layout arithmetic and design assembly — exact, deterministic.**
    ///
    /// *Methodology.* The `n * (k + 2)` evaluation count and the block offsets
    /// are the classic error in a Sobol study, so they are asserted directly
    /// rather than inferred from a converged index. For `k = 3`, `n = 4`:
    /// `model_evaluations() == 20`, `A` at rows `0..4`, `B` at `4..8`,
    /// `A_B^(i)` at `(2+i)*4`. The assembled design is then checked
    /// element-wise: block `A_B^(i)` must equal `A` in every column except `i`,
    /// and equal `B` in column `i`.
    ///
    /// *Pass criterion.* Exact equality — no tolerance.
    ///
    /// *Results, measured 2026-08-06.* All assertions hold exactly; the design
    /// has 20 rows of 3 values (60 elements) and every column-substitution is
    /// in the right place.
    #[test]
    fn layout_and_design_assembly_are_exact() {
        let k = 3;
        let n = 4;
        let layout = SobolSampleLayout::new(k, n).unwrap();
        assert_eq!(layout.model_evaluations(), n * (k + 2));
        assert_eq!(layout.block_a(), 0..4);
        assert_eq!(layout.block_b(), 4..8);
        assert_eq!(layout.block_ab(0).unwrap(), 8..12);
        assert_eq!(layout.block_ab(2).unwrap(), 16..20);
        assert!(layout.block_ab(3).is_err());

        // A[j][i] = 10*j + i, B[j][i] = 100 + 10*j + i — every value distinct,
        // so a misplaced column is unmissable.
        let a: Vec<f64> = (0..n)
            .flat_map(|j| (0..k).map(move |i| (10 * j + i) as f64))
            .collect();
        let b: Vec<f64> = (0..n)
            .flat_map(|j| (0..k).map(move |i| (100 + 10 * j + i) as f64))
            .collect();

        let design = layout.build_design(&a, &b).unwrap();
        assert_eq!(design.len(), layout.model_evaluations() * k);
        assert_eq!(&design[0..n * k], &a[..]);
        assert_eq!(&design[n * k..2 * n * k], &b[..]);

        for i in 0..k {
            let block = layout.block_ab(i).unwrap();
            for j in 0..n {
                for col in 0..k {
                    let got = design[(block.start + j) * k + col];
                    let want = if col == i {
                        b[j * k + col]
                    } else {
                        a[j * k + col]
                    };
                    assert_eq!(got, want, "A_B^({i}) row {j} col {col}");
                }
            }
        }
    }

    /// **Shape mismatches return `Err`, never a panic.**
    ///
    /// *Methodology.* Every public entry point is handed deliberately wrong
    /// shapes — zero inputs, one base sample, an output vector of the wrong
    /// length, mismatched correlation samples, a constant (zero-variance)
    /// sample, an out-of-range input index — and must return a
    /// [`RafflesError`] rather than panic or return a silent `NaN`.
    ///
    /// *Pass criterion.* Every call returns `Err`; the test process does not
    /// abort.
    ///
    /// *Results, measured 2026-08-06.* All eight cases return `Err` as
    /// specified; no panics.
    #[test]
    fn caller_shape_errors_are_reported_not_panicked() {
        assert!(SobolSampleLayout::new(0, 16).is_err());
        assert!(SobolSampleLayout::new(3, 1).is_err());

        let layout = SobolSampleLayout::new(2, 8).unwrap();
        assert!(sobol_indices(layout, &[0.0; 10]).is_err()); // needs 8*4 = 32
        assert!(layout.build_design(&[0.0; 15], &[0.0; 16]).is_err());
        // Constant model: zero pooled variance.
        assert!(sobol_indices(layout, &[1.0; 32]).is_err());

        assert!(pearson_correlation(&[1.0, 2.0], &[1.0]).is_err());
        assert!(pearson_correlation(&[1.0, 1.0], &[1.0, 2.0]).is_err());
        assert!(spearman_correlation(&[1.0, 2.0, 3.0], &[1.0, 2.0]).is_err());
        assert!(input_output_correlations(
            &[1.0, 2.0, 3.0],
            2,
            &[1.0, 2.0],
            CorrelationKind::Pearson
        )
        .is_err());
    }

    // -- Correlation measures ------------------------------------------------

    /// **Pearson and Spearman against exactly-known constructions.**
    ///
    /// *Methodology.* Five constructions whose coefficients are known in closed
    /// form, so the reference is exact rather than sampled:
    ///
    /// 1. **Perfect positive** — `y = 2x + 1`: Pearson and Spearman both `+1`.
    /// 2. **Perfect negative** — `y = -3x + 7`: both `-1`.
    /// 3. **Uncorrelated but dependent** — `x = (-2,-1,0,1,2)`,
    ///    `y = x^2 = (4,1,0,1,4)`. The centred cross-product sums to exactly 0,
    ///    so Pearson is exactly 0 while `y` is a deterministic function of `x`.
    ///    Spearman is also exactly 0 here (the symmetric tie structure makes
    ///    the rank cross-product vanish), which additionally exercises the
    ///    average-ties rank convention.
    /// 4. **Known non-trivial `r`** — with `x = (-3,-1,1,3)` and the orthogonal
    ///    `z = (1,-1,-1,1)`, both centred, `y = 0.6 x/|x| + 0.8 z/|z|` has
    ///    Pearson correlation with `x` of exactly `0.6` by construction.
    /// 5. **Monotone non-linear transform** — `y = exp(x)`. Spearman is exactly
    ///    `+1` (rank-preserving); Pearson is strictly less than 1. This is the
    ///    gate that distinguishes the two measures.
    ///
    /// *Pass criterion.* `1e-12` of the exact value for cases 1-4 and for the
    /// Spearman half of case 5; for the Pearson half of case 5, strictly
    /// between 0.5 and 0.999.
    ///
    /// *Results, measured 2026-08-06* (release build, x86_64 Linux):
    ///
    /// - Case 1: Pearson `1.000000000000000`, Spearman `1.000000000000000`.
    /// - Case 2: Pearson `-1.000000000000000`, Spearman `-1.000000000000000`.
    /// - Case 3: Pearson `0.000000000000000`, Spearman `0.000000000000000`
    ///   (both exactly zero, not merely small).
    /// - Case 4: Pearson `0.600000000000000` against the constructed `0.6`.
    /// - Case 5: Spearman `1.000000000000000`; Pearson `0.886275` — clearly
    ///   below 1, confirming the two measures are not the same computation.
    ///
    /// *Interpretation.* Every exact case is recovered to machine precision,
    /// including the two that are exactly zero. Verification only.
    #[test]
    fn correlations_match_exact_constructions() {
        let x = [1.0_f64, 2.0, 3.0, 4.0, 5.0];

        // 1. perfectly correlated
        let y_pos: Vec<f64> = x.iter().map(|v| 2.0 * v + 1.0).collect();
        let p_pos = pearson_correlation(&x, &y_pos).unwrap();
        let s_pos = spearman_correlation(&x, &y_pos).unwrap();
        assert!((p_pos - 1.0).abs() < 1e-12, "pearson +1: {p_pos}");
        assert!((s_pos - 1.0).abs() < 1e-12, "spearman +1: {s_pos}");

        // 2. perfectly anti-correlated
        let y_neg: Vec<f64> = x.iter().map(|v| -3.0 * v + 7.0).collect();
        let p_neg = pearson_correlation(&x, &y_neg).unwrap();
        let s_neg = spearman_correlation(&x, &y_neg).unwrap();
        assert!((p_neg + 1.0).abs() < 1e-12, "pearson -1: {p_neg}");
        assert!((s_neg + 1.0).abs() < 1e-12, "spearman -1: {s_neg}");

        // 3. uncorrelated (but dependent): symmetric parabola
        let xs = [-2.0_f64, -1.0, 0.0, 1.0, 2.0];
        let ys: Vec<f64> = xs.iter().map(|v| v * v).collect();
        let p_zero = pearson_correlation(&xs, &ys).unwrap();
        let s_zero = spearman_correlation(&xs, &ys).unwrap();
        assert!(p_zero.abs() < 1e-12, "pearson 0: {p_zero}");
        assert!(s_zero.abs() < 1e-12, "spearman 0: {s_zero}");

        // 4. constructed r = 0.6
        let xr = [-3.0_f64, -1.0, 1.0, 3.0];
        let zr = [1.0_f64, -1.0, -1.0, 1.0];
        let nx = (xr.iter().map(|v| v * v).sum::<f64>()).sqrt();
        let nz = (zr.iter().map(|v| v * v).sum::<f64>()).sqrt();
        let yr: Vec<f64> = (0..4)
            .map(|j| 0.6 * xr[j] / nx + 0.8 * zr[j] / nz)
            .collect();
        let p_r = pearson_correlation(&xr, &yr).unwrap();
        assert!((p_r - 0.6).abs() < 1e-12, "pearson 0.6: {p_r}");

        // 5. monotone non-linear transform: Spearman preserved, Pearson not
        let y_exp: Vec<f64> = x.iter().map(|v| v.exp()).collect();
        let p_exp = pearson_correlation(&x, &y_exp).unwrap();
        let s_exp = spearman_correlation(&x, &y_exp).unwrap();
        assert!((s_exp - 1.0).abs() < 1e-12, "spearman exp: {s_exp}");
        assert!(p_exp > 0.5 && p_exp < 0.999, "pearson exp: {p_exp}");

        println!(
            "correlations: +1=({p_pos:.15},{s_pos:.15}) -1=({p_neg:.15},{s_neg:.15}) \
             0=({p_zero:.15},{s_zero:.15}) r=({p_r:.15}) exp=(P {p_exp:.6}, S {s_exp:.15})"
        );
    }

    /// **Average-tie ranking and the `CorrelationKind` / column-wise wrappers.**
    ///
    /// *Methodology.* Ranks of `(3, 1, 4, 1, 5)` must be
    /// `(3, 1.5, 4, 1.5, 5)` — the two tied `1`s share ranks 1 and 2 — and any
    /// rank vector must sum to `n(n+1)/2 = 15`. Then a `4 x 2` input matrix
    /// with `y` equal to its own second column is pushed through
    /// [`input_output_correlations`] under both [`CorrelationKind`] variants:
    /// the second coefficient must be exactly `+1` under both, and the enum
    /// must agree with the free functions it dispatches to.
    ///
    /// *Pass criterion.* Exact equality for the ranks; `1e-12` for the
    /// coefficients.
    ///
    /// *Results, measured 2026-08-06.* Ranks `(3, 1.5, 4, 1.5, 5)`, sum `15`,
    /// exactly as specified. `input_output_correlations` returns a `+1` second
    /// coefficient under both variants, and `CorrelationKind::correlation`
    /// matches [`pearson_correlation`] / [`spearman_correlation`] bit-for-bit.
    #[test]
    fn ranking_and_column_wise_wrappers_are_correct() {
        let ranks = average_ranks(&[3.0, 1.0, 4.0, 1.0, 5.0]).unwrap();
        assert_eq!(ranks, vec![3.0, 1.5, 4.0, 1.5, 5.0]);
        assert_eq!(ranks.iter().sum::<f64>(), 15.0);

        // 4 samples, 2 inputs, row-major.
        let xs = [1.0, 5.0, 2.0, 3.0, 3.0, 9.0, 4.0, 1.0];
        let y = [5.0, 3.0, 9.0, 1.0]; // exactly input column 1

        for kind in [CorrelationKind::Pearson, CorrelationKind::Spearman] {
            let c = input_output_correlations(&xs, 2, &y, kind).unwrap();
            assert_eq!(c.len(), 2);
            assert!(
                (c[1] - 1.0).abs() < 1e-12,
                "{kind:?} self-correlation {c:?}"
            );
        }

        let a = [1.0, 2.0, 4.0, 8.0];
        let b = [1.0, 3.0, 2.0, 9.0];
        assert_eq!(
            CorrelationKind::Pearson.correlation(&a, &b).unwrap(),
            pearson_correlation(&a, &b).unwrap()
        );
        assert_eq!(
            CorrelationKind::Spearman.correlation(&a, &b).unwrap(),
            spearman_correlation(&a, &b).unwrap()
        );
    }
}
