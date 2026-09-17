//! Surrogate models — cheap reduced-order stand-ins for an expensive model.
//!
//! ## What is implemented
//!
//! - **[`polynomial`]** — multivariate polynomial regression by least squares,
//!   with automatic input rescaling and optional ridge regularisation. Start
//!   here: it fits in closed form, it reproduces a polynomial exactly, and its
//!   coefficients mean something.
//! - **[`neural`]** — a `burn`-trained neural-network regressor, behind the
//!   crate's `burn` feature. For responses a bounded-degree polynomial cannot
//!   express.
//!
//! Gaussian process regression / kriging and the sparse-grid polynomial chaos
//! route are **not** implemented. Neither is the cross-validation machinery;
//! the fitted models report `r_squared` and `rmse` on whatever data they are
//! given, and the tests here demonstrate why that must be held-out data.
//!
//! ## Scope — what belongs here
//!
//! Models fitted to a sample set (inputs and the model outputs at those
//! inputs) and then evaluated in place of re-running the expensive simulation
//! — RAVEN's `SupervisedLearning` / ROM layer:
//!
//! - Polynomial chaos expansions, including the sparse-grid collocation route.
//! - Gaussian process regression / kriging.
//! - Linear and polynomial regression models.
//! - The cross-validation machinery needed to say whether a fit is any good.
//!
//! ## What would NOT belong here
//!
//! - The sample design the surrogate is fitted to ([`crate::samplers`]).
//! - Sensitivity measures ([`crate::sensitivity`]) — though a polynomial chaos
//!   expansion yields Sobol indices directly from its coefficients, so the two
//!   modules will interact once both exist.
//! - Any physics model. A surrogate here approximates a caller's black box.
//!
//! ## Before starting work here
//!
//! Surrogate fitting is where the temptation to add a BLAS/LAPACK dependency
//! appears. Do not. The workspace Android/Termux rule is hard: prefer the
//! pure-Rust `faer` already in the root `[workspace.dependencies]`, and if
//! something BLAS-backed is genuinely unavoidable, declare it under
//! `[target.'cfg(not(target_os = "android"))'.dependencies]` in the same
//! change and note it in the README.
//!
//! ## Design
//!
//! Enum dispatch, as everywhere else in this crate — never
//! `Box<dyn Surrogate>`. No lifetime parameters.
//!
//! ## Verification requirement
//!
//! A surrogate is not done until it is checked against a function whose exact
//! answer is known: a polynomial the expansion should reproduce to machine
//! precision at the right order, and a published test problem (Ishigami,
//! Sobol g-function, or a standard regression benchmark) with reported error
//! metrics. Record the methodology and the measured errors.
//!
//! ## Provenance
//!
//! No RAVEN code has been ported into this module. When it is, each derived
//! file carries the attribution header shown in the crate `CLAUDE.md`, naming
//! the upstream file under `ravenframework/SupervisedLearning/`, the commit,
//! the copyright holder and the licence.

pub mod polynomial;

#[cfg(feature = "burn")]
pub mod neural;

pub use polynomial::PolynomialSurrogate;

#[cfg(feature = "burn")]
pub use neural::{NeuralSurrogate, TrainingConfig, TrainingReport};
