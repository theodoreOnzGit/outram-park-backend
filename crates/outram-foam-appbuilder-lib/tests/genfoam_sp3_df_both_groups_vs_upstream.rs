// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//! # GeN-Foam SP3 with the discontinuity-factor adjustment of BOTH energy groups, against the real binary (gh:#1887, gh:#1898;
//! README "Round 47")
//!
//! AI-written, not human-reviewed. Research, education and V&V only.
//!
//! ## Methodology
//!
//! Upstream: GeN-Foam `652b3da0cc55` on OpenFOAM-ESI v2506, unmodified binary, tight protocol (writePrecision 17, solvers
//! 1e-14 / relTol 0), `SP3Neutronics`, pseudo-time eigenvalue run to 60 s, on the round-39 mesh (2.197 m slab, 451 cells, four
//! zones) with the PHYSICAL two-group data of `scripts/make_r47_data.sh` (IAEA 2-D benchmark cross sections, in-group
//! scattering from `D = 1/(3 Sigma_t)`), `integralFlux` = zone fractions of upstream's own converged unadjusted run
//! (`i1`: the SP3 scalar flux; `i2`: the diffusion flux). Why rounds 39 and 42 diverged (README "Round 47"): the shipped
//! SP3 start `defaultFlux2 = 1` is a NEGATIVE scalar flux (`flux = fluxStar - 2 fluxStar2`), so step 1 is a keff clamp and the
//! factors computed from it are negative; the physical start is `defaultFlux2 = 0`. References: `refs/r47_solver.ref.txt.gz`
//! (`scripts/extract_r47_reference.sh`). Pass criteria fixed in the README before the runs: round 42's eigenvalue level
//! (`|dk|` < 1e-12 every step, power rel < 1e-12, `Time` rel < 5e-8, residual rel < 1e-6 / abs < 1e-9, flux shape < 1e-10)
//! plus the final factors rel < 1e-9. Results: README, "Round 47 results"; `--nocapture` prints every tally.

#![cfg(feature = "std")]
use std::path::{Path, PathBuf};
use std::sync::Arc;

use miniz_oxide::inflate::decompress_to_vec;
use outram_foam_appbuilder_lib::genfoam::neutronics::diffusion::XsEvolution;
use outram_foam_appbuilder_lib::genfoam::neutronics::pseudo_time::{
    run_pseudo_time_eigenvalue, PseudoTimeControls, PseudoTimeRecord,
};
use outram_foam_appbuilder_lib::genfoam::neutronics::sp3::{Sp3Neutronics, Sp3Settings};
use outram_foam_appbuilder_lib::genfoam::neutronics::xs::disc_factors::DiscFactorControls;
use outram_foam_appbuilder_lib::genfoam::neutronics::xs::CrossSectionData;
use outram_foam_appbuilder_lib::io::nuclear_data::{read_disc_factor_controls, read_nuclear_data};
use outram_foam_appbuilder_lib::io::poly_mesh::{read_cell_zones, read_poly_mesh, zone_of_cell};
use outram_foam_basic_lib::fields::boundary::bc::BoundaryCondition;
use outram_foam_basic_lib::ldu_matrix::SolverSettings;
use outram_foam_basic_lib::mesh::FvMesh;

fn vv_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("verification_and_validation/genfoam_vs_upstream_gh1361")
}

