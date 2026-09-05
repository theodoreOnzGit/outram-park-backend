//! `sonicFoam` — trans-sonic / supersonic compressible solver.
//!
//! **Honest stub — not yet cleanly constructible from a case.** Like
//! `rhoPimpleFoam`, the [`SonicFoam`] constructor is satisfiable (mesh + default
//! control/schemes/solution), but the pressure–density coupling it solves is
//! driven by the compressibility `ψ = ρ/p` and closed with `μ` and the internal
//! energy `e`. `ψ` and `μ` come from `constant/thermophysicalProperties`, which
//! the current case reader ([`outram_foam_basic_lib::io`]) does **not** parse.
//!
//! Constructing the solver would silently use a hardcoded default `ψ` unrelated
//! to the case fluid — the wrong equation of state — so this binary reports what
//! is missing instead of faking a run.
//!
//! ## To make this live
//!
//! Add a `constant/thermophysicalProperties` reader (for `ψ`/`R`/`γ`, `μ`) to
//! the io layer and wire the parsed values into [`SonicFoam`] before `run()`,
//! plus require `0/e` (specific internal energy) as an initial field. Then this

//! `sonicFoam` — not case-wired; see
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
        eprintln!("sonicFoam: error: {e}");
        std::process::exit(1);
    }
}

fn run(args: &CaseArgs) -> Result<(), CliError> {
    let case_dir = args.case_dir()?;
    CaseRun::from_case(&case_dir, SolverKind::SonicFoam)
        .map(|_| ())
        .map_err(|e| CliError::Tool(e.to_string()))
}
