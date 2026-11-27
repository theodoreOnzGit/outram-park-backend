// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//! # `2D_MSFR` `steadyStateTH`: the `porousKEpsilon` options no shipped case exercises, against the real binary (gh:#1361 round 29; gh:#1842)
//!
//! AI-written, not human-reviewed. Research, education and V&V only.
//!
//! ## Methodology
//!
//! Upstream: GeN-Foam `652b3da` on OpenFOAM-ESI v2506, unmodified, the `2D_MSFR` `steadyStateTH` stage of round 19/20 with the case
//! VARIANTS of `cases/msfr_th_variants/` (overlay files over `cases/msfr_th`, made by `scripts/make_msfr_th_variants.sh`; the same
//! bytes are read by the port and by upstream, `scripts/run_msfr_th_variants.sh`): the wall-function entries `Cmu`, `kappa`, `E`,
//! `blending` (`stepwise`, `max`, `binomial` with `n`, `exponential`, `tanh`), `lowReCorrection`, `U`; the nut stabilisation
//! `DhStruct`/`laminarReStruct`; and `Final` solver/relaxation entries. Tight protocol (every solver 1e-14, `relTol 0`, except the
//! `fin_solver` entries), `writePrecision 17`, three steps (`deltaT 1e-3`, then `maxDeltaT`), fields `U k epsilon nut` at steps 1
//! and 3 from `refs/msfr_th_opt_<variant>.ref.txt.gz`, and every `k`/`epsilon` linear solve of the log.
//!
//! Pass criterion: `max |port - upstream| / max |upstream|` <= 1e-9 per field and step (round 20's gate). Besides: the cell-wise
//! relative maximum and the number of bit-equal values are printed.
//!
//! **Control for every option** (the comparison must FAIL without it): the port reading the BASE case (option absent = upstream's
//! default) against the variant's reference. For `Final`: the port with the stock-OpenFOAM selection switched ON
//! (`with_stock_final_iteration_selection`), which is NOT what GeN-Foam does (README round 29: `customPimpleControl::corr_`
//! shadows the base class's, so `isFinalIteration()` stays false).
//!
//! Predictions: README "Round 29" (committed before the first run); results: README "Round 29 results".

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

/// Stage the root case, overlay `variant` (`None`: the root case with `turbulence true` only), and the polyMesh.
fn stage(tag: &str, variant: Option<&str>) -> (PathBuf, PathBuf) {
    let tmp = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("gh1361_r29_{tag}"));
    let _ = std::fs::remove_dir_all(&tmp);
    let root = tmp.join("case");
    for d in ["constant", "system", "0"] {
        copy_dir(&vv_dir().join("cases/msfr_th").join(d), &root.join(d));
    }
    match variant {
        Some(v) => copy_dir(&vv_dir().join("cases/msfr_th_variants").join(v), &root),
        None => {
            let p = root.join("constant/fluidRegion/turbulenceProperties");
            let text = std::fs::read_to_string(&p).expect("turbulenceProperties");
            std::fs::write(&p, text.replace("turbulence      false;", "turbulence      true;")).expect("write");
        }
    }
    let poly = tmp.join("polyMesh");
    std::fs::create_dir_all(&poly).expect("mkdir");
    let src = vv_dir().join("cases/msfr_neutro/polyMesh");
    for f in ["points", "faces", "owner", "neighbour", "cellZones"] {
        std::fs::write(poly.join(f), gunzip(&src.join(format!("{f}.gz")))).expect("polyMesh");
    }
    std::fs::copy(vv_dir().join("cases/msfr_th/polyMesh/boundary"), poly.join("boundary")).expect("boundary");
    (root, poly)
}

struct Reference {
    fields: HashMap<String, Vec<f64>>,
    times: Vec<String>,
    /// `(kind, step, outer, initial residual, iterations)` of the `EPS`/`K` lines.
    solves: Vec<(String, usize, usize, f64, i64)>,
}

