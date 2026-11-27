// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//! # GeN-Foam neutronics: `fvSolution` solver entries and discontinuity factors in SP3, the stationary-fuel step and the
//! static eigenvalue solve, against the real binary (gh:#1887, gh:#1890; README "Round 42")
//!
//! AI-written, not human-reviewed. Research, education and V&V only.
//!
//! ## Methodology
//!
//! Upstream: GeN-Foam `652b3da0cc55` on OpenFOAM-ESI v2506, the unmodified binary at the tight protocol (writePrecision 17,
//! solvers 1e-14 / relTol 0 unless the variant says otherwise), on the round-39 `df_slab` case (guided slab 2 re-zoned,
//! synthetic two-group data) varied by `scripts/make_r42_cases.sh`, `run_r42_extsrc.sh`, `r42_sn_hook.sh`. References:
//! `refs/r42_solver.ref.txt.gz`, `refs/r42_extsrc.ref.txt.gz`, `refs/df_slab_solver.ref.txt.gz`.
//!
//! Pass criteria were fixed in the README ("Round 42, Predictions") before any run: eigenvalue runs, round 39's T1 to T7
//! (`|dk|` < 1e-12 every step, power rel < 1e-12, `Time` rel < 5e-8, residual rel < 1e-6 / abs < 1e-9, flux shape < 1e-10);
//! non-eigenvalue runs: step count, `Time` rel < 5e-8, `power:` rel < 1e-12, neutron-iteration count equal at every step,
//! last residual rel < 1e-6 (abs < 1e-9 below), final fluxes rel < 1e-10 of the group maximum. Results: README,
//! "Round 42 results"; `--nocapture` prints every tally.

#![cfg(feature = "std")]
use std::path::{Path, PathBuf};
use std::sync::Arc;

use miniz_oxide::inflate::decompress_to_vec;
use outram_foam_appbuilder_lib::genfoam::common::linear_solvers::{LinearSolverForm, NeutronSolverKind};
use outram_foam_appbuilder_lib::genfoam::neutronics::diffusion::external_source::{
    NeutronicsDdtScheme, NeutronicsLaplacianScheme, StationaryRecord, StationaryStepControls,
};
use outram_foam_appbuilder_lib::genfoam::neutronics::diffusion::external_source_case::ExternalSourceCase;
use outram_foam_appbuilder_lib::genfoam::neutronics::diffusion::{DiffusionNeutronics, DiffusionSettings, XsEvolution};
use outram_foam_appbuilder_lib::genfoam::neutronics::pseudo_time::{
    run_pseudo_time_eigenvalue, PseudoTimeControls, PseudoTimeRecord,
};
use outram_foam_appbuilder_lib::genfoam::neutronics::sp3::{Sp3Neutronics, Sp3Settings};
use outram_foam_appbuilder_lib::genfoam::neutronics::xs::disc_factors::DiscFactorControls;
use outram_foam_appbuilder_lib::genfoam::neutronics::xs::CrossSectionData;
use outram_foam_appbuilder_lib::io::nuclear_data::{
    read_disc_factor_controls, read_neutron_fv_solution, read_nuclear_data,
};
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

// ---------------------------------------------------------------------------------------------------------------
// the df_slab case
// ---------------------------------------------------------------------------------------------------------------

struct Case {
    mesh: Arc<FvMesh>,
    xs: CrossSectionData,
    mesh_zone: Vec<usize>,
    cell_zones: Vec<(String, Vec<usize>)>,
    controls: DiscFactorControls,
    flux_boundary: Vec<BoundaryCondition<f64>>,
    dir: PathBuf,
}

/// Stage `nuclearData.<nd>` (and `fvSolution.<fv>`, when given) of `cases/df_slab` into a scratch directory and read it.
fn load(nd: &str, fv: Option<&str>) -> Case {
    let vv = vv_dir();
    let tag = format!("{nd}_{}", fv.unwrap_or("none"));
    // one scratch directory per test thread: tests run in parallel and each stages and removes its own (round 43: the driveline
    // tests copy a `CRmove` into it after `load`, which a sibling test's `remove_dir_all` of a shared name would race with)
    let thread = format!("{:?}", std::thread::current().id()).replace(['(', ')'], "");
    let dst = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("gh1887_{tag}_{thread}"));
    let _ = std::fs::remove_dir_all(&dst);
    std::fs::create_dir_all(dst.join("polyMesh")).expect("scratch dir");
    for f in ["points", "faces", "owner", "neighbour", "boundary"] {
        let data = gunzip(&vv.join("cases/guided2/polyMesh").join(format!("{f}.gz")));
        std::fs::write(dst.join("polyMesh").join(f), data).expect("write");
    }
    std::fs::write(dst.join("polyMesh/cellZones"), gunzip(&vv.join("cases/df_slab/cellZones.gz"))).expect("zones");
    std::fs::copy(vv.join(format!("cases/df_slab/nuclearData.{nd}")), dst.join("nuclearData")).expect("nuclearData");
    if let Some(fv) = fv {
        std::fs::copy(vv.join(format!("cases/df_slab/fvSolution.{fv}")), dst.join("fvSolution")).expect("fvSolution");
    }
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
    Case { mesh, xs, mesh_zone, cell_zones, controls, flux_boundary, dir: dst }
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

