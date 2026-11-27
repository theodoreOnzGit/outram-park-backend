// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//! # `legacyThermoMechanics`: `compactNormalStress no` (`fvMatrix::flux`) and the transient `Euler` `d2dt2`, against GeN-Foam's own class (gh:#1845, round 27)
//!
//! AI-written, not human-reviewed. Research, education and V&V only.
//!
//! ## Methodology
//!
//! Upstream: GeN-Foam `652b3da0cc55` on OpenFOAM-ESI v2506, `solvers::legacyThermoMechanics` UNMODIFIED
//! (`libThermoMechanicsSolvers`, `scripts/build_and_run_legacy_tm_dump.sh`), driven by
//! `upstream_driver/legacy_tm_dump/legacyTmDump.C` (`correctPhysics()` once per step, after `runTime++`; in round 27 the
//! driver also writes the step's `deltaT`/`deltaT0` and, with `LEGACY_TM_DT_RATIO`, grows `deltaT` by a factor each step
//! through `Time::setDeltaT`). Case: the round-23 3D_SmallESFR mechanics region (23340 cells, 8 zones, `leastSquares`
//! gradients, `Gauss linear corrected` Laplacians, `nCorrectors 3`, tight protocol 1e-14 / `relTol 0`,
//! `writePrecision 17`), staged by `scripts/make_legacy_tm_case.sh` with the round-27 options:
//!
//! | case | `compactNormalStress` | `d2dt2` | `deltaT` | steps | reference |
//! |---|---|---|---|---|---|
//! | F | `no` | `steadyState` | 1 s | 3 | `refs/legacy_tm_flux_f*` |
//! | E | `yes` | `Euler` | 1e-3 s | 4 | `refs/legacy_tm_d2dt2_e*` |
//! | FE | `no` | `Euler` | 1e-3 s, x1.3 per step from step 2 | 4 | `refs/legacy_tm_flux_d2dt2_fe*` |
//!
//! The port reads the SAME case files (the staged copy of `cases/legacy_tm_esfr` has `compactNormalStress`,
//! `d2dt2Schemes.default` and `controlDict.deltaT` rewritten exactly as the script does), so the case reader is part of
//! the test. The coupling inputs are the closed-form functions of the round-23 driver; each step's `deltaT` is READ from
//! the reference (it is an input that upstream's `Time` produced, not something tuned). The reference holds `disp` at
//! every step and every field (`TStruct`, `fuelDisp`, `CRDisp`, `meshDisp`) at the last step, plus every linear solve's
//! residuals and iteration count, `Max sigmaEq` and `residual_`.
//!
//! Pass criterion: bit-identity (`exact == n`), solve for solve (initial and final residual and iteration count), the
//! round-off allowance of the workspace (1e-9 relative, explained) being used only if a number is printed below.
//!
//! Predictions (committed before the first run) and results: README, "Round 27".

// Reads case files: `std` only (gh:#1633).
#![cfg(feature = "std")]
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use miniz_oxide::inflate::decompress_to_vec;
use outram_foam_appbuilder_lib::error::AppBuilderError;
use outram_foam_appbuilder_lib::genfoam::thermo_mechanics::legacy_fvm::case::{LegacyFvmCase, TStructGamgPolicy};
use outram_foam_appbuilder_lib::genfoam::thermo_mechanics::legacy_fvm::{D2dt2Scheme, PhysicsLog};

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

/// What a round-27 case changes in the staged round-23 case (the options of `make_legacy_tm_case.sh`).
#[derive(Clone, Copy)]
struct Spec {
    compact: bool,
    scheme: &'static str,
    delta_t: &'static str,
    fields: &'static str,
    log: &'static str,
}

const F: Spec = Spec { compact: false, scheme: "steadyState", delta_t: "1", fields: "refs/legacy_tm_flux_f.ref.txt.gz", log: "refs/legacy_tm_flux_f_log.ref.txt.gz" };
const E: Spec = Spec { compact: true, scheme: "Euler", delta_t: "1e-3", fields: "refs/legacy_tm_d2dt2_e.ref.txt.gz", log: "refs/legacy_tm_d2dt2_e_log.ref.txt.gz" };
const FE: Spec = Spec {
    compact: false,
    scheme: "Euler",
    delta_t: "1e-3",
    fields: "refs/legacy_tm_flux_d2dt2_fe.ref.txt.gz",
    log: "refs/legacy_tm_flux_d2dt2_fe_log.ref.txt.gz",
};

