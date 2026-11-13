//! `PimpleFoam` against upstream OpenFOAM-dev `icoFoam` on the lid-driven
//! cavity, per cell (gh:#1363, row 1).
//!
//! # Method
//! `verification_and_validation/upstream_openfoam_gh1363/` holds the case
//! (dev tutorial `legacy/incompressible/icoFoam/cavity/cavity`, 20 x 20 x 1,
//! nu = 0.01, dt = 0.005, Euler, `Gauss linear` convection, orthogonal
//! Laplacian, PISO 2 correctors, linear solvers tightened to 1e-14), the
//! upstream `blockMesh` polyMesh the port reads unchanged, and the committed
//! upstream outputs: U and p per cell at steps 1, 2, 5, 10, 25, 50 and 100
//! (17 significant digits, which round-trips) and the initial residuals of all
//! 100 steps. `scripts/run_cavity.sh` regenerates them against the real
//! upstream binary. The port runs the same case with the same settings and the
//! test counts bit-identical cells and the worst difference per quantity.
//!
//! # Results
//! See `verification_and_validation/pimple_foam_vs_openfoam.md` (the numbers
//! pinned below are the ones recorded there).

use miniz_oxide::inflate::decompress_to_vec;
use outram_foam_appbuilder_lib::io::control_dict::{ControlDict, StartControl, StopControl};
use outram_foam_appbuilder_lib::io::field_reader::{
    read_vol_scalar_field_full, read_vol_vector_field_full,
};
use outram_foam_appbuilder_lib::io::fv_schemes::{DivScheme, FvSchemes, LaplacianScheme};
use outram_foam_appbuilder_lib::io::fv_solution::FvSolution;
use outram_foam_appbuilder_lib::io::poly_mesh::read_poly_mesh;
use outram_foam_appbuilder_lib::solvers::pimple_foam::{PimpleFoam, SolveRecord};
use outram_foam_basic_lib::prelude::{SolverSettings, VolScalarField};
use std::collections::BTreeMap;
use std::path::PathBuf;

const SNAPSHOT_STEPS: [usize; 7] = [1, 2, 5, 10, 25, 50, 100];

fn case_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("verification_and_validation/upstream_openfoam_gh1363/cavity")
}

fn gunzip(path: &PathBuf) -> String {
    let bytes = std::fs::read(path).expect("read reference");
    // gzip written with `-n`: 10-byte header, 8-byte trailer.
    let raw = decompress_to_vec(&bytes[10..bytes.len() - 8]).expect("inflate");
    String::from_utf8(raw).expect("utf8")
}

/// Upstream U (x, y, z) and p per cell at one snapshot.
struct Snapshot {
    u: Vec<[f64; 3]>,
    p: Vec<f64>,
    phi: Vec<f64>,
}

fn read_reference() -> BTreeMap<usize, Snapshot> {
    let text = gunzip(&case_dir().join("reference/cavity_dev_icoFoam.txt.gz"));
    let mut out = BTreeMap::new();
    let mut cur: Option<Snapshot> = None;
    let mut step = 0usize;
    for line in text.lines() {
        let mut it = line.split_whitespace();
        match it.next() {
            Some("#time") => {
                if let Some(s) = cur.take() {
                    out.insert(step, s);
                }
                let t: f64 = it.next().unwrap().parse().unwrap();
                step = (t / 0.005).round() as usize;
                cur = Some(Snapshot { u: Vec::new(), p: Vec::new(), phi: Vec::new() });
            }
            Some("U") => {
                let v: Vec<f64> = it.map(|s| s.parse().unwrap()).collect();
                cur.as_mut().unwrap().u.push([v[0], v[1], v[2]]);
            }
            Some("p") => cur.as_mut().unwrap().p.push(it.next().unwrap().parse().unwrap()),
            Some("phi") => cur.as_mut().unwrap().phi.push(it.next().unwrap().parse().unwrap()),
            _ => {}
        }
    }
    if let Some(s) = cur.take() {
        out.insert(step, s);
    }
    out
}

/// Upstream initial residuals per step: (Ux, Uy, p corrector 1, p corrector 2).
fn read_residuals() -> Vec<[f64; 4]> {
    let text = std::fs::read_to_string(case_dir().join("reference/residuals.txt")).unwrap();
    text.lines()
        .map(|l| {
            let v: Vec<f64> = l.split_whitespace().skip(1).map(|s| s.parse().unwrap()).collect();
            [v[0], v[1], v[2], v[3]]
        })
        .collect()
}

/// Tight linear-solver settings, the same 1e-14 as the upstream fvSolution.
const TIGHT: SolverSettings = SolverSettings { tolerance: 1e-14, max_iter: 20_000 };

/// Build the cavity solver as the upstream case defines it.
fn build_solver() -> PimpleFoam {
    let case = case_dir();
    let mesh = read_poly_mesh(&case.join("constant/polyMesh")).expect("polyMesh");
    let control = ControlDict {
        start: StartControl::StartTime(0.0),
        stop: StopControl::EndTime(0.5),
        delta_t: 0.005,
        ..ControlDict::default()
    };
    let mut schemes = FvSchemes::default();
    schemes.default_div = DivScheme::GaussLinear;
    schemes.default_laplacian = LaplacianScheme::GaussLinearOrthogonal;
    let mut solution = FvSolution::default();
    solution.pimple.n_outer_correctors = 1;
    solution.pimple.n_correctors = 2;
    let mut s = PimpleFoam::new(mesh.clone(), control, schemes, solution).with_solve_log();
    s.u = read_vol_vector_field_full(&case.join("0/U"), &mesh).expect("0/U");
    s.p = read_vol_scalar_field_full(&case.join("0/p"), &mesh).expect("0/p");
    s.nu = VolScalarField::uniform("nu", mesh, 0.01);
    s.u_linear_solver = TIGHT;
    s.p_linear_solver = TIGHT;
    s
}