fn pt_controls() -> PseudoTimeControls {
    PseudoTimeControls::guided(1e-9, 60.0, 1.0, 1.0, 0.01, 1e6)
}

fn diffusion(c: &Case) -> DiffusionNeutronics {
    let settings = DiffusionSettings { linear: tight(), ..DiffusionSettings::default() };
    DiffusionNeutronics::new(c.mesh.clone(), &c.xs, &c.mesh_zone, &[], &c.flux_boundary, settings).expect("diffusion")
}

fn sp3(c: &Case) -> Sp3Neutronics {
    let settings = Sp3Settings { linear: tight(), ..Sp3Settings::default() };
    let mut s = Sp3Neutronics::with_cross_sections(c.mesh.clone(), &c.xs, &c.mesh_zone, &[], &c.flux_boundary, settings)
        .expect("sp3");
    s.set_uniform_second_moment(1.0);
    s
}

// ---------------------------------------------------------------------------------------------------------------
// references
// ---------------------------------------------------------------------------------------------------------------

#[derive(Default, Clone)]
struct Up {
    time: Vec<f64>,
    keff: Vec<f64>,
    power: Vec<f64>,
    resid1: Vec<f64>,
    residl: Vec<f64>,
    niter: Vec<f64>,
    flux: [Vec<f64>; 2],
}

fn reference(file: &str, case: &str) -> Up {
    let text = String::from_utf8(gunzip(&vv_dir().join("refs").join(file))).unwrap();
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
            Some("RESID1") | Some("RESID") => {
                u.resid1 = v.clone();
                if key == Some("RESID") {
                    u.residl = v;
                }
            }
            Some("RESIDL") => u.residl = v,
            Some("NITER") => u.niter = v,
            Some("FLUX0") | Some("FLUX0_100") => u.flux[0] = v,
            Some("FLUX1") | Some("FLUX1_100") => u.flux[1] = v,
            _ => {}
        }
    }
    assert!(!u.time.is_empty() || !u.keff.is_empty(), "no reference for {case} in {file}");
    u
}

fn r42(case: &str) -> Up {
    reference("r42_solver.ref.txt.gz", case)
}

fn shape(v: &[f64]) -> Vec<f64> {
    let s: f64 = v.iter().sum();
    v.iter().map(|x| x / s).collect()
}

fn rel(a: f64, b: f64) -> f64 {
    (a - b).abs() / b.abs().max(f64::MIN_POSITIVE)
}

// ---------------------------------------------------------------------------------------------------------------
// eigenvalue (pseudo-time) comparison: round 39's T1 to T7
// ---------------------------------------------------------------------------------------------------------------

struct Tally {
    max_dt_rel: f64,
    max_dk: f64,
    worst_step: usize,
    first_dk: f64,
    max_dp_rel: f64,
    resid_rel: f64,
    resid_abs: f64,
    flux_rel: f64,
}

fn tally(up: &Up, port: &[PseudoTimeRecord], flux: &[Vec<f64>]) -> Tally {
    assert_eq!(port.len(), up.keff.len(), "port steps vs upstream steps");
    let mut t = Tally {
        max_dt_rel: 0.0,
        max_dk: 0.0,
        worst_step: 0,
        first_dk: 0.0,
        max_dp_rel: 0.0,
        resid_rel: 0.0,
        resid_abs: 0.0,
        flux_rel: 0.0,
    };
    for (i, r) in port.iter().enumerate() {
        t.max_dt_rel = t.max_dt_rel.max(rel(r.time, up.time[i]));
        let dk = (r.step.k_eff - up.keff[i]).abs();
        if dk > t.max_dk {
            t.max_dk = dk;
            t.worst_step = i + 1;
        }
        if i == 0 {
            t.first_dk = dk;
        }
        t.max_dp_rel = t.max_dp_rel.max(rel(r.step.power, up.power[i]));
        let (ru, rp) = (up.resid1[i], r.step.first_residual);
        if ru >= 1e-6 {
            t.resid_rel = t.resid_rel.max(rel(rp, ru));
        } else {
            t.resid_abs = t.resid_abs.max((rp - ru).abs());
        }
    }
    for g in 0..2 {
        let (sp, su) = (shape(&flux[g]), shape(&up.flux[g]));
        t.flux_rel = t.flux_rel.max(sp.iter().zip(&su).map(|(a, b)| rel(*a, *b)).fold(0.0_f64, f64::max));
    }
    t
}

fn print_tally(label: &str, t: &Tally, up: &Up) {
    println!(
        "{label}: steps {} | max |dt|/t {:.2e} | max |dk| {:.3e} (step {}) first {:.2e} | max rel dP {:.2e} | resid rel {:.2e} abs {:.2e} | flux shape {:.2e} | upstream keff first {:.6} last {:.6}",
        up.keff.len(),
        t.max_dt_rel,
        t.max_dk,
        t.worst_step,
        t.first_dk,
        t.max_dp_rel,
        t.resid_rel,
        t.resid_abs,
        t.flux_rel,
        up.keff[0],
        up.keff[up.keff.len() - 1],
    );
}

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

