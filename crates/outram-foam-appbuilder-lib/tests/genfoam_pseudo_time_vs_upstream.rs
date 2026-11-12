// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//! # GeN-Foam pseudo-time eigenvalue history against the real binary (gh:#1361, S4)
//!
//! AI-written, not human-reviewed. Research, education and V&V only.
//!
//! ## Methodology
//!
//! Upstream: GeN-Foam `652b3da0cc55` on OpenFOAM-ESI v2506, the unmodified
//! `GeN-Foam.C` (`scripts/build_neutronics_stack.sh`), running the guided slab
//! tutorials 1 and 2 with `diffusionNeutronics` and with `SP3Neutronics` at the
//! tight protocol (`scripts/tight_protocol.sh`: `writePrecision 17`, every linear
//! solver `tolerance 1e-14`, `relTol 0`, `endTime 60`). The committed references
//! `refs/guided_slabs.ref.txt.gz` and `refs/sp3_slabs.ref.txt.gz` hold, per
//! pseudo-time step, `Time`, `keff_`, `power:` and the first-solve initial
//! residual, and the final `flux0`.
//!
//! The port runs `run_pseudo_time_eigenvalue` (the translation of the
//! `GeN-Foam.C` loop with `setDeltaT`/`adjustDeltaT`, `Time::adjustDeltaT`,
//! `maxDeltaT`, and the per-step body of `solveNeutronics.H`/`calcKeff.H`/
//! `normFluxes.H`) on the same committed polyMesh and nuclearData with
//! `controlDict` values copied from the tutorials (`deltaT 1e-9`,
//! `writeInterval 1`, `maxDeltaT 1`, `maxPowerVariation 0.01`, `endTime 60`,
//! `pTarget 1e6`) and the linear solver at `1e-14`. SP3 starts from
//! `defaultFlux2 uniform 1`, as the tutorials' `0.orig` does.
//!
//! Pass criteria are the predictions T1 to T7 of the README, committed before
//! this test was first run. Results: README, "Round 2 results".

use std::path::{Path, PathBuf};
use std::sync::Arc;

use miniz_oxide::inflate::decompress_to_vec;
use outram_foam_appbuilder_lib::genfoam::neutronics::diffusion::{
    DiffusionNeutronics, DiffusionSettings,
};
use outram_foam_appbuilder_lib::genfoam::neutronics::pseudo_time::{
    run_pseudo_time_eigenvalue, PseudoTimeControls, PseudoTimeRecord,
};
use outram_foam_appbuilder_lib::genfoam::neutronics::sp3::{Sp3Neutronics, Sp3Settings};
use outram_foam_appbuilder_lib::genfoam::neutronics::xs::CrossSectionData;
use outram_foam_appbuilder_lib::io::nuclear_data::read_nuclear_data;
use outram_foam_appbuilder_lib::io::poly_mesh::{read_cell_zones, read_poly_mesh, zone_of_cell};
use outram_foam_basic_lib::fields::boundary::bc::BoundaryCondition;
use outram_foam_basic_lib::ldu_matrix::SolverSettings;
use outram_foam_basic_lib::mesh::FvMesh;

fn vv_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("verification_and_validation/genfoam_vs_upstream_gh1361")
}

fn gunzip(path: &Path) -> Vec<u8> {
    let bytes = std::fs::read(path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    decompress_to_vec(&bytes[10..bytes.len() - 8]).expect("inflate")
}

fn stage_case(case: &str) -> PathBuf {
    let src = vv_dir().join("cases").join(case);
    let dst = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("gh1361_pt_{case}"));
    let _ = std::fs::remove_dir_all(&dst);
    std::fs::create_dir_all(dst.join("polyMesh")).expect("scratch dir");
    std::fs::copy(src.join("nuclearData"), dst.join("nuclearData")).expect("nuclearData");
    for f in ["points", "faces", "owner", "neighbour", "boundary", "cellZones"] {
        let data = gunzip(&src.join("polyMesh").join(format!("{f}.gz")));
        std::fs::write(dst.join("polyMesh").join(f), data).expect("write polyMesh file");
    }
    dst
}

struct Upstream {
    time: Vec<f64>,
    keff: Vec<f64>,
    power: Vec<f64>,
    resid: Vec<f64>,
    flux: Vec<f64>,
}

fn upstream(file: &str, case_index: usize) -> Upstream {
    let text = String::from_utf8(gunzip(&vv_dir().join(format!("refs/{file}.ref.txt.gz")))).unwrap();
    let mut cur = 0usize;
    let mut u = Upstream { time: vec![], keff: vec![], power: vec![], resid: vec![], flux: vec![] };
    for line in text.lines() {
        let mut it = line.split_whitespace();
        let key = it.next();
        if key == Some("CASE") {
            cur += 1;
            continue;
        }
        if cur != case_index {
            continue;
        }
        let vals = |it: std::str::SplitWhitespace| -> Vec<f64> { it.map(|w| w.parse().unwrap()).collect() };
        match key {
            Some("TIME") => u.time = vals(it),
            Some("KEFF") => u.keff = vals(it),
            Some("POWER") => u.power = vals(it),
            Some("RESID") => u.resid = vals(it),
            Some("FLUX0") => u.flux = vals(it),
            _ => {}
        }
    }
    u
}

