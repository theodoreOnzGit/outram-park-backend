// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//! # `2D_MSFR` transient stage, COUPLED, against the real binary (gh:#1361 round 9; issues 1773, 1774, 1775)
//!
//! AI-written, not human-reviewed. Research, education and V&V only.
//!
//! ## Methodology
//!
//! Upstream: GeN-Foam `652b3da0cc55` on OpenFOAM-ESI v2506, `reactorCases/2D_MSFR`, `transient` stage
//! (`scripts/run_msfr_chain.sh transient`, `PROTOCOL=tight`, `END_TIME=165.31`: 50 steps, started from the tight `steadyStateEN` output, i.e. round 11's `msfr_chain_transient` reference; every linear solver
//! 1e-14 / relTol 0, `writePrecision 17`, one write per step), `multiPhysicsSolver` = `onePhase`
//! (`solveFluidMechanics false`, frozen flux, `porousKEpsilon` fields read) + `diffusionNeutronics`.
//! The reference (`refs/msfr_chain_transient.ref.txt.gz`, `scripts/extract_msfr_fluid_reference.sh`) holds the
//! fluid `T` after every step, the time step of every step, and from the log the five `h` initial residuals,
//! the five `T (avg min max)` lines, the Courant numbers and `power:` of every step.
//!
//! The port starts from upstream's t = 165 s state (round 14: the TIGHT-solver `steadyStateEN` output, critical at `pTarget` to 1.3e-8; the shipped-tolerance state this test used before was only critical to ~2e-4) (fluid fields: `cases/msfr_fluid/restart_165`; neutronics:
//! `refs/msfr_transient.ref.txt.gz` `START_*`) and runs ITS OWN coupled loop: `correct_physics_loop`
//! (`multiPhysicsSolver::correctPhysics`), the cell-centred energy-only `OnePhaseCcSolver`, the neutronics
//! `upstream_transient_step`, the identical-mesh `cellVolumeWeight` map ([`IdenticalMeshMap`]), and `adjustDeltaT`
//! with `onePhase::maxDeltaT` and `diffusionNeutronics::maxDeltaT`. Nothing of upstream's per-step results is an
//! input: not the step size, not the power, not the temperature.
//!
//! Predictions: README, "Round 9", committed before the first run. Results: README, "Round 9 results".

// Reads case files: `std` only (gh:#1633).
#![cfg(feature = "std")]
use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use miniz_oxide::inflate::decompress_to_vec;
use outram_foam_appbuilder_lib::genfoam::multi_region::coupled_loop::{
    adjust_delta_t, correct_physics_loop, initial_delta_t, CouplingDict, IdenticalMeshMap,
};
use outram_foam_appbuilder_lib::genfoam::neutronics::albedo::AlbedoLinearisation;
use outram_foam_appbuilder_lib::genfoam::neutronics::diffusion::upstream_transient::{
    diffusion_max_delta_t, UpstreamTransientControls, UpstreamTransientFeedback,
};
use outram_foam_appbuilder_lib::genfoam::neutronics::diffusion::{
    DiffusionNeutronics, DiffusionSettings, PrecursorTransport,
};
use outram_foam_appbuilder_lib::genfoam::neutronics::xs::CrossSectionData;
use outram_foam_appbuilder_lib::genfoam::thermal_hydraulics::solver::two_phase::one_phase_cc::{
    OnePhaseCcReport, OnePhaseCcSolver,
};
use outram_foam_appbuilder_lib::io::nuclear_data::read_nuclear_data;
use outram_foam_appbuilder_lib::io::poly_mesh::{read_cell_zones, read_poly_mesh, zone_of_cell};
use outram_foam_basic_lib::fields::boundary::bc::BoundaryCondition;
use outram_foam_basic_lib::fields::field::Field;
use outram_foam_basic_lib::fields::vol_field::VolVectorField;
use outram_foam_basic_lib::fv_operators::fvc;
use outram_foam_basic_lib::ldu_matrix::SolverSettings;
use outram_foam_basic_lib::mesh::{FvMesh, PatchKind};
use outram_foam_basic_lib::primitives::Vector3;

const N_STEPS: usize = 50;

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

/// Stage the fluid case (dictionaries, gunzipped restart fields, polyMesh) and the neutronics case.
struct Staged {
    fluid_root: PathBuf,
    poly: PathBuf,
    neutro: PathBuf,
}

fn stage(tag: &str) -> Staged {
    let tmp = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("gh1361_r9_{tag}"));
    let _ = std::fs::remove_dir_all(&tmp);
    let fluid_root = tmp.join("fluid");
    copy_dir(&vv_dir().join("cases/msfr_fluid/constant"), &fluid_root.join("constant"));
    copy_dir(&vv_dir().join("cases/msfr_fluid/system"), &fluid_root.join("system"));
    let fdir = fluid_root.join("165/fluidRegion");
    std::fs::create_dir_all(&fdir).expect("mkdir");
    for e in std::fs::read_dir(vv_dir().join("cases/msfr_fluid/restart_165")).expect("restart dir") {
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
    std::fs::create_dir_all(&neutro).expect("mkdir");
    std::fs::copy(src.join("nuclearData"), neutro.join("nuclearData")).expect("nuclearData");
    Staged { fluid_root, poly, neutro }
}

fn alpha_cells() -> Vec<f64> {
    let csv = std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/genfoam_msfr_precursor_cells.csv"))
        .expect("precursor cell fixture");
    csv.lines()
        .filter(|l| !(l.starts_with('#') || l.starts_with("alpha,") || l.trim().is_empty()))
        .map(|l| l.split(',').next().unwrap().parse::<f64>().unwrap())
        .collect()
}

/// `phi = fvc::flux(U alpha)` on the neutronics mesh (wedge patches carry no flux), as in round 6.
fn face_flux(mesh: &Arc<FvMesh>, u: &[f64], alpha: &[f64]) -> PrecursorTransport {
    // gh:#1891 (round 43): upstream's `fvc::flux(U*alpha)`, wedge patches included (`wedgeFvPatchField::evaluate`).
    let uvec: Vec<Vector3> = (0..mesh.n_cells).map(|c| Vector3::new(u[3 * c], u[3 * c + 1], u[3 * c + 2])).collect();
    outram_foam_appbuilder_lib::genfoam::neutronics::diffusion::precursor_drift::upstream_precursor_transport(mesh, &uvec, alpha)
}