fn fluxes(s: &DiffusionNeutronics) -> Vec<Vec<f64>> {
    s.state().flux().iter().map(|f| f.internal.as_slice().to_vec()).collect()
}

fn run_diffusion_pt(s: &mut DiffusionNeutronics) -> (Vec<PseudoTimeRecord>, Vec<Vec<f64>>) {
    let h = run_pseudo_time_eigenvalue(s, &pt_controls()).expect("pseudo-time loop");
    (h, fluxes(s))
}

/// Diffusion pseudo-time run with the entries of `fvSolution.<fv>` read from the case.
fn run_with_fv(nd: &str, fv: &str, force_rel_tol_zero: bool, force_min_iter_zero: bool) -> (Vec<PseudoTimeRecord>, Vec<Vec<f64>>) {
    let c = load(nd, Some(fv));
    let sol = read_neutron_fv_solution(&c.dir.join("fvSolution"), 2, 6, 0).expect("fvSolution");
    let mut entries = sol.entries;
    if force_rel_tol_zero {
        entries.flux.rel_tol = 0.0;
    }
    if force_min_iter_zero {
        entries.flux.min_iter = 0;
    }
    let mut s = diffusion(&c).with_form(LinearSolverForm::UpstreamFvSolution(entries));
    run_diffusion_pt(&mut s)
}

trait WithForm {
    fn with_form(self, form: LinearSolverForm) -> Self;
}

impl WithForm for DiffusionNeutronics {
    fn with_form(mut self, form: LinearSolverForm) -> Self {
        self.set_linear_form(form);
        self
    }
}

// ---------------------------------------------------------------------------------------------------------------
// item 1: fvSolution
// ---------------------------------------------------------------------------------------------------------------

/// F1 (as amended after the run, README): the committed `df_slab` tight `fvSolution` reads as the tight protocol; the
/// committed `external_source` `fvSolution` is the SHIPPED one (tolerance 1e-6, relTol 1e-3 flux and precursors, 1e-7 / 0.1
/// angular flux), and `fvSolution.tight` / `.BR` are the staged variants of the upstream runs.
#[test]
fn f1_the_committed_fv_solutions_read_as_the_files_say() {
    let c = load("v0", Some("tight"));
    let e = read_neutron_fv_solution(&c.dir.join("fvSolution"), 2, 6, 4).expect("tight").entries;
    for x in [e.flux, e.precursor, e.angular_flux] {
        assert_eq!((x.tolerance, x.rel_tol, x.max_iter, x.min_iter), (1e-14, 0.0, 1000, 0));
    }
    assert_eq!(e.flux.kind, NeutronSolverKind::PcgDic);
    assert_eq!(e.precursor.kind, NeutronSolverKind::PbicgDilu);
    assert_eq!(e.angular_flux.kind, NeutronSolverKind::PbicgstabDilu);
    let m = read_neutron_fv_solution(&load("v0", Some("m")).dir.join("fvSolution"), 2, 6, 0).expect("m").entries;
    assert_eq!((m.flux.rel_tol, m.flux.min_iter, m.precursor.rel_tol), (0.1, 3, 0.0));
    // `Final` entries are read by nothing: the `fin` file resolves exactly as the tight one does
    let fin = read_neutron_fv_solution(&load("v0", Some("fin")).dir.join("fvSolution"), 2, 6, 4).expect("fin").entries;
    assert_eq!(fin, e);
    let shipped = read_neutron_fv_solution(&vv_dir().join("cases/external_source/system/neutroRegion/fvSolution"), 2, 6, 4)
        .expect("shipped")
        .entries;
    assert_eq!((shipped.flux.tolerance, shipped.flux.rel_tol), (1e-6, 1e-3));
    assert_eq!((shipped.precursor.tolerance, shipped.precursor.rel_tol), (1e-6, 1e-3));
    assert_eq!((shipped.angular_flux.tolerance, shipped.angular_flux.rel_tol), (1e-7, 0.1));
}

/// F4: `v0r` (flux relTol 0.1; a 1-D slab solve takes one iteration whatever relTol says, so this is a weak test of
/// relTol) and `v0m` (relTol 0.1, minIter 3, which acts) against the binary.
#[test]
fn f4_relative_tolerance_and_min_iter_match_upstream() {
    for (nd, fv, up) in [("v0", "r", "v0r"), ("v0", "m", "v0m")] {
        let (h, flux) = run_with_fv(nd, fv, false, false);
        let u = r42(up);
        let t = tally(&u, &h, &flux);
        print_tally(up, &t, &u);
        assert_criteria(up, &t, &h);
    }
}

/// F5: the `Final` entry is never selected by the neutronics: upstream's `fin` run equals the tight `v0` run bit for bit
/// (keff series), and the port reading `fvSolution.fin` equals the port reading `.tight`.
#[test]
fn f5_final_entries_are_never_selected() {
    let fin = r42("fin");
    let base = reference("df_slab_solver.ref.txt.gz", "v0");
    assert_eq!(fin.keff, base.keff, "upstream keff series with and without a Final entry");
    let (a, _) = run_with_fv("v0", "fin", false, false);
    let (b, _) = run_with_fv("v0", "tight", false, false);
    assert!(a.iter().zip(&b).all(|(x, y)| x.step.k_eff == y.step.k_eff && x.step.power == y.step.power));
}

