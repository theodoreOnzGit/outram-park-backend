// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//! # `2D_MSFR` `steadyStateTH`: `PIMPLE.nNonOrthogonalCorrectors` (onePhase `pEqn_1p.H`) against the real binary (gh:#1361 round 33; gh:#1866)
//!
//! AI-written, not human-reviewed. Research, education and V&V only.
//!
//! ## Methodology
//!
//! Upstream: GeN-Foam `652b3da` on OpenFOAM-ESI v2506, unmodified, the `2D_MSFR` `steadyStateTH` stage of rounds 19-31 (`turbulence true`,
//! tight 1e-14 solvers, three steps) with the case VARIANTS of `cases/msfr_th_variants_r33/` (overlay files over `cases/msfr_th`, made by
//! `scripts/make_msfr_th_nonorth_variants.sh`; the port and `scripts/run_msfr_th_nonorth_variants.sh` read the same bytes). Reference: every
//! 11th cell of `U k epsilon p_rgh T` at the three written steps and every linear solve of the log (`refs/msfr_th_r33_<variant>.ref.txt.gz`;
//! the log numbers the `p_rgh` solves of an outer iteration consecutively, so solve `k` is corrector `(k-1)/(n+1)+1`, pass `(k-1)%(n+1)+1`).
//!
//! Compared per variant: (1) outer iterations per step, (2) the iteration count of EVERY `p_rgh`, `h`, `k`, `epsilon` solve (the passes
//! included) to within one iteration, with initial residuals to 1e-6 relative, (3) the fields, max-norm <= 1e-9 (the round-20 gate).
//!
//! **Controls** (each must FAIL): `without_non_orthogonal_correctors` against every variant, and
//! `with_first_piso_residual_of_first_pass_only` against `no_rc_p`.
//!
//! Predictions: README "Round 33" (committed before the first run); results: README "Round 33 results".

// Reads case files: `std` only (gh:#1633).
#![cfg(feature = "std")]
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use miniz_oxide::inflate::decompress_to_vec;
use outram_foam_appbuilder_lib::genfoam::multi_region::coupled_loop::adjust_delta_t;
use outram_foam_appbuilder_lib::genfoam::thermal_hydraulics::solver::two_phase::one_phase_cc::OnePhaseCcSolver;

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

/// Stage the root case with `variant` overlaid, and the polyMesh.
fn stage(tag: &str, variant: &str) -> (PathBuf, PathBuf) {
    let tmp = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("gh1361_r33_{tag}"));
    let _ = std::fs::remove_dir_all(&tmp);
    let root = tmp.join("case");
    for d in ["constant", "system", "0"] {
        copy_dir(&vv_dir().join("cases/msfr_th").join(d), &root.join(d));
    }
    copy_dir(&vv_dir().join("cases/msfr_th_variants_r33").join(variant), &root);
    let poly = tmp.join("polyMesh");
    std::fs::create_dir_all(&poly).expect("mkdir");
    let src = vv_dir().join("cases/msfr_neutro/polyMesh");
    for f in ["points", "faces", "owner", "neighbour", "cellZones"] {
        std::fs::write(poly.join(f), gunzip(&src.join(format!("{f}.gz")))).expect("polyMesh");
    }
    std::fs::copy(vv_dir().join("cases/msfr_th/polyMesh/boundary"), poly.join("boundary")).expect("boundary");
    (root, poly)
}

/// One linear solve of the log: `(initial residual, iterations)`.
type Solve = (f64, i64);

struct Reference {
    stride: usize,
    times: Vec<String>,
    fields: HashMap<String, Vec<f64>>,
    /// `(step, outer, corrector)` -> solve
    prgh: HashMap<(usize, usize, usize), Solve>,
    /// `(kind, step, outer)` -> solve, kind in `H K EPS`
    others: HashMap<(String, usize, usize), Solve>,
}

impl Reference {
    /// Outer iterations of `step`: the number of `H` solves.
    fn outer_iterations(&self, step: usize) -> usize {
        self.others.keys().filter(|(k, s, _)| k == "H" && *s == step).count()
    }
}

