//! # kovan-codegen
//!
//! A **deterministic** engineering-code generation framework. It emits
//! implementations of **known** algorithms, engineering patterns, and numerical
//! methods — it does **not** generate speculative or arbitrary software, and it
//! is **not** an AI coding assistant. See docs/kovan.md § "KOVAN Codegen".
//!
//! ## Priority order
//!
//! ```text
//! Correctness > Traceability > Maintainability > Performance > Convenience
//! ```
//!
//! ## How generation works
//!
//! Every generated method's source lives in a small, human-readable template
//! file under `src/<family>/templates/`. Each template file is used **two
//! ways**:
//!
//! * returned verbatim by [`generate`] (via `include_str!`), and
//! * compiled into that family's `reference` module (via `include!`),
//!
//! so the crate's own tests execute the *exact bytes* [`generate`] emits. This
//! is the verification link: the numerical-correctness tests (e.g. "the
//! generated Newton finder recovers sqrt(2)", "the generated RK4 stepper
//! integrates `dy/dt = y` to `e`") validate the emitted source itself, not a
//! separate copy. Generation is deterministic — the same [`Method`] always
//! yields byte-identical output — because the templates are compile-time
//! constants (no formatting, no randomness, no I/O).
//!
//! ## Why string templates, not proc-macros
//!
//! KOVAN is offline- and Android-first. String templating with `include_str!`
//! is pure `core`/`std`, cross-compiles to `aarch64-linux-android` with no
//! native dependency, and keeps every generated artifact inspectable and
//! diffable. Procedural macros (`syn`/`quote`) are *scaffolded* in
//! [`macros_support`] but not used for the numerical-method path — see
//! `DECISIONS.md`.
//!
//! ## Modules
//!
//! * [`root_finding`], [`linear`], [`nonlinear`], [`ode`], [`pde`] — the
//!   numerical-method families, each with a `generate` fn and a `reference`
//!   module.
//! * [`patterns`] — the Engineering Pattern Library (convergence/relaxation
//!   helpers).
//! * [`macros_support`] — the macro-support framework (declarative macro +
//!   proc-macro/`build.rs` scaffolds).

#![forbid(unsafe_code)]

pub mod linear;
pub mod macros_support;
pub mod nonlinear;
pub mod ode;
pub mod patterns;
pub mod pde;
pub mod root_finding;

pub use kovan_common::GeneratedArtifact;
pub use pde::PdeScheme;

/// Root-finding methods KOVAN can generate patterns for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RootFinder {
    Bisection,
    RegulaFalsi,
    Illinois,
    Pegasus,
    Secant,
    NewtonRaphson,
    Brent,
}

/// Linear-solver methods KOVAN can generate patterns for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinearSolver {
    Jacobi,
    GaussSeidel,
    Sor,
    ConjugateGradient,
    BiCgStab,
    Gmres,
    Lu,
    Qr,
    Cholesky,
}

/// Nonlinear-solver methods KOVAN can generate patterns for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NonlinearSolver {
    Newton,
    QuasiNewton,
    Broyden,
    TrustRegion,
}

/// ODE-solver methods KOVAN can generate patterns for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OdeSolver {
    Euler,
    Rk2,
    Rk4,
    DormandPrince,
    BackwardEuler,
    CrankNicolson,
}

/// A numerical method to generate a pattern for. Dispatch is by enum (closed,
/// compile-time-known set) rather than trait objects.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Method {
    /// A scalar root finder.
    Root(RootFinder),
    /// A linear-system solver.
    Linear(LinearSolver),
    /// A nonlinear (Newton-family) solver.
    Nonlinear(NonlinearSolver),
    /// An ODE initial-value-problem integrator.
    Ode(OdeSolver),
    /// A PDE spatial-discretisation scheme.
    Pde(PdeScheme),
}

/// Errors produced by the code generator.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CodegenError {
    /// The requested method's pattern is catalogued but not yet generated. The
    /// payload names the specific method (e.g. `"root finder: Illinois"`).
    Unimplemented(&'static str),
}

impl std::fmt::Display for CodegenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CodegenError::Unimplemented(what) => write!(f, "not implemented yet: {what}"),
        }
    }
}

impl std::error::Error for CodegenError {}

/// Generate Rust source implementing the given [`Method`].
///
/// Returns compilable, `///`-documented Rust source for the requested method,
/// using plain `f64` numerical kernels (these are generic math templates, so
/// they are intentionally `uom`-agnostic). The output contains no `unsafe`, no
/// trait-object dispatch, and no external-crate dependencies, so it drops into
/// any crate — including Android targets.
///
/// Generation is deterministic: calling this twice with the same [`Method`]
/// yields byte-identical strings.
///
/// # Errors
/// [`CodegenError::Unimplemented`] for catalogue entries that are named but not
/// yet backed by a template (e.g. GMRES, Dormand–Prince, Broyden). The payload
/// identifies the specific method.
///
/// # Examples
/// ```
/// use kovan_codegen::{generate, Method, OdeSolver};
/// let src = generate(Method::Ode(OdeSolver::Rk4)).unwrap();
/// assert!(src.contains("pub fn rk4_step"));
/// ```
pub fn generate(method: Method) -> Result<String, CodegenError> {
    match method {
        Method::Root(m) => root_finding::generate(m),
        Method::Linear(m) => linear::generate(m),
        Method::Nonlinear(m) => nonlinear::generate(m),
        Method::Ode(m) => ode::generate(m),
        Method::Pde(m) => pde::generate(m),
    }
}

