// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//! # `2D_MSFR` Allrun chain with GAMG KEPT AS SHIPPED, against the real binary (gh:#1951; README "Round 77")
//!
//! AI-written, not human-reviewed. Research, education and V&V only.
//!
//! ## Methodology
//!
//! Upstream: GeN-Foam `652b3da` on OpenFOAM-ESI v2506, `reactorCases/2D_MSFR`, replayed by
//! `scripts/run_msfr_allrun_gamg.sh` (`p_rgh` KEEPS `solver GAMG; smoother DIC;`), serial, `writePrecision 17`.
//! Port: `AllrunEdits::as_shipped_gamg()` (`GamgPolicy::Native`: upstream's own `GAMGSolver`, round 67) from `cases/msfr_root`.
//! Tight protocol (T): every `tolerance 1e-14`, `relTol 0` (GAMG then stops at `maxIter 1000`); loose (L): as shipped.
//! Criteria, predictions, results: README "Round 77" (criteria committed before any run). No upstream number is an input of the port.
//!
//! References (`refs/msfr_gamg_r77_*`, `scripts/extract_msfr_gamg_r77.sh`, `extract_msfr_en_reference.sh`, `extract_msfr_fluid_reference.sh`).
//! The first test is the default-run one: it reads the references and runs the first loose steps.

// Reads case files: `std` only (gh:#1633).
#![cfg(feature = "std")]
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use miniz_oxide::inflate::decompress_to_vec;
use outram_foam_appbuilder_lib::genfoam::thermal_hydraulics::solver::two_phase::one_phase_cc::{GamgPolicy, OnePhaseCcSolver};
use outram_foam_appbuilder_lib::genfoam::workflow::allrun::{AllrunEdits, RootCase};
use outram_foam_appbuilder_lib::genfoam::workflow::{
    EigenvalueStage, EnergyEigenvalueRecord, NormalisedState, ThRecord, TransientRun, TransientStart,
};

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

/// `key v1 v2 ...` lines of a reference (EN / transient / log header lines).
fn read_ref(path: &Path) -> HashMap<String, Vec<f64>> {
    let text = String::from_utf8(gunzip(path)).expect("utf8");
    let mut m = HashMap::new();
    for line in text.lines() {
        let mut it = line.split_whitespace();
        if let Some(key) = it.next() {
            if matches!(key, "PRGH" | "EPS" | "K" | "H" | "TSTAT") {
                continue;
            }
            m.insert(key.to_string(), it.filter_map(|w| w.parse().ok()).collect());
        }
    }
    m
}

/// `F <step> <name> v...` lines, keyed `"<name> <step>"`.
fn read_fields(path: &Path) -> HashMap<String, Vec<f64>> {
    let text = String::from_utf8(gunzip(path)).expect("utf8");
    let mut m = HashMap::new();
    for line in text.lines() {
        let mut it = line.split_whitespace();
        if it.next() == Some("F") {
            let step = it.next().expect("step");
            let name = it.next().expect("name");
            m.insert(format!("{name} {step}"), it.map(|w| w.parse().expect("number")).collect());
        }
    }
    m
}

/// The solve log of a `msfr_gamg_r77_*_log` reference.
#[derive(Default)]
struct Log {
    n: usize,
    dt: Vec<f64>,
    /// `step -> [(outer, corrector, initial residual, iterations)]`.
    prgh: HashMap<usize, Vec<(usize, usize, f64, i64)>>,
    /// `step -> [(outer, initial residual, iterations)]`.
    h: HashMap<usize, Vec<(usize, f64, i64)>>,
    tstat: HashMap<(usize, usize), [f64; 3]>,
}

