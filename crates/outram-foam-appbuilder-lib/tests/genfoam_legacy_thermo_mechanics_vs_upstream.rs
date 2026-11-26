// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//! # `legacyThermoMechanics` on the 3D_SmallESFR mechanics region, against GeN-Foam's own class (gh:#1781, round 23)
//!
//! AI-written, not human-reviewed. Research, education and V&V only.
//!
//! ## Methodology
//!
//! Upstream: GeN-Foam `652b3da0cc55` on OpenFOAM-ESI v2506, `solvers::legacyThermoMechanics` UNMODIFIED, built into
//! `libThermoMechanicsSolvers` by `scripts/build_and_run_legacy_tm_dump.sh` and driven by
//! `upstream_driver/legacy_tm_dump/legacyTmDump.C` (constructed as `meshHandler`/`regionSolvers` do; `correctPhysics()`
//! once per step as `GeN-Foam.C` does). Case: `reactorCases/3D_SmallESFR_NewSolverVerification/legacySolver`,
//! `thermoMechanicalRegion` (23340 cells, 8 cell zones, `leastSquares` gradients, `Gauss linear corrected` Laplacians,
//! `compactNormalStress yes`, `nCorrectors 3`), with the edits of `scripts/make_legacy_tm_case.sh`: the
//! `GFTractionDisplacement` patches (gh:#1235) become `fixedValue`/`zeroGradient`; the eight zone cards differ; every
//! linear solver `tolerance 1e-14`, `relTol 0`, `TStruct` `GAMG` -> `PBiCG`/`DILU` (named deviation); `TStruct` starts at
//! 668 K; `writePrecision 17`.
//!
//! The coupling inputs (`TStructFromTH`, `TFuel`, `powerDensityNeutronics`) are closed-form functions of the cell centre
//! and the step number (`+ - * /` only), evaluated identically here and in the driver; nothing of upstream's output is an
//! input. Reference: `refs/legacy_tm_esfr.ref.txt.gz` (`TStruct`, `disp`, `fuelDisp`, `CRDisp`, `meshDisp` per cell after
//! each of 3 steps, 17 digits) and `refs/legacy_tm_esfr_log.ref.txt.gz` (every linear solve's residuals and iteration
//! count, `Max sigmaEq`, `residual_`).
//!
//! Predictions (committed before the first run) and results: README, "Round 23".

// Reads case files: `std` only (gh:#1633).
#![cfg(feature = "std")]
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use miniz_oxide::inflate::decompress_to_vec;
use outram_foam_appbuilder_lib::genfoam::thermo_mechanics::legacy_fvm::case::{LegacyFvmCase, TStructGamgPolicy};
use outram_foam_appbuilder_lib::genfoam::thermo_mechanics::legacy_fvm::PhysicsLog;

fn vv_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("verification_and_validation/genfoam_vs_upstream_gh1361")
}

fn gunzip(path: &Path) -> Vec<u8> {
    let bytes = std::fs::read(path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    let flg = bytes[3];
    let mut at = 10;
    if flg & 4 != 0 {
        at += 2 + usize::from(bytes[at]) + 256 * usize::from(bytes[at + 1]);
    }
    for bit in [8u8, 16u8] {
        if flg & bit != 0 {
            while bytes[at] != 0 {
                at += 1;
            }
            at += 1;
        }
    }
    if flg & 2 != 0 {
        at += 2;
    }
    decompress_to_vec(&bytes[at..bytes.len() - 8]).unwrap_or_else(|_| panic!("inflate {}", path.display()))
}

fn copy_dir(src: &Path, dst: &Path) {
    std::fs::create_dir_all(dst).expect("mkdir");
    for e in std::fs::read_dir(src).expect("read_dir") {
        let e = e.expect("entry");
        let (s, d) = (e.path(), dst.join(e.file_name()));
        if s.is_dir() {
            copy_dir(&s, &d);
        } else {
            std::fs::copy(&s, &d).expect("copy");
        }
    }
}

/// Stage `cases/legacy_tm_esfr` with its polyMesh gunzipped.
fn stage(tag: &str) -> PathBuf {
    let root = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("gh1781_{tag}"));
    let _ = std::fs::remove_dir_all(&root);
    copy_dir(&vv_dir().join("cases/legacy_tm_esfr"), &root);
    let poly = root.join("constant/thermoMechanicalRegion/polyMesh");
    for f in ["points", "faces", "owner", "neighbour", "cellZones"] {
        let gz = poly.join(format!("{f}.gz"));
        std::fs::write(poly.join(f), gunzip(&gz)).expect("write polyMesh");
        std::fs::remove_file(gz).expect("rm gz");
    }
    root
}

