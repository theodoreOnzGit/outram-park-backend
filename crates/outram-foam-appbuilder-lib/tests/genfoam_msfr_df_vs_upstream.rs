// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//! # `2D_MSFR` light chain with discontinuity factors != 1, against the real binary (gh:#1893, README round 43)
//!
//! AI-written, not human-reviewed. Research, education and V&V only.
//!
//! ## Methodology
//!
//! The light chain of `genfoam_msfr_chain_vs_upstream.rs` (`steadyStateEN` 15 -> 20 s from the committed 15 s fluid restart at
//! the tight protocol, then 5 `transient` steps from the port's OWN 20 s state against upstream's transient started from ITS
//! 20 s output) with `cases/msfr_neutro/nuclearData.df`: `useGivenDiscFactors true` and a non-unit `discFactor` in every zone
//! and group (`scripts/make_msfr_df_data.sh`, 0.96 to 1.04, a deterministic input of both codes, not physical data).
//! Upstream: GeN-Foam 652b3da on ESI v2506, the unmodified binary. References `refs/msfr_en20_df.ref.txt.gz`,
//! `refs/msfr_chain20_df_transient.ref.txt.gz`. The liquid-fuel steps `upstream_eigenvalue_step` and `upstream_transient_step`
//! divide by the factors as `solveNeutronics.H` / `createFluxMatrices.H` / `fluxEq.H` / `normFluxes.H` do (the unknown is
//! `fluxStar_`, `flux_ = fluxStar_/discFactor`). Predictions D1-D9 and the criteria: README "Round 43" (committed before
//! the first run); results: README "Round 43 results".

// Reads case files: `std` only (gh:#1633).
#![cfg(feature = "std")]
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use miniz_oxide::inflate::decompress_to_vec;
use outram_foam_appbuilder_lib::genfoam::multi_region::coupled_loop::CouplingDict;
use outram_foam_appbuilder_lib::genfoam::neutronics::diffusion::upstream_transient::UpstreamTransientControls;
use outram_foam_appbuilder_lib::genfoam::neutronics::xs::CrossSectionData;
use outram_foam_appbuilder_lib::genfoam::thermal_hydraulics::solver::two_phase::one_phase_cc::OnePhaseCcSolver;
use outram_foam_appbuilder_lib::genfoam::workflow::energy_neutro::read_albedo_patches;
use outram_foam_appbuilder_lib::genfoam::workflow::{
    EigenvalueRunControls, EigenvalueStage, EnergyEigenvalueRecord, EnergyNeutroDiffusion, NeutronLinear, NormalisedState, ReactorState, TransientRun,
    TransientSettings, TransientStart, WorkflowError,
};
use outram_foam_appbuilder_lib::io::nuclear_data::read_nuclear_data;
use outram_foam_appbuilder_lib::io::poly_mesh::{read_cell_zones, read_poly_mesh, zone_of_cell};
use outram_foam_basic_lib::ldu_matrix::SolverSettings;

fn vv_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("verification_and_validation/genfoam_vs_upstream_gh1361")
}

