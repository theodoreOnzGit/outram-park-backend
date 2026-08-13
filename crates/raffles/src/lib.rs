//! # RAFFLES — Risk Analysis Framework For Learning & Ensemble Simulation
//!
//! An independent pure-Rust port of the uncertainty-quantification (UQ) and
//! risk-analysis core of [RAVEN](https://github.com/idaholab/raven), the
//! probabilistic risk-analysis / UQ framework developed by Idaho National
//! Laboratory. Upstream is Apache-2.0; RAFFLES is GPL-3.0-only. That direction
//! is **one-way** — see [Licensing](#licensing) below.
//!
//! **Owner: Adolphus Lye.** They chose the RAFFLES backronym and this crate is
//! theirs to steer; changes of direction are their call. See the crate
//! `CLAUDE.md`.
//!
//! ## Status — SCAFFOLD ONLY
//!
//! Nothing is implemented. Every module below is an empty, documented
//! placeholder that states its own scope. No distribution, sampler, estimator
//! or surrogate exists yet, and none of this has been through human V&V. Do
//! not describe any part of this crate as working, verified or validated.
//!
//! ## What belongs in this crate
//!
//! The *statistical machinery* for running ensembles of simulations and
//! reasoning about the spread of their answers:
//!
//! - **[`distributions`]** — probability distributions: densities, cumulative
//!   distribution functions, inverse CDFs, analytic moments.
//! - **[`samplers`]** — strategies that turn distributions into a concrete set
//!   of sample points: Monte Carlo, Latin hypercube, grid / stratified.
//! - **[`sensitivity`]** — importance measures computed from an existing
//!   sample set: Sobol variance decomposition, correlation coefficients.
//! - **[`surrogate`]** — reduced-order models fitted to a sample set and
//!   evaluated in place of the expensive simulation.
//!
//! ## What does NOT belong in this crate
//!
//! - **Physics.** RAFFLES holds no reactor, thermal-hydraulic, neutronic or
//!   chemistry model. It samples inputs and consumes outputs; the physics
//!   lives in the other Outram Park crates.
//! - **Simulation drivers, job scheduling, file/XML input parsing, plotting,
//!   databases.** RAVEN is a whole workflow application; RAFFLES ports only
//!   its statistical core. A caller drives their own runs and hands RAFFLES
//!   arrays of numbers.
//! - **Optimisation.** RAVEN's optimisers (gradient descent, genetic
//!   algorithms, Bayesian optimisation) are out of scope unless the crate
//!   owner decides otherwise.
//! - **Anything Android-hostile.** No system BLAS/LAPACK, no C or Fortran
//!   toolchain, no GUI. The crate must build natively on Termux
//!   (`aarch64-linux-android`). If dense linear algebra becomes necessary,
//!   prefer the pure-Rust `faer` already in the workspace, and target-gate
//!   anything BLAS-backed off Android in the same change.
//!
//! ## Units
//!
//! RAFFLES quantities are dimensionless by nature — probabilities, quantiles,
//! variance fractions, correlation coefficients, sample counts — so `uom` is
//! deliberately not used here. Sample *values* are plain `f64` in whatever
//! units the caller's model uses; RAFFLES never interprets them physically.
//! Where a doc comment gives a range it is an ordinary numeric range, e.g. a
//! probability in `[0, 1]` or a Sobol index in `[0, 1]`.
//!
//! ## Design rules that bind every module here
//!
//! RAVEN is deeply inheritance-based (`Sampler` -> `ForwardSampler` ->
//! `MonteCarlo`, and so on). That structure must **not** be transcribed into
//! Rust as trait objects. Per the workspace design rules:
//!
//! - **Enum dispatch, never `Box<dyn Trait>` / `&dyn Trait` / `Arc<dyn Trait>`.**
//!   The set of distributions and samplers is closed and known at compile
//!   time, so each family becomes one enum with a variant per concrete model.
//!   A trait may still be used as a compiler-enforced contract on the concrete
//!   structs — just not for runtime dispatch.
//! - **No `Box<T>`** — own by value, or share with `Arc<T>`.
//! - **No lifetime parameters** on structs, traits or impls — own the data, or
//!   share it with `Arc<T>`.
//!
//! ## Verification
//!
//! Nothing here is "done" until it is checked against something that is known
//! independently: analytic moments for a distribution, the published Sobol
//! indices of the Ishigami function for a sensitivity estimator, a published
//! test problem for anything else. The workspace V&V rule requires both the
//! *methodology* and the measured *results* to be written down. See
//! `CLAUDE.md` in this crate.
//!
//! ## Licensing
//!
//! Upstream RAVEN is **Apache-2.0**; RAFFLES is **GPL-3.0-only**. Apache-2.0
//! code may be taken into a GPLv3 work, but GPLv3 code may **not** be taken
//! into an Apache-2.0 work. Code therefore flows RAVEN -> RAFFLES and never
//! RAFFLES -> RAVEN. Do not contribute RAFFLES code upstream. Full provenance,
//! the required Battelle Energy Alliance / Idaho National Laboratory
//! attribution, and the verbatim upstream licence text are in the crate's
//! `NOTICE`, `LICENSE-APACHE-RAVEN` and `NOTICE-RAVEN`.
//!
//! ## Intended use
//!
//! Education, research, capability building and V&V only. Despite the name,
//! RAFFLES is **not** for nuclear facility operation, reactor control,
//! licensing decisions, probabilistic safety assessment of a real facility,
//! safety-critical decision-making or emergency response.
//!
//! ## Scoping
//!
//! The port scope — which RAVEN capabilities are in, which are out, and in
//! what order — is written up in `docs/raven-port-scoping.md` at the workspace
//! root.

pub mod distributions;
pub mod samplers;
pub mod sensitivity;
pub mod surrogate;

/// Errors produced by RAFFLES.
///
/// Deliberately small: these are the two failure shapes every planned module
/// needs (a caller-supplied parameter that cannot describe a valid
/// distribution or design, and a shape mismatch between arrays). Variants are
/// added as real code lands — this enum is scaffold, not a finished taxonomy.
///
/// No variant here signals "unimplemented". The unimplemented parts of RAFFLES
/// have no public entry point at all, so a caller cannot reach one by
/// accident.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum RafflesError {
    /// A caller-supplied parameter is outside the range the model admits —
    /// for example a negative standard deviation, or a probability outside
    /// `[0, 1]`.
    ///
    /// `parameter` names the offending argument as the caller wrote it,
    /// `value` is what was passed, and `reason` states the constraint that was
    /// violated.
    #[error("invalid parameter `{parameter}` = {value}: {reason}")]
    InvalidParameter {
        /// Name of the offending parameter, as it appears in the public API.
        parameter: String,
        /// The value that was rejected.
        value: f64,
        /// The constraint it violated, phrased for a human reader.
        reason: String,
    },

    /// Two arrays that had to agree in length or dimension did not — for
    /// example a sample matrix whose column count does not match the number of
    /// input variables.
    #[error("dimension mismatch: expected {expected}, found {found}")]
    DimensionMismatch {
        /// The length or dimension that was required.
        expected: usize,
        /// The length or dimension actually supplied.
        found: usize,
    },
}

/// Convenient alias for a fallible RAFFLES result.
pub type Result<T> = core::result::Result<T, RafflesError>;
