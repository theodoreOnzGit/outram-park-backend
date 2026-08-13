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
//!
//! `U`, `p`, `rho`, `e` into `<endTime>/` inside the case. The face mass flux
//! `phi` is a `SurfaceScalarField` and is not written (the io layer writes only
//! `volScalarField`/`volVectorField`).

use std::path::Path;

use outram_foam_appbuilder_lib::prelude::{ControlDict, RhoCentralFoam, StartControl, StopControl};
use outram_foam_basic_lib::io::field::{write_vol_scalar_field, write_vol_vector_field};
use outram_foam_basic_lib::io::case::{CaseField, FoamCase};
use outram_foam_cli::{CaseArgs, CliError};

// OpenFOAM SI dimension exponents [kg m s K mol A cd].
const U_DIMS: [f64; 7] = [0.0, 1.0, -1.0, 0.0, 0.0, 0.0, 0.0]; // m/s
const P_DIMS: [f64; 7] = [1.0, -1.0, -2.0, 0.0, 0.0, 0.0, 0.0]; // Pa
const RHO_DIMS: [f64; 7] = [1.0, -3.0, 0.0, 0.0, 0.0, 0.0, 0.0]; // kg/m^3
const E_DIMS: [f64; 7] = [0.0, 2.0, -2.0, 0.0, 0.0, 0.0, 0.0]; // J/kg = m^2/s^2

fn main() {
    let args = outram_foam_cli::openfoam_args();
    if let Err(e) = run(&args) {
        eprintln!("rhoCentralFoam: error: {e}");
        std::process::exit(1);
    }
}

fn run(args: &CaseArgs) -> Result<(), CliError> {
    let case_dir = args.case_dir()?;
    let case = FoamCase::read(&case_dir).map_err(|e| CliError::Io(e.to_string()))?;

    let mesh = case
        .mesh
        .clone()
        .ok_or_else(|| CliError::Tool("rhoCentralFoam: case has no constant/polyMesh".into()))?;

    let control = control_from_case(&case);
    let (start, end, dt) = time_span(&control);

    // Initial state fields (required: U, rho, e — the conserved-variable set).
    let u = find_vector(&case, "U").ok_or_else(|| {
        CliError::Tool("rhoCentralFoam: missing 0/U (velocity) in the time directory".into())
    })?;
    let rho = find_scalar(&case, "rho").ok_or_else(|| {
        CliError::Tool("rhoCentralFoam: missing 0/rho (density) in the time directory".into())
    })?;
    let e = find_scalar(&case, "e").ok_or_else(|| {
        CliError::Tool(
            "rhoCentralFoam: missing 0/e (specific internal energy). This port evolves (rho, e); \
             there is no thermophysicalProperties reader to derive e from a temperature T."
                .into(),
        )
    })?;

    let mut solver = RhoCentralFoam::new(
        mesh,
        control,
        outram_foam_appbuilder_lib::prelude::FvSchemes::default(),
        outram_foam_appbuilder_lib::prelude::FvSolution::default(),
    );
    solver.u = u;
    solver.rho = rho;
    solver.e = e;
    if let Some(p) = find_scalar(&case, "p") {
        solver.p = p;
    }

    // ── Time loop (OpenFOAM-style progress) ─────────────────────────────────
    println!("Reading case {}", case_dir.display());
    println!("nCells = {}", solver.mesh.n_cells);
    println!("deltaT = {}\nendTime = {}\n", fmt_time(dt), fmt_time(end));
    println!("Starting time loop\n");

    let mut time = start;
    let mut n_steps = 0usize;
    while time < end - dt * 1e-9 {
        solver
            .step()
            .map_err(|e| CliError::Tool(format!("rhoCentralFoam: solver step failed: {e}")))?;
        time += dt;
        n_steps += 1;
        println!("Time = {}", fmt_time(time));
    }
    println!(
        "\nEnd (marched {n_steps} step(s) to Time = {})",
        fmt_time(time)
    );

    // ── Write result fields into <endTime>/ ─────────────────────────────────
    let out_dir = case_dir.join(fmt_time(time));
    std::fs::create_dir_all(&out_dir).map_err(|e| CliError::Io(e.to_string()))?;
    write_scalar(&out_dir, "p", &solver.p, P_DIMS)?;
    write_scalar(&out_dir, "rho", &solver.rho, RHO_DIMS)?;
    write_scalar(&out_dir, "e", &solver.e, E_DIMS)?;
    write_vector(&out_dir, "U", &solver.u, U_DIMS)?;
    println!("Wrote fields to {}", out_dir.display());

    Ok(())
}

// ── case → solver helpers (inline; bins cannot share a private module) ───────

/// Build a [`ControlDict`] from the case's `system/controlDict`, lifting the
/// three keys the time loop needs (`deltaT`, `startTime`, `endTime`) and leaving
/// every other control setting at its default. The appbuilder's typed
/// `ControlDict::read` is not implemented yet, so this reads them through the
/// working `outram_foam_basic_lib` dictionary parser.
fn control_from_case(case: &FoamCase) -> ControlDict {
    let mut ctrl = ControlDict::default();
    if let Some(cd) = case.system_dict("controlDict") {
        let d = &cd.dict;
        if let Some(dt) = d.get_scalar("deltaT") {
            ctrl.delta_t = dt;
        }
        if let Some(t0) = d.get_scalar("startTime") {
            ctrl.start = StartControl::StartTime(t0);
        }
        if let Some(te) = d.get_scalar("endTime") {
            ctrl.stop = StopControl::EndTime(te);
        }
        if let Some(wi) = d.get_scalar("writeInterval") {
            ctrl.write_interval = wi;
        }
    }
    ctrl
}

fn time_span(ctrl: &ControlDict) -> (f64, f64, f64) {
    let start = match ctrl.start {
        StartControl::StartTime(t) => t,
        _ => 0.0,
    };
    let end = match ctrl.stop {
        StopControl::EndTime(t) => t,
        _ => start,
    };
    (start, end, ctrl.delta_t)
}

fn find_scalar(
    case: &FoamCase,
    name: &str,
) -> Option<outram_foam_basic_lib::fields::VolScalarField> {
    case.fields.iter().find_map(|f| match f {
        CaseField::Scalar(sf, _) if sf.name == name => Some(sf.clone()),
        _ => None,
    })
}

fn find_vector(
    case: &FoamCase,
    name: &str,
) -> Option<outram_foam_basic_lib::fields::VolVectorField> {
    case.fields.iter().find_map(|f| match f {
        CaseField::Vector(vf, _) if vf.name == name => Some(vf.clone()),
        _ => None,
    })
}

fn write_scalar(
    dir: &Path,
    name: &str,
    field: &outram_foam_basic_lib::fields::VolScalarField,
    dims: [f64; 7],
) -> Result<(), CliError> {
    write_vol_scalar_field(dir.join(name), field, dims).map_err(|e| CliError::Io(e.to_string()))
}

fn write_vector(
    dir: &Path,
    name: &str,
    field: &outram_foam_basic_lib::fields::VolVectorField,
    dims: [f64; 7],
) -> Result<(), CliError> {
    write_vol_vector_field(dir.join(name), field, dims).map_err(|e| CliError::Io(e.to_string()))
}

/// Format a time value the OpenFOAM way: an integer when whole (`1`), otherwise
/// the shortest fixed-point form with trailing zeros trimmed (`0.0005`).
fn fmt_time(t: f64) -> String {
    if t.is_finite() && t.fract().abs() < 1e-12 && t.abs() < 1e15 {
        return format!("{}", t.round() as i64);
    }
    let s = format!("{t:.9}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    s.to_string()
}
