// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//! # GeN-Foam `onePhase` port vs a run of the real GeN-Foam binary (gh:#1361, round 4)
//!
//! AI-written, not human-reviewed. Research, education and V&V only.
//!
//! Methodology, predictions (written and committed BEFORE the first port run) and results:
//! `verification_and_validation/genfoam_vs_upstream_gh1361/README.md`, "Round 4". The reference is
//! gzipped text written by `scripts/extract_onephase_reference.sh` from a run of the unmodified
//! `GeN-Foam` binary (652b3da0cc55 on ESI v2506, serial) made by `scripts/run_case.sh` with
//! `scripts/onephase_protocol.sh` (`TIGHT=1`: every linear-solver tolerance 1e-14, relTol 0;
//! `writePrecision 17`; every step written) on `tests/data/genfoam_1d_onephase`. The protocol is
//! applied to a per-test copy by [`protocol_copy`], mirroring the script line for line.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use miniz_oxide::inflate::decompress_to_vec;
use outram_foam_appbuilder_lib::genfoam::thermal_hydraulics::solver::two_phase::one_phase_fc::{
    OnePhaseFcCase, OnePhaseFcSolver,
};

struct Reference {
    dt: Vec<f64>,
    cum_err: f64,
    fields: HashMap<(String, String), Vec<f64>>,
    times: Vec<String>,
}