fn read_reference(variant: &str) -> Reference {
    let text = String::from_utf8(gunzip(&vv_dir().join(format!("refs/msfr_th_opt_{variant}.ref.txt.gz")))).expect("utf8");
    let mut r = Reference { fields: HashMap::new(), times: Vec::new(), solves: Vec::new() };
    for line in text.lines() {
        let w: Vec<&str> = line.split_whitespace().collect();
        match w[0] {
            "TIMEPRINT" => r.times = w[1..].iter().map(|s| (*s).to_string()).collect(),
            "F" => {
                r.fields.insert(format!("{} {}", w[2], w[1]), w[3..].iter().map(|x| x.parse().expect("number")).collect());
            }
            "EPS" | "K" => r.solves.push((w[0].to_string(), w[1].parse().unwrap(), w[2].parse().unwrap(), w[3].parse().unwrap(), w[4].parse().unwrap())),
            _ => {}
        }
    }
    r
}

/// Difference of one field to its reference.
#[derive(Clone, Copy, Debug)]
struct Diff {
    /// `max |a - b| / max |b|`.
    max_norm: f64,
    /// `max |a - b| / |b|` over the entries with `b != 0` (a vector: per cell, magnitudes).
    cellwise: f64,
    exact: usize,
    total: usize,
}

fn diff(a: &[f64], b: &[f64], stride: usize) -> Diff {
    assert_eq!(a.len(), b.len());
    let scale = b.iter().fold(0.0_f64, |m, v| m.max(v.abs()));
    let max_norm = a.iter().zip(b).fold(0.0_f64, |m, (x, y)| m.max((x - y).abs())) / scale;
    let mut cellwise = 0.0_f64;
    for (ca, cb) in a.chunks(stride).zip(b.chunks(stride)) {
        let dn = ca.iter().zip(cb).map(|(x, y)| (x - y) * (x - y)).sum::<f64>().sqrt();
        let bn = cb.iter().map(|y| y * y).sum::<f64>().sqrt();
        if bn > 0.0 {
            cellwise = cellwise.max(dn / bn);
        }
    }
    let exact = a.iter().zip(b).filter(|(x, y)| x == y).count();
    Diff { max_norm, cellwise, exact, total: a.len() }
}

/// The four compared fields of one step: `U k epsilon nut`.
struct StepDiff {
    step: usize,
    u: Diff,
    k: Diff,
    epsilon: Diff,
    nut: Diff,
}

impl StepDiff {
    fn worst_max_norm(&self) -> f64 {
        self.u.max_norm.max(self.k.max_norm).max(self.epsilon.max_norm).max(self.nut.max_norm)
    }
}

struct RunResult {
    steps: Vec<StepDiff>,
    /// `(kind, step, outer, initial residual, iterations)` of the port.
    solves: Vec<(String, usize, usize, f64, i64)>,
    /// `U k epsilon nut` of the port at step 3 (for the bit-comparison of two port runs).
    final_fields: Vec<Vec<f64>>,
}