/// F6 (MISSED as predicted, recorded post-run, README): the committed prediction was that forcing `minIter` to 0 departs
/// from `v0m` by `|dk| > 1e-13`. Measured: 6.7e-15 at most. Cause: on this 1-D slab DIC is the exact factorisation, so the
/// iterations `minIter 3` forces after convergence only move the solution by round-off. The effect of `minIter` on the
/// iteration count is pinned in the `linear_solvers` unit tests; the check here is that the control stays within round-off
/// of `v0m` (<= 1e-14), i.e. that the departure is NOT visible on this case.
#[test]
fn f6_controls_min_iter_is_not_visible_on_a_one_dimensional_slab() {
    let u = r42("v0m");
    for (zero_rel, zero_min, label) in [(false, true, "minIter 0"), (true, true, "relTol 0 and minIter 0")] {
        let (h, _) = run_with_fv("v0", "m", zero_rel, zero_min);
        let worst = h.iter().zip(&u.keff).map(|(r, k)| (r.step.k_eff - k).abs()).fold(0.0_f64, f64::max);
        println!("control {label}: max |dk| against v0m {worst:.3e}");
        assert!(worst <= 1e-14, "{label}: {worst:e}");
    }
}

// ----- external source: BR and BS -----

fn copy_dir(src: &Path, dst: &Path) {
    std::fs::create_dir_all(dst).expect("mkdir");
    for e in std::fs::read_dir(src).expect("read_dir") {
        let e = e.expect("entry");
        let to = dst.join(e.file_name());
        if e.file_type().expect("type").is_dir() {
            copy_dir(&e.path(), &to);
        } else if e.path().extension().is_some_and(|x| x == "gz") {
            let name = e.path().file_stem().expect("stem").to_owned();
            std::fs::write(dst.join(name), gunzip(&e.path())).expect("write");
        } else {
            std::fs::copy(e.path(), &to).expect("copy");
        }
    }
}

fn zero_gradient_walls(path: &Path) {
    let text = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let at = text.find("walls").expect("walls patch");
    let open = at + text[at..].find('{').expect("{");
    let close = open + text[open..].find('}').expect("}");
    let edited = format!("{}{{ type zeroGradient; {}", &text[..open], &text[close..]);
    std::fs::write(path, edited).expect("write");
}

/// Case B of round 32 (walls `zeroGradient`), with `system/neutroRegion/fvSolution` = `cases/external_source/<fv>` (`None`:
/// the committed, shipped one).
fn stage_b(name: &str, fv: Option<&str>) -> PathBuf {
    let dst = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("gh1887_ext_{name}"));
    let _ = std::fs::remove_dir_all(&dst);
    copy_dir(&vv_dir().join("cases/external_source"), &dst);
    for f in ["defaultFlux", "defaultFlux2", "defaultExternalSourceFlux"] {
        zero_gradient_walls(&dst.join("0/neutroRegion").join(f));
    }
    if let Some(fv) = fv {
        std::fs::copy(vv_dir().join(format!("cases/external_source/fvSolution.{fv}")), dst.join("system/neutroRegion/fvSolution"))
            .expect("fvSolution");
    }
    dst
}

struct ExtRun {
    history: Vec<StationaryRecord>,
    flux: Vec<Vec<f64>>,
    p_target: Option<f64>,
}

fn run_ext(dir: &Path, from_fv: bool) -> ExtRun {
    let mut case = if from_fv {
        ExternalSourceCase::read_with_fv_solution(dir).expect("read").0
    } else {
        ExternalSourceCase::read(dir, tight()).expect("read")
    };
    let (clock, step) = (case.clock, case.step);
    let (history, p_target) = case.model.run_stationary_transient(&clock, &step).expect("run");
    let flux = case.model.state().flux().iter().map(|f| f.internal.as_slice().to_vec()).collect();
    ExtRun { history, flux, p_target }
}

/// Worst power relative difference, iteration counts equal?, worst flux(t=100) / max, against `refs/r42_extsrc`.
fn ext_compare(name: &str, run: &ExtRun, up: &Up) -> (f64, usize, f64) {
    assert_eq!(run.history.len(), up.power.len(), "{name}: steps");
    let mut worst_p = 0.0_f64;
    let mut niter_equal = 0;
    for (i, h) in run.history.iter().enumerate() {
        worst_p = worst_p.max(rel(h.step.power, up.power[i]));
        if h.step.neutron_iterations as f64 == up.niter[i] {
            niter_equal += 1;
        }
    }
    let mut worst_f = 0.0_f64;
    for g in 0..2 {
        let scale = up.flux[g].iter().cloned().fold(0.0, f64::max);
        for (a, b) in run.flux[g].iter().zip(&up.flux[g]) {
            worst_f = worst_f.max((a - b).abs() / scale);
        }
    }
    println!("{name}: steps {} | power worst {worst_p:.3e} | iterations equal {niter_equal}/{} | flux(t=100) worst/max {worst_f:.3e}", run.history.len(), run.history.len());
    (worst_p, niter_equal, worst_f)
}

