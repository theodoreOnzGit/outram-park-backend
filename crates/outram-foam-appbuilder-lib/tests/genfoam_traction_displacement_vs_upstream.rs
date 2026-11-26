// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//! # `GFTractionDisplacement` against GeN-Foam's own class (gh:#1235, round 24)
//!
//! AI-written, not human-reviewed. Research, education and V&V only.
//!
//! ## Methodology
//!
//! Same upstream, driver, case, zone cards, coupling functions and tight protocol as
//! `genfoam_legacy_thermo_mechanics_vs_upstream.rs` (round 23: GeN-Foam `652b3da` on ESI v2506,
//! `solvers::legacyThermoMechanics` unmodified, `libThermoMechanicsSolvers` including
//! `GFTractionDisplacementFvPatchVectorField`), with these differences, all stated in README "Round 24":
//!
//! * T0: `0/disp` is the tutorial's OWN file (five `GFTractionDisplacement` patches, zero traction and pressure);
//!   T1: loaded (nonzero traction and pressure, per-face lists on `bottom` and `topCR`); T2: T1 with `planeStress yes`.
//! * `maxIter 100000` for every linear solver (the released bottom makes `disp` far worse conditioned).
//! * the driver also dumps the boundary `value` and `gradient` of every `GFTractionDisplacement` patch.
//!
//! References: `refs/legacy_tm_traction_{t0,t1,t2}{,_log}.ref.txt.gz` (`disp` every step, `meshDisp` step 3, patch
//! `value`/`gradient` steps 1 and 3, every linear solve, `Max sigmaEq`, `residual_`; 17 digits).
//! Pass criterion: bit identity (the controls must differ). Results: README "Round 24 results".
//!
//! `gapContact` has no differential test (needs `regionCoupled` two-region coupling, gh:#1846); its arithmetic is
//! unit-tested in `legacy_fvm::tests`.

// Reads case files: `std` only (gh:#1633).
#![cfg(feature = "std")]
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use miniz_oxide::inflate::decompress_to_vec;
use outram_foam_appbuilder_lib::genfoam::thermo_mechanics::legacy_fvm::case::{LegacyFvmCase, TStructGamgPolicy};
use outram_foam_appbuilder_lib::genfoam::thermo_mechanics::legacy_fvm::traction::GfTractionDisplacement;
use outram_foam_appbuilder_lib::genfoam::thermo_mechanics::legacy_fvm::{DispPatch, PhysicsLog};
use outram_foam_basic_lib::primitives::Vector3;

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

#[derive(Debug, Default, Clone)]
struct Tally {
    n: usize,
    exact: usize,
    worst_abs: f64,
    worst_rel: f64,
    scale: f64,
    first_diff: Option<usize>,
}

fn compare(a: &[f64], b: &[f64]) -> Tally {
    assert_eq!(a.len(), b.len());
    let scale = b.iter().fold(0.0_f64, |m, v| m.max(v.abs()));
    let mut t = Tally { n: a.len(), scale, ..Tally::default() };
    for (i, (x, y)) in a.iter().zip(b).enumerate() {
        if x == y {
            t.exact += 1;
        } else if t.first_diff.is_none() {
            t.first_diff = Some(i);
        }
        t.worst_abs = t.worst_abs.max((x - y).abs());
    }
    t.worst_rel = if scale > 0.0 { t.worst_abs / scale } else { t.worst_abs };
    t
}

fn flatten(v: &[Vector3]) -> Vec<f64> {
    v.iter().flat_map(|d| [d.x, d.y, d.z]).collect()
}

/// The three differential cases of round 24.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum TCase {
    /// The tutorial's OWN `0/disp` (zero traction and pressure).
    T0,
    /// Loaded: nonzero traction and pressure, per-face lists on `bottom` and `topCR`.
    T1,
    /// T1 with `planeStress yes` (one step).
    T2,
}

impl TCase {
    fn tag(self) -> &'static str {
        match self {
            TCase::T0 => "t0",
            TCase::T1 => "t1",
            TCase::T2 => "t2",
        }
    }
}