struct Setup {
    mesh: Arc<FvMesh>,
    xs: CrossSectionData,
    zone_map: Vec<usize>,
    flux_boundary: Vec<BoundaryCondition<f64>>,
}

fn setup(case: &str) -> Setup {
    let dir = stage_case(case);
    let mesh: Arc<FvMesh> = read_poly_mesh(&dir.join("polyMesh")).expect("polyMesh");
    let input = read_nuclear_data(&dir.join("nuclearData")).expect("nuclearData");
    let xs = CrossSectionData::from_input(&input).expect("cross sections");
    let zones = read_cell_zones(&dir.join("polyMesh")).expect("cellZones");
    let order: Vec<&str> = (0..xs.zone_count()).map(|i| xs.zone(i).expect("zone").name()).collect();
    let zone_map = zone_of_cell(&zones, &order, mesh.n_cells).expect("zone map");
    let flux_boundary = mesh
        .patches
        .iter()
        .map(|p| {
            if p.name == "walls" {
                BoundaryCondition::ZeroGradient
            } else {
                assert!(p.name == "top" || p.name == "bottom", "patch {}", p.name);
                BoundaryCondition::FixedValue(0.0)
            }
        })
        .collect();
    Setup { mesh, xs, zone_map, flux_boundary }
}

fn controls() -> PseudoTimeControls {
    PseudoTimeControls::guided(1e-9, 60.0, 1.0, 1.0, 0.01, 1e6)
}

fn linear() -> SolverSettings {
    SolverSettings { tolerance: 1e-14, max_iter: 100_000 }
}

fn run_diffusion(case: &str) -> (Vec<PseudoTimeRecord>, Vec<f64>) {
    let s = setup(case);
    let settings = DiffusionSettings { linear: linear(), ..DiffusionSettings::default() };
    let mut solver =
        DiffusionNeutronics::new(s.mesh.clone(), &s.xs, &s.zone_map, &[], &s.flux_boundary, settings)
            .expect("diffusion");
    let h = run_pseudo_time_eigenvalue(&mut solver, &controls()).expect("pseudo-time loop");
    (h, solver.state().flux()[0].internal.as_slice().to_vec())
}

fn run_sp3(case: &str) -> (Vec<PseudoTimeRecord>, Vec<f64>) {
    let s = setup(case);
    let settings = Sp3Settings { linear: linear(), ..Sp3Settings::default() };
    let mut solver = Sp3Neutronics::with_cross_sections(
        s.mesh.clone(),
        &s.xs,
        &s.zone_map,
        &[],
        &s.flux_boundary,
        settings,
    )
    .expect("sp3");
    solver.set_uniform_second_moment(1.0);
    let h = run_pseudo_time_eigenvalue(&mut solver, &controls()).expect("pseudo-time loop");
    (h, solver.state().flux()[0].internal.as_slice().to_vec())
}

fn shape(v: &[f64]) -> Vec<f64> {
    let s: f64 = v.iter().sum();
    v.iter().map(|x| x / s).collect()
}