/// The form before round 43 (wedge faces exactly zero): the control of the fix.
#[allow(dead_code)]
fn face_flux_zero_wedge(mesh: &Arc<FvMesh>, u: &[f64], alpha: &[f64]) -> PrecursorTransport {
    let n = mesh.n_cells;
    let mut field = VolVectorField::uniform("U", mesh.clone(), Vector3::new(0.0, 0.0, 0.0));
    field.internal = Field::new(
        (0..n).map(|c| Vector3::new(u[3 * c] * alpha[c], u[3 * c + 1] * alpha[c], u[3 * c + 2] * alpha[c])).collect(),
    );
    let phi = fvc::flux(&field);
    let phi_internal: Vec<f64> = (0..mesh.n_internal_faces).map(|f| phi.internal[f]).collect();
    let phi_boundary: Vec<Vec<f64>> = mesh
        .patches
        .iter()
        .enumerate()
        .map(|(pi, p)| {
            (0..p.size).map(|fi| if p.kind == PatchKind::Wedge { 0.0 } else { phi.boundary[pi].values[fi] }).collect()
        })
        .collect();
    PrecursorTransport::new(mesh, alpha.to_vec(), phi_internal, phi_boundary, vec![0.0; n])
}

/// The coupled state the three loop closures share.
struct Coupled {
    fluid: OnePhaseCcSolver,
    neutro: DiffusionNeutronics,
    xs: CrossSectionData,
    zone_map: Vec<usize>,
    map: IdenticalMeshMap,
    controls: UpstreamTransientControls,
    /// `powerDensity` of the neutronics region (what the fluid's `powerDensityNeutronicsToLiquid` is mapped from).
    neutro_power_density: Vec<f64>,
    /// the neutronics region's `TFuel`/`rhoCool` (mapped from the fluid).
    neutro_t: Vec<f64>,
    neutro_rho: Vec<f64>,
    dt: f64,
    fluid_residual: f64,
    neutro_residual: f64,
    p_tot: f64,
    p_tot_old: f64,
    neutro_iterations: usize,
    fluid_report: Option<OnePhaseCcReport>,
    first_step: bool,
    p_old_reference: f64,
    /// per coupling iteration of the current step: `(power, neutron iterations, residual_, second-solve term)` (gh:#1813)
    iter_records: Vec<(f64, usize, f64, f64)>,
    /// per coupling iteration of the current step: the fluid's `getResidual()`
    fluid_iter_residuals: Vec<f64>,
}

fn rel(a: f64, b: f64) -> f64 {
    ((a - b) / b).abs()
}