/// Three steps of `solver`, compared with `reference` at its steps 1 and 3.
fn run(solver: &mut OnePhaseCcSolver, reference: &Reference) -> RunResult {
    let mut dt = solver.case().delta_t;
    let mut steps = Vec::new();
    let mut solves = Vec::new();
    let mut final_fields = Vec::new();
    for s in 1..=3usize {
        solver.begin_time_step(dt);
        let (_r, _rep) = solver.correct_physics();
        for (o, pair) in solver.turbulence_solves().expect("solves").iter().enumerate() {
            solves.push(("EPS".to_string(), s, o + 1, pair[0].0, pair[0].1 as i64));
            solves.push(("K".to_string(), s, o + 1, pair[1].0, pair[1].1 as i64));
        }
        if s == 1 || s == 3 {
            let t = &reference.times[if s == 1 { 0 } else { 1 }];
            let u: Vec<f64> = solver.velocity().iter().flat_map(|v| [v.x, v.y, v.z]).collect();
            let k = solver.turbulent_kinetic_energy().expect("k").to_vec();
            let e = solver.turbulent_dissipation().expect("epsilon").to_vec();
            let nut = solver.turbulent_viscosity().expect("nut").to_vec();
            let key = |name: &str| format!("{name} {t}");
            let d = StepDiff {
                step: s,
                u: diff(&u, &reference.fields[&key("U")], 3),
                k: diff(&k, &reference.fields[&key("k")], 1),
                epsilon: diff(&e, &reference.fields[&key("epsilon")], 1),
                nut: diff(&nut, &reference.fields[&key("nut")], 1),
            };
            println!(
                "  step {s}: max-norm U {:.2e} k {:.2e} eps {:.2e} nut {:.2e} | cell-wise U {:.2e} k {:.2e} eps {:.2e} nut {:.2e} | bit-equal eps {}/{} nut {}/{}",
                d.u.max_norm, d.k.max_norm, d.epsilon.max_norm, d.nut.max_norm, d.u.cellwise, d.k.cellwise, d.epsilon.cellwise, d.nut.cellwise,
                d.epsilon.exact, d.epsilon.total, d.nut.exact, d.nut.total
            );
            if s == 3 {
                final_fields = vec![u, k, e, nut];
            }
            steps.push(d);
        }
        dt = adjust_delta_t(true, &[solver.max_delta_t()], 1e-300, dt).expect("adjustable");
    }
    RunResult { steps, solves, final_fields }
}

const GATE: f64 = 1e-9;

fn build(tag: &str, case_variant: Option<&str>) -> OnePhaseCcSolver {
    let (root, poly) = stage(tag, case_variant);
    OnePhaseCcSolver::from_case_dir(&root, &poly).unwrap_or_else(|e| panic!("{tag}: {e}"))
}

/// Port on `variant`'s own case against `variant`'s reference: every field within the gate.
fn assert_matches(variant: &str) -> RunResult {
    println!("{variant}: port on its own case vs the real binary");
    let reference = read_reference(variant);
    let mut solver = build(&format!("pass_{variant}"), Some(variant));
    let res = run(&mut solver, &reference);
    for d in &res.steps {
        assert!(d.worst_max_norm() <= GATE, "{variant} step {} worst {:e}", d.step, d.worst_max_norm());
    }
    res
}

/// The control: the port on `base_case` (option absent) against `variant`'s reference; returns the step-3 `epsilon`, `nut`, `U`, `k` max-norm differences.
fn control(variant: &str, base_case: Option<&str>, tag: &str) -> StepDiff {
    println!("{variant}: CONTROL, port on {} vs the reference of {variant}", base_case.unwrap_or("the shipped-mu root case"));
    let reference = read_reference(variant);
    let mut solver = build(&format!("ctl_{tag}"), base_case);
    let mut res = run(&mut solver, &reference);
    res.steps.pop().expect("step 3")
}

// ---- wall-function variants (mu 0.5) -------------------------------------------------------------------------------------

#[test]
fn wf_base_matches_the_real_binary() {
    assert_matches("wf_base");
}

#[test]
fn lowre_correction_matches_and_the_default_does_not() {
    assert_matches("wf_lowre");
    let c = control("wf_lowre", Some("wf_base"), "lowre");
    assert!(c.epsilon.max_norm > 1e-6, "{:e}", c.epsilon.max_norm);
}

#[test]
fn max_blender_matches_and_the_default_does_not() {
    assert_matches("wf_max");
    let c = control("wf_max", Some("wf_base"), "max");
    assert!(c.epsilon.max_norm > 1e-6, "{:e}", c.epsilon.max_norm);
}

#[test]
fn binomial_blender_with_n_matches_and_the_default_does_not() {
    assert_matches("wf_binomial");
    let c = control("wf_binomial", Some("wf_base"), "binomial");
    assert!(c.epsilon.max_norm > 1e-6, "{:e}", c.epsilon.max_norm);
}