/// Stage the ESFR case with the traction `disp`, `maxIter 100000`.
fn stage_traction(tag: &str, tc: TCase) -> PathBuf {
    let root = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("gh1235_{tag}"));
    let _ = std::fs::remove_dir_all(&root);
    copy_dir(&vv_dir().join("cases/legacy_tm_esfr"), &root);
    let poly = root.join("constant/thermoMechanicalRegion/polyMesh");
    for f in ["points", "faces", "owner", "neighbour", "cellZones"] {
        let gz = poly.join(format!("{f}.gz"));
        std::fs::write(poly.join(f), gunzip(&gz)).expect("write polyMesh");
        std::fs::remove_file(gz).expect("rm gz");
    }
    let t = vv_dir().join("cases/legacy_tm_esfr_traction");
    let disp = if tc == TCase::T0 { "disp.t0" } else { "disp.t1" };
    std::fs::copy(t.join(disp), root.join("0/thermoMechanicalRegion/disp")).expect("disp");
    std::fs::copy(t.join("fvSolution"), root.join("system/thermoMechanicalRegion/fvSolution")).expect("fvSolution");
    if tc == TCase::T2 {
        let p = root.join("constant/thermoMechanicalRegion/thermoMechanicalProperties");
        let text = std::fs::read_to_string(&p).expect("properties");
        assert!(text.contains("planeStress \tno;"));
        std::fs::write(&p, text.replace("planeStress \tno;", "planeStress \tyes;")).expect("write");
    }
    root
}

struct TRun {
    tallies: Vec<(usize, String, Tally)>,
    /// (step, field, initial identical, final identical, iterations equal, iterations port, iterations upstream)
    solves: Vec<(usize, String, bool, bool, bool, usize, usize)>,
    sigma_eq: Vec<(usize, f64, f64)>,
    residual: Vec<(usize, f64, f64)>,
}

