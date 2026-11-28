// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//! # GeN-Foam `twoPhase` MULES `solvePhase` (the `solveAlpha` lambda), non-uniform initial alpha and sub-cycles > 1, vs the real binary (gh:#1799, round 34)
//!
//! AI-written, not human-reviewed. Research, education and V&V only.
//!
//! Methodology, predictions (committed BEFORE any run) and results:
//! `verification_and_validation/genfoam_vs_upstream_gh1361/README.md`, "Round 34". The cases are overlays
//! (`cases/boiling_mules_r34/<variant>`, made by `scripts/make_boiling_mules_variants.sh`) over the vendored `1D_boiling`,
//! with the round-7 protocol (`writePrecision 17`, every step written, every linear tolerance 1e-14, `relTol 0`). The
//! references are fresh runs of the installed upstream binary (`refs/boiling_mules_r34_*.ref.txt.gz`).

// Reads OpenFOAM case files: `std` only (gh:#1633).
#![cfg(feature = "std")]
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use miniz_oxide::inflate::decompress_to_vec;
use outram_foam_appbuilder_lib::genfoam::thermal_hydraulics::solver::two_phase::{AlphaSolver, TwoPhaseCase, TwoPhaseSolver};

struct Reference {
    dt: Vec<f64>,
    subcyc: Vec<i64>,
    fields: HashMap<(String, String), Vec<f64>>,
    times: Vec<String>,
    first: usize,
}

fn vv_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("verification_and_validation/genfoam_vs_upstream_gh1361")
}