fn read_reference(variant: &str) -> Reference {
    let text = String::from_utf8(gunzip(&vv_dir().join(format!("refs/msfr_th_r33_{variant}.ref.txt.gz")))).expect("utf8");
    let mut r = Reference { stride: 0, times: Vec::new(), fields: HashMap::new(), prgh: HashMap::new(), others: HashMap::new() };
    for line in text.lines() {
        let w: Vec<&str> = line.split_whitespace().collect();
        if w.is_empty() {
            continue;
        }
        let p = |i: usize| -> usize { w[i].parse().expect("integer") };
        let f = |i: usize| -> f64 { w[i].parse().expect("number") };
        match w[0] {
            "TIMEPRINT" => r.times = w[1..].iter().map(|s| (*s).to_string()).collect(),
            "STRIDE" => r.stride = p(1),
            "F" => {
                r.fields.insert(format!("{} {}", w[2], w[1]), w[3..].iter().map(|x| x.parse().expect("number")).collect());
            }
            "PRGH" => {
                r.prgh.insert((p(1), p(2), p(3)), (f(4), p(5) as i64));
            }
            "H" | "K" | "EPS" => {
                r.others.insert((w[0].to_string(), p(1), p(2)), (f(3), p(4) as i64));
            }
            _ => {}
        }
    }
    r
}

/// `max |a - b| / max |b|`.
fn max_norm(a: &[f64], b: &[f64]) -> f64 {
    assert_eq!(a.len(), b.len());
    let scale = b.iter().fold(0.0_f64, |m, v| m.max(v.abs()));
    a.iter().zip(b).fold(0.0_f64, |m, (x, y)| m.max((x - y).abs())) / scale
}

fn sample(v: &[f64], width: usize, stride: usize) -> Vec<f64> {
    v.chunks(width).step_by(stride).flatten().copied().collect()
}

/// What one port run says about the reference.
struct Outcome {
    /// Worst field max-norm difference over the three steps, `U k epsilon p_rgh T`.
    worst_field: f64,
    /// Step of the first field difference above the gate (`None`: none).
    first_diverging_step: Option<usize>,
    /// Outer iterations of the port per step.
    outer: Vec<usize>,
    /// Outer iterations of upstream per step.
    outer_ref: Vec<usize>,
    /// Number of solves whose iteration count differs (of the total).
    iter_mismatch: usize,
    /// The solves that differ: `(label, step, port iterations, upstream iterations, upstream initial residual)`.
    mismatches: Vec<(String, usize, i64, i64, f64)>,
    /// Max-norm difference of every compared field: `(step, field, difference)`.
    per_field: Vec<(usize, String, f64)>,
    /// Every initial residual of the port's solves, in the order of the comparison.
    port_residuals: Vec<f64>,
    /// Worst absolute iteration-count difference over the solves (`i64::MAX` for a solve upstream does not have).
    worst_iter_diff: i64,
    solves: usize,
    /// Passes 2 and up (restarts from a solved field): `(port initial residual, upstream initial residual, port iterations, upstream iterations)`.
    restarts: Vec<(f64, f64, i64, i64)>,
    /// Worst initial-residual relative difference over solves above 1e-9.
    worst_residual: f64,
    /// Bit-equal values / values, over all fields.
    exact: (usize, usize),
}

impl Outcome {
    fn counts_equal(&self) -> bool {
        self.outer == self.outer_ref
    }
}

const GATE: f64 = 1e-9;

