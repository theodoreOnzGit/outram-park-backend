// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//! # `legacyThermoMechanics` on the 3D_SmallESFR mechanics region, against GeN-Foam's own class (gh:#1781, round 23)
//!
//! AI-written, not human-reviewed. Research, education and V&V only.
//!
//! ## Methodology
//!
//! Upstream: GeN-Foam `652b3da0cc55` on OpenFOAM-ESI v2506, `solvers::legacyThermoMechanics` UNMODIFIED, built into
//! `libThermoMechanicsSolvers` by `scripts/build_and_run_legacy_tm_dump.sh` and driven by
//! `upstream_driver/legacy_tm_dump/legacyTmDump.C` (constructed as `meshHandler`/`regionSolvers` do; `correctPhysics()`
//! once per step as `GeN-Foam.C` does). Case: `reactorCases/3D_SmallESFR_NewSolverVerification/legacySolver`,
//! `thermoMechanicalRegion` (23340 cells, 8 cell zones, `leastSquares` gradients, `Gauss linear corrected` Laplacians,
//! `compactNormalStress yes`, `nCorrectors 3`), with the edits of `scripts/make_legacy_tm_case.sh`: the
//! `GFTractionDisplacement` patches (gh:#1235) become `fixedValue`/`zeroGradient`; the eight zone cards differ; every
//! linear solver `tolerance 1e-14`, `relTol 0`, `TStruct` `GAMG` -> `PBiCG`/`DILU` (named deviation); `TStruct` starts at
//! 668 K; `writePrecision 17`.
//!
//! The coupling inputs (`TStructFromTH`, `TFuel`, `powerDensityNeutronics`) are closed-form functions of the cell centre
//! and the step number (`+ - * /` only), evaluated identically here and in the driver; nothing of upstream's output is an
//! input. Reference: `refs/legacy_tm_esfr.ref.txt.gz` (`TStruct`, `disp`, `fuelDisp`, `CRDisp`, `meshDisp` per cell after
//! each of 3 steps, 17 digits) and `refs/legacy_tm_esfr_log.ref.txt.gz` (every linear solve's residuals and iteration
//! count, `Max sigmaEq`, `residual_`).
//!
//! Predictions (committed before the first run) and results: README, "Round 23".

// Reads case files: `std` only (gh:#1633).
#![cfg(feature = "std")]
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use miniz_oxide::inflate::decompress_to_vec;
use outram_foam_appbuilder_lib::genfoam::thermo_mechanics::legacy_fvm::case::{LegacyFvmCase, TStructGamgPolicy};
use outram_foam_appbuilder_lib::genfoam::thermo_mechanics::legacy_fvm::PhysicsLog;

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

/// Stage `cases/legacy_tm_esfr` with its polyMesh gunzipped.
fn stage(tag: &str, shipped: bool) -> PathBuf {
    let root = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("gh1781_{tag}"));
    let _ = std::fs::remove_dir_all(&root);
    copy_dir(&vv_dir().join("cases/legacy_tm_esfr"), &root);
    let poly = root.join("constant/thermoMechanicalRegion/polyMesh");
    for f in ["points", "faces", "owner", "neighbour", "cellZones"] {
        let gz = poly.join(format!("{f}.gz"));
        std::fs::write(poly.join(f), gunzip(&gz)).expect("write polyMesh");
        std::fs::remove_file(gz).expect("rm gz");
    }
    if shipped {
        // the tutorial's own fvSolution: shipped tolerances, `GAMG` for `TStruct`
        std::fs::copy(
            vv_dir().join("cases/legacy_tm_esfr_shipped/system/thermoMechanicalRegion/fvSolution"),
            root.join("system/thermoMechanicalRegion/fvSolution"),
        )
        .expect("shipped fvSolution");
    }
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

/// The driver's `setCoupling(step)`, expression for expression.
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
        let d = (x - y).abs();
        t.worst_abs = t.worst_abs.max(d);
    }
    t.worst_rel = if scale > 0.0 { t.worst_abs / scale } else { t.worst_abs };
    t
}