fn load_reference(name: &str) -> Reference {
    let path = vv_dir().join("refs").join(format!("{name}.ref.txt.gz"));
    let bytes = std::fs::read(&path).expect("reference present (git submodule update --init refs)");
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

/// The vendored case, the variant's overlay files, then the round-7 protocol (`boiling_mules_protocol.sh`: EVERY_STEP, TIGHT).
fn variant_copy(variant: &str) -> PathBuf {
    static COUNTER: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let k = COUNTER.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    let dst = std::env::temp_dir().join(format!("gh1799_{variant}_{}_{k}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dst);
    copy_dir(&PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/data/genfoam_1d_boiling"), &dst);
    copy_dir(&vv_dir().join("cases/boiling_mules_r34").join(variant), &dst);
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
    let up = if up.len() == port.len() + 1 && up.iter().all(|x| *x == up[0]) { &up[..port.len()] } else { up };
    assert_eq!(port.len(), up.len(), "reference field has the wrong length");
    let scale = up.iter().fold(0.0_f64, |m, x| m.max(x.abs()));
    port.iter().zip(up).map(|(a, b)| if scale == 0.0 { (a - b).abs() } else { (a - b).abs() / scale }).fold(0.0, f64::max)
}

fn port_fields(s: &TwoPhaseSolver) -> Vec<(&'static str, Vec<f64>)> {
    let ni = s.geom.n_internal();
    vec![
        ("p", s.p.c.clone()),
        ("p_rgh", s.p_rgh.c.clone()),
        ("T.liquid", s.fluids[0].t.c.clone()),
        ("T.vapour", s.fluids[1].t.c.clone()),
        ("T.interface", s.t_interface.clone()),
        ("U.liquid", s.fluids[0].u.c.iter().map(|u| u.x).collect()),
        ("U.vapour", s.fluids[1].u.c.iter().map(|u| u.x).collect()),
        ("alpha.vapour", s.fluids[1].alpha.c.clone()),
        ("alpha.liquid", s.fluids[0].alpha.c.clone()),
        ("dmdt.liquid.vapour", s.dmdt.clone()),
        ("alphaPhi.vapour", s.fluids[1].alpha_phi[..ni].to_vec()),
        ("alphaPhi.liquid", s.fluids[0].alpha_phi[..ni].to_vec()),
        ("phi.vapour", s.fluids[1].phi[..ni].to_vec()),
        ("phi.liquid", s.fluids[0].phi[..ni].to_vec()),
    ]
}

/// (step, relative step-size difference, sub-cycle count equal to upstream's, port sub-cycles, port outer iterations,
/// worst relative difference per field).
type Row = (usize, f64, bool, usize, usize, Vec<(&'static str, f64)>);

/// Options that alter the port's alpha treatment, for the controls.
#[derive(Clone, Copy, Default)]
struct Ablation {
    /// Run this variant's overlay but with `solvePhase` replaced (`None` keeps the case's).
    solve_phase: Option<Option<usize>>,
    /// Force `adjustSubCycles false` and `nSubCycles 1`.
    no_sub_cycles: bool,
    /// The port's pre-round-34 old-time of the other fluid's alpha (`with_eager_other_alpha_old_time`).
    eager_other_old: bool,
}

fn run_rows(variant: &str, refs: &[&str], n_steps: usize, ab: Ablation) -> Vec<Row> {
    let refs: Vec<Reference> = refs.iter().map(|r| load_reference(r)).collect();
    let dir = variant_copy(variant);
    let mut case = TwoPhaseCase::read(&dir).expect("read the variant");
    assert!(case.deviations.is_empty(), "MULES runs as selected: {:?}", case.deviations);
    if let AlphaSolver::Mules(mc) = &mut case.controls.alpha {
        if let Some(sp) = ab.solve_phase {
            mc.solve_phase = sp;
        }
        if ab.no_sub_cycles {
            mc.adjust_sub_cycles = false;
            mc.n_sub_cycles = 1;
        }
    } else {
        panic!("the variants select MULES");
    }
    let mut s = TwoPhaseSolver::new(case).with_upstream_stale_pimple_converged_flag();
    if ab.eager_other_old {
        s = s.with_eager_other_alpha_old_time();
    }
    let mut rows = Vec::new();
    for n in 1..=n_steps {
        let rep = match s.step() {
            Ok(r) => r,
            Err(e) => {
                // inside upstream's own post-onset instability the port can diverge (README, variant a): the compared rows end here
                eprintln!("DIVERGED at step {n}: {e:?}");
                break;
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
        rows.push((n, dt_rel, sub_ok, s.alpha_sub_cycles, rep.outer_iterations, worst));
    }
    let _ = std::fs::remove_dir_all(&dir);
    rows
}

fn print_rows(rows: &[Row]) {
    for (n, dt_rel, sub_ok, sub, iters, w) in rows {
        let cells: Vec<String> = w.iter().map(|(k, v)| format!("{k}={v:.2e}")).collect();
        eprintln!("step {n:4} dt_rel={dt_rel:.2e} sub={sub} sub_ok={sub_ok} iters={iters} {}", cells.join(" "));
    }
}

const A: (&str, [&str; 2], usize) = ("a_sp_vap_uniform", ["boiling_mules_r34_a_head", "boiling_mules_r34_a_onset"], 660);
const B: (&str, &str) = ("b_both_bump_sub", "boiling_mules_r34_b_both_bump_sub");
const C: (&str, &str) = ("c_vap_bump_sub", "boiling_mules_r34_c_vap_bump_sub");
const D: (&str, &str) = ("d_liq_bump_sub", "boiling_mules_r34_d_liq_bump_sub");
const E: (&str, &str) = ("e_vap_bump_nosub", "boiling_mules_r34_e_vap_bump_nosub");

/// Diagnostic: print every compared step of variant `VARIANT` (a, b, c, d or e; `--ignored --nocapture`).
#[test]
#[ignore]
fn solve_phase_dump_rows() {
    let v = std::env::var("VARIANT").unwrap_or_else(|_| "c".into());
    let rows = match v.as_str() {
        "a" => run_rows(A.0, &A.1, A.2, Ablation::default()),
        "b" => run_rows(B.0, &[B.1], 60, Ablation::default()),
        "c" => run_rows(C.0, &[C.1], 115, Ablation::default()),
        "d" => run_rows(D.0, &[D.1], 60, Ablation::default()),
        _ => run_rows(E.0, &[E.1], 60, Ablation::default()),
    };
    print_rows(&rows);
}

/// Diagnostic: per-cell port vs upstream at `STEP` (default 1), variant `VARIANT` (c) and the fields in `FIELDS`.
#[test]
#[ignore]
fn solve_phase_cells_dump() {
    let n_steps: usize = std::env::var("STEP").ok().and_then(|v| v.parse().ok()).unwrap_or(1);
    let fields = std::env::var("FIELDS").unwrap_or_else(|_| "alpha.vapour".into());
    let (variant, rname) = match std::env::var("VARIANT").unwrap_or_else(|_| "c".into()).as_str() {
        "b" => B,
        "d" => D,
        "e" => E,
        _ => C,
    };
    let r = load_reference(rname);
    let dir = variant_copy(variant);
    let case = TwoPhaseCase::read(&dir).expect("read");
    let mut s = TwoPhaseSolver::new(case).with_upstream_stale_pimple_converged_flag();
    for _ in 1..=n_steps {
        s.step().expect("step");
    }
    let tname = &r.times[n_steps - r.first];
    for (name, v) in port_fields(&s) {
        if !fields.split(',').any(|f| f == name) {
            continue;
        }
        let u = &r.fields[&(tname.clone(), name.to_string())];
        for c in 0..v.len() {
            eprintln!("CELL {name} {c:3} port={:.17e} up={:.17e} diff={:.2e}", v[c], u[c], v[c] - u[c]);
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
}

// ---------------------------------------------------------------------------------------------------------------
// The gates (measured 2026-11-27, release; README "Round 34").
// ---------------------------------------------------------------------------------------------------------------

const FLOW: [&str; 7] = ["p", "p_rgh", "U.liquid", "U.vapour", "T.liquid", "T.vapour", "T.interface"];

/// First step at which a flow field leaves 1e-6, the step size leaves 1e-12, the sub-cycle count differs from upstream's.
fn first_diverging(rows: &[Row]) -> (Option<usize>, Option<usize>, Option<usize>) {
    let flow = rows.iter().find(|r| r.5.iter().any(|(k, v)| FLOW.contains(k) && *v > 1e-6)).map(|r| r.0);
    let dt = rows.iter().find(|r| r.1 > 1e-12).map(|r| r.0);
    let sub = rows.iter().find(|r| !r.2).map(|r| r.0);
    (flow, dt, sub)
}

/// Every row of steps `lo..=hi` must exist, have upstream's step size to 1e-12 and sub-cycle count, and every field
/// within `bound(field)` of upstream's.
fn assert_span(rows: &[Row], lo: usize, hi: usize, bound: &dyn Fn(&str) -> f64) {
    let mut seen = 0;
    for (n, dt_rel, sub_ok, _, _, w) in rows {
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

/// Round-off bounds of the non-uniform-start variants (b, c, e), 10x the measured worst: p, T 1e-11 and 1e-12, the rest 1e-10.
fn round_off_bound(k: &str) -> f64 {
    match k {
        "p" | "p_rgh" => 1e-11,
        "T.liquid" | "T.vapour" | "T.interface" => 1e-12,
        _ => 1e-10,
    }
}

fn sub_cycles_seen(rows: &[Row]) -> Vec<usize> {
    let mut s: Vec<usize> = rows.iter().map(|r| r.3).collect();
    s.sort_unstable();
    s.dedup();
    s
}

/// Variant c: `solvePhase "vapour"`, a non-uniform initial vapour bump, `alphaMaxCo 0.05`, 115 steps with 1, 2 and 3 sub-cycles
/// (the count equals upstream's at every step). Measured worst over all 115 steps: p 6.6e-13, T <= 4.1e-14, U 8.0e-12,
/// alpha.vapour 2.2e-12, dmdt 5.7e-12, alphaPhi.vapour 6.3e-12, alphaPhi.liquid 5.0e-12, phi 5.1e-12. Before the lazy old-time of
/// the other fluid's alpha was translated (README "Round 34", defect 1) step 1 was off by p 3.1e-6, alpha.vapour 1.6e-6.
#[test]
fn solve_phase_vapour_with_a_non_uniform_start_and_sub_cycles_matches_upstream() {
    let rows = run_rows(C.0, &[C.1], 115, Ablation::default());
    assert_span(&rows, 1, 115, &round_off_bound);
    assert_eq!(sub_cycles_seen(&rows), vec![1, 2, 3], "1, 2 and 3 sub-cycles are all exercised");
}

/// Variant e: the same lambda path with ONE sub-cycle (`alphaMaxCo 0.25`), 60 steps. Measured worst: p 5.0e-13, U 8.0e-12, alpha.vapour 6.8e-13.
#[test]
fn solve_phase_vapour_without_sub_cycles_matches_upstream() {
    let rows = run_rows(E.0, &[E.1], 60, Ablation::default());
    assert_span(&rows, 1, 60, &round_off_bound);
    assert_eq!(sub_cycles_seen(&rows), vec![1]);
}

/// Variant b: the EXISTING `solvePhase both` path with a non-uniform start and 2-3 sub-cycles (the round-7 limitation "sub-cycles
/// not compared before the trajectories separate" is closed here), 60 steps. Measured worst: p 6.0e-13, U 4.3e-12, alpha.vapour 8.8e-13, alphaPhi 3.8e-12.
#[test]
fn solve_phases_both_with_a_non_uniform_start_and_sub_cycles_matches_upstream() {
    let rows = run_rows(B.0, &[B.1], 60, Ablation::default());
    assert_span(&rows, 1, 60, &round_off_bound);
    assert!(sub_cycles_seen(&rows).contains(&3) && rows.iter().any(|r| r.3 == 2));
}

/// Variant d: `solvePhase "liquid"` (the `fluid1_` branch: the vapour is `movingAlpha - alpha.liquid`). Round-off to step 10, and
/// 1e-9..1e-8 (growing: the vapour is a difference of two numbers near 0.5, so its cells at 1e-17 are round-off) to step 44; at step 45
/// a discrete event (README: the noise-floor control leaves 1e-6 within ... steps) takes the port away from upstream.
#[test]
fn solve_phase_liquid_matches_upstream_until_step_44() {
    let rows = run_rows(D.0, &[D.1], 60, Ablation::default());
    assert_span(&rows, 1, 10, &|k| if k == "p" || k == "p_rgh" || k.starts_with("T.") { 1e-11 } else { 1e-10 });
    assert_span(&rows, 11, 44, &|k| match k {
        "p" | "p_rgh" | "T.liquid" | "T.vapour" | "T.interface" => 1e-11,
        "U.liquid" | "phi.liquid" | "alphaPhi.liquid" | "alpha.liquid" => 1e-10,
        "alpha.vapour" => 1e-9,
        _ => 1e-8,
    });
    let (flow, dt, sub) = first_diverging(&rows);
    // ~~(Some(45), None, None)~~ **MOVED 2026-11-27 to (None, None, None)** (gh:#1936 round 72): the default `p_rgh` solver is now upstream's
    // own GAMG; the real run's `p_rgh` solver IS that GAMG, and no flow field leaves 1e-6 within the 60 steps compared (the divergence at 45
    // was the ILU(0)-BiCGStab's 1e-13 stopping noise). Measured 2026-11-27; re-pinned with the cause recorded.
    assert_eq!((flow, dt, sub), (None, None, None), "first diverging step of variant d");
}

/// Diagnostic: the first diverging steps of variant `VARIANT` (`--ignored --nocapture`).
#[test]
#[ignore]
fn solve_phase_first_diverging_dump() {
    let rows = match std::env::var("VARIANT").unwrap_or_else(|_| "a".into()).as_str() {
        "d" => run_rows(D.0, &[D.1], 60, Ablation::default()),
        _ => run_rows(A.0, &A.1, A.2, Ablation::default()),
    };
    eprintln!("(flow > 1e-6, dt > 1e-12, sub-cycles differ) = {:?}", first_diverging(&rows));
}

/// Noise-floor control: the port against ITSELF with the initial liquid temperature scaled by `1 + eps`. Returns the
/// per-step worst relative difference per field, and the first step at which a flow field leaves 1e-6.
fn noise_rows(variant: &str, eps: f64, n_steps: usize) -> (Vec<(usize, Vec<(&'static str, f64)>)>, Option<usize>) {
    let dir = variant_copy(variant);
    let case = TwoPhaseCase::read(&dir).expect("read");
    let mut pert = case.clone();
    for v in pert.fluids[0].t.c.iter_mut().chain(pert.fluids[0].t.b.iter_mut()) {
        *v *= 1.0 + eps;
    }
    let mut a = TwoPhaseSolver::new(case).with_upstream_stale_pimple_converged_flag();
    let mut b = TwoPhaseSolver::new(pert).with_upstream_stale_pimple_converged_flag();
    let mut rows = Vec::new();
    for n in 1..=n_steps {
        if a.step().is_err() || b.step().is_err() {
            break;
        }
        let fa = port_fields(&a);
        let fb = port_fields(&b);
        rows.push((n, fa.iter().zip(&fb).map(|((k, x), (_, y))| (*k, worst_rel(y, x))).collect::<Vec<_>>()));
    }
    let _ = std::fs::remove_dir_all(&dir);
    let first = rows.iter().find(|r| r.1.iter().any(|(k, v)| FLOW.contains(k) && *v > 1e-6)).map(|r| r.0);
    (rows, first)
}

/// Diagnostic: noise-floor control of variant `VARIANT` with `EPS` (default 1e-12).
#[test]
#[ignore]
fn solve_phase_noise_floor_dump() {
    let eps: f64 = std::env::var("EPS").ok().and_then(|v| v.parse().ok()).unwrap_or(1e-12);
    let (variant, n) = match std::env::var("VARIANT").unwrap_or_else(|_| "d".into()).as_str() {
        "a" => (A.0, 660),
        "c" => (C.0, 115),
        _ => (D.0, 80),
    };
    let (rows, first) = noise_rows(variant, eps, n);
    for (k, w) in rows.iter().filter(|(k, _)| k % 5 == 0 || (40..=50).contains(k) || (630..=645).contains(k)) {
        let cells: Vec<String> = w.iter().filter(|(f, _)| ["p", "U.vapour", "alpha.vapour"].contains(f)).map(|(f, v)| format!("{f}={v:.2e}")).collect();
        eprintln!("noise step {k:4} {}", cells.join(" "));
    }
    eprintln!("first flow > 1e-6: {first:?}");
}

/// Variant a: `solvePhase "vapour"` on the SHIPPED initial state (uniform 0 vapour), to and through the boiling onset. Measured worst:
/// steps 1-40 p 2.6e-13, U 5.5e-12, alpha.vapour/dmdt/alphaPhi.vapour EXACTLY 0; 600-615 p 4.7e-14, U 4.6e-12, vapour exactly 0; onset
/// 616-634 (vapour first > 0 at 616) alpha.vapour 1.0e-10, dmdt 9.0e-11, alphaPhi.vapour 1.0e-10, p 1.2e-12, U 6.5e-12. First step with a flow field
/// off by > 1e-6: 637 (the step size differs there too); the sub-cycle counts differ from 639.
#[test]
fn solve_phase_vapour_on_the_shipped_start_matches_upstream_through_the_onset() {
    let rows = run_rows(A.0, &A.1, A.2, Ablation::default());
    let vapour_exact = |k: &str| matches!(k, "alpha.vapour" | "alpha.liquid" | "dmdt.liquid.vapour" | "alphaPhi.vapour");
    let single_phase = |k: &str| if vapour_exact(k) { 0.0 } else if k == "p" || k == "p_rgh" || k.starts_with("T.") { 1e-11 } else { 1e-10 };
    assert_span(&rows, 1, 40, &single_phase);
    assert_span(&rows, 600, 615, &single_phase);
    assert_span(&rows, 616, 634, &|k| match k {
        "alpha.vapour" | "dmdt.liquid.vapour" | "alphaPhi.vapour" => 1e-9,
        "T.liquid" | "T.vapour" | "T.interface" => 1e-11,
        _ => 1e-10,
    });
    let (flow, dt, sub) = first_diverging(&rows);
    assert_eq!(flow, Some(637), "first step at which a flow field leaves 1e-6");
    assert_eq!(dt, Some(637));
    assert_eq!(sub, Some(639), "upstream has 2 sub-cycles at 637-638 (the port agrees), 13 at 639");
}

/// Noise-floor controls: the SAME events happen when the port is disturbed by round-off against ITSELF, so they are the system's
/// sensitivity and not a translation difference. Variant a (initial liquid temperature x (1 + 1e-9)): flow first leaves 1e-6 at 637, the
/// step port-vs-upstream does. Variant d (x (1 + 1e-12)): at 45, the step port-vs-upstream does.
#[test]
fn the_first_diverging_steps_are_the_noise_floor_of_the_system() {
    assert_eq!(noise_rows(A.0, 1e-9, 660).1, Some(637));
    // ~~Some(45)~~ **MOVED 2026-11-27 to None** (gh:#1936 round 72): with upstream's own GAMG for `p_rgh` the 1e-12 disturbance does not take
    // the flow past 1e-6 within 80 steps either (the noise at step 45 was the ILU(0)-BiCGStab stopping rule's).
    assert_eq!(noise_rows(D.0, 1e-12, 80).1, None);
}

/// Controls (each must FAIL the round-off gate of variant c by orders of magnitude, so the comparison can fail): the other branch
/// (`solvePhase` liquid), `solvePhase both`, no sub-cycling, and the pre-round-34 old-time of the other fluid's alpha (step 1 off by
/// p 3.1e-6, alpha.vapour 1.6e-6).
#[test]
fn the_comparison_can_fail_wrong_branch_wrong_path_no_sub_cycles_eager_old_time() {
    let worst = |rows: &[Row], k: &str| rows.iter().map(|r| r.5.iter().find(|(f, _)| *f == k).map(|x| x.1).unwrap_or(0.0)).fold(0.0, f64::max);
    let wrong_branch = run_rows(C.0, &[C.1], 60, Ablation { solve_phase: Some(Some(0)), ..Ablation::default() });
    assert!(worst(&wrong_branch, "alpha.vapour") > 1e-6 && worst(&wrong_branch, "p") > 1e-8, "{:.2e}", worst(&wrong_branch, "alpha.vapour"));
    let both = run_rows(C.0, &[C.1], 60, Ablation { solve_phase: Some(None), ..Ablation::default() });
    assert!(worst(&both, "alpha.vapour") > 1e-6 && worst(&both, "p") > 1e-8, "{:.2e}", worst(&both, "alpha.vapour"));
    let no_sub = run_rows(C.0, &[C.1], 60, Ablation { no_sub_cycles: true, ..Ablation::default() });
    assert!(no_sub.iter().any(|r| !r.2), "forcing one sub-cycle must disagree with upstream's count");
    assert!(worst(&no_sub, "alpha.vapour") > 1e-9, "{:.2e}", worst(&no_sub, "alpha.vapour"));
    let eager = run_rows(C.0, &[C.1], 60, Ablation { eager_other_old: true, ..Ablation::default() });
    assert!(worst(&eager, "alpha.vapour") > 1e-7 && worst(&eager, "p") > 1e-7, "{:.2e} {:.2e}", worst(&eager, "alpha.vapour"), worst(&eager, "p"));
}
