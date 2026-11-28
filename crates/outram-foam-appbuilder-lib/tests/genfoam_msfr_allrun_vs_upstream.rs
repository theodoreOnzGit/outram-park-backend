// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//! # `2D_MSFR` `Allrun` chain from the ROOT case, against the real binary (gh:#1361 round 21; gh:#1841)
//!
//! AI-written, not human-reviewed. Research, education and V&V only.
//!
//! ## Methodology
//!
//! Upstream: GeN-Foam `652b3da0cc55` on OpenFOAM-ESI v2506, unmodified, `reactorCases/2D_MSFR`. The port runs
//! `genfoam::workflow::allrun::RootCase` on `cases/msfr_root` (the tutorial's `rootCase` without its meshes; the meshes are
//! `cases/msfr_neutro/polyMesh` for both regions plus the fluid region's own `boundary` from `cases/msfr_th/polyMesh`):
//! `steadyStateTH` (15 s, adjustable step aligned to the integer seconds), its fluid fields handed IN MEMORY to
//! `steadyStateEN` (600 steps of 0.25 s), whose state starts the `transient`. No file of the reference is an input.
//!
//! Protocol of the comparison (README round 21): every linear solver `tolerance 1e-14`, `relTol 0` in both codes (the
//! test edits the staged `fvSolution` as `scripts/run_msfr_th_tight.sh` does); `p_rgh` `GAMG` is replaced by `PCG`/`DIC`
//! in upstream by the script and in the port by `GamgPolicy::SubstitutePcgDic` (a NAMED deviation, reported by the stage).
//!
//! Light tests (this file, no `long-tests`): the stage driver against the 14-step references of round 20, the hand-over,
//! the refusals. Long tests: the 15 s stage, the EN stage and the transient against the upstream chain.
//!
//! Predictions: README "Round 21", committed before the first port run. Results: README "Round 21 results".

