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
//! - `0/nu` (optional) → kinematic viscosity ν as a `volScalarField`. If absent
//!   the solver's default ν is used, because the case reader does **not** parse
//!   `constant/transportProperties`; a note is printed when the default is used.
//!
//! ## What it writes
//!
//! `U`, `p` into `<endTime>/`. The face flux `phi` is a `SurfaceScalarField`
//! and is not written (the io layer writes only volume fields).
//! `pimpleFoam` — incompressible transient PIMPLE.
//!
//! A thin wrapper over
//! [`outram_foam_appbuilder_lib::case_runner`]: the case reading, solver
//! construction, time loop and field writing all live there, so that the same
//! path is reachable from a library caller (and from Python through the
//! `outram-park` wheel) and not only from this command line.

use outram_foam_appbuilder_lib::case_runner::{format_time, CaseRun, SolverKind};
use outram_foam_cli::{CaseArgs, CliError};

fn main() {
    let args = outram_foam_cli::openfoam_args();
    if let Err(e) = run(&args) {
        eprintln!("pimpleFoam: error: {e}");
        std::process::exit(1);
    }
}

fn run(args: &CaseArgs) -> Result<(), CliError> {
    let case_dir = args.case_dir()?;
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

    let summary = case_run
        .write_fields()
        .map_err(|e| CliError::Tool(format!("pimpleFoam: {e}")))?;
    println!("Wrote fields to {}", summary.output_dir.display());
    Ok(())
}
