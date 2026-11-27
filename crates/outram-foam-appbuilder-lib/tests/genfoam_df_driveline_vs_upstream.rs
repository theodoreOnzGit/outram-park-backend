// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//! # GeN-Foam XS discontinuity-factor adjustment and control-rod driveline against the real binary (gh:#1785, gh:#1474)
//!
//! AI-written, not human-reviewed. Research, education and V&V only.
//!
//! ## Methodology
//!
//! Upstream: GeN-Foam `652b3da0cc55` on OpenFOAM-ESI v2506, the unmodified `GeN-Foam` binary at the tight protocol of
//! round 2 and, for the XS-level instrument, `upstream_driver/dump_xs_df` (a class derived from upstream's own `XS`).
//! Case: guided slab 2 re-zoned into `reflector`, `coreLow`, `follower`, `controlRod` with a synthetic two-group data
//! set (`scripts/make_df_driveline_cases.sh`, `cases/df_slab`). Eight variants (README "Round 39"): `v0` (control),
//! `v1`, `v2` (adjustment), `v3` (given factors), `v0_d1`, `v1_d1`, `v2_d1`, `v0_d3` (driveline, also with adjustment).
//! References: `refs/df_slab_solver.ref.txt.gz` (per step `TIME`, `keff_`, `power:`, first residual; final fluxes) and
//! `refs/df_slab_xs.ref.txt.gz` (per probe time: rod position, `discFactorList_` after `adjustDiscFactors`, twelve
//! per-cell fields).
//!
//! Pass criteria were fixed in the README before any run: XS level, every value `==`; solver level, round 2's T1 to T7
//! (steps and final time equal, `Time` rel < 5e-8, `|dk|` < 1e-12, power rel < 1e-12, residual rel < 1e-6 / abs < 1e-9,
//! unit-sum shape of both final fluxes rel < 1e-10). Results: README, "Round 39 results"; `--nocapture` prints every
//! tally.

// Reads OpenFOAM case files: `std` only (gh:#1633).
#![cfg(feature = "std")]
use std::path::{Path, PathBuf};
use std::sync::Arc;

use miniz_oxide::inflate::decompress_to_vec;
use outram_foam_appbuilder_lib::genfoam::neutronics::diffusion::{
    DiffusionNeutronics, DiffusionSettings, XsEvolution,
};
use outram_foam_appbuilder_lib::genfoam::neutronics::pseudo_time::{
    run_pseudo_time_eigenvalue, PseudoTimeControls, PseudoTimeError, PseudoTimeRecord,
};
use outram_foam_appbuilder_lib::genfoam::neutronics::sp3::{Sp3Neutronics, Sp3Settings};
use outram_foam_appbuilder_lib::genfoam::neutronics::xs::disc_factors::DiscFactorControls;
use outram_foam_appbuilder_lib::genfoam::neutronics::xs::driveline::ControlRod;
use outram_foam_appbuilder_lib::genfoam::neutronics::xs::CrossSectionData;
use outram_foam_appbuilder_lib::io::nuclear_data::{read_control_rods, read_disc_factor_controls, read_nuclear_data};
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
    rods: Vec<ControlRod>,
    controls: DiscFactorControls,
    flux_boundary: Vec<BoundaryCondition<f64>>,
}