fn replace_in(path: &Path, from: &str, to: &str) {
    let text = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    assert!(text.contains(from), "{}: `{from}` not found", path.display());
    std::fs::write(path, text.replacen(from, to, 1)).expect("write");
}

/// Stage `cases/legacy_tm_esfr` with its polyMesh gunzipped and the case selections of `spec` written into the files.
fn stage(tag: &str, spec: Spec) -> PathBuf {
    let root = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("gh1845_{tag}"));
    let _ = std::fs::remove_dir_all(&root);
    copy_dir(&vv_dir().join("cases/legacy_tm_esfr"), &root);
    let poly = root.join("constant/thermoMechanicalRegion/polyMesh");
    for f in ["points", "faces", "owner", "neighbour", "cellZones"] {
        let gz = poly.join(format!("{f}.gz"));
        std::fs::write(poly.join(f), gunzip(&gz)).expect("write polyMesh");
        std::fs::remove_file(gz).expect("rm gz");
    }
    replace_in(&root.join("system/thermoMechanicalRegion/fvSchemes"), "default         steadyState;//Euler;//", &format!("default         {};", spec.scheme));
    replace_in(
        &root.join("system/thermoMechanicalRegion/fvSolution"),
        "compactNormalStress yes;",
        if spec.compact { "compactNormalStress yes;" } else { "compactNormalStress no;" },
    );
    replace_in(&root.join("system/controlDict"), "deltaT          1;", &format!("deltaT          {};", spec.delta_t));
    root
}

/// `F <step> <name> v1 v2 ...` lines.
fn read_fields(path: &Path) -> HashMap<(usize, String), Vec<f64>> {
    let text = String::from_utf8(gunzip(path)).expect("utf8");
    let mut m = HashMap::new();
    for line in text.lines() {
        let mut it = line.split_whitespace();
        if it.next() != Some("F") {
            continue;
        }
        let step: usize = it.next().unwrap().parse().unwrap();
        let name = it.next().unwrap().to_string();
        m.insert((step, name), it.map(|w| w.parse::<f64>().expect("number")).collect());
    }
    m
}

/// The driver's `setCoupling(step)`, expression for expression (as in round 23).
fn coupling(
    mesh: &outram_foam_basic_lib::mesh::FvMesh,
    zones: &[(String, Vec<usize>)],
    step: usize,
) -> (Vec<f64>, Vec<f64>, Vec<f64>) {
    let n = step as f64;
    let nc = mesh.n_cells;
    let mut is_core = vec![false; nc];
    let mut is_por = vec![false; nc];
    for (name, cells) in zones {
        for &c in cells {
            match name.as_str() {
                "innerCore" | "outerCore" => {
                    is_core[c] = true;
                    is_por[c] = true;
                }
                "follower" | "controlRod" => is_por[c] = true,
                _ => {}
            }
        }
    }
    let (mut th, mut fuel, mut pow) = (vec![0.0; nc], vec![0.0; nc], vec![0.0; nc]);
    for c in 0..nc {
        let (x, y, z) = (mesh.cell_centres[c].x, mesh.cell_centres[c].y, mesh.cell_centres[c].z);
        let s = (z + 0.2) / 2.51;
        let r2 = x * x + y * y;
        let shape = 4.0 * s * (1.0 - s);
        th[c] = if is_por[c] { 700.0 + 300.0 * shape + 25.0 * r2 + 10.0 * n * s } else { 0.0 };
        fuel[c] = if is_core[c] { 900.0 + 400.0 * shape * (1.0 - 0.1 * r2) + 30.0 * n * (1.0 - 0.25 * r2) } else { 0.0 };
        pow[c] = 1.0e5 * (1.0 + 0.5 * s + 0.1 * n) * (1.0 + 0.2 * r2);
    }
    (th, fuel, pow)
}

