// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//! # GeN-Foam `twoPhase` through-flow boundary conditions vs the real binary (gh:#1361 round 12, gh:#1801)
//!
//! AI-written, not human-reviewed. Research, education and V&V only.
//!
//! The case is `featureCases/1D_PSBT_SC/Phase_Ex1_12223` AS SHIPPED (`tests/data/genfoam_psbt_shipped_gh1801`: velocity
//! inlet `fixedValue`, `pressureInletOutletVelocity` outlets, `slip` walls, `inletOutlet` `alpha.vapour`, `fixedFluxPressure`
//! `p_rgh`). Methodology, predictions (committed before the first port run) and results:
//! `verification_and_validation/genfoam_vs_upstream_gh1361/README.md`, "Round 12". Protocol: `scripts/psbt_shipped_protocol.sh`
//! (mirrored by [`protocol_copy`]). References: `refs/psbt_shipped_*.ref.txt.gz`, a fresh run of the installed binary.

// Reads OpenFOAM case files: `std` only (gh:#1633).
#![cfg(feature = "std")]
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use miniz_oxide::inflate::decompress_to_vec;
use outram_foam_appbuilder_lib::genfoam::thermal_hydraulics::solver::two_phase::{TwoPhaseCase, TwoPhaseSolver};

struct Reference {
    dt: Vec<f64>,
    fields: HashMap<(String, String), Vec<f64>>,
    times: Vec<String>,
    first: usize,
}

fn load_reference(name: &str) -> Reference {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("verification_and_validation/genfoam_vs_upstream_gh1361/refs")
        .join(format!("{name}.ref.txt.gz"));
    let bytes = std::fs::read(&path).expect("reference present");
    let text = String::from_utf8(decompress_to_vec(&bytes[10..bytes.len() - 8]).expect("inflate")).expect("utf8");
    let mut r = Reference { dt: vec![], fields: HashMap::new(), times: vec![], first: 1 };
    for line in text.lines() {
        let mut w = line.split_whitespace();
        match w.next() {
            Some("FIRST") => r.first = w.next().expect("first").parse().expect("first"),
            Some("DT") => r.dt = w.map(|x| x.parse().expect("dt")).collect(),
            Some("F") => {
                let t = w.next().expect("time").to_string();
                let f = w.next().expect("field").to_string();
                let v: Vec<f64> = w.map(|x| x.parse().expect("value")).collect();
                if !r.times.contains(&t) {
                    r.times.push(t.clone());
                }
                r.fields.insert((t, f), v);
            }
            _ => {}
        }
    }
    r
}

fn copy_dir(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).expect("mkdir");
    for e in std::fs::read_dir(from).expect("read_dir") {
        let e = e.expect("entry");
        let dest = to.join(e.file_name());
        if e.file_type().expect("type").is_dir() {
            copy_dir(&e.path(), &dest);
        } else {
            std::fs::copy(e.path(), &dest).expect("copy");
        }
    }
}

