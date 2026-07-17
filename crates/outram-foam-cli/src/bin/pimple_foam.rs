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

use outram_foam_appbuilder_lib::prelude::{ControlDict, PimpleFoam, StartControl, StopControl};
use outram_foam_basic_lib::io::case::{CaseField, FoamCase};
use outram_foam_basic_lib::io::field::{write_vol_scalar_field, write_vol_vector_field};
use outram_foam_cli::{CaseArgs, CliError};

const U_DIMS: [f64; 7] = [0.0, 1.0, -1.0, 0.0, 0.0, 0.0, 0.0]; // m/s
const P_KIN_DIMS: [f64; 7] = [0.0, 2.0, -2.0, 0.0, 0.0, 0.0, 0.0]; // m^2/s^2 (kinematic p)

fn main() {
    let args = outram_foam_cli::openfoam_args();
    if let Err(e) = run(&args) {
        eprintln!("pimpleFoam: error: {e}");
        std::process::exit(1);
    }
}

fn run(args: &CaseArgs) -> Result<(), CliError> {
    let case_dir = args.case_dir()?;
    let case = FoamCase::read(&case_dir).map_err(|e| CliError::Io(e.to_string()))?;

    let mesh = case
        .mesh
        .clone()
        .ok_or_else(|| CliError::Tool("pimpleFoam: case has no constant/polyMesh".into()))?;

    let control = control_from_case(&case);
    let (start, end, dt) = time_span(&control);

    let u = find_vector(&case, "U")
        .ok_or_else(|| CliError::Tool("pimpleFoam: missing 0/U (velocity)".into()))?;
    let p = find_scalar(&case, "p")
        .ok_or_else(|| CliError::Tool("pimpleFoam: missing 0/p (kinematic pressure)".into()))?;

    let mut solver = PimpleFoam::new(
        mesh,
        control,
        outram_foam_appbuilder_lib::prelude::FvSchemes::default(),
        outram_foam_appbuilder_lib::prelude::FvSolution::default(),
    );
    solver.u = u;
    solver.p = p;
    match find_scalar(&case, "nu") {
        Some(nu) => solver.nu = nu,
        None => println!(
            "note: no 0/nu field; using the solver's default kinematic viscosity \
             (constant/transportProperties is not parsed by the case reader)."
        ),
    }

    println!("Reading case {}", case_dir.display());
    println!("nCells = {}", solver.mesh.n_cells);
    println!("deltaT = {}\nendTime = {}\n", fmt_time(dt), fmt_time(end));
    println!("Starting time loop\n");

    let mut time = start;
    let mut n_steps = 0usize;
    while time < end - dt * 1e-9 {
        solver
            .step()
            .map_err(|e| CliError::Tool(format!("pimpleFoam: solver step failed: {e}")))?;
        time += dt;
        n_steps += 1;
        println!("Time = {}", fmt_time(time));
    }
    println!("\nEnd (marched {n_steps} step(s) to Time = {})", fmt_time(time));

    let out_dir = case_dir.join(fmt_time(time));
    std::fs::create_dir_all(&out_dir).map_err(|e| CliError::Io(e.to_string()))?;
    write_vol_scalar_field(out_dir.join("p"), &solver.p, P_KIN_DIMS)
        .map_err(|e| CliError::Io(e.to_string()))?;
    write_vol_vector_field(out_dir.join("U"), &solver.u, U_DIMS)
        .map_err(|e| CliError::Io(e.to_string()))?;
    println!("Wrote fields to {}", out_dir.display());

    Ok(())
}

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

fn fmt_time(t: f64) -> String {
    if t.is_finite() && t.fract().abs() < 1e-12 && t.abs() < 1e15 {
        return format!("{}", t.round() as i64);
    }
    let s = format!("{t:.9}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    s.to_string()
}
