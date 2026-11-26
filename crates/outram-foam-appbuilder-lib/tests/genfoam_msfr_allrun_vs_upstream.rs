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