fn read_log(path: &Path) -> Log {
    let text = String::from_utf8(gunzip(path)).expect("utf8");
    let mut l = Log::default();
    for line in text.lines() {
        let w: Vec<&str> = line.split_whitespace().collect();
        if w.is_empty() {
            continue;
        }
        let u = |i: usize| w[i].parse::<usize>().expect("int");
        let f = |i: usize| w[i].parse::<f64>().expect("float");
        match w[0] {
            "N" => l.n = u(1),
            "DT" => l.dt = w[1..].iter().map(|x| x.parse().expect("dt")).collect(),
            "PRGH" => l.prgh.entry(u(1)).or_default().push((u(2), u(3), f(4), w[5].parse().expect("it"))),
            "H" => l.h.entry(u(1)).or_default().push((u(2), f(3), w[4].parse().expect("it"))),
            "TSTAT" => {
                l.tstat.insert((u(1), u(2)), [f(3), f(4), f(5)]);
            }
            _ => {}
        }
    }
    l
}

fn rel(a: f64, b: f64) -> f64 {
    ((a - b) / b).abs()
}

/// `max |a - b| / max |b|`.
fn rel_max(a: &[f64], b: &[f64]) -> f64 {
    assert_eq!(a.len(), b.len());
    let scale = b.iter().fold(0.0_f64, |m, v| m.max(v.abs()));
    a.iter().zip(b).fold(0.0_f64, |m, (x, y)| m.max((x - y).abs())) / scale
}

/// `max_cell |dU| / max_cell |U_ref|` for flattened `x y z` vectors (the criterion's `max|U|`).
fn rel_max_vec(a: &[f64], b: &[f64]) -> f64 {
    assert_eq!(a.len(), b.len());
    let mut scale = 0.0_f64;
    let mut diff = 0.0_f64;
    for (x, y) in a.chunks(3).zip(b.chunks(3)) {
        let n2: f64 = y.iter().map(|v| v * v).sum();
        let d2: f64 = x.iter().zip(y).map(|(p, q)| (p - q) * (p - q)).sum();
        scale = scale.max(n2);
        diff = diff.max(d2);
    }
    (diff / scale).sqrt()
}

fn tighten(fv_solution: &Path) {
    let text = std::fs::read_to_string(fv_solution).expect("fvSolution");
    let mut out = String::new();
    for line in text.lines() {
        let t = line.trim();
        let indent = &line[..line.len() - line.trim_start().len()];
        if t.starts_with("tolerance") && t.ends_with(';') {
            out.push_str(&format!("{indent}tolerance        1e-14;\n"));
        } else if t.starts_with("relTol") && t.ends_with(';') {
            out.push_str(&format!("{indent}relTol           0;\n"));
        } else {
            out.push_str(line);
            out.push('\n');
        }
    }
    std::fs::write(fv_solution, out).expect("write fvSolution");
}

/// Stage `cases/msfr_root` and the two plain meshes; `tight` applies round 67's tight protocol (`p_rgh` KEEPS `GAMG`).
fn stage(tag: &str, tight: bool) -> RootCase {
    let tmp = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("gh1951_{tag}"));
    let _ = std::fs::remove_dir_all(&tmp);
    let root = tmp.join("rootCase");
    copy_dir(&vv_dir().join("cases/msfr_root"), &root);
    if tight {
        tighten(&root.join("system/fluidRegion/fvSolution"));
        tighten(&root.join("system/neutroRegion/fvSolution"));
    }
    let src = vv_dir().join("cases/msfr_neutro/polyMesh");
    let fluid = tmp.join("fluidMesh");
    let neutro = tmp.join("neutroMesh");
    for dir in [&fluid, &neutro] {
        std::fs::create_dir_all(dir).expect("mkdir");
        for f in ["points", "faces", "owner", "neighbour", "cellZones"] {
            std::fs::write(dir.join(f), gunzip(&src.join(format!("{f}.gz")))).expect("polyMesh");
        }
    }
    std::fs::write(neutro.join("boundary"), gunzip(&src.join("boundary.gz"))).expect("boundary");
    std::fs::copy(vv_dir().join("cases/msfr_th/polyMesh/boundary"), fluid.join("boundary")).expect("fluid boundary");
    RootCase::new(root, fluid, neutro)
}