fn gunzip(path: &Path) -> Vec<u8> {
    let bytes = std::fs::read(path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    decompress_to_vec(&bytes[10..bytes.len() - 8]).expect("inflate")
}

struct Case {
    mesh: Arc<FvMesh>,
    xs: CrossSectionData,
    mesh_zone: Vec<usize>,
    cell_zones: Vec<(String, Vec<usize>)>,
    controls: DiscFactorControls,
    flux_boundary: Vec<BoundaryCondition<f64>>,
}

/// Stage `cases/df_slab/nuclearData.<nd>` into a per-thread scratch directory and read it.
fn load(nd: &str) -> Case {
    let vv = vv_dir();
    let thread = format!("{:?}", std::thread::current().id()).replace(['(', ')'], "");
    let dst = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("gh1887r47_{nd}_{thread}"));
    let _ = std::fs::remove_dir_all(&dst);
    std::fs::create_dir_all(dst.join("polyMesh")).expect("scratch dir");
    for f in ["points", "faces", "owner", "neighbour", "boundary"] {
        let data = gunzip(&vv.join("cases/guided2/polyMesh").join(format!("{f}.gz")));
        std::fs::write(dst.join("polyMesh").join(f), data).expect("write");
    }
    std::fs::write(dst.join("polyMesh/cellZones"), gunzip(&vv.join("cases/df_slab/cellZones.gz"))).expect("zones");
    std::fs::copy(vv.join(format!("cases/df_slab/nuclearData.{nd}")), dst.join("nuclearData")).expect("nuclearData");
    let mesh = read_poly_mesh(&dst.join("polyMesh")).expect("polyMesh");
    let input = read_nuclear_data(&dst.join("nuclearData")).expect("nuclearData");
    let xs = CrossSectionData::from_input(&input).expect("cross sections");
    let zones = read_cell_zones(&dst.join("polyMesh")).expect("cellZones");
    let order: Vec<&str> = (0..xs.zone_count()).map(|i| xs.zone(i).expect("zone").name()).collect();
    let mesh_zone = zone_of_cell(&zones, &order, mesh.n_cells).expect("zone map");
    let cell_zones = zones.iter().map(|z| (z.name.clone(), z.cells.clone())).collect();
    let controls = read_disc_factor_controls(&dst.join("nuclearData")).expect("controls");
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
    Case { mesh, xs, mesh_zone, cell_zones, controls, flux_boundary }
}

fn evolution(c: &Case, adjust: bool) -> XsEvolution {
    let mut controls = c.controls.clone();
    controls.adjust_disc_factors = adjust;
    XsEvolution::new(&c.xs, &c.mesh, c.mesh_zone.clone(), &c.cell_zones, Vec::new(), Vec::new(), controls)
        .expect("evolution")
}

fn tight() -> SolverSettings {
    SolverSettings { tolerance: 1e-14, max_iter: 100_000 }
}

fn controls_to(end: f64) -> PseudoTimeControls {
    PseudoTimeControls::guided(1e-9, end, 1.0, 1.0, 0.01, 1e6)
}

/// SP3 at the second moment `flux2` (the `defaultFlux2` of the upstream run: 0 physical, 1 the shipped tutorial value).
fn sp3(c: &Case, flux2: f64) -> Sp3Neutronics {
    let settings = Sp3Settings { linear: tight(), ..Sp3Settings::default() };
    let mut s = Sp3Neutronics::with_cross_sections(c.mesh.clone(), &c.xs, &c.mesh_zone, &[], &c.flux_boundary, settings)
        .expect("sp3");
    s.set_uniform_second_moment(flux2);
    s
}

#[derive(Default, Clone)]
struct Up {
    time: Vec<f64>,
    keff: Vec<f64>,
    power: Vec<f64>,
    resid1: Vec<f64>,
    clamps: usize,
    dfall: Vec<f64>,
    flux: [Vec<f64>; 2],
}

fn reference(case: &str) -> Up {
    let text = String::from_utf8(gunzip(&vv_dir().join("refs/r47_solver.ref.txt.gz"))).unwrap();
    let mut on = false;
    let mut u = Up::default();
    for line in text.lines() {
        let mut it = line.split_whitespace();
        let key = it.next();
        if key == Some("CASE") {
            on = it.next() == Some(case);
            continue;
        }
        if !on {
            continue;
        }
        let v: Vec<f64> = it.map(|w| w.parse().unwrap()).collect();
        match key {
            Some("TIME") => u.time = v,
            Some("KEFF") => u.keff = v,
            Some("POWER") => u.power = v,
            Some("RESID1") => u.resid1 = v,
            Some("CLAMPS") => u.clamps = v[0] as usize,
            Some("DFALL") => u.dfall = v,
            Some("FLUX0") => u.flux[0] = v,
            Some("FLUX1") => u.flux[1] = v,
            _ => {}
        }
    }
    assert!(!u.keff.is_empty(), "no reference for {case}");
    u
}

fn shape(v: &[f64]) -> Vec<f64> {
    let s: f64 = v.iter().sum();
    v.iter().map(|x| x / s).collect()
}

fn rel(a: f64, b: f64) -> f64 {
    (a - b).abs() / b.abs().max(f64::MIN_POSITIVE)
}