/// Tally of one quantity against its reference.
#[derive(Debug, Default, Clone)]
struct Tally {
    n: usize,
    exact: usize,
    worst_abs: f64,
    worst_rel: f64,
    scale: f64,
}

fn compare(a: &[f64], b: &[f64]) -> Tally {
    assert_eq!(a.len(), b.len());
    let scale = b.iter().fold(0.0_f64, |m, v| m.max(v.abs()));
    let mut t = Tally { n: a.len(), scale, ..Tally::default() };
    for (x, y) in a.iter().zip(b) {
        if x == y {
            t.exact += 1;
        }
        t.worst_abs = t.worst_abs.max((x - y).abs());
    }
    t.worst_rel = if scale > 0.0 { t.worst_abs / scale } else { t.worst_abs };
    t
}

fn flatten(v: &[outram_foam_basic_lib::primitives::Vector3]) -> Vec<f64> {
    v.iter().flat_map(|d| [d.x, d.y, d.z]).collect()
}

struct Run {
    tallies: Vec<(usize, &'static str, Tally)>,
    /// (step, field, initial exact, final exact, iterations equal, rel diff of initial residual)
    solves: Vec<(usize, String, bool, bool, bool, f64)>,
    sigma_eq: Vec<(usize, f64, f64)>,
    residual: Vec<(usize, f64, f64)>,
}

/// Run the port over `n_steps` against `spec`'s reference. `tweak` edits the case after it is read; `use_ref_dt` feeds
/// each step's `deltaT` from the reference (upstream's `Time`), otherwise the case's constant `deltaT` is kept.
fn run(tag: &str, spec: Spec, n_steps: usize, use_ref_dt: bool, tweak: impl Fn(&mut LegacyFvmCase)) -> Run {
    let root = stage(tag, spec);
    let mut case = LegacyFvmCase::read(&root, "thermoMechanicalRegion", TStructGamgPolicy::SubstitutePbicgDilu).expect("case");
    assert_eq!(case.settings.compact_normal_stress, spec.compact, "the reader takes compactNormalStress from the file");
    assert_eq!(case.d2dt2 == D2dt2Scheme::Euler, spec.scheme == "Euler", "the reader takes d2dt2Schemes from the file");
    tweak(&mut case);
    let mut tm = case.build().expect("solver");
    assert_eq!(tm.d2dt2_scheme(), case.d2dt2);
    let refs = read_fields(&vv_dir().join(spec.fields));
    let log_text = String::from_utf8(gunzip(&vv_dir().join(spec.log))).unwrap();
    let mut ref_steps: Vec<Vec<Vec<String>>> = Vec::new();
    for line in log_text.lines() {
        let w: Vec<String> = line.split_whitespace().map(str::to_string).collect();
        if w[0] == "T" {
            ref_steps.push(Vec::new());
        } else {
            ref_steps.last_mut().unwrap().push(w);
        }
    }
    let mut out = Run { tallies: Vec::new(), solves: Vec::new(), sigma_eq: Vec::new(), residual: Vec::new() };
    for step in 1..=n_steps {
        if use_ref_dt {
            let dt = refs[&(step, "deltaT".to_string())][0];
            tm.set_delta_t(dt);
        }
        let (th, fuel, pow) = coupling(&case.mesh, &case.zones, step);
        tm.set_coupling_fields(&th, &fuel, &pow);
        let log: PhysicsLog = tm.correct_physics();
        let mut tally = |name: &'static str, mine: &[f64]| {
            if let Some(theirs) = refs.get(&(step, name.to_string())) {
                out.tallies.push((step, name, compare(mine, theirs)));
            }
        };
        tally("TStruct", tm.t_struct());
        tally("disp", &flatten(tm.displacement()));
        tally("fuelDisp", tm.fuel_displacement());
        tally("CRDisp", tm.control_rod_displacement());
        tally("meshDisp", &flatten(tm.mesh_displacement()));
        let recs = &ref_steps[step - 1];
        let ref_solves: Vec<&Vec<String>> = recs.iter().filter(|r| r[0] == "S").collect();
        // a control run may take a different number of correctors: compare as many solves as both have
        if log.solves.len() != ref_solves.len() {
            out.solves.push((step, "count".to_string(), false, false, false, f64::INFINITY));
        }
        for (mine, theirs) in log.solves.iter().zip(&ref_solves) {
            if mine.field != theirs[1] {
                out.solves.push((step, format!("order {} vs {}", mine.field, theirs[1]), false, false, false, f64::INFINITY));
                continue;
            }
            let (ri, rf, rn): (f64, f64, usize) = (theirs[2].parse().unwrap(), theirs[3].parse().unwrap(), theirs[4].parse().unwrap());
            let rel = if ri != 0.0 { (mine.initial_residual - ri).abs() / ri.abs() } else { (mine.initial_residual - ri).abs() };
            out.solves.push((step, mine.field.to_string(), mine.initial_residual == ri, mine.final_residual == rf, mine.n_iterations == rn, rel));
        }
        for r in recs {
            match r[0].as_str() {
                "M" => out.sigma_eq.push((step, log.max_sigma_eq.expect("sigmaEq"), r[1].parse().unwrap())),
                "R" => out.residual.push((step, log.residual, r[1].parse().unwrap())),
                _ => {}
            }
        }
    }
    out
}

