// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//! # `reactorCases/3D_HTR-10` against the real GeN-Foam binary (gh:#1876, README round 87)
//!
//! AI-written, not human-reviewed. Research, education and V&V only.
//!
//! ## Methodology
//!
//! The case is the shipped `rootCase` (337 675 tetrahedra, one `fluidRegion`, `onePhase` `faceCentered`, helium `perfectGas` +
//! `hPolynomial` + `polynomial`, `porousKEpsilon`, `nuclearSteadyStatePebble`) with ONE scheme edit and the tight protocol:
//! `laplacianSchemes Gauss linear uncorrected`, `snGradSchemes uncorrected` (the shipped `corrected` is not ported: gh:#1876
//! remainder), every solver `tolerance 1e-14`, `relTol 0`, `p_rgh` `PCG`/`DIC` `maxIter 5000` (the shipped `GAMG` stalls near
//! 1e-10 on this mesh in the real binary: 1000 iterations, README round 87), `writePrecision 17`, `endTime 0.55`,
//! `writeControl timeStep`, `purgeWrite 1`. It is staged from the refs submodule (`refs/htr10_r87/case`, written by
//! `scripts/make_htr10_r87_refs.py`), where the real binary (GeN-Foam 652b3da on OpenFOAM-ESI v2506, serial) ran on the same
//! files. References: each run's log and every 7th cell of the written fields (`%.17g`), compared by the max-norm difference
//! over the reference's max-norm. Variants: `ut2` (the same case, one step), `vlam` (one step, `simulationType laminar`).
//!
//! Gate: every compared field and every `T.nuclearSteadyStatePebble.<X> (avg min max)` number <= 1e-9 relative; every `deltaT`
//! <= 1e-9 relative. Bit-identity is the goal; the residual differences come from the 1e-14 solver tolerance (the solutions are
//! converged only to it, and the two codes stop the CG iterations at different counts on the near-floor second passes).
//!
//! ## Results (measured 2026-11-28)
//!
//! Printed by `--nocapture`; README "Round 87 results" records them.
#![cfg(feature = "std")]
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use miniz_oxide::inflate::decompress_to_vec;

use outram_foam_appbuilder_lib::genfoam::multi_region::coupled_loop::adjust_delta_t;
use outram_foam_appbuilder_lib::genfoam::thermal_hydraulics::solver::two_phase::one_phase_cc::{CaseReadOptions, OnePhaseCcCase, OnePhaseCcSolver};
use outram_foam_appbuilder_lib::genfoam::thermal_hydraulics::structure::nuclear_steady_state_pebble::coupled::NuclearSteadyStatePebble;
use outram_foam_appbuilder_lib::genfoam::thermal_hydraulics::structure::radial_power_model::RadialPowerModel;

const GATE: f64 = 1e-9;

fn refs_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("verification_and_validation/genfoam_vs_upstream_gh1361/refs/htr10_r87")
}

/// A case directory named by an environment variable, or `None`.
fn env_case(var: &str) -> Option<PathBuf> {
    match std::env::var(var) {
        Ok(s) => Some(PathBuf::from(s)),
        Err(_) => None,
    }
}

fn gunzip(path: &Path) -> Vec<u8> {
    let bytes = match std::fs::read(path) {
        Ok(b) => b,
        Err(e) => panic!("read {}: {e}", path.display()),
    };
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
    match decompress_to_vec(&bytes[at..bytes.len() - 8]) {
        Ok(v) => v,
        Err(_) => panic!("inflate {}", path.display()),
    }
}

/// Copy `src` to `dst`, inflating every `*.gz` (the case of the refs submodule).
fn stage_dir(src: &Path, dst: &Path) {
    if let Err(e) = std::fs::create_dir_all(dst) {
        panic!("mkdir {}: {e}", dst.display());
    }
    let entries = match std::fs::read_dir(src) {
        Ok(r) => r,
        Err(e) => panic!("read_dir {}: {e}", src.display()),
    };
    for e in entries {
        let e = match e {
            Ok(e) => e,
            Err(err) => panic!("entry: {err}"),
        };
        let s = e.path();
        let name = e.file_name().to_string_lossy().to_string();
        if s.is_dir() {
            stage_dir(&s, &dst.join(&name));
        } else if let Some(stem) = name.strip_suffix(".gz") {
            if let Err(err) = std::fs::write(dst.join(stem), gunzip(&s)) {
                panic!("write {stem}: {err}");
            }
        } else if let Err(err) = std::fs::copy(&s, dst.join(&name)) {
            panic!("copy {name}: {err}");
        }
    }
}

