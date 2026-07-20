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
//! binary can follow `rhoCentralFoam`'s read→construct→march→write path.

use outram_foam_cli::{CaseArgs, CliError};

fn main() {
    let args = outram_foam_cli::openfoam_args();
    if let Err(e) = run(&args) {
        eprintln!("sonicFoam: error: {e}");
        std::process::exit(1);
    }
}

fn run(args: &CaseArgs) -> Result<(), CliError> {
    let _case = args.case_dir()?;
    Err(CliError::Tool(
        "sonicFoam: not yet case-wired. The solver needs the compressibility psi = rho/p (and mu) \
         from constant/thermophysicalProperties, which the case reader does not parse yet (it \
         reads only vol fields + system/ dicts). Running with a default psi would impose the wrong \
         equation of state, so no run is performed. See rhoCentralFoam for the case-wiring pattern \
         once a thermo reader exists."
            .into(),
    ))
}
