//! `rhoPimpleFoam` — transient compressible PIMPLE solver.
//!
//! **Honest stub — not yet cleanly constructible from a case.** The
//! [`RhoPimpleFoam`] solver constructor itself is satisfiable (mesh + default
//! control/schemes/solution), but a *faithful* run needs the fluid's
//! thermophysical closure — the compressibility `ψ = ρ/p` (equation of state),
//! the specific heat `Cp` for the energy equation, and the transport
//! coefficients `μ` and `αh`. In OpenFOAM these come from
//! `constant/thermophysicalProperties`, which the current case reader
//! ([`outram_foam_basic_lib::io`]) does **not** parse — it reads only
//! `volScalarField`/`volVectorField` files and `system/` dictionaries.
//!
//! Constructing the solver anyway would silently fall back to hardcoded default
//! `ψ`, `Cp`, `μ` unrelated to the case's fluid, i.e. it would run with the
//! wrong equation of state. Rather than fake a physically meaningless run, this
//! binary reports exactly what is missing.
//!
//! ## To make this live
//!
//! Add a `constant/thermophysicalProperties` reader to the io layer (γ or Cp/Cv,
//! molar mass / R, μ, Pr) and wire the parsed `ψ`, `Cp`, `μ`, `αh` into
//! [`RhoPimpleFoam`] before `run()`. Then this binary can follow the same

//! `rhoPimpleFoam` — not case-wired; see
//! [`outram_foam_appbuilder_lib::case_runner::SolverKind::is_case_wired`].
//!
//! The solver itself is implemented. What is missing is a reader for
//! `constant/thermophysicalProperties`, without which its initial state
//! cannot be built from a case directory. The explanation now lives in one
//! place, on the error, rather than being restated here.

use outram_foam_appbuilder_lib::case_runner::{CaseRun, SolverKind};
use outram_foam_cli::{CaseArgs, CliError};

fn main() {
    let args = outram_foam_cli::openfoam_args();
    if let Err(e) = run(&args) {
        eprintln!("rhoPimpleFoam: error: {e}");
        std::process::exit(1);
    }
}

fn run(args: &CaseArgs) -> Result<(), CliError> {
    let case_dir = args.case_dir()?;
    CaseRun::from_case(&case_dir, SolverKind::RhoPimpleFoam)
        .map(|_| ())
        .map_err(|e| CliError::Tool(e.to_string()))
}