/// Compare one run with its reference, print the table row, assert T1 to T5, T7.
fn compare(label: &str, up: &Upstream, port: &[PseudoTimeRecord], flux: &[f64], n_expected: usize) {
    // T1
    assert_eq!(up.keff.len(), n_expected, "{label}: reference steps");
    assert_eq!(port.len(), up.keff.len(), "{label}: port steps vs upstream steps");
    let last = port.last().unwrap().time;
    assert!((last - 60.0).abs() < 1e-9, "{label}: final time {last}");
    let mut max_dt_rel = 0.0_f64;
    let mut dks = Vec::new();
    let mut max_dp_rel = 0.0_f64;
    let mut max_res_rel_big = 0.0_f64;
    let mut max_res_abs_small = 0.0_f64;
    let mut worst_step = 0;
    for (i, r) in port.iter().enumerate() {
        let dt_rel = ((r.time - up.time[i]) / up.time[i]).abs();
        if dt_rel > 5e-8 && max_dt_rel <= 5e-8 {
            println!("{label}: first Time mismatch at step {}: port {:.12e} (dt {:.6e}) upstream {:.12e}; previous port {:.12e} upstream {:.12e}", i + 1, r.time, r.delta_t, up.time[i], port[i - 1].time, up.time[i - 1]);
        }
        max_dt_rel = max_dt_rel.max(dt_rel);
        let dk = (r.step.k_eff - up.keff[i]).abs();
        if dk > dks.iter().cloned().fold(0.0_f64, f64::max) {
            worst_step = i;
        }
        dks.push(dk);
        let (ru, rp) = (up.resid[i], r.step.first_residual);
        if !up.power.is_empty() {
            max_dp_rel = max_dp_rel.max(((r.step.power - up.power[i]) / up.power[i]).abs());
        }
        if ru >= 1e-6 {
            max_res_rel_big = max_res_rel_big.max(((rp - ru) / ru).abs());
        } else {
            max_res_abs_small = max_res_abs_small.max((rp - ru).abs());
        }
    }
    let max_dk = dks.iter().cloned().fold(0.0_f64, f64::max);
    let mut sorted = dks.clone();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let median = sorted[sorted.len() / 2];
    let (sp, su) = (shape(flux), shape(&up.flux));
    let flux_rel = sp.iter().zip(&su).map(|(a, b)| ((a - b) / b).abs()).fold(0.0_f64, f64::max);
    println!(
        "{label}: steps {} | max |dt|/t {max_dt_rel:.2e} | max |dk| {max_dk:.3e} (step {}) median {median:.2e} | max rel dP {max_dp_rel:.2e} | resid rel (>=1e-6) {max_res_rel_big:.2e}, abs (<1e-6) {max_res_abs_small:.2e} | flux shape {flux_rel:.2e}",
        port.len(),
        worst_step + 1,
    );
    assert!(max_dt_rel < 5e-8, "{label}: T2 Time {max_dt_rel:e}");
    assert!(max_dk < 1e-12, "{label}: T3 dk {max_dk:e} at step {}", worst_step + 1);
    assert!(max_dp_rel < 1e-12, "{label}: T4 power {max_dp_rel:e}");
    assert!(max_res_rel_big < 1e-6, "{label}: T5 residual (big) {max_res_rel_big:e}");
    assert!(max_res_abs_small < 1e-9, "{label}: T5 residual (small) {max_res_abs_small:e}");
    assert!(flux_rel < 1e-10, "{label}: T7 flux shape {flux_rel:e}");
}

#[test]
fn diffusion_history_matches_upstream_step_by_step() {
    for (case, idx) in [("guided1", 1usize), ("guided2", 2)] {
        let up = upstream("guided_slabs", idx);
        let (h, flux) = run_diffusion(case);
        compare(&format!("diffusion {case}"), &up, &h, &flux, 230);
    }
}

#[test]
fn sp3_history_matches_upstream_step_by_step() {
    for (case, idx, n) in [("guided1", 1usize, 355usize), ("guided2", 2, 357)] {
        let up = upstream("sp3_slabs", idx);
        let (h, flux) = run_sp3(case);
        assert_eq!(up.power.len(), 0, "the SP3 reference log prints `pow:`, not `power:`");
        // T6
        // Step 1 is clamped in both cases; step 2 only in case 1 (the README's
        // T6 said both: a prediction that generalised from case 1's head).
        assert_eq!(h[0].step.k_eff, 0.1, "{case}: step 1 clamp");
        assert_eq!(up.keff[0], 0.1_f64);
        if idx == 1 {
            assert_eq!(h[1].step.k_eff, 0.1, "{case}: step 2 clamp");
            assert_eq!(up.keff[1], 0.1_f64);
        }
        compare(&format!("sp3 {case}"), &up, &h, &flux, n);
    }
}

/// The two refusals of the driver: a state-only SP3 scaffold has no cross
/// sections, and circulating fuel (`precEq.H`'s transported branch) is not
/// stepped by it.
#[test]
fn the_driver_refuses_what_it_does_not_implement() {
    use outram_foam_appbuilder_lib::genfoam::neutronics::diffusion::PrecursorTransport;
    use outram_foam_appbuilder_lib::genfoam::neutronics::pseudo_time::PseudoTimeError;
    let s = setup("guided1");
    let mut scaffold = Sp3Neutronics::new(s.mesh.clone(), 1, 6);
    let err = run_pseudo_time_eigenvalue(&mut scaffold, &controls()).unwrap_err();
    assert_eq!(err, PseudoTimeError::NoCrossSections);

    let mut diffusion = DiffusionNeutronics::new(
        s.mesh.clone(),
        &s.xs,
        &s.zone_map,
        &[],
        &s.flux_boundary,
        DiffusionSettings::default(),
    )
    .expect("diffusion");
    let n = s.mesh.n_cells;
    let phi_boundary: Vec<Vec<f64>> = s.mesh.patches.iter().map(|p| vec![0.0; p.size]).collect();
    diffusion.set_precursor_drift(PrecursorTransport::new(
        &s.mesh,
        vec![1.0; n],
        vec![0.0; s.mesh.n_internal_faces],
        phi_boundary,
        vec![0.0; n],
    ));
    let err = run_pseudo_time_eigenvalue(&mut diffusion, &controls()).unwrap_err();
    assert_eq!(err, PseudoTimeError::LiquidFuel);
}
