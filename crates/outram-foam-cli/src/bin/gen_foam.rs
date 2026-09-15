//! `gen-foam` — GeN-Foam deterministic-neutronics + thermal-hydraulics solver.
//!
//! **Honest stub — the case-driven solver does not exist yet**, though rather
//! less of it is missing than this file used to claim.
//!
//! ## What the library actually has
//!
//! [`outram_foam_appbuilder_lib::genfoam`] carries the multigroup **diffusion**
//! (eigenvalue and transient), **SP3**, **SN** and **point-kinetics**
//! neutronics, the cross-section parametrisation layer, the porous-medium
//! thermal-hydraulics, the thermo-mechanics field solve, and the multi-region
//! coupling pieces. The io layer reads `constant/<region>/polyMesh` (including
//! `cellZones`) and `constant/<region>/nuclearData`, so a case's mesh and group
//! constants *can* be loaded — that path is exercised end to end against
//! upstream's own ESFR and MSFR tutorials in
//! `crates/outram-foam-appbuilder-lib/tests/genfoam_tutorial_keff.rs`.
//!
//! An earlier version of this comment said the diffusion/SP3/SN neutronics and
//! the coupling driver were "still planned" and that nothing read
//! `nuclearData`. That was stale on all three counts. It is recorded here
//! because a CLI that understates its own library is the kind of error that
//! sends the next reader off to rebuild what exists.
//!
//! ## What is genuinely missing
//!
//! A **top-level case assembly**: something that reads `system/controlDict`'s
//! `regionSolvers`, `constant/regionProperties` and
//! `constant/multiRegionCouplingDict`, builds the region solvers from them,
//! wires the neutronics ⟂ TH ⟂ TM coupling, and marches the time loop writing
//! OpenFOAM time directories. Each piece it would assemble exists; the assembly
//! does not. Also missing: the field-file boundary-condition types a real case
//! names — notably `albedoSP3`, which upstream's MSFR tutorial uses.
//!
//! This binary is the thin CLI shell to be wired once that assembly lands.
//! See `docs/genfoam-port-plan.md`.

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
        "gen-foam: not yet case-wired. The library has the diffusion/SP3/SN/point-kinetics \
         neutronics, the cross-section layer, the porous-medium thermal-hydraulics and the \
         thermo-mechanics solve, and the io layer reads polyMesh, cellZones and nuclearData \
         -- but there is no top-level assembly that builds the region solvers from \
         controlDict/regionProperties/multiRegionCouplingDict and marches the coupled time \
         loop. No run is performed. See docs/genfoam-port-plan.md for the port status, and \
         tests/genfoam_tutorial_keff.rs in outram-foam-appbuilder-lib for what the neutronics \
         path does reproduce on upstream's own tutorial cases."
            .into(),
    ))
}