/// Time after `n` steps of the reference's dt sequence (step 1 uses `deltaT 1e-3`) plus a quarter of the next dt, so that
/// `Time::run()` (`value < endTime - 0.5 deltaT`) stops after exactly `n` steps.
fn end_time_after(dt: &[f64], n: usize) -> f64 {
    let mut t = 1e-3;
    for d in dt.iter().take(n - 1) {
        t += d;
    }
    t + 0.25 * dt[n - 1]
}

/// What a TH comparison measured.
#[derive(Default, Debug)]
struct ThReport {
    steps: usize,
    dt_worst: f64,
    t_line_worst: f64,
    /// `(solves compared, upstream solves at maxIter, port solves at maxIter, differing counts below maxIter, worst absolute initial residual difference (solves above 1e-10), first differing step)`.
    prgh: (usize, usize, usize, Vec<String>, f64, Option<usize>),
    /// Per kept step: `(step, [(field, difference)])`.
    fields: Vec<(usize, Vec<(&'static str, f64)>)>,
    /// Worst relative difference of the first `h` residual (noise floor, reported).
    h_first_worst: f64,
}

fn compare_fields(f: &OnePhaseCcSolver, rf: &HashMap<String, Vec<f64>>, step: usize) -> Vec<(&'static str, f64)> {
    let key = |name: &str| format!("{name} {step}");
    let u: Vec<f64> = f.velocity().iter().flat_map(|v| [v.x, v.y, v.z]).collect();
    let fields = f.fluid_fields();
    let mut out = Vec::new();
    let mut put = |name: &'static str, mine: &[f64]| {
        if let Some(r) = rf.get(&key(name)) {
            out.push((name, if name == "U" { rel_max_vec(&mine[..r.len()], r) } else { rel_max(&mine[..r.len()], r) }));
        }
    };
    put("U", &u);
    put("p", f.pressure());
    put("p_rgh", f.p_rgh().expect("flow"));
    put("T", f.temperature());
    put("k", f.turbulent_kinetic_energy().expect("k"));
    put("epsilon", f.turbulent_dissipation().expect("epsilon"));
    put("nut", f.turbulent_viscosity().expect("nut"));
    put("alphat", &fields.alphat);
    put("alphaPhi", &fields.alpha_phi);
    put("alphaRhoPhi", &fields.alpha_rho_phi);
    put("htc", &fields.htc);
    put("T.fixedTemperature", &fields.t_fixed);
    out
}

/// Fold one port step into the report against the reference log (and, at kept steps, the reference fields).
fn compare_step(rep: &mut ThReport, step: usize, rec: &ThRecord, st: &OnePhaseCcSolver, lg: &Log, fields: Option<&HashMap<String, Vec<f64>>>) {
    rep.steps = step;
    if let Some(&up) = lg.dt.get(step - 1) {
        rep.dt_worst = rep.dt_worst.max(rel(rec.next_delta_t, up));
    }
    for (o, ts) in rec.t_stats.iter().enumerate() {
        if let Some(r) = lg.tstat.get(&(step, o + 1)) {
            for (a, b) in [(ts.0, r[0]), (ts.1, r[1]), (ts.2, r[2])] {
                rep.t_line_worst = rep.t_line_worst.max(rel(a, b));
            }
        }
    }
    if let (Some(r), Some(h)) = (lg.h.get(&step).and_then(|v| v.first()), rec.h_residuals.first()) {
        rep.h_first_worst = rep.h_first_worst.max(rel(*h, r.1));
    }
    for &(o, c, ini, it) in lg.prgh.get(&step).map_or(&[][..], Vec::as_slice) {
        let Some(mine) = rec.pressure_solves.iter().find(|x| x.outer == o && x.corrector == c) else { continue };
        rep.prgh.0 += 1;
        let up_floor = it >= 1000;
        if up_floor {
            rep.prgh.1 += 1;
        }
        if mine.iterations >= 1000 {
            rep.prgh.2 += 1;
        }
        if !up_floor && i64::try_from(mine.iterations).unwrap_or(-1) != it {
            if rep.prgh.5.is_none() {
                rep.prgh.5 = Some(step);
            }
            rep.prgh.3.push(format!("step {step} outer {o} corr {c}: upstream {it} its (res {ini:e}), port {} (res {:e})", mine.iterations, mine.initial_residual));
        }
        if ini > 1e-10 {
            rep.prgh.4 = rep.prgh.4.max((mine.initial_residual - ini).abs());
        }
    }
    if let Some(rf) = fields {
        if rf.keys().any(|k| k.ends_with(&format!(" {step}"))) {
            rep.fields.push((step, compare_fields(st, rf, step)));
        }
    }
}

/// The TH stage of `edits` with the comparison; returns the report, the final state and the history (the hand-over of the chain).
fn run_th_state(root: &RootCase, edits: &AllrunEdits, lg: &Log, fields: Option<&HashMap<String, Vec<f64>>>) -> (ThReport, NormalisedState) {
    let EigenvalueStage::SteadyStateTh { mut stage, controls } = root.steady_state_th(edits).expect("rootCase") else { unreachable!() };
    let mut rep = ThReport::default();
    let mut step = 0usize;
    let history = stage
        .run(&controls, |rec, st| {
            step += 1;
            compare_step(&mut rep, step, rec, &st.fluid, lg, fields);
        })
        .expect("TH stage");
    (rep, NormalisedState::SteadyStateTh { stage, history, deviations: Vec::new() })
}

fn run_th(root: &RootCase, edits: &AllrunEdits, lg: &Log, fields: Option<&HashMap<String, Vec<f64>>>) -> ThReport {
    run_th_state(root, edits, lg, fields).0
}

fn print_th(name: &str, r: &ThReport) {
    println!(
        "{name}: steps {}; dt worst {:.3e}; T lines worst {:.3e}; first h residual worst relative {:.3e} (noise floor, reported)",
        r.steps, r.dt_worst, r.t_line_worst, r.h_first_worst
    );
    println!(
        "{name}: p_rgh solves compared {}; upstream at maxIter {}; port at maxIter {} (reported, not gated); counts that differ below maxIter {}; worst absolute initial residual difference (solves above 1e-10) {:.3e}; first differing step {:?}",
        r.prgh.0, r.prgh.1, r.prgh.2, r.prgh.3.len(), r.prgh.4, r.prgh.5
    );
    for l in r.prgh.3.iter().take(6) {
        println!("{name}:   {l}");
    }
    for (s, v) in &r.fields {
        println!("{name}: step {s} fields {v:?}");
    }
}

fn ref_path(name: &str) -> PathBuf {
    vv_dir().join("refs").join(name)
}

fn refs_present(names: &[&str]) -> bool {
    let ok = names.iter().all(|n| ref_path(n).is_file());
    if !ok {
        eprintln!("SKIP: references {names:?} not present (refs submodule)");
    }
    ok
}

/// DEFAULT RUN: the first 5 steps of the LOOSE as-shipped protocol (`GamgPolicy::Native`, shipped tolerances) against the real as-shipped run `R-L1`:
/// the port reads the whole shipped case through `as_shipped_gamg()`, runs upstream's `GAMGSolver`, and its first steps match the real log (the
/// loose caveat of README round 67/77: only the first ~9 steps are gated at the solve level).
#[test]
fn loose_as_shipped_gamg_first_steps_follow_the_real_log() {
    if !refs_present(&["msfr_gamg_r77_loose_log.ref.txt.gz"]) {
        return;
    }
    let lg = read_log(&ref_path("msfr_gamg_r77_loose_log.ref.txt.gz"));
    let n = 5;
    let root = stage("loose5", false);
    let edits = AllrunEdits { end_time_th: end_time_after(&lg.dt, n), write_interval_th: Some(1.0), ..AllrunEdits::as_shipped_gamg() };
    assert_eq!(edits.gamg, GamgPolicy::Native);
    let r = run_th(&root, &edits, &lg, None);
    print_th("loose5", &r);
    assert_eq!(r.steps, n);
    assert!(r.prgh.0 > 0 && r.prgh.3.is_empty(), "p_rgh iteration counts: {:?}", r.prgh.3);
    assert!(r.dt_worst <= 1e-6 && r.t_line_worst <= 1e-6, "dt {:e} T {:e}", r.dt_worst, r.t_line_worst);
}

/// The kept steps of the tight reference and the gates of README "Round 77" C1 (fixed before any run).
fn check_tight(name: &str, r: &ThReport, n_ref: usize) {
    print_th(name, r);
    assert_eq!(r.steps, n_ref, "step count");
    assert!(r.dt_worst <= 1e-12, "dt {:e}", r.dt_worst);
    assert!(r.t_line_worst <= 1e-9, "T lines {:e}", r.t_line_worst);
    assert!(r.prgh.3.is_empty(), "p_rgh iteration counts below maxIter differ: {:?}", &r.prgh.3[..r.prgh.3.len().min(5)]);
    assert!(r.prgh.4 <= 1e-11, "p_rgh initial residual absolute difference {:e}", r.prgh.4);
    assert!(!r.fields.is_empty(), "no kept step compared");
    for (s, v) in &r.fields {
        for (f, d) in v {
            assert!(*d <= 1e-9, "step {s} {f}: {d:e}");
        }
    }
}

/// README Round 77 C1: the tight protocol (GAMG kept), every step of `R-T1`, against the real binary. `#[ignore]`: ~30 s per step in release.
#[test]
#[ignore = "tens of minutes in release (tight GAMG: every p_rgh solve runs 1000 V-cycles); run with --ignored"]
fn tight_gamg_th_steps_match_the_real_binary() {
    if !refs_present(&["msfr_gamg_r77_tight_log.ref.txt.gz", "msfr_gamg_r77_tight_fields.ref.txt.gz"]) {
        return;
    }
    let lg = read_log(&ref_path("msfr_gamg_r77_tight_log.ref.txt.gz"));
    let fields = read_fields(&ref_path("msfr_gamg_r77_tight_fields.ref.txt.gz"));
    let root = stage("tight", true);
    let edits = AllrunEdits { end_time_th: end_time_after(&lg.dt, lg.n), write_interval_th: None, ..AllrunEdits::as_shipped_gamg() };
    let t0 = std::time::Instant::now();
    let r = run_th(&root, &edits, &lg, Some(&fields));
    println!("tight GAMG: {} steps in {:.0} s", r.steps, t0.elapsed().as_secs_f64());
    check_tight("tight_gamg", &r, lg.n);
}

/// Control (README Round 77): the same steps under `AllrunEdits::default()` (`SubstitutePcgDic`) must NOT meet C1's field gate at 1e-9.
#[test]
#[ignore = "minutes in release; run with --ignored"]
fn the_pcg_substitute_does_not_meet_the_tight_gamg_gate() {
    if !refs_present(&["msfr_gamg_r77_tight_log.ref.txt.gz", "msfr_gamg_r77_tight_fields.ref.txt.gz"]) {
        return;
    }
    let lg = read_log(&ref_path("msfr_gamg_r77_tight_log.ref.txt.gz"));
    let fields = read_fields(&ref_path("msfr_gamg_r77_tight_fields.ref.txt.gz"));
    let root = stage("tight_pcg", true);
    let n = lg.n.min(10);
    let edits = AllrunEdits { end_time_th: end_time_after(&lg.dt, n), write_interval_th: None, ..AllrunEdits::default() };
    let r = run_th(&root, &edits, &lg, Some(&fields));
    print_th("control_pcg", &r);
    let worst = r.fields.iter().flat_map(|(_, v)| v.iter().map(|x| x.1)).fold(0.0_f64, f64::max);
    println!("control: worst field difference of the PCG substitute {worst:e}");
    assert!(worst > 1e-9, "the substitute met the gate: C1 does not discriminate");
}

/// Worst relative `a_i / b_i - 1` over two `DT` lists.
fn worst_dt(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b).fold(0.0, |m, (x, y)| m.max(rel(*x, *y)))
}