#[test]
fn msfr_coupled_transient_matches_upstream_over_50_steps() {
    let st = stage("coupled");
    let reference = read_ref(&vv_dir().join("refs/msfr_transient.ref.txt.gz"));
    let fref = read_ref(&vv_dir().join("refs/msfr_chain_transient.ref.txt.gz"));

    // ---- the case-driven multiPhysicsSolver
    let cdict = CouplingDict::read(&st.fluid_root, "energyNeutroLoop").expect("multiRegionCouplingDict");
    assert_eq!(cdict.solvers, vec![("fluidRegion".into(), "onePhase".into()), ("neutroRegion".into(), "diffusionNeutronics".into())]);
    assert_eq!((cdict.min_residual, cdict.max_iterations), (5e-5, 3));
    let only_energy = cdict.correct_only_energy();

    // ---- neutronics (as round 6)
    let mesh: Arc<FvMesh> = read_poly_mesh(&st.poly).expect("polyMesh");
    let input = read_nuclear_data(&st.neutro.join("nuclearData")).expect("nuclearData");
    let xs = CrossSectionData::from_input(&input).expect("cross sections");
    let zones = read_cell_zones(&st.poly).expect("cellZones");
    let order: Vec<&str> = (0..xs.zone_count()).map(|i| xs.zone(i).expect("zone").name()).collect();
    let zone_map = zone_of_cell(&zones, &order, mesh.n_cells).expect("zone map");
    let n = mesh.n_cells;
    let groups = xs.energy_groups();
    let precs = xs.prec_groups();
    let flux_boundary: Vec<BoundaryCondition<f64>> = mesh.patches.iter().map(|_| BoundaryCondition::ZeroGradient).collect();
    let settings = DiffusionSettings {
        linear: SolverSettings { tolerance: 1e-14, max_iter: 100_000 },
        ..DiffusionSettings::default()
    };

    // ---- fluid
    let fluid = OnePhaseCcSolver::from_case_dir(&st.fluid_root, &st.poly).expect("fluid case");
    let t_start: Vec<f64> = fluid.temperature().to_vec();
    let t0_ref = &reference["T_0"];
    let dstart = t_start.iter().zip(t0_ref).map(|(a, b)| rel(*a, *b)).fold(0.0, f64::max);
    println!("start state: fluid T (restart) against the reference T_0: worst rel {dstart:.3e}");
    let rho_start = fluid.thermo_density();
    let params0: Vec<f64> = t_start.iter().zip(&rho_start).flat_map(|(&t, &r)| [t, r]).collect();
    let mut neutro = DiffusionNeutronics::new_with_cell_parameters(mesh.clone(), &xs, &zone_map, &params0, &flux_boundary, settings)
        .expect("diffusion model");
    for (name, gamma) in [("topwall", 0.1), ("bottomwall", 0.1), ("reflector", 0.1), ("hx", 0.5)] {
        let idx = mesh.patches.iter().position(|p| p.name == name).expect("patch");
        neutro.set_albedo_boundary(idx, gamma, AlbedoLinearisation::CellValue);
    }
    neutro.set_precursor_drift(face_flux(&mesh, &reference["START_U"], &alpha_cells()));
    let flux0: Vec<Vec<f64>> = (0..groups).map(|g| reference[&format!("START_flux{g}")].clone()).collect();
    neutro.set_initial_guess(&flux0, reference["START_keff"][0]);
    let prec_star0: Vec<Vec<f64>> = (0..precs).map(|k| reference[&format!("START_precStar{k}")].clone()).collect();
    neutro.set_prec_star(&prec_star0).expect("precStar");

    // the identical-mesh cellVolumeWeight map (fluid mesh and neutronics mesh are the same mesh)
    let fluid_mesh = fluid.case().mesh.clone();
    let map = IdenticalMeshMap::new(&fluid_mesh, &mesh).expect("identical meshes");

    // upstream's powerDensity at t = 165 s is what step 1 maps (the neutronics region reads it from its restart)
    let pd_start = fref["START_powerDensity"].clone();
    let pd_port = neutro.upstream_power_density();
    println!(
        "start state: neutronics powerDensity from the port's flux and cross sections against upstream's restart field: worst rel {:.3e}",
        pd_port.iter().zip(&pd_start).filter(|(_, b)| **b != 0.0).map(|(a, b)| rel(*a, *b)).fold(0.0, f64::max)
    );

    let control_delta_t = fluid.case().delta_t;
    let max_delta_t = fluid.case().controls.max_delta_t;
    let min_residual = cdict.min_residual;
    let max_iterations = cdict.max_iterations;
    let mut dt = initial_delta_t(control_delta_t, control_delta_t);
    let p0 = 0.0;
    let p_old_reference = neutro.upstream_power_at_reference_state(&xs, &zone_map).expect("reference-state power");
    println!("pTotOld_ of step 1 (flux of t = 165 s, sigmaPow of the reference state): {p_old_reference:.10e}");
    let coupled = RefCell::new(Coupled {
        fluid,
        neutro,
        xs,
        zone_map,
        map,
        controls: UpstreamTransientControls { neutron_iteration_residual: 1e-6, max_neutron_iterations: 50, sc_no: 1.0 },
        neutro_power_density: pd_start,
        neutro_t: t_start.clone(),
        neutro_rho: rho_start,
        dt,
        fluid_residual: 0.0,
        neutro_residual: 0.0,
        p_tot: p0,
        p_tot_old: p0,
        neutro_iterations: 0,
        fluid_report: None,
        first_step: true,
        p_old_reference,
        iter_records: Vec::new(),
        fluid_iter_residuals: Vec::new(),
    });
    let mu = vec![0.01; n];
    let alphat = vec![0.01 * (1.0 / 8.0); n];
    let started = std::time::Instant::now();

    println!("step  dt_port           dt_ref            rel_dt     maxrel_T      power_port       rel_P      rel_h1res  iters");
    let mut first_dt_bad = None;
    let mut first_t_bad = None;
    let mut worst_t = 0.0_f64;
    let mut worst_p = 0.0_f64;
    let mut worst_dt = 0.0_f64;
    let mut worst_hres = 0.0_f64;
    let mut worst_tstat = 0.0_f64;
    let mut worst_co = 0.0_f64;
    let mut converged_in_one = 0usize;
    let mut worst_h1 = 0.0_f64;
    let mut worst_re = 0.0_f64;
    let mut iters_equal = 0usize;
    let mut per_step_t: Vec<f64> = Vec::new();
    for step in 1..=N_STEPS {
        {
            let mut c = coupled.borrow_mut();
            c.dt = dt;
            c.fluid.begin_time_step(dt);
        }
        // mapAllFields(runTime) at the top of the step, then correctPhysics (which maps again at every iteration)
        let outcome = correct_physics_loop(
            min_residual,
            max_iterations,
            &only_energy,
            || {
                let mut c = coupled.borrow_mut();
                // fluidRegion <- neutroRegion: powerDensity -> powerDensityNeutronicsToLiquid
                let mut pd = c.fluid.case().power_density.clone();
                let src = c.neutro_power_density.clone();
                c.map.map(&src, &mut pd);
                c.fluid.set_power_density(&pd);
                // neutroRegion <- fluidRegion: T -> TFuel (TCool), thermo:rho -> rhoCool
                let t = c.fluid.temperature().to_vec();
                let rho = c.fluid.thermo_density();
                let mut nt = c.neutro_t.clone();
                let mut nr = c.neutro_rho.clone();
                c.map.map(&t, &mut nt);
                c.map.map(&rho, &mut nr);
                c.neutro_t = nt;
                c.neutro_rho = nr;
            },
            |i, tight| {
                let mut guard = coupled.borrow_mut();
                let c = &mut *guard;
                if i == 0 {
                    if tight {
                        c.fluid_residual = c.fluid.correct_tightly_coupled_physics();
                    } else {
                        let (r, rep) = c.fluid.correct_physics();
                        c.fluid_residual = r;
                        c.fluid_report = Some(rep);
                    }
                } else {
                    let params: Vec<f64> = c.neutro_t.iter().zip(&c.neutro_rho).flat_map(|(&t, &r)| [t, r]).collect();
                    let rep = c
                        .neutro
                        .upstream_transient_step(
                            &c.xs,
                            &c.zone_map,
                            UpstreamTransientFeedback { cell_parameters: &params, rho_cool: &c.neutro_rho, alphat: &alphat, mu: &mu },
                            c.dt,
                            &c.controls,
                        )
                        .expect("neutronics step");
                    c.p_tot = rep.power;
                    // pTotOld_ = power() at the start of correctPhysics: at the first step of a restart the
                    // XS object is still at its reference state (xs_.correct() runs inside solveNeutronics.H)
                    c.p_tot_old = if c.first_step { c.p_old_reference } else { rep.power_old };
                    c.first_step = false;
                    c.neutro_iterations = rep.neutron_iterations;
                    // residual_ = max(initial residuals of the flux solves, neutroResidual); the port exposes the
                    // first iteration's group residuals and the neutroResidual history (~~a lower bound~~ **CORRECTED 2026-11-26 (gh:#1813): an upper bound when the neutron loop iterates more than once; equal at one iteration**; far from
                    // the 5e-5 threshold: <= 2e-6 at every step of the reference)
                    c.neutro_residual = rep
                        .first_iteration_group_residuals
                        .iter()
                        .chain(rep.residual_history.iter())
                        .copied()
                        .fold(0.0, f64::max);
                    c.neutro_power_density = c.neutro.upstream_power_density();
                }
            },
            |i| {
                let c = coupled.borrow();
                if i == 0 { c.fluid_residual } else { c.neutro_residual }
            },
        );
        if outcome.iterations == 1 && outcome.converged {
            converged_in_one += 1;
        }
        // adjustDeltaT
        let (t_port, rep, power, iters, p_tot, p_tot_old, fluid_max_dt, time) = {
            let c = coupled.borrow();
            (
                c.fluid.temperature().to_vec(),
                c.fluid_report.clone().expect("fluid report"),
                c.p_tot,
                c.neutro_iterations,
                c.p_tot,
                c.p_tot_old,
                c.fluid.max_delta_t(),
                c.fluid.time(),
            )
        };
        let neutro_max = diffusion_max_delta_t(time, control_delta_t, max_delta_t, 0.025, dt, p_tot, p_tot_old);
        let dt_before = dt;
        if let Some(new_dt) = adjust_delta_t(true, &[fluid_max_dt, neutro_max], f64::MIN_POSITIVE, dt) {
            dt = new_dt;
        }
        // ---- compare step `step`
        let t_ref = &fref[&format!("T_{step}")];
        let dt_ref_next = fref["DT"][step - 1];
        let dtr = rel(dt, dt_ref_next);
        let dtt = t_port.iter().zip(t_ref).map(|(a, b)| rel(*a, *b)).fold(0.0, f64::max);
        let p_ref = fref[&format!("POWER_{step}")][0];
        let dpr = rel(power, p_ref);
        let hres_ref = &fref[&format!("HRES_{step}")];
        let dh1 = rel(rep.h_residuals[0], hres_ref[0]);
        let dh = rep.h_residuals.iter().zip(hres_ref).map(|(a, b)| rel(*a, *b)).fold(0.0, f64::max);
        let tstat_ref = &fref[&format!("TSTAT_{step}")];
        let mut dts = 0.0_f64;
        for (k, (a, mn, mx)) in rep.t_stats.iter().enumerate() {
            dts = dts.max(rel(*a, tstat_ref[3 * k])).max(rel(*mn, tstat_ref[3 * k + 1])).max(rel(*mx, tstat_ref[3 * k + 2]));
        }
        let co_ref = &fref[&format!("CO_{step}")];
        let co_port = coupled.borrow().fluid.courant();
        let dco = rel(co_port.0, co_ref[1]).max(rel(co_port.1, co_ref[0]));
        println!(
            "{step:4}  {dt_before:.10e}->{dt:.10e}  {dt_ref_next:.10e}  {dtr:.2e}  {dtt:.3e}  {power:.10e}  {dpr:.2e}  {dh1:.2e}  {iters:3}  (hres {dh:.1e}, Tstat {dts:.1e}, Co {dco:.1e})"
        );
        if dtr > 1e-9 && first_dt_bad.is_none() {
            first_dt_bad = Some(step);
        }
        if dtt > 1e-9 && first_t_bad.is_none() {
            first_t_bad = Some(step);
        }
        worst_h1 = worst_h1.max(dh1);
        if iters == reference[&format!("STEP_{step}")][3] as usize {
            iters_equal += 1;
        }
        worst_t = worst_t.max(dtt);
        worst_p = worst_p.max(dpr);
        worst_dt = worst_dt.max(dtr);
        worst_hres = worst_hres.max(dh);
        worst_tstat = worst_tstat.max(dts);
        worst_co = worst_co.max(dco);
        per_step_t.push(dtt);
        if step == 1 || step == 10 || step == 25 || step == 50 {
            let re_ref = &fref[&format!("RE_{step}")];
            let re_port = coupled.borrow().fluid.reynolds().to_vec();
            let dre = re_port.iter().zip(re_ref).map(|(a, b)| rel(*a, *b)).fold(0.0, f64::max);
            println!("      Re (restart-style field after step {step}): worst rel {dre:.3e}");
            worst_re = worst_re.max(dre);
        }
    }
    println!(
        "SUMMARY worst rel: dt {worst_dt:.3e}, T {worst_t:.3e}, power {worst_p:.3e}, h residuals {worst_hres:.3e}, T(avg,min,max) {worst_tstat:.3e}, Courant {worst_co:.3e}; first dt > 1e-9 at {first_dt_bad:?}, first T > 1e-9 at {first_t_bad:?}; coupling converged after 1 iteration at {converged_in_one}/{N_STEPS} steps; wall {:.1} s",
        started.elapsed().as_secs_f64()
    );
    assert_eq!(converged_in_one, N_STEPS, "upstream converges after 1 iteration at every one of the first 50 steps");
    // Gates at the measured values (2026-11-26) with a margin of at least 20x; the README records the numbers.
    assert!(first_dt_bad.is_none() && worst_dt < 1e-11, "step sizes: worst {worst_dt:e}");
    assert!(first_t_bad.is_none() && worst_t < 1e-12, "T: worst {worst_t:e}");
    assert!(worst_p < 1e-12, "power: worst {worst_p:e}");
    assert!(worst_tstat < 1e-13, "T (avg min max): worst {worst_tstat:e}");
    assert!(worst_co < 1e-11, "Courant: worst {worst_co:e}");
    assert!(worst_h1 < 1e-8, "first h residual of each step: worst {worst_h1:e}");
    assert!(worst_re < 1e-12, "Re: worst {worst_re:e}");
    assert_eq!(iters_equal, N_STEPS, "neutron iteration counts equal at every step");
    // the mapping dictionary, in the order mapTheseFields visits it
    let regions = vec!["fluidRegion".to_string(), "neutroRegion".to_string()];
    let order = cdict.mapping_order(&regions);
    assert_eq!(order.len(), 2);
    assert_eq!((order[0].target_region.as_str(), order[0].source_region.as_str()), ("fluidRegion", "neutroRegion"));
    assert_eq!(order[0].source_fields, vec!["secondaryPowerDensity", "powerDensity"]);
    assert_eq!(order[0].target_fields, vec!["powerDensityNeutronics", "powerDensityNeutronicsToLiquid"]);
    assert_eq!((order[1].target_region.as_str(), order[1].source_region.as_str()), ("neutroRegion", "fluidRegion"));
    assert_eq!(order[1].source_fields.len(), 9);
}

