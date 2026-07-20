//! `kovan gen` — deterministic numerical-method code generation
//! (`kovan-codegen`). One nested subcommand per method family, mirroring
//! `kovan methods`'s catalogue grouping.
//!
//! `clap::ValueEnum` is a foreign trait, so it cannot be derived directly on
//! `kovan-codegen`'s own catalogue enums from this crate; each family gets a
//! local `clap`-facing mirror plus a `From` conversion (same pattern as
//! [`super::KindArg`]/[`super::LangArg`]), matched exhaustively both ways.

use std::path::PathBuf;

use clap::{Subcommand, ValueEnum};

use kovan_codegen::{
    generate, LinearSolver, Method, NonlinearSolver, OdeSolver, PdeScheme, RootFinder,
};

/// `kovan gen <family> <method> [--out <path>]`.
#[derive(Subcommand)]
pub enum GenCommand {
    /// Generate a scalar root finder.
    Root {
        #[arg(value_enum)]
        method: RootFinderArg,
        /// Write the generated Rust source here instead of stdout.
        #[arg(long)]
        out: Option<PathBuf>,
    },
    /// Generate a linear-system solver.
    Linear {
        #[arg(value_enum)]
        method: LinearSolverArg,
        #[arg(long)]
        out: Option<PathBuf>,
    },
    /// Generate a nonlinear (Newton-family) solver.
    Nonlinear {
        #[arg(value_enum)]
        method: NonlinearSolverArg,
        #[arg(long)]
        out: Option<PathBuf>,
    },
    /// Generate an ODE initial-value-problem integrator.
    Ode {
        #[arg(value_enum)]
        method: OdeSolverArg,
        #[arg(long)]
        out: Option<PathBuf>,
    },
    /// Generate a PDE spatial-discretisation scheme.
    Pde {
        #[arg(value_enum)]
        method: PdeSchemeArg,
        #[arg(long)]
        out: Option<PathBuf>,
    },
}

/// Dispatch a parsed [`GenCommand`]: resolve it to a [`Method`], generate,
/// then either print the source or write it to `--out`.
pub fn run(command: GenCommand) -> Result<(), String> {
    let (method, out) = match command {
        GenCommand::Root { method, out } => (Method::Root(method.into()), out),
        GenCommand::Linear { method, out } => (Method::Linear(method.into()), out),
        GenCommand::Nonlinear { method, out } => (Method::Nonlinear(method.into()), out),
        GenCommand::Ode { method, out } => (Method::Ode(method.into()), out),
        GenCommand::Pde { method, out } => (Method::Pde(method.into()), out),
    };

    let src = generate(method).map_err(|e| e.to_string())?;
    match out {
        Some(path) => {
            std::fs::write(&path, &src).map_err(|e| format!("write {}: {e}", path.display()))?;
            println!("wrote {}", path.display());
        }
        None => print!("{src}"),
    }
    Ok(())
}

#[derive(Clone, Copy, ValueEnum)]
pub enum RootFinderArg {
    Bisection,
    RegulaFalsi,
    Illinois,
    Pegasus,
    Secant,
    NewtonRaphson,
    Brent,
}

impl From<RootFinderArg> for RootFinder {
    fn from(m: RootFinderArg) -> Self {
        match m {
            RootFinderArg::Bisection => RootFinder::Bisection,
            RootFinderArg::RegulaFalsi => RootFinder::RegulaFalsi,
            RootFinderArg::Illinois => RootFinder::Illinois,
            RootFinderArg::Pegasus => RootFinder::Pegasus,
            RootFinderArg::Secant => RootFinder::Secant,
            RootFinderArg::NewtonRaphson => RootFinder::NewtonRaphson,
            RootFinderArg::Brent => RootFinder::Brent,
        }
    }
}

#[derive(Clone, Copy, ValueEnum)]
pub enum LinearSolverArg {
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

impl From<LinearSolverArg> for LinearSolver {
    fn from(m: LinearSolverArg) -> Self {
        match m {
            LinearSolverArg::Jacobi => LinearSolver::Jacobi,
            LinearSolverArg::GaussSeidel => LinearSolver::GaussSeidel,
            LinearSolverArg::Sor => LinearSolver::Sor,
            LinearSolverArg::ConjugateGradient => LinearSolver::ConjugateGradient,
            LinearSolverArg::BiCgStab => LinearSolver::BiCgStab,
            LinearSolverArg::Gmres => LinearSolver::Gmres,
            LinearSolverArg::Lu => LinearSolver::Lu,
            LinearSolverArg::Qr => LinearSolver::Qr,
            LinearSolverArg::Cholesky => LinearSolver::Cholesky,
        }
    }
}

#[derive(Clone, Copy, ValueEnum)]
pub enum NonlinearSolverArg {
    Newton,
    QuasiNewton,
    Broyden,
    TrustRegion,
}

impl From<NonlinearSolverArg> for NonlinearSolver {
    fn from(m: NonlinearSolverArg) -> Self {
        match m {
            NonlinearSolverArg::Newton => NonlinearSolver::Newton,
            NonlinearSolverArg::QuasiNewton => NonlinearSolver::QuasiNewton,
            NonlinearSolverArg::Broyden => NonlinearSolver::Broyden,
            NonlinearSolverArg::TrustRegion => NonlinearSolver::TrustRegion,
        }
    }
}

#[derive(Clone, Copy, ValueEnum)]
pub enum OdeSolverArg {
    Euler,
    Rk2,
    Rk4,
    DormandPrince,
    BackwardEuler,
    CrankNicolson,
}

impl From<OdeSolverArg> for OdeSolver {
    fn from(m: OdeSolverArg) -> Self {
        match m {
            OdeSolverArg::Euler => OdeSolver::Euler,
            OdeSolverArg::Rk2 => OdeSolver::Rk2,
            OdeSolverArg::Rk4 => OdeSolver::Rk4,
            OdeSolverArg::DormandPrince => OdeSolver::DormandPrince,
            OdeSolverArg::BackwardEuler => OdeSolver::BackwardEuler,
            OdeSolverArg::CrankNicolson => OdeSolver::CrankNicolson,
        }
    }
}

#[derive(Clone, Copy, ValueEnum)]
pub enum PdeSchemeArg {
    Poisson1dFiniteDifference,
    Diffusion1dFiniteVolume,
    BoundaryConditionScaffold,
}

impl From<PdeSchemeArg> for PdeScheme {
    fn from(m: PdeSchemeArg) -> Self {
        match m {
            PdeSchemeArg::Poisson1dFiniteDifference => PdeScheme::Poisson1dFiniteDifference,
            PdeSchemeArg::Diffusion1dFiniteVolume => PdeScheme::Diffusion1dFiniteVolume,
            PdeSchemeArg::BoundaryConditionScaffold => PdeScheme::BoundaryConditionScaffold,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_root_finder_arg_maps_and_generates_or_reports_unimplemented() {
        for arg in [
            RootFinderArg::Bisection,
            RootFinderArg::RegulaFalsi,
            RootFinderArg::Illinois,
            RootFinderArg::Pegasus,
            RootFinderArg::Secant,
            RootFinderArg::NewtonRaphson,
            RootFinderArg::Brent,
        ] {
            let _ = generate(Method::Root(arg.into()));
        }
    }

    #[test]
    fn newton_raphson_generates_expected_source() {
        let src = generate(Method::Root(RootFinderArg::NewtonRaphson.into())).unwrap();
        assert!(src.contains("pub fn newton_raphson"));
    }

    #[test]
    fn gmres_is_unimplemented() {
        assert!(generate(Method::Linear(LinearSolverArg::Gmres.into())).is_err());
    }
}