struct Tally {
    max_dt_rel: f64,
    max_dk: f64,
    worst_step: usize,
    max_dp_rel: f64,
    resid_rel: f64,
    resid_abs: f64,
    flux_rel: f64,
    df_rel: f64,
    bit_identical: bool,
}

/// One port run: records, final flux, factors after the last step (`[group * 4 + zone]` like upstream's log order).
struct Run {
    history: Vec<PseudoTimeRecord>,
    flux: Vec<Vec<f64>>,
    df_last: Vec<f64>,
}

fn run(nd: &str, flux2: f64, adjust: bool, force_unity: bool, end: f64) -> Run {
    let c = load(nd);
    let mut s = sp3(&c, flux2);
    if force_unity {
        s = s.without_disc_factors();
    } else {
        s.set_xs_evolution(evolution(&c, adjust));
    }
    let history = run_pseudo_time_eigenvalue(&mut s, &controls_to(end)).expect("pseudo-time loop");
    let flux = s.state().flux().iter().map(|f| f.internal.as_slice().to_vec()).collect();
    let mut df_last = Vec::new();
    if let Some(e) = s.xs_evolution() {
        let v = e.disc().values();
        for g in 0..2 {
            for z in 0..v.len() {
                df_last.push(v[z][g]);
            }
        }
    }
    Run { history, flux, df_last }
}

fn tally(up: &Up, r: &Run, compare_flux: bool) -> Tally {
    assert_eq!(r.history.len(), up.keff.len(), "port steps vs upstream steps");
    let mut t = Tally {
        max_dt_rel: 0.0,
        max_dk: 0.0,
        worst_step: 0,
        max_dp_rel: 0.0,
        resid_rel: 0.0,
        resid_abs: 0.0,
        flux_rel: 0.0,
        df_rel: 0.0,
        bit_identical: true,
    };
    for (i, h) in r.history.iter().enumerate() {
        t.max_dt_rel = t.max_dt_rel.max(rel(h.time, up.time[i]));
        let dk = (h.step.k_eff - up.keff[i]).abs();
        if dk > t.max_dk {
            t.max_dk = dk;
            t.worst_step = i + 1;
        }
        if h.step.k_eff != up.keff[i] || h.step.power != up.power[i] {
            t.bit_identical = false;
        }
        t.max_dp_rel = t.max_dp_rel.max(rel(h.step.power, up.power[i]));
        let (ru, rp) = (up.resid1[i], h.step.first_residual);
        if ru >= 1e-6 {
            t.resid_rel = t.resid_rel.max(rel(rp, ru));
        } else {
            t.resid_abs = t.resid_abs.max((rp - ru).abs());
        }
    }
    if compare_flux {
        for g in 0..2 {
            let (sp, su) = (shape(&r.flux[g]), shape(&up.flux[g]));
            t.flux_rel = t.flux_rel.max(sp.iter().zip(&su).map(|(a, b)| rel(*a, *b)).fold(0.0_f64, f64::max));
        }
    }
    let n = r.history.len();
    // the unity run prints no factors (adjustDiscFactors never runs): nothing to compare
    if !r.df_last.is_empty() && !up.dfall.is_empty() {
        let last = &up.dfall[(n - 1) * 8..n * 8];
        t.df_rel = r.df_last.iter().zip(last).map(|(a, b)| rel(*a, *b)).fold(0.0_f64, f64::max);
    }
    t
}

fn print_tally(label: &str, t: &Tally, up: &Up) {
    let dfmin = up.dfall.iter().cloned().fold(f64::INFINITY, f64::min);
    let dfmax = up.dfall.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    println!(
        "{label}: steps {} | max |dt|/t {:.2e} | max |dk| {:.3e} (step {}) | max rel dP {:.2e} | resid rel {:.2e} abs {:.2e} | flux shape {:.2e} | final df rel {:.2e} | bit-identical keff+power {} | upstream keff {:.6} -> {:.6}, clamps {}, df range {:.3}..{:.3}",
        up.keff.len(),
        t.max_dt_rel,
        t.max_dk,
        t.worst_step,
        t.max_dp_rel,
        t.resid_rel,
        t.resid_abs,
        t.flux_rel,
        t.df_rel,
        t.bit_identical,
        up.keff[0],
        up.keff[up.keff.len() - 1],
        up.clamps,
        dfmin,
        dfmax,
    );
}

