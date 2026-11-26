// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//! # GeN-Foam `porousKEpsilon2PhaseCorrected` vs the real binary (gh:#1361 round 16, gh:#1802)
//!
//! AI-written, not human-reviewed. Research, education and V&V only.
//!
//! The case is `featureCases/1D_PSBT_SC/PhaseII_Ex1_01_5215` AS SHIPPED (`tests/data/genfoam_psbt2_ex1_gh1802`): RAS
//! `porousKEpsilon2PhaseCorrected` on both fluids, `symmetry` lateral patch, through-flow boundary conditions. Methodology,
//! predictions (committed before the first port run) and results: `verification_and_validation/genfoam_vs_upstream_gh1361/README.md`,
//! "Round 16". Protocol: `scripts/psbt_shipped_protocol.sh` (EVERY_STEP=1, TIGHT=1), mirrored by [`protocol_copy`].
//! Reference: `refs/psbt2_ex1_head.ref.txt.gz`, a fresh run of the installed binary.

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
    protocol_copy_of(test, "genfoam_psbt2_ex1_gh1802")
}

/// [`protocol_copy`] for the vendored case directory `data` under `tests/data`.
fn protocol_copy_of(test: &str, data: &str) -> PathBuf {
    let src = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/data").join(data);
    static COUNTER: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let k = COUNTER.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    let dst = std::env::temp_dir().join(format!("gh1802_psbt2_{test}_{}_{k}", std::process::id()));
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
    let t = |k: usize| s.fluids[k].turbulence.as_ref().expect("RAS");
    vec![
        ("p", s.p.c.clone()),
        ("p_rgh", s.p_rgh.c.clone()),
        ("T.liquid", s.fluids[0].t.c.clone()),
        ("T.vapour", s.fluids[1].t.c.clone()),
        ("T.fixedPower", s.structure.t_act.clone()),
        ("U.liquid", s.fluids[0].u.c.iter().map(|u| u.x).collect()),
        ("U.vapour", s.fluids[1].u.c.iter().map(|u| u.x).collect()),
        ("alpha.vapour", s.fluids[1].alpha.c.clone()),
        ("Kd.liquid.structure", s.kd_fs[0].clone()),
        ("dgdt.liquid", s.fluids[0].dgdt.clone()),
        ("contErr.liquid", s.fluids[0].cont_err.clone()),
        ("Re.liquid.structure", s.re_fs[0].clone()),
        ("htc.liquid.structure", s.htc_fs[0].clone()),
        ("k.liquid", t(0).k.c.clone()),
        ("k.vapour", t(1).k.c.clone()),
        ("epsilon.liquid", t(0).epsilon.c.clone()),
        ("epsilon.vapour", t(1).epsilon.c.clone()),
        ("nut.liquid", t(0).nut.c.clone()),
        ("nut.vapour", t(1).nut.c.clone()),
        ("alphat.liquid", t(0).alphat.c.clone()),
        ("alphat.vapour", t(1).alphat.c.clone()),
    ]
}