/// `correctTightlyCoupledPhysics` is `correctModels` + one `correctEnergy`: it equals the first outer iteration
/// of `correctPhysics` bit for bit. (The 50 steps above never take this branch: the loop converges in 1 iteration.)
#[test]
fn tightly_coupled_physics_is_one_outer_iteration() {
    let st = stage("tight");
    let a = OnePhaseCcSolver::from_case_dir(&st.fluid_root, &st.poly).expect("fluid case");
    let mut case = a.case().clone();
    case.controls.pimple.n_outer = 1;
    let mut one = OnePhaseCcSolver::new(case);
    let mut tight = a;
    one.begin_time_step(1e-3);
    tight.begin_time_step(1e-3);
    let (r1, rep) = one.correct_physics();
    let r2 = tight.correct_tightly_coupled_physics();
    assert_eq!(r1, r2);
    assert_eq!(rep.h_residuals.len(), 1);
    assert_eq!(one.temperature(), tight.temperature());
    assert_eq!(one.enthalpy(), tight.enthalpy());
    assert_eq!(one.htc().iter().cloned().fold(0.0, f64::max), 1e4);
    assert!(tight.max_delta_t() > 0.0);
}

/// The refusals name what is not ported (extend the port, never the refusal).
#[test]
fn a_case_that_solves_fluid_mechanics_is_refused_by_name() {
    let st = stage("refuse");
    let p = st.fluid_root.join("system/fluidRegion/fvSolution");
    let text = std::fs::read_to_string(&p).expect("fvSolution");
    std::fs::write(&p, text.replace("solveFluidMechanics false", "solveFluidMechanics true")).expect("write");
    let err = OnePhaseCcSolver::from_case_dir(&st.fluid_root, &st.poly).expect_err("must refuse").to_string();
    // ~~round 19 (gh:#1837) ported the turbulence-off flow: this case (turbulence true) is refused for THAT~~
    // **CORRECTED 2026-11-26 (round 20, gh:#1838/#1839):** `turbulence true` is ported, so the reader now gets as far as the
    // shipped `p_rgh` solver (`GAMG`, not ported: the references of rounds 18-20 replace it by `PCG`/`DIC`)
    assert!(err.contains("solvers.p_rgh: only PCG with the DIC preconditioner is ported"), "{err}");
}