/// `F <step> <name> v1 v2 ...` lines.
fn read_fields(path: &Path) -> HashMap<(usize, String), Vec<f64>> {
    let text = String::from_utf8(gunzip(path)).expect("utf8");
    let mut m = HashMap::new();
    for line in text.lines() {
        let mut it = line.split_whitespace();
        if it.next() != Some("F") {
            continue;
        }
        let step: usize = it.next().unwrap().parse().unwrap();
        let name = it.next().unwrap().to_string();
        m.insert((step, name), it.map(|w| w.parse::<f64>().expect("number")).collect());
    }
    m
}

/// The driver's `setCoupling(step)`, expression for expression.
fn coupling(
    mesh: &outram_foam_basic_lib::mesh::FvMesh,
    zones: &[(String, Vec<usize>)],
    step: usize,
) -> (Vec<f64>, Vec<f64>, Vec<f64>) {
    let n = step as f64;
    let nc = mesh.n_cells;
    let mut is_core = vec![false; nc];
    let mut is_por = vec![false; nc];
    for (name, cells) in zones {
        for &c in cells {
            match name.as_str() {
                "innerCore" | "outerCore" => {
                    is_core[c] = true;
                    is_por[c] = true;
                }
                "follower" | "controlRod" => is_por[c] = true,
                _ => {}
            }
        }
    }
    let (mut th, mut fuel, mut pow) = (vec![0.0; nc], vec![0.0; nc], vec![0.0; nc]);
    for c in 0..nc {
        let (x, y, z) = (mesh.cell_centres[c].x, mesh.cell_centres[c].y, mesh.cell_centres[c].z);
        let s = (z + 0.2) / 2.51;
        let r2 = x * x + y * y;
        let shape = 4.0 * s * (1.0 - s);
        th[c] = if is_por[c] { 700.0 + 300.0 * shape + 25.0 * r2 + 10.0 * n * s } else { 0.0 };
        fuel[c] = if is_core[c] { 900.0 + 400.0 * shape * (1.0 - 0.1 * r2) + 30.0 * n * (1.0 - 0.25 * r2) } else { 0.0 };
        pow[c] = 1.0e5 * (1.0 + 0.5 * s + 0.1 * n) * (1.0 + 0.2 * r2);
    }
    (th, fuel, pow)
}

/// Tally of one quantity against its reference.
#[derive(Debug, Default, Clone)]
struct Tally {
    n: usize,
    exact: usize,
    worst_abs: f64,
    worst_rel: f64,
    scale: f64,
}

fn compare(a: &[f64], b: &[f64]) -> Tally {
    assert_eq!(a.len(), b.len());
    let scale = b.iter().fold(0.0_f64, |m, v| m.max(v.abs()));
    let mut t = Tally { n: a.len(), scale, ..Tally::default() };
    for (x, y) in a.iter().zip(b) {
        if x == y {
            t.exact += 1;
        }
        let d = (x - y).abs();
        t.worst_abs = t.worst_abs.max(d);
    }
    t.worst_rel = if scale > 0.0 { t.worst_abs / scale } else { t.worst_abs };
    t
}

fn flatten(v: &[outram_foam_basic_lib::primitives::Vector3]) -> Vec<f64> {
    v.iter().flat_map(|d| [d.x, d.y, d.z]).collect()
}

