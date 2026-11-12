// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//! # GeN-Foam `twoPhase` port vs a run of the real GeN-Foam binary on `1D_boiling` (gh:#1361, round 3; #1239)
//!
//! AI-written, not human-reviewed. Research, education and V&V only.
//!
//! Methodology, predictions (written and committed BEFORE the first port run)
//! and results: `verification_and_validation/genfoam_vs_upstream_gh1361/README.md`,
//! "Round 3". References are gzipped text written by
//! `scripts/extract_boiling_reference.sh` from runs of the unmodified
//! `GeN-Foam` binary (652b3da0cc55 on ESI v2506, serial) made by
//! `scripts/run_tutorial.sh` with `scripts/boiling_protocol.sh`:
//! `alpha { solver implicitUpwind; solvePhase vapour; }` in both codes,
//! `writePrecision 17`; `tight` also sets every linear-solver tolerance in the
//! `solvers` block to 1e-14 with `relTol 0` (residualControl untouched).
//!
//! The case files are upstream's, vendored unchanged in
//! `tests/data/genfoam_1d_boiling`; the protocol is applied to a per-test copy
//! by [`protocol_copy`], mirroring the script line for line.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use miniz_oxide::inflate::decompress_to_vec;
use outram_foam_appbuilder_lib::genfoam::thermal_hydraulics::solver::two_phase::{
    TwoPhaseCase, TwoPhaseSolver,
};

/// One parsed reference file.
struct Reference {
    /// Printed `deltaT` after each step (the NEXT step's size).
    dt: Vec<f64>,
    /// `(liquid, vapour)` cumulative continuity errors [kg/m3] at the end.
    cum_err: (f64, f64),
    /// `(time directory name, field name) -> 40 cell values`.
    fields: HashMap<(String, String), Vec<f64>>,
    /// Time directory names in file order.
    times: Vec<String>,
}