// ============================================================================================================
// Round 22 (gh:#1813): the full transient (to the tutorial's endTime 175 s), a multi-iteration coupling loop,
// the neutronics `residual_` value and a `temperatureTimeProfile`, each against the real binary.
// Predictions and results: README, "Round 22". References: `refs/msfr_full_*.ref.txt.gz`
// (`scripts/extract_msfr_full_reference.sh`; upstream run by `scripts/run_msfr_chain.sh transient`, `PROTOCOL=tight`).
// ============================================================================================================

/// One round 22 case: which reference, which edits of upstream's own case files (the SAME edits the reference run made).
struct FullCfg {
    tag: &'static str,
    ref_name: &'static str,
    /// `multiRegionCouplingDict` `minResidual` / `maxIterations` text replacements (`None`: the tutorial's)
    min_residual: Option<&'static str>,
    max_iterations: Option<&'static str>,
    /// `temperatureTimeProfile` on the `hx` `fixedTemperature` model: `(0 1)(165 1)(175 0.95)`
    profile: bool,
    /// the earlier harness' stand-in for the neutronics `residual_` (deprecated, ablation)
    lower_bound_residual: bool,
    /// stop after this many steps (`None`: every step of the reference)
    max_steps: Option<usize>,
}

#[derive(Debug, Default)]
struct FullSummary {
    n_steps: usize,
    worst_dt: f64,
    worst_t_field: f64,
    worst_tstat: f64,
    worst_power: f64,
    worst_co: f64,
    worst_h_first_iteration: f64,
    worst_h_later: f64,
    worst_resid: f64,
    iters_equal: usize,
    cpiter_equal: usize,
    neutron_iters_equal: usize,
    first_dt_bad: Option<usize>,
    port_cpiter: Vec<usize>,
    port_power_last: f64,
    worst_second_solve_port: f64,
    worst_second_solve_upstream: f64,
    resid_mismatch_steps: Vec<usize>,
}

fn edit_staged_case(st: &Staged, cfg: &FullCfg) {
    let dict = st.fluid_root.join("constant/multiRegionCouplingDict");
    let mut text = std::fs::read_to_string(&dict).expect("multiRegionCouplingDict");
    if let Some(m) = cfg.min_residual {
        assert!(text.contains("minResidual 0.00005;"), "minResidual line");
        text = text.replace("minResidual 0.00005;", &format!("minResidual {m};"));
    }
    if let Some(m) = cfg.max_iterations {
        assert!(text.contains("maxIterations 3;"), "maxIterations line");
        text = text.replace("maxIterations 3;", &format!("maxIterations {m};"));
    }
    std::fs::write(&dict, text).expect("write dict");
    if cfg.profile {
        let pp = st.fluid_root.join("constant/fluidRegion/phaseProperties");
        let text = std::fs::read_to_string(&pp).expect("phaseProperties");
        let anchor = "            volumetricArea  200;\n            T               900;\n";
        assert!(text.contains(anchor), "hx powerModel text");
        let add = "            temperatureTimeProfile\n            {\n                type            table;\n                startTime       0;\n                table           table\n                (\n                    (0 1)\n                    (165 1)\n                    (175 0.95)\n                );\n            }\n";
        std::fs::write(&pp, text.replace(anchor, &format!("{anchor}{add}"))).expect("write phaseProperties");
    }
}