fn assert_criteria(label: &str, t: &Tally, last_time: f64) {
    assert!((last_time - 60.0).abs() < 1e-9, "{label}: final time {last_time}");
    assert!(t.max_dt_rel < 5e-8, "{label}: Time {:e}", t.max_dt_rel);
    assert!(t.max_dk < 1e-12, "{label}: dk {:e} at step {}", t.max_dk, t.worst_step);
    assert!(t.max_dp_rel < 1e-12, "{label}: power {:e}", t.max_dp_rel);
    assert!(t.resid_rel < 1e-6, "{label}: residual rel {:e}", t.resid_rel);
    assert!(t.resid_abs < 1e-9, "{label}: residual abs {:e}", t.resid_abs);
    assert!(t.flux_rel < 1e-10, "{label}: flux shape {:e}", t.flux_rel);
    assert!(t.df_rel < 1e-9, "{label}: final factors {:e}", t.df_rel);
}

fn check(nd: &str, case: &str) -> (Tally, Run, Up) {
    let up = reference(case);
    let r = run(nd, 0.0, true, false, 60.0);
    let t = tally(&up, &r, true);
    print_tally(&format!("SP3 {case}"), &t, &up);
    (t, r, up)
}

/// M4: the unity run of the new data (the adjustment off), physical start.
#[test]
fn m4_sp3_physical_data_unity_factors_match_upstream() {
    let up = reference("s_i0");
    let r = run("i0", 0.0, false, false, 60.0);
    let mut t = tally(&up, &r, true);
    t.df_rel = 0.0; // no adjustment: the factors stay 1, upstream prints none
    print_tally("SP3 i0 (unity)", &t, &up);
    assert_criteria("i0", &t, r.history.last().unwrap().time);
}

/// P1: both groups adjusting, reference = the SP3 scalar flux of the unadjusted run.
#[test]
fn p1_sp3_both_groups_adjusting_match_upstream() {
    let (t, r, up) = check("i1", "s_i1");
    assert_eq!(up.clamps, 0);
    assert_criteria("i1", &t, r.history.last().unwrap().time);
}

/// P2: both groups adjusting, reference = the diffusion flux.
#[test]
fn p2_sp3_both_groups_adjusting_to_the_diffusion_reference_match_upstream() {
    let (t, r, up) = check("i2", "s_i2");
    assert_eq!(up.clamps, 0);
    assert_criteria("i2", &t, r.history.last().unwrap().time);
}

/// K1 / K2 controls: no adjustment, and every factor forced to 1, on `i1`.
#[test]
fn k_controls_without_the_adjustment_differ_from_upstream() {
    let up = reference("s_i1");
    for (label, r) in [
        ("no adjustment", run("i1", 0.0, false, false, 60.0)),
        ("factors forced to 1", run("i1", 0.0, true, true, 60.0)),
    ] {
        let d = (r.history.last().unwrap().step.k_eff - up.keff[up.keff.len() - 1]).abs();
        println!("SP3 i1 {label}: |dk| at the last step {d:.3e}");
        assert!(d > 1e-4, "{label}: {d:e}");
    }
}

/// M1 / P3: the shipped start `defaultFlux2 = 1`. Upstream's own run on `i1` dies (FPE at step 58, 45 keff clamps, factors
/// from -177 to 334); the port's first step is compared with upstream's printed factors after step 1.
#[test]
fn p3_the_shipped_start_gives_upstream_s_nonphysical_first_factors() {
    let up = reference("s_i1_shipped");
    assert!(up.clamps > 0, "upstream clamps keff on the shipped start");
    let dfmin = up.dfall.iter().cloned().fold(f64::INFINITY, f64::min);
    assert!(dfmin < 0.0, "upstream's factors go negative: {dfmin}");
    let r = run("i1", 1.0, true, false, 1e-9);
    assert_eq!(r.history.len(), 1);
    let t = tally_first(&up, &r);
    println!("SP3 i1 shipped start: step-1 factors port against upstream, worst rel {t:.3e}; keff port {} upstream {}", r.history[0].step.k_eff, up.keff[0]);
    assert!(t < 1e-9, "{t:e}");
}

fn tally_first(up: &Up, r: &Run) -> f64 {
    r.df_last.iter().zip(&up.dfall[0..8]).map(|(a, b)| rel(*a, *b)).fold(0.0_f64, f64::max)
}
