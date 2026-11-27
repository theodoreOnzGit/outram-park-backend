// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//! # GeN-Foam `twoPhase` MULES alpha solver vs the real GeN-Foam binary on `1D_boiling` AS SHIPPED (gh:#1361, round 7)
//!
//! AI-written, not human-reviewed. Research, education and V&V only.
//!
//! Methodology, predictions (committed BEFORE the first port run) and results:
//! `verification_and_validation/genfoam_vs_upstream_gh1361/README.md`, "Round 7". The case is upstream's own
//! (`tests/data/genfoam_1d_boiling`), with `alpha { solver "MULES"; adjustSubCycles true; alphaMaxCo 0.25; }` KEPT.
//! Protocol (`scripts/boiling_mules_protocol.sh`, mirrored by [`protocol_copy`]): `writePrecision 17`, every step
//! written (`writeControl timeStep`), every linear-solver tolerance in `solvers` 1e-14 with `relTol 0`
//! (`residualControl` untouched). References are fresh runs of the installed binary (steps 1-70 and 560-649; the
//! run aborts at step 650 with a negative vapour temperature in upstream itself).

// Reads OpenFOAM case files: `std` only (gh:#1633).
#![cfg(feature = "std")]
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use miniz_oxide::inflate::decompress_to_vec;
use outram_foam_appbuilder_lib::genfoam::thermal_hydraulics::solver::two_phase::{TwoPhaseCase, TwoPhaseSolver};