/// Copy of the vendored SHIPPED case with `scripts/psbt_shipped_protocol.sh` applied (EVERY_STEP=1, TIGHT=1).
fn protocol_copy(test: &str) -> PathBuf {
    let src = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/data/genfoam_psbt_shipped_gh1801");
    static COUNTER: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let k = COUNTER.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    let dst = std::env::temp_dir().join(format!("gh1801_psbt_{test}_{}_{k}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dst);
    copy_dir(&src, &dst);
    let cd = dst.join("system/controlDict");
    let text = std::fs::read_to_string(&cd).expect("controlDict");
    let text: Vec<String> = text
        .lines()
        .map(|l| {
            if l.starts_with("writeControl ") {
                "writeControl    timeStep;".to_string()
            } else if l.starts_with("writeInterval ") {
                "writeInterval   1;".to_string()
            } else {
                l.to_string()
            }
        })
        .collect();
    std::fs::write(&cd, text.join("\n") + "\n").expect("write controlDict");
    let f = dst.join("system/fluidRegion/fvSolution");
    let text = std::fs::read_to_string(&f).expect("fvSolution");
    let mut out = String::new();
    let mut in_solvers = false;
    for line in text.lines() {
        let t = line.trim_start();
        if line.starts_with("solvers") {
            in_solvers = true;
        }
        if line.starts_with("PIMPLE") {
            in_solvers = false;
        }
        if in_solvers && t.starts_with("tolerance") {
            out.push_str("        tolerance       1e-14;\n");
        } else if in_solvers && t.starts_with("relTol") {
            out.push_str("        relTol          0;\n");
        } else {
            out.push_str(line);
            out.push('\n');
        }
    }
    std::fs::write(&f, out).expect("write fvSolution");
    dst
}

/// Largest `|a - b|` over a field divided by the field's largest `|b|` (an exact zero compares exactly).
fn worst_rel(port: &[f64], up: &[f64]) -> f64 {
    assert_eq!(port.len(), up.len(), "reference field has the wrong length");
    let scale = up.iter().fold(0.0_f64, |m, x| m.max(x.abs()));
    port.iter()
        .zip(up)
        .map(|(a, b)| if scale == 0.0 { (a - b).abs() } else { (a - b).abs() / scale })
        .fold(0.0, f64::max)
}

fn port_fields(s: &TwoPhaseSolver) -> Vec<(&'static str, Vec<f64>)> {
    vec![
        ("p", s.p.c.clone()),
        ("p_rgh", s.p_rgh.c.clone()),
        ("T.liquid", s.fluids[0].t.c.clone()),
        ("T.vapour", s.fluids[1].t.c.clone()),
        ("T.interface", s.t_interface.clone()),
        ("T.fixedPower", s.structure.t_act.clone()),
        ("U.liquid", s.fluids[0].u.c.iter().map(|u| u.x).collect()),
        ("U.vapour", s.fluids[1].u.c.iter().map(|u| u.x).collect()),
        ("alpha.vapour", s.fluids[1].alpha.c.clone()),
        ("dmdt.liquid.vapour", s.dmdt.clone()),
        ("htc.liquid.structure", s.htc_fs[0].clone()),
        ("areaDensity.interface", s.interfacial_area.clone()),
        ("alphaRhoMagU.liquid", s.fluids[0].alpha_rho_mag_u.clone()),
        ("phi.liquid", s.fluids[0].phi[..s.geom.n_internal()].to_vec()),
        ("phi.vapour", s.fluids[1].phi[..s.geom.n_internal()].to_vec()),
    ]
}

/// (step, relative step-size difference, port outer iterations, worst relative difference per field).
type Row = (usize, f64, usize, Vec<(&'static str, f64)>);

fn run_rows(test: &str, refs: &[&str], n_steps: usize) -> Vec<Row> {
    let refs: Vec<Reference> = refs.iter().map(|r| load_reference(r)).collect();
    let dir = protocol_copy(test);
    let case = TwoPhaseCase::read(&dir).expect("read the SHIPPED PSBT case");
    // upstream's customPimpleControl defect is the specification (as in rounds 3-7)
    let mut s = round13_solver(TwoPhaseSolver::new(round13_case(case)).with_upstream_stale_pimple_converged_flag());
    let mut rows = Vec::new();
    for n in 1..=n_steps {
        let rep = s.step().expect("step");
        let Some(r) = refs.iter().find(|r| n >= r.first && n - r.first < r.times.len()) else { continue };
        let dt_up = if n == 1 { 5e-3 } else { r.dt[n - 2] };
        let dt_rel = (rep.dt - dt_up).abs() / dt_up;
        let tname = &r.times[n - r.first];
        let mut worst = Vec::new();
        for (name, v) in port_fields(&s) {
            if let Some(u) = r.fields.get(&(tname.clone(), name.to_string())) {
                worst.push((name, worst_rel(&v, u)));
            }
        }
        rows.push((n, dt_rel, rep.outer_iterations, worst));
    }
    let _ = std::fs::remove_dir_all(&dir);
    rows
}

fn print_rows(rows: &[Row]) {
    for (n, dt_rel, iters, w) in rows {
        let cells: Vec<String> = w.iter().map(|(k, v)| format!("{k}={v:.2e}")).collect();
        eprintln!("step {n:4} dt_rel={dt_rel:.2e} iters={iters} {}", cells.join(" "));
    }
}

/// Diagnostic: print every compared step (`--ignored --nocapture`); `REFS` names the reference files, `STEPS` the steps.
#[test]
#[ignore]
fn psbt_shipped_dump_rows() {
    let refs = std::env::var("REFS").unwrap_or_else(|_| "psbt_shipped_head".into());
    let refs: Vec<&str> = refs.split(',').collect();
    let n: usize = std::env::var("STEPS").ok().and_then(|v| v.parse().ok()).unwrap_or(60);
    print_rows(&run_rows("dump", &refs, n));
}

/// Diagnostic: the port's own state at selected steps (time, step size, maximum vapour fraction, `U.liquid` maximum, outer iterations).
#[test]
#[ignore]
fn psbt_shipped_port_summary() {
    let dir = protocol_copy("summary");
    let case = TwoPhaseCase::read(&dir).expect("read the SHIPPED PSBT case");
    let mut s = round13_solver(TwoPhaseSolver::new(round13_case(case)).with_upstream_stale_pimple_converged_flag());
    for n in 1..=700usize {
        let rep = match s.step() {
            Ok(r) => r,
            Err(e) => {
                eprintln!("SUMMARY step {n}: {e:?}");
                break;
            }
        };
        if [13, 14, 40, 50, 51, 60, 100, 200, 300, 407, 500, 600, 700].contains(&n) {
            let amax = s.fluids[1].alpha.c.iter().fold(0.0_f64, |m, x| m.max(*x));
            let umax = s.fluids[0].u.c.iter().fold(0.0_f64, |m, x| m.max(x.x));
            eprintln!("SUMMARY step {n} t={:.6} dt={:.4e} alpha_max={amax:.5} U_max={umax:.4} iters={}", s.time, rep.dt, rep.outer_iterations);
        }
        if s.time >= 0.6 {
            eprintln!("SUMMARY reached t = {:.6} at step {n}", s.time);
            break;
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// Diagnostic: the cells in which `FIELD` (default `T.vapour`) of the port differs from upstream's by more than `TOL` (default 1e-4 of the
/// field maximum) at step `STEP` (default 51), with the vapour fraction there.
#[test]
#[ignore]
fn psbt_shipped_cell_dump() {
    let step: usize = std::env::var("STEP").ok().and_then(|v| v.parse().ok()).unwrap_or(51);
    let field = std::env::var("FIELD").unwrap_or_else(|_| "T.vapour".into());
    let tol: f64 = std::env::var("TOL").ok().and_then(|v| v.parse().ok()).unwrap_or(1e-4);
    let r = load_reference("psbt_shipped_head");
    let dir = protocol_copy("cells");
    let mut s = TwoPhaseSolver::new(TwoPhaseCase::read(&dir).expect("read")).with_upstream_stale_pimple_converged_flag();
    for _ in 1..=step {
        s.step().expect("step");
    }
    let t = &r.times[step - r.first];
    let up = &r.fields[&(t.clone(), field.clone())];
    let al = &r.fields[&(t.clone(), "alpha.vapour".to_string())];
    let port = port_fields(&s).into_iter().find(|(k, _)| *k == field).expect("field").1;
    let scale = up.iter().fold(0.0_f64, |m, x| m.max(x.abs()));
    for c in 0..up.len() {
        if (port[c] - up[c]).abs() / scale > tol {
            eprintln!("CELL {c} {field} port={:.12e} up={:.12e} alpha_up={:.6e} alpha_port={:.6e}", port[c], up[c], al[c], s.fluids[1].alpha.c[c]);
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
}

fn worst_over(rows: &[Row], lo: usize, hi: usize, field: &str) -> f64 {
    rows.iter()
        .filter(|(n, ..)| *n >= lo && *n <= hi)
        .flat_map(|(_, _, _, w)| w.iter().filter(|(k, _)| *k == field).map(|(_, v)| *v))
        .fold(0.0, f64::max)
}

/// Round 12 gate (results in the README, "Round 12"): the shipped through-flow case against upstream's fresh 17-digit run.
/// Steps 1-9 (single phase) agree to 1e-8 of the field maximum in every compared field INCLUDING the internal-face fluxes,
/// the step size to 1e-5 and the outer-iteration counts (12, 0, 11, 11, 11, 11, 10, 10, 10) are upstream's; the vapour appears at
/// step 13; steps 10-50 stay within 1e-3 (`alpha.vapour`, `dmdt`, `areaDensity`) and 1e-4 (everything else).
#[test]
fn shipped_through_flow_case_matches_upstream_to_the_onset_and_beyond() {
    let rows = run_rows("gate", &["psbt_shipped_head"], 50);
    let iters: Vec<usize> = rows.iter().take(9).map(|r| r.2).collect();
    assert_eq!(iters, vec![12, 0, 11, 11, 11, 11, 10, 10, 10], "outer iterations of steps 1-9");
    for (n, dt_rel, _, w) in &rows {
        assert!(*dt_rel < 1e-5, "step {n}: dt differs by {dt_rel:e}");
        for (k, v) in w {
            let lim = if *n <= 9 {
                1e-8
            } else if *k == "alpha.vapour" || *k == "dmdt.liquid.vapour" || *k == "areaDensity.interface" {
                1e-3
            } else {
                1e-4
            };
            assert!(*v <= lim, "step {n} {k}: {v:e} exceeds {lim:e}");
        }
    }
    // the vapour first appears at step 13 in both codes
    let r = load_reference("psbt_shipped_head");
    for n in [12usize, 13] {
        let t = &r.times[n - 1];
        let up_max = r.fields[&(t.clone(), "alpha.vapour".to_string())].iter().fold(0.0_f64, |m, x| m.max(*x));
        assert_eq!(up_max > 0.0, n == 13, "upstream onset step {n}");
    }
}

/// The measured end of agreement (gh:#1361 round 12, `psbt_shipped_cell_dump`): at step 51 the port's `T.vapour` in cells 64-68 (vapour
/// fraction 4.4e-3, below `thermoResidualAlpha` 1e-2 of the vapour) leaves upstream's saturated 613.8 K for 640-725 K, and the two
/// trajectories separate. This pins the measured size of that difference (a recorded, unexplained defect), so that a change that fixes it is noticed.
#[test]
fn shipped_case_vapour_temperature_separates_at_step_51() {
    let rows = run_rows("gate51", &["psbt_shipped_head"], 51);
    let before = worst_over(&rows, 40, 50, "T.vapour");
    let at = worst_over(&rows, 51, 51, "T.vapour");
    assert!(before < 1e-6, "T.vapour steps 40-50: {before:e}");
    assert!(at > 1e-2, "T.vapour at step 51 should show the recorded separation, measured 1.8e-1: {at:e}");
}

/// gh:#1801: a boundary condition the solver does not implement is refused BY NAME on every field (it used to become `calculated`
/// silently), and the shipped selections read with no deviation.
#[test]
fn unported_boundary_conditions_are_refused_by_name() {
    let dir = protocol_copy("refuse");
    let shipped = TwoPhaseCase::read(&dir).expect("the shipped case reads");
    assert!(shipped.deviations.is_empty());
    let cases = [
        ("p_rgh", "fixedFluxPressure", "totalPressure"),
        ("T.liquid", "fixedValue", "inletOutlet"),
        ("alpha.vapour", "inletOutlet", "fixedValue"),
        ("U.liquid", "slip", "partialSlip"),
        ("U.liquid", "fixedValue", "flowRateInletVelocity"),
    ];
    for (file, from, to) in cases {
        let path = dir.join("0/fluidRegion").join(file);
        let original = std::fs::read_to_string(&path).expect("field file");
        assert!(original.contains(from), "{file} has no {from}");
        std::fs::write(&path, original.replacen(from, to, 1)).expect("write");
        let err = TwoPhaseCase::read(&dir).err().unwrap_or_else(|| panic!("{file}: {to} must be refused"));
        let msg = format!("{err:?}");
        assert!(msg.contains(to) && msg.contains(file), "{file}: the refusal names neither field nor type: {msg}");
        std::fs::write(&path, original).expect("restore");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// Round 13 ablation (diagnostic): `ROUND13_OFF=gas,newton,markers,flux` (any subset) switches the named round-13 translations back
/// to the port's earlier forms, to measure how much of the agreement each one carries. Unset: upstream's forms (the defaults).
fn round13_off(name: &str) -> bool {
    std::env::var("ROUND13_OFF").map(|v| v.split(',').any(|x| x == name)).unwrap_or(false)
}

fn round13_case(case: TwoPhaseCase) -> TwoPhaseCase {
    if round13_off("gas") {
        case.with_codata2019_gas_constant()
    } else {
        case
    }
}

fn round13_solver(mut s: TwoPhaseSolver) -> TwoPhaseSolver {
    if round13_off("newton") {
        s = s.with_converged_temperature_newton();
    }
    if round13_off("markers") {
        s = s.with_live_thermo_residual_markers();
    }
    if round13_off("flux") {
        s = s.with_earlier_interfacial_flux_ratio();
    }
    s
}