/// Stage `variant` (`<nuclearData>[_<CRmove>]`) into a scratch directory and read it.
fn load(variant: &str) -> Case {
    let (nd, cr) = match variant.split_once('_') {
        Some((a, b)) => (a, Some(b)),
        None => (variant, None),
    };
    let vv = vv_dir();
    let dst = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("gh1785_{variant}"));
    let _ = std::fs::remove_dir_all(&dst);
    std::fs::create_dir_all(dst.join("polyMesh")).expect("scratch dir");
    for f in ["points", "faces", "owner", "neighbour", "boundary"] {
        let data = gunzip(&vv.join("cases/guided2/polyMesh").join(format!("{f}.gz")));
        std::fs::write(dst.join("polyMesh").join(f), data).expect("write");
    }
    std::fs::write(dst.join("polyMesh/cellZones"), gunzip(&vv.join("cases/df_slab/cellZones.gz"))).expect("zones");
    std::fs::copy(vv.join(format!("cases/df_slab/nuclearData.{nd}")), dst.join("nuclearData")).expect("nuclearData");
    if let Some(cr) = cr {
        std::fs::copy(vv.join(format!("cases/df_slab/CRmove.{cr}")), dst.join("CRmove")).expect("CRmove");
    }
    let mesh = read_poly_mesh(&dst.join("polyMesh")).expect("polyMesh");
    let input = read_nuclear_data(&dst.join("nuclearData")).expect("nuclearData");
    let xs = CrossSectionData::from_input(&input).expect("cross sections");
    let zones = read_cell_zones(&dst.join("polyMesh")).expect("cellZones");
    let order: Vec<&str> = (0..xs.zone_count()).map(|i| xs.zone(i).expect("zone").name()).collect();
    let mesh_zone = zone_of_cell(&zones, &order, mesh.n_cells).expect("zone map");
    let cell_zones = zones.iter().map(|z| (z.name.clone(), z.cells.clone())).collect();
    let rods = read_control_rods(&dst.join("CRmove")).expect("CRmove");
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
    Case { mesh, xs, mesh_zone, cell_zones, rods, controls, flux_boundary }
}

fn evolution(c: &Case, rods: Vec<ControlRod>, controls: DiscFactorControls) -> XsEvolution {
    XsEvolution::new(&c.xs, &c.mesh, c.mesh_zone.clone(), &c.cell_zones, Vec::new(), rods, controls).expect("evolution")
}

fn solver(c: &Case) -> DiffusionNeutronics {
    solver_with(c, false, 100_000)
}

/// `basic_lib`: the deprecated pre-round-40 linear solves (`with_basic_lib_cg`); `max_iter`: the `maxIter` of the
/// solve (upstream's own is 1000, the default of OpenFOAM, as the case's `fvSolution` sets none).
fn solver_with(c: &Case, basic_lib: bool, max_iter: usize) -> DiffusionNeutronics {
    let mut settings = DiffusionSettings {
        linear: SolverSettings { tolerance: 1e-14, max_iter },
        ..DiffusionSettings::default()
    };
    if basic_lib {
        settings = settings.with_basic_lib_cg();
    }
    DiffusionNeutronics::new(c.mesh.clone(), &c.xs, &c.mesh_zone, &[], &c.flux_boundary, settings).expect("diffusion")
}

fn pt_controls() -> PseudoTimeControls {
    PseudoTimeControls::guided(1e-9, 60.0, 1.0, 1.0, 0.01, 1e6)
}

struct Upstream {
    time: Vec<f64>,
    keff: Vec<f64>,
    power: Vec<f64>,
    resid: Vec<f64>,
    flux: [Vec<f64>; 2],
}

fn parse_floats(it: std::str::SplitWhitespace) -> Vec<f64> {
    it.map(|w| w.parse().unwrap()).collect()
}

fn upstream(variant: &str) -> Upstream {
    let text = String::from_utf8(gunzip(&vv_dir().join("refs/df_slab_solver.ref.txt.gz"))).unwrap();
    let mut on = false;
    let mut u = Upstream { time: vec![], keff: vec![], power: vec![], resid: vec![], flux: [vec![], vec![]] };
    for line in text.lines() {
        let mut it = line.split_whitespace();
        let key = it.next();
        if key == Some("CASE") {
            on = it.next() == Some(variant);
            continue;
        }
        if !on {
            continue;
        }
        match key {
            Some("TIME") => u.time = parse_floats(it),
            Some("KEFF") => u.keff = parse_floats(it),
            Some("POWER") => u.power = parse_floats(it),
            Some("RESID") => u.resid = parse_floats(it),
            Some("FLUX0") => u.flux[0] = parse_floats(it),
            Some("FLUX1") => u.flux[1] = parse_floats(it),
            _ => {}
        }
    }
    assert!(!u.keff.is_empty(), "no reference for {variant}");
    u
}

fn shape(v: &[f64]) -> Vec<f64> {
    let s: f64 = v.iter().sum();
    v.iter().map(|x| x / s).collect()
}