fn load_reference(name: &str) -> Reference {
    // An absolute path is a scratch plain-text reference (diagnostics only).
    let text = if name.starts_with('/') {
        std::fs::read_to_string(name).expect("scratch reference present")
    } else {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("verification_and_validation/genfoam_vs_upstream_gh1361/refs")
            .join(format!("{name}.ref.txt.gz"));
        let bytes = std::fs::read(&path).expect("reference present");
        String::from_utf8(decompress_to_vec(&bytes[10..bytes.len() - 8]).expect("inflate")).expect("utf8")
    };
    let mut r = Reference { dt: vec![], cum_err: 0.0, fields: HashMap::new(), times: vec![] };
    for line in text.lines() {
        let mut w = line.split_whitespace();
        match w.next() {
            Some("DT") => r.dt = w.map(|x| x.parse().expect("dt")).collect(),
            Some("CUMERR") => r.cum_err = w.next().and_then(|x| x.parse().ok()).unwrap_or(0.0),
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

/// Copy of the vendored case with the tight protocol of `scripts/onephase_protocol.sh`
/// (`TIGHT=1`) applied; a per-test scratch directory.
fn protocol_copy(test: &str) -> PathBuf {
    let src = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/data/genfoam_1d_onephase");
    let dst = std::env::temp_dir().join(format!("gh1361_op_{test}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dst);
    copy_dir(&src, &dst);
    let cd = dst.join("system/controlDict");
    let text = std::fs::read_to_string(&cd).expect("controlDict");
    let text: Vec<String> = text
        .lines()
        .map(|l| {
            if l.starts_with("endTime ") {
                "endTime         1;".to_string()
            } else if l.starts_with("writePrecision ") {
                "writePrecision  17;".to_string()
            } else if l.starts_with("writeControl ") {
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

/// `max |a - b| / scale` with an explicit scale (`contErr` and `dgdt` are differences of large
/// cancelling terms, `rho alpha / dt` and `1 / dt`, so their own maximum is round-off noise).
fn worst_scaled(port: &[f64], up: &[f64], scale: f64) -> f64 {
    port.iter().zip(up).map(|(a, b)| (a - b).abs() / scale).fold(0.0, f64::max)
}

/// `max |a - b| / max |b|` (absolute when the reference is identically zero).
fn worst_rel(port: &[f64], up: &[f64]) -> f64 {
    assert_eq!(port.len(), up.len(), "reference field has the wrong length");
    let scale = up.iter().fold(0.0_f64, |m, x| m.max(x.abs()));
    port.iter()
        .zip(up)
        .map(|(a, b)| if scale == 0.0 { (a - b).abs() } else { (a - b).abs() / scale })
        .fold(0.0, f64::max)
}

fn port_fields(s: &OnePhaseFcSolver) -> Vec<(&'static str, Vec<f64>)> {
    vec![
        ("p", s.p.c.clone()),
        ("p_rgh", s.p_rgh.c.clone()),
        ("T", s.fluid.t.c.clone()),
        ("U", s.fluid.u.c.iter().map(|u| u.x).collect()),
        ("T.fixedPower", s.structure.t_act.clone()),
        ("htc", s.htc.clone()),
        ("Re", s.re.clone()),
        ("contErr", s.fluid.cont_err.clone()),
        ("dgdt", s.fluid.dgdt.clone()),
    ]
}

type Rows = Vec<(usize, f64, Vec<(&'static str, f64)>)>;

/// Per-field worst difference of the solver state against the reference time directory `tname`:
/// relative to the field's maximum, except `contErr` (scaled by `max(alpha rho)/dt`) and `dgdt` (by `1/dt`).
fn compare(s: &OnePhaseFcSolver, r: &Reference, tname: &str, dt_used: f64) -> Vec<(&'static str, f64)> {
    let rho_a = s.fluid.rho.c.iter().zip(&s.fluid.alpha.c).map(|(a, b)| a * b).fold(0.0_f64, f64::max);
    let mut worst = Vec::new();
    for (name, v) in port_fields(s) {
        if let Some(u) = r.fields.get(&(tname.to_string(), name.to_string())) {
            let w = match name {
                "contErr" => worst_scaled(&v, u, rho_a / dt_used),
                "dgdt" => worst_scaled(&v, u, 1.0 / dt_used),
                _ => worst_rel(&v, u),
            };
            worst.push((name, w));
        }
    }
    worst
}

fn run_steps_with(test: &str, n_steps: usize, ablation: fn(OnePhaseFcSolver) -> OnePhaseFcSolver) -> (Rows, OnePhaseFcSolver) {
    let r = load_reference(&std::env::var("ONEPHASE_DIAG_ALL").unwrap_or_else(|_| "onephase_tight_steps40".to_string()));
    let dir = protocol_copy(test);
    let case = OnePhaseFcCase::read(&dir).expect("read the onePhase channel");
    let mut s = ablation(OnePhaseFcSolver::new(case));
    let mut rows = Vec::new();
    for n in 1..=n_steps {
        let dt_used = s.dt;
        let rep = s.step().expect("step");
        let dt_up = if n == 1 { 1e-3 } else { r.dt[n - 2] };
        let dt_rel = (dt_used - dt_up).abs() / dt_up;
        assert!((rep.dt - dt_used).abs() < 1e-300);
        let tname = &r.times[n - 1];
        rows.push((n, dt_rel, compare(&s, &r, tname, dt_used)));
    }
    let _ = std::fs::remove_dir_all(&dir);
    (rows, s)
}

fn print_rows(rows: &Rows) {
    for (n, dt_rel, w) in rows {
        let cells: Vec<String> = w.iter().map(|(k, v)| format!("{k}={v:.2e}")).collect();
        eprintln!("step {n:3} dt_rel={dt_rel:.2e} {}", cells.join(" "));
    }
}

fn first_exceeding(rows: &Rows, tol: f64) -> Option<(usize, &'static str, f64)> {
    for (n, _, w) in rows {
        for (k, v) in w {
            if *v > tol {
                return Some((*n, *k, *v));
            }
        }
    }
    None
}

/// Round 4: the first 40 steps at the tight protocol, every step, every written field.
///
/// Measured 2026-11-12 (README, Round 4): the step size is bit-identical at every step; every field
/// agrees to 4.4e-14 (p), 2.8e-12 (U), 4.3e-14 (T), 5.3e-14 (`T.fixedPower`) of its maximum over the
/// 40 steps; `contErr` and `dgdt`, scaled as in [`compare`], to 1e-9 and 1e-9. The bounds are one to
/// two orders above the measurements, set before this run from the predictions O1 and O2 (1e-10, 1e-9).
#[test]
fn one_phase_channel_first_steps_tight_vs_upstream() {
    let (rows, s) = run_steps_with("steps", 40, |s| s);
    print_rows(&rows);
    for (n, dt_rel, w) in &rows {
        assert!(*dt_rel == 0.0, "step {n}: the step size differs from upstream's by {dt_rel:.3e}");
        let bound = if *n == 1 { 1e-10 } else { 1e-9 };
        for (k, v) in w {
            assert!(*v <= bound, "step {n}: {k} differs from upstream by {v:.3e}, bound {bound:.1e}");
        }
    }
    assert!((s.time - 0.3184966026895361).abs() < 1e-15);
}

/// The whole 1 s run (108 steps): the last written state and the cumulative continuity error.
#[test]
fn one_phase_channel_full_second_vs_upstream() {
    let r = load_reference("onephase_tight_steps40");
    let dir = protocol_copy("full");
    let mut s = OnePhaseFcSolver::from_case_dir(&dir).expect("read");
    let _ = std::fs::remove_dir_all(&dir);
    let mut n = 0;
    let mut dt_used = 0.0;
    while s.running() {
        dt_used = s.dt;
        s.step().expect("step");
        n += 1;
    }
    assert_eq!(n, 108, "upstream took 108 steps to 1 s");
    let last = r.times.last().expect("last time").clone();
    let w = compare(&s, &r, &last, dt_used);
    for (k, v) in &w {
        eprintln!("step 108 {k}: {v:.3e}");
        assert!(*v <= 1e-9, "{k} differs from upstream by {v:.3e}");
    }
    // Measured 2026-11-12: port -8.750609708659356e-6, upstream -8.750922625618369e-6 kg/m3, 3.1e-10
    // apart (3.6e-5 relative). The sum is over the 20 steps whose iteration is flagged converged of
    // `contErr V dt / V`, each `contErr` a difference of `alpha rho / dt` ~ 4e5 terms that agrees to 1e-13
    // of that scale (round-off), so the absolute bound, not the relative one, is the meaningful one.
    let diff = (s.fluid.cumul_cont_err - r.cum_err).abs();
    eprintln!("cumulative continuity error port {:e} upstream {:e} abs diff {diff:.2e}", s.fluid.cumul_cont_err, r.cum_err);
    assert!(diff < 1e-9, "cumulative continuity error differs by {diff:.2e} kg/m3");
    assert!(diff / r.cum_err.abs() < 1e-3);
}

/// The named options keep the earlier or clean forms callable, and each misses upstream by what the
/// README records (measured 2026-11-12).
#[test]
fn one_phase_channel_options_reproduce_the_recorded_misses() {
    let worst = |name: &str, f: fn(OnePhaseFcSolver) -> OnePhaseFcSolver, field: &str| -> f64 {
        let (rows, _) = run_steps_with(name, 1, f);
        rows[0].2.iter().find(|(k, _)| *k == field).expect("field").1
    };
    // `fvMatrix::relax(1)`: boundary inflow makes it NOT neutral: p off by 1.5e-5 without it.
    let p = worst("o_relax", |s| s.without_equation_relaxation(), "p");
    assert!(p > 1e-6 && p < 1e-4, "no relax: p {p:.3e}");
    // lazy `p.oldTime()` and structure old time: p off by 1.2e-2.
    let p = worst("o_lazy", |s| s.without_first_step_lazy_old_time(), "p");
    assert!(p > 1e-3 && p < 1e-1, "no lazy old time: p {p:.3e}");
    // kinetic-energy term: its net source is tiny here (uniform U): p off by 7e-11.
    let p = worst("o_k", |s| s.without_kinetic_energy(), "p");
    assert!(p > 1e-12 && p < 1e-8, "no kinetic energy: p {p:.3e}");
    // structure with the post-solve temperature: p off by 3.5e-9, T.fixedPower by 6.7e-11.
    let p = worst("o_t", |s| s.with_structure_after_solve_temperature(), "p");
    assert!(p > 1e-10 && p < 1e-6, "post-solve structure T: p {p:.3e}");
    // the clean PIMPLE ending solves the steps upstream skips: the step-108 state differs.
    let r = load_reference("onephase_tight_steps40");
    let dir = protocol_copy("o_flag");
    let mut s = OnePhaseFcSolver::new(OnePhaseFcCase::read(&dir).expect("read")).without_stale_pimple_converged_flag();
    let _ = std::fs::remove_dir_all(&dir);
    let mut dt_used = 0.0;
    while s.running() {
        dt_used = s.dt;
        s.step().expect("step");
    }
    let last = r.times.last().expect("last").clone();
    let w = compare(&s, &r, &last, dt_used);
    let t = w.iter().find(|(k, _)| *k == "T").expect("T").1;
    assert!(t > 1e-5, "without the stale flag T differs from upstream by {t:.3e}");
}

/// Ablations of the named options, steps 1 and 2 (numbers go to the README).
#[test]
fn one_phase_channel_ablations_step_1_and_2() {
    let variants: [(&str, fn(OnePhaseFcSolver) -> OnePhaseFcSolver); 5] = [
        ("default", |s| s),
        ("no_relax", |s| s.without_equation_relaxation()),
        ("no_lazy", |s| s.without_first_step_lazy_old_time()),
        ("no_K", |s| s.without_kinetic_energy()),
        ("t_after", |s| s.with_structure_after_solve_temperature()),
    ];
    for (name, f) in variants {
        let (rows, _) = run_steps_with(name, 2, f);
        eprintln!("== {name}");
        print_rows(&rows);
    }
}

/// Diagnostic (ignored): step 1 after `ONEPHASE_DIAG_NOUTER` outer iterations against the scratch
/// reference `ONEPHASE_DIAG_REF` (plain text from `extract_onephase_reference.sh`), per-cell values.
#[test]
#[ignore = "needs ONEPHASE_DIAG_REF and ONEPHASE_DIAG_NOUTER"]
fn one_phase_diag_step_1_outer_iterations() {
    let (Ok(rf), Ok(k)) = (std::env::var("ONEPHASE_DIAG_REF"), std::env::var("ONEPHASE_DIAG_NOUTER")) else {
        return;
    };
    let r = load_reference(&rf);
    let dir = protocol_copy("diag");
    let case = OnePhaseFcCase::read(&dir).expect("read");
    let mut s = OnePhaseFcSolver::new(case);
    s.controls.n_outer = k.parse().expect("n");
    if let Ok(c) = std::env::var("ONEPHASE_DIAG_NCORR") {
        s.controls.n_corr = c.parse().expect("ncorr");
    }
    s.step().expect("step");
    let tname = &r.times[0];
    for (name, v) in port_fields(&s) {
        if let Some(u) = r.fields.get(&(tname.clone(), name.to_string())) {
            eprintln!("{name}: worst {:.3e}", worst_rel(&v, u));
            for c in [0usize, 1, 9, 10, 11, 20, 29, 30, 38, 39] {
                eprintln!("   cell {c:2} port {:.15e} up {:.15e}", v[c], u[c]);
            }
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// Diagnostic (ignored): all 108 steps against the scratch reference in `ONEPHASE_DIAG_ALL`.
#[test]
#[ignore = "needs ONEPHASE_DIAG_ALL"]
fn one_phase_diag_all_steps() {
    if std::env::var("ONEPHASE_DIAG_ALL").is_err() {
        return;
    }
    let (rows, _) = run_steps_with("all", 108, |s| s);
    print_rows(&rows);
}