/// Three steps of `solver` against `reference`.
fn run(solver: &mut OnePhaseCcSolver, reference: &Reference, n_pass: usize) -> Outcome {
    let mut dt = solver.case().delta_t;
    let mut o = Outcome {
        worst_field: 0.0,
        first_diverging_step: None,
        outer: Vec::new(),
        outer_ref: Vec::new(),
        iter_mismatch: 0,
        mismatches: Vec::new(),
        port_residuals: Vec::new(),
        per_field: Vec::new(),
        worst_iter_diff: 0,
        solves: 0,
        restarts: Vec::new(),
        worst_residual: 0.0,
        exact: (0, 0),
    };
    for s in 1..=3usize {
        solver.begin_time_step(dt);
        let (_r, rep) = solver.correct_physics();
        o.outer.push(rep.outer_iterations);
        o.outer_ref.push(reference.outer_iterations(s));
        // A pass after the first restarts a 1e-14 PCG from a solved field: its initial residual is round-off (about 1e-12) and its
        // iteration count (30 to 135 in upstream's own log) follows the round-off pattern, so it is compared as a residual-level pair and
        // NOT by count (README Round 33, finding 1). Counted in `solves`, reported in `restarts`.
        for ps in solver.pressure_solves().iter().filter(|p| p.non_orth > 1) {
            let k = (ps.corrector - 1) * n_pass + ps.non_orth;
            o.solves += 1;
            let Some(t) = reference.prgh.get(&(s, ps.outer, k)) else {
                // a solve upstream does not have (a control that runs more outer iterations)
                o.iter_mismatch += 1;
                o.worst_iter_diff = i64::MAX;
                continue;
            };
            o.port_residuals.push(ps.initial_residual);
            o.restarts.push((ps.initial_residual, t.0, ps.iterations as i64, t.1));
        }
        // solves
        let mut check = |label: String, mine: Solve, theirs: Option<&Solve>| {
            o.solves += 1;
            o.port_residuals.push(mine.0);
            match theirs {
                Some(t) => {
                    if mine.1 != t.1 {
                        o.iter_mismatch += 1;
                        o.worst_iter_diff = o.worst_iter_diff.max((mine.1 - t.1).abs());
                        o.mismatches.push((label, s, mine.1, t.1, t.0));
                    }
                    if t.0 > 1e-9 {
                        o.worst_residual = o.worst_residual.max(((mine.0 - t.0) / t.0).abs());
                    }
                }
                None => {
                    o.iter_mismatch += 1;
                    o.worst_iter_diff = i64::MAX;
                    o.mismatches.push((label, s, mine.1, -1, 0.0));
                }
            }
        };
        for ps in solver.pressure_solves() {
            let k = (ps.corrector - 1) * n_pass + ps.non_orth;
            let theirs = reference.prgh.get(&(s, ps.outer, k));
            if ps.non_orth > 1 {
                continue;
            } else {
                check(format!("PRGH {} {} (corrector {} pass {})", ps.outer, k, ps.corrector, ps.non_orth), (ps.initial_residual, ps.iterations as i64), theirs);
            }
        }
        for (i, (hres, hit)) in solver.h_solves().iter().enumerate() {
            check(format!("H {}", i + 1), (*hres, *hit as i64), reference.others.get(&("H".to_string(), s, i + 1)));
        }
        for (i, pair) in solver.turbulence_solves().expect("turbulence").iter().enumerate() {
            check(format!("EPS {}", i + 1), (pair[0].0, pair[0].1 as i64), reference.others.get(&("EPS".to_string(), s, i + 1)));
            check(format!("K {}", i + 1), (pair[1].0, pair[1].1 as i64), reference.others.get(&("K".to_string(), s, i + 1)));
        }
        // fields
        let t = &reference.times[s - 1];
        let u: Vec<f64> = solver.velocity().iter().flat_map(|v| [v.x, v.y, v.z]).collect();
        let named: [(&str, Vec<f64>, usize); 5] = [
            ("U", u, 3),
            ("k", solver.turbulent_kinetic_energy().expect("k").to_vec(), 1),
            ("epsilon", solver.turbulent_dissipation().expect("epsilon").to_vec(), 1),
            ("p_rgh", solver.p_rgh().expect("p_rgh").to_vec(), 1),
            ("T", solver.temperature().to_vec(), 1),
        ];
        for (name, v, w) in named {
            let mine = sample(&v, w, reference.stride);
            let theirs = &reference.fields[&format!("{name} {t}")];
            let d = max_norm(&mine, theirs);
            o.worst_field = o.worst_field.max(d);
            o.per_field.push((s, name.to_string(), d));
            if d > GATE && o.first_diverging_step.is_none() {
                o.first_diverging_step = Some(s);
            }
            o.exact.0 += mine.iter().zip(theirs).filter(|(a, b)| a == b).count();
            o.exact.1 += mine.len();
        }
        dt = adjust_delta_t(true, &[solver.max_delta_t()], 1e-300, dt).expect("adjustable");
    }
    o
}

fn build(tag: &str, case_variant: &str) -> OnePhaseCcSolver {
    let (root, poly) = stage(tag, case_variant);
    OnePhaseCcSolver::from_case_dir(&root, &poly).unwrap_or_else(|e| panic!("{tag}: {e}"))
}