/// F2: reading the entries from the case's own `fvSolution` (tight copy) is the explicit tight settings, bit for bit.
#[test]
fn f2_reading_the_tight_fv_solution_changes_nothing() {
    let dir = stage_b("f2", Some("tight"));
    let a = run_ext(&dir, true);
    let b = run_ext(&dir, false);
    assert_eq!(a.history.len(), b.history.len());
    for (x, y) in a.history.iter().zip(&b.history) {
        assert_eq!(x.step.power, y.step.power);
        assert_eq!(x.step.neutron_iterations, y.step.neutron_iterations);
    }
    assert_eq!(a.flux, b.flux);
    assert_eq!(a.p_target, b.p_target);
}

/// F3: the flux solver at relTol 0.1 (`BR`) and the SHIPPED `fvSolution` untouched (`BS`: tolerance 1e-6, relTol 1e-3),
/// read from the case, against the binary; K1: the port with relTol 0 against `BR`.
#[test]
fn f3_ext_relative_tolerance_and_shipped_entries_match_upstream() {
    let ext = |case: &str| reference("r42_extsrc.ref.txt.gz", case);
    for (name, fv, up) in [("BR", Some("BR"), "BR"), ("BS", None, "BS")] {
        let dir = stage_b(name, fv);
        let run = run_ext(&dir, true);
        let (p, n, f) = ext_compare(name, &run, &ext(up));
        assert_eq!(n, run.history.len(), "{name}: iteration counts");
        // measured bit-identical (README, round 42): gate tightened from the committed 1e-12 to `==`
        assert!(p == 0.0, "{name}: power {p:e}");
        assert!(f == 0.0, "{name}: flux {f:e}");
    }
    let dir = stage_b("k1", Some("BR"));
    let ctrl = run_ext(&dir, false);
    let up = ext("BR");
    let worst = ctrl.history.iter().zip(&up.power).map(|(h, p)| rel(h.step.power, *p)).fold(0.0_f64, f64::max);
    println!("K1 control (relTol 0 against BR): worst power {worst:.3e}");
    assert!(worst > 1e-9, "relTol 0.1 must be visible: {worst:e}");
}

// ---------------------------------------------------------------------------------------------------------------
// item 2: discontinuity factors
// ---------------------------------------------------------------------------------------------------------------

/// S0..S3 and K: SP3 pseudo-time with factors. `variant`: nuclearData name, `adjust`: `adjustDiscFactors` of the data.
fn run_sp3_pt(nd: &str, force_unity: bool, adjust_off: bool) -> (Vec<PseudoTimeRecord>, Vec<Vec<f64>>) {
    let c = load(nd, None);
    let mut s = sp3(&c);
    if force_unity {
        s = s.without_disc_factors();
    } else {
        s.set_xs_evolution(evolution(&c, c.controls.adjust_disc_factors && !adjust_off));
    }
    let h = run_pseudo_time_eigenvalue(&mut s, &pt_controls()).expect("pseudo-time loop");
    let flux = s.state().flux().iter().map(|f| f.internal.as_slice().to_vec()).collect();
    (h, flux)
}

fn check_sp3(nd: &str, case: &str) -> Tally {
    let (h, flux) = run_sp3_pt(nd, false, false);
    let u = r42(case);
    let t = tally(&u, &h, &flux);
    print_tally(&format!("SP3 {case}"), &t, &u);
    t
}

#[test]
fn s0_sp3_unity_factors_match_upstream() {
    let t = check_sp3("v0", "s0");
    assert!(t.max_dk < 1e-12 && t.max_dp_rel < 1e-12 && t.flux_rel < 1e-10 && t.max_dt_rel < 5e-8);
}

#[test]
fn s1_sp3_given_factors_match_upstream() {
    let t = check_sp3("v3", "s3");
    assert!(t.max_dk < 1e-12 && t.max_dp_rel < 1e-12 && t.flux_rel < 1e-10 && t.max_dt_rel < 5e-8);
}

#[test]
fn s2_sp3_adjustment_with_skipped_group_and_zone_matches_upstream() {
    let t = check_sp3("s2", "s2");
    assert!(t.max_dk < 1e-12 && t.max_dp_rel < 1e-12 && t.flux_rel < 1e-10 && t.max_dt_rel < 5e-8);
}

/// `s4` (group 0 skipped, group 1 adjusted): MISSED as a whole-run comparison (README, post-run). The data drive UPSTREAM's
/// own adjustment loop into the keff clamp ping-pong 0.1 / 5 from step 4 on, a regime that amplifies a round-off by about
/// 10x per step (`dk` 1e-15 at step 2, 1.7e-10 at step 12, 1.1e-6 at 23, O(1) at 38; diagnostic `diag_sp3_s4_dk_series`).
/// The comparison is therefore asserted where it carries information: the first 11 steps, `|dk|` <= 1e-9 and power rel
/// <= 1e-9 (measured <= 8.9e-15 and <= 4.4e-10), not the 345.
#[test]
fn s2b_sp3_unstable_adjustment_agrees_until_round_off_is_amplified() {
    let (h, _) = run_sp3_pt("s4", false, false);
    let u = r42("s4");
    for i in 0..11 {
        assert!((h[i].step.k_eff - u.keff[i]).abs() <= 1e-9, "step {}", i + 1);
        assert!(rel(h[i].step.power, u.power[i]) <= 1e-9, "step {}", i + 1);
    }
}