/// A scratch copy of `refs/htr10_r87/case` at a path unique to the call (the tag and the process id).
fn stage(tag: &str) -> PathBuf {
    let root = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("gh1876_r87_{tag}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    stage_dir(&refs_dir().join("case"), &root);
    root
}

/// `max |a - b| / max |b|` (the absolute difference where `b` is all zero).
fn max_norm(a: &[f64], b: &[f64]) -> f64 {
    assert_eq!(a.len(), b.len());
    let scale = b.iter().fold(0.0_f64, |m, v| m.max(v.abs()));
    let d = a.iter().zip(b).fold(0.0_f64, |m, (x, y)| m.max((x - y).abs()));
    if scale == 0.0 {
        d
    } else {
        d / scale
    }
}

/// The values of a `<tag>_<field>_s7.txt.gz` reference (a vector: its three components in a row).
fn sample_ref(tag: &str, field: &str) -> Option<Vec<f64>> {
    let p = refs_dir().join(format!("{tag}_{field}_s7.txt.gz"));
    if !p.exists() {
        return None;
    }
    let text = String::from_utf8_lossy(&gunzip(&p)).to_string();
    let mut out = Vec::new();
    for tok in text.split_whitespace() {
        match tok.parse::<f64>() {
            Ok(v) => out.push(v),
            Err(_) => panic!("{tag}_{field}: `{tok}`"),
        }
    }
    Some(out)
}

/// The real log of a reference run.
fn real_log(tag: &str) -> String {
    String::from_utf8_lossy(&gunzip(&refs_dir().join(format!("{tag}_log.gz")))).to_string()
}

/// The numbers after `key` on every line of `log` that starts with it.
fn log_numbers(log: &str, key: &str) -> Vec<Vec<f64>> {
    log.lines()
        .filter_map(|l| l.strip_prefix(key))
        .map(|rest| rest.split(|c: char| c.is_whitespace() || c == ',').filter_map(|x| x.parse::<f64>().ok()).collect())
        .collect()
}

/// Every 7th value of a cell field.
fn every_7th(v: &[f64]) -> Vec<f64> {
    v.iter().step_by(7).copied().collect()
}

/// The pebble model of the solver.
fn pebble(solver: &OnePhaseCcSolver) -> &NuclearSteadyStatePebble {
    for m in solver.radial_power_models() {
        if let RadialPowerModel::NuclearSteadyStatePebble(pm) = m {
            return pm;
        }
    }
    panic!("no nuclearSteadyStatePebble model")
}

/// What one comparison measured.
struct Outcome {
    rows: Vec<(String, f64)>,
    pebble: Vec<(usize, String, f64, bool)>,
    dts: Vec<(f64, f64)>,
    prgh: Vec<(f64, f64, usize)>,
    turb: Vec<((f64, usize), (f64, usize))>,
    steps: usize,
}

impl Outcome {
    fn worst(&self) -> f64 {
        let f = self.rows.iter().fold(0.0_f64, |m, r| m.max(r.1));
        let p = self.pebble.iter().fold(0.0_f64, |m, r| m.max(r.2));
        let d = self.dts.iter().fold(0.0_f64, |m, (a, b)| m.max(((a - b) / b).abs()));
        f.max(p).max(d)
    }

    fn print(&self, label: &str) {
        println!("== {label}: {} steps", self.steps);
        for (n, d) in &self.rows {
            println!("   field {n:32} {d:.3e}");
        }
        let exact = self.pebble.iter().filter(|r| r.3).count();
        let worst_p = self.pebble.iter().fold(0.0_f64, |m, r| m.max(r.2));
        println!("   pebble log numbers: {exact} of {} bit-identical, worst {worst_p:.3e}", self.pebble.len());
        for (i, (a, b)) in self.dts.iter().enumerate() {
            println!("   deltaT after step {}: port {a:.17e} real {b:.17e} ({:.2e})", i + 1, ((a - b) / b).abs());
        }
        for (i, (a, b, it)) in self.prgh.iter().enumerate() {
            println!("   p_rgh solve {:2}: initial port {a:.6e} real {b:.6e} ({:.1e}), port iterations {it}", i + 1, ((a - b) / b).abs());
        }
        for (i, (e, k)) in self.turb.iter().enumerate() {
            println!("   outer {:2}: epsilon {:.17e} ({} it), k {:.17e} ({} it)", i + 1, e.0, e.1, k.0, k.1);
        }
        println!("   worst {:.3e} (gate {GATE:.0e})", self.worst());
    }
}

/// Run the staged case to the real run's last step and compare (`tag`: the reference set; `laminar`: the `vlam` edit).
fn run_and_compare(tag: &str, laminar: bool, max_steps: usize) -> Outcome {
    let root = stage(tag);
    if laminar {
        let tp = "FoamFile\n{\n    version 2.0;\n    format ascii;\n    class dictionary;\n    object turbulenceProperties;\n}\nsimulationType laminar;\n";
        if let Err(e) = std::fs::write(root.join("constant/fluidRegion/turbulenceProperties"), tp) {
            panic!("write: {e}");
        }
    }
    let poly = root.join("constant/fluidRegion/polyMesh");
    let case = match OnePhaseCcCase::read_with_options(&root, &poly, &CaseReadOptions::default()) {
        Ok(c) => c,
        Err(e) => panic!("the port refuses the variant: {e}"),
    };
    let end = case.end_time;
    let mut solver = OnePhaseCcSolver::new(case);
    let log = real_log(tag);
    let real_dts: Vec<f64> = log_numbers(&log, "deltaT = ").into_iter().filter_map(|v| v.first().copied()).collect();
    let real_prgh: Vec<f64> = log
        .lines()
        .filter(|l| l.contains("Solving for p_rgh"))
        .filter_map(|l| l.split("Initial residual = ").nth(1).and_then(|r| r.split(',').next()).and_then(|x| x.trim().parse::<f64>().ok()))
        .collect();
    let names = ["TpS", "Tmout", "Tmav", "TfS", "Tfav", "Tfmax"];
    let real_peb: Vec<Vec<Vec<f64>>> = names.iter().map(|n| log_numbers(&log, &format!("T.nuclearSteadyStatePebble.{n} (avg min max) = "))).collect();
    let mut o = Outcome { rows: Vec::new(), pebble: Vec::new(), dts: Vec::new(), prgh: Vec::new(), turb: Vec::new(), steps: 0 };
    let mut dt = solver.case().delta_t;
    let mut time = solver.case().start_time;
    let mut solve_i = 0usize;
    // `Time::run()`: `value() < endTime - 0.5 deltaT`
    while time < end - 0.5 * dt && o.steps < max_steps {
        solver.begin_time_step(dt);
        time += dt;
        let (_r, _rep) = solver.correct_physics();
        o.steps += 1;
        for ps in solver.pressure_solves() {
            if let Some(&r) = real_prgh.get(solve_i) {
                o.prgh.push((ps.initial_residual, r, ps.iterations));
            }
            solve_i += 1;
        }
        if let Some(ts) = solver.turbulence_solves() {
            for [e, k] in ts {
                o.turb.push((*e, *k));
            }
        }
        if let Some(rep) = pebble(&solver).report() {
            let mine = [rep.tps, rep.tmout, rep.tmav, rep.tfs, rep.tfav, rep.tfmax];
            for (i, m) in mine.iter().enumerate() {
                if let Some(r) = real_peb[i].get(o.steps - 1) {
                    for (j, x) in [m.avg, m.min, m.max].iter().enumerate() {
                        let d = ((x - r[j]) / r[j]).abs();
                        o.pebble.push((o.steps, format!("{} {}", names[i], ["avg", "min", "max"][j]), d, *x == r[j]));
                    }
                }
            }
        }
        dt = match adjust_delta_t(true, &[solver.max_delta_t()], 1e-300, dt) {
            Some(v) => v,
            None => panic!("deltaT: not adjustable"),
        };
        if let Some(&r) = real_dts.get(o.steps - 1) {
            o.dts.push((dt, r));
        }
    }
    let flat: Vec<f64> = solver.velocity().iter().step_by(7).flat_map(|v| [v.x, v.y, v.z]).collect();
    let mut mine: HashMap<&str, Vec<f64>> = HashMap::new();
    mine.insert("T", every_7th(solver.temperature()));
    mine.insert("p", every_7th(solver.pressure()));
    if let Some(p) = solver.p_rgh() {
        mine.insert("p_rgh", every_7th(p));
    }
    mine.insert("U", flat);
    mine.insert("htc", every_7th(solver.htc()));
    if let Some(k) = solver.turbulent_kinetic_energy() {
        mine.insert("k", every_7th(k));
    }
    if let Some(e) = solver.turbulent_dissipation() {
        mine.insert("epsilon", every_7th(e));
    }
    if let Some(v) = solver.turbulent_viscosity() {
        mine.insert("nut", every_7th(v));
    }
    if let Some(v) = solver.turbulent_thermal_diffusivity() {
        mine.insert("alphat", every_7th(v));
    }
    mine.insert("TpS.nuclearSteadyStatePebble", every_7th(&pebble(&solver).fields().tps));
    mine.insert("Tfmax.nuclearSteadyStatePebble", every_7th(&pebble(&solver).fields().tfmax));
    let mut keys: Vec<&&str> = mine.keys().collect();
    keys.sort();
    for k in keys {
        if let Some(r) = sample_ref(tag, k) {
            o.rows.push((k.to_string(), max_norm(&mine[*k], &r)));
        }
    }
    let _ = std::fs::remove_dir_all(&root);
    o
}

/// Round 87 inventory probe: the first refusal of the `onePhase` reader on a case (prints; runs only with `GENFOAM_HTR10_CASE`,
/// e.g. a copy of the shipped `rootCase`).
#[test]
fn probe_shipped_first_refusal() {
    let root = match env_case("GENFOAM_HTR10_CASE") {
        Some(r) => r,
        None => return,
    };
    let poly = root.join("constant/fluidRegion/polyMesh");
    match OnePhaseCcCase::read_with_options(&root, &poly, &CaseReadOptions::default()) {
        Ok(c) => println!("READS: {} cells", c.t.len()),
        Err(e) => println!("FIRST REFUSAL: {e}"),
    }
}

/// The shipped case is refused by name for its `corrected` Laplacian, and only for it: the one-edit `uncorrected` variant reads.
#[test]
fn r87_the_shipped_schemes_are_refused_by_name() {
    let root = stage("refusal");
    let poly = root.join("constant/fluidRegion/polyMesh");
    match OnePhaseCcCase::read_with_options(&root, &poly, &CaseReadOptions::default()) {
        Ok(_) => {}
        Err(e) => panic!("the uncorrected variant must read: {e}"),
    }
    let schemes = root.join("system/fluidRegion/fvSchemes");
    let text = match std::fs::read_to_string(&schemes) {
        Ok(t) => t,
        Err(e) => panic!("{e}"),
    };
    let shipped = text.replace("Gauss linear uncorrected;", "Gauss linear corrected;").replace("default         uncorrected;", "default         corrected;");
    if let Err(e) = std::fs::write(&schemes, shipped) {
        panic!("{e}");
    }
    match OnePhaseCcCase::read_with_options(&root, &poly, &CaseReadOptions::default()) {
        Ok(_) => panic!("the corrected scheme is not ported and must be refused"),
        Err(e) => assert!(e.to_string().contains("corrected"), "{e}"),
    }
    let _ = std::fs::remove_dir_all(&root);
}

/// Step 1 of the tight `uncorrected` variant with `porousKEpsilon` (`ut2`).
#[test]
fn r87_ut2_step1_matches_the_real_binary() {
    let o = run_and_compare("ut2", false, 1);
    o.print("ut2 (porousKEpsilon, step 1)");
    assert!(o.worst() <= GATE, "worst {:.3e}", o.worst());
}

/// Step 1 of the laminar variant (`vlam`).
#[test]
fn r87_vlam_step1_matches_the_real_binary() {
    let o = run_and_compare("vlam", true, 1);
    o.print("vlam (laminar, step 1)");
    assert!(o.worst() <= GATE, "worst {:.3e}", o.worst());
}

/// The tight `uncorrected` variant to `endTime 0.55` (`ut3`): every step's `deltaT`, `p_rgh` residuals and pebble log, the
/// last step's fields.
#[test]
fn r87_ut3_steps_match_the_real_binary() {
    let o = run_and_compare("ut3", false, usize::MAX);
    o.print("ut3 (porousKEpsilon, to 0.55 s)");
    assert!(o.worst() <= GATE, "worst {:.3e}", o.worst());
}