struct Tally {
    steps: usize,
    max_dt_rel: f64,
    max_dk: f64,
    worst_step: usize,
    max_dp_rel: f64,
    resid_rel: f64,
    resid_abs: f64,
    flux_rel: f64,
    last_dk: f64,
    first_dk: f64,
}

fn tally(up: &Upstream, port: &[PseudoTimeRecord], flux: &[Vec<f64>]) -> Tally {
    assert_eq!(port.len(), up.keff.len(), "port steps vs upstream steps");
    let mut t = Tally {
        steps: port.len(),
        max_dt_rel: 0.0,
        max_dk: 0.0,
        worst_step: 0,
        max_dp_rel: 0.0,
        resid_rel: 0.0,
        resid_abs: 0.0,
        flux_rel: 0.0,
        last_dk: 0.0,
        first_dk: 0.0,
    };
    for (i, r) in port.iter().enumerate() {
        t.max_dt_rel = t.max_dt_rel.max(((r.time - up.time[i]) / up.time[i]).abs());
        let dk = (r.step.k_eff - up.keff[i]).abs();
        if dk > t.max_dk {
            t.max_dk = dk;
            t.worst_step = i + 1;
        }
        if i == 0 {
            t.first_dk = dk;
        }
        t.last_dk = dk;
        t.max_dp_rel = t.max_dp_rel.max(((r.step.power - up.power[i]) / up.power[i]).abs());
        let (ru, rp) = (up.resid[i], r.step.first_residual);
        if ru >= 1e-6 {
            t.resid_rel = t.resid_rel.max(((rp - ru) / ru).abs());
        } else {
            t.resid_abs = t.resid_abs.max((rp - ru).abs());
        }
    }
    for g in 0..2 {
        let (sp, su) = (shape(&flux[g]), shape(&up.flux[g]));
        t.flux_rel = t.flux_rel.max(sp.iter().zip(&su).map(|(a, b)| ((a - b) / b).abs()).fold(0.0_f64, f64::max));
    }
    t
}

fn print_tally(label: &str, t: &Tally, up: &Upstream) {
    println!(
        "{label}: steps {} | max |dt|/t {:.2e} | max |dk| {:.3e} (step {}) first {:.2e} last {:.2e} | max rel dP {:.2e} | resid rel {:.2e} abs {:.2e} | flux shape {:.2e} | upstream keff first {:.6} last {:.6} (range {:.4})",
        t.steps,
        t.max_dt_rel,
        t.max_dk,
        t.worst_step,
        t.first_dk,
        t.last_dk,
        t.max_dp_rel,
        t.resid_rel,
        t.resid_abs,
        t.flux_rel,
        up.keff[0],
        up.keff[up.keff.len() - 1],
        up.keff.iter().cloned().fold(f64::MIN, f64::max) - up.keff.iter().cloned().fold(f64::MAX, f64::min),
    );
}

/// Round 2's T1 to T7.
fn assert_criteria(label: &str, t: &Tally, port: &[PseudoTimeRecord]) {
    let last = port.last().unwrap().time;
    assert!((last - 60.0).abs() < 1e-9, "{label}: final time {last}");
    assert!(t.max_dt_rel < 5e-8, "{label}: Time {:e}", t.max_dt_rel);
    assert!(t.max_dk < 1e-12, "{label}: dk {:e} at step {}", t.max_dk, t.worst_step);
    assert!(t.max_dp_rel < 1e-12, "{label}: power {:e}", t.max_dp_rel);
    assert!(t.resid_rel < 1e-6, "{label}: residual rel {:e}", t.resid_rel);
    assert!(t.resid_abs < 1e-9, "{label}: residual abs {:e}", t.resid_abs);
    assert!(t.flux_rel < 1e-10, "{label}: flux shape {:e}", t.flux_rel);
}

fn final_fluxes(s: &DiffusionNeutronics) -> Vec<Vec<f64>> {
    s.state().flux().iter().map(|f| f.internal.as_slice().to_vec()).collect()
}

