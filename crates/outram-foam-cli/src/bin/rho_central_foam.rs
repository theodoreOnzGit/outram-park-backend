//! `rhoCentralFoam` — density-based compressible central-upwind (KNP) solver.
//!
//! **Live, case-wired.** This binary reads an OpenFOAM case directory, builds
//! the [`RhoCentralFoam`] solver from
//! [`outram_foam_appbuilder_lib`], marches it to `endTime`, and writes the
//! result fields into a new time directory.
//!
//! ## What it reads from the case
//!
//! - `constant/polyMesh` → the [`FvMesh`] the solver runs on.
//! - `system/controlDict` → `deltaT`, `startTime`, `endTime` (via the
//!   [`outram_foam_basic_lib`] dictionary reader; the appbuilder's typed
//!   `ControlDict::read` is still a `todo!()`, so those three keys are lifted
//!   directly here and the remaining control settings take their defaults).
//! - `0/U`, `0/rho`, `0/e` → the initial state fields. `rhoCentralFoam` here
//!   evolves the conserved set (ρ, ρU, ρE) with a **built-in calorically-perfect
//!   gas** EOS (`p = (γ−1)ρe`, `γ` fixed in the solver), so no
//!   `thermophysicalProperties` file is needed — but the internal energy `e`
//!   must be supplied directly (there is no thermo reader to derive `e` from a
//!   temperature `T`). `0/p` is optional; it is recovered from ρ and e.
//!
//! ## What it writes
//! `rhoCentralFoam` — density-based central-upwind (Kurganov-Tadmor).
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
        eprintln!("rhoCentralFoam: error: {e}");
        std::process::exit(1);
    }
}

fn run(args: &CaseArgs) -> Result<(), CliError> {
    let case_dir = args.case_dir()?;
    let mut case_run = CaseRun::from_case(&case_dir, SolverKind::RhoCentralFoam)
        .map_err(|e| CliError::Tool(format!("rhoCentralFoam: {e}")))?;

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
            .map_err(|e| CliError::Tool(format!("rhoCentralFoam: solver step failed: {e}")))?;
        println!("Time = {}", format_time(t));
    }
    println!(
        "\nEnd (marched {} step(s) to Time = {})",
        case_run.n_steps,
        format_time(case_run.time)
    );

    let summary = case_run
        .write_fields()
        .map_err(|e| CliError::Tool(format!("rhoCentralFoam: {e}")))?;
    println!("Wrote fields to {}", summary.output_dir.display());
    Ok(())
}
