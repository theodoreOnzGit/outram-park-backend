//! `RhoCentralFoam` against upstream OpenFOAM-v2506 `rhoCentralFoam` on the
//! shockTube, per cell (gh:#1363, row 2).
//!
//! # Method
//! `verification_and_validation/upstream_openfoam_gh1363/` holds the case (v2506
//! tutorial `compressible/rhoCentralFoam/shockTube`, 100 cells, fixed
//! dt = 1e-5 s), the v2506 `blockMesh` polyMesh the port reads unchanged, and
//! the upstream outputs as IEEE-754 bit patterns (rho, Ux, T, p, e at steps
//! 1, 2, 10, 100, 700) from `driver/rhoCentralDump.C` (rhoCentralFoam.C line
//! for line on the upstream libraries; see
//! `verification_and_validation/rho_central_foam_vs_openfoam.md` for the one
//! substitution). The port runs the same case and the test counts bit-identical
//! cells and the worst difference per quantity.
//!
//! # Results
//! See `verification_and_validation/rho_central_foam_vs_openfoam.md`.

use miniz_oxide::inflate::decompress_to_vec;
use outram_foam_appbuilder_lib::io::control_dict::{ControlDict, StartControl, StopControl};
use outram_foam_appbuilder_lib::io::field_reader::{read_vol_scalar_field_full, read_vol_vector_field_full};
use outram_foam_appbuilder_lib::io::fv_schemes::FvSchemes;
use outram_foam_appbuilder_lib::io::fv_solution::FvSolution;
use outram_foam_appbuilder_lib::io::poly_mesh::read_poly_mesh;
use outram_foam_appbuilder_lib::solvers::rho_central_foam::{PerfectGas, RhoCentralFoam};
use std::collections::BTreeMap;
use std::path::PathBuf;

const DT: f64 = 1e-5;
const DUMP_STEPS: [usize; 15] = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 20, 50, 100, 300, 700];

fn case_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("verification_and_validation/upstream_openfoam_gh1363/shock")
}

/// Upstream fields at one step, keyed by name ("rho", "Ux", "T", "p", "e").
type Fields = BTreeMap<String, Vec<f64>>;

/// Upstream gas constants (bit patterns) and per-step fields.
struct Reference {
    /// `[R, Cp, Cv, W]` of the mixture, as upstream computes them.
    consts: [f64; 8],
    steps: BTreeMap<usize, Fields>,
    /// Upstream mesh geometry: `V`, `Sfx` (internal faces), `Cx`.
    geom: Fields,
}

fn read_reference() -> Reference {
    let path = case_dir().join("reference/shock_v2506_rhoCentral.txt.gz");
    let bytes = std::fs::read(path).expect("reference");
    let text = String::from_utf8(decompress_to_vec(&bytes[10..bytes.len() - 8]).expect("inflate")).expect("utf8");
    let mut consts = [0.0; 8];
    let mut steps: BTreeMap<usize, Fields> = BTreeMap::new();
    let mut geom = Fields::new();
    let mut step = 0usize;
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("# STEP ") {
            step = rest.trim().parse().unwrap();
            steps.insert(step, Fields::new());
            continue;
        }
        if line.starts_with('#') {
            continue;
        }
        let mut it = line.split_whitespace();
        let (name, idx, hex) = (it.next().unwrap(), it.next().unwrap(), it.next().unwrap());
        let v = f64::from_bits(u64::from_str_radix(hex, 16).unwrap());
        if ["V", "Sfx", "Cx"].contains(&name) {
            geom.entry(name.to_string()).or_default().push(v);
        } else if name == "const" {
            consts[idx.parse::<usize>().unwrap()] = v;
        } else {
            steps.get_mut(&step).unwrap().entry(name.to_string()).or_default().push(v);
        }
    }
    Reference { consts, steps, geom }
}

/// Count of bit-identical values, worst |difference| over the field max, and
/// worst per-cell relative difference.
#[derive(Debug, Clone, Copy)]
struct Cmp {
    identical: usize,
    worst_vs_max: f64,
    worst_cell_rel: f64,
}

fn compare(port: &[f64], up: &[f64]) -> Cmp {
    let max = up.iter().fold(0.0_f64, |m, v| m.max(v.abs()));
    let mut c = Cmp { identical: 0, worst_vs_max: 0.0, worst_cell_rel: 0.0 };
    for (a, b) in port.iter().zip(up) {
        if a.to_bits() == b.to_bits() {
            c.identical += 1;
        }
        let d = (a - b).abs();
        if max > 0.0 {
            c.worst_vs_max = c.worst_vs_max.max(d / max);
        }
        if b.abs() > 1e-6 * max {
            c.worst_cell_rel = c.worst_cell_rel.max(d / b.abs());
        }
    }
    c
}