/// S3 controls: factors forced to 1 on `s3`; no adjustment on `s2`.
#[test]
fn s3_sp3_controls_differ_from_upstream() {
    // README prediction S3 said `|dk| > 1e-4 at step 1`: MISSED, step 1 of SP3 is the keff clamp 0.1 in both codes whatever
    // the factors are (measured 0); asserted post-run on the worst step instead.
    let u3 = r42("s3");
    let (h, _) = run_sp3_pt("v3", true, false);
    assert_eq!(h[0].step.k_eff, u3.keff[0]);
    let d1 = h.iter().zip(&u3.keff).map(|(r, k)| (r.step.k_eff - k).abs()).fold(0.0_f64, f64::max);
    println!("SP3 s3 with factors forced to 1: worst |dk| {d1:.3e}");
    assert!(d1 > 1e-4);
    let u2 = r42("s2");
    let (h, _) = run_sp3_pt("s2", false, true);
    let d = (h.last().unwrap().step.k_eff - u2.keff[u2.keff.len() - 1]).abs();
    println!("SP3 s2 without the adjustment: |dk| at the last step {d:.3e}");
    assert!(d > 1e-4);
}

// ----- stationary-fuel step -----

fn stationary_controls() -> PseudoTimeControls {
    PseudoTimeControls {
        delta_t: 1e-9,
        end_time: 3.5e-4,
        write_interval: None,
        adjust_time_step: true,
        max_delta_t: 1.0,
        min_delta_t: 1e-300,
        max_power_variation: 0.01,
        p_target: 1e6,
    }
}

fn run_stationary(nd: &str, force_unity: bool) -> (Vec<StationaryRecord>, Vec<Vec<f64>>) {
    let c = load(nd, None);
    let mut s = diffusion(&c);
    if force_unity {
        s = s.without_disc_factors();
    }
    s.set_initial_guess(&vec![vec![1.0; c.mesh.n_cells]; 2], 1.0);
    let controls = StationaryStepControls {
        ddt: NeutronicsDdtScheme::Euler,
        laplacian: NeutronicsLaplacianScheme::Corrected,
        neutron_iteration_residual: 1e-6,
        max_neutron_iterations: 50,
    };
    let (history, _) = s.run_stationary_transient(&stationary_controls(), &controls).expect("run");
    let flux = fluxes(&s);
    (history, flux)
}

fn stationary_tally(name: &str, h: &[StationaryRecord], flux: &[Vec<f64>], up: &Up) -> (f64, usize, f64, f64, f64) {
    assert_eq!(h.len(), up.power.len(), "{name}: steps");
    let (mut dp, mut dt, mut niter_eq, mut resid) = (0.0_f64, 0.0_f64, 0, 0.0_f64);
    for (i, r) in h.iter().enumerate() {
        dp = dp.max(rel(r.step.power, up.power[i]));
        dt = dt.max(rel(r.time, up.time[i]));
        if r.step.neutron_iterations as f64 == up.niter[i] {
            niter_eq += 1;
            let last = *r.step.residual_history.last().unwrap();
            let u = up.residl[i];
            resid = resid.max(if u >= 1e-6 { rel(last, u) } else { (last - u).abs() * 1e3 });
        }
    }
    let mut df = 0.0_f64;
    for g in 0..2 {
        let scale = up.flux[g].iter().cloned().fold(0.0, f64::max);
        for (a, b) in flux[g].iter().zip(&up.flux[g]) {
            df = df.max((a - b).abs() / scale);
        }
    }
    println!("{name}: steps {} | power worst {dp:.3e} | Time {dt:.2e} | iterations equal {niter_eq}/{} | last residual {resid:.2e} | final flux worst/max {df:.3e}", h.len(), h.len());
    (dp, niter_eq, dt, resid, df)
}

fn check_stationary(nd: &str, case: &str) {
    let (h, flux) = run_stationary(nd, false);
    let up = r42(case);
    let (dp, n, dt, resid, df) = stationary_tally(case, &h, &flux, &up);
    assert_eq!(n, h.len(), "{case}: neutron iteration counts");
    assert!(dt < 5e-8 && dp < 1e-12 && resid < 1e-6 && df < 1e-10, "{case}: dt {dt:e} dp {dp:e} resid {resid:e} flux {df:e}");
}

#[test]
fn n0_stationary_step_unity_factors_match_upstream() {
    check_stationary("v0", "n0");
}

#[test]
fn n3_stationary_step_given_factors_match_upstream() {
    check_stationary("v3", "n3");
}

/// N1: `adjustDiscFactors true` in the data changes nothing outside the eigenvalue branch: upstream's `n1` equals `n0`
/// bit for bit, and so does the port's own `v1` run (the stationary step never adjusts).
#[test]
fn n1_no_adjustment_outside_the_eigenvalue_branch() {
    let (u0, u1) = (r42("n0"), r42("n1"));
    assert_eq!(u0.power, u1.power);
    assert_eq!(u0.time, u1.time);
    let (h0, _) = run_stationary("v0", false);
    let (h1, _) = run_stationary("v1", false);
    assert!(h0.iter().zip(&h1).all(|(a, b)| a.step.power == b.step.power));
}

