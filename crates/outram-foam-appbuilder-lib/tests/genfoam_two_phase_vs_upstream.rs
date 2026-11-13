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
    /// 1-based step index of the first stored time directory (1 unless the reference starts late).
    first: usize,
}

fn load_reference(name: &str) -> Reference {
    // An absolute path is a scratch reference (diagnostics only).
    let path = if name.starts_with('/') {
        PathBuf::from(name)
    } else {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("verification_and_validation/genfoam_vs_upstream_gh1361/refs")
            .join(format!("{name}.ref.txt.gz"))
    };
    let bytes = std::fs::read(&path).expect("reference present");
    // gzip -n: 10-byte header, 8-byte trailer.
    let text = String::from_utf8(decompress_to_vec(&bytes[10..bytes.len() - 8]).expect("inflate"))
        .expect("utf8");
    let mut r = Reference { dt: vec![], cum_err: (0.0, 0.0), fields: HashMap::new(), times: vec![], first: 1 };
    for line in text.lines() {
        let mut w = line.split_whitespace();
        match w.next() {
            Some("FIRST") => r.first = w.next().expect("first").parse().expect("first"),
            Some("DT") => r.dt = w.map(|x| x.parse().expect("dt")).collect(),
            Some("CUMERR") => {
                let v: Vec<f64> = w.map(|x| x.parse().expect("err")).collect();
                if v.len() == 2 {
                    r.cum_err = (v[0], v[1]);
                }
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
fn protocol_copy(test: &str, tight: bool, every_step: bool) -> PathBuf {
    let src = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/data/genfoam_1d_boiling");
    let dst = std::env::temp_dir().join(format!("gh1361_{test}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dst);
    copy_dir(&src, &dst);
    if every_step {
        // `EVERY_STEP=1` of the script: write control by time step, so the step size is
        // not snapped to the write interval (`adjustableRunTime` does that).
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
    }
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
    assert_eq!(port.len(), up.len(), "reference field has the wrong length (a uniform field not expanded?)");
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
    run_steps_with(test, refname, tight, n_steps, |s| s.with_upstream_stale_pimple_converged_flag())
}

/// [`run_steps`] with the solver transformed first (an ablation builder).
fn run_steps_with(
    test: &str,
    refname: &str,
    tight: bool,
    n_steps: usize,
    ablation: fn(TwoPhaseSolver) -> TwoPhaseSolver,
) -> Vec<(usize, f64, Vec<(&'static str, f64)>)> {
    let r = load_reference(refname);
    let dir = protocol_copy(test, tight, true);
    let case = TwoPhaseCase::read(&dir).expect("read 1D_boiling");
    let mut s = ablation(TwoPhaseSolver::new(case));
    let mut rows = Vec::new();
    for n in 1..=n_steps {
        let rep = s.step().expect("step");
        // step n used dt = 0.001 for n = 1, else the deltaT printed after step n-1.
        let dt_up = if n == 1 { 1e-3 } else { r.dt[n - 2] };
        let dt_rel = (rep.dt - dt_up).abs() / dt_up;
        if n < r.first {
            continue;
        }
        let tname = &r.times[n - r.first];
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

type Rows = [(usize, f64, Vec<(&'static str, f64)>)];

/// Assert `bound(field)` on every row of steps `lo..=hi`.
fn assert_bounds(rows: &Rows, lo: usize, hi: usize, bound: &dyn Fn(&str) -> f64) {
    for (n, dt_rel, w) in rows {
        if *n < lo || *n > hi {
            continue;
        }
        assert!(*dt_rel <= 1e-12, "step {n}: the step size differs from upstream's by {dt_rel:.3e}");
        for (k, v) in w {
            assert!(*v <= bound(k), "step {n}: {k} differs from upstream by {v:.3e}, bound {:.1e}", bound(k));
        }
    }
}

/// Round 3, row 1: the first 65 steps at the tight protocol, every step.
#[test]
fn one_d_boiling_first_steps_tight_vs_upstream() {
    let rows = run_steps("steps_tight", "boiling_tight_steps80", true, 65);
    print_rows(&rows);
    for tol in [1e-12, 1e-9, 1e-6, 1e-3] {
        eprintln!("first step above {tol:e}: {:?}", first_exceeding(&rows, tol));
    }
    // Measured 2026-11-12 (README, Round 3): steps 3-65 worst p 4.7e-12, T.liquid 2.5e-13,
    // U.liquid 9.3e-12, T.vapour 2.3e-10, T.interface 7.6e-10, U.vapour 7.0e-10; the vapour
    // is absent (alpha and dmdt are exactly 0 in both). Steps 1-2: only U.vapour is looser
    // (5.7e-6 at step 2, 2.1e-5 at step 1), the absent-vapour inlet cell, a known residual.
    assert_bounds(&rows, 1, 2, &|k| if k == "U.vapour" { 5e-5 } else { 1e-10 });
    assert_bounds(&rows, 3, 65, &|k| match k {
        "p" | "p_rgh" => 1e-11,
        "T.liquid" | "T.fixedPower" => 1e-12,
        "U.liquid" => 1e-10,
        "T.vapour" => 1e-9,
        "T.interface" | "U.vapour" => 5e-9,
        _ => 0.0,
    });
}

/// The named options keep the port's earlier forms callable, and each one
/// misses upstream by what the README records (measured 2026-11-12, tight
/// protocol, every step).
#[test]
fn one_d_boiling_ablations_reproduce_the_recorded_misses() {
    // Step 1 with the old-time pressure taken as the initial field and the structure's
    // old-time temperature as the start-of-step value: p off by 1.5e-3 of its maximum.
    let rows = run_steps_with("abl_lazy", "boiling_tight_steps80", true, 2, |s| s.without_first_step_lazy_old_time());
    print_rows(&rows);
    let p1 = rows[0].2.iter().find(|(k, _)| *k == "p").expect("p").1;
    assert!(p1 > 1e-4, "ablation should miss step 1 p by ~1.5e-3, got {p1:.3e}");
    // The documented `maxTInterfaceDdt` limiter: T.interface off by ~3 K of 1157 K at step 2.
    let rows = run_steps_with("abl_ti", "boiling_tight_steps80", true, 2, |s| s.with_documented_t_interface_rate_limiter());
    print_rows(&rows);
    let ti = rows[1].2.iter().find(|(k, _)| *k == "T.interface").expect("Ti").1;
    assert!(ti > 1e-3, "ablation should miss step 2 T.interface by ~3e-3, got {ti:.3e}");
    // The port's earlier write-time rule: the step differs from ESI's at step 3 under
    // `writeControl adjustableRunTime` (compared with the adjustable reference's DT list).
    let r = load_reference("boiling_tight_full");
    let dir = protocol_copy("abl_adj", true, false);
    let case = TwoPhaseCase::read(&dir).expect("read");
    let mut esi = TwoPhaseSolver::new(case.clone());
    let mut old = TwoPhaseSolver::new(case).with_foundation_style_write_time_adjust();
    let _ = std::fs::remove_dir_all(&dir);
    let (mut worst_esi, mut worst_old) = (0.0_f64, 0.0_f64);
    for n in 1..=30 {
        esi.step().expect("esi step");
        old.step().expect("old step");
        // upstream's dt for step n is the deltaT printed after step n-1
        worst_esi = worst_esi.max((esi.dt - r.dt[n - 1]).abs() / r.dt[n - 1]);
        worst_old = worst_old.max((old.dt - r.dt[n - 1]).abs() / r.dt[n - 1]);
    }
    eprintln!("dt vs upstream over 30 adjustable steps: ESI {worst_esi:.2e}, earlier rule {worst_old:.2e}");
    assert!(worst_esi < 1e-12);
    assert!(worst_old > 1e-4);
}

/// Round 3: steps 616-627 across the boiling onset (alpha.vapour first exceeds 0 in
/// upstream at step 618, t = 6.02 s), every step, tight protocol.
#[test]
fn one_d_boiling_boiling_onset_tight_vs_upstream() {
    let rows = run_steps("onset_tight", "boiling_tight_onset", true, 627);
    print_rows(&rows);
    for tol in [1e-12, 1e-9, 1e-6, 1e-3] {
        eprintln!("first step above {tol:e}: {:?}", first_exceeding(&rows, tol));
    }
    // Step 616 (alpha.vapour = 0): everything at round-off. Steps 617-627 (measured 2026-11-12):
    // the vapour fraction and the phase-change rate, tiny numbers whose tail is the noise of
    // upstream's `PBiCGStab` stopping rule, differ by 3.4e-6 and 3.1e-6 of their maxima.
    assert_bounds(&rows, 616, 616, &|_| 1e-10);
    assert_bounds(&rows, 617, 627, &|k| match k {
        "p" | "p_rgh" => 2e-9,
        "T.liquid" | "T.vapour" | "T.interface" | "T.fixedPower" => 1e-10,
        "U.liquid" => 1e-8,
        "U.vapour" => 1e-7,
        "alpha.vapour" | "dmdt.liquid.vapour" => 1e-5,
        _ => 0.0,
    });
}

/// The noise floor of the post-onset comparison: the port against ITSELF with the
/// alpha equation solved exactly instead of OpenFOAM's `PBiCGStab` (an
/// ablation that changes only the vapour tail at the 1e-6 level at onset). If
/// two runs that differ that little diverge by O(1) after the boiling onset,
/// the port-vs-upstream divergence there is the flow's sensitivity, not a model
/// difference.
#[test]
fn one_d_boiling_post_onset_sensitivity_of_the_port_to_the_alpha_solve() {
    let dir = protocol_copy("sens", true, false);
    let case = TwoPhaseCase::read(&dir).expect("read 1D_boiling");
    let mut a = TwoPhaseSolver::new(case.clone());
    let mut b = TwoPhaseSolver::new(case).with_exact_alpha_solve();
    let _ = std::fs::remove_dir_all(&dir);
    let mut worst_at: Vec<(f64, f64)> = Vec::new();
    for probe in [6.0_f64, 6.5, 7.0] {
        while a.time < probe - 1e-9 {
            a.step().expect("step a");
        }
        while b.time < probe - 1e-9 {
            b.step().expect("step b");
        }
        let (fa, fb) = (port_fields(&a), port_fields(&b));
        let mut w = 0.0_f64;
        for ((name, va), (_, vb)) in fa.iter().zip(&fb) {
            let r = worst_rel(va, vb);
            eprintln!("sensitivity t={probe} {name} {r:.2e}");
            w = w.max(r);
        }
        worst_at.push((probe, w));
    }
    eprintln!("sensitivity worst by time: {worst_at:?}");
    // Before the onset the two ablations are identical (alpha.vapour is 0, the solve is
    // trivial); after it a change of 1e-6 in a tail grows to O(1) within 0.5 s (measured
    // 2026-11-12: 0.76 at t = 6.5 s, 0.40 at 7 s), the size of the port-vs-upstream gap.
    assert_eq!(worst_at[0].1, 0.0);
    assert!(worst_at[1].1 > 1e-2, "post-onset boiling should amplify a 1e-6 tail difference: {}", worst_at[1].1);
}

/// Round 3, row 1: the whole 12 s at the tight protocol, compared at every 0.5 s,
/// plus the final cumulative continuity errors (#1239).
#[test]
fn one_d_boiling_full_tight_vs_upstream() {
    // Measured 2026-11-12: worst 2.5e-11 (t = 6 s), vapour absent.
    full("full_tight", "boiling_tight_full", true, 1e-10, true);
}

/// Same at the case's shipped tolerances (1e-6, relTol 0.001 on the generic solver).
#[test]
fn one_d_boiling_full_shipped_vs_upstream() {
    // Measured 2026-11-12: worst 3.0e-5 (T.liquid, t = 6 s): the two codes' loose linear
    // solvers (GAMG 1e-6 vs ILU-BiCGStab for p_rgh, relTol 0.001) stop at different residuals.
    // The upstream option is NOT set here: with it, this run stalls after t = 9.5 s (step 4903, over 20
    // minutes of CPU with no further reference time reached; cause not investigated, README "Round 4
    // follow-up"). The gated comparison (t <= 6 s) is identical either way: the first step whose
    // criteria are first met in the last outer iteration is 735, after the onset at step 618.
    full("full_shipped", "boiling_shipped_full", false, 1e-4, false);
}

/// Default (corrected) vs upstream-defect option on the whole tight run (gh:#1361 round 4 follow-up,
/// maintainer decision 2026-11-12): the default solves every step, the upstream option skips the
/// step after each one whose criteria were first met in the last outer iteration. Pins the default
/// (pattern of `outram-mc-libs/tests/correct_physics_is_default.rs`).
#[test]
fn one_d_boiling_default_is_the_corrected_pimple_ending() {
    let dir = protocol_copy("def_ending", true, false);
    let case = TwoPhaseCase::read(&dir).expect("read 1D_boiling");
    let _ = std::fs::remove_dir_all(&dir);
    let end = case.end_time;
    let fresh = TwoPhaseSolver::new(case);
    assert!(!fresh.controls.stale_pimple_converged_flag, "the default must be the corrected ending");
    let run = |mut s: TwoPhaseSolver| {
        let (mut skipped, mut would) = (Vec::new(), Vec::new());
        let mut n = 0usize;
        while s.time < end - 1e-9 {
            let rep = s.step().expect("step");
            n += 1;
            if rep.outer_iterations == 0 {
                skipped.push(n);
            }
            if rep.upstream_would_skip_next_step {
                would.push(n);
            }
        }
        (n, skipped, would)
    };
    let (n_c, skipped_c, would_c) = run(fresh.clone());
    let (n_u, skipped_u, would_u) = run(fresh.with_upstream_stale_pimple_converged_flag());
    eprintln!("default: {n_c} steps, skipped {skipped_c:?}, criteria first met in last iteration after {would_c:?}");
    eprintln!("upstream option: {n_u} steps, skipped {skipped_u:?}, flagged {would_u:?}");
    assert!(skipped_c.is_empty(), "the corrected default solves every step");
    assert!(!would_c.is_empty(), "1D_boiling does reach the last-iteration convergence");
    assert!(!skipped_u.is_empty(), "the upstream option skips steps");
}

fn full(test: &str, refname: &str, tight: bool, pre_onset_bound: f64, stale: bool) {
    let r = load_reference(refname);
    let dir = protocol_copy(test, tight, false);
    let case = TwoPhaseCase::read(&dir).expect("read 1D_boiling");
    let end = case.end_time;
    let mut s = TwoPhaseSolver::new(case);
    if stale {
        s = s.with_upstream_stale_pimple_converged_flag();
    }
    let mut steps = 0usize;
    let mut would_skip = Vec::new();
    let mut done: Vec<String> = Vec::new();
    while s.time < end - 1e-9 {
        let rep = s.step().expect("step");
        steps += 1;
        if rep.upstream_would_skip_next_step {
            would_skip.push(steps);
        }
        for t in &r.times {
            let tv: f64 = t.parse().expect("time");
            if (s.time - tv).abs() < 1e-9 && !done.contains(t) {
                done.push(t.clone());
                let mut cells = Vec::new();
                for (name, v) in port_fields(&s) {
                    if let Some(u) = r.fields.get(&(t.clone(), name.to_string())) {
                        let w = worst_rel(&v, u);
                        cells.push(format!("{name}={w:.2e}"));
                        // The boiling onset is at t = 6.02 s in upstream: up to 6.0 s the
                        // run is single-phase.
                        if tv <= 6.0 + 1e-9 {
                            assert!(w <= pre_onset_bound, "t = {t}: {name} differs from upstream by {w:.3e}, bound {pre_onset_bound:.1e}");
                        }
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
    eprintln!("{test}: steps whose criteria were first met in the last outer iteration: {would_skip:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Diagnostic (not a gate): per-cell values of step 1, port and upstream, for the
/// fields in `GH1361_FIELDS` (comma separated, default `p,U.liquid,T.liquid`).
#[test]
#[ignore]
fn dump_first_step_cells() {
    let r = load_reference(&std::env::var("GH1361_REF").unwrap_or_else(|_| "boiling_tight_steps".into()));
    let dir = protocol_copy("dump1", true, true);
    let mut s = TwoPhaseSolver::new(TwoPhaseCase::read(&dir).expect("read"));
    if let Ok(k) = std::env::var("GH1361_NOUTER") {
        s.controls.n_outer = k.parse().expect("n_outer");
    }
    let n_steps: usize = std::env::var("GH1361_STEPS").ok().map_or(1, |v| v.parse().expect("steps"));
    for _ in 1..n_steps {
        s.step().expect("step");
    }
    let rep = s.step().expect("step");
    eprintln!("REPORT {rep:?}");
    let ni = s.geom.n_internal();
    for k in 0..2 {
        for f in ni..s.geom.n_faces() {
            if std::env::var("GH1361_BFACE").is_ok() { eprintln!("BFACE fluid {k} face {f} phi {:.17e}", s.fluids[k].phi[f]); }
        }
    }
    let want = std::env::var("GH1361_FIELDS").unwrap_or_else(|_| "p,U.liquid,T.liquid".into());
    for (name, v) in port_fields(&s) {
        if !want.split(',').any(|w| w == name) {
            continue;
        }
        let u = &r.fields[&(r.times[n_steps - r.first].clone(), name.to_string())];
        for (i, (a, b)) in v.iter().zip(u).enumerate() {
            eprintln!("{name} cell {i:2} port {a:.17e} upstream {b:.17e} diff {:.3e}", a - b);
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// Diagnostic (not a gate), gh:#1361 round 5: write the port's PIMPLE control flow of a whole run in the
/// format of `scripts/extract_pimple_flow.sh` (one `STEP n time iterations ending p_rgh-list h-list` line
/// per step) to `GH1361_FLOW_OUT`, so it can be compared with the upstream log step by step.
/// `GH1361_TIGHT` (default 1), `GH1361_STALE` (default 1), `GH1361_NOUTER`, `GH1361_END` (end time, default 12).
#[test]
#[ignore]
fn dump_pimple_flow() {
    let out = std::env::var("GH1361_FLOW_OUT").expect("GH1361_FLOW_OUT");
    let tight = std::env::var("GH1361_TIGHT").map_or(true, |v| v != "0");
    let stale = std::env::var("GH1361_STALE").map_or(true, |v| v != "0");
    let dir = protocol_copy("flow", tight, std::env::var("GH1361_EVERY_STEP").is_ok());
    let mut s = TwoPhaseSolver::new(TwoPhaseCase::read(&dir).expect("read"));
    let _ = std::fs::remove_dir_all(&dir);
    if stale {
        s = s.with_upstream_stale_pimple_converged_flag();
    }
    if let Ok(k) = std::env::var("GH1361_NOUTER") {
        s.controls.n_outer = k.parse().expect("n_outer");
    }
    let end: f64 = std::env::var("GH1361_END").map_or_else(|_| 12.0, |v| v.parse().expect("end"));
    let n_outer = s.controls.n_outer;
    let mut text = String::new();
    let mut n = 0usize;
    while s.time < end - 1e-9 {
        let rep = s.step().expect("step");
        n += 1;
        let ending = if rep.outer_iterations == 0 {
            "conv0".to_string()
        } else if rep.converged_at != 0 && rep.converged_at < n_outer {
            format!("conv{}", rep.converged_at)
        } else {
            "notconv".to_string()
        };
        let k = s.pimple_trace.len().saturating_sub(1);
        let join = |f: &dyn Fn(&(f64, f64)) -> f64| -> String {
            if k == 0 {
                "-".to_string()
            } else {
                s.pimple_trace[..k].iter().map(|t| format!("{:e}", f(t))).collect::<Vec<_>>().join(",")
            }
        };
        text.push_str(&format!(
            "STEP {n} {} {} {ending} {} {}\n",
            rep.time,
            rep.outer_iterations,
            join(&|t| t.0),
            join(&|t| t.1)
        ));
    }
    std::fs::write(out, text).expect("write");
}