/// The coupled transient of rounds 9/22 against a round 22 reference; returns the worst differences.
fn run_full(cfg: &FullCfg) -> FullSummary {
    let st = stage(cfg.tag);
    edit_staged_case(&st, cfg);
    let reference = read_ref(&vv_dir().join("refs/msfr_transient.ref.txt.gz"));
    let fref = read_ref(&vv_dir().join("refs/msfr_chain_transient.ref.txt.gz"));
    let full = read_ref(&vv_dir().join(format!("refs/{}.ref.txt.gz", cfg.ref_name)));
    let n_steps = cfg.max_steps.unwrap_or(full["N"][0] as usize).min(full["N"][0] as usize);

    let cdict = CouplingDict::read(&st.fluid_root, "energyNeutroLoop").expect("multiRegionCouplingDict");
    let only_energy = cdict.correct_only_energy();
    let mesh: Arc<FvMesh> = read_poly_mesh(&st.poly).expect("polyMesh");
    let input = read_nuclear_data(&st.neutro.join("nuclearData")).expect("nuclearData");
    let xs = CrossSectionData::from_input(&input).expect("cross sections");
    let zones = read_cell_zones(&st.poly).expect("cellZones");
    let order: Vec<&str> = (0..xs.zone_count()).map(|i| xs.zone(i).expect("zone").name()).collect();
    let zone_map = zone_of_cell(&zones, &order, mesh.n_cells).expect("zone map");
    let n = mesh.n_cells;
    let groups = xs.energy_groups();
    let precs = xs.prec_groups();
    let flux_boundary: Vec<BoundaryCondition<f64>> = mesh.patches.iter().map(|_| BoundaryCondition::ZeroGradient).collect();
    let settings = DiffusionSettings {
        linear: SolverSettings { tolerance: 1e-14, max_iter: 100_000 },
        ..DiffusionSettings::default()
    };
    let fluid = OnePhaseCcSolver::from_case_dir(&st.fluid_root, &st.poly).expect("fluid case");
    let t_start: Vec<f64> = fluid.temperature().to_vec();
    let rho_start = fluid.thermo_density();
    let params0: Vec<f64> = t_start.iter().zip(&rho_start).flat_map(|(&t, &r)| [t, r]).collect();
    let mut neutro = DiffusionNeutronics::new_with_cell_parameters(mesh.clone(), &xs, &zone_map, &params0, &flux_boundary, settings)
        .expect("diffusion model");
    for (name, gamma) in [("topwall", 0.1), ("bottomwall", 0.1), ("reflector", 0.1), ("hx", 0.5)] {
        let idx = mesh.patches.iter().position(|p| p.name == name).expect("patch");
        neutro.set_albedo_boundary(idx, gamma, AlbedoLinearisation::CellValue);
    }
    neutro.set_precursor_drift(face_flux(&mesh, &reference["START_U"], &alpha_cells()));
    let flux0: Vec<Vec<f64>> = (0..groups).map(|g| reference[&format!("START_flux{g}")].clone()).collect();
    neutro.set_initial_guess(&flux0, reference["START_keff"][0]);
    let prec_star0: Vec<Vec<f64>> = (0..precs).map(|k| reference[&format!("START_precStar{k}")].clone()).collect();
    neutro.set_prec_star(&prec_star0).expect("precStar");
    let fluid_mesh = fluid.case().mesh.clone();
    let map = IdenticalMeshMap::new(&fluid_mesh, &mesh).expect("identical meshes");
    let pd_start = fref["START_powerDensity"].clone();
    let control_delta_t = fluid.case().delta_t;
    let max_delta_t = fluid.case().controls.max_delta_t;
    let (min_residual, max_iterations) = (cdict.min_residual, cdict.max_iterations);
    let mut dt = initial_delta_t(control_delta_t, control_delta_t);
    let p_old_reference = neutro.upstream_power_at_reference_state(&xs, &zone_map).expect("reference-state power");
    let coupled = RefCell::new(Coupled {
        fluid,
        neutro,
        xs,
        zone_map,
        map,
        controls: UpstreamTransientControls { neutron_iteration_residual: 1e-6, max_neutron_iterations: 50, sc_no: 1.0 },
        neutro_power_density: pd_start,
        neutro_t: t_start.clone(),
        neutro_rho: rho_start,
        dt,
        fluid_residual: 0.0,
        neutro_residual: 0.0,
        p_tot: 0.0,
        p_tot_old: 0.0,
        neutro_iterations: 0,
        fluid_report: None,
        first_step: true,
        p_old_reference,
        iter_records: Vec::new(),
        fluid_iter_residuals: Vec::new(),
    });
    let mu = vec![0.01; n];
    let alphat = vec![0.01 * (1.0 / 8.0); n];
    let lower_bound = cfg.lower_bound_residual;
    let started = std::time::Instant::now();
    let mut sum = FullSummary { n_steps, ..FullSummary::default() };
    for step in 1..=n_steps {
        {
            let mut c = coupled.borrow_mut();
            c.dt = dt;
            c.fluid.begin_time_step(dt);
            c.neutro.store_old_times();
            c.iter_records.clear();
            c.fluid_iter_residuals.clear();
        }
        let outcome = correct_physics_loop(
            min_residual,
            max_iterations,
            &only_energy,
            || {
                let mut c = coupled.borrow_mut();
                let mut pd = c.fluid.case().power_density.clone();
                let src = c.neutro_power_density.clone();
                c.map.map(&src, &mut pd);
                c.fluid.set_power_density(&pd);
                let t = c.fluid.temperature().to_vec();
                let rho = c.fluid.thermo_density();
                let mut nt = c.neutro_t.clone();
                let mut nr = c.neutro_rho.clone();
                c.map.map(&t, &mut nt);
                c.map.map(&rho, &mut nr);
                c.neutro_t = nt;
                c.neutro_rho = nr;
            },
            |i, tight| {
                let mut guard = coupled.borrow_mut();
                let c = &mut *guard;
                if i == 0 {
                    if tight {
                        c.fluid_residual = c.fluid.correct_tightly_coupled_physics();
                    } else {
                        let (r, rep) = c.fluid.correct_physics();
                        c.fluid_residual = r;
                        c.fluid_report = Some(rep);
                    }
                    let r = c.fluid_residual;
                    c.fluid_iter_residuals.push(r);
                } else {
                    let params: Vec<f64> = c.neutro_t.iter().zip(&c.neutro_rho).flat_map(|(&t, &r)| [t, r]).collect();
                    let rep = c
                        .neutro
                        .upstream_transient_step(
                            &c.xs,
                            &c.zone_map,
                            UpstreamTransientFeedback { cell_parameters: &params, rho_cool: &c.neutro_rho, alphat: &alphat, mu: &mu },
                            c.dt,
                            &c.controls,
                        )
                        .expect("neutronics step");
                    c.p_tot = rep.power;
                    c.p_tot_old = if c.first_step { c.p_old_reference } else { rep.power_old };
                    c.first_step = false;
                    c.neutro_iterations = rep.neutron_iterations;
                    c.neutro_residual = if lower_bound { rep.residual_lower_bound_form } else { rep.residual };
                    c.iter_records.push((rep.power, rep.neutron_iterations, rep.residual, rep.second_solve_residual));
                    c.neutro_power_density = c.neutro.upstream_power_density();
                }
            },
            |i| {
                let c = coupled.borrow();
                if i == 0 { c.fluid_residual } else { c.neutro_residual }
            },
        );
        sum.port_cpiter.push(outcome.iterations);
        let (t_port, rep, p_tot, p_tot_old, fluid_max_dt, time, records, fluid_res, co_port) = {
            let c = coupled.borrow();
            (
                c.fluid.temperature().to_vec(),
                c.fluid_report.clone().expect("fluid report"),
                c.p_tot,
                c.p_tot_old,
                c.fluid.max_delta_t(),
                c.fluid.time(),
                c.iter_records.clone(),
                c.fluid_iter_residuals.clone(),
                c.fluid.courant(),
            )
        };
        let neutro_max = diffusion_max_delta_t(time, control_delta_t, max_delta_t, 0.025, dt, p_tot, p_tot_old);
        if let Some(new_dt) = adjust_delta_t(true, &[fluid_max_dt, neutro_max], f64::MIN_POSITIVE, dt) {
            dt = new_dt;
        }
        // ---- compare step `step`
        let dtr = rel(dt, full["DT"][step - 1]);
        sum.worst_dt = sum.worst_dt.max(dtr);
        if dtr > 1e-9 && sum.first_dt_bad.is_none() {
            sum.first_dt_bad = Some(step);
        }
        if outcome.iterations == full[&format!("CPITER_{step}")][0] as usize {
            sum.cpiter_equal += 1;
        }
        let p_ref = &full[&format!("POWER_{step}")];
        let n_ref = &full[&format!("NITER_{step}")];
        let r_ref = &full[&format!("RESID_{step}")];
        let s2_ref = &full[&format!("S2MAX_{step}")];
        let mut all_iters_equal = records.len() == n_ref.len();
        for (j, (pw, ni, res, s2)) in records.iter().enumerate() {
            if j < p_ref.len() {
                sum.worst_power = sum.worst_power.max(rel(*pw, p_ref[j]));
                let rr = rel(*res, r_ref[j]);
                sum.worst_resid = sum.worst_resid.max(rr);
                if rr > 1e-4 {
                    sum.resid_mismatch_steps.push(step);
                }
                if *ni != n_ref[j] as usize {
                    all_iters_equal = false;
                }
                sum.worst_second_solve_port = sum.worst_second_solve_port.max(*s2);
                sum.worst_second_solve_upstream = sum.worst_second_solve_upstream.max(s2_ref[j]);
            }
        }
        if all_iters_equal {
            sum.neutron_iters_equal += 1;
        }
        let hres_ref = &full[&format!("HRES_{step}")];
        for (a, b) in rep.h_residuals.iter().zip(hres_ref.iter()) {
            sum.worst_h_first_iteration = sum.worst_h_first_iteration.max(rel(*a, *b));
        }
        for (j, r) in fluid_res.iter().enumerate().skip(1) {
            // iteration j (>= 1) of the step solves ONE `h` equation: its initial residual follows the five of iteration 1
            let idx = rep.h_residuals.len() + (j - 1);
            if idx < hres_ref.len() {
                sum.worst_h_later = sum.worst_h_later.max(rel(*r, hres_ref[idx]));
            }
        }
        let tstat_ref = &full[&format!("TSTAT_{step}")];
        for (k, (a, mn, mx)) in rep.t_stats.iter().enumerate() {
            sum.worst_tstat = sum
                .worst_tstat
                .max(rel(*a, tstat_ref[3 * k]))
                .max(rel(*mn, tstat_ref[3 * k + 1]))
                .max(rel(*mx, tstat_ref[3 * k + 2]));
        }
        let co_ref = &full[&format!("CO_{step}")];
        sum.worst_co = sum.worst_co.max(rel(co_port.0, co_ref[1])).max(rel(co_port.1, co_ref[0]));
        if full.contains_key(&format!("T_{step}")) {
            let t_ref = &full[&format!("T_{step}")];
            let d = t_port.iter().zip(t_ref).map(|(a, b)| rel(*a, *b)).fold(0.0, f64::max);
            sum.worst_t_field = sum.worst_t_field.max(d);
            println!("step {step}: T field worst rel {d:.3e}");
        }
        sum.iters_equal += 1;
        sum.port_power_last = p_tot;
        println!(
            "step {step:3} coupling iterations {} (upstream {}) dt rel {dtr:.2e} power {p_tot:.10e} (ref last {:.10e})",
            outcome.iterations,
            full[&format!("CPITER_{step}")][0],
            p_ref[p_ref.len() - 1]
        );
    }
    println!(
        "SUMMARY {}: steps {n_steps}; worst rel dt {:.3e}, T field {:.3e}, T(avg,min,max) {:.3e}, power {:.3e}, Courant {:.3e}, h(iter 1) {:.3e}, h(later iters) {:.3e}, residual_ {:.3e}; coupling iterations equal {}/{n_steps}, neutron iteration counts equal {}/{n_steps}; second-solve term port max {:.3e} upstream max {:.3e}; steps with residual_ off by >1e-4: {:?}; wall {:.1} s",
        cfg.tag,
        sum.worst_dt,
        sum.worst_t_field,
        sum.worst_tstat,
        sum.worst_power,
        sum.worst_co,
        sum.worst_h_first_iteration,
        sum.worst_h_later,
        sum.worst_resid,
        sum.cpiter_equal,
        sum.neutron_iters_equal,
        sum.worst_second_solve_port,
        sum.worst_second_solve_upstream,
        sum.resid_mismatch_steps,
        started.elapsed().as_secs_f64()
    );
    sum
}