/// K2: the port with factors forced to 1 on `n3` departs from upstream.
#[test]
fn k2_stationary_control_forced_unity_differs() {
    let (h, _) = run_stationary("v3", true);
    let up = r42("n3");
    let worst = h.iter().zip(&up.power).map(|(r, p)| rel(r.step.power, *p)).fold(0.0_f64, f64::max);
    println!("K2 control (factors forced to 1 against n3): worst power {worst:.3e}");
    assert!(worst > 1e-6);
}

// ----- static eigenvalue solve -----

fn static_k(nd: &str, force_unity: bool) -> (f64, Vec<Vec<f64>>) {
    let c = load(nd, None);
    let settings = DiffusionSettings {
        linear: tight(),
        k_tolerance: 1e-13,
        flux_tolerance: 1e-11,
        max_outer_iterations: 20_000,
        ..DiffusionSettings::default()
    };
    let mut s = DiffusionNeutronics::new(c.mesh.clone(), &c.xs, &c.mesh_zone, &[], &c.flux_boundary, settings)
        .expect("diffusion");
    if force_unity {
        s = s.without_disc_factors();
    }
    let report = s.solve_eigenvalue().expect("static eigenvalue");
    (report.k_eff, fluxes(&s))
}

/// E1, E2 and K3: the port's own power iteration (tolerance 1e-13) against the converged `keff_` of upstream's pseudo-time
/// run at t = 60 (`refs/df_slab_solver`, rounds 39).
#[test]
fn e_static_eigenvalue_with_factors_matches_the_converged_upstream_keff() {
    for (nd, case) in [("v0", "v0"), ("v3", "v3")] {
        let up = reference("df_slab_solver.ref.txt.gz", case);
        let (k, flux) = static_k(nd, false);
        let dk = (k - up.keff[up.keff.len() - 1]).abs();
        let mut shape_rel = 0.0_f64;
        for g in 0..2 {
            let (sp, su) = (shape(&flux[g]), shape(&up.flux[g]));
            shape_rel = shape_rel.max(sp.iter().zip(&su).map(|(a, b)| rel(*a, *b)).fold(0.0_f64, f64::max));
        }
        println!("static {case}: k {k:.15} upstream {:.15} |dk| {dk:.3e} flux shape {shape_rel:.3e}", up.keff[up.keff.len() - 1]);
        assert!(dk <= 1e-9, "{case}: dk {dk:e}");
        assert!(shape_rel <= 1e-8, "{case}: shape {shape_rel:e}");
    }
    let up = reference("df_slab_solver.ref.txt.gz", "v3");
    let (k, _) = static_k("v3", true);
    let d = (k - up.keff[up.keff.len() - 1]).abs();
    println!("K3 control (factors forced to 1 against v3): |dk| {d:.3e}");
    assert!(d > 1e-4);
}

// ----- SN -----

/// M1: upstream's SN ignores `discFactor` (`sn1` and `sn2` keff series bit-identical in the binary); the port's SN reads no
/// discontinuity factor either (`grep disc_factor neutronics/sn` is empty), pinned here by the reference.
#[test]
fn m1_upstream_sn_ignores_the_discontinuity_factor() {
    let (a, b) = (r42("sn_1"), r42("sn_13"));
    assert_eq!(a.keff, b.keff);
    assert!(a.keff.len() >= 6);
}

// ----- refusals kept by name -----

/// X1: the paths that stay refused for a factor other than 1: the port's own single-pass transient (`step`; the stationary
/// step with an `XsEvolution` is no longer refused since round 43).
#[test]
fn x1_refusals_are_kept_by_name() {
    let c = load("v3", None);
    let mut s = diffusion(&c);
    let err = s.step(uom::si::f64::Time::new::<uom::si::time::second>(1e-3)).unwrap_err();
    assert!(err.to_string().contains("discontinuity"), "{err}");
    let mut s = diffusion(&c);
    s.set_xs_evolution(evolution(&c, false));
    s.set_initial_guess(&vec![vec![1.0; c.mesh.n_cells]; 2], 1.0);
    let controls = StationaryStepControls {
        ddt: NeutronicsDdtScheme::Euler,
        laplacian: NeutronicsLaplacianScheme::Corrected,
        neutron_iteration_residual: 1e-6,
        max_neutron_iterations: 50,
    };
    // ~~refused by name (round 42)~~ **CHANGED 2026-11-27 (round 43, gh:#1893)**: the stationary-fuel step materialises the
    // evolution at the time of the step; the test that assumed the refusal is this one (D7 compares it with the binary)
    s.upstream_stationary_step(1e-9, 1e-9, &controls).expect("the stationary step runs with an XsEvolution");
}