fn flatten(v: &[outram_foam_basic_lib::primitives::Vector3]) -> Vec<f64> {
    v.iter().flat_map(|d| [d.x, d.y, d.z]).collect()
}

struct Run {
    /// (step, field) -> tally
    tallies: Vec<(usize, &'static str, Tally)>,
    /// (step, solve index) -> (initial exact, final exact, iterations equal, rel diff of initial residual)
    solves: Vec<(usize, String, bool, bool, bool, f64)>,
    sigma_eq: Vec<(usize, f64, f64)>,
    residual: Vec<(usize, f64, f64)>,
}

fn run(tag: &str, n_steps: usize, tweak: impl Fn(&mut LegacyFvmCase)) -> Run {
    run_with(tag, n_steps, Variant::Tight, tweak)
}

/// Which upstream run the port is compared with.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Variant {
    /// `legacyTmDump` driver, tight protocol (`refs/legacy_tm_esfr*`): the coupling fields are imposed.
    Tight,
    /// The driver with the tutorial's own `fvSolution` (shipped tolerances, `GAMG` `TStruct`; `refs/..._shipped*`).
    Shipped,
    /// The REAL `GeN-Foam` executable, single region, tight protocol (`refs/..._binary*`): the coupling fields keep
    /// the constructor's defaults (`TStructFromTH = TFuel = 1`, `powerDensityNeutronics = 0`).
    Binary,
    /// gh:#1936 round 72: 20 steps of the driver, tight protocol except the tutorial's own `GAMG` entry for `TStruct` (shipped options), real
    /// `GAMG` (`refs/legacy_tm_r72_gamg20*`); the port runs `TStructGamgPolicy::Native`.
    Gamg20,
}