/// Comparison of one field: bit-identical count, worst |difference| over the
/// upstream field's max |value|, and worst per-cell relative difference over
/// cells where the upstream value is above 1e-6 of the field max.
#[derive(Debug, Clone, Copy)]
struct Cmp {
    n: usize,
    identical: usize,
    worst_vs_max: f64,
    worst_cell_rel: f64,
}

fn compare(port: &[f64], up: &[f64]) -> Cmp {
    let max = up.iter().fold(0.0_f64, |m, v| m.max(v.abs()));
    let mut c = Cmp { n: up.len(), identical: 0, worst_vs_max: 0.0, worst_cell_rel: 0.0 };
    for (a, b) in port.iter().zip(up) {
        if a.to_bits() == b.to_bits() {
            c.identical += 1;
        }
        let d = (a - b).abs();
        c.worst_vs_max = c.worst_vs_max.max(d / max);
        if b.abs() > 1e-6 * max {
            c.worst_cell_rel = c.worst_cell_rel.max(d / b.abs());
        }
    }
    c
}

struct Run {
    snaps: BTreeMap<usize, (Vec<f64>, Vec<f64>, Vec<f64>, Vec<f64>, Vec<f64>)>,
    log: Vec<SolveRecord>,
}

fn run_port() -> Run {
    let mut s = build_solver();
    let mut snaps = BTreeMap::new();
    for step in 1..=100 {
        s.step().expect("step");
        if SNAPSHOT_STEPS.contains(&step) {
            let ux = s.u.internal.iter().map(|v| v.x).collect();
            let uy = s.u.internal.iter().map(|v| v.y).collect();
            let uz = s.u.internal.iter().map(|v| v.z).collect();
            let p = s.p.internal.iter().copied().collect();
            let phi = s.phi.internal.iter().copied().collect();
            snaps.insert(step, (ux, uy, uz, p, phi));
        }
    }
    Run { snaps, log: s.solve_log.take().unwrap() }
}

#[test]
fn cavity_fields_against_upstream_icofoam() {
    let reference = read_reference();
    let run = run_port();
    let mut worst = 0.0_f64;
    for (step, up) in &reference {
        let (ux, uy, uz, p, phi) = &run.snaps[step];
        let upx: Vec<f64> = up.u.iter().map(|v| v[0]).collect();
        let upy: Vec<f64> = up.u.iter().map(|v| v[1]).collect();
        let cx = compare(ux, &upx);
        let cy = compare(uy, &upy);
        let cp = compare(p, &up.p);
        let cphi = compare(phi, &up.phi);
        let uz_max = uz.iter().fold(0.0_f64, |m, v| m.max(v.abs()));
        println!("step {step:3}  Ux {cx:?}");
        println!("          Uy {cy:?}");
        println!("          p  {cp:?}   |Uz|max port {uz_max:e}");
        println!("          phi {cphi:?}");
        for c in [cx, cy, cp, cphi] {
            worst = worst.max(c.worst_vs_max);
            // Pin (gh:#1363): measured worst 1.2e-13 of the field maximum
            // over all quantities and steps; the bound is not relaxed.
            assert!(c.worst_vs_max < 3.0e-13, "step {step}: {c:?}");
            // The agreement is not bit-identity: the linear solvers differ.
            assert!(c.identical < c.n / 10, "step {step}: {c:?}");
        }
        assert!(uz_max < 1.0e-14, "step {step}: |Uz| {uz_max:e}");
    }
    println!("worst over all quantities and steps: {worst:e}");
}

#[test]
fn cavity_initial_residuals_against_upstream_icofoam() {
    let up = read_residuals();
    let run = run_port();
    assert_eq!(run.log.len(), 100 * 4);
    let mut worst_u = 0.0_f64;
    let mut worst_p = 0.0_f64;
    let mut compared = 0usize;
    for (i, row) in up.iter().enumerate() {
        let r = &run.log[4 * i..4 * i + 4];
        assert_eq!(r[0].field, "Ux");
        assert_eq!(r[1].field, "Uy");
        assert_eq!(r[2].field, "p");
        assert_eq!(r[3].field, "p");
        for k in 0..4 {
            let port = r[k].initial_residual;
            // Below 1e-6 the residual is dominated by the 1e-14 state difference.
            if row[k] < 1e-6 {
                continue;
            }
            compared += 1;
            let rel = (port - row[k]).abs() / row[k];
            if k < 2 {
                worst_u = worst_u.max(rel);
            } else {
                worst_p = worst_p.max(rel);
            }
        }
    }
    println!("compared {compared}; worst relative difference U {worst_u:e}, p {worst_p:e}");
    // Step 1: the first Ux and p initial residuals are exactly 1 on both sides.
    assert_eq!(run.log[0].initial_residual, 1.0);
    assert_eq!(run.log[2].initial_residual, 1.0);
    assert!(worst_u < 1.0e-7, "U residual worst {worst_u:e}");
    assert!(worst_p < 1.0e-7, "p residual worst {worst_p:e}");
}