#[test]
fn exponential_blender_matches_and_the_default_does_not() {
    assert_matches("wf_exponential");
    let c = control("wf_exponential", Some("wf_base"), "exponential");
    assert!(c.epsilon.max_norm > 1e-6, "{:e}", c.epsilon.max_norm);
}

#[test]
fn tanh_blender_matches_and_the_default_does_not() {
    assert_matches("wf_tanh");
    let c = control("wf_tanh", Some("wf_base"), "tanh");
    assert!(c.epsilon.max_norm > 1e-6, "{:e}", c.epsilon.max_norm);
}

#[test]
fn a_different_blender_per_patch_matches_and_the_default_does_not() {
    assert_matches("wf_mixed");
    let c = control("wf_mixed", Some("wf_base"), "mixed");
    assert!(c.epsilon.max_norm > 1e-6, "{:e}", c.epsilon.max_norm);
}

#[test]
fn wall_function_coefficients_per_patch_match_and_the_default_does_not() {
    assert_matches("wf_coeffs");
    let c = control("wf_coeffs", Some("wf_base"), "coeffs");
    assert!(c.epsilon.max_norm > 1e-6 && c.nut.max_norm > 1e-9, "{:e} {:e}", c.epsilon.max_norm, c.nut.max_norm);
}

/// `U` is read by `nutkWallFunction::yPlus()` only: upstream with `U UNoSuchField` equals upstream without it to the bit, and so does the port.
#[test]
fn the_u_entry_of_the_nut_wall_function_changes_nothing() {
    let (a, b) = (read_reference("wf_uname"), read_reference("wf_base"));
    assert_eq!(a.fields.len(), b.fields.len());
    for (k, v) in &a.fields {
        assert!(v == &b.fields[k], "upstream: {k} differs between wf_uname and wf_base");
    }
    let res = assert_matches("wf_uname");
    let mut base = build("uname_base", Some("wf_base"));
    let base_res = run(&mut base, &b);
    assert!(res.final_fields == base_res.final_fields, "the port's step-3 fields differ between wf_uname and wf_base");
}

// ---- DhStruct -------------------------------------------------------------------------------------------------------------

#[test]
fn nut_stabilisation_with_the_default_laminar_re_matches_and_the_default_does_not() {
    assert_matches("dh_a");
    let c = control("dh_a", None, "dh_a");
    assert!(c.u.max_norm > 1e-6 || c.nut.cellwise > 1e-3, "U {:e} nut cellwise {:e}", c.u.max_norm, c.nut.cellwise);
}

#[test]
fn nut_stabilisation_with_laminar_re_struct_matches_and_the_default_does_not() {
    assert_matches("dh_b");
    let c = control("dh_b", None, "dh_b");
    assert!(c.u.max_norm > 1e-6 || c.nut.cellwise > 1e-3, "U {:e} nut cellwise {:e}", c.u.max_norm, c.nut.cellwise);
}

// ---- Final entries ----------------------------------------------------------------------------------------------------------

/// Solves: initial residual to 1e-6 relative where it exceeds 1e-9, iteration counts equal.
fn assert_solves_follow(res: &RunResult, reference: &Reference) -> (f64, i64, usize) {
    let (mut worst_res, mut worst_it, mut gated) = (0.0_f64, 0_i64, 0usize);
    for (kind, step, outer, ini, it) in &reference.solves {
        if *step > 3 {
            continue;
        }
        let mine = res.solves.iter().find(|x| &x.0 == kind && x.1 == *step && x.2 == *outer).expect("solve");
        if *ini > 1e-9 {
            worst_res = worst_res.max(((mine.3 - ini) / ini).abs());
            worst_it = worst_it.max((mine.4 - it).abs());
            gated += 1;
        }
    }
    println!("  {gated} solves gated: worst initial-residual relative difference {worst_res:e}, worst iteration difference {worst_it}");
    assert!(worst_res <= 1e-6 && worst_it <= 1, "{worst_res:e} {worst_it}");
    (worst_res, worst_it, gated)
}