fn run_traction(tag: &str, tc: TCase, n_steps: usize, tweak: impl Fn(&mut LegacyFvmCase)) -> TRun {
    let root = stage_traction(tag, tc);
    let mut case = LegacyFvmCase::read(&root, "thermoMechanicalRegion", TStructGamgPolicy::SubstitutePbicgDilu).expect("case");
    tweak(&mut case);
    let mut tm = case.build().expect("solver");
    let t = tc.tag();
    let refs = read_fields(&vv_dir().join(format!("refs/legacy_tm_traction_{t}.ref.txt.gz")));
    let log_text = String::from_utf8(gunzip(&vv_dir().join(format!("refs/legacy_tm_traction_{t}_log.ref.txt.gz")))).unwrap();
    let mut ref_steps: Vec<Vec<Vec<String>>> = Vec::new();
    for line in log_text.lines() {
        let w: Vec<String> = line.split_whitespace().map(str::to_string).collect();
        if w[0] == "T" {
            ref_steps.push(Vec::new());
        } else {
            ref_steps.last_mut().unwrap().push(w);
        }
    }
    let mut out = TRun { tallies: Vec::new(), solves: Vec::new(), sigma_eq: Vec::new(), residual: Vec::new() };
    for step in 1..=n_steps {
        let (th, fuel, pow) = coupling(&case.mesh, &case.zones, step);
        tm.set_coupling_fields(&th, &fuel, &pow);
        let log: PhysicsLog = tm.correct_physics();
        let mut tally = |name: &str, mine: &[f64]| {
            if let Some(theirs) = refs.get(&(step, name.to_string())) {
                out.tallies.push((step, name.to_string(), compare(mine, theirs)));
            }
        };
        tally("disp", &flatten(tm.displacement()));
        tally("meshDisp", &flatten(tm.mesh_displacement()));
        for (pi, p) in case.mesh.patches.iter().enumerate() {
            if let DispPatch::GfTractionDisplacement(g) = tm.disp_patch(pi) {
                tally(&format!("gradB_{}", p.name), &flatten(g.gradient()));
                tally(&format!("dispB_{}", p.name), &flatten(&tm.displacement_boundary()[pi]));
            }
        }
        let recs = &ref_steps[step - 1];
        let ref_solves: Vec<&Vec<String>> = recs.iter().filter(|r| r[0] == "S").collect();
        assert_eq!(log.solves.len(), ref_solves.len(), "step {step}: number of linear solves");
        for (mine, theirs) in log.solves.iter().zip(&ref_solves) {
            assert_eq!(mine.field, theirs[1], "step {step}: solve order");
            let (ri, rf, rn): (f64, f64, usize) = (theirs[2].parse().unwrap(), theirs[3].parse().unwrap(), theirs[4].parse().unwrap());
            out.solves.push((
                step,
                mine.field.to_string(),
                mine.initial_residual == ri,
                mine.final_residual == rf,
                mine.n_iterations == rn,
                mine.n_iterations,
                rn,
            ));
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

fn report(r: &TRun) {
    for (step, name, t) in &r.tallies {
        println!(
            "step {step} {name:20}: exact {:6}/{:6}  worst abs {:.3e}  worst rel-to-max {:.3e}  (max |ref| {:.6e}) first diff {:?}",
            t.exact, t.n, t.worst_abs, t.worst_rel, t.scale, t.first_diff
        );
    }
    let ex = |f: fn(&(usize, String, bool, bool, bool, usize, usize)) -> bool| r.solves.iter().filter(|s| f(s)).count();
    println!(
        "linear solves {}: initial identical {}, final identical {}, iterations equal {}",
        r.solves.len(),
        ex(|s| s.2),
        ex(|s| s.3),
        ex(|s| s.4)
    );
    for s in r.solves.iter().filter(|s| s.1.starts_with("disp")).take(3) {
        println!("  step {} {} iterations port {} upstream {}", s.0, s.1, s.5, s.6);
    }
    for s in r.solves.iter().filter(|s| !(s.2 && s.3 && s.4)).take(4) {
        println!("  first non-identical solve: step {} {} init {} final {} iters {} ({} vs {})", s.0, s.1, s.2, s.3, s.4, s.5, s.6);
    }
    for (step, a, b) in &r.sigma_eq {
        println!("step {step} Max sigmaEq: port {a:.17e} upstream {b:.17e}");
    }
}

fn assert_identical(r: &TRun) {
    for (step, name, t) in &r.tallies {
        assert_eq!(t.exact, t.n, "step {step} {name}: {} of {} values differ, worst rel {:.3e}", t.n - t.exact, t.n, t.worst_rel);
    }
    assert!(r.solves.iter().all(|s| s.2 && s.3 && s.4), "a linear solve differs from upstream's");
    assert!(r.sigma_eq.iter().all(|(_, a, b)| a == b), "Max sigmaEq");
    assert!(r.residual.iter().all(|(_, a, b)| a == b), "residual_");
}

fn worst_disp(r: &TRun, step: usize) -> f64 {
    r.tallies.iter().filter(|(s, n, _)| *s == step && n == "disp").map(|(_, _, t)| t.worst_rel).fold(0.0_f64, f64::max)
}

/// T0: the tutorial's own `GFTractionDisplacement` patches (zero traction and pressure), 3 steps: `disp`, `meshDisp`
/// (step 3), boundary value and gradient of all five patches (steps 1 and 3), every linear solve, `Max sigmaEq`,
/// `residual_`.
#[test]
fn gf_traction_displacement_tutorial_form_matches_the_real_class() {
    let r = run_traction("tr_t0", TCase::T0, 3, |_| {});
    report(&r);
    assert_eq!(r.solves.len(), 36);
    assert!(r.tallies.len() >= 3 + 1 + 2 * 10, "disp x3, meshDisp, 5 patches x (gradient, value) x 2 steps");
    assert_identical(&r);
}

/// T1: loaded (nonzero traction and pressure, per-face lists on `bottom` and `topCR`).
#[test]
fn gf_traction_displacement_loaded_matches_the_real_class() {
    let r = run_traction("tr_t1", TCase::T1, 3, |_| {});
    report(&r);
    assert_eq!(r.solves.len(), 36);
    assert_identical(&r);
}

/// T2: T1 with `planeStress yes`: upstream's BC stays plane strain (misspelled lookup); the port reproduces that.
#[test]
fn gf_traction_displacement_ignores_plane_stress_like_upstream() {
    let r = run_traction("tr_t2", TCase::T2, 1, |c| assert!(c.settings.plane_stress));
    report(&r);
    assert_identical(&r);
}

/// Control (a): round 23's replacement (`fixedValue 0` on `zeroDispl`, `bottom`, `bottomCR`; `zeroGradient` elsewhere)
/// differs from the T0 reference, so the comparison can fail.
#[test]
fn control_round_23_replacement_differs_from_the_traction_reference() {
    let r = run_traction("tr_ctl_a", TCase::T0, 1, |c| {
        for (pi, p) in c.mesh.patches.iter().enumerate() {
            c.disp_patches[pi] = match p.name.as_str() {
                "zeroDispl" | "bottom" | "bottomCR" => DispPatch::FixedValue(Vector3::ZERO),
                _ => DispPatch::ZeroGradient,
            };
        }
    });
    let w = worst_disp(&r, 1);
    println!("control (a): worst disp difference {w:.3e} of max |disp|");
    assert!(w > 1e-2, "{w:e}");
}

fn zeroed(c: &mut LegacyFvmCase, traction: bool, pressure: bool) {
    for p in c.disp_patches.iter_mut() {
        if let DispPatch::GfTractionDisplacement(g) = p {
            let t = if traction { vec![Vector3::ZERO; g.len()] } else { g.traction().to_vec() };
            let q = if pressure { vec![0.0; g.len()] } else { g.pressure().to_vec() };
            *g = GfTractionDisplacement::new(t, q).expect("same length");
        }
    }
}

/// Controls (b) and (c): zero `pressure`, zero `traction` in the port differ from the T1 reference.
#[test]
fn control_dropping_pressure_or_traction_differs_from_the_loaded_reference() {
    let rp = run_traction("tr_ctl_b", TCase::T1, 1, |c| zeroed(c, false, true));
    let rt = run_traction("tr_ctl_c", TCase::T1, 1, |c| zeroed(c, true, false));
    let (wp, wt) = (worst_disp(&rp, 1), worst_disp(&rt, 1));
    println!("control (b) pressure dropped: {wp:.3e}; control (c) traction dropped: {wt:.3e}");
    assert!(wp > 1e-6 && wt > 1e-6, "{wp:e} {wt:e}");
}

/// Control (d): the INTENDED plane-stress lookup (not upstream's) differs from the T2 reference.
#[test]
fn control_honouring_plane_stress_in_the_bc_differs_from_upstream() {
    let r = run_traction("tr_ctl_d", TCase::T2, 1, |c| c.settings.traction_bc_honours_plane_stress = true);
    let w = worst_disp(&r, 1);
    println!("control (d): worst disp difference {w:.3e}");
    assert!(w > 1e-6, "{w:e}");
}

/// `gapContact` in a case and a `rheologyOptions` key are refused by name.
#[test]
fn gap_contact_and_rheology_options_are_refused_by_name() {
    let root = stage_traction("tr_refuse", TCase::T0);
    let disp = root.join("0/thermoMechanicalRegion/disp");
    let text = std::fs::read_to_string(&disp).unwrap();
    std::fs::write(&disp, text.replacen("GFTractionDisplacement", "gapContact", 1)).unwrap();
    let e = LegacyFvmCase::read(&root, "thermoMechanicalRegion", TStructGamgPolicy::SubstitutePbicgDilu).unwrap_err();
    assert!(format!("{e:?}").contains("gapContact"), "{e:?}");
    std::fs::write(&disp, text).unwrap();
    let p = root.join("constant/thermoMechanicalRegion/thermoMechanicalProperties");
    let props = std::fs::read_to_string(&p).unwrap();
    assert!(props.contains("rheologyOption\n"));
    std::fs::write(&p, props.replace("rheologyOption\n", "rheologyOptions\n")).unwrap();
    let e = LegacyFvmCase::read(&root, "thermoMechanicalRegion", TStructGamgPolicy::SubstitutePbicgDilu).unwrap_err();
    assert!(format!("{e:?}").contains("rheologyOptions"), "{e:?}");
}