/// Run the port on `variant` with the chosen pieces of `XS::correct()` switched on.
fn run_port(
    c: &Case,
    rods: Vec<ControlRod>,
    adjust: bool,
    evolve: bool,
) -> (Vec<PseudoTimeRecord>, Vec<Vec<f64>>) {
    run_port_with(c, rods, adjust, evolve, false, 100_000)
}

fn run_port_with(
    c: &Case,
    rods: Vec<ControlRod>,
    adjust: bool,
    evolve: bool,
    basic_lib: bool,
    max_iter: usize,
) -> (Vec<PseudoTimeRecord>, Vec<Vec<f64>>) {
    let mut s = solver_with(c, basic_lib, max_iter);
    if evolve {
        let mut controls = c.controls.clone();
        controls.adjust_disc_factors = adjust;
        s.set_xs_evolution(evolution(c, rods, controls));
    }
    let h = run_pseudo_time_eigenvalue(&mut s, &pt_controls()).expect("pseudo-time loop");
    (h, final_fluxes(&s))
}

fn solver_variant(variant: &str) -> (Tally, Upstream, Vec<PseudoTimeRecord>) {
    let c = load(variant);
    let up = upstream(variant);
    let (h, flux) = run_port(&c, c.rods.clone(), c.controls.adjust_disc_factors, true);
    let t = tally(&up, &h, &flux);
    print_tally(variant, &t, &up);
    (t, up, h)
}

fn check_variant(variant: &str) {
    let (t, _, h) = solver_variant(variant);
    assert_criteria(variant, &t, &h);
}

#[test]
fn v0_control_without_discontinuity_factors_matches_upstream() {
    check_variant("v0");
}

#[test]
fn v1_adjustment_matches_upstream() {
    check_variant("v1");
}

#[test]
fn v2_adjustment_with_skipped_group_and_zone_matches_upstream() {
    check_variant("v2");
}

#[test]
fn v3_given_discontinuity_factors_match_upstream() {
    check_variant("v3");
}

#[test]
fn v0_d1_driveline_entering_matches_upstream() {
    check_variant("v0_d1");
}

/// CONTROL (round 40): `v1_d1` and `v2_d1` with the DEPRECATED basic-lib linear solves (`with_basic_lib_cg`) still MISS
/// the criteria exactly as in round 39, which pins the cause of that miss to the linear solver's stopping rule. The
/// original text of round 39 follows. `v1_d1` and `v2_d1` MISS the pre-registered criteria (README "Round 39 results", predictions C2/C4 missed): the
/// adjustment drives the rod zone's factor to ~1e-14, and from one step on upstream's first solve takes 0 iterations at
/// an initial residual just under its 1e-14 tolerance while its power is 5e-3 to 7e-3 off `pTarget`, i.e. UPSTREAM's
/// flux is the unconverged one (the port's power is within 2e-5 of the target at that step, trending as before). This
/// test asserts what holds, stated AFTER the run and not replacing the criteria: every step before upstream's power
/// leaves the target by more than 1e-3 meets `Time`, `|dk|` < 1e-12, power and residual, the departure exists, and at
/// that step the port is the converged one.
fn check_variant_until_upstream_unconverged(variant: &str) {
    let c = load(variant);
    let up = upstream(variant);
    let (h, _) = run_port_with(&c, c.rods.clone(), c.controls.adjust_disc_factors, true, true, 100_000);
    assert_eq!(h.len(), up.keff.len());
    let k = (0..h.len())
        .find(|&i| (h[i].step.k_eff - up.keff[i]).abs() > 1e-12)
        .unwrap_or_else(|| panic!("{variant}: upstream never leaves the target; the miss is gone, update the README"));
    let mut worst_dk = 0.0_f64;
    let mut worst_dp = 0.0_f64;
    for i in 0..k {
        worst_dk = worst_dk.max((h[i].step.k_eff - up.keff[i]).abs());
        worst_dp = worst_dp.max(((h[i].step.power - up.power[i]) / up.power[i]).abs());
        let dt = ((h[i].time - up.time[i]) / up.time[i]).abs();
        assert!(dt < 5e-8, "{variant}: Time step {}", i + 1);
        let (ru, rp) = (up.resid[i], h[i].step.first_residual);
        if ru >= 1e-6 {
            assert!(((rp - ru) / ru).abs() < 1e-6, "{variant}: residual step {}", i + 1);
        } else {
            assert!((rp - ru).abs() < 1e-9, "{variant}: residual step {}", i + 1);
        }
    }
    let port_off = (h[k].step.power / 1e6 - 1.0).abs();
    let dk_k = (h[k].step.k_eff - up.keff[k]).abs();
    println!(
        "{variant}: steps before upstream leaves its target: {k} (first unconverged upstream step {}, t={}) | max |dk| before {worst_dk:.2e}, max rel dP before {worst_dp:.2e} | at that step: upstream power {:.3e} off target, port {port_off:.2e} off, |dk| {dk_k:.3e}, upstream first residual {:.3e}",
        k + 1,
        h[k].time,
        up.power[k] / 1e6 - 1.0,
        up.resid[k],
    );
    assert!(worst_dk < 1e-12, "{variant}: dk before the departure {worst_dk:e}");
    assert!(worst_dp < 1e-12, "{variant}: power before the departure {worst_dp:e}");
    assert!(port_off < 1e-4, "{variant}: the port converges at the departure step ({port_off:e})");
    assert!(dk_k > 1e-3, "{variant}: the pre-registered criterion is missed at the departure step");
    assert!(
        (up.power[k] / 1e6 - 1.0).abs() > 1e-3,
        "{variant}: upstream's own power is off its target at the departure step"
    );
}