/// GeN-Foam never selects `kFinal`/`epsilonFinal` (README round 29): the plain entries run in every outer iteration, and the port does the same.
#[test]
fn final_solver_entries_are_not_selected_by_the_real_binary_and_the_port_follows() {
    let res = assert_matches("fin_solver");
    assert_solves_follow(&res, &read_reference("fin_solver"));
    let c = control_final("fin_solver", "fin_solver_ctl");
    assert!(c.k.max_norm > 1e-9 || c.epsilon.max_norm > 1e-9, "k {:e} eps {:e}", c.k.max_norm, c.epsilon.max_norm);
}

#[test]
fn final_relaxation_entries_are_not_selected_by_the_real_binary_and_the_port_follows() {
    assert_matches("fin_relax");
    let c = control_final("fin_relax", "fin_relax_ctl");
    assert!(c.k.max_norm > 1e-9 || c.epsilon.max_norm > 1e-9, "k {:e} eps {:e}", c.k.max_norm, c.epsilon.max_norm);
}

#[test]
fn a_missing_final_relaxation_entry_is_not_an_unrelaxed_final_iteration_upstream_and_the_port_follows() {
    assert_matches("fin_norelax");
    let c = control_final("fin_norelax", "fin_norelax_ctl");
    assert!(c.k.max_norm > 1e-9 || c.epsilon.max_norm > 1e-9, "k {:e} eps {:e}", c.k.max_norm, c.epsilon.max_norm);
}

/// The control of the `Final` variants: the port with stock OpenFOAM's `finalIteration` selection ON.
fn control_final(variant: &str, tag: &str) -> StepDiff {
    println!("{variant}: CONTROL, port with the stock-OpenFOAM Final selection vs the reference");
    let reference = read_reference(variant);
    let mut solver = build(tag, Some(variant)).with_stock_final_iteration_selection();
    let mut res = run(&mut solver, &reference);
    res.steps.pop().expect("step 3")
}

// ---- refusals ---------------------------------------------------------------------------------------------------------------

fn edit(root: &Path, file: &str, from: &str, to: &str) {
    let p = root.join(file);
    let text = std::fs::read_to_string(&p).expect("read");
    assert!(text.contains(from), "{file}: {from}");
    std::fs::write(&p, text.replacen(from, to, 1)).expect("write");
}

#[test]
fn unknown_blender_and_unported_entries_are_refused_by_name() {
    let (root, poly) = stage("refuse_blender", Some("wf_base"));
    edit(&root, "0/fluidRegion/nut", "type            nutkWallFunction;", "type            nutkWallFunction;\n\tblending smooth;");
    let err = OnePhaseCcSolver::from_case_dir(&root, &poly).expect_err("must refuse").to_string();
    assert!(err.contains("blending"), "{err}");

    let (root, poly) = stage("refuse_entry", Some("wf_base"));
    edit(&root, "0/fluidRegion/epsilon", "type            epsilonWallFunction;", "type            epsilonWallFunction;\n\tfoo 1;");
    let err = OnePhaseCcSolver::from_case_dir(&root, &poly).expect_err("must refuse").to_string();
    assert!(err.contains("foo"), "{err}");

    let (root, poly) = stage("refuse_kappa", Some("wf_base"));
    edit(&root, "0/fluidRegion/nut", "type            nutkWallFunction;", "type            nutkWallFunction;\n\tkappa 0;");
    let err = OnePhaseCcSolver::from_case_dir(&root, &poly).expect_err("must refuse").to_string();
    assert!(err.contains("kappa"), "{err}");

    let (root, poly) = stage("refuse_kqr", Some("wf_base"));
    edit(&root, "0/fluidRegion/k", "type            kqRWallFunction;", "type            kqRWallFunction;\n\tCmu 0.09;");
    let err = OnePhaseCcSolver::from_case_dir(&root, &poly).expect_err("must refuse").to_string();
    assert!(err.contains("Cmu"), "{err}");
}
