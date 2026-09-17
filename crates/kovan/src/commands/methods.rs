//! `kovan-cli methods` — list the `kovan-codegen` numerical-method catalogue and
//! report, per entry, whether it is backed by a generated template yet
//! (`ready`) or only catalogued (`not-implemented`).

use kovan_codegen::{generate, LinearSolver, Method, NonlinearSolver, OdeSolver, PdeScheme, RootFinder};

/// Print the full catalogue, one method per line, grouped by family.
pub fn run() {
    println!("root-finders:");
    for m in [
        RootFinder::Bisection,
        RootFinder::RegulaFalsi,
        RootFinder::Illinois,
        RootFinder::Pegasus,
        RootFinder::Secant,
        RootFinder::NewtonRaphson,
        RootFinder::Brent,
    ] {
        println!("  {:?} (codegen: {})", m, status(Method::Root(m)));
    }

    println!("linear-solvers:");
    for m in [
        LinearSolver::Jacobi,
        LinearSolver::GaussSeidel,
        LinearSolver::Sor,
        LinearSolver::ConjugateGradient,
        LinearSolver::BiCgStab,
        LinearSolver::Gmres,
        LinearSolver::Lu,
        LinearSolver::Qr,
        LinearSolver::Cholesky,
    ] {
        println!("  {:?} (codegen: {})", m, status(Method::Linear(m)));
    }

    println!("nonlinear-solvers:");
    for m in [
        NonlinearSolver::Newton,
        NonlinearSolver::QuasiNewton,
        NonlinearSolver::Broyden,
        NonlinearSolver::TrustRegion,
    ] {
        println!("  {:?} (codegen: {})", m, status(Method::Nonlinear(m)));
    }

    println!("ode-solvers:");
    for m in [
        OdeSolver::Euler,
        OdeSolver::Rk2,
        OdeSolver::Rk4,
        OdeSolver::DormandPrince,
        OdeSolver::BackwardEuler,
        OdeSolver::CrankNicolson,
    ] {
        println!("  {:?} (codegen: {})", m, status(Method::Ode(m)));
    }

    println!("pde-schemes:");
    for m in [
        PdeScheme::Poisson1dFiniteDifference,
        PdeScheme::Diffusion1dFiniteVolume,
        PdeScheme::BoundaryConditionScaffold,
    ] {
        println!("  {:?} (codegen: {})", m, status(Method::Pde(m)));
    }
}

/// `"ready"` if [`generate`] actually emits source for `method`, else
/// `"not-implemented"` (catalogued but not yet templated).
fn status(method: Method) -> &'static str {
    match generate(method) {
        Ok(_) => "ready",
        Err(_) => "not-implemented",
    }
}