/// Round 40: with upstream's `PCG`/`DIC` and its normalised-residual stopping rule the port follows upstream through all
/// 230 steps, past the step (202) where upstream's first `fluxStar0` solve does 0 iterations.
#[test]
fn v1_d1_adjustment_and_driveline_match_upstream() {
    check_variant("v1_d1");
}

#[test]
fn v2_d1_adjustment_and_driveline_match_upstream() {
    check_variant("v2_d1");
}

#[test]
fn control_basic_lib_cg_misses_v1_d1_at_step_202() {
    check_variant_until_upstream_unconverged("v1_d1");
}

#[test]
fn control_basic_lib_cg_misses_v2_d1_at_step_205() {
    check_variant_until_upstream_unconverged("v2_d1");
}

/// S7: upstream's `maxIter` is OpenFOAM's default 1000 (the case's `fvSolution` sets none); the tests use 100 000. With
/// 1000 the port's numbers are bit-identical, i.e. no solve of these runs needs more than 1000 iterations.
#[test]
fn max_iter_1000_changes_nothing_on_v1_d1_and_v2_d1() {
    for variant in ["v1_d1", "v2_d1"] {
        let c = load(variant);
        let (a, fa) = run_port_with(&c, c.rods.clone(), c.controls.adjust_disc_factors, true, false, 100_000);
        let (b, fb) = run_port_with(&c, c.rods.clone(), c.controls.adjust_disc_factors, true, false, 1000);
        assert_eq!(a.len(), b.len());
        for i in 0..a.len() {
            assert_eq!(a[i].step.k_eff, b[i].step.k_eff, "{variant} step {}", i + 1);
        }
        assert_eq!(fa, fb, "{variant} final flux");
    }
}

#[test]
fn v0_d3_driveline_retracting_matches_upstream() {
    check_variant("v0_d3");
}

/// K1: every factor forced to 1 (the pre-round-39 form) on `v3`.
#[test]
fn control_k1_without_given_factors_differs() {
    let c = load("v3");
    let up = upstream("v3");
    let mut s = solver(&c).without_disc_factors();
    let h = run_pseudo_time_eigenvalue(&mut s, &pt_controls()).expect("loop");
    let dk = (h[0].step.k_eff - up.keff[0]).abs();
    println!("K1 v3 without factors: step-1 |dk| {dk:.3e}");
    assert!(dk > 1e-4, "K1: {dk:e}");
}

/// K2: no adjustment on `v1`.
#[test]
fn control_k2_without_adjustment_differs() {
    let c = load("v1");
    let up = upstream("v1");
    let (h, _) = run_port(&c, Vec::new(), false, true);
    let dk = (h[h.len() - 1].step.k_eff - up.keff[up.keff.len() - 1]).abs();
    println!("K2 v1 without adjustment: last-step |dk| {dk:.3e}");
    assert!(dk > 1e-4, "K2: {dk:e}");
}