/// Part 1 of gh:#1813: the whole transient of the tutorial (`endTime 175`: 1347 steps, not the 142 of the issue; see README
/// "Round 22") with the tutorial's coupling dictionary. Measured values: README, "Round 22 results".
#[test]
fn msfr_full_transient_to_175_s_matches_upstream() {
    let s = run_full(&FullCfg {
        tag: "fullA",
        ref_name: "msfr_full_a",
        min_residual: None,
        max_iterations: None,
        profile: false,
        lower_bound_residual: false,
        max_steps: None,
    });
    assert_eq!(s.cpiter_equal, s.n_steps, "coupling iterations equal at every step");
    assert!(s.port_cpiter.iter().all(|&c| c == 1));
    assert_eq!(s.neutron_iters_equal, s.n_steps, "neutron iteration counts equal at every step");
    // measured 2026-11-26 (README, "Round 22 results"), gates ~2.5x outward
    assert!(s.first_dt_bad.is_none() && s.worst_dt < 3e-13, "dt {:e}", s.worst_dt);
    assert!(s.worst_t_field < 4e-12, "T field {:e}", s.worst_t_field);
    assert!(s.worst_tstat < 4e-12, "T stats {:e}", s.worst_tstat);
    assert!(s.worst_power < 3e-11, "power {:e}", s.worst_power);
    assert!(s.worst_co < 3e-13, "Courant {:e}", s.worst_co);
    assert!(s.worst_resid < 4e-5, "residual_ {:e}", s.worst_resid);
    assert!(s.worst_second_solve_port < 1e-13 && s.worst_second_solve_upstream < 1e-13, "second-solve term is at the solver tolerance");
}