/// Generate a [`Method`] and package it as a [`GeneratedArtifact`] provenance
/// record — the "Correlation → Implementation" link of the KOVAN integration
/// vision (`docs/kovan.md`, "Integration Vision").
///
/// The returned artifact carries the emitted source plus the (optional)
/// [`GeneratedArtifact::correlation_id`] and [`GeneratedArtifact::source_document_id`]
/// so a generated kernel can be traced back to the correlation and paper it
/// implements. Pass `None` for either when generating a bare method with no
/// literature link. `method` is recorded as its `Debug` label (e.g.
/// `"Ode(Rk4)"`), which is stable and deterministic.
///
/// # Errors
/// Propagates [`CodegenError::Unimplemented`] from [`generate`] for catalogue
/// entries that are named but not yet backed by a template.
///
/// # Examples
/// ```
/// use kovan_codegen::{generate_artifact, Method, OdeSolver};
/// let art = generate_artifact(
///     Method::Ode(OdeSolver::Rk4),
///     Some("corr-rk4".to_string()),
///     None,
/// )
/// .unwrap();
/// assert_eq!(art.method, "Ode(Rk4)");
/// assert!(art.source.contains("pub fn rk4_step"));
/// assert_eq!(art.correlation_id.as_deref(), Some("corr-rk4"));
/// ```
pub fn generate_artifact(
    method: Method,
    correlation_id: Option<String>,
    source_document_id: Option<String>,
) -> Result<GeneratedArtifact, CodegenError> {
    let source = generate(method)?;
    Ok(GeneratedArtifact {
        method: format!("{method:?}"),
        source,
        correlation_id,
        source_document_id,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn methods_compose_from_families() {
        let m = Method::Ode(OdeSolver::Rk4);
        assert_eq!(m, Method::Ode(OdeSolver::Rk4));
        assert_ne!(m, Method::Root(RootFinder::Brent));
    }

    #[test]
    fn generate_dispatches_to_each_family() {
        assert!(generate(Method::Root(RootFinder::NewtonRaphson))
            .unwrap()
            .contains("pub fn newton_raphson"));
        assert!(generate(Method::Linear(LinearSolver::Lu))
            .unwrap()
            .contains("pub fn lu_solve"));
        assert!(generate(Method::Nonlinear(NonlinearSolver::Newton))
            .unwrap()
            .contains("pub fn newton_system"));
        assert!(generate(Method::Ode(OdeSolver::Rk4))
            .unwrap()
            .contains("pub fn rk4_step"));
        assert!(generate(Method::Pde(PdeScheme::Poisson1dFiniteDifference))
            .unwrap()
            .contains("pub fn solve_poisson_1d"));
    }

    #[test]
    fn unimplemented_catalogue_entries_report_the_method() {
        assert_eq!(
            generate(Method::Linear(LinearSolver::Gmres)),
            Err(CodegenError::Unimplemented("linear solver: GMRES"))
        );
        assert!(matches!(
            generate(Method::Ode(OdeSolver::DormandPrince)),
            Err(CodegenError::Unimplemented(_))
        ));
    }

    #[test]
    fn generate_is_deterministic_across_families() {
        for m in [
            Method::Root(RootFinder::Brent),
            Method::Linear(LinearSolver::Lu),
            Method::Ode(OdeSolver::BackwardEuler),
            Method::Pde(PdeScheme::Poisson1dFiniteDifference),
        ] {
            assert_eq!(generate(m).unwrap(), generate(m).unwrap());
        }
    }

    #[test]
    fn generate_artifact_packages_source_and_provenance() {
        let art = generate_artifact(
            Method::Ode(OdeSolver::Rk4),
            Some("corr-1".to_string()),
            Some("doc-1".to_string()),
        )
        .expect("rk4 generates");
        assert_eq!(art.method, "Ode(Rk4)");
        assert!(art.source.contains("pub fn rk4_step"));
        // The packaged source is exactly what `generate` emits.
        assert_eq!(art.source, generate(Method::Ode(OdeSolver::Rk4)).unwrap());
        assert_eq!(art.correlation_id.as_deref(), Some("corr-1"));
        assert_eq!(art.source_document_id.as_deref(), Some("doc-1"));
    }

    #[test]
    fn generate_artifact_propagates_unimplemented() {
        assert!(matches!(
            generate_artifact(Method::Linear(LinearSolver::Gmres), None, None),
            Err(CodegenError::Unimplemented(_))
        ));
    }

    #[test]
    fn emitted_source_is_the_compiled_reference_verbatim() {
        // The verification contract: what `generate` emits is byte-for-byte the
        // source compiled into the `reference` modules that the correctness
        // tests exercise. If a template were edited without recompiling, this
        // (and the family tests) would catch the divergence.
        assert_eq!(
            generate(Method::Root(RootFinder::NewtonRaphson)).unwrap(),
            include_str!("root_finding/templates/newton_raphson.rs")
        );
        assert_eq!(
            generate(Method::Ode(OdeSolver::Rk4)).unwrap(),
            include_str!("ode/templates/rk4.rs")
        );
    }
}