/// K3: no driveline on `v0_d1`.
#[test]
fn control_k3_without_driveline_differs() {
    let c = load("v0_d1");
    let up = upstream("v0_d1");
    let (h, _) = run_port(&c, Vec::new(), false, true);
    let dk = (h[h.len() - 1].step.k_eff - up.keff[up.keff.len() - 1]).abs();
    println!("K3 v0_d1 without driveline: last-step |dk| {dk:.3e}");
    assert!(dk > 1e-4, "K3: {dk:e}");
}

// ---- instrument 1: the XS class ------------------------------------------------------------------------------

struct XsStep {
    time: f64,
    cr: Vec<f64>,
    dflist: Vec<f64>,
    arrays: Vec<(String, Vec<f64>)>,
}

fn xs_reference(variant: &str) -> Vec<XsStep> {
    let text = String::from_utf8(gunzip(&vv_dir().join("refs/df_slab_xs.ref.txt.gz"))).unwrap();
    let mut on = false;
    let mut out: Vec<XsStep> = Vec::new();
    for line in text.lines() {
        let mut it = line.split_whitespace();
        match it.next() {
            Some("CASE") => on = it.next() == Some(variant),
            Some("T") if on => {
                it.next();
                out.push(XsStep {
                    time: it.next().unwrap().parse().unwrap(),
                    cr: vec![],
                    dflist: vec![],
                    arrays: vec![],
                });
            }
            Some("CR") if on => out.last_mut().unwrap().cr = parse_floats(it),
            Some("DFLIST") if on => out.last_mut().unwrap().dflist = parse_floats(it),
            Some("ARR") if on => {
                let name = it.next().unwrap().to_string();
                out.last_mut().unwrap().arrays.push((name, parse_floats(it)));
            }
            _ => {}
        }
    }
    assert!(!out.is_empty(), "no xs reference for {variant}");
    out
}

/// `1 + double((7c + 13g + 17i) % 101)/101` of `dump_xs_df`.
fn synthetic_flux_star(n: usize, groups: usize, i: usize) -> Vec<Vec<f64>> {
    (0..groups)
        .map(|g| (0..n).map(|c| 1.0 + ((7 * c + 13 * g + 17 * i) % 101) as f64 / 101.0).collect())
        .collect()
}

fn array_of(f: &outram_foam_appbuilder_lib::genfoam::neutronics::diffusion::DiffusionXsFields, name: &str) -> Vec<f64> {
    let take = |v: &outram_foam_basic_lib::fields::VolScalarField| v.internal.as_slice().to_vec();
    match name {
        "D0" => take(&f.d[0]),
        "D1" => take(&f.d[1]),
        "nuSigmaEff0" => take(&f.nu_sigma_f[0]),
        "nuSigmaEff1" => take(&f.nu_sigma_f[1]),
        "sigmaRemoval0" => take(&f.sigma_removal[0]),
        "sigmaRemoval1" => take(&f.sigma_removal[1]),
        "discFactor0" => take(&f.disc_factor[0]),
        "discFactor1" => take(&f.disc_factor[1]),
        "IV0" => take(&f.inv_velocity[0]),
        "IV1" => take(&f.inv_velocity[1]),
        "sigmaFromTo_0_1" => take(&f.scattering[0][1]),
        "BetaTot" => take(&f.beta_tot),
        other => panic!("unknown array {other}"),
    }
}