fn report(tag: &str, r: &Run) {
    println!("== {tag}");
    for (step, name, t) in &r.tallies {
        println!(
            "step {step} {name:9}: exact {:6}/{:6}  worst abs {:.3e}  worst rel-to-max {:.3e}  (max |ref| {:.6e})",
            t.exact, t.n, t.worst_abs, t.worst_rel, t.scale
        );
    }
    let ex_i = r.solves.iter().filter(|s| s.2).count();
    let ex_f = r.solves.iter().filter(|s| s.3).count();
    let ex_n = r.solves.iter().filter(|s| s.4).count();
    println!("linear solves: {}; initial residual bit-identical {ex_i}, final {ex_f}, iterations equal {ex_n}", r.solves.len());
    println!("worst relative difference of an initial residual: {:.3e}", r.solves.iter().fold(0.0_f64, |m, s| m.max(s.5)));
    for (step, a, b) in &r.sigma_eq {
        println!("step {step} Max sigmaEq: port {a:.17e} upstream {b:.17e} rel {:.3e}", (a - b).abs() / b.abs());
    }
    for (step, a, b) in &r.residual {
        println!("step {step} residual_: port {a:.17e} upstream {b:.17e}");
    }
    for s in r.solves.iter().filter(|s| !(s.2 && s.3 && s.4)).take(4) {
        println!("  first non-identical solves: step {} {} init-exact {} final-exact {} iters-equal {} rel {:.3e}", s.0, s.1, s.2, s.3, s.4, s.5);
    }
    if let Some((step, name, t)) = r.tallies.iter().find(|(_, _, t)| t.exact != t.n) {
        println!("  first diverging field: step {step} {name}: {} of {} values differ, worst rel {:.3e}", t.n - t.exact, t.n, t.worst_rel);
    }
}

fn assert_bit_identical(r: &Run, n_solves: usize) {
    for (step, name, t) in &r.tallies {
        assert_eq!(t.exact, t.n, "step {step} {name}: {} of {} values differ, worst rel {:.3e}", t.n - t.exact, t.n, t.worst_rel);
    }
    assert!(r.solves.iter().all(|s| s.2 && s.3 && s.4), "a linear solve differs from upstream's");
    assert!(r.sigma_eq.iter().all(|(_, a, b)| a == b), "Max sigmaEq");
    assert!(r.residual.iter().all(|(_, a, b)| a == b), "residual_");
    assert_eq!(r.solves.len(), n_solves);
}

fn worst_disp(r: &Run) -> f64 {
    r.tallies.iter().filter(|(_, n, _)| *n == "disp").map(|(_, _, t)| t.worst_rel).fold(0.0_f64, f64::max)
}