/// The four settings types take the entries of an `fvSolution` (`with_fv_solution`) and `Sp3Neutronics` exposes its evolution.
#[test]
fn the_settings_builders_select_the_fv_solution_form() {
    use outram_foam_appbuilder_lib::genfoam::common::linear_solvers::{NeutronSolverEntries, NeutronSolverEntry};
    use outram_foam_appbuilder_lib::genfoam::neutronics::sn::pseudo_time::SnPseudoTimeSettings;
    use outram_foam_appbuilder_lib::genfoam::neutronics::sn::SnSettings;
    let entries = NeutronSolverEntries {
        flux: NeutronSolverEntry::with_defaults(NeutronSolverKind::PcgDic),
        precursor: NeutronSolverEntry::with_defaults(NeutronSolverKind::PbicgDilu),
        angular_flux: NeutronSolverEntry::with_defaults(NeutronSolverKind::PbicgstabDilu),
    };
    let form = LinearSolverForm::UpstreamFvSolution(entries);
    assert_eq!(DiffusionSettings::default().with_fv_solution(entries).form, form);
    assert_eq!(Sp3Settings::default().with_fv_solution(entries).form, form);
    assert_eq!(SnSettings::default().with_fv_solution(entries).form, form);
    assert_eq!(SnPseudoTimeSettings::default().with_fv_solution(entries).form, form);
    let c = load("v3", None);
    let mut s = sp3(&c);
    assert!(s.xs_evolution().is_none());
    s.set_xs_evolution(evolution(&c, false));
    assert!(s.xs_evolution().is_some());
}

/// Diagnostic (ignored): the per-step `|dk|` of SP3 `s4`, to localise the first departure.
#[test]
#[ignore = "diagnostic"]
fn diag_sp3_s4_dk_series() {
    let (h, _) = run_sp3_pt("s4", false, false);
    let u = r42("s4");
    for i in 0..60.min(h.len()) {
        println!(
            "step {} port {:.17e} up {:.17e} dk {:.2e} dP {:.2e}",
            i + 1,
            h[i].step.k_eff,
            u.keff[i],
            (h[i].step.k_eff - u.keff[i]).abs(),
            rel(h[i].step.power, u.power[i])
        );
    }
}

// ----- round 43 (gh:#1893, D7): the stationary-fuel step with a control-rod driveline (`XsEvolution`) -----

/// `run_stationary` with the rod motion of `CRmove.d4` (`with_motion`) or without any evolution (the control).
fn run_stationary_driveline(nd: &str, with_motion: bool) -> (Vec<StationaryRecord>, Vec<Vec<f64>>) {
    let c = load(nd, None);
    std::fs::copy(vv_dir().join("cases/df_slab/CRmove.d4"), c.dir.join("CRmove")).expect("CRmove");
    let rods = outram_foam_appbuilder_lib::io::nuclear_data::read_control_rods(&c.dir.join("CRmove")).expect("CRmove");
    let mut s = diffusion(&c);
    if with_motion {
        let evo = XsEvolution::new(&c.xs, &c.mesh, c.mesh_zone.clone(), &c.cell_zones, Vec::new(), rods, c.controls.clone())
            .expect("evolution");
        s.set_xs_evolution(evo);
    }
    s.set_initial_guess(&vec![vec![1.0; c.mesh.n_cells]; 2], 1.0);
    let controls = StationaryStepControls {
        ddt: NeutronicsDdtScheme::Euler,
        laplacian: NeutronicsLaplacianScheme::Corrected,
        neutron_iteration_residual: 1e-6,
        max_neutron_iterations: 50,
    };
    let (history, _) = s.run_stationary_transient(&stationary_controls(), &controls).expect("run");
    let flux = fluxes(&s);
    (history, flux)
}

/// D7 (criteria fixed in the README before the run): the round-42 non-eigenvalue criteria.
fn check_driveline(nd: &str, case: &str) -> (f64, usize, usize) {
    let (h, flux) = run_stationary_driveline(nd, true);
    let up = reference("r43_stationary.ref.txt.gz", case);
    if let Some(i) = (0..h.len().min(up.power.len())).find(|&i| rel(h[i].step.power, up.power[i]) > 1e-12) {
        println!("{case}: first step with power rel > 1e-12: {} (t = {:.6e}), port {:.15e} upstream {:.15e}", i + 1, h[i].time, h[i].step.power, up.power[i]);
    }
    let (dp, n, dt, resid, df) = stationary_tally(case, &h, &flux, &up);
    assert!(dt < 5e-8 && dp < 1e-12 && resid < 1e-6 && df < 1e-10, "{case}: dt {dt:e} dp {dp:e} resid {resid:e} flux {df:e}");
    (dp, n, h.len())
}

#[test]
fn d7_stationary_step_with_driveline_motion_and_unity_factors_matches_upstream() {
    let (_, n, len) = check_driveline("v0", "n5");
    assert_eq!(n, len, "n5: neutron iteration counts");
}

#[test]
fn d7_stationary_step_with_driveline_motion_and_given_factors_matches_upstream() {
    let (_, n, len) = check_driveline("v3", "n4");
    assert_eq!(n, len, "n4: neutron iteration counts");
}

/// D7 control: the same port run without the evolution (the rod stays) misses upstream's rod run.
#[test]
fn d7_control_a_static_rod_misses_the_moving_rod_run() {
    let (h, _) = run_stationary_driveline("v3", false);
    let up = reference("r43_stationary.ref.txt.gz", "n4");
    let last = h.len().min(up.power.len()) - 1;
    let miss = rel(h[last].step.power, up.power[last]);
    println!("D7 control: static rod, last-step power rel {miss:.3e} (steps {} / {})", h.len(), up.power.len());
    assert!(miss > 1e-6 || h.len() != up.power.len(), "the rod motion must matter: {miss:e}");
}