// Reads case files: `std` only (gh:#1633).
#![cfg(feature = "std")]
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use miniz_oxide::inflate::decompress_to_vec;
use outram_foam_basic_lib::ldu_matrix::SolverSettings;
use outram_foam_appbuilder_lib::genfoam::thermal_hydraulics::solver::two_phase::one_phase_cc::{
    CaseReadOptions, GamgPolicy, OnePhaseCcCase,
};
use outram_foam_appbuilder_lib::genfoam::workflow::allrun::{check_steady_state_th_mapping, AllrunEdits, RootCase};
use outram_foam_appbuilder_lib::genfoam::workflow::{
    Deviation, EigenvalueStage, NormalisedState, ThRunControls, WorkflowError,
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

/// `max |a - b| / max |b|`.
fn rel_max(a: &[f64], b: &[f64]) -> f64 {
    assert_eq!(a.len(), b.len());
    let scale = b.iter().fold(0.0_f64, |m, v| m.max(v.abs()));
    a.iter().zip(b).fold(0.0_f64, |m, (x, y)| m.max((x - y).abs())) / scale
}

fn read_ref(path: &Path) -> HashMap<String, Vec<f64>> {
    let text = String::from_utf8(gunzip(path)).expect("utf8");
    let mut m = HashMap::new();
    for line in text.lines() {
        let mut it = line.split_whitespace();
        if let Some(key) = it.next() {
            let rest: Vec<&str> = it.collect();
            if key == "F" {
                m.insert(format!("{} {}", rest[1], rest[0]), rest[2..].iter().map(|w| w.parse().expect("number")).collect());
            } else if let Ok(nums) = rest.iter().map(|w| w.parse::<f64>()).collect::<Result<Vec<f64>, _>>() {
                m.insert(key.to_string(), nums);
            }
        }
    }
    m
}

/// Tighten every linear solver as `scripts/run_msfr_th_tight.sh` does (`tolerance 1e-14`, `relTol 0`).
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

/// Stage `cases/msfr_root` and the two plain meshes; `tight` applies the comparison protocol.
fn stage(tag: &str, tight: bool) -> RootCase {
    let tmp = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("gh1841_{tag}"));
    let _ = std::fs::remove_dir_all(&tmp);
    let root = tmp.join("rootCase");
    copy_dir(&vv_dir().join("cases/msfr_root"), &root);
    if tight {
        tighten(&root.join("system/fluidRegion/fvSolution"));
        // gh:#1890 (round 43): the workflow reads the neutronics `fvSolution`, so the staged file is tightened as
        // upstream's runs did (`run_msfr_chain.sh tighten_solvers` edits every `system/*/fvSolution`).
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

/// The 14-step comparison of round 20, through the stage driver: fields of steps 1-3 (`msfr_th_on_head`), steps 6, 9, 14
/// (`msfr_th_on_later`), the step-1 time step. `writeControl timeStep` upstream, so no write-time alignment (`None`).
#[test]
fn stage_driver_reproduces_the_fourteen_step_reference() {
    let root = stage("th14", true);
    let edits = AllrunEdits { end_time_th: 0.06, write_interval_th: None, ..AllrunEdits::default() };
    let EigenvalueStage::SteadyStateTh { mut stage, controls } = root.steady_state_th(&edits).expect("rootCase") else { unreachable!() };
    assert_eq!(controls.write_interval, None);
    let head = read_ref(&vv_dir().join("refs/msfr_th_on_head.ref.txt.gz"));
    let later = read_ref(&vv_dir().join("refs/msfr_th_on_later.ref.txt.gz"));
    let mut step = 0usize;
    let mut worst = 0.0_f64;
    let history = stage
        .run(&controls, |_rec, st| {
            step += 1;
            let f = &st.fluid;
            let u: Vec<f64> = f.velocity().iter().flat_map(|v| [v.x, v.y, v.z]).collect();
            let sample: Option<(&HashMap<String, Vec<f64>>, f64)> = if step <= 3 {
                Some((&head, head["TIMEPRINT"][step - 1]))
            } else {
                [6usize, 9, 14].iter().position(|&s| s == step).map(|i| (&later, later["TIMEPRINT"][i]))
            };
            if let Some((rf, tp)) = sample {
                let key = |name: &str| format!("{name} {tp}");
                let mut r = vec![
                    rel_max(&u, &rf[&key("U")]),
                    rel_max(f.p_rgh().expect("flow"), &rf[&key("p_rgh")]),
                    rel_max(f.turbulent_kinetic_energy().expect("k"), &rf[&key("k")]),
                    rel_max(f.turbulent_dissipation().expect("eps"), &rf[&key("epsilon")]),
                    rel_max(f.turbulent_viscosity().expect("nut"), &rf[&key("nut")]),
                ];
                if step <= 3 {
                    r.push(rel_max(f.pressure(), &rf[&key("p")]));
                    r.push(rel_max(f.temperature(), &rf[&key("T")]));
                }
                println!("step {step} t={tp}: {r:?}");
                worst = r.iter().fold(worst, |m, v| m.max(*v));
            }
        })
        .expect("stage");
    assert_eq!(history.len(), 14, "14 steps to endTime 0.06");
    assert!(worst <= 1e-9, "worst field difference {worst:e}");
    assert!((history[0].delta_t - 1e-3).abs() < 1e-18);
    assert!((history[1].delta_t - head["DT"][0]).abs() <= 1e-15 * history[1].delta_t);
    assert!(history.iter().all(|r| !r.write_time), "no write alignment without adjustableRunTime");
    assert_eq!(stage.deviations(), vec![Deviation::PressureGamgSubstitutedByPcgDic]);
}

/// With `adjustableRunTime` the stage lands on its write times (`Time::adjustDeltaT`): the first write time of a
/// 0.002 s stage with `writeInterval 0.001` is a step end, and the last step ends at the `endTime`.
#[test]
fn adjustable_run_time_lands_on_the_write_times() {
    let root = stage("align", true);
    let edits = AllrunEdits { end_time_th: 0.002, write_interval_th: Some(0.001), ..AllrunEdits::default() };
    let EigenvalueStage::SteadyStateTh { mut stage, controls } = root.steady_state_th(&edits).expect("rootCase") else { unreachable!() };
    let history = stage.run(&controls, |_, _| {}).expect("stage");
    let writes: Vec<f64> = history.iter().filter(|r| r.write_time).map(|r| r.time).collect();
    assert_eq!(writes.len(), 2, "{writes:?}");
    assert!((writes[0] - 0.001).abs() < 1e-15 && (writes[1] - 0.002).abs() < 1e-15, "{writes:?}");
    assert!((history.last().expect("steps").time - 0.002).abs() < 1e-15);
}

/// The hand-over: the EN stage's fluid region starts from the TH stage's own fields, bitwise, at `startTime = endTimeTH`,
/// with no powerDensity; the EN stage then steps.
#[test]
fn the_fluid_fields_of_the_th_stage_start_the_en_stage_bitwise() {
    let root = stage("handover", true);
    let edits = AllrunEdits { end_time_th: 0.004, write_interval_th: None, end_time_en: 0.004 + 0.5, delta_t_en: 0.25, ..AllrunEdits::default() };
    let th = root.steady_state_th(&edits).expect("rootCase").run().expect("th stage");
    let NormalisedState::SteadyStateTh { stage, deviations, .. } = &th else { panic!("TH state") };
    assert_eq!(deviations, &vec![Deviation::PressureGamgSubstitutedByPcgDic]);
    assert!(Deviation::PressureGamgSubstitutedByPcgDic.describe().contains("GAMG"));
    let handed = stage.fluid.fluid_fields();
    assert!(handed.power_density.iter().all(|&p| p == 0.0), "the TH stage has no power");
    let en = root.steady_state_en(th.clone(), &edits).expect("EN stage");
    let EigenvalueStage::EnergyNeutroDiffusion { stage: en_stage, controls } = en else { panic!("EN stage") };
    assert_eq!(controls.start_time, 0.004);
    assert_eq!(en_stage.fluid.time(), 0.004, "startTime of the EN clone is endTimeTH");
    assert_eq!(en_stage.fluid.temperature(), handed.t.as_slice());
    assert_eq!(en_stage.fluid.pressure(), handed.p.as_slice());
    assert_eq!(en_stage.fluid.velocity(), handed.u.as_slice());
    assert_eq!(en_stage.fluid.case().alphat, handed.alphat);
    assert_eq!(en_stage.fluid.case().alpha_phi, handed.alpha_phi);
    assert_eq!(en_stage.fluid.case().alpha_rho_phi, handed.alpha_rho_phi);
    assert_eq!(en_stage.fluid.case().htc0, handed.htc);
    assert!(en_stage.fluid.case().flow.is_none(), "solveFluidMechanics false in the EN stage");
    assert_eq!((en_stage.reactor.k_eff, en_stage.reactor.p_target), (0.930537, 2e7));
    // a step of the EN stage runs on it
    let state = EigenvalueStage::EnergyNeutroDiffusion { stage: en_stage, controls }.run().expect("EN");
    let NormalisedState::EnergyNeutroDiffusion { history, .. } = state else { panic!("EN state") };
    assert_eq!(history.len(), 2);
    assert!(history[0].k_eff > 0.0 && history[0].power > 0.0);
}

/// The history and final fluxes of the EN stage of `edits` on `root` (TH 0.004 s, then 2 EN steps).
fn en_two_steps(root: &RootCase, edits: &AllrunEdits) -> (Vec<(f64, f64, f64)>, Vec<Vec<f64>>) {
    let th = root.steady_state_th(edits).expect("rootCase").run().expect("th stage");
    let en = root.steady_state_en(th, edits).expect("EN stage");
    let state = en.run().expect("EN");
    let NormalisedState::EnergyNeutroDiffusion { stage, history } = state else { panic!("EN state") };
    let flux = (0..stage.neutro.energy_groups()).map(|g| stage.neutro.state().flux()[g].internal.as_slice().to_vec()).collect();
    (history.iter().map(|r| (r.k_eff, r.power, r.neutro_residual)).collect(), flux)
}

/// gh:#1890 (README round 43, W1-W5): the workflow reads the neutronics `fvSolution` of the root case.
///
/// W1: with the staged file tightened (as upstream's runs), the default (read the file) and the named explicit option
/// (`tolerance 1e-14`, `maxIter 100000`) give the same EN history and fluxes. W2 (control): the SHIPPED file (flux and
/// precursor entries `relTol 1e-1`) moves them. W5: an entry the port does not translate is refused by name.
#[test]
fn the_workflow_reads_the_neutronics_fv_solution() {
    let edits = AllrunEdits { end_time_th: 0.004, write_interval_th: None, end_time_en: 0.004 + 0.5, delta_t_en: 0.25, ..AllrunEdits::default() };
    let root = stage("fvsol_tight", true);
    let (h_file, f_file) = en_two_steps(&root, &edits);
    let explicit = edits.clone().with_explicit_neutron_linear(SolverSettings { tolerance: 1e-14, max_iter: 100_000 });
    let (h_expl, f_expl) = en_two_steps(&root, &explicit);
    println!("W1 file {h_file:?}\n   explicit {h_expl:?}");
    assert_eq!(h_file.len(), 2);
    let bit_identical = h_file == h_expl && f_file == f_expl;
    let worst = h_file.iter().zip(&h_expl).map(|(a, b)| ((a.0 - b.0) / b.0).abs().max(((a.1 - b.1) / b.1).abs())).fold(0.0, f64::max);
    println!("W1 bit-identical {bit_identical}, worst keff/power difference {worst:.3e}");
    assert!(worst < 1e-12, "W1: {worst:e}");

    // W2: the shipped neutronics file read by the workflow
    let loose = stage("fvsol_shipped", false);
    let th_edits = edits.clone();
    let tight_th = {
        // the fluid file of the control is tightened too, so only the neutronics file differs from the W1 run
        tighten(&loose.root.join("system/fluidRegion/fvSolution"));
        &loose
    };
    let (h_ship, _) = en_two_steps(tight_th, &th_edits);
    let moved = h_ship.iter().zip(&h_file).map(|(a, b)| ((a.0 - b.0) / b.0).abs().max(((a.1 - b.1) / b.1).abs())).fold(0.0, f64::max);
    println!("W2 shipped neutronics fvSolution: keff/power moved by {moved:.3e}");
    assert!(moved > 1e-9, "W2: the shipped relTol 1e-1 must act: {moved:e}");

    // W5: an unported solver in the file is refused, naming the entry
    let bad = stage("fvsol_bad", true);
    let p = bad.root.join("system/neutroRegion/fvSolution");
    let text = std::fs::read_to_string(&p).expect("fvSolution");
    std::fs::write(&p, text.replacen("PCG", "smoothSolver", 1)).expect("write");
    let th = bad.steady_state_th(&edits).expect("rootCase").run().expect("th stage");
    let err = bad.steady_state_en(th, &edits).expect_err("smoothSolver refused").to_string();
    println!("W5 {err}");
    assert!(err.contains("smoothSolver") || err.contains("flux"), "{err}");
}

/// gh:#1947 (README round 76): the SHIPPED `2D_MSFR` rootCase (`p_rgh` `GAMG`) is read by the ordinary path, `CaseReadOptions::default()`,
/// with upstream's own `GAMGSolver` (`GamgPolicy::Native` is the default; ~~`Refuse`~~ was); `AllrunEdits::default()` stays the PROTOCOL
/// of the committed references (`GAMG` -> `PCG`/`DIC` in both runs) and `AllrunEdits::as_shipped_gamg()` keeps `GAMG` as shipped.
/// Pattern: `outram-mc-libs/tests/correct_physics_is_default.rs` (construct through the ordinary path, assert the physics is present).
#[test]
fn gamg_is_the_default_and_the_allrun_protocol_is_named() {
    let root = stage("gamg_default", false);
    // the ordinary path: no policy named
    let ordinary = CaseReadOptions { check_region_solvers: false, ..CaseReadOptions::default() };
    assert_eq!(ordinary.gamg, GamgPolicy::Native);
    let case = OnePhaseCcCase::read_with_options(&root.root, &root.fluid_poly_mesh, &ordinary).expect("the shipped GAMG case is read by default");
    let flow = case.flow.as_ref().expect("flow");
    assert!(!flow.p_rgh_gamg_substituted, "default: no PCG/DIC substitution");
    // the named ablation is still an explicit act
    let ablated = CaseReadOptions { gamg: GamgPolicy::SubstitutePcgDic, ..ordinary.clone() };
    let case = OnePhaseCcCase::read_with_options(&root.root, &root.fluid_poly_mesh, &ablated).expect("substituted");
    assert!(case.flow.as_ref().expect("flow").p_rgh_gamg_substituted);
    // the protocol and its as-shipped sibling
    assert_eq!(AllrunEdits::default().gamg, GamgPolicy::SubstitutePcgDic, "the references' protocol replaced GAMG by PCG/DIC in the real run");
    let shipped = AllrunEdits::as_shipped_gamg();
    assert_eq!(shipped.gamg, GamgPolicy::Native);
    assert_eq!(shipped.end_time_th, AllrunEdits::default().end_time_th, "only the GAMG choice differs");
    root.steady_state_th(&shipped).expect("the case as shipped (GAMG) is accepted by steadyStateTH under the as-shipped protocol");
}

/// The remaining refusals are by name: `GAMG` when not substituted, a coupling dictionary that maps, a non-TH state,
/// and the stages without a port.
#[test]
fn refusals_are_by_name() {
    let root = stage("refuse", false);
    // shipped case, GAMG refused unless substituted
    let refuse = AllrunEdits { gamg: GamgPolicy::Refuse, ..AllrunEdits::default() };
    let err = root.steady_state_th(&refuse).expect_err("GAMG refused").to_string();
    assert!(err.contains("GAMG"), "{err}");
    let options = CaseReadOptions { gamg: GamgPolicy::SubstitutePcgDic, check_region_solvers: false, ..CaseReadOptions::default() };
    let case = OnePhaseCcCase::read_with_options(&root.root, &root.fluid_poly_mesh, &options).expect("substituted");
    let flow = case.flow.as_ref().expect("flow");
    assert!(flow.p_rgh_gamg_substituted);
    assert_eq!((flow.p_rgh.tolerance, flow.p_rgh.rel_tol), (1e-7, 0.0), "the shipped tolerance is kept");
    // a coupling dictionary that maps a field is not `steadyStateTH`
    assert!(check_steady_state_th_mapping(&root.root.join("constant/multiRegionCouplingDict_steadyStateTH")).is_ok());
    assert!(check_steady_state_th_mapping(&root.root.join("constant/multiRegionCouplingDict_steadyStateEN")).is_err());
    // EN from a state that is not the TH state
    let th_wrong = EigenvalueStage::Sn.run();
    assert!(matches!(th_wrong, Err(WorkflowError::NotPorted { .. })));
    // the transient settings come from the transient dictionary and the controlDict
    let settings = root.transient_settings(&AllrunEdits::default()).expect("transient settings");
    assert_eq!(settings.coupling.max_iterations, 3);
    assert_eq!(settings.control_delta_t, 1e-3);
    let _ = ThRunControls { end_time: 1.0, delta_t: 1.0, adjust_time_step: true, write_interval: None };
}

// ---------------------------------------------------------------------------------------------------------------------
// The whole chain (hour-scale: see the doc comment)
// ---------------------------------------------------------------------------------------------------------------------

use outram_foam_appbuilder_lib::genfoam::thermal_hydraulics::solver::two_phase::one_phase_cc::OnePhaseCcSolver;
use outram_foam_appbuilder_lib::genfoam::workflow::{EnergyEigenvalueRecord, TransientRun, TransientStart};

fn rel(a: f64, b: f64) -> f64 {
    ((a - b) / b).abs()
}

fn worst_rel(a: &[f64], b: &[f64]) -> f64 {
    assert_eq!(a.len(), b.len());
    a.iter().zip(b).filter(|(_, y)| **y != 0.0).map(|(x, y)| rel(*x, *y)).fold(0.0, f64::max)
}

/// Every field of `refs/msfr_th15_fields` present at `tp`, compared with the solver's state: `(name, max-norm relative difference)`.
fn compare_th_fields(f: &OnePhaseCcSolver, rf: &HashMap<String, Vec<f64>>, tp: &str) -> Vec<(&'static str, f64)> {
    let key = |name: &str| format!("{name} {tp}");
    let u: Vec<f64> = f.velocity().iter().flat_map(|v| [v.x, v.y, v.z]).collect();
    let fields = f.fluid_fields();
    let mut out = Vec::new();
    let mut put = |name: &'static str, mine: &[f64]| {
        if let Some(r) = rf.get(&key(name)) {
            out.push((name, rel_max(&mine[..r.len()], r)));
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

/// `steadyStateTH -> steadyStateEN -> transient` from `cases/msfr_root`, tight protocol, against the upstream chain run with
/// `scripts/run_msfr_th_tight.sh` (`END_TIME=15 WRITE=adjustable PSOLVER=pcg`) and `scripts/run_msfr_chain.sh` (`PROTOCOL=tight`)
/// from THAT run's own `15`: references `msfr_th15_log`, `msfr_th15_fields`, `msfr_en_root`, `msfr_root_transient`
/// (`extract_*.sh`). Hour-scale (~1800 TH steps at ~1 s, 600 EN steps, 50 transient steps), so a plain `#[ignore]`:
/// `cargo test --release -p outram-foam-appbuilder-lib --test genfoam_msfr_allrun_vs_upstream -- --ignored --nocapture`.
/// Gates at the measured values with a margin of >= 20x (2026-11-26: TH fields 2.7e-13, dt bit-identical, EN `keff_` 2.2e-13,
/// transient `power:` 7.1e-14; the predicted 1e-9 gates were never close). Results: README "Round 21 results".
#[test]
#[ignore = "hour-scale: the 15 s steadyStateTH stage is ~1800 steps; run with --ignored"]
fn msfr_allrun_chain_from_the_root_case_matches_upstream() {
    let started = std::time::Instant::now();
    let root = stage("chain", true);
    let edits = AllrunEdits::default();
    let th_log = read_ref(&vv_dir().join("refs/msfr_th15_log.ref.txt.gz"));
    let th_fields = read_ref(&vv_dir().join("refs/msfr_th15_fields.ref.txt.gz"));
    let EigenvalueStage::SteadyStateTh { stage: mut th_stage, controls } = root.steady_state_th(&edits).expect("rootCase") else {
        unreachable!()
    };
    let mut samples: Vec<(String, Vec<(&'static str, f64)>)> = Vec::new();
    let history = th_stage
        .run(&controls, |rec, st| {
            if rec.write_time {
                for tp in ["1", "5", "10", "15"] {
                    let t: f64 = tp.parse().expect("time");
                    if (rec.time - t).abs() < 1e-9 && th_fields.contains_key(&format!("U {tp}")) {
                        let r = compare_th_fields(&st.fluid, &th_fields, tp);
                        println!("t={tp}: {r:?}");
                        samples.push((tp.to_string(), r));
                    }
                }
            }
        })
        .expect("TH stage");
    println!("TH stage: {} steps in {:.0} s", history.len(), started.elapsed().as_secs_f64());
    let n_ref = th_log["N"][0] as usize;
    let dts: Vec<f64> = history.iter().map(|r| r.next_delta_t).collect();
    let worst_dt = dts.iter().zip(&th_log["DT"]).map(|(a, b)| rel(*a, *b)).fold(0.0, f64::max);
    println!("TH steps: port {} upstream {n_ref}; worst dt rel {worst_dt:.3e}; last time {:.17}", history.len(), history.last().expect("steps").time);
    assert_eq!(history.len(), n_ref, "step count");
    assert!(worst_dt <= 1e-12, "dt: {worst_dt:e}");
    assert_eq!(samples.len(), 4, "the four sampled write times");
    for (tp, r) in &samples {
        for (name, d) in r {
            assert!(*d <= 1e-11, "t={tp} {name}: {d:e}");
        }
    }
    let th = NormalisedState::SteadyStateTh { stage: th_stage, history, deviations: vec![Deviation::PressureGamgSubstitutedByPcgDic] };

    let en_ref = read_ref(&vv_dir().join("refs/msfr_en_root.ref.txt.gz"));
    let EigenvalueStage::EnergyNeutroDiffusion { stage: en_stage, controls } = root.steady_state_en(th, &edits).expect("EN stage") else {
        unreachable!()
    };
    let normalised = EigenvalueStage::EnergyNeutroDiffusion { stage: en_stage, controls }.run().expect("EN run");
    println!("EN stage done at {:.0} s", started.elapsed().as_secs_f64());
    let NormalisedState::EnergyNeutroDiffusion { stage: s, history } = normalised else { unreachable!() };
    assert_eq!(history.len(), 600);
    let w = compare_en(&history, &en_ref);
    println!("EN 600 steps: worst rel keff {:.3e}, power {:.3e}, h1 {:.3e}, T {:.3e}, time {:.3e}", w[0], w[1], w[2], w[3], w[4]);
    let k_final = s.reactor.k_eff;
    println!("final keff_ {k_final:.17} (upstream {:.17}); rel {:.3e}", en_ref["END_KEFF"][0], rel(k_final, en_ref["END_KEFF"][0]));
    assert!(w[0] <= 1e-11 && w[1] <= 1e-11 && w[2] <= 1e-7 && w[3] <= 1e-12 && w[4] == 0.0, "EN: {w:?}");
    assert!(rel(k_final, en_ref["END_KEFF"][0]) <= 1e-11);
    let dpd = worst_rel(&s.neutro_power_density, &en_ref["END_powerDensity"]);
    let dt_end = worst_rel(s.fluid.temperature(), &en_ref["END_T"]);
    println!("fields at 165 s: powerDensity {dpd:.3e}, T {dt_end:.3e}");
    assert!(dpd <= 1e-8 && dt_end <= 1e-12);

    let tr = read_ref(&vv_dir().join("refs/msfr_root_transient.ref.txt.gz"));
    let settings = root.transient_settings(&edits).expect("transient settings");
    let mut run = NormalisedState::EnergyNeutroDiffusion { stage: s, history }
        .start_transient(TransientStart::EnergyNeutro(settings))
        .expect("transient");
    let frozen = run.k_eff();
    let mut worst = [0.0_f64; 4];
    for step in 1..=50usize {
        let rec = run.step_adjustable().expect("transient step");
        let TransientRun::EnergyNeutroDiffusion(t) = &run else { unreachable!() };
        let d = [
            rel(rec.next_delta_t, tr["DT"][step - 1]),
            worst_rel(t.inner.fluid.temperature(), &tr[&format!("T_{step}")]),
            rel(rec.power, tr[&format!("POWER_{step}")][0]),
            rel(rec.fluid_report.h_residuals[0], tr[&format!("HRES_{step}")][0]),
        ];
        println!("transient step {step}: dt {:.2e} T {:.2e} power {:.2e} h1 {:.2e}", d[0], d[1], d[2], d[3]);
        for i in 0..4 {
            worst[i] = worst[i].max(d[i]);
        }
    }
    assert_eq!(run.k_eff(), frozen);
    println!("TRANSIENT 50 steps: worst dt {:.3e} T {:.3e} power {:.3e} h1 {:.3e}; total {:.0} s", worst[0], worst[1], worst[2], worst[3], started.elapsed().as_secs_f64());
    assert!(worst[0] <= 1e-11 && worst[1] <= 1e-12 && worst[2] <= 1e-11 && worst[3] <= 1e-7, "{worst:?}");
}

/// The SHIPPED protocol (all linear solvers at their shipped `1e-7`; upstream with its real `GAMG`, the port with the named
/// `SubstitutePcgDic` deviation): the 15 s `steadyStateTH` stage is NOT expected to agree to round-off. This test measures
/// and prints the agreement (README "Round 21 results", prediction T6) and asserts bounds at 20x the measured agreement (2026-11-26: p/p_rgh <= 5.1e-7, T 1.2e-10, U 4.1e-6, k/nut/alphat 1.2e-5; the pre-run prediction was looser).
/// References: `msfr_th15_shipped_{log,fields}` (`scripts/run_msfr_th_tight.sh` with `PSOLVER=gamg TOLERANCE=shipped`).
#[test]
#[ignore = "~20 minutes: the 15 s steadyStateTH stage; run with --ignored"]
fn shipped_protocol_gamg_substitution_agreement_at_fifteen_seconds() {
    let root = stage("shipped", false);
    let edits = AllrunEdits::default();
    let log = read_ref(&vv_dir().join("refs/msfr_th15_shipped_log.ref.txt.gz"));
    let fields = read_ref(&vv_dir().join("refs/msfr_th15_shipped_fields.ref.txt.gz"));
    let EigenvalueStage::SteadyStateTh { stage: mut th_stage, controls } = root.steady_state_th(&edits).expect("rootCase") else {
        unreachable!()
    };
    let mut samples: Vec<(String, Vec<(&'static str, f64)>)> = Vec::new();
    let history = th_stage
        .run(&controls, |rec, st| {
            if rec.write_time {
                for tp in ["1", "5", "10", "15"] {
                    let t: f64 = tp.parse().expect("time");
                    if (rec.time - t).abs() < 1e-9 {
                        samples.push((tp.to_string(), compare_th_fields(&st.fluid, &fields, tp)));
                    }
                }
            }
        })
        .expect("TH stage");
    let n_ref = log["N"][0] as usize;
    println!("SHIPPED: port {} steps, upstream {n_ref} steps", history.len());
    for (tp, r) in &samples {
        println!("SHIPPED t={tp}: {r:?}");
    }
    assert_eq!(samples.len(), 4);
    for (tp, r) in &samples {
        for (name, d) in r {
            let bound = match *name {
                "U" => 1e-4,
                "k" | "epsilon" | "nut" | "alphat" | "alphaPhi" | "alphaRhoPhi" => 3e-4,
                "p" | "p_rgh" => 1e-5,
                "T" => 3e-9,
                _ => 1e-9,
            };
            assert!(*d <= bound, "t={tp} {name}: {d:e} (predicted <= {bound:e})");
        }
    }
}