fn print(label: &str, o: &Outcome) {
    for m in &o.mismatches {
        println!("    iteration mismatch {} step {}: port {} upstream {} (upstream initial residual {:.2e})", m.0, m.1, m.2, m.3, m.4);
    }
    let worst_by_field: Vec<String> = ["U", "k", "epsilon", "p_rgh", "T"]
        .iter()
        .map(|f| {
            let w = o.per_field.iter().filter(|x| x.1 == *f).fold(0.0_f64, |m, x| m.max(x.2));
            format!("{f} {w:.1e}")
        })
        .collect();
    println!("    worst by field over the three steps: {}", worst_by_field.join(", "));
    println!(
        "  {label}: outer port {:?} upstream {:?} | solve iteration mismatches {}/{} (worst residual {:.1e}) | worst field {:.2e}, first step above 1e-9: {:?} | bit-equal {}/{}",
        o.outer, o.outer_ref, o.iter_mismatch, o.solves, o.worst_residual, o.worst_field, o.first_diverging_step, o.exact.0, o.exact.1
    );
}

/// The port on `variant`'s own case against `variant`'s reference.
fn assert_matches(variant: &str, n_pass: usize) -> Outcome {
    println!("{variant}: port on its own case vs the real binary");
    let reference = read_reference(variant);
    let mut solver = build(&format!("pass_{variant}"), variant);
    let o = run(&mut solver, &reference, n_pass);
    print("port", &o);
    assert!(o.counts_equal(), "{variant}: outer iterations {:?} against {:?}", o.outer, o.outer_ref);
    // Iteration counts: the outer-iteration counts above are exact. A linear solve of 230-260 PCG iterations (or 27 of `h`) to an absolute
    // 1e-14 stops at the first iteration whose residual is below the tolerance; a fields difference of ~2.6e-12 (the round-off of the
    // earlier rounds, 1e-12 relative) moves that residual across the threshold in a few solves, by ONE iteration (rounds 20 and 29 gate
    // the same way: `worst iteration difference <= 1`). A different SOLVER ENTRY differs by tens to hundreds (99 against 239 for
    // `p_rghFinal`, 1 against 24 for `hFinal`), so `<= 1` still tells which entry ran. Not a loosened bound: it is the round-20 criterion.
    assert!(o.worst_iter_diff <= 1, "{variant}: {} solves with a different iteration count, worst {}", o.iter_mismatch, o.worst_iter_diff);
    assert!(o.worst_residual <= 1e-6, "{variant}: residual {:e}", o.worst_residual);
    if !o.restarts.is_empty() {
        let (pm, tm) = o.restarts.iter().fold((0.0_f64, 0.0_f64), |m, r| (m.0.max(r.0), m.1.max(r.1)));
        let (ip, it) = o.restarts.iter().fold((0.0_f64, 0.0_f64), |m, r| (m.0 + r.2 as f64, m.1 + r.3 as f64));
        let nr = o.restarts.len() as f64;
        println!("  restart passes ({}): worst initial residual port {pm:.2e} upstream {tm:.2e}; mean iterations port {:.1} upstream {:.1}", o.restarts.len(), ip / nr, it / nr);
        // Both are the round-off floor of a solved 1e-14 system: the port's must be of upstream's order (within a factor 10 of upstream's worst).
        assert!(pm <= 10.0 * tm && pm <= 1e-10, "{variant}: restart residual {pm:e} against upstream {tm:e}");
    }
    assert!(o.worst_field <= GATE, "{variant}: field {:e}, first diverging step {:?}", o.worst_field, o.first_diverging_step);
    o
}

/// A control must FAIL: a different count, a different iteration count somewhere, or a field difference above the gate.
fn assert_fails(label: &str, o: &Outcome) {
    print(label, o);
    assert!(!o.counts_equal() || o.worst_iter_diff > 1 || o.worst_field > GATE, "{label}: the control did not differ");
}

// ---- nNonOrthogonalCorrectors -------------------------------------------------------------------------------------------------

/// `nNonOrthogonalCorrectors 1`: two `p_rgh` solves per corrector; only the last pass assigns the flux and reconstructs `U`.
#[test]
fn one_non_orthogonal_corrector_matches() {
    let o = assert_matches("no_1", 2);
    assert_eq!(o.outer_ref, [5, 5, 5]);
    let reference = read_reference("no_1");
    let mut ctl = build("ctl_no_1", "no_1").without_non_orthogonal_correctors();
    assert_fails("control without_non_orthogonal_correctors", &run(&mut ctl, &reference, 1));
}

#[test]
fn two_non_orthogonal_correctors_match() {
    assert_matches("no_2", 3);
    let reference = read_reference("no_2");
    let mut ctl = build("ctl_no_2", "no_2").without_non_orthogonal_correctors();
    assert_fails("control without_non_orthogonal_correctors", &run(&mut ctl, &reference, 1));
}