/// (step, relative step-size difference, port outer iterations, worst relative difference per field).
type Row = (usize, f64, usize, Vec<(&'static str, f64)>);

fn run_rows(test: &str, refs: &[&str], n_steps: usize, ablate: &str) -> Vec<Row> {
    run_rows_of(test, "genfoam_psbt2_ex1_gh1802", 1e-3, refs, n_steps, ablate)
}

/// [`run_rows`] on the vendored case `data` whose first step is `dt0`.
fn run_rows_of(test: &str, data: &str, dt0: f64, refs: &[&str], n_steps: usize, ablate: &str) -> Vec<Row> {
    let refs: Vec<Reference> = refs.iter().map(|r| load_reference(r)).collect();
    let dir = protocol_copy_of(test, data);
    let mut case = TwoPhaseCase::read(&dir).expect("read the SHIPPED PhaseII case");
    if ablate == "off" {
        for f in case.fluids.iter_mut() {
            if let Some(t) = f.turbulence.as_mut() {
                t.turbulence = false;
            }
        }
    }
    if ablate == "nopfile" {
        case = case.without_p_patch_from_file();
    }
    if ablate == "norelax" {
        for f in case.fluids.iter_mut() {
            f.turbulence = f.turbulence.take().map(|t| t.without_equation_relaxation());
        }
    }
    let mut s = TwoPhaseSolver::new(case).with_upstream_stale_pimple_converged_flag();
    if ablate == "nolazy" {
        s = s.without_first_step_lazy_old_time();
    }
    let mut rows = Vec::new();
    for n in 1..=n_steps {
        let rep = s.step().expect("step");
        let Some(r) = refs.iter().find(|r| n >= r.first && n - r.first < r.times.len()) else { continue };
        let dt_up = if n == 1 { dt0 } else { r.dt[n - 2] };
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

/// Diagnostic: print every compared step (`--ignored --nocapture`); `STEPS`, `ABLATE=norelax`.
#[test]
#[ignore]
fn psbt2_dump_rows() {
    let n: usize = std::env::var("STEPS").ok().and_then(|v| v.parse().ok()).unwrap_or(19);
    let ab = std::env::var("ABLATE").unwrap_or_default();
    let r = std::env::var("REFS").unwrap_or_else(|_| "psbt2_ex1_head".into());
    let data = std::env::var("DATA").unwrap_or_else(|_| "genfoam_psbt2_ex1_gh1802".into());
    let dt0: f64 = std::env::var("DT0").ok().and_then(|v| v.parse().ok()).unwrap_or(1e-3);
    print_rows(&run_rows_of("dump", &data, dt0, &[r.as_str()], n, &ab));
}

/// Diagnostic: the cells in which `FIELD` of the port differs from the reference `REFS` most at step `STEP` (default 1), with
/// their centres; `ABLATE`, `TOP` cells (default 12).
#[test]
#[ignore]
fn psbt2_cell_dump() {
    let step: usize = std::env::var("STEP").ok().and_then(|v| v.parse().ok()).unwrap_or(1);
    let field = std::env::var("FIELD").unwrap_or_else(|_| "p".into());
    let top: usize = std::env::var("TOP").ok().and_then(|v| v.parse().ok()).unwrap_or(12);
    let refname = std::env::var("REFS").unwrap_or_else(|_| "psbt2_ex1_head".into());
    let ablate = std::env::var("ABLATE").unwrap_or_default();
    let r = load_reference(&refname);
    let data = std::env::var("DATA").unwrap_or_else(|_| "genfoam_psbt2_ex1_gh1802".into());
    let dir = protocol_copy_of("cells", &data);
    let mut case = TwoPhaseCase::read(&dir).expect("read");
    if ablate == "off" {
        for f in case.fluids.iter_mut() {
            if let Some(t) = f.turbulence.as_mut() {
                t.turbulence = false;
            }
        }
    }
    let mut s = TwoPhaseSolver::new(case).with_upstream_stale_pimple_converged_flag();
    for _ in 1..=step {
        s.step().expect("step");
    }
    let t = &r.times[step - r.first];
    let up = &r.fields[&(t.clone(), field.clone())];
    let port = port_fields(&s).into_iter().find(|(k, _)| *k == field).expect("field").1;
    let scale = up.iter().fold(0.0_f64, |m, x| m.max(x.abs()));
    let mut idx: Vec<usize> = (0..up.len()).collect();
    idx.sort_by(|a, b| (port[*b] - up[*b]).abs().partial_cmp(&(port[*a] - up[*a]).abs()).expect("finite"));
    if std::env::var("LINE").is_ok() {
        let mut cs: Vec<usize> = (0..up.len()).filter(|&c| { let x = s.geom.mesh.cell_centres[c]; (x.y - 0.0261).abs() < 1e-3 && (x.z - 0.0261).abs() < 1e-3 }).collect();
        cs.sort_by(|a, b| s.geom.mesh.cell_centres[*a].x.partial_cmp(&s.geom.mesh.cell_centres[*b].x).expect("finite"));
        for c in cs { eprintln!("LINE {:.4} port-up={:.4e} p={:.8e}", s.geom.mesh.cell_centres[c].x, port[c] - up[c], up[c]); }
    }
    for &c in idx.iter().take(top) {
        let x = s.geom.mesh.cell_centres[c];
        eprintln!("CELL {c} ({:.4},{:.4},{:.4}) {field} port={:.12e} up={:.12e} rel={:.2e}", x.x, x.y, x.z, port[c], up[c], (port[c] - up[c]).abs() / scale);
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// The tutorial's own `regressionResults` check (`Alltest`: `alpha.vapour` and `T.fixedPower` of the cell on line 418 of the
/// t = 5 s files, i.e. cell 394, against 0.123835 and 620.178 within 1 %), run on the port with the SHIPPED settings. Diagnostic
/// (216 s wall, release, shared machine). Measured 2026-11-26: 1065 steps, `alpha.vapour` 0.124575 (+0.60 %), `T.fixedPower`
/// 620.1682 K (-0.0016 %): both inside the tutorial's own 1 % criterion. A separate check from the step-by-step comparison.
#[test]
fn psbt2_regression_results_at_5_s() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/data/genfoam_psbt2_ex1_gh1802");
    let case = TwoPhaseCase::read(&dir).expect("read");
    let t_end: f64 = std::env::var("T_END").ok().and_then(|v| v.parse().ok()).unwrap_or(5.0);
    let mut s = TwoPhaseSolver::new(case);
    let start = std::time::Instant::now();
    let mut n = 0usize;
    while s.time < t_end - 1e-9 {
        s.step().expect("step");
        n += 1;
        if n % 100 == 0 {
            let a = s.fluids[1].alpha.c.iter().fold(0.0_f64, |m, x| m.max(*x));
            eprintln!("REGR step {n} t={:.5} dt={:.3e} alpha_max={a:.6} wall={:.0}s", s.time, s.dt, start.elapsed().as_secs_f64());
        }
    }
    let a = s.fluids[1].alpha.c.iter().fold(0.0_f64, |m, x| m.max(*x));
    eprintln!(
        "REGR DONE steps={n} t={:.6} alpha[394]={:.6} (ref 0.123835) T.fixedPower[394]={:.4} (ref 620.178) alpha_max={a:.6} wall={:.0}s",
        s.time,
        s.fluids[1].alpha.c[394],
        s.structure.t_act[394],
        start.elapsed().as_secs_f64()
    );
    assert!((s.fluids[1].alpha.c[394] / 0.123835 - 1.0).abs() < 0.01, "alpha.vapour {}", s.fluids[1].alpha.c[394]);
    assert!((s.structure.t_act[394] / 620.178 - 1.0).abs() < 0.01, "T.fixedPower {}", s.structure.t_act[394]);
}

/// The control of README round 16: with `turbulence off` in BOTH codes (`refs/psbt2_ex1_off.ref.txt.gz`, `scripts/psbt2_pcg_protocol.sh`
/// with `TURB_OFF=1`) the port still differs from upstream by the same flow baseline (`p` 3.8e-6 and `U.liquid` 3.8e-5 at step 8, measured),
/// i.e. that baseline is not the turbulence model's.
#[test]
fn control_without_turbulence_has_the_same_flow_baseline() {
    let rows = run_rows("ctl", &["psbt2_ex1_off"], 8, "off");
    let worst = |f: &str| rows.iter().flat_map(|r| r.3.iter().filter(|(k, _)| *k == f).map(|(_, v)| *v)).fold(0.0, f64::max);
    assert!(worst("p") <= 6e-6 && worst("p") >= 1e-6, "p {:e}", worst("p"));
    assert!(worst("U.liquid") <= 6e-5 && worst("U.liquid") >= 1e-5, "U {:e}", worst("U.liquid"));
    assert!(worst("T.liquid") <= 6e-9, "T.liquid {:e}", worst("T.liquid"));
}

/// Which of the three shipped PhaseII cases `TwoPhaseCase::read` accepts (README round 16, X1); prints the refusal otherwise.
#[test]
fn phase_ii_cases_read_or_refuse_by_name() {
    let base = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/data");
    for name in ["genfoam_psbt2_ex1_gh1802", "genfoam_psbt2_ex2_gh1802", "genfoam_psbt2_ex3_gh1802"] {
        match TwoPhaseCase::read(&base.join(name)) {
            Ok(c) => {
                assert!(c.deviations.is_empty(), "{name}: {:?}", c.deviations);
                assert!(c.fluids.iter().all(|f| f.turbulence.is_some()), "{name}: RAS on both fluids");
                eprintln!("READ {name}: ok");
            }
            Err(e) => eprintln!("READ {name}: REFUSED {e:?}"),
        }
    }
}

/// A copy of the vendored Ex1 case with `edit` applied to `rel` (a text file under the case).
fn edited_case(test: &str, rel: &str, from: &str, to: &str) -> PathBuf {
    let src = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/data/genfoam_psbt2_ex1_gh1802");
    let dst = std::env::temp_dir().join(format!("gh1802_edit_{test}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dst);
    copy_dir(&src, &dst);
    let f = dst.join(rel);
    let text = std::fs::read_to_string(&f).expect("read");
    assert!(text.contains(from), "{rel} has no `{from}`");
    std::fs::write(&f, text.replacen(from, to, 1)).expect("write");
    dst
}

/// Refusals by name (README round 16, X1): another RAS model, `DhStruct` (nutStabilization), a wall function on `nut`, and an
/// unported linear solver for `k`.
#[test]
fn unported_turbulence_selections_are_refused_by_name() {
    let tp = "constant/fluidRegion/turbulenceProperties.liquid";
    let d = edited_case("kEps", tp, "RASModel        porousKEpsilon2PhaseCorrected;", "RASModel        kEpsilon;");
    let e = format!("{:?}", TwoPhaseCase::read(&d).err().expect("refused"));
    assert!(e.contains("RASModel `kEpsilon` is not ported"), "{e}");
    let d = edited_case("dh", tp, "turbulenceLengthScaleCoeff  0.07;", "turbulenceLengthScaleCoeff  0.07; DhStruct 0.01;");
    let e = format!("{:?}", TwoPhaseCase::read(&d).err().expect("refused"));
    assert!(e.contains("DhStruct") && e.contains("not ported"), "{e}");
    let d = edited_case("wf", "0/fluidRegion/nut.liquid", "type            zeroGradient;", "type            nutkWallFunction;");
    let e = format!("{:?}", TwoPhaseCase::read(&d).err().expect("refused"));
    assert!(e.contains("nutkWallFunction") && e.contains("not ported"), "{e}");
    let d = edited_case("lam", tp, "simulationType  RAS;", "simulationType  LES;");
    let e = format!("{:?}", TwoPhaseCase::read(&d).err().expect("refused"));
    assert!(e.contains("simulationType `LES`"), "{e}");
    for p in ["kEps", "dh", "wf", "lam"] {
        let _ = std::fs::remove_dir_all(std::env::temp_dir().join(format!("gh1802_edit_{p}_{}", std::process::id())));
    }
}

/// `fvMatrix::relax(1)` is a no-op on every equation of the first steps of this case (README round 16, X7): the fields with and without
/// it agree to 1e-12 (the matrices are already diagonally dominant).
#[test]
fn equation_relaxation_changes_nothing_in_the_first_steps() {
    let run = |ablate: &str| -> TwoPhaseSolver {
        let dir = protocol_copy("relax");
        let mut case = TwoPhaseCase::read(&dir).expect("read");
        if ablate == "norelax" {
            for f in case.fluids.iter_mut() {
                f.turbulence = f.turbulence.take().map(|t| t.without_equation_relaxation());
            }
        }
        let mut s = TwoPhaseSolver::new(case).with_upstream_stale_pimple_converged_flag();
        for _ in 0..4 {
            s.step().expect("step");
        }
        let _ = std::fs::remove_dir_all(&dir);
        s
    };
    let (a, b) = (run(""), run("norelax"));
    for (name, x) in port_fields(&a) {
        let y = port_fields(&b).into_iter().find(|(k, _)| *k == name).expect("field").1;
        assert!(worst_rel(&x, &y) <= 1e-12, "{name}: {:e}", worst_rel(&x, &y));
    }
}

/// Production path (`G`, the dissipation and the compressibility sinks) is switched on only where the structure fraction is <= 0.
/// The shipped cases have structure in every cell, so one region's `volumeFraction` is set to 0: the model must run, stay finite and
/// make `G` non-zero somewhere after the first step (`nut` is 0 until the first `correct`).
#[test]
fn structure_free_cells_switch_the_production_terms_on() {
    let dir = edited_case("nostruct", "constant/fluidRegion/phaseProperties", "volumeFraction  0.3371328621;", "volumeFraction  0;");
    let mut s = TwoPhaseSolver::new(TwoPhaseCase::read(&dir).expect("read")).with_upstream_stale_pimple_converged_flag();
    for _ in 0..2 {
        s.step().expect("step");
    }
    let t = s.fluids[0].turbulence.as_ref().expect("RAS");
    assert!(t.k.c.iter().chain(&t.epsilon.c).all(|v| v.is_finite() && *v > 0.0));
    assert!(t.production.iter().any(|g| *g > 0.0), "G must be active in the structure-free cells");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Round 16 gate (README "Round 16"): `PhaseII_Ex1_01_5215` AS SHIPPED with RAS `porousKEpsilon2PhaseCorrected` against the real binary,
/// 25 tight-tolerance steps (single phase). Step sizes and the outer-iteration counts are upstream's. The field bounds are the MEASURED
/// worst of the 25 steps with about 1.5x margin; they are NOT round-off level because the flow itself differs from upstream by 7e-7
/// (`p`) at step 1 and growing, with `turbulence off` as well (the control run), i.e. independently of the turbulence model; the
/// turbulence solves of the first outer iteration agree with upstream's to the printed residuals (README "Round 16", open issue).
/// The ablation `turbulence off` misses upstream by 40x more in `U.liquid` and by O(1) in `k`.
#[test]
fn phase_ii_ex1_turbulence_agrees_with_upstream_at_the_flow_baseline_and_the_ablation_misses() {
    let on = run_rows("on", &["psbt2_ex1_head"], 25, "");
    let up_iters = [10, 9, 10, 9, 10, 9, 10, 9, 10, 9, 10, 9, 10, 9, 10, 9, 10, 9, 10, 9, 9, 9, 9, 9, 9];
    assert_eq!(on.iter().map(|r| r.2).collect::<Vec<_>>(), up_iters, "outer iterations per step");
    let limits = [
        ("p", 4e-5),
        ("p_rgh", 4e-5),
        ("T.liquid", 5e-6),
        ("T.vapour", 1.5e-5),
        ("T.fixedPower", 3e-5),
        ("U.liquid", 2e-3),
        ("U.vapour", 2e-3),
        ("alpha.vapour", 0.0),
        ("dgdt.liquid", 1e-3),
        ("htc.liquid.structure", 2e-3),
        ("k.liquid", 3e-3),
        ("epsilon.liquid", 4e-3),
        ("nut.liquid", 2e-3),
        ("alphat.liquid", 2e-3),
        ("k.vapour", 8e-2),
        ("epsilon.vapour", 6e-2),
        ("nut.vapour", 9e-2),
        ("alphat.vapour", 9e-2),
    ];
    for (n, dt_rel, _, w) in &on {
        assert!(*dt_rel < 1e-12, "step {n}: dt differs by {dt_rel:e}");
        for (k, v) in w {
            if let Some((_, lim)) = limits.iter().find(|(name, _)| name == k) {
                assert!(*v <= *lim, "step {n} {k}: {v:e} exceeds {lim:e}");
            }
        }
    }
    let off = run_rows("off", &["psbt2_ex1_head"], 25, "off");
    let at = |rows: &[Row], f: &str| rows.last().and_then(|r| r.3.iter().find(|(k, _)| *k == f)).map(|(_, v)| *v).expect("field");
    assert!(at(&off, "U.liquid") > 10.0 * at(&on, "U.liquid"), "turbulence off must miss U.liquid by far more");
    assert!(at(&off, "k.liquid") > 0.5, "with turbulence off k stays at its initial value: {}", at(&off, "k.liquid"));
}

/// `PhaseII_Ex2_04_6770` and `PhaseII_Ex3_11-0312_power_increase` AS SHIPPED (TRACECHF + lookUpTableCHF + VapourTRACE, `powerDensity` from
/// `0/fluidRegion/powerDensity.fixedPower`, `symmetry`-like `frontAndright` patch) against the real binary, 5 tight steps each (first step 5e-3 s;
/// `refs/psbt2_ex{2,3}_head.ref.txt.gz`). Step sizes equal upstream's. Ex3's step 1 runs ONE outer iteration, so the turbulence solves see the
/// initial fields only: `k`, `epsilon`, `nut`, `alphat` of both fluids agree with upstream to round-off (<= 1e-12; measured <= 6e-16) while `p`
/// (1.9e-5) and `U` (7.8e-5) already differ: the flow baseline of README round 16, not the turbulence model. Later steps are bounded by the
/// measured worst (about 1.5x margin).
#[test]
fn phase_ii_ex2_and_ex3_run_and_agree_at_the_flow_baseline() {
    for (name, data, dt0, p_lim, u_lim) in [
        ("psbt2_ex2_head", "genfoam_psbt2_ex2_gh1802", 5e-3, 6e-5, 1.2e-3),
        ("psbt2_ex3_head", "genfoam_psbt2_ex3_gh1802", 5e-3, 2.5e-5, 6e-4),
    ] {
        let rows = run_rows_of(name, data, dt0, &[name], 5, "");
        assert_eq!(rows.len(), 5, "{name}");
        for (n, dt_rel, _, w) in &rows {
            assert!(*dt_rel < 1e-12, "{name} step {n}: dt differs by {dt_rel:e}");
            for (k, v) in w {
                let lim = match *k {
                    "p" | "p_rgh" => p_lim,
                    "U.liquid" | "U.vapour" => u_lim,
                    "alpha.vapour" => 0.0,
                    "T.liquid" | "T.fixedPower" => 2.5e-6,
                    _ => continue,
                };
                assert!(*v <= lim, "{name} step {n} {k}: {v:e} exceeds {lim:e}");
            }
        }
        if name == "psbt2_ex3_head" {
            for (k, v) in &rows[0].3 {
                if matches!(*k, "k.liquid" | "k.vapour" | "epsilon.liquid" | "epsilon.vapour" | "nut.liquid" | "nut.vapour" | "alphat.liquid" | "alphat.vapour") {
                    assert!(*v <= 1e-12, "Ex3 step 1 {k}: {v:e}");
                }
            }
        }
    }
}
