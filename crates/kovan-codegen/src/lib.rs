//! # kovan-codegen
//!
//! A deterministic engineering-code generation framework. It emits
//! implementations of **known** algorithms, engineering patterns, and numerical
//! methods — it does **not** generate speculative or arbitrary software, and it
//! is **not** an AI coding assistant.
//!
//! ## Priority order
//!
//! ```text
//! Correctness > Traceability > Maintainability > Performance > Convenience
//! ```
//!
//! The catalogue enums below enumerate the numerical methods KOVAN intends to
//! generate patterns for. Placeholder stage: [`generate`] is a `// TODO(kovan)`
//! stub; the enums exist so downstream code and tests can already name methods.

#![forbid(unsafe_code)]

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
    Root(RootFinder),
    Linear(LinearSolver),
    Nonlinear(NonlinearSolver),
    Ode(OdeSolver),
}

/// Errors produced by the code generator.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CodegenError {
    /// The requested method's pattern is not implemented yet (placeholder stage).
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
/// TODO(kovan): emit correct, traceable, well-documented implementations from
/// vetted templates (declarative/procedural macros or `build.rs`).
pub fn generate(_method: Method) -> Result<String, CodegenError> {
    Err(CodegenError::Unimplemented("generate"))
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
    fn generate_is_stubbed() {
        assert!(matches!(
            generate(Method::Linear(LinearSolver::Gmres)),
            Err(CodegenError::Unimplemented(_))
        ));
    }
}