fn load_reference(name: &str) -> Reference {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("verification_and_validation/genfoam_vs_upstream_gh1361/refs")
        .join(format!("{name}.ref.txt.gz"));
    let bytes = std::fs::read(&path).expect("reference present");
    // gzip -n: 10-byte header, 8-byte trailer.
    let text = String::from_utf8(decompress_to_vec(&bytes[10..bytes.len() - 8]).expect("inflate"))
        .expect("utf8");
    let mut r = Reference { dt: vec![], cum_err: (0.0, 0.0), fields: HashMap::new(), times: vec![] };
    for line in text.lines() {
        let mut w = line.split_whitespace();
        match w.next() {
            Some("DT") => r.dt = w.map(|x| x.parse().expect("dt")).collect(),
            Some("CUMERR") => {
                let v: Vec<f64> = w.map(|x| x.parse().expect("err")).collect();
                r.cum_err = (v[0], v[1]);
            }
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

/// Copy of the vendored case with the comparison protocol applied (the same
/// edits as `scripts/boiling_protocol.sh`). `tight`: every tolerance inside the
/// `solvers` block becomes 1e-14 and every `relTol` 0.
fn protocol_copy(test: &str, tight: bool) -> PathBuf {
    let src = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/data/genfoam_1d_boiling");
    let dst = std::env::temp_dir().join(format!("gh1361_{test}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dst);
    copy_dir(&src, &dst);
    let f = dst.join("system/fluidRegion/fvSolution");
    let text = std::fs::read_to_string(&f).expect("fvSolution");
    let text = text.replace(
        "solver          \"MULES\";",
        "solver          implicitUpwind; solvePhase vapour;",
    );
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
        if t.starts_with("adjustSubCycles") || t.starts_with("alphaMaxCo") {
            continue;
        }
        if tight && in_solvers && t.starts_with("tolerance") {
            out.push_str("        tolerance       1e-14;\n");
        } else if tight && in_solvers && t.starts_with("relTol") {
            out.push_str("        relTol          0;\n");
        } else {
            out.push_str(line);
            out.push('\n');
        }
    }
    std::fs::write(&f, out).expect("write fvSolution");
    dst
}

/// Largest `|a - b| / max(|b|, floor)` over a field, floor = 1e-300 so a
/// reference zero compares exactly.
fn worst_rel(port: &[f64], up: &[f64]) -> f64 {
    let scale = up.iter().fold(0.0_f64, |m, x| m.max(x.abs()));
    port.iter()
        .zip(up)
        .map(|(a, b)| if scale == 0.0 { (a - b).abs() } else { (a - b).abs() / scale })
        .fold(0.0, f64::max)
}

/// The compared quantities of one solver state, keyed by the reference's field names.
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
    ]
}

/// Step-by-step comparison of the first `n_steps` steps (every step written).
/// Returns the per-step worst relative difference per field.
fn run_steps(test: &str, refname: &str, tight: bool, n_steps: usize) -> Vec<(usize, f64, Vec<(&'static str, f64)>)> {
    let r = load_reference(refname);
    let dir = protocol_copy(test, tight);
    let case = TwoPhaseCase::read(&dir).expect("read 1D_boiling");
    let mut s = TwoPhaseSolver::new(case);
    let mut rows = Vec::new();
    for n in 1..=n_steps {
        let rep = s.step().expect("step");
        // step n used dt = 0.001 for n = 1, else the deltaT printed after step n-1.
        let dt_up = if n == 1 { 1e-3 } else { r.dt[n - 2] };
        let dt_rel = (rep.dt - dt_up).abs() / dt_up;
        let tname = &r.times[n - 1];
        let mut worst = Vec::new();
        for (name, v) in port_fields(&s) {
            if let Some(u) = r.fields.get(&(tname.clone(), name.to_string())) {
                worst.push((name, worst_rel(&v, u)));
            }
        }
        rows.push((n, dt_rel, worst));
    }
    let _ = std::fs::remove_dir_all(&dir);
    rows
}

fn print_rows(rows: &[(usize, f64, Vec<(&'static str, f64)>)]) {
    for (n, dt_rel, w) in rows {
        let cells: Vec<String> = w.iter().map(|(k, v)| format!("{k}={v:.2e}")).collect();
        eprintln!("step {n:4} dt_rel={dt_rel:.2e} {}", cells.join(" "));
    }
}

/// First step at which any field's worst relative difference exceeds `tol`.
fn first_exceeding(rows: &[(usize, f64, Vec<(&'static str, f64)>)], tol: f64) -> Option<(usize, &'static str, f64)> {
    for (n, _, w) in rows {
        for (k, v) in w {
            if *v > tol {
                return Some((*n, *k, *v));
            }
        }
    }
    None
}

/// Round 3, row 1: the first 20 steps at the tight protocol, every step.
#[test]
fn one_d_boiling_first_steps_tight_vs_upstream() {
    let rows = run_steps("steps_tight", "boiling_tight_steps", true, 20);
    print_rows(&rows);
    for tol in [1e-12, 1e-9, 1e-6, 1e-3] {
        eprintln!("first step above {tol:e}: {:?}", first_exceeding(&rows, tol));
    }
}

/// Round 3, row 1: the whole 12 s at the tight protocol, compared at every 0.5 s,
/// plus the final cumulative continuity errors (#1239).
#[test]
fn one_d_boiling_full_tight_vs_upstream() {
    full("full_tight", "boiling_tight_full", true);
}

/// Same at the case's shipped tolerances (1e-6, relTol 0.001 on the generic solver).
#[test]
fn one_d_boiling_full_shipped_vs_upstream() {
    full("full_shipped", "boiling_shipped_full", false);
}

fn full(test: &str, refname: &str, tight: bool) {
    let r = load_reference(refname);
    let dir = protocol_copy(test, tight);
    let case = TwoPhaseCase::read(&dir).expect("read 1D_boiling");
    let end = case.end_time;
    let mut s = TwoPhaseSolver::new(case);
    let mut steps = 0usize;
    let mut done: Vec<String> = Vec::new();
    while s.time < end - 1e-9 {
        s.step().expect("step");
        steps += 1;
        for t in &r.times {
            let tv: f64 = t.parse().expect("time");
            if (s.time - tv).abs() < 1e-9 && !done.contains(t) {
                done.push(t.clone());
                let mut cells = Vec::new();
                for (name, v) in port_fields(&s) {
                    if let Some(u) = r.fields.get(&(t.clone(), name.to_string())) {
                        cells.push(format!("{name}={:.2e}", worst_rel(&v, u)));
                    }
                }
                eprintln!("t={t:>5} step {steps:5} {}", cells.join(" "));
            }
        }
    }
    eprintln!(
        "{test}: port steps = {steps}; compared {} of {} reference times; cumulative continuity errors port = ({:.6e}, {:.6e}) kg/m3, upstream = ({:.6e}, {:.6e})",
        done.len(),
        r.times.len(),
        s.fluids[0].cumul_cont_err,
        s.fluids[1].cumul_cont_err,
        r.cum_err.0,
        r.cum_err.1
    );
    let _ = std::fs::remove_dir_all(&dir);
}
