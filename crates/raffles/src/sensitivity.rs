//! Sensitivity analysis — Sobol variance decomposition and correlation measures.
//!
//! **UNIMPLEMENTED.** This module is an empty placeholder. Nothing is exported
//! from it yet, so there is no public path a caller can reach. Do not describe
//! any estimator as available until it appears here with its verification
//! test.
//!
//! ## Scope — what belongs here
//!
//! Importance measures computed *from an existing sample set* — a matrix of
//! input points and the corresponding model outputs:
//!
//! - **Variance-based (Sobol) indices** — first-order `S_i` (the fraction of
//!   output variance explained by input `i` alone) and total-effect `S_Ti`
//!   (that fraction plus every interaction involving `i`). Both are
//!   dimensionless and lie in `[0, 1]`; first-order indices sum to at most 1,
//!   with equality only for a purely additive model.
//! - **The sampling scheme those estimators require**, where it is specific to
//!   them — the Saltelli/Sobol A-B-AB matrix construction is part of the
//!   estimator, not a general-purpose sampler.
//! - **Correlation measures** — Pearson linear correlation, Spearman rank
//!   correlation, partial correlation coefficients (PCC/PRCC), and
//!   standardised regression coefficients (SRC/SRRC). All dimensionless, in
//!   `[-1, 1]`.
//! - **Basic ensemble statistics** where they are what the measures are built
//!   on — sample mean, variance, covariance.
//!
//! ## What does NOT belong here
//!
//! - **Generating the design.** [`crate::samplers`] does that. This module
//!   consumes an already-evaluated sample.
//! - **Surrogate construction.** Sobol indices read off the coefficients of a
//!   polynomial-chaos expansion are a [`crate::surrogate`] capability that
//!   this module may later *consume*; the surrogate itself is not built here.
//! - **Plotting, reporting, file output.**
//!
//! ## Design
//!
//! One enum per family of measure, dispatched by `match` — never
//! `Box<dyn SensitivityMeasure>`. Estimators take slices and return owned
//! results; no lifetime parameters on any type.
//!
//! ## Verification requirement
//!
//! A sensitivity estimator is not done until it reproduces indices that are
//! known analytically. Use, at minimum:
//!
//! - **The Ishigami function** — the standard test function for Sobol
//!   estimators, with closed-form first-order and total indices for its three
//!   inputs at the conventional parameters. An estimator that cannot recover
//!   them within its own sampling error is wrong.
//! - **A purely additive linear model**, where first-order indices are known
//!   exactly and must sum to 1, and total indices must equal first-order
//!   indices (no interactions).
//! - **The Sobol g-function**, whose indices follow from its published
//!   analytic variance decomposition, for a case with strong interactions.
//!
//! For correlation measures, check against a construction with a known
//! correlation matrix, and check the rank measures against a monotone
//! non-linear transform of it (where Spearman is preserved and Pearson is
//! not).
//!
//! Record for every gate: the reference used, the estimator settings and
//! sample size, the measured index values with their sampling uncertainty, the
//! date, and the pass criterion — per the workspace V&V rule, both methodology
//! and results.
//!
//! ## Provenance
//!
//! No RAVEN code has been ported into this module. Note that the relevant
//! upstream area is the one where RAVEN vendors third-party **BSD** code
//! (AMSC, from the University of Utah's Scientific Computing and Imaging
//! Institute; and NGL). Files derived from those parts need the BSD
//! attribution header, not the Apache-2.0 one — check which upstream component
//! a file comes from before writing its header. See the crate `NOTICE` and
//! `NOTICE-RAVEN`.
