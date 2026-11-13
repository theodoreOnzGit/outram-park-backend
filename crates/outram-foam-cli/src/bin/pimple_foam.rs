//! `pimpleFoam` — transient incompressible PISO/PIMPLE solver.
//!
//! **Live, case-wired.** Reads an OpenFOAM case, builds the incompressible
//! [`PimpleFoam`] solver from [`outram_foam_appbuilder_lib`], marches it to
//! `endTime`, and writes `U` and `p` into a new time directory.
//!
//! ## What it reads from the case
//!
//! - `constant/polyMesh` → the [`FvMesh`].
//! - `system/controlDict` → `deltaT`, `startTime`, `endTime` (lifted through the
//!   working [`outram_foam_basic_lib`] dictionary reader; the appbuilder's typed
//!   `ControlDict::read` is a `todo!()`).
//! - `0/U`, `0/p` → the initial velocity and **kinematic** pressure (p/ρ,
//!   m²/s²), as in icoFoam / incompressible pimpleFoam.
//! - `0/nu` (optional) → kinematic viscosity ν as a `volScalarField`; else
//!   `constant/transportProperties` (`transportModel Newtonian`, `nu`); else the
//!   solver default. ~~If absent the solver default was used because the case
//!   reader does not parse `constant/transportProperties`~~ **CORRECTED
//!   2026-11-12 (gh:#1363 row 7)**: `transportProperties` is read
//!   (`case_runner::newtonian_nu`; the shipped cavity ran at the default ν before).
//! - `system/fvSchemes` `div(phi,U)` (`Gauss linear` or `Gauss upwind`) and
//!   `system/fvSolution` `solvers` `U`/`UFinal`, `p`/`pFinal` `tolerance` (only
//!   entries with `relTol 0`; gh:#1291 is otherwise open).
//! - `-flavour esi` writes ESI v2506's bytes (banner, `arch`, `%g` at `writePrecision`).
//!
//! ## What it writes
//!
//! `U`, `p` into `<endTime>/`. ~~The face flux `phi` is not written (the io layer
//! writes only volume fields).~~ **CORRECTED 2026-11-12 (gh:#1363 row 7):** with
//! `-flavour esi` the face flux `phi` and `uniform/time` are written as ESI's
//! `icoFoam` does; the default layout writes neither.
//! `pimpleFoam` — incompressible transient PIMPLE.
//!
//! A thin wrapper over
//! [`outram_foam_appbuilder_lib::case_runner`]: the case reading, solver
//! construction, time loop and field writing all live there, so that the same
//! path is reachable from a library caller (and from Python through the
//! `outram-park` wheel) and not only from this command line.

use outram_foam_appbuilder_lib::case_runner::{format_time, CaseRun, SolverKind};
use outram_foam_cli::{CliError, FlavourArgs, WriteFlavour};

fn main() {
    let args = outram_foam_cli::flavour_args();
    if let Err(e) = run(&args) {
        eprintln!("pimpleFoam: error: {e}");
        std::process::exit(1);
    }
}

fn run(fargs: &FlavourArgs) -> Result<(), CliError> {
    let flavour = WriteFlavour::parse(&fargs.flavour)?;
    let case_dir = fargs.case.case_dir()?;
    let mut case_run = CaseRun::from_case(&case_dir, SolverKind::PimpleFoam)
        .map_err(|e| CliError::Tool(format!("pimpleFoam: {e}")))?;

    println!("Reading case {}", case_dir.display());
    println!("nCells = {}", case_run.solver.n_cells());
    println!(
        "deltaT = {}\nendTime = {}\n",
        format_time(case_run.delta_t),
        format_time(case_run.end_time)
    );
    println!("Starting time loop\n");

    while !case_run.is_done() {
        let t = case_run
            .step()
            .map_err(|e| CliError::Tool(format!("pimpleFoam: solver step failed: {e}")))?;
        println!("Time = {}", format_time(t));
    }
    println!(
        "\nEnd (marched {} step(s) to Time = {})",
        case_run.n_steps,
        format_time(case_run.time)
    );

    let summary = outram_foam_cli::write_run_fields(&case_run, &case_dir, flavour)?;
    println!("Wrote fields to {}", summary.output_dir.display());
    Ok(())
}