/// `compactNormalStress no`, `steadyState` `d2dt2`, 3 steps: `divSigmaExp = div(DEqn.flux()) + div(sigmaD)` with
/// `fvMatrix::flux`'s `faceH`, boundary and `faceFluxCorrection` parts (README "Round 27").
#[test]
fn flux_form_three_steps_is_bit_identical_to_the_real_class() {
    let r = run("f", F, 3, true, |_| {});
    report("F: compactNormalStress no, steadyState", &r);
    assert_bit_identical(&r, 36);
}

/// `Euler` `d2dt2`, compact form, constant `deltaT = 1e-3`, 4 steps: the old and old-old displacement levels shift
/// once per step; step 1 has `old = oldold = 0`, step 3 is the first with a shifted `oldold`.
#[test]
fn euler_d2dt2_four_steps_is_bit_identical_to_the_real_class() {
    let r = run("e", E, 4, true, |_| {});
    report("E: compactNormalStress yes, Euler d2dt2, constant dt", &r);
    assert_bit_identical(&r, 48);
}

/// Both at once, with `deltaT` growing by 1.3 each step so that `deltaT0 != deltaT` (`coefft`, `coefft00`,
/// `rDeltaT2 = 4/sqr(dt+dt0)` all differ from the constant-step values).
#[test]
fn flux_form_with_euler_d2dt2_and_a_variable_step_is_bit_identical_to_the_real_class() {
    let r = run("fe", FE, 4, true, |_| {});
    report("FE: compactNormalStress no, Euler d2dt2, dt x1.3", &r);
    assert_bit_identical(&r, 48);
}

/// Control (X3): the compact form against the `fvMatrix::flux` reference differs, so F can tell the forms apart.
#[test]
fn control_compact_form_differs_from_the_flux_reference() {
    let r = run("c_f", F, 1, true, |c| c.settings.compact_normal_stress = true);
    report("control: compact form vs flux reference, step 1", &r);
    let worst = worst_disp(&r);
    assert!(worst > 1e-9, "the control must be distinguishable from the reference: {worst:e}");
}

/// Control (X5): `steadyState` against the `Euler` reference differs strongly at step 1 (the inertial regime).
#[test]
fn control_steady_state_differs_from_the_euler_reference() {
    let r = run("c_e", E, 1, true, |c| c.d2dt2 = D2dt2Scheme::SteadyState);
    report("control: steadyState vs Euler reference, step 1", &r);
    let worst = worst_disp(&r);
    assert!(worst > 1e-2, "inertia must change the first step by more than 1 % of max|disp|: {worst:e}");
}

/// Control (X8): ignoring the step schedule (constant `dt`, so `dt0 = dt`) differs from the variable-step reference
/// from step 2 on, so the `deltaT0` handling is exercised.
#[test]
fn control_constant_step_differs_from_the_variable_step_reference() {
    let r = run("c_fe", FE, 3, false, |_| {});
    report("control: constant dt vs variable-step reference, 3 steps", &r);
    let at = |step: usize| r.tallies.iter().filter(|(s, n, _)| *s == step && *n == "disp").map(|(_, _, t)| t.worst_rel).fold(0.0_f64, f64::max);
    assert!(at(2) > 1e-3, "step 2 must differ when the step schedule is ignored: {:e}", at(2));
}

/// The reader refuses a `d2dt2` scheme ESI v2506 does not have (`backward`), by name, and accepts `Euler`.
#[test]
fn unknown_d2dt2_schemes_are_refused_by_name() {
    let root = stage("refuse", E);
    let ok = LegacyFvmCase::read(&root, "thermoMechanicalRegion", TStructGamgPolicy::SubstitutePbicgDilu);
    assert!(ok.is_ok());
    replace_in(&root.join("system/thermoMechanicalRegion/fvSchemes"), "default         Euler;\n}\n\nddtSchemes", "default         backward;\n}\n\nddtSchemes");
    match LegacyFvmCase::read(&root, "thermoMechanicalRegion", TStructGamgPolicy::SubstitutePbicgDilu) {
        Err(AppBuilderError::UnsupportedScheme { family, reason, .. }) => {
            assert_eq!(family, "d2dt2Schemes");
            assert!(reason.contains("no backward"), "{reason}");
        }
        other => panic!("expected a refusal, got {:?}", other.map(|_| ())),
    }
}