fn gunzip(path: &Path) -> Vec<u8> {
    let bytes = std::fs::read(path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    decompress_to_vec(&bytes[10..bytes.len() - 8]).expect("inflate")
}

fn read_ref(path: &Path) -> HashMap<String, Vec<f64>> {
    let text = String::from_utf8(gunzip(path)).expect("utf8");
    let mut m = HashMap::new();
    for line in text.lines() {
        let mut it = line.split_whitespace();
        if let Some(key) = it.next() {
            m.insert(key.to_string(), it.map(|w| w.parse().expect("number")).collect());
        }
    }
    m
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

struct Staged {
    fluid_root: PathBuf,
    poly: PathBuf,
    neutro: PathBuf,
}

/// The `steadyStateEN` case: the fluid dictionaries with `startTime 15`, the t = 15 s restart, the neutronics case.
fn stage(tag: &str) -> Staged {
    let tmp = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("gh1814_{tag}"));
    let _ = std::fs::remove_dir_all(&tmp);
    let fluid_root = tmp.join("fluid");
    copy_dir(&vv_dir().join("cases/msfr_fluid/constant"), &fluid_root.join("constant"));
    copy_dir(&vv_dir().join("cases/msfr_fluid/system"), &fluid_root.join("system"));
    let cd = fluid_root.join("system/controlDict");
    let text = std::fs::read_to_string(&cd).expect("controlDict");
    std::fs::write(&cd, text.replace("startTime 165;", "startTime 15;").replace("endTime 165.31;", "endTime 165;")).expect("write");
    let fdir = fluid_root.join("15/fluidRegion");
    std::fs::create_dir_all(&fdir).expect("mkdir");
    for e in std::fs::read_dir(vv_dir().join("cases/msfr_fluid/restart_15")).expect("restart dir") {
        let p = e.expect("entry").path();
        let name = p.file_name().unwrap().to_string_lossy().trim_end_matches(".gz").to_string();
        std::fs::write(fdir.join(name), gunzip(&p)).expect("write field");
    }
    let src = vv_dir().join("cases/msfr_neutro");
    let poly = tmp.join("polyMesh");
    std::fs::create_dir_all(&poly).expect("mkdir");
    for f in ["points", "faces", "owner", "neighbour", "boundary", "cellZones"] {
        std::fs::write(poly.join(f), gunzip(&src.join("polyMesh").join(format!("{f}.gz")))).expect("polyMesh");
    }
    let neutro = tmp.join("neutro");
    copy_dir(&src.join("uniform"), &neutro.join("uniform"));
    std::fs::copy(src.join("nuclearData"), neutro.join("nuclearData")).expect("nuclearData");
    std::fs::copy(src.join("defaultFlux"), neutro.join("defaultFlux")).expect("defaultFlux");
    Staged { fluid_root, poly, neutro }
}

fn build(st: &Staged) -> (EnergyNeutroDiffusion, TransientSettings) {
    let mesh = read_poly_mesh(&st.poly).expect("polyMesh");
    let input = read_nuclear_data(&st.neutro.join("nuclearData")).expect("nuclearData");
    let xs = CrossSectionData::from_input(&input).expect("cross sections");
    let zones = read_cell_zones(&st.poly).expect("cellZones");
    let order: Vec<&str> = (0..xs.zone_count()).map(|i| xs.zone(i).expect("zone").name()).collect();
    let zone_map = zone_of_cell(&zones, &order, mesh.n_cells).expect("zone map");
    let fluid = OnePhaseCcSolver::from_case_dir(&st.fluid_root, &st.poly).expect("fluid case");
    let albedo = read_albedo_patches(&st.neutro.join("defaultFlux")).expect("albedo patches");
    let reactor = ReactorState::read(&st.neutro).expect("reactorState");
    let settings = TransientSettings {
        coupling: CouplingDict::read(&st.fluid_root, "energyNeutroLoop").expect("multiRegionCouplingDict"),
        control_delta_t: fluid.case().delta_t,
        max_delta_t: fluid.case().controls.max_delta_t,
        max_power_variation: 0.025,
    };
    let stage = EnergyNeutroDiffusion::new(
        fluid,
        mesh,
        xs,
        zone_map,
        &albedo,
        UpstreamTransientControls { neutron_iteration_residual: 1e-6, max_neutron_iterations: 50, sc_no: 1.0 },
        NeutronLinear::Explicit(SolverSettings { tolerance: 1e-14, max_iter: 100_000 }),
        reactor,
        1.0,
        1.0,
    )
    .expect("coupled stage");
    (stage, settings)
}

fn rel(a: f64, b: f64) -> f64 {
    ((a - b) / b).abs()
}

fn worst_rel(a: &[f64], b: &[f64]) -> f64 {
    assert_eq!(a.len(), b.len());
    a.iter().zip(b).filter(|(_, y)| **y != 0.0).map(|(x, y)| rel(*x, *y)).fold(0.0, f64::max)
}

struct EnReport {
    worst_keff: f64,
    first_keff_bad: Option<usize>,
    worst_power: f64,
    worst_h1: f64,
    worst_tstat: f64,
    worst_time: f64,
}

/// Compare every record of an EN stage with the log reference.
fn compare_en(history: &[EnergyEigenvalueRecord], r: &HashMap<String, Vec<f64>>, verbose: bool) -> EnReport {
    let mut rep = EnReport { worst_keff: 0.0, first_keff_bad: None, worst_power: 0.0, worst_h1: 0.0, worst_tstat: 0.0, worst_time: 0.0 };
    for (i, rec) in history.iter().enumerate() {
        let dk = rel(rec.k_eff, r["KEFF"][i]);
        let dp = rel(rec.power, r["POWER"][i]);
        let hres = &r[&format!("HRES_{}", i + 1)];
        let dh = rel(rec.h_residuals[0], hres[0]);
        let ts = &r[&format!("TSTAT_{}", i + 1)];
        let last = rec.t_stats.last().expect("t stats");
        let dts = rel(last.0, ts[12]).max(rel(last.1, ts[13])).max(rel(last.2, ts[14]));
        let dtime = (rec.time - r["TIME"][i]).abs() / r["TIME"][i];
        if verbose && (i < 5 || (i + 1) % 50 == 0) {
            println!(
                "step {:4} t {:8.2}  keff {:.16} (ref {:.16}) rel {dk:.2e}  power rel {dp:.2e}  h1 rel {dh:.2e}  T rel {dts:.2e}",
                i + 1, rec.time, rec.k_eff, r["KEFF"][i]
            );
        }
        if dk > 1e-9 && rep.first_keff_bad.is_none() {
            rep.first_keff_bad = Some(i + 1);
        }
        rep.worst_keff = rep.worst_keff.max(dk);
        rep.worst_power = rep.worst_power.max(dp);
        rep.worst_h1 = rep.worst_h1.max(dh);
        rep.worst_tstat = rep.worst_tstat.max(dts);
        rep.worst_time = rep.worst_time.max(dtime);
    }
    rep
}

struct TransientReport {
    worst_dt: f64,
    worst_t: f64,
    worst_p: f64,
    worst_h1: f64,
    first_dt_bad: Option<usize>,
    first_t_bad: Option<usize>,
    first_power: f64,
}

/// Step `run` through `steps` transient steps and compare each with the `tr` reference (`DT`, `T_n`, `POWER_n`,
/// `HRES_n`); asserts that the coupling loop converged after one iteration, as upstream's did at every step, and the
/// gates of the 50-step test (measured 2026-11-26 with a margin of >= 20x; README "Round 11 results").
fn compare_transient(run: &mut TransientRun, tr: &HashMap<String, Vec<f64>>, steps: usize, upstream_loop: (bool, usize), dt_gate: f64) -> TransientReport {
    let mut rep = TransientReport {
        worst_dt: 0.0,
        worst_t: 0.0,
        worst_p: 0.0,
        worst_h1: 0.0,
        first_dt_bad: None,
        first_t_bad: None,
        first_power: 0.0,
    };
    for step in 1..=steps {
        let rec = run.step_adjustable().expect("transient step");
        let TransientRun::EnergyNeutroDiffusion(t) = &*run else { unreachable!() };
        let dtr = rel(rec.next_delta_t, tr["DT"][step - 1]);
        let dtt = worst_rel(t.inner.fluid.temperature(), &tr[&format!("T_{step}")]);
        let dpr = rel(rec.power, tr[&format!("POWER_{step}")][0]);
        let dh1 = rel(rec.fluid_report.h_residuals[0], tr[&format!("HRES_{step}")][0]);
        println!("step {step:3} dt rel {dtr:.2e}  T rel {dtt:.2e}  power {:.10e} rel {dpr:.2e}  h1 rel {dh1:.2e}", rec.power);
        if dtr > 1e-9 && rep.first_dt_bad.is_none() {
            rep.first_dt_bad = Some(step);
        }
        if dtt > 1e-9 && rep.first_t_bad.is_none() {
            rep.first_t_bad = Some(step);
        }
        rep.worst_dt = rep.worst_dt.max(dtr);
        rep.worst_t = rep.worst_t.max(dtt);
        rep.worst_p = rep.worst_p.max(dpr);
        rep.worst_h1 = rep.worst_h1.max(dh1);
        assert_eq!((rec.coupling_converged, rec.coupling_iterations), upstream_loop, "coupling loop at step {step}");
        if step == 1 {
            rep.first_power = rec.power;
        }
    }
    assert!(
        rep.worst_dt < dt_gate && rep.worst_t < 1e-12 && rep.worst_p < 1e-11 && rep.worst_h1 < 1e-7,
        "transient: dt {:e}, T {:e}, power {:e}, h1 {:e}",
        rep.worst_dt,
        rep.worst_t,
        rep.worst_p,
        rep.worst_h1
    );
    rep
}


/// `with_factors` selects `nuclearData.df` (the round-43 data) or the committed unity `nuclearData`.
fn stage_df(tag: &str, with_factors: bool) -> Staged {
    let st = stage(tag);
    if with_factors {
        std::fs::copy(vv_dir().join("cases/msfr_neutro/nuclearData.df"), st.neutro.join("nuclearData")).expect("nuclearData.df");
    }
    st
}

/// D1-D3: EN 20 steps and 5 transient steps with the non-unit factors, and the control with every factor forced to 1.
#[test]
fn msfr_light_chain_with_discontinuity_factors_matches_upstream() {
    let st = stage_df("df_light", true);
    let en = read_ref(&vv_dir().join("refs/msfr_en20_df.ref.txt.gz"));
    let tr = read_ref(&vv_dir().join("refs/msfr_chain20_df_transient.ref.txt.gz"));
    let (stage, settings) = build(&st);
    assert!(!stage.neutro.xs_fields().disc_factors_are_unity(), "the data carries factors");
    let started = std::time::Instant::now();
    let normalised = EigenvalueStage::EnergyNeutroDiffusion {
        stage,
        controls: EigenvalueRunControls { start_time: 15.0, end_time: 20.0, delta_t: 0.25 },
    }
    .run()
    .expect("eigenvalue stage");
    let NormalisedState::EnergyNeutroDiffusion { stage: s, history } = normalised else { unreachable!() };
    assert_eq!(history.len(), 20);
    let rep = compare_en(&history, &en, true);
    println!(
        "DF EN 20 steps: worst rel keff {:.3e}, power {:.3e}, h1 {:.3e}, T(avg,min,max) {:.3e}, time {:.3e}",
        rep.worst_keff, rep.worst_power, rep.worst_h1, rep.worst_tstat, rep.worst_time
    );
    let k_end = s.reactor.k_eff;
    let mut worst_flux = 0.0_f64;
    for g in 0..s.neutro.energy_groups() {
        let flux = s.neutro.state().flux()[g].internal.as_slice().to_vec();
        worst_flux = worst_flux.max(worst_rel(&flux, &en[&format!("END_flux{g}")]));
    }
    let mut worst_prec = 0.0_f64;
    for (k, p) in s.neutro.prec_star_values().iter().enumerate() {
        worst_prec = worst_prec.max(worst_rel(p, &en[&format!("END_precStar{k}")]));
    }
    let dpd = worst_rel(&s.neutro_power_density, &en["END_powerDensity"]);
    let dt_end = worst_rel(s.fluid.temperature(), &en["END_T"]);
    println!(
        "DF fields at 20 s: keff {:.3e}, flux {worst_flux:.3e}, precStar {worst_prec:.3e}, powerDensity {dpd:.3e}, T {dt_end:.3e}",
        rel(k_end, en["END_KEFF"][0])
    );
    // D1 (criteria fixed before the run): the gates of the unity light chain
    assert!(rep.first_keff_bad.is_none() && rep.worst_keff < 1e-11, "D1 keff: worst {:e}", rep.worst_keff);
    assert!(rep.worst_power < 1e-11 && rep.worst_time == 0.0, "D1 power {:e}", rep.worst_power);
    assert!(rep.worst_h1 < 1e-7 && rep.worst_tstat < 1e-12, "D1 h1 {:e} T {:e}", rep.worst_h1, rep.worst_tstat);
    assert!(rel(k_end, en["END_KEFF"][0]) < 1e-11 && worst_flux < 1e-9 && worst_prec < 1e-9, "D1 fields at 20 s");
    assert!(dpd < 1e-9 && dt_end < 1e-12, "D1 powerDensity {dpd:e}, T {dt_end:e}");

    let normalised = NormalisedState::EnergyNeutroDiffusion { stage: s, history };
    let mut run = normalised.start_transient(TransientStart::EnergyNeutro(settings)).expect("transient");
    let frozen = run.k_eff();
    let trep = compare_transient(&mut run, &tr, 5, (false, 3), 2e-10);
    assert_eq!(run.k_eff(), frozen, "keff_ is frozen in the transient");
    println!(
        "DF transient 5 steps: worst rel dt {:.3e}, T {:.3e}, power {:.3e}, h1 {:.3e}; wall {:.1} s",
        trep.worst_dt, trep.worst_t, trep.worst_p, trep.worst_h1, started.elapsed().as_secs_f64()
    );

    // D3 control: the same port run on the unity data (every factor 1) misses upstream's factor run
    let (c, _) = build(&stage_df("df_control", false));
    let ctl = EigenvalueStage::EnergyNeutroDiffusion {
        stage: c,
        controls: EigenvalueRunControls { start_time: 15.0, end_time: 20.0, delta_t: 0.25 },
    }
    .run()
    .expect("control stage");
    let NormalisedState::EnergyNeutroDiffusion { history: ch, .. } = ctl else { unreachable!() };
    let crep = compare_en(&ch, &en, false);
    println!("DF control (unity data): worst rel keff {:.3e}", crep.worst_keff);
    assert!(crep.worst_keff > 1e-4, "D3: the factors must matter: {:e}", crep.worst_keff);
}

/// D5/D6: an `XsEvolution` (driveline motion, factor adjustment) with the circulating-fuel steps is refused by name.
#[test]
fn an_xs_evolution_is_still_refused_by_name_in_the_liquid_fuel_steps() {
    use outram_foam_appbuilder_lib::genfoam::neutronics::diffusion::upstream_transient::UpstreamTransientFeedback;
    use outram_foam_appbuilder_lib::genfoam::neutronics::diffusion::XsEvolution;
    use outram_foam_appbuilder_lib::io::nuclear_data::read_disc_factor_controls;
    let st = stage_df("df_refuse", true);
    let (stage, _) = build(&st);
    let mut model = stage.neutro.clone();
    let nd = st.neutro.join("nuclearData");
    let input = read_nuclear_data(&nd).expect("nuclearData");
    let xs = CrossSectionData::from_input(&input).expect("cross sections");
    let mesh = read_poly_mesh(&st.poly).expect("polyMesh");
    let zones = read_cell_zones(&st.poly).expect("cellZones");
    let order: Vec<&str> = (0..xs.zone_count()).map(|i| xs.zone(i).expect("zone").name()).collect();
    let zone_map = zone_of_cell(&zones, &order, mesh.n_cells).expect("zone map");
    let cell_zones: Vec<(String, Vec<usize>)> = zones.iter().map(|z| (z.name.clone(), z.cells.clone())).collect();
    let controls = read_disc_factor_controls(&nd).expect("controls");
    let evo = XsEvolution::new(&xs, &mesh, zone_map.clone(), &cell_zones, Vec::new(), Vec::new(), controls).expect("evolution");
    model.set_xs_evolution(evo);
    let feedback = || UpstreamTransientFeedback { cell_parameters: &[], rho_cool: &[], alphat: &[], mu: &[] };
    let ctl = UpstreamTransientControls { neutron_iteration_residual: 1e-6, max_neutron_iterations: 50, sc_no: 1.0 };
    let e1 = model.upstream_eigenvalue_step(&xs, &zone_map, feedback(), 2e7, true, &ctl).expect_err("refused").to_string();
    let e2 = model.upstream_transient_step(&xs, &zone_map, feedback(), 1e-3, &ctl).expect_err("refused").to_string();
    println!("D5 {e1}\nD5 {e2}");
    assert!(e1.contains("XsEvolution") && e2.contains("XsEvolution"));
}
