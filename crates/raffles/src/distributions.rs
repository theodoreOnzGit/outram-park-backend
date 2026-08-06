//! Probability distributions — densities, CDFs, inverse CDFs, analytic moments.
//!
//! **UNIMPLEMENTED.** This module is an empty placeholder. Nothing is exported
//! from it yet, so there is no public path a caller can reach. Do not describe
//! any distribution as available until it appears here with its verification
//! test.
//!
//! ## Scope — what belongs here
//!
//! The distribution objects themselves, and only those:
//!
//! - **Continuous 1-D**: uniform, normal, log-normal, triangular,
//!   exponential, Weibull, gamma, beta, logistic, Laplace, Gumbel (RAVEN's
//!   `Distributions.py` / `Distributions1D.py`).
//! - **Discrete 1-D**: Bernoulli, binomial, geometric, Poisson, categorical,
//!   uniform-discrete.
//! - **Truncated and shifted variants** of the above — RAVEN supports
//!   truncation on nearly every continuous distribution, and it changes the
//!   normalisation, so it is part of the distribution and not a wrapper the
//!   caller bolts on.
//! - **Multivariate**: multivariate normal and N-D distributions defined on a
//!   grid (RAVEN's `DistributionsND.py`), including the correlation/covariance
//!   structure they need.
//!
//! Each distribution should expose, at minimum: probability density (or mass)
//! function, cumulative distribution function, inverse CDF (percent-point
//! function), support bounds, and its analytic mean and variance.
//!
//! ## What does NOT belong here
//!
//! - **Sampling strategies.** Drawing points is [`crate::samplers`]' job. A
//!   distribution supplies the inverse CDF; the sampler decides *where* in
//!   `[0, 1]` to evaluate it. Keeping that split is what lets Latin hypercube
//!   and grid sampling reuse every distribution unchanged.
//! - **Fitting / inference from data.** Estimating parameters from a sample is
//!   a separate concern and is not currently in scope.
//! - **Random number generation.** The RNG belongs with the samplers.
//!
//! ## Design
//!
//! One enum per family with a variant per concrete distribution, dispatched by
//! `match` — never `Box<dyn Distribution>`. RAVEN's Python class hierarchy
//! must not be transcribed as trait objects. See the crate-level docs and
//! `CLAUDE.md`.
//!
//! ## Verification requirement
//!
//! A distribution is not done until it is checked against something known
//! independently of the implementation: the analytic mean/variance/skewness of
//! the distribution, the CDF/inverse-CDF round trip over the support, and
//! published tabulated values at named quantiles. Record the methodology *and*
//! the measured numbers, per the workspace V&V rule.
//!
//! ## Provenance
//!
//! No RAVEN code has been ported into this module. When it is, each derived
//! file carries the attribution header shown in the crate `CLAUDE.md`, naming
//! the upstream file (`ravenframework/Distributions.py`,
//! `Distributions1D.py`, `DistributionsND.py`), the commit, the copyright
//! holder and the licence.