fn build_port(consts: &[f64; 8]) -> RhoCentralFoam {
    let case = case_dir();
    let mesh = read_poly_mesh(&case.join("constant/polyMesh")).expect("polyMesh");
    let n = mesh.n_cells;
    let p0 = read_vol_scalar_field_full(&case.join("0/p"), &mesh).expect("0/p");
    let t0 = read_vol_scalar_field_full(&case.join("0/T"), &mesh).expect("0/T");
    let u0 = read_vol_vector_field_full(&case.join("0/U"), &mesh).expect("0/U");
    let control = ControlDict {
        start: StartControl::StartTime(0.0),
        stop: StopControl::EndTime(DT * 700.0 + DT / 2.0),
        delta_t: DT,
        ..ControlDict::default()
    };
    // thermophysicalProperties: molWeight 28.96, Cp 1004.5 (hConst, perfectGas)
    let gas = PerfectGas::from_molar_weight_and_cp(28.96, 1004.5);
    // The port's gas arithmetic is upstream's: constants and Es are bit-identical.
    assert_eq!(gas.r.to_bits(), consts[0].to_bits(), "R");
    assert_eq!(gas.cp.to_bits(), consts[1].to_bits(), "Cp");
    assert_eq!(gas.cv().to_bits(), consts[2].to_bits(), "Cv");
    assert_eq!(gas.es(1e5, 348.432).to_bits(), consts[4].to_bits(), "Es(1e5, 348.432)");
    assert_eq!(gas.es(1e4, 278.746).to_bits(), consts[5].to_bits(), "Es(1e4, 278.746)");
    assert_eq!(gas.the(consts[4], 1e5, 300.0).unwrap().to_bits(), consts[6].to_bits(), "THE 1");
    assert_eq!(gas.the(consts[5], 1e4, 348.432).unwrap().to_bits(), consts[7].to_bits(), "THE 2");
    let mut s = RhoCentralFoam::new(mesh, control, FvSchemes::default(), FvSolution::default())
        .with_gas(gas);
    let mut t_file = Vec::with_capacity(n);
    for c in 0..n {
        // upstream: e = HE(p, T), psi = 1/(R T), rho = psi*p
        let t = t0.internal[c];
        t_file.push(t);
        let psi = gas.psi(t);
        s.rho.internal[c] = psi * p0.internal[c];
        s.e.internal[c] = gas.es(p0.internal[c], t);
    }
    s.set_initial_temperature(t_file);
    s.p = p0;
    s.u = u0;
    s
}

fn run_port(consts: &[f64; 8]) -> BTreeMap<usize, Fields> {
    let mut s = build_port(consts);
    let mut out = BTreeMap::new();
    for step in 1..=700usize {
        s.step().expect("step");
        if DUMP_STEPS.contains(&step) {
            let mut f = Fields::new();
            let rho: Vec<f64> = s.rho.internal.iter().copied().collect();
            let p: Vec<f64> = s.p.internal.iter().copied().collect();
            f.insert("Ux".into(), s.u.internal.iter().map(|v| v.x).collect());
            f.insert("T".into(), s.temperature().unwrap().to_vec());
            f.insert("rho".into(), rho);
            f.insert("e".into(), s.e.internal.iter().copied().collect());
            f.insert("p".into(), p);
            out.insert(step, f);
        }
    }
    out
}

#[test]
fn shock_tube_is_bit_identical_to_upstream_rhocentralfoam() {
    let reference = read_reference();
    let port = run_port(&reference.consts);

    // Geometry first: the port's mesh reader must reproduce upstream's cell
    // volumes and centres bit for bit (it did not before gh:#1363: 63 of 100
    // volumes differed, up to 14 ulp, from a private older copy of the
    // geometry code).
    let mesh = read_poly_mesh(&case_dir().join("constant/polyMesh")).unwrap();
    let v: Vec<f64> = mesh.cell_volumes.to_vec();
    let sfx: Vec<f64> = (0..mesh.n_internal_faces).map(|f| mesh.face_area_vectors[f].x).collect();
    let cx: Vec<f64> = mesh.cell_centres.iter().map(|c| c.x).collect();
    for (name, port_vals) in [("V", &v), ("Sfx", &sfx), ("Cx", &cx)] {
        let c = compare(port_vals, &reference.geom[name]);
        assert_eq!(c.identical, port_vals.len(), "geometry {name}: {c:?}");
    }

    // Fields: bit-identity at every dumped step. Measured 2026-11-11,
    // release: 100/100 cells for rho, Ux, T, p and e at steps 1, 2, ..., 10,
    // 20, 50, 100, 300 and 700 (t = 0.007 s, the tutorial end time).
    for (step, up) in &reference.steps {
        for name in ["rho", "Ux", "T", "p", "e"] {
            let c = compare(&port[step][name], &up[name]);
            assert_eq!(c.identical, up[name].len(), "step {step} {name}: {c:?}");
            assert_eq!(c.worst_vs_max, 0.0, "step {step} {name}");
        }
    }
}

/// The gas arithmetic is upstream's: constants, `Es` and the Newton
/// iteration agree with the values the upstream thermo classes print
/// (checked inside `build_port`), and the default gas is calorically perfect
/// with `e = Cv T`, as every pre-gh:#1363 caller assumes.
#[test]
fn default_gas_is_calorically_perfect_air_with_e_equal_cv_t() {
    let g = PerfectGas::default();
    let t = 300.0;
    let p = 1.0e5;
    let e = g.es(p, t);
    assert!((e - g.cv() * t).abs() < 1e-9 * e.abs());
    assert!((g.cp / g.cv() - 1.4).abs() < 1e-14);
    let back = g.the(e, p, 250.0).unwrap();
    assert!((back - t).abs() < 1e-9);
    assert!(g.the(e, p, -1.0).is_err());
}