/// Worst relative difference of the `T (avg min max)` lines of two logs over the steps both have.
fn worst_tstat(a: &Log, b: &Log) -> f64 {
    let mut w = 0.0_f64;
    for (k, x) in &a.tstat {
        if let Some(y) = b.tstat.get(k) {
            for i in 0..3 {
                w = w.max(rel(x[i], y[i]));
            }
        }
    }
    w
}

/// README Round 77 C3: the loose as-shipped protocol against `R-L1`, gates derived from the real binary's own sensitivity (`R-L2`, one input
/// changed by one ulp): gate = 10 x (R-L1 vs R-L2), floored at 1e-9, over the same span. Steps from `GAMG_R77_LOOSE_STEPS` (default 300, `all`).
#[test]
#[ignore = "minutes to an hour in release; run with --ignored (GAMG_R77_LOOSE_STEPS)"]
fn loose_as_shipped_gamg_macroscopic_quantities_follow_the_real_run_within_its_own_sensitivity() {
    if !refs_present(&["msfr_gamg_r77_loose_log.ref.txt.gz", "msfr_gamg_r77_loose_pert_log.ref.txt.gz"]) {
        return;
    }
    let l1 = read_log(&ref_path("msfr_gamg_r77_loose_log.ref.txt.gz"));
    let l2 = read_log(&ref_path("msfr_gamg_r77_loose_pert_log.ref.txt.gz"));
    let n = match std::env::var("GAMG_R77_LOOSE_STEPS").ok().as_deref() {
        Some("all") => l1.n,
        Some(v) => v.parse().expect("steps"),
        None => 300,
    }
    .min(l1.n);
    let m = n.min(l2.n);
    let sens_dt = worst_dt(&l1.dt[..m - 1], &l2.dt[..m - 1]);
    let first_part = (1..=m).find(|s| l1.prgh.get(s).map(|v| v.iter().map(|x| x.3).collect::<Vec<_>>()) != l2.prgh.get(s).map(|v| v.iter().map(|x| x.3).collect::<Vec<_>>()));
    let sens_t = worst_tstat(&l1, &l2);
    println!("real sensitivity over {m} steps: step counts {} / {}; dt {sens_dt:.3e}; T lines {sens_t:.3e}; first step with different p_rgh counts {first_part:?}", l1.n, l2.n);
    let (gate_dt, gate_t) = ((10.0 * sens_dt).max(1e-9), (10.0 * sens_t).max(1e-9));
    let root = stage("loose", false);
    let edits = AllrunEdits { end_time_th: if n == l1.n { 15.0 } else { end_time_after(&l1.dt, n) }, write_interval_th: Some(1.0), ..AllrunEdits::as_shipped_gamg() };
    let t0 = std::time::Instant::now();
    let lf = read_fields(&ref_path("msfr_gamg_r77_loose_fields.ref.txt.gz"));
    let EigenvalueStage::SteadyStateTh { mut stage, controls } = root.steady_state_th(&edits).expect("rootCase") else { unreachable!() };
    let mut r = ThReport::default();
    let mut step = 0usize;
    stage
        .run(&controls, |rec, st| {
            step += 1;
            compare_step(&mut r, step, rec, &st.fluid, &l1, None);
            for k in [1usize, 5, 10, 15] {
                if rec.write_time && (rec.time - k as f64).abs() < 1e-9 {
                    r.fields.push((k, compare_fields(&st.fluid, &lf, k)));
                }
            }
        })
        .expect("TH stage");
    println!("loose: {} steps in {:.0} s (gates: dt {gate_dt:.3e}, T lines {gate_t:.3e})", r.steps, t0.elapsed().as_secs_f64());
    print_th("loose", &r);
    println!("loose: p_rgh counts parted from the real ones first at step {:?}", r.prgh.5);
    assert!(r.dt_worst <= gate_dt, "dt {:e} > {gate_dt:e}", r.dt_worst);
    assert!(r.t_line_worst <= gate_t, "T lines {:e} > {gate_t:e}", r.t_line_worst);
}