struct Run {
    /// (step, field) -> tally
    tallies: Vec<(usize, &'static str, Tally)>,
    /// (step, solve index) -> (initial exact, final exact, iterations equal, rel diff of initial residual)
    solves: Vec<(usize, String, bool, bool, bool, f64)>,
    sigma_eq: Vec<(usize, f64, f64)>,
    residual: Vec<(usize, f64, f64)>,
}

fn run(tag: &str, n_steps: usize) -> Run {
    let root = stage(tag);
    let case = LegacyFvmCase::read(&root, "thermoMechanicalRegion", TStructGamgPolicy::SubstitutePbicgDilu).expect("case");
    let mut tm = case.build().expect("solver");
    let refs = read_fields(&vv_dir().join("refs/legacy_tm_esfr.ref.txt.gz"));
    let log_text = String::from_utf8(gunzip(&vv_dir().join("refs/legacy_tm_esfr_log.ref.txt.gz"))).unwrap();
    // per-step reference records
    let mut ref_steps: Vec<Vec<Vec<String>>> = Vec::new();
    for line in log_text.lines() {
        let w: Vec<String> = line.split_whitespace().map(str::to_string).collect();
        if w[0] == "T" {
            ref_steps.push(Vec::new());
        } else {
            ref_steps.last_mut().unwrap().push(w);
        }
    }
    let mut out = Run { tallies: Vec::new(), solves: Vec::new(), sigma_eq: Vec::new(), residual: Vec::new() };
    for step in 1..=n_steps {
        let (th, fuel, pow) = coupling(&case.mesh, &case.zones, step);
        tm.set_coupling_fields(&th, &fuel, &pow);
        let log: PhysicsLog = tm.correct_physics();
        let get = |name: &str| refs.get(&(step, name.to_string())).unwrap_or_else(|| panic!("ref {step} {name}"));
        out.tallies.push((step, "TStruct", compare(tm.t_struct(), get("TStruct"))));
        out.tallies.push((step, "disp", compare(&flatten(tm.displacement()), get("disp"))));
        out.tallies.push((step, "fuelDisp", compare(tm.fuel_displacement(), get("fuelDisp"))));
        out.tallies.push((step, "CRDisp", compare(tm.control_rod_displacement(), get("CRDisp"))));
        out.tallies.push((step, "meshDisp", compare(&flatten(tm.mesh_displacement()), get("meshDisp"))));
        let recs = &ref_steps[step - 1];
        let ref_solves: Vec<&Vec<String>> = recs.iter().filter(|r| r[0] == "S").collect();
        assert_eq!(log.solves.len(), ref_solves.len(), "step {step}: number of linear solves");
        for (mine, theirs) in log.solves.iter().zip(&ref_solves) {
            assert_eq!(mine.field, theirs[1], "step {step}: solve order");
            let (ri, rf, rn): (f64, f64, usize) = (theirs[2].parse().unwrap(), theirs[3].parse().unwrap(), theirs[4].parse().unwrap());
            let rel = if ri != 0.0 { (mine.initial_residual - ri).abs() / ri.abs() } else { (mine.initial_residual - ri).abs() };
            out.solves.push((step, mine.field.to_string(), mine.initial_residual == ri, mine.final_residual == rf, mine.n_iterations == rn, rel));
        }
        for r in recs {
            match r[0].as_str() {
                "M" => out.sigma_eq.push((step, log.max_sigma_eq.expect("sigmaEq"), r[1].parse().unwrap())),
                "R" => out.residual.push((step, log.residual, r[1].parse().unwrap())),
                _ => {}
            }
        }
    }
    out
}

fn report(r: &Run) {
    for (step, name, t) in &r.tallies {
        println!(
            "step {step} {name:9}: exact {:6}/{:6}  worst abs {:.3e}  worst rel-to-max {:.3e}  (max |ref| {:.6e})",
            t.exact, t.n, t.worst_abs, t.worst_rel, t.scale
        );
    }
    let n_solves = r.solves.len();
    let ex_i = r.solves.iter().filter(|s| s.2).count();
    let ex_f = r.solves.iter().filter(|s| s.3).count();
    let ex_n = r.solves.iter().filter(|s| s.4).count();
    println!("linear solves: {n_solves}; initial residual bit-identical {ex_i}, final {ex_f}, iterations equal {ex_n}");
    let worst = r.solves.iter().fold(0.0_f64, |m, s| m.max(s.5));
    println!("worst relative difference of an initial residual: {worst:.3e}");
    for (step, a, b) in &r.sigma_eq {
        println!("step {step} Max sigmaEq: port {a:.17e} upstream {b:.17e} rel {:.3e}", (a - b).abs() / b.abs());
    }
    for (step, a, b) in &r.residual {
        println!("step {step} residual_: port {a:.17e} upstream {b:.17e}");
    }
    for s in r.solves.iter().filter(|s| !(s.2 && s.3 && s.4)).take(6) {
        println!("  first non-identical solves: step {} {} init-exact {} final-exact {} iters-equal {} rel {:.3e}", s.0, s.1, s.2, s.3, s.4, s.5);
    }
}

/// The whole comparison: prints every tally (`--nocapture`); the pass criterion is stated in the README, "Round 23".
#[test]
fn legacy_thermo_mechanics_esfr_three_steps_vs_the_real_class() {
    let r = run("main", 3);
    report(&r);
    // Gate (README Round 23): see the results section for what it is and why.
    for (step, name, t) in &r.tallies {
        assert!(t.worst_rel <= GATE_REL, "step {step} {name}: worst rel {:.3e} > gate {GATE_REL:e}", t.worst_rel);
    }
}

/// The relative-to-max-norm gate on the fields, set from the measurement (README Round 23, results).
const GATE_REL: f64 = 1e-9;