/// Returns (arrays compared, arrays bit-identical, worst CRposition relative difference, values of dfList compared,
/// of which bit-identical, upstream's largest |df - 1|).
fn xs_compare(variant: &str, physical_flux_for_adjust: bool) -> (usize, usize, f64, usize, usize, f64, Vec<f64>) {
    let c = load(variant);
    let reference = xs_reference(variant);
    let mut evo = evolution(&c, c.rods.clone(), c.controls.clone());
    let n = c.mesh.n_cells;
    let (mut arrays, mut same_arrays, mut worst_cr, mut dfs, mut same_dfs, mut df_dev) = (0, 0, 0.0_f64, 0, 0, 0.0_f64);
    let mut dfdiff = Vec::new();
    for (k, step) in reference.iter().enumerate() {
        let fields = evo.materialize(&c.mesh, step.time).expect("materialize");
        let positions = evo.driveline().map(|d| d.positions(step.time)).unwrap_or_default();
        assert_eq!(positions.len(), step.cr.len(), "{variant}: rod count at t={}", step.time);
        for (p, r) in positions.iter().zip(&step.cr) {
            worst_cr = worst_cr.max(((p - r) / r).abs());
        }
        for (name, up) in &step.arrays {
            let port = array_of(&fields, name);
            arrays += 1;
            if port == *up {
                same_arrays += 1;
            } else if !physical_flux_for_adjust {
                let worst = port.iter().zip(up).map(|(a, b)| ((a - b) / b).abs()).fold(0.0_f64, f64::max);
                println!("{variant}: t={} `{name}` differs, worst rel {worst:.3e}", step.time);
            }
        }
        let star = synthetic_flux_star(n, 2, k + 1);
        let fed = if physical_flux_for_adjust {
            (0..2)
                .map(|g| {
                    let df = fields.disc_factor[g].internal.as_slice();
                    (0..n).map(|cc| star[g][cc] / df[cc]).collect::<Vec<f64>>()
                })
                .collect()
        } else {
            star
        };
        evo.adjust_disc_factors(&fed, &c.mesh.cell_volumes);
        let port_df: Vec<f64> = evo.disc().values().iter().flat_map(|z| z.iter().copied()).collect();
        assert_eq!(port_df.len(), step.dflist.len());
        for (a, b) in port_df.iter().zip(&step.dflist) {
            dfs += 1;
            if a == b {
                same_dfs += 1;
            }
            df_dev = df_dev.max((b - 1.0).abs());
            dfdiff.push(((a - b) / b).abs());
        }
    }
    (arrays, same_arrays, worst_cr, dfs, same_dfs, df_dev, dfdiff)
}

fn xs_variant(variant: &str) {
    let (arrays, same_arrays, worst_cr, dfs, same_dfs, df_dev, _) = xs_compare(variant, false);
    println!(
        "XS {variant}: arrays {same_arrays}/{arrays} bit-identical | CRposition worst rel {worst_cr:.2e} | dfList {same_dfs}/{dfs} bit-identical | upstream max |df-1| {df_dev:.3e}"
    );
    assert_eq!(same_arrays, arrays, "{variant}: per-cell cross sections");
    assert_eq!(worst_cr, 0.0, "{variant}: CRposition");
    assert_eq!(same_dfs, dfs, "{variant}: discFactorList");
}

#[test]
fn xs_class_adjustment_is_bit_identical_v1() {
    xs_variant("v1");
    // A3: the case is not a null test.
    let (.., df_dev, _) = xs_compare("v1", false);
    assert!(df_dev > 1e-3, "upstream's factors move by {df_dev:e}");
}

#[test]
fn xs_class_driveline_is_bit_identical_v0_d1() {
    xs_variant("v0_d1");
}

#[test]
fn xs_class_driveline_with_adjustment_is_bit_identical_v1_d1() {
    xs_variant("v1_d1");
}

#[test]
fn xs_class_retracting_driveline_is_bit_identical_v0_d3() {
    xs_variant("v0_d3");
}

/// K4: adjusting with the physical flux instead of `fluxStar`.
#[test]
fn control_k4_adjusting_with_the_physical_flux_differs() {
    let (.., diffs) = xs_compare("v1", true);
    // 8 values per call; the first call starts from 1, where flux == fluxStar.
    let worst_later = diffs.iter().skip(8).cloned().fold(0.0_f64, f64::max);
    println!("K4 v1, adjust fed flux: worst relative difference of dfList from the 2nd call on {worst_later:.3e}");
    assert!(worst_later > 1e-6, "K4: {worst_later:e}");
}