/// Part 2 of gh:#1813: the coupling loop runs more than once. `minResidual 1e-7` (inside the range of upstream's own first `h`
/// residuals, 2.6e-8..1.8e-7) and `maxIterations 6` (the value of upstream's `3D_SmallESFR_old` `..._tightly` dictionary):
/// upstream stops at 1 iteration for steps 1-10, runs all 6 from step 11 to 263 and is back at 1 from step 264 (the
/// running residual crosses the threshold in both directions, so the residual VALUE decides every step). Exercises
/// `correctTightlyCoupledPhysics` (iterations 2-6 of the fluid) and the residual rule. Results: README, "Round 22 results".
#[test]
fn msfr_multi_iteration_coupling_matches_upstream() {
    let full = read_ref(&vv_dir().join("refs/msfr_full_b.ref.txt.gz"));
    let a = read_ref(&vv_dir().join("refs/msfr_full_a.ref.txt.gz"));
    let s = run_full(&FullCfg {
        tag: "fullB",
        ref_name: "msfr_full_b",
        min_residual: Some("1e-7"),
        max_iterations: Some("6"),
        profile: false,
        lower_bound_residual: false,
        max_steps: None,
    });
    let n = s.n_steps;
    let upstream_iters: Vec<usize> = (1..=n).map(|k| full[&format!("CPITER_{k}")][0] as usize).collect();
    assert!(upstream_iters[..10].iter().all(|&c| c == 1), "steps 1-10 stop after one iteration");
    assert!(upstream_iters[10..263].iter().all(|&c| c == 6), "upstream runs the cap from step 11 to 263");
    assert!(upstream_iters.iter().all(|&c| c == 1 || c == 6));
    assert_eq!(s.port_cpiter, upstream_iters, "the port's loop takes the same number of iterations at every step");
    assert_eq!(s.neutron_iters_equal, n);
    // the extra iterations change the answer: B and A differ at the last common step (the test is not vacuous)
    let (pb, pa) = (full[&format!("POWER_{n}")][0], a[&format!("POWER_{n}")][0]);
    assert!(((pb - pa) / pa).abs() > 1e-6, "B differs from A by {:e}", ((pb - pa) / pa).abs());
    // measured 2026-11-26 (README, "Round 22 results"), gates ~2.5x outward; the `h` residual of iterations 2-6 (a
    // near-converged solve, 1.7e-3 relative) is printed, not gated
    assert!(s.first_dt_bad.is_none() && s.worst_dt < 3e-13, "dt {:e}", s.worst_dt);
    assert!(s.worst_t_field < 4e-12, "T field {:e}", s.worst_t_field);
    assert!(s.worst_tstat < 3e-12, "T stats {:e}", s.worst_tstat);
    assert!(s.worst_power < 6e-11, "power {:e}", s.worst_power);
    assert!(s.worst_co < 3e-13, "Courant {:e}", s.worst_co);
    assert!(s.worst_resid < 4e-5, "residual_ {:e}", s.worst_resid);
}

fn profile_case(max_steps: Option<usize>) -> FullSummary {
    run_full(&FullCfg {
        tag: if max_steps.is_some() { "fullC20" } else { "fullC" },
        ref_name: "msfr_full_c",
        min_residual: None,
        max_iterations: None,
        profile: true,
        lower_bound_residual: false,
        max_steps,
    })
}

/// Part 4 of gh:#1813 (the part that `2D_MSFR` can exercise): `temperatureTimeProfile` on the `hx` `fixedTemperature`
/// power model, a table of scaling factors `(0 1)(165 1)(175 0.95)`: the heat-sink surface falls from 900 K to 855 K over the
/// transient. Energy-only region, tutorial coupling dictionary. The upstream run was stopped at step 452 (t = 167.9 s) because
/// the cooling drives a power excursion (+5 %) in which the neutron loop hits its cap of 50 and the coupling loop its cap of 3
/// at 300 of the 452 steps. This is the first 20 steps (the `T` field at steps 1, 10, 20); the whole 452 steps are
/// [`msfr_fixed_temperature_time_profile_452_steps_match_upstream`]. Heat exchangers
/// (`structureProperties.<zone>.heatExchangers`) are NOT exercised: no case runs them in both codes (README, "Round 22").
#[test]
fn msfr_fixed_temperature_time_profile_matches_upstream() {
    let s = profile_case(Some(20));
    assert_eq!(s.cpiter_equal, 20);
    assert_eq!(s.neutron_iters_equal, 20);
    // measured 2026-11-26: see the README; gates ~2.5x outward
    assert!(s.first_dt_bad.is_none() && s.worst_dt < 3e-13, "dt {:e}", s.worst_dt);
    assert!(s.worst_t_field < 1e-14, "T field {:e}", s.worst_t_field);
    assert!(s.worst_tstat < 1e-13, "T stats {:e}", s.worst_tstat);
    assert!(s.worst_power < 1e-12, "power {:e}", s.worst_power);
    assert!(s.worst_co < 3e-13, "Courant {:e}", s.worst_co);
}

/// As above for all 452 steps of the reference. **Hour-scale: plain `#[ignore]`, 1090 s wall (2026-11-26, release, shared
/// 4-core machine)**; run with `--ignored`. Measured: README, "Round 22 results".
#[test]
#[ignore = "1090 s wall; run with --ignored"]
fn msfr_fixed_temperature_time_profile_452_steps_match_upstream() {
    let full = read_ref(&vv_dir().join("refs/msfr_full_c.ref.txt.gz"));
    let a = read_ref(&vv_dir().join("refs/msfr_full_a.ref.txt.gz"));
    let s = profile_case(None);
    let n = s.n_steps;
    assert_eq!(n, 452);
    assert_eq!(s.cpiter_equal, n);
    assert_eq!(s.neutron_iters_equal, n);
    // the profile matters (the test is not vacuous): the upstream runs with and without it differ in power by > 1e-4
    let (pc, pa) = (full["POWER_452"][2], a["POWER_452"][0]);
    assert!(((pc - pa) / pa).abs() > 1e-4, "profile changes the power at step 452 by {:e}", ((pc - pa) / pa).abs());
    assert!(s.first_dt_bad.is_none() && s.worst_dt < 3e-13, "dt {:e}", s.worst_dt);
    assert!(s.worst_t_field < 5e-12, "T field {:e}", s.worst_t_field);
    assert!(s.worst_tstat < 5e-12, "T stats {:e}", s.worst_tstat);
    assert!(s.worst_power < 9e-11, "power {:e}", s.worst_power);
    assert!(s.worst_co < 3e-13, "Courant {:e}", s.worst_co);
}