/// `compare_en` of the Allrun test: worst of `keff_`, `power:`, first `h` residual, `T` line, `Time`.
fn compare_en(history: &[EnergyEigenvalueRecord], r: &HashMap<String, Vec<f64>>) -> [f64; 5] {
    let mut w = [0.0_f64; 5];
    for (i, rec) in history.iter().enumerate() {
        let hres = &r[&format!("HRES_{}", i + 1)];
        let ts = &r[&format!("TSTAT_{}", i + 1)];
        let last = rec.t_stats.last().expect("t stats");
        w[0] = w[0].max(rel(rec.k_eff, r["KEFF"][i]));
        w[1] = w[1].max(rel(rec.power, r["POWER"][i]));
        w[2] = w[2].max(rel(rec.h_residuals[0], hres[0]));
        w[3] = w[3].max(rel(last.0, ts[12]).max(rel(last.1, ts[13])).max(rel(last.2, ts[14])));
        w[4] = w[4].max((rec.time - r["TIME"][i]).abs() / r["TIME"][i]);
    }
    w
}

/// README Round 77 C2/C2b: the chain from the port's OWN tight-GAMG TH state at 0.5 s: EN (600 steps), then the first 50 transient steps,
/// against `R-TE`/`R-TT`; and the EN stage is bit-identical under `as_shipped_gamg()` and `AllrunEdits::default()` from the same TH state
/// (the GAMG policy cannot matter where `solveFluidMechanics false`).
#[test]
#[ignore = "about half an hour in release (TH stage with tight GAMG, then 600 EN steps); run with --ignored"]
fn the_chain_from_the_tight_gamg_th_state_matches_the_real_binary() {
    if !refs_present(&["msfr_gamg_r77_chain_th_log.ref.txt.gz", "msfr_gamg_r77_chain_en.ref.txt.gz", "msfr_gamg_r77_chain_transient.ref.txt.gz"]) {
        return;
    }
    let started = std::time::Instant::now();
    let root = stage("chain", true);
    let th_edits = AllrunEdits { end_time_th: 0.5, end_time_en: 150.5, end_time_transient: 150.81, write_interval_th: Some(0.5), ..AllrunEdits::as_shipped_gamg() };
    let lg = read_log(&ref_path("msfr_gamg_r77_chain_th_log.ref.txt.gz"));
    let (th, th_state) = run_th_state(&root, &th_edits, &lg, None);
    print_th("chain_th", &th);
    assert_eq!(th.steps, lg.n, "TH step count");
    assert!(th.dt_worst <= 1e-12 && th.t_line_worst <= 1e-9, "dt {:e} T {:e}", th.dt_worst, th.t_line_worst);
    println!("TH stage done at {:.0} s", started.elapsed().as_secs_f64());
    // C2b: first 40 EN steps under both policies from the same TH state
    let short = AllrunEdits { end_time_en: 0.5 + 40.0 * 0.25, ..th_edits.clone() };
    let run_en = |edits: &AllrunEdits| -> Vec<(f64, f64)> {
        let EigenvalueStage::EnergyNeutroDiffusion { stage, controls } = root.steady_state_en(th_state.clone(), edits).expect("EN") else { unreachable!() };
        let NormalisedState::EnergyNeutroDiffusion { history, .. } = (EigenvalueStage::EnergyNeutroDiffusion { stage, controls }).run().expect("EN run") else { unreachable!() };
        history.iter().map(|h| (h.k_eff, h.power)).collect()
    };
    let a = run_en(&short);
    let b = run_en(&AllrunEdits { gamg: GamgPolicy::SubstitutePcgDic, ..short.clone() });
    assert_eq!(a.len(), 40);
    assert!(a.iter().zip(&b).all(|(x, y)| x.0.to_bits() == y.0.to_bits() && x.1.to_bits() == y.1.to_bits()), "EN differs under the two policies");
    println!("C2b: 40 EN steps bit-identical under as_shipped_gamg and SubstitutePcgDic");
    // C2: full EN against the real run
    let en_ref = read_ref(&ref_path("msfr_gamg_r77_chain_en.ref.txt.gz"));
    let EigenvalueStage::EnergyNeutroDiffusion { stage: en_stage, controls } = root.steady_state_en(th_state, &th_edits).expect("EN") else { unreachable!() };
    let NormalisedState::EnergyNeutroDiffusion { stage: s, history } = (EigenvalueStage::EnergyNeutroDiffusion { stage: en_stage, controls }).run().expect("EN run") else { unreachable!() };
    assert_eq!(history.len(), 600);
    let w = compare_en(&history, &en_ref);
    println!("EN 600 steps: worst rel keff {:.3e}, power {:.3e}, first h {:.3e} (reported), T {:.3e}, time {:.3e}; at {:.0} s", w[0], w[1], w[2], w[3], w[4], started.elapsed().as_secs_f64());
    let k_final = s.reactor.k_eff;
    let dk = rel(k_final, en_ref["END_KEFF"][0]);
    let dpd = rel_max(&s.neutro_power_density, &en_ref["END_powerDensity"]);
    let dt_end = rel_max(s.fluid.temperature(), &en_ref["END_T"]);
    println!("final keff_ {k_final:.17} rel {dk:.3e}; powerDensity {dpd:.3e}; T {dt_end:.3e}");
    assert!(w[0] <= 1e-9 && w[1] <= 1e-9 && w[3] <= 1e-9 && w[4] == 0.0 && dk <= 1e-9 && dpd <= 1e-8 && dt_end <= 1e-9, "EN: {w:?} {dk:e} {dpd:e} {dt_end:e}");
    // transient: first 50 steps
    let tr = read_ref(&ref_path("msfr_gamg_r77_chain_transient.ref.txt.gz"));
    let settings = root.transient_settings(&th_edits).expect("transient settings");
    let mut run = NormalisedState::EnergyNeutroDiffusion { stage: s, history }.start_transient(TransientStart::EnergyNeutro(settings)).expect("transient");
    let frozen = run.k_eff();
    let mut worst = [0.0_f64; 3];
    for step in 1..=50usize {
        let rec = run.step_adjustable().expect("transient step");
        let TransientRun::EnergyNeutroDiffusion(t) = &run else { unreachable!() };
        let d = [rel(rec.next_delta_t, tr["DT"][step - 1]), rel_max(t.inner.fluid.temperature(), &tr[&format!("T_{step}")]), rel(rec.power, tr[&format!("POWER_{step}")][0])];
        for i in 0..3 {
            worst[i] = worst[i].max(d[i]);
        }
    }
    assert_eq!(run.k_eff().to_bits(), frozen.to_bits());
    println!("TRANSIENT 50 steps: worst dt {:.3e} T {:.3e} power {:.3e}; total {:.0} s", worst[0], worst[1], worst[2], started.elapsed().as_secs_f64());
    assert!(worst.iter().all(|x| *x <= 1e-9), "{worst:?}");
}