/// B3 / B4: IV and BetaTot stay with the mesh zone; no rods means no remapping.
#[test]
fn constants_are_not_remapped_and_no_rods_means_no_remap() {
    let c = load("v0_d1");
    let evo = evolution(&c, c.rods.clone(), c.controls.clone());
    // Late in the run the rod has entered the follower: some follower cells carry the rod's cross sections.
    let late = evo.xs_zone_of_cell(40.0);
    let remapped: Vec<usize> = (0..late.len()).filter(|&i| late[i] != c.mesh_zone[i]).collect();
    assert!(!remapped.is_empty(), "the rod must have moved cells");
    let fields = evo.materialize(&c.mesh, 40.0).expect("fields");
    let rod_zone = c.xs.zone_index("controlRod").unwrap();
    let follower = c.xs.zone_index("follower").unwrap();
    for &i in &remapped {
        assert_eq!(c.mesh_zone[i], follower);
        assert_eq!(late[i], rod_zone);
        // IV is the follower's, equal to the rod's here only by construction of the data: compare BetaTot's zone source.
        let beta = c.xs.precursor_constants(follower).unwrap().beta_tot.value;
        assert_eq!(fields.beta_tot.internal.as_slice()[i], beta);
    }
    let none = evolution(&c, Vec::new(), c.controls.clone());
    assert_eq!(none.xs_zone_of_cell(40.0), c.mesh_zone, "B4");
}

/// R2: SP3 refuses a factor other than 1 by name.
#[test]
fn sp3_refuses_discontinuity_factors_other_than_one() {
    let c = load("v3");
    let settings = Sp3Settings {
        linear: SolverSettings { tolerance: 1e-14, max_iter: 100_000 },
        ..Sp3Settings::default()
    };
    let mut s =
        Sp3Neutronics::with_cross_sections(c.mesh.clone(), &c.xs, &c.mesh_zone, &[], &c.flux_boundary, settings)
            .expect("sp3");
    let err = run_pseudo_time_eigenvalue(&mut s, &pt_controls()).unwrap_err();
    assert_eq!(err, PseudoTimeError::DiscFactorsNotPorted);
}

/// The readers.
#[test]
fn the_readers_parse_the_keys_upstream_reads() {
    let c = load("v2_d1");
    assert!(c.controls.adjust_disc_factors);
    assert!(!c.controls.use_given_disc_factors);
    assert_eq!(c.controls.groups_wo_df, vec![1]);
    assert_eq!(c.rods.len(), 1);
    assert_eq!(c.rods[0].name, "controlRod");
    assert_eq!(c.rods[0].follower_name, "follower");
    assert_eq!((c.rods[0].start_time, c.rods[0].end_time, c.rods[0].speed), (10.0, 30.0, 0.005));
    let c0 = load("v0");
    assert!(c0.rods.is_empty() && !c0.controls.adjust_disc_factors && c0.controls.groups_wo_df.is_empty());
}

/// Diagnostic: the first step at which `|dk| > 1e-12` for each variant (prints; asserts nothing).
#[test]
#[ignore = "diagnostic"]
fn diagnostic_first_diverging_step() {
    for v in ["v1_d1", "v2_d1"] {
        let c = load(v);
        let up = upstream(v);
        let (h, _) = run_port(&c, c.rods.clone(), c.controls.adjust_disc_factors, true);
        let first = (0..h.len()).find(|&i| (h[i].step.k_eff - up.keff[i]).abs() > 1e-12);
        println!("{v}: first |dk| > 1e-12 at step {:?}", first.map(|i| i + 1));
        if let Some(i) = first {
            for j in i.saturating_sub(3)..(i + 3).min(h.len()) {
                println!(
                    "  step {} t={} port {:.17} up {:.17} dk {:.3e} | power/pTarget - 1: port {:.3e} upstream {:.3e} | upstream first residual {:.3e}",
                    j + 1,
                    h[j].time,
                    h[j].step.k_eff,
                    up.keff[j],
                    (h[j].step.k_eff - up.keff[j]).abs(),
                    h[j].step.power / 1e6 - 1.0,
                    up.power[j] / 1e6 - 1.0,
                    up.resid[j]
                );
            }
        }
    }
}
