//! `gen-foam` — GeN-Foam deterministic-neutronics + thermal-hydraulics solver.
//!
//! **Honest stub — the case-driven solver does not exist yet.** Unlike the CFD
//! solvers, the GeN-Foam port in
//! [`outram_foam_appbuilder_lib::genfoam`] does **not** yet expose a top-level
//! solver that can be constructed from an OpenFOAM case and marched. Only the
//! 0-D point-kinetics ODE core (`genfoam::neutronics::point_kinetics`) is
//! translated so far; the multigroup diffusion/SP3/SN neutronics, the
//! multi-region (neutronics ⟂ TH ⟂ TM) coupling driver, and the reactor
//! thermal-hydraulics are still planned (`docs/genfoam-port-plan.md`).
//!
//! GeN-Foam additionally reads its own `constant/nuclearData` dictionaries
//! (group cross sections, delayed-neutron data) which the current io layer does
//! not parse.
//!
//! ## To make this live
//!
//! Two prerequisites, both upstream of this binary: (1) a top-level GeN-Foam
//! solver in `genfoam` that assembles the neutronics + TH + coupling from a
//! case, and (2) a `nuclearData` reader in the io layer. This binary is the
//! thin CLI shell to be wired once those land.

use outram_foam_cli::{CaseArgs, CliError};

fn main() {
    let args = outram_foam_cli::openfoam_args();
    if let Err(e) = run(&args) {
        eprintln!("gen-foam: error: {e}");
        std::process::exit(1);
    }
}

fn run(args: &CaseArgs) -> Result<(), CliError> {
    let _case = args.case_dir()?;
    Err(CliError::Tool(
        "gen-foam: not yet case-wired. The GeN-Foam port currently provides only the 0-D \
         point-kinetics core; there is no top-level case-driven solver to construct (no \
         neutronics/TH/coupling assembly), and no constant/nuclearData reader in the io layer. \
         No run is performed. See docs/genfoam-port-plan.md for the port status."
            .into(),
    ))
}