fn run_with(tag: &str, n_steps: usize, variant: Variant, tweak: impl Fn(&mut LegacyFvmCase)) -> Run {
    let shipped = variant == Variant::Shipped;
    let root = stage(tag, shipped);
    let (fields_ref, log_ref) = match variant {
        Variant::Tight => ("refs/legacy_tm_esfr.ref.txt.gz", "refs/legacy_tm_esfr_log.ref.txt.gz"),
        Variant::Shipped => ("refs/legacy_tm_esfr_shipped.ref.txt.gz", "refs/legacy_tm_esfr_shipped_log.ref.txt.gz"),
        Variant::Binary => ("refs/legacy_tm_esfr_binary.ref.txt.gz", "refs/legacy_tm_esfr_binary_log.ref.txt.gz"),
        Variant::Gamg20 => ("refs/legacy_tm_r72_gamg20.ref.txt.gz", "refs/legacy_tm_r72_gamg20_log.ref.txt.gz"),
    };
    let policy = if variant == Variant::Gamg20 { TStructGamgPolicy::Native } else { TStructGamgPolicy::SubstitutePbicgDilu };
    if variant == Variant::Gamg20 {
        // the tight `fvSolution` of the driver case with the tutorial's own `TStruct` entry (the edit `run_tm.sh` made for the real run)
        let f = root.join("system/thermoMechanicalRegion/fvSolution");
        let text = std::fs::read_to_string(&f).expect("fvSolution");
        let i = text.find("\"TStruct\"").expect("TStruct entry");
        let j = i + text[i..].find('}').expect("end of entry");
        let gamg = "\"TStruct\"\n    {\n        solver          GAMG;\n        tolerance       1e-06;\n        relTol          0.001;\n        smoother        GaussSeidel;\n        cacheAgglomeration true;\n        nCellsInCoarsestLevel 50;\n        agglomerator    faceAreaPair;\n        mergeLevels     1;\n        processorAgglomerator masterCoarsest;\n    ";
        std::fs::write(&f, format!("{}{}{}", &text[..i], gamg, &text[j..])).expect("write fvSolution");
    }
    let mut case = LegacyFvmCase::read(&root, "thermoMechanicalRegion", policy).expect("case");
    tweak(&mut case);
    let mut tm = case.build().expect("solver");
    let refs = read_fields(&vv_dir().join(fields_ref));
    let log_text = String::from_utf8(gunzip(&vv_dir().join(log_ref))).unwrap();
    // per-step reference records
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
        if variant != Variant::Binary {
            let (th, fuel, pow) = coupling(&case.mesh, &case.zones, step);
            tm.set_coupling_fields(&th, &fuel, &pow);
        }
        let log: PhysicsLog = tm.correct_physics();
        // a field is compared at the steps the reference holds (the shipped and binary references keep fewer, for size)
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
        assert_eq!(log.solves.len(), ref_solves.len(), "step {step}: number of linear solves");
        for (mine, theirs) in log.solves.iter().zip(&ref_solves) {
            assert_eq!(mine.field, theirs[1], "step {step}: solve order");
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

fn report(r: &Run) {
    for (step, name, t) in &r.tallies {
        println!(
            "step {step} {name:9}: exact {:6}/{:6}  worst abs {:.3e}  worst rel-to-max {:.3e}  (max |ref| {:.6e})",
            t.exact, t.n, t.worst_abs, t.worst_rel, t.scale
        );
    }
    let n_solves = r.solves.len();
    let ex_i = r.solves.iter().filter(|s| s.2).count();
    let ex_f = r.solves.iter().filter(|s| s.3).count();
    let ex_n = r.solves.iter().filter(|s| s.4).count();
    println!("linear solves: {n_solves}; initial residual bit-identical {ex_i}, final {ex_f}, iterations equal {ex_n}");
    let worst = r.solves.iter().fold(0.0_f64, |m, s| m.max(s.5));
    println!("worst relative difference of an initial residual: {worst:.3e}");
    for (step, a, b) in &r.sigma_eq {
        println!("step {step} Max sigmaEq: port {a:.17e} upstream {b:.17e} rel {:.3e}", (a - b).abs() / b.abs());
    }
    for (step, a, b) in &r.residual {
        println!("step {step} residual_: port {a:.17e} upstream {b:.17e}");
    }
    for s in r.solves.iter().filter(|s| !(s.2 && s.3 && s.4)).take(6) {
        println!("  first non-identical solves: step {} {} init-exact {} final-exact {} iters-equal {} rel {:.3e}", s.0, s.1, s.2, s.3, s.4, s.5);
    }
}

/// The whole comparison, bit for bit: every field of every step, every linear solve's initial and final residual and
/// iteration count, `Max sigmaEq` and `residual_` (README "Round 23", results: all identical).
#[test]
fn legacy_thermo_mechanics_esfr_three_steps_is_bit_identical_to_the_real_class() {
    let r = run("main", 3, |_| {});
    report(&r);
    for (step, name, t) in &r.tallies {
        assert_eq!(t.exact, t.n, "step {step} {name}: {} of {} values differ, worst rel {:.3e}", t.n - t.exact, t.n, t.worst_rel);
    }
    assert!(r.solves.iter().all(|s| s.2 && s.3 && s.4), "a linear solve differs from upstream's");
    assert!(r.sigma_eq.iter().all(|(_, a, b)| a == b), "Max sigmaEq");
    assert!(r.residual.iter().all(|(_, a, b)| a == b), "residual_");
    assert_eq!(r.solves.len(), 36);
}

/// Control: the tally CAN fail. The same run with `Gauss linear` gradients instead of the case's `leastSquares` (the
/// scheme the explicit non-orthogonal correction and `grad(disp)` use) differs from the reference, so agreement above is
/// not an artefact of the comparison; the size of the difference is the ablation of the gradient scheme.
#[test]
fn control_gauss_linear_gradients_differ_from_the_least_squares_reference() {
    use outram_foam_appbuilder_lib::io::fv_schemes::GradScheme;
    let r = run("ctl_grad", 1, |c| c.settings.grad = GradScheme::GaussLinear);
    report(&r);
    let worst = r.tallies.iter().filter(|(_, n, _)| *n == "disp").map(|(_, _, t)| t.worst_rel).fold(0.0_f64, f64::max);
    assert!(worst > 1e-8, "the control must be distinguishable from the reference: {worst:e}");
}

/// Control: dropping the non-orthogonal correction (`uncorrected`) changes the answer, so the correction is exercised.
#[test]
fn control_uncorrected_laplacian_differs_from_the_corrected_reference() {
    use outram_foam_appbuilder_lib::solvers::non_orthogonal::LaplacianCorrection;
    let r = run("ctl_lap", 1, |c| c.settings.laplacian = LaplacianCorrection::Uncorrected);
    report(&r);
    let worst = r.tallies.iter().filter(|(_, n, _)| *n == "disp").map(|(_, _, t)| t.worst_rel).fold(0.0_f64, f64::max);
    assert!(worst > 1e-8, "the control must be distinguishable from the reference: {worst:e}");
}

/// The SHIPPED solver settings of the tutorial (`PCG` `1e-6`/`relTol 1e-3` for `disp`, `PBiCG` `1e-5`/`0.01` for the
/// expansions, `GAMG` `1e-6`/`0.001` for `TStruct`), where the port runs `PBiCG`/`DILU` for `TStruct` (the named
/// deviation, `TStructSolver::GamgSubstitutedByPbicgDilu`). Measures what the substitution and the loose tolerance cost.
/// Prints every tally; gates are the README's prediction Q6 as measured (see "Round 23 results").
#[test]
fn shipped_tolerances_measure_the_gamg_substitution() {
    let case_root = stage("shipped_probe", true);
    let c = LegacyFvmCase::read(&case_root, "thermoMechanicalRegion", TStructGamgPolicy::SubstitutePbicgDilu).expect("case");
    assert!(c.settings.t_solver_kind == outram_foam_appbuilder_lib::genfoam::thermo_mechanics::legacy_fvm::TStructSolver::GamgSubstitutedByPbicgDilu);
    assert!(c.build().expect("solver").t_struct_gamg_substituted());
    let r = run_with("shipped", 2, Variant::Shipped, |_| {});
    report(&r);
    let step1_t = r.tallies.iter().find(|(s, n, _)| *s == 1 && *n == "TStruct").map(|(_, _, t)| t.clone()).unwrap();
    // |T^1 - T^0| is at most max|T| - 668 ~ 381 K here; Q6: within 1e-3 of the step change
    assert!(step1_t.worst_abs < 1e-3 * 400.0, "TStruct: worst abs {:.3e} K", step1_t.worst_abs);
}

/// The REAL, unmodified `GeN-Foam` executable (`regionSolvers { thermoMechanicalRegion legacyThermoMechanics; }`, the
/// library loaded through `controlDict` `libs`), single region, 3 steps, tight protocol, the fields it WROTE at
/// `writePrecision 17` (`scripts/extract_legacy_tm_written.sh`). With no other region the coupling fields keep the
/// constructor's defaults, which is what the port starts from too: `TStruct` is forced to 1 K, `fuelDisp`/`CRDisp`
/// are driven by `TFuel = TStruct = 1` below their reference temperatures, and the displacement by the zone-wise
/// `threeKalpha (T - Tref)`. Steps 1 and 3 are kept in the reference. Every linear solve's residuals and iteration
/// count are compared too.
#[test]
fn real_geni_foam_executable_single_region_is_bit_identical() {
    let r = run_with("binary", 3, Variant::Binary, |_| {});
    report(&r);
    assert_eq!(r.tallies.len(), 10, "steps 1 and 3 of five fields");
    for (step, name, t) in &r.tallies {
        assert_eq!(t.exact, t.n, "step {step} {name}: {} of {} values differ, worst rel {:.3e}", t.n - t.exact, t.n, t.worst_rel);
    }
    assert!(r.solves.iter().all(|s| s.2 && s.3 && s.4), "a linear solve differs from the executable's");
    assert_eq!(r.solves.len(), 36);
    // `Max sigmaEq` is printed with 14 significant digits by the executable
    assert!(r.sigma_eq.iter().all(|(_, a, b)| (a - b).abs() <= 1e-13 * b.abs()), "Max sigmaEq");
}

/// gh:#1936 round 72, criterion R-tm: 20 steps of the legacy thermo-mechanics region with upstream's own `GAMG` for `TStruct` (`GaussSeidel`,
/// asymmetric path), against the real class: no first diverging step; `Max sigmaEq` and `residual_` of every step and every field at the kept
/// steps (`disp` at 1, 2, 5, 10, 15, 20; everything at 20) within 1e-9 of the maximum; iteration counts and initial residuals REPORTED.
#[test]
fn r72_twenty_steps_with_the_real_gamg_for_tstruct_match_the_real_class() {
    let r = run_with("r72_gamg20", 20, Variant::Gamg20, |_| {});
    report(&r);
    let worst_field = r.tallies.iter().map(|(_, _, t)| t.worst_rel).fold(0.0_f64, f64::max);
    let n_exact = r.tallies.iter().filter(|(_, _, t)| t.exact == t.n).count();
    let worst_sigma = r.sigma_eq.iter().fold(0.0_f64, |m, (_, a, b)| m.max((a - b).abs() / b.abs()));
    let worst_res = r.residual.iter().fold(0.0_f64, |m, (_, a, b)| m.max((a - b).abs() / b.abs()));
    let t_struct: Vec<_> = r.solves.iter().filter(|s| s.1 == "TStruct").collect();
    let t_exact = t_struct.iter().filter(|s| s.2 && s.3 && s.4).count();
    println!("R-tm: {} field tallies, {n_exact} bit-identical, worst field difference {worst_field:e} of max; worst Max sigmaEq {worst_sigma:e}, worst residual_ {worst_res:e}; TStruct solves bit-identical (initial, final, iterations) {t_exact} of {}", r.tallies.len(), t_struct.len());
    assert!(worst_field <= 1e-9, "{worst_field:e}");
    assert!(worst_sigma <= 1e-9 && worst_res <= 1e-9, "{worst_sigma:e} {worst_res:e}");
    assert_eq!(r.sigma_eq.len(), 20);
}

/// Control for R-tm: the substitute (`PBiCG`/`DILU` at the same shipped `TStruct` tolerances) is distinguishable from the real `GAMG` run: its
/// `TStruct` solve log differs (iteration counts or residuals) in at least one step.
#[test]
fn r72_control_the_pbicg_substitute_differs_from_the_real_gamg_log() {
    let root = stage("r72_ctl", false);
    let f = root.join("system/thermoMechanicalRegion/fvSolution");
    let text = std::fs::read_to_string(&f).expect("fvSolution");
    let i = text.find("\"TStruct\"").expect("TStruct entry");
    let j = i + text[i..].find('}').expect("end of entry");
    let sub = "\"TStruct\"\n    {\n        solver          GAMG;\n        tolerance       1e-06;\n        relTol          0.001;\n        smoother        GaussSeidel;\n        nCellsInCoarsestLevel 50;\n    ";
    std::fs::write(&f, format!("{}{}{}", &text[..i], sub, &text[j..])).expect("write");
    let mut case = LegacyFvmCase::read(&root, "thermoMechanicalRegion", TStructGamgPolicy::SubstitutePbicgDilu).expect("case");
    case.t_gamg = None;
    let mut tm = case.build().expect("solver");
    assert!(tm.t_struct_gamg_substituted());
    let log_text = String::from_utf8(gunzip(&vv_dir().join("refs/legacy_tm_r72_gamg20_log.ref.txt.gz"))).unwrap();
    let up: Vec<(f64, usize)> = log_text.lines().filter(|l| l.starts_with("S TStruct")).map(|l| { let w: Vec<&str> = l.split_whitespace().collect(); (w[3].parse().unwrap(), w[4].parse().unwrap()) }).collect();
    let mut differ = 0;
    for step in 1..=3 {
        let (th, fuel, pow) = coupling(&case.mesh, &case.zones, step);
        tm.set_coupling_fields(&th, &fuel, &pow);
        let log = tm.correct_physics();
        let mine = log.solves.iter().find(|s| s.field == "TStruct").expect("TStruct solve");
        println!("control step {step}: substitute final residual {:e} in {} iterations; real GAMG {:e} in {}", mine.final_residual, mine.n_iterations, up[step - 1].0, up[step - 1].1);
        if mine.final_residual.to_bits() != up[step - 1].0.to_bits() || mine.n_iterations != up[step - 1].1 {
            differ += 1;
        }
    }
    assert!(differ >= 1);
}