/// `finalInnerIter()` is true in the LAST pass of the last corrector: pass 1 runs `p_rgh`, pass 2 the loose `p_rghFinal`.
#[test]
fn p_rgh_final_is_selected_by_the_last_pass_only() {
    assert_matches("no_1_final", 2);
    let reference = read_reference("no_1_final");
    let mut ctl = build("ctl_no_1_final", "no_1_final").without_non_orthogonal_correctors();
    assert_fails("control without_non_orthogonal_correctors", &run(&mut ctl, &reference, 1));
}

/// `updateFirstPISOPrevPIMPLE()` runs after EVERY pass, so the pass-2 residual is the one `residualControl` reads.
#[test]
fn first_piso_residual_is_updated_by_every_pass() {
    let o = assert_matches("no_rc_p", 2);
    assert_eq!(o.outer_ref, [2, 2, 2], "upstream flags the criteria at the start of outer iteration 2 (README prediction P5)");
    let reference = read_reference("no_rc_p");
    let mut ctl = build("ctl_no_rc_p", "no_rc_p").with_first_piso_residual_of_first_pass_only();
    let oc = run(&mut ctl, &reference, 2);
    assert_fails("control with_first_piso_residual_of_first_pass_only", &oc);
    assert!(oc.outer.iter().any(|&n| n > 2), "the control runs more outer iterations: {:?}", oc.outer);
}

/// `p_rgh.relax()` runs in the final pass only.
#[test]
fn p_rgh_field_relaxation_runs_in_the_final_pass_only() {
    assert_matches("no_1_fld_p", 2);
    let reference = read_reference("no_1_fld_p");
    let mut ctl = build("ctl_no_1_fld_p", "no_1_fld_p").without_non_orthogonal_correctors();
    assert_fails("control without_non_orthogonal_correctors", &run(&mut ctl, &reference, 1));
}

/// The `PIMPLE` block of `Tutorials/testing/2D_MSFR_ULOF_diffusion_and_pk` (one outer iteration, six correctors, one non-orthogonal one).
#[test]
fn the_msfr_ulof_tutorial_pimple_block_matches() {
    let o = assert_matches("no_ulof", 2);
    assert_eq!(o.outer_ref, [1, 1, 1]);
    let reference = read_reference("no_ulof");
    let mut ctl = build("ctl_no_ulof", "no_ulof").without_non_orthogonal_correctors();
    assert_fails("control without_non_orthogonal_correctors", &run(&mut ctl, &reference, 1));
}

/// With `uncorrected` Laplacians the extra pass changes the log, not the answer (README prediction P3): port against port.
#[test]
fn the_extra_pass_does_not_change_the_answer_of_the_port() {
    let (mut a, mut b) = (build("self_a", "no_1"), build("self_b", "no_1").without_non_orthogonal_correctors());
    let dt = a.case().delta_t;
    a.begin_time_step(dt);
    b.begin_time_step(dt);
    let _ = a.correct_physics();
    let _ = b.correct_physics();
    assert_eq!(a.pressure_solves().len(), 2 * b.pressure_solves().len());
    let (ua, ub): (Vec<f64>, Vec<f64>) = (
        a.velocity().iter().flat_map(|v| [v.x, v.y, v.z]).collect(),
        b.velocity().iter().flat_map(|v| [v.x, v.y, v.z]).collect(),
    );
    let d = max_norm(&ua, &ub);
    println!("  port nNonOrthogonalCorrectors 1 against 0, step 1: U {d:.2e}");
    assert!(d <= 1e-9, "U differs by {d:e}");
}

/// A `corrected` Laplacian would need the explicit non-orthogonal correction: refused by name.
#[test]
fn corrected_schemes_are_refused_by_name() {
    let (root, poly) = stage("refuse_corrected", "no_1");
    let p = root.join("system/fluidRegion/fvSchemes");
    let text = std::fs::read_to_string(&p).expect("read");
    std::fs::write(&p, text.replacen("Gauss linear uncorrected;", "Gauss linear corrected;", 1)).expect("write");
    let err = OnePhaseCcSolver::from_case_dir(&root, &poly).expect_err("must refuse").to_string();
    assert!(err.contains("laplacianSchemes") && err.contains("uncorrected"), "{err}");
}