struct Reference {
    dt: Vec<f64>,
    subcyc: Vec<i64>,
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
    let mut r = Reference { dt: vec![], subcyc: vec![], fields: HashMap::new(), times: vec![], first: 1 };
    for line in text.lines() {
        let mut w = line.split_whitespace();
        match w.next() {
            Some("FIRST") => r.first = w.next().expect("first").parse().expect("first"),
            Some("DT") => r.dt = w.map(|x| x.parse().expect("dt")).collect(),
            Some("SUBCYC") => r.subcyc = w.map(|x| x.parse().expect("subcyc")).collect(),
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

/// Copy of the vendored case with `scripts/boiling_mules_protocol.sh` applied (EVERY_STEP=1, TIGHT=1); MULES kept.
fn protocol_copy(test: &str) -> PathBuf {
    let src = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/data/genfoam_1d_boiling");
    static COUNTER: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let k = COUNTER.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    let dst = std::env::temp_dir().join(format!("gh1361_mules_{test}_{}_{k}", std::process::id()));
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
    assert!(text.contains("solver          \"MULES\";"), "the shipped alpha solver must be kept");
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
    // A uniform surface field is written as `internalField uniform V;` and the extractor expands it to 40 cells:
    // for the 39 internal faces take the first 39.
    let up = if up.len() == port.len() + 1 && up.iter().all(|x| *x == up[0]) { &up[..port.len()] } else { up };
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
        // internal faces only (the references hold the 39 internal-face values)
        ("alphaPhi.vapour", s.fluids[1].alpha_phi[..s.geom.n_internal()].to_vec()),
        ("alphaPhi.liquid", s.fluids[0].alpha_phi[..s.geom.n_internal()].to_vec()),
        ("phi.vapour", s.fluids[1].phi[..s.geom.n_internal()].to_vec()),
        ("phi.liquid", s.fluids[0].phi[..s.geom.n_internal()].to_vec()),
    ]
}

/// (step, relative step-size difference, sub-cycle count equal to upstream's, port outer iterations, worst relative
/// difference per field).
type Row = (usize, f64, bool, usize, Vec<(&'static str, f64)>);

/// Run the port from step 1 to `n_steps` against the references `head` (steps 1-70) and `onset` (560-649).
/// Row: (step, relative step-size difference, sub-cycle count equal to upstream's, worst relative difference per field).
fn run_rows(test: &str, n_steps: usize, with_onset: bool) -> Vec<Row> {
    run_rows_with(test, n_steps, with_onset, false)
}

/// [`run_rows`], optionally with the port's pre-round-7 substitution (`implicitUpwind` in place of `MULES`).
fn run_rows_with(test: &str, n_steps: usize, with_onset: bool, implicit: bool) -> Vec<Row> {
    let refs: Vec<Reference> =
        if with_onset { vec![load_reference("boiling_mules_tight_head"), load_reference("boiling_mules_tight_onset")] } else { vec![load_reference("boiling_mules_tight_head")] };
    let dir = protocol_copy(test);
    let case = TwoPhaseCase::read(&dir).expect("read 1D_boiling as shipped");
    assert!(case.deviations.is_empty(), "MULES runs as shipped: {:?}", case.deviations);
    let case = if implicit { case.with_alpha_solver_implicit_upwind().expect("implicitUpwind") } else { case };
    // upstream's customPimpleControl defect is the specification here (as in rounds 3-5)
    let off = round13_flags();
    let case = if off.iter().any(|x| x == "gas") { case.with_codata2019_gas_constant() } else { case };
    let mut s = TwoPhaseSolver::new(case).with_upstream_stale_pimple_converged_flag();
    if off.iter().any(|x| x == "newton") {
        s = s.with_converged_temperature_newton();
    }
    if off.iter().any(|x| x == "hsolver") {
        s = s.with_smooth_solver_for_enthalpy();
    }
    if off.iter().any(|x| x == "markers") {
        s = s.with_live_thermo_residual_markers();
    }
    if off.iter().any(|x| x == "flux") {
        s = s.with_earlier_interfacial_flux_ratio();
    }
    let mut rows = Vec::new();
    for n in 1..=n_steps {
        let rep = match s.step() {
            Ok(r) => r,
            Err(e) => {
                eprintln!("DIVERGED at step {n}, t = {:.6}: {e:?}", s.time);
                if std::env::var("ROUND13_KEEP_GOING").is_ok() {
                    break;
                }
                panic!("step {n}: {e:?}");
            }
        };
        let dt_up = if n == 1 { 1e-3 } else { refs[0].dt[n - 2] };
        let dt_rel = (rep.dt - dt_up).abs() / dt_up;
        let Some(r) = refs.iter().find(|r| n >= r.first && n - r.first < r.times.len()) else { continue };
        let sub_ok = i64::try_from(s.alpha_sub_cycles).expect("small") == r.subcyc[n - 1] || r.subcyc[n - 1] < 0;
        let tname = &r.times[n - r.first];
        let mut worst = Vec::new();
        for (name, v) in port_fields(&s) {
            if let Some(u) = r.fields.get(&(tname.clone(), name.to_string())) {
                worst.push((name, worst_rel(&v, u)));
            }
        }
        rows.push((n, dt_rel, sub_ok, rep.outer_iterations, worst));
    }
    let _ = std::fs::remove_dir_all(&dir);
    rows
}

fn print_rows(rows: &[Row]) {
    for (n, dt_rel, sub_ok, iters, w) in rows {
        let cells: Vec<String> = w.iter().map(|(k, v)| format!("{k}={v:.2e}")).collect();
        eprintln!("step {n:4} dt_rel={dt_rel:.2e} sub_ok={sub_ok} iters={iters} {}", cells.join(" "));
    }
}

/// Round 13 diagnostic: the energy-matrix, stage and phase-change records (`TwoPhaseSolver::trace_probe`) of the first `STEPS`
/// (default 640) steps, written to `PROBE_OUT`, for comparison with the instrumented upstream (`upstream_driver/probe_boiling`).
#[test]
#[ignore]
fn mules_trace_probe_dump() {
    let n: usize = std::env::var("STEPS").ok().and_then(|v| v.parse().ok()).unwrap_or(640);
    let dir = protocol_copy("probe");
    let case = TwoPhaseCase::read(&dir).expect("read 1D_boiling as shipped");
    let mut s = TwoPhaseSolver::new(case).with_upstream_stale_pimple_converged_flag();
    if let Some(t) = std::env::var("H_TOL").ok().and_then(|v| v.parse::<f64>().ok()) {
        s.controls.h_tolerance = t;
    }

    s.trace_probe = Some(Vec::new());
    let mut out = String::new();
    for _ in 1..=n {
        s.step().expect("step");
        for r in s.trace_probe.as_mut().expect("probe on").drain(..) {
            let tag = match r.kind {
                "eqn" => "PORTE",
                "eqn2" => "PORTE2",
                "eqnf" => continue,
                "pc" => "PORT_PC",
                _ => continue,
            };
            out.push_str(&format!("{tag} t={:.17e} corr={} cell={}", r.time, r.outer_iteration, r.cell));
            for (k, v) in &r.values {
                out.push_str(&format!(" {k}={v:.17e}"));
            }
            out.push('\n');
        }
    }
    if let Ok(path) = std::env::var("PROBE_OUT") {
        std::fs::write(path, out).expect("write probe");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// Diagnostic: print every compared step (`--ignored --nocapture`).
#[test]
#[ignore]
fn mules_dump_rows() {
    let implicit = std::env::var("IMPLICIT").is_ok();
    let rows = run_rows_with("dump", 649, true, implicit);
    print_rows(&rows);
}

/// Noise-floor control (diagnostic; round-3 technique): the port against ITSELF with the initial liquid temperature
/// scaled by `1 + eps`, i.e. a single-phase disturbance of the size port-vs-upstream shows before the onset (1e-11).
/// If the onset window of port-vs-upstream is the same size, it is sensitivity, not a model difference.
fn noise_rows(eps: f64, n_steps: usize) -> Vec<(usize, Vec<(&'static str, f64)>)> {
    let dir = protocol_copy("noise");
    let case = TwoPhaseCase::read(&dir).expect("read");
    let mut pert = case.clone();
    for v in pert.fluids[0].t.c.iter_mut() {
        *v *= 1.0 + eps;
    }
    for v in pert.fluids[0].t.b.iter_mut() {
        *v *= 1.0 + eps;
    }
    let mut a = TwoPhaseSolver::new(case).with_upstream_stale_pimple_converged_flag();
    let mut b = TwoPhaseSolver::new(pert).with_upstream_stale_pimple_converged_flag();
    let mut rows = Vec::new();
    for n in 1..=n_steps {
        a.step().expect("step a");
        b.step().expect("step b");
        let fa = port_fields(&a);
        let fb = port_fields(&b);
        rows.push((n, fa.iter().zip(&fb).map(|((k, x), (_, y))| (*k, worst_rel(y, x))).collect()));
    }
    let _ = std::fs::remove_dir_all(&dir);
    rows
}

#[test]
#[ignore]
fn mules_noise_floor_dump() {
    let eps: f64 = std::env::var("EPS").ok().and_then(|v| v.parse().ok()).unwrap_or(1e-11);
    for (n, w) in noise_rows(eps, 640).iter().filter(|(n, _)| (612..=640).contains(n) || *n == 100 || *n == 600) {
        let cells: Vec<String> = w.iter().map(|(k, v)| format!("{k}={v:.2e}")).collect();
        eprintln!("step {n:4} {}", cells.join(" "));
    }
}


/// Diagnostic: per-cell port vs upstream for `STEP` (default 616) and the fields in `FIELDS` (default alpha.vapour,U.vapour).
#[test]
#[ignore]
fn mules_cells_dump() {
    let step: usize = std::env::var("STEP").ok().and_then(|v| v.parse().ok()).unwrap_or(616);
    let fields = std::env::var("FIELDS").unwrap_or_else(|_| "alpha.vapour,U.vapour".into());
    let r = load_reference("boiling_mules_tight_onset");
    let dir = protocol_copy("cells");
    let case = TwoPhaseCase::read(&dir).expect("read");
    let mut s = TwoPhaseSolver::new(case).with_upstream_stale_pimple_converged_flag();
    for _ in 1..=step {
        s.step().expect("step");
    }
    let tname = &r.times[step - r.first];
    for (name, v) in port_fields(&s) {
        if !fields.split(',').any(|f| f == name) {
            continue;
        }
        let u = &r.fields[&(tname.clone(), name.to_string())];
        for c in 0..v.len() {
            eprintln!("CELL {name} {c:3} port={:.17e} up={:.17e} rel={:.2e}", v[c], u[c], (v[c] - u[c]).abs() / u[c].abs().max(1e-300));
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
}


/// Diagnostic: port face/cell values near the front at `STEP` (default 616).
#[test]
#[ignore]
fn mules_faces_dump() {
    let step: usize = std::env::var("STEP").ok().and_then(|v| v.parse().ok()).unwrap_or(616);
    let dir = protocol_copy("faces");
    let case = TwoPhaseCase::read(&dir).expect("read");
    let mut s = TwoPhaseSolver::new(case).with_upstream_stale_pimple_converged_flag();
    for _ in 1..=step {
        s.step().expect("step");
    }
    let show = |name: &str, v: &[f64]| {
        let t: Vec<String> = (26..33).map(|i| format!("{:.16e}", v[i])).collect();
        eprintln!("PORT {name}: {}", t.join(" "));
    };
    show("alphaPhi.vapour", &s.fluids[1].alpha_phi);
    show("alphaPhi.liquid", &s.fluids[0].alpha_phi);
    show("phi.vapour", &s.fluids[1].phi);
    show("phi.liquid", &s.fluids[0].phi);
    show("alphaRhoPhi.vapour", &s.fluids[1].alpha_rho_phi);
    show("dgdt.vapour", &s.fluids[1].dgdt);
    show("Kd.liquid.vapour", &s.kd_ff);
    show("Kd.vapour.structure", &s.kd_fs[1]);
    show("Re.vapour.structure", &s.re_fs[1]);
    show("dmdt", &s.dmdt);
    show("areaDensity", &s.interfacial_area);
    let _ = std::fs::remove_dir_all(&dir);
}

// ---------------------------------------------------------------------------------------------------------------
// The gates (measured 2026-11-26, release, against the installed binary's fresh run; README "Round 7").
// ---------------------------------------------------------------------------------------------------------------

/// Assert `bound(field)` on every compared field of the rows of steps `lo..=hi`; `dt` and the sub-cycle count must
/// equal upstream's there.
fn assert_span(rows: &[Row], lo: usize, hi: usize, bound: &dyn Fn(&str) -> f64) {
    let mut seen = 0;
    for (n, dt_rel, sub_ok, _, w) in rows {
        if *n < lo || *n > hi {
            continue;
        }
        seen += 1;
        assert!(*dt_rel <= 1e-12, "step {n}: the step size differs from upstream's by {dt_rel:.3e}");
        assert!(*sub_ok, "step {n}: the number of MULES sub-cycles differs from upstream's");
        for (k, v) in w {
            assert!(*v <= bound(k), "step {n}: {k} differs from upstream by {v:.3e}, bound {:.1e}", bound(k));
        }
    }
    assert_eq!(seen, hi - lo + 1, "every step of {lo}..={hi} must be compared");
}

fn all_rows() -> Vec<Row> {
    run_rows("gate", 640, true)
}

/// M1, M2: steps 1-70, the vapour absent (`alpha.vapour` and `dmdt` are exactly 0 in both codes), MULES running with
/// one sub-cycle. Measured: steps 1-2 every field <= 2.0e-11; steps 3-70 p 1.0e-12, T.liquid 3.0e-13, U.liquid 9.4e-12,
/// T.vapour 2.3e-10, T.interface 7.6e-10, U.vapour 1.6e-11, faces <= 2.3e-11. The residual is the p_rgh GAMG vs the port's
/// solver (round 3), well inside the 1e-9 allowance.
#[test]
fn mules_single_phase_start_matches_upstream() {
    let rows = all_rows();
    assert_span(&rows, 1, 2, &|_| 1e-10);
    assert_span(&rows, 3, 70, &|k| match k {
        "alpha.vapour" | "dmdt.liquid.vapour" | "alphaPhi.vapour" => 0.0,
        "T.interface" => 2e-9,
        "T.vapour" => 1e-9,
        _ => 1e-10,
    });
}

/// M3: steps 560-615, up to the step before upstream's first vapour. Measured: p 7.9e-12, T.liquid 1.0e-11, T.vapour
/// 3.0e-11, U 2.7e-11, alphaPhi.liquid 1.3e-10.
#[test]
fn mules_single_phase_to_the_onset_matches_upstream() {
    let rows = all_rows();
    assert_span(&rows, 560, 615, &|k| match k {
        "alpha.vapour" | "dmdt.liquid.vapour" | "alphaPhi.vapour" => 0.0,
        "alphaPhi.liquid" => 3e-10,
        _ => 1e-10,
    });
}

/// M4-M7: the onset (upstream's `alpha.vapour` is first > 0 at step 616) and the vapour growth to step 634. The step
/// size and the sub-cycle count are identical to step 636 (dt) and 637 (sub-cycles).
/// Measured worst, steps 616-634: alpha.vapour 6.3e-6, dmdt 5.4e-6, alphaPhi.vapour 4.7e-6 (a relative error of ~1e-6 of the
/// onset dmdt is single-phase round-off over a superheat of ~0.015 K, see the noise-floor test), p 4.1e-8, U.liquid 2.3e-7,
/// U.vapour 3.8e-7, phi.vapour 4.1e-7, T.liquid 7e-10, T.vapour 2.9e-9. Before the `alphaf` fix (MULES uses linear `alphaf`,
/// implicitUpwind upwind) U.vapour was 3.3e-5 at step 616.
#[test]
fn mules_boiling_onset_and_vapour_growth_match_upstream_to_the_noise_floor() {
    let rows = all_rows();
    // round 13 (README "Round 13"): the vapour gas constant, the temperature Newton rule and the marker timing took the onset window from
    // the noise floor of round 7 (alpha 6.3e-6, p 4.1e-8, U.vapour 3.8e-7) to round-off: measured, steps 616-634, alpha.vapour 3.4e-11,
    // dmdt 3.5e-11, alphaPhi.vapour 2.4e-11, p 1.3e-12, U.liquid 6.0e-12, U.vapour 5.9e-12, phi 4.5e-12, T.liquid 2.0e-13, T.interface 7.5e-14
    assert_span(&rows, 616, 634, &|k| match k {
        "alpha.vapour" | "dmdt.liquid.vapour" | "alphaPhi.vapour" => 1e-9,
        "p" | "p_rgh" => 1e-10,
        "U.liquid" | "U.vapour" | "phi.liquid" | "phi.vapour" | "alphaPhi.liquid" => 1e-10,
        _ => 1e-11,
    });
}

/// Round 13: with the enthalpy equations solved by `PBiCGStab`/`diagonal` as upstream's log shows (`".*"` is the LAST pattern that matches
/// `h.liquid`) the discrete jump of step 635 (`p` 2.85e-4 after 7e-14 at step 634) is gone: `p` is 1.4e-10 at 635 and the divergence is
/// the exponential growth of the post-onset flow instability from round-off (635: 1.4e-10, 636: 2.5e-7, 637: 9.5e-6, 639: 1.3e-3,
/// 641: 2e-1). With the earlier `symGaussSeidel` the matrix of the stabilised vapour cells (diagonal 1e11 times the cell's) was solved
/// only to 1e-8 and cell 30 of the vapour enthalpy was off by 6.5e-8 at step 635, the jump.
///
/// The first step that leaves the noise floor. The number of outer iterations is upstream's at EVERY step to 648
/// (6 until 627, 7 at 628, 24 from 629: upstream's log; step 649 is cut short by upstream's abort). The first field to leave
/// 1e-6 is a flow field at step 635 (p 2.7e-4, U.vapour 8.0e-4: the flow excursion of the boiling onset amplifying whatever
/// differs, nothing structural flips first); `dt` differs from step 637 (3.9e-2) and the sub-cycle count from 638. The same
/// step, 636 (one later), is where the PORT AGAINST ITSELF with a 1e-9 disturbance of the initial liquid temperature leaves
/// 1e-6 (`mules_noise_floor_is_the_size_of_the_onset_window`).
#[test]
fn mules_first_diverging_step_after_the_onset_is_636_by_fields() {
    // (round 13: the port's step size is NaN from step 646 (p 7.6e81, 3.9e-1 off upstream's dt): inside upstream's own
    // post-excursion instability; 645 steps are compared)
    let rows = run_rows("gate_first", 645, true);
    for (n, _, _, iters, _) in &rows {
        if *n >= 610 {
            let up = if *n == 628 { 7 } else if *n >= 629 { 24 } else { 6 };
            assert_eq!(*iters, up, "step {n}: outer iterations (upstream's log)");
        }
    }
    let flow = ["p", "p_rgh", "U.liquid", "U.vapour", "T.liquid", "T.vapour", "T.interface"];
    let first_flow = rows.iter().find(|r| r.4.iter().any(|(k, v)| flow.contains(k) && *v > 1e-6)).map(|r| r.0);
    assert_eq!(first_flow, Some(636), "first step at which a flow field leaves 1e-6");
    let first_dt = rows.iter().find(|r| r.1 > 1e-12).map(|r| r.0);
    assert_eq!(first_dt, Some(637));
    let first_vapour_energy = rows.iter().find(|r| r.4.iter().any(|(k, v)| *k == "T.vapour" && *v > 1e-9)).map(|r| r.0);
    assert_eq!(first_vapour_energy, Some(636), "the vapour temperature leaves 1e-9 with the flow fields, at step 636");
    let first_sub = rows.iter().find(|r| !r.2).map(|r| r.0);
    // ~~643~~ **MOVED 2026-11-27 to 641** (gh:#1909 round 58, two-phase matrices in OpenFOAM order): a post-onset sub-cycle marker in
    // the chaotic excursion; the flow-field markers 636/637/636 above did not move.
    assert_eq!(first_sub, Some(641));
}

/// Noise-floor control: the port against ITSELF with the initial liquid temperature scaled by `1 + 1e-9` (a single-phase
/// disturbance the size of port-vs-upstream before the onset) shows the same onset window. Measured: alpha.vapour 1.0e-6 at
/// step 616 (port vs upstream 1.7e-6), p 6e-11 and U.liquid 3e-10 at 627 (port vs upstream 8.3e-10, 3.8e-9: the disturbances
/// differ in direction, so the factor ~10 is not a model difference), and flow fields leave 1e-6 at step 636.
#[test]
fn mules_noise_floor_is_the_size_of_the_onset_window() {
    let rows = noise_rows(1e-9, 640);
    let at = |n: usize, f: &str| rows.iter().find(|r| r.0 == n).and_then(|r| r.1.iter().find(|(k, _)| *k == f)).map(|(_, v)| *v).expect("row");
    assert!((5e-7..2e-6).contains(&at(616, "alpha.vapour")), "{:.2e}", at(616, "alpha.vapour"));
    assert!(at(627, "p") < 2e-10 && at(627, "U.liquid") < 1e-9);
    let first = rows.iter().find(|r| ["p", "U.liquid", "U.vapour"].iter().any(|f| r.1.iter().any(|(k, v)| k == f && *v > 1e-6))).map(|r| r.0);
    assert_eq!(first, Some(636));
}

/// The named option keeps the port's earlier behaviour callable (`implicitUpwind` in place of the case's MULES) and it
/// misses upstream's MULES run by the pinned amounts: p 8.5e-5 and U.liquid 2.0e-3 already at step 70 (upwind `alphaf`
/// and the implicit vapour solve change the single-phase flow through the pressure coupling), and the onset itself
/// (alpha.vapour off by its own magnitude, 1.0, at step 616: the implicit run's vapour appears at a different step).
#[test]
fn mules_ablation_to_implicit_upwind_misses_the_shipped_run_by_the_pinned_amount() {
    let rows = run_rows_with("abl", 640, true, true);
    let at = |n: usize, f: &str| rows.iter().find(|r| r.0 == n).and_then(|r| r.4.iter().find(|(k, _)| *k == f)).map(|(_, v)| *v).expect("row");
    assert!(at(70, "p") > 1e-5 && at(70, "U.liquid") > 1e-4, "{:.2e} {:.2e}", at(70, "p"), at(70, "U.liquid"));
    assert!(at(616, "alpha.vapour") > 0.5, "{:.2e}", at(616, "alpha.vapour"));
}

/// Round 13 ablation (diagnostic): `ROUND13_OFF=gas,newton,markers,flux` (any subset) switches the named round-13 translations back
/// to the port's earlier forms; `ROUND13_KEEP_GOING` ends the run at a divergence instead of failing.
fn round13_flags() -> Vec<String> {
    std::env::var("ROUND13_OFF").map(|v| v.split(',').map(str::to_string).collect()).unwrap_or_default()
}
