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
//! read→construct→march→write path as `rhoCentralFoam`.

use outram_foam_cli::{CaseArgs, CliError};

fn main() {
    let args = outram_foam_cli::openfoam_args();
    if let Err(e) = run(&args) {
        eprintln!("rhoPimpleFoam: error: {e}");
        std::process::exit(1);
    }
}

fn run(args: &CaseArgs) -> Result<(), CliError> {
    let _case = args.case_dir()?;
    Err(CliError::Tool(
        "rhoPimpleFoam: not yet case-wired. The solver needs a thermophysical closure \
         (compressibility psi = rho/p, Cp, mu, alphaEff) from constant/thermophysicalProperties, \
         which the case reader does not parse yet (it reads only vol fields + system/ dicts). \
         Running with default psi/Cp/mu would impose the wrong equation of state, so no run is \
         performed. See rhoCentralFoam for the case-wiring pattern once a thermo reader exists."
            .into(),
    ))
}
