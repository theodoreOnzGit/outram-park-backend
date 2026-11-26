// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//! # `2D_MSFR` transient stage, neutronics half, against the real binary (gh:#1361, round 6)
//!
//! AI-written, not human-reviewed. Research, education and V&V only.
//!
//! ## Methodology
//!
//! Upstream: GeN-Foam `652b3da0cc55` on OpenFOAM-ESI v2506, the unmodified
//! `GeN-Foam.C`, running `reactorCases/2D_MSFR` through `Allrun`'s three stages
//! (`scripts/run_msfr_chain.sh`): `steadyStateTH` (15 s), `steadyStateEN` (to
//! t = 165 s, `keff_` = 0.96031194083006322), then `transient`
//! (`eigenvalueNeutronics false`, `multiPhysicsSolver`: `onePhase` +
//! `diffusionNeutronics`, 1 coupling iteration per step) at the tight protocol
//! (every linear solver 1e-14 / relTol 0, `writePrecision 17`, one write per step).
//!
//! The port has no cell-centred `onePhase` region (gh:#1475), so the TH is **not**
//! compared. The comparison is one-way: the committed reference holds, for each of
//! the first 50 steps, the temperature field upstream's neutronics saw
//! (`neutroRegion/<t>/TFuel`, mapped from the fluid at the start of the step;
//! `rhoCool` follows bitwise from it by the Boussinesq law and `U`/`alpha` are
//! constant) and the time step upstream took. The port starts from upstream's
//! t = 165 s `flux`, `precStar` and `keff_`, advances **its own** flux and
//! precursors with
//! [`DiffusionNeutronics::upstream_transient_step`] (translation of
//! `solveNeutronics.H`, `liquidFuel true`), and is compared with upstream's own
//! `power:` line, neutron iteration count, per-group precursor and flux integrals
//! at every step, and per cell at step 50. Inputs, not answers, are handed over.
//!
//! Predictions Q1 to Q7 are in the README, "Round 6", committed before the first
//! run. Results: README, "Round 6 results".
//!
//! Nothing is skipped: all inputs are committed (`cases/msfr_neutro`,
//! `refs/msfr_transient.ref.txt.gz`, `tests/data/genfoam_msfr_precursor_cells.csv`).

// Reads case files: `std` only (gh:#1633).
#![cfg(feature = "std")]
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use miniz_oxide::inflate::decompress_to_vec;
use outram_foam_appbuilder_lib::genfoam::neutronics::albedo::AlbedoLinearisation;
use outram_foam_appbuilder_lib::genfoam::neutronics::diffusion::upstream_transient::{
    UpstreamTransientControls, UpstreamTransientFeedback,
};
use outram_foam_appbuilder_lib::genfoam::neutronics::diffusion::{
    DiffusionNeutronics, DiffusionSettings, PrecursorTransport,
};
use outram_foam_appbuilder_lib::genfoam::neutronics::xs::CrossSectionData;
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
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("verification_and_validation/genfoam_vs_upstream_gh1361")
}

fn gunzip(path: &Path) -> Vec<u8> {
    let bytes = std::fs::read(path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    decompress_to_vec(&bytes[10..bytes.len() - 8]).expect("inflate")
}

fn scratch_id() -> String {
    format!("{:?}", std::thread::current().id()).replace(['(', ')'], "")
}

fn stage_case() -> PathBuf {
    let src = vv_dir().join("cases/msfr_neutro");
    let dst = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("gh1361_msfr_{}", scratch_id()));
    let _ = std::fs::remove_dir_all(&dst);
    std::fs::create_dir_all(dst.join("polyMesh")).expect("scratch dir");
    std::fs::copy(src.join("nuclearData"), dst.join("nuclearData")).expect("nuclearData");
    for f in ["points", "faces", "owner", "neighbour", "boundary", "cellZones"] {
        let data = gunzip(&src.join("polyMesh").join(format!("{f}.gz")));
        std::fs::write(dst.join("polyMesh").join(f), data).expect("write polyMesh file");
    }
    dst
}

/// `KEY v v v ...` lines of the reference file.
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

struct Setup {
    mesh: Arc<FvMesh>,
    xs: CrossSectionData,
    zone_map: Vec<usize>,
}

/// The setup the comparison uses: chi at the reference state, as GeN-Foam's `XS` has it.
fn setup() -> Setup {
    setup_with(true)
}

/// `upstream_chi false` is the port's earlier data-driven chi (the ablation;
/// deprecated since 2026-11-26, when upstream's form became the default).
#[allow(deprecated)]
fn setup_with(upstream_chi: bool) -> Setup {
    let dir = stage_case();
    let mesh: Arc<FvMesh> = read_poly_mesh(&dir.join("polyMesh")).expect("polyMesh");
    let input = read_nuclear_data(&dir.join("nuclearData")).expect("nuclearData");
    let mut xs = CrossSectionData::from_input(&input).expect("cross sections");
    xs = if upstream_chi { xs.with_upstream_unparametrised_chi() } else { xs.with_data_driven_chi() };
    let zones = read_cell_zones(&dir.join("polyMesh")).expect("cellZones");
    let order: Vec<&str> = (0..xs.zone_count()).map(|i| xs.zone(i).expect("zone").name()).collect();
    let zone_map = zone_of_cell(&zones, &order, mesh.n_cells).expect("zone map");
    Setup { mesh, xs, zone_map }
}

/// `alpha` of the neutronics mesh (geometry: fluid volume fraction).
fn alpha_cells() -> Vec<f64> {
    let csv = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/genfoam_msfr_precursor_cells.csv"),
    )
    .expect("precursor cell fixture");
    let mut alpha = Vec::new();
    for line in csv.lines() {
        if line.starts_with('#') || line.starts_with("alpha,") || line.trim().is_empty() {
            continue;
        }
        alpha.push(line.split(',').next().unwrap().parse::<f64>().unwrap());
    }
    alpha
}

/// `phi = fvc::flux(U alpha)` on the neutronics mesh, as `solveNeutronics.H`
/// builds it. The two wedge patches carry no flux: upstream's wedge patch value
/// is the cell value rotated into the wedge plane, normal to nothing.
fn face_flux(mesh: &Arc<FvMesh>, u: &[f64], alpha: &[f64]) -> PrecursorTransport {
    let n = mesh.n_cells;
    let mut field = VolVectorField::uniform("U", mesh.clone(), Vector3::new(0.0, 0.0, 0.0));
    field.internal = Field::new(
        (0..n)
            .map(|c| Vector3::new(u[3 * c] * alpha[c], u[3 * c + 1] * alpha[c], u[3 * c + 2] * alpha[c]))
            .collect(),
    );
    let phi = fvc::flux(&field);
    let phi_internal: Vec<f64> = (0..mesh.n_internal_faces).map(|f| phi.internal[f]).collect();
    let phi_boundary: Vec<Vec<f64>> = mesh
        .patches
        .iter()
        .enumerate()
        .map(|(pi, p)| {
            (0..p.size)
                .map(|fi| if p.kind == PatchKind::Wedge { 0.0 } else { phi.boundary[pi].values[fi] })
                .collect()
        })
        .collect();
    PrecursorTransport::new(mesh, alpha.to_vec(), phi_internal, phi_boundary, vec![0.0; n])
}

/// ESI `Boussinesq::rho` of `thermophysicalProperties`: `rho0 (1 - beta (T - T0))`.
fn rho_cool(t: f64) -> f64 {
    4125.0 * (1.0 - 2e-4 * (t - 900.0))
}

/// What one run of the port produced, per step.
struct PortStep {
    power: f64,
    iterations: usize,
    prec_totals: Vec<f64>,
    flux_totals: Vec<f64>,
    residuals: Vec<f64>,
    group_residuals: Vec<f64>,
}

struct PortRun {
    steps: Vec<PortStep>,
    flux: Vec<Vec<f64>>,
    prec: Vec<Vec<f64>>,
}

fn run_port(s: &Setup, reference: &HashMap<String, Vec<f64>>, n_steps: usize) -> PortRun {
    let mesh = s.mesh.clone();
    let n = mesh.n_cells;
    let groups = s.xs.energy_groups();
    let precs = s.xs.prec_groups();

    // Flux boundary: albedo on the four walls, zero gradient elsewhere (the
    // wedge planes carry no current), as in `genfoam_tutorial_keff`.
    let flux_boundary: Vec<BoundaryCondition<f64>> =
        mesh.patches.iter().map(|_| BoundaryCondition::ZeroGradient).collect();
    let settings = DiffusionSettings {
        linear: SolverSettings { tolerance: 1e-14, max_iter: 100_000 },
        ..DiffusionSettings::default()
    };
    // Cell parameters of step 0: the t = 165 s temperature, only to build the model.
    let t0 = &reference["T_0"];
    let params0: Vec<f64> = t0.iter().flat_map(|&t| [t, rho_cool(t)]).collect();
    let mut solver = DiffusionNeutronics::new_with_cell_parameters(
        mesh.clone(),
        &s.xs,
        &s.zone_map,
        &params0,
        &flux_boundary,
        settings,
    )
    .expect("diffusion model");
    for (name, gamma) in [("topwall", 0.1), ("bottomwall", 0.1), ("reflector", 0.1), ("hx", 0.5)] {
        let idx = mesh.patches.iter().position(|p| p.name == name).expect("patch");
        solver.set_albedo_boundary(idx, *&gamma, AlbedoLinearisation::CellValue);
    }
    let alpha = alpha_cells();
    solver.set_precursor_drift(face_flux(&mesh, &reference["START_U"], &alpha));
    let flux0: Vec<Vec<f64>> = (0..groups).map(|g| reference[&format!("START_flux{g}")].clone()).collect();
    solver.set_initial_guess(&flux0, reference["START_keff"][0]);
    let prec_star0: Vec<Vec<f64>> =
        (0..precs).map(|k| reference[&format!("START_precStar{k}")].clone()).collect();
    solver.set_prec_star(&prec_star0).expect("precStar");

    let controls = UpstreamTransientControls {
        neutron_iteration_residual: 1e-6,
        max_neutron_iterations: 50,
        sc_no: 1.0,
    };
    let mu = vec![0.01; n];
    let alphat = vec![0.01 * (1.0 / 8.0); n];
    let vols = mesh.cell_volumes.clone();
    let mut steps = Vec::new();
    for i in 1..=n_steps {
        let t = &reference[&format!("T_{i}")];
        let params: Vec<f64> = t.iter().flat_map(|&x| [x, rho_cool(x)]).collect();
        let rho: Vec<f64> = t.iter().map(|&x| rho_cool(x)).collect();
        let dt = reference[&format!("STEP_{i}")][1];
        let rep = solver
            .upstream_transient_step(
                &s.xs,
                &s.zone_map,
                UpstreamTransientFeedback { cell_parameters: &params, rho_cool: &rho, alphat: &alphat, mu: &mu },
                dt,
                &controls,
            )
            .expect("transient step");
        let prec_totals = (0..precs)
            .map(|k| {
                let p = solver.state().precursors()[k].internal.as_slice();
                (0..n).map(|c| vols[c] * p[c]).sum::<f64>()
            })
            .collect();
        let flux_totals = (0..groups)
            .map(|g| {
                let p = solver.state().flux()[g].internal.as_slice();
                (0..n).map(|c| vols[c] * p[c]).sum::<f64>()
            })
            .collect();
        steps.push(PortStep {
            power: rep.power,
            iterations: rep.neutron_iterations,
            prec_totals,
            flux_totals,
            residuals: rep.residual_history,
            group_residuals: rep.first_iteration_group_residuals,
        });
    }
    PortRun {
        steps,
        flux: (0..groups).map(|g| solver.state().flux()[g].internal.as_slice().to_vec()).collect(),
        prec: (0..precs).map(|k| solver.state().precursors()[k].internal.as_slice().to_vec()).collect(),
    }
}

fn rel(a: f64, b: f64) -> f64 {
    ((a - b) / b).abs()
}

/// Port against upstream, 50 steps. Prints the per-step table; the assertions are
/// the predictions of the README (Q1-Q6) at the values the README records.
#[test]
fn msfr_transient_neutronics_matches_upstream_over_50_steps() {
    let reference = read_ref(&vv_dir().join("refs/msfr_transient.ref.txt.gz"));
    let s = setup();
    let started = std::time::Instant::now();
    let run = run_port(&s, &reference, N_STEPS);
    let wall = started.elapsed().as_secs_f64();

    for k in [1usize, 2, 3] {
        let up = &reference[&format!("NRES_{k}")];
        let pr = &run.steps[k - 1].residuals;
        println!("step {k} neutroResidual by iteration (port / upstream):");
        for j in 0..up.len().max(pr.len()).min(8) {
            println!("   {:2}: {:.10e} / {:.10e}", j + 1, pr.get(j).copied().unwrap_or(f64::NAN), up.get(j).copied().unwrap_or(f64::NAN));
        }
    }
    println!("step 1 first-iteration group residuals (port): {:?}", run.steps[0].group_residuals);
    let mut worst_power = 0.0_f64;
    let mut worst_prec = 0.0_f64;
    let mut worst_flux = 0.0_f64;
    let mut iter_equal = 0usize;
    let mut first_power_bad = None;
    println!("step  port_power            up_power              rel_dP      iter(p/u)  rel_prec_max  rel_flux_max");
    for (i, st) in run.steps.iter().enumerate() {
        let k = i + 1;
        let up = &reference[&format!("STEP_{k}")];
        let (up_power, up_iter) = (up[2], up[3] as usize);
        let dp = rel(st.power, up_power);
        let up_prec = &reference[&format!("PREC_TOT_{k}")];
        let up_flux = &reference[&format!("FLUX_INT_{k}")];
        let dprec = st.prec_totals.iter().zip(up_prec).map(|(a, b)| rel(*a, *b)).fold(0.0, f64::max);
        let dflux = st.flux_totals.iter().zip(up_flux).map(|(a, b)| rel(*a, *b)).fold(0.0, f64::max);
        println!(
            "{k:4}  {:.12e}  {:.12e}  {dp:.3e}  {:3}/{:3}    {dprec:.3e}     {dflux:.3e}",
            st.power, up_power, st.iterations, up_iter
        );
        worst_power = worst_power.max(dp);
        worst_prec = worst_prec.max(dprec);
        worst_flux = worst_flux.max(dflux);
        if st.iterations == up_iter {
            iter_equal += 1;
        }
        if dp > 1e-9 && first_power_bad.is_none() {
            first_power_bad = Some(k);
        }
    }

    // per cell at step 50, relative to the field maximum
    let mut worst_cell = 0.0_f64;
    for g in 0..run.flux.len() {
        let up = &reference[&format!("END_flux{g}")];
        let mx = up.iter().cloned().fold(0.0, f64::max);
        for (a, b) in run.flux[g].iter().zip(up) {
            worst_cell = worst_cell.max(((a - b) / mx).abs());
        }
    }
    let mut worst_cell_prec = 0.0_f64;
    for k in 0..run.prec.len() {
        let up = &reference[&format!("END_prec{k}")];
        let mx = up.iter().cloned().fold(0.0, f64::max);
        for (a, b) in run.prec[k].iter().zip(up) {
            worst_cell_prec = worst_cell_prec.max(((a - b) / mx).abs());
        }
    }
    {
        let last = &run.steps[N_STEPS - 1];
        let up = &reference["PREC_TOT_50"];
        let per_group: Vec<String> = last.prec_totals.iter().zip(up).map(|(a, b)| format!("{:.2e}", (a - b) / b)).collect();
        println!("step 50 signed precursor-total difference by group: {per_group:?}");
        let upf = &reference["FLUX_INT_50"];
        let per_flux: Vec<String> = last.flux_totals.iter().zip(upf).map(|(a, b)| format!("{:.2e}", (a - b) / b)).collect();
        println!("step 50 signed flux-integral difference by group: {per_flux:?}");
    }
    let up_dp = reference["STEP_50"][2] - 2.0e7;
    let port_dp = run.steps[N_STEPS - 1].power - 2.0e7;
    println!(
        "worst power {worst_power:.3e}, precursor totals {worst_prec:.3e}, flux integrals {worst_flux:.3e}; \
         iteration counts equal {iter_equal}/{N_STEPS}; step 50 per cell (of field max): flux {worst_cell:.3e}, \
         prec {worst_cell_prec:.3e}; power change since 2e7: port {port_dp:.6e} upstream {up_dp:.6e} (rel {:.3e}); \
         first step with power > 1e-9: {first_power_bad:?}; wall {wall:.1} s",
        rel(port_dp, up_dp)
    );

    assert_eq!(run.steps.len(), N_STEPS);
    // The second-run predictions H3-H6 of the README, as written before that run.
    assert!(wall < 300.0, "Q7: {wall} s");
    assert!(worst_power <= 1e-8, "H3: worst power difference {worst_power:e}, first above 1e-9 at step {first_power_bad:?}");
    assert!(iter_equal >= 48, "H4: iteration counts equal at {iter_equal}/50");
    assert!(worst_prec <= 1e-8 && worst_flux <= 1e-8, "H5: precursor totals {worst_prec:e}, flux integrals {worst_flux:e}");
    assert!(worst_cell <= 1e-8 && worst_cell_prec <= 1e-8, "H5: per cell {worst_cell:e} / {worst_cell_prec:e}");
    assert!(rel(port_dp, up_dp) <= 1e-5, "H6: power change {port_dp} vs {up_dp}");
}

/// Ablation, pinned at its measured miss: the same 50 steps with the port's
/// ~~default~~ **deprecated (since 2026-11-26)** **data-driven chi**
/// (`CrossSectionData::with_data_driven_chi`). Upstream never interpolates chi, so this
/// is the port differing from upstream by exactly that one thing.
///
/// Measured 2026-11-25: worst power difference 1.838e-4 (first step 7.4e-6, from
/// step 3 on 1.75e-4 to 1.84e-4), neutron iteration counts equal at 43 of 50, power
/// change at step 50 +1.0045e4 W against upstream's +1.3566e4 W. Bounds below are
/// the measured values rounded outward by about 2x; they pin that the one option
/// is what closes the gap, not a tolerance on a quantity to be matched.
#[test]
fn msfr_transient_with_data_driven_chi_misses_by_the_pinned_amount() {
    let reference = read_ref(&vv_dir().join("refs/msfr_transient.ref.txt.gz"));
    let s = setup_with(false);
    let run = run_port(&s, &reference, N_STEPS);
    let mut worst = 0.0_f64;
    let mut equal = 0usize;
    for (i, st) in run.steps.iter().enumerate() {
        let up = &reference[&format!("STEP_{}", i + 1)];
        worst = worst.max(rel(st.power, up[2]));
        if st.iterations == up[3] as usize {
            equal += 1;
        }
    }
    let up_dp = reference["STEP_50"][2] - 2.0e7;
    let port_dp = run.steps[N_STEPS - 1].power - 2.0e7;
    println!("data-driven chi: worst power {worst:.3e}, iteration counts equal {equal}/50, change {port_dp:.4e} vs {up_dp:.4e}");
    assert!((1e-4..4e-4).contains(&worst), "worst power difference {worst:e}");
    assert!(equal < 48, "iteration counts equal at {equal}/50");
    assert!((port_dp / up_dp - 1.0).abs() > 0.1, "power change {port_dp} vs {up_dp}");
}

/// Writes `refs/msfr_transient.ref.txt` from an upstream run directory. Needs the
/// run (`GENFOAM_MSFR_RUN`, default `/home/user/upstream-build/gh1361/runs/msfr/transient`), so it is
/// `#[ignore]`d; gzip the output by hand. Cell volumes are the port's.
#[test]
#[ignore = "needs the upstream run directory; regenerates refs/msfr_transient.ref.txt"]
fn extract_msfr_transient_reference() {
    let run = PathBuf::from(
        std::env::var("GENFOAM_MSFR_RUN")
            .unwrap_or_else(|_| "/home/user/upstream-build/gh1361/runs/msfr/transient".to_string()),
    );
    let out = PathBuf::from(std::env::var("GENFOAM_MSFR_OUT").expect("GENFOAM_MSFR_OUT"));
    let s = setup();
    let n = s.mesh.n_cells;
    let vols = s.mesh.cell_volumes.clone();
    let read = |dir: &str, name: &str| -> Vec<f64> {
        outram_foam_appbuilder_lib::io::field_reader::read_vol_scalar_field(
            &run.join(dir).join("neutroRegion").join(name),
            n,
        )
        .unwrap_or_else(|e| panic!("{dir}/{name}: {e}"))
    };
    let line = |key: &str, v: &[f64]| -> String {
        let mut s = String::from(key);
        for x in v {
            s.push(' ');
            s.push_str(&format!("{x:e}"));
        }
        s.push('\n');
        s
    };
    let mut text = String::new();

    // the log: per step Time, power, iterations, residual, deltaT (of the NEXT step)
    let log = std::fs::read_to_string(run.join("log.GeN-Foam")).expect("log");
    let mut times = Vec::new();
    let mut rows: Vec<(String, f64, usize, f64)> = Vec::new();
    let mut dts = Vec::new();
    let mut nres: Vec<Vec<f64>> = Vec::new();
    for l in log.lines() {
        if l.starts_with("Time = ") {
            nres.push(Vec::new());
        }
        if let Some(r) = l.strip_prefix("neutroResidual (neutroIter: ") {
            nres.last_mut().unwrap().push(r.split_once("): ").unwrap().1.trim().parse().unwrap());
        }
        if let Some(t) = l.strip_prefix("Time = ") {
            times.push(t.trim().to_string());
        } else if let Some(r) = l.strip_prefix("Final neutroResidual at neutroIter no ") {
            // "50: 1.08e-06 - pow:20003657.47"
            let (it, rest) = r.split_once(':').unwrap();
            let (res, pow) = rest.trim().split_once(" - pow:").unwrap();
            rows.push((times.last().unwrap().clone(), pow.trim().parse().unwrap(), it.trim().parse().unwrap(), res.trim().parse().unwrap()));
        } else if let Some(d) = l.strip_prefix("deltaT = ") {
            dts.push(d.trim().parse::<f64>().unwrap());
        }
    }
    let mut dt_used = vec![1.0e-3];
    dt_used.extend(dts.iter().copied());

    let flux_names: Vec<String> = (0..6).map(|g| format!("flux{g}")).collect();
    let prec_names: Vec<String> = (0..8).map(|k| format!("prec{k}")).collect();
    text += &line("START_keff", &[{
        let t = std::fs::read_to_string(run.join("165/uniform/reactorState")).unwrap();
        t.lines().find(|l| l.starts_with("keff")).unwrap().split_whitespace().nth(1).unwrap().trim_end_matches(';').parse().unwrap()
    }]);
    for g in &flux_names {
        text += &line(&format!("START_{g}"), &read("165", g));
    }
    for k in 0..8 {
        text += &line(&format!("START_precStar{k}"), &read("165", &format!("precStar{k}")));
    }
    let u = outram_foam_appbuilder_lib::io::field_reader::read_vol_vector_field(&run.join("165/neutroRegion/U"), n)
        .expect("U");
    let u_flat: Vec<f64> = u.iter().flat_map(|v| [v.x, v.y, v.z]).collect();
    text += &line("START_U", &u_flat);
    text += &line("T_0", &read("165", "TFuel"));

    for i in 1..=N_STEPS {
        let (tname, power, iters, res) = &rows[i - 1];
        assert_eq!(tname, &times[i - 1], "step {i}");
        text += &line(&format!("STEP_{i}"), &[*&(i as f64), dt_used[i - 1], *power, *iters as f64, *res]);
        text += &line(&format!("NRES_{i}"), &nres[i - 1]);
        text += &line(&format!("T_{i}"), &read(tname, "TFuel"));
        let pt: Vec<f64> = (0..8)
            .map(|k| {
                let p = read(tname, &format!("prec{k}"));
                (0..n).map(|c| vols[c] * p[c]).sum()
            })
            .collect();
        text += &line(&format!("PREC_TOT_{i}"), &pt);
        let ft: Vec<f64> = (0..6)
            .map(|g| {
                let p = read(tname, &format!("flux{g}"));
                (0..n).map(|c| vols[c] * p[c]).sum()
            })
            .collect();
        text += &line(&format!("FLUX_INT_{i}"), &ft);
        if i == N_STEPS {
            for g in &flux_names {
                text += &line(&format!("END_{g}"), &read(tname, g));
            }
            for k in 0..8 {
                text += &line(&format!("END_prec{k}"), &read(tname, &format!("prec{k}")));
            }
        }
    }
    std::fs::write(out, text).expect("write");
    let _ = prec_names;
}

/// Diagnostic (not a gate): the port's static eigenvalue at upstream's t = 165 s
/// state against upstream's `keff_`, for each linearisation of the albedo boundary.
#[test]
#[ignore = "diagnostic: static eigenvalue of the port at upstream's t = 165 s state"]
fn diagnose_static_keff_at_start_state() {
    let reference = read_ref(&vv_dir().join("refs/msfr_transient.ref.txt.gz"));
    let s = setup();
    let mesh = s.mesh.clone();
    let n = mesh.n_cells;
    let t0 = &reference["T_0"];
    let params0: Vec<f64> = t0.iter().flat_map(|&t| [t, rho_cool(t)]).collect();
    let alpha = alpha_cells();
    let up_k = reference["START_keff"][0];
    for lin in [AlbedoLinearisation::CellValue, AlbedoLinearisation::FaceValue] {
        let flux_boundary: Vec<BoundaryCondition<f64>> =
            mesh.patches.iter().map(|_| BoundaryCondition::ZeroGradient).collect();
        let settings = DiffusionSettings {
            k_tolerance: 1e-13,
            flux_tolerance: 1e-11,
            max_outer_iterations: 20000,
            linear: SolverSettings { tolerance: 1e-14, max_iter: 100_000 },
            ..DiffusionSettings::default()
        };
        let mut solver = DiffusionNeutronics::new_with_cell_parameters(
            mesh.clone(), &s.xs, &s.zone_map, &params0, &flux_boundary, settings,
        )
        .unwrap();
        for (name, gamma) in [("topwall", 0.1), ("bottomwall", 0.1), ("reflector", 0.1), ("hx", 0.5)] {
            let idx = mesh.patches.iter().position(|p| p.name == name).unwrap();
            solver.set_albedo_boundary(idx, gamma, lin);
        }
        let mut tr = face_flux(&mesh, &reference["START_U"], &alpha);
        let diff: Vec<f64> = t0.iter().map(|&t| (0.01 * (1.0 / 8.0) + 0.01 / 1.0) / rho_cool(t)).collect();
        tr.diffusivity.internal = Field::new(diff);
        solver.set_precursor_drift(tr);
        let flux0: Vec<Vec<f64>> = (0..6).map(|g| reference[&format!("START_flux{g}")].clone()).collect();
        solver.set_initial_guess(&flux0, up_k);
        let rep = solver.solve_eigenvalue().expect("eigenvalue");
        println!(
            "{lin:?}: k_port = {:.10} upstream {:.10}  diff {:+.3} pcm (outer {}, converged {}) n={n}",
            rep.k_eff, up_k, 1e5 * (rep.k_eff - up_k) / up_k, rep.outer_iterations, rep.converged
        );
    }
}

/// Diagnostic (not a gate): the port's mesh geometry against the values ESI v2506
/// computes for the same polyMesh (`upstream_driver/dump_laplacian_geometry`).
#[test]
#[ignore = "diagnostic: needs the upstream geometry dump"]
fn diagnose_mesh_geometry_against_upstream() {
    use outram_foam_basic_lib::prelude::fvm::DeltaCoeff;
    let dir = PathBuf::from(
        std::env::var("GENFOAM_MSFR_RUN")
            .unwrap_or_else(|_| "/home/user/upstream-build/gh1361/runs/msfr/transient".to_string()),
    )
    .join("geometry");
    let read = |n: &str| -> Vec<f64> {
        std::fs::read_to_string(dir.join(n))
            .unwrap_or_else(|e| panic!("{n}: {e}"))
            .lines()
            .map(|l| l.trim().parse().unwrap())
            .collect()
    };
    let s = setup();
    let m = &s.mesh;
    // The port's `fvc::interpolate` weight (`fvc/interpolate.rs::linear_weight`), restated.
    let linear_weight = |m: &FvMesh, f: usize| -> f64 {
        let (co, cn, cf, sf) = (
            m.cell_centres[m.owner[f]],
            m.cell_centres[m.neighbour[f]],
            m.face_centres[f],
            m.face_area_vectors[f],
        );
        let own = sf.dot(cf - co).abs();
        let nei = sf.dot(cn - cf).abs();
        nei / (own + nei)
    };
    let cmp = |name: &str, port: Vec<f64>, up: Vec<f64>| {
        assert_eq!(port.len(), up.len(), "{name}");
        let mut worst = 0.0_f64;
        let mut at = 0;
        let mut differ = 0;
        for i in 0..port.len() {
            let r = if up[i] == 0.0 { (port[i] - up[i]).abs() } else { ((port[i] - up[i]) / up[i]).abs() };
            if r > 0.0 {
                differ += 1;
            }
            if r > worst {
                worst = r;
                at = i;
            }
        }
        println!("{name:34} n={:6} differ={differ:6} worst rel {worst:.3e} at {at} (port {:.17e} up {:.17e})", port.len(), port[at], up[at]);
    };
    cmp("V", m.cell_volumes.to_vec(), read("V"));
    cmp("magSf internal", (0..m.n_internal_faces).map(|f| m.face_area_vectors[f].mag()).collect(), read("magSf"));
    cmp("weights", (0..m.n_internal_faces).map(|f| linear_weight(m, f)).collect(), read("weights"));
    cmp(
        "nonOrthDeltaCoeffs",
        (0..m.n_internal_faces).map(|f| 1.0 / DeltaCoeff::NonOrthogonal.interior_delta(m, f)).collect(),
        read("nonOrthDeltaCoeffs"),
    );
    cmp(
        "deltaCoeffs",
        (0..m.n_internal_faces).map(|f| 1.0 / DeltaCoeff::Orthogonal.interior_delta(m, f)).collect(),
        read("deltaCoeffs"),
    );
    for p in &m.patches {
        let faces = || (p.start..p.start + p.size);
        cmp(
            &format!("bnd {} magSf", p.name),
            faces().map(|f| m.face_area_vectors[f].mag()).collect(),
            read(&format!("bnd_{}_magSf", p.name)),
        );
        cmp(
            &format!("bnd {} nonOrthDeltaCoeffs", p.name),
            faces().map(|f| 1.0 / DeltaCoeff::NonOrthogonal.boundary_delta(m, f)).collect(),
            read(&format!("bnd_{}_nonOrthDeltaCoeffs", p.name)),
        );
        cmp(
            &format!("bnd {} patchDeltaCoeffs", p.name),
            faces().map(|f| 1.0 / DeltaCoeff::Orthogonal.boundary_delta(m, f)).collect(),
            read(&format!("bnd_{}_patchDeltaCoeffs", p.name)),
        );
    }
}

/// Diagnostic (not a gate): where in the mesh the port's balance of upstream's
/// t = 165 s flux is not satisfied. Steady form of `fluxEq.H` per group with
/// upstream's own `prec` (no precursor re-solve), `keff_` of upstream; prints the
/// per-group residual norm relative to the source norm and the five worst cells.
#[test]
#[ignore = "diagnostic: balance residual of upstream's start state in the port"]
fn diagnose_start_state_balance() {
    use outram_foam_basic_lib::prelude::fvm;
    let reference = read_ref(&vv_dir().join("refs/msfr_transient.ref.txt.gz"));
    let s = setup();
    let mesh = s.mesh.clone();
    let n = mesh.n_cells;
    let t0 = &reference["T_0"];
    let params0: Vec<f64> = t0.iter().flat_map(|&t| [t, rho_cool(t)]).collect();
    let alpha = alpha_cells();
    let keff = reference["START_keff"][0];
    let flux_boundary: Vec<BoundaryCondition<f64>> =
        mesh.patches.iter().map(|_| BoundaryCondition::ZeroGradient).collect();
    let mut solver = DiffusionNeutronics::new_with_cell_parameters(
        mesh.clone(), &s.xs, &s.zone_map, &params0, &flux_boundary, DiffusionSettings::default(),
    )
    .unwrap();
    for (name, gamma) in [("topwall", 0.1), ("bottomwall", 0.1), ("reflector", 0.1), ("hx", 0.5)] {
        let idx = mesh.patches.iter().position(|p| p.name == name).unwrap();
        solver.set_albedo_boundary(idx, gamma, AlbedoLinearisation::CellValue);
    }
    solver.set_precursor_drift(face_flux(&mesh, &reference["START_U"], &alpha));
    let flux0: Vec<Vec<f64>> = (0..6).map(|g| reference[&format!("START_flux{g}")].clone()).collect();
    solver.set_initial_guess(&flux0, keff);
    let prec_star0: Vec<Vec<f64>> = (0..8).map(|k| reference[&format!("START_precStar{k}")].clone()).collect();
    solver.set_prec_star(&prec_star0).unwrap();
    let xs = solver.xs_fields().clone();
    let vols = mesh.cell_volumes.clone();
    // S = sum nuSigmaEff phi, delayed = sum lambda prec
    let mut ssrc = vec![0.0; n];
    for g in 0..6 {
        for c in 0..n {
            ssrc[c] += xs.nu_sigma_f[g].internal[c] * flux0[g][c];
        }
    }
    let mut delayed = vec![0.0; n];
    for k in 0..8 {
        for c in 0..n {
            delayed[c] += xs.lambda[k].internal[c] * (prec_star0[k][c] * alpha[c]);
        }
    }
    for g in 0..6 {
        let mut scatter = vec![0.0; n];
        for j in 0..6 {
            if j != g {
                for c in 0..n {
                    scatter[c] += xs.scattering[j][g].internal[c] * flux0[j][c];
                }
            }
        }
        let flux_g = &solver.state().flux()[g];
        let tmpl = flux_g.clone();
        let mk = |v: Vec<f64>| {
            let mut f = tmpl.clone();
            f.internal = Field::new(v);
            f
        };
        let rhs: Vec<f64> = (0..n)
            .map(|c| {
                ((ssrc[c] * (1.0 - xs.beta_tot.internal[c]) * xs.chi_prompt[g].internal[c]) / keff)
                    + delayed[c] * xs.chi_delayed[g].internal[c]
                    + scatter[c]
            })
            .collect();
        let eqn = fvm::laplacian_with_delta(&xs.d_face[g], flux_g, DiffusionSettings::default().delta_coeff)
            + fvm::sp(&xs.sigma_removal[g], flux_g)
            - fvm::su(&mk(rhs.clone()), flux_g);
        let b: Vec<f64> = eqn.source.iter().copied().collect();
        let res = eqn.ldu.openfoam_initial_residual(flux_g.internal.as_slice(), &b);
        // cell-wise residual b - A x
        let x = flux_g.internal.as_slice();
        let mut ax = vec![0.0; n];
        for c in 0..n {
            ax[c] = eqn.ldu.diag[c] * x[c];
        }
        for f in 0..mesh.n_internal_faces {
            let (o, nb) = (mesh.owner[f], mesh.neighbour[f]);
            ax[o] += eqn.ldu.upper[f] * x[nb];
            ax[nb] += eqn.ldu.lower[f] * x[o];
        }
        let mut worst: Vec<(f64, usize)> = (0..n).map(|c| ((b[c] - ax[c]).abs() / (vols[c] * rhs[c].abs().max(1e-300)), c)).collect();
        worst.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());
        let sumabs: f64 = (0..n).map(|c| (b[c] - ax[c]).abs()).sum();
        let sumsrc: f64 = (0..n).map(|c| b[c].abs()).sum();
        if g <= 1 {
            // sensitivity of the OpenFOAM initial residual to a uniform scaling of the
            // prompt source (i.e. of keff): is the imbalance a uniform factor?
            for delta in [-3e-5, -1e-5, -5e-6, 0.0, 5e-6, 1e-5, 3e-5] {
                let rhs2: Vec<f64> = (0..n)
                    .map(|c| {
                        ((ssrc[c] * (1.0 - xs.beta_tot.internal[c]) * xs.chi_prompt[g].internal[c]) / (keff * (1.0 + delta)))
                            + delayed[c] * xs.chi_delayed[g].internal[c]
                            + scatter[c]
                    })
                    .collect();
                let e2 = fvm::laplacian_with_delta(&xs.d_face[g], flux_g, DiffusionSettings::default().delta_coeff)
                    + fvm::sp(&xs.sigma_removal[g], flux_g)
                    - fvm::su(&mk(rhs2), flux_g);
                let b2: Vec<f64> = e2.source.iter().copied().collect();
                println!(
                    "  group {g}: keff x (1{delta:+.0e}) -> initial residual {:.3e}",
                    e2.ldu.openfoam_initial_residual(flux_g.internal.as_slice(), &b2)
                );
            }
        }
        if g == 0 {
            for &(r, c) in worst.iter().take(12) {
                let mut touching = Vec::new();
                for p in &mesh.patches {
                    for f in p.start..p.start + p.size {
                        if mesh.owner[f] == c && p.name != "front" && p.name != "back" {
                            touching.push(p.name.clone());
                        }
                    }
                }
                let mut nbr_zones = Vec::new();
                for f in 0..mesh.n_internal_faces {
                    if mesh.owner[f] == c {
                        nbr_zones.push(s.zone_map[mesh.neighbour[f]]);
                    } else if mesh.neighbour[f] == c {
                        nbr_zones.push(s.zone_map[mesh.owner[f]]);
                    }
                }
                let mut fdesc = Vec::new();
                for f in 0..mesh.n_internal_faces {
                    if mesh.owner[f] == c || mesh.neighbour[f] == c {
                        let d = mesh.cell_centres[mesh.neighbour[f]] - mesh.cell_centres[mesh.owner[f]];
                        let cosang = d.dot(mesh.face_area_vectors[f]) / (d.mag() * mesh.face_area_vectors[f].mag());
                        fdesc.push(format!("{:.3}", cosang.abs()));
                    }
                }
                println!("    internal faces {} cos(non-orth) {:?}", fdesc.len(), fdesc);
                let sgn = (b[c] - ax[c]).signum();
                println!(
                    "  cell {c}: r/(V src) {sgn:+}{r:.2e}, zone {}, neighbour zones {nbr_zones:?}, centre ({:.3},{:.3}), other patches {:?}, alpha {}",
                    s.zone_map[c], mesh.cell_centres[c].y, mesh.cell_centres[c].z, touching, alpha[c]
                );
            }
        }
        println!(
            "group {g}: openfoam initial residual {res:.3e}, sum|r|/sum|b| {:.3e}; worst cells (|r|/(V*src)): {:?}",
            sumabs / sumsrc,
            worst.iter().take(4).map(|(r, c)| format!("c{c}:{r:.1e}")).collect::<Vec<_>>()
        );
    }
}

/// Diagnostic (not a gate): the port's cross-section parametrisation against the
/// one cross-section combination upstream writes: `powerDensity = sum_g flux_g
/// sigmaPow_g / (fuelFraction (1 - fractionToSecondaryPower))`, per cell, at the
/// t = 165 s state.
#[test]
#[ignore = "diagnostic: needs the upstream run directory"]
fn diagnose_power_density_per_cell() {
    let run = PathBuf::from(
        std::env::var("GENFOAM_MSFR_RUN")
            .unwrap_or_else(|_| "/home/user/upstream-build/gh1361/runs/msfr/transient".to_string()),
    );
    let reference = read_ref(&vv_dir().join("refs/msfr_transient.ref.txt.gz"));
    let s = setup();
    let mesh = s.mesh.clone();
    let n = mesh.n_cells;
    let rd = |name: &str| {
        outram_foam_appbuilder_lib::io::field_reader::read_vol_scalar_field(&run.join("165/neutroRegion").join(name), n).unwrap()
    };
    let pd_up = rd("powerDensity");
    let spd_up = rd("secondaryPowerDensity");
    let t0 = &reference["T_0"];
    let params0: Vec<f64> = t0.iter().flat_map(|&t| [t, rho_cool(t)]).collect();
    let flux_boundary: Vec<BoundaryCondition<f64>> =
        mesh.patches.iter().map(|_| BoundaryCondition::ZeroGradient).collect();
    let solver = DiffusionNeutronics::new_with_cell_parameters(
        mesh.clone(), &s.xs, &s.zone_map, &params0, &flux_boundary, DiffusionSettings::default(),
    )
    .unwrap();
    let xs = solver.xs_fields();
    let mut worst = Vec::new();
    for c in 0..n {
        let mut p = 0.0;
        for g in 0..6 {
            p += reference[&format!("START_flux{g}")][c] * xs.sigma_pow[g].internal[c];
        }
        worst.push((((p - pd_up[c]) / pd_up[c]).abs(), c, p, pd_up[c]));
    }
    worst.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());
    let nz = pd_up.iter().filter(|&&v| v != 0.0).count();
    println!("powerDensity cells {n}, nonzero upstream {nz}; secondaryPowerDensity max {:e}", spd_up.iter().cloned().fold(0.0, f64::max));
    for (r, c, p, u) in worst.iter().take(8) {
        println!("  cell {c} zone {} T {:.3}: port {p:.17e} up {u:.17e} rel {r:.3e}", s.zone_map[*c], t0[*c]);
    }
    let mut hist = [0usize; 6];
    for (r, ..) in &worst {
        let k = if *r == 0.0 { 0 } else if *r < 1e-14 { 1 } else if *r < 1e-12 { 2 } else if *r < 1e-9 { 3 } else if *r < 1e-6 { 4 } else { 5 };
        hist[k] += 1;
    }
    println!("rel diff histogram: =0 {} | <1e-14 {} | <1e-12 {} | <1e-9 {} | <1e-6 {} | >=1e-6 {}", hist[0], hist[1], hist[2], hist[3], hist[4], hist[5]);
}

/// Diagnostic (not a gate): group-0..2 initial residual of upstream's start state
/// when one albedo coefficient is scaled; says which boundary the imbalance sits on.
#[test]
#[ignore = "diagnostic: albedo sensitivity of the start-state balance"]
fn diagnose_albedo_sensitivity() {
    use outram_foam_basic_lib::prelude::fvm;
    let reference = read_ref(&vv_dir().join("refs/msfr_transient.ref.txt.gz"));
    let s = setup();
    let mesh = s.mesh.clone();
    let n = mesh.n_cells;
    let t0 = &reference["T_0"];
    let params0: Vec<f64> = t0.iter().flat_map(|&t| [t, rho_cool(t)]).collect();
    let alpha = alpha_cells();
    let keff = reference["START_keff"][0];
    let flux0: Vec<Vec<f64>> = (0..6).map(|g| reference[&format!("START_flux{g}")].clone()).collect();
    let prec_star0: Vec<Vec<f64>> = (0..8).map(|k| reference[&format!("START_precStar{k}")].clone()).collect();
    let base = [("topwall", 0.1), ("bottomwall", 0.1), ("reflector", 0.1), ("hx", 0.5)];
    for (label, scale) in [
        ("none", [1.0, 1.0, 1.0, 1.0]),
        ("top x1.01", [1.01, 1.0, 1.0, 1.0]),
        ("bottom x1.01", [1.0, 1.01, 1.0, 1.0]),
        ("reflector x1.01", [1.0, 1.0, 1.01, 1.0]),
        ("hx x1.01", [1.0, 1.0, 1.0, 1.01]),
        ("hx x0.99", [1.0, 1.0, 1.0, 0.99]),
        ("hx x1.001", [1.0, 1.0, 1.0, 1.001]),
        ("top x0.999", [0.999, 1.0, 1.0, 1.0]),
        ("bottom x0.999", [1.0, 0.999, 1.0, 1.0]),
        ("reflector x0.999", [1.0, 1.0, 0.999, 1.0]),
        ("reflector x0.9999", [1.0, 1.0, 0.9999, 1.0]),
        ("reflector x1.0001", [1.0, 1.0, 1.0001, 1.0]),
        ("all x1.001", [1.001, 1.001, 1.001, 1.001]),
    ] {
        let flux_boundary: Vec<BoundaryCondition<f64>> =
            mesh.patches.iter().map(|_| BoundaryCondition::ZeroGradient).collect();
        let mut solver = DiffusionNeutronics::new_with_cell_parameters(
            mesh.clone(), &s.xs, &s.zone_map, &params0, &flux_boundary, DiffusionSettings::default(),
        )
        .unwrap();
        for (i, (name, gamma)) in base.iter().enumerate() {
            let idx = mesh.patches.iter().position(|p| p.name == *name).unwrap();
            solver.set_albedo_boundary(idx, gamma * scale[i], AlbedoLinearisation::CellValue);
        }
        solver.set_precursor_drift(face_flux(&mesh, &reference["START_U"], &alpha));
        solver.set_initial_guess(&flux0, keff);
        solver.set_prec_star(&prec_star0).unwrap();
        let xs = solver.xs_fields().clone();
        let mut ssrc = vec![0.0; n];
        let mut delayed = vec![0.0; n];
        for g in 0..6 {
            for c in 0..n {
                ssrc[c] += xs.nu_sigma_f[g].internal[c] * flux0[g][c];
            }
        }
        for k in 0..8 {
            for c in 0..n {
                delayed[c] += xs.lambda[k].internal[c] * (prec_star0[k][c] * alpha[c]);
            }
        }
        let mut out = Vec::new();
        for g in 0..3 {
            let mut scatter = vec![0.0; n];
            for j in 0..6 {
                if j != g {
                    for c in 0..n {
                        scatter[c] += xs.scattering[j][g].internal[c] * flux0[j][c];
                    }
                }
            }
            let flux_g = &solver.state().flux()[g];
            let mut rf = flux_g.clone();
            rf.internal = Field::new(
                (0..n)
                    .map(|c| {
                        ((ssrc[c] * (1.0 - xs.beta_tot.internal[c]) * xs.chi_prompt[g].internal[c]) / keff)
                            + delayed[c] * xs.chi_delayed[g].internal[c]
                            + scatter[c]
                    })
                    .collect(),
            );
            let eqn = fvm::laplacian_with_delta(&xs.d_face[g], flux_g, DiffusionSettings::default().delta_coeff)
                + fvm::sp(&xs.sigma_removal[g], flux_g)
                - fvm::su(&rf, flux_g);
            let b: Vec<f64> = eqn.source.iter().copied().collect();
            out.push(eqn.ldu.openfoam_initial_residual(flux_g.internal.as_slice(), &b));
        }
        println!("albedo {label:16}: group residuals {:.3e} {:.3e} {:.3e}", out[0], out[1], out[2]);
    }
}

/// Diagnostic (not a gate): the port's per-cell cross sections at the t = 165 s
/// state against GeN-Foam's own `XS` class (`upstream_driver/dump_xs`, run on the
/// same case at the same time directory), quantity by quantity.
#[test]
#[ignore = "diagnostic: needs the upstream xsdump"]
fn diagnose_cross_sections_against_upstream_xs_class() {
    let dir = PathBuf::from(
        std::env::var("GENFOAM_MSFR_RUN")
            .unwrap_or_else(|_| "/home/user/upstream-build/gh1361/runs/msfr/transient".to_string()),
    )
    .join("xsdump");
    let read = |n: &str| -> Vec<f64> {
        std::fs::read_to_string(dir.join(n))
            .unwrap_or_else(|e| panic!("{n}: {e}"))
            .lines()
            .map(|l| l.trim().parse().unwrap())
            .collect()
    };
    let reference = read_ref(&vv_dir().join("refs/msfr_transient.ref.txt.gz"));
    let s = setup();
    let mesh = s.mesh.clone();
    let t0 = &reference["T_0"];
    let params0: Vec<f64> = t0.iter().flat_map(|&t| [t, rho_cool(t)]).collect();
    let flux_boundary: Vec<BoundaryCondition<f64>> =
        mesh.patches.iter().map(|_| BoundaryCondition::ZeroGradient).collect();
    let solver = DiffusionNeutronics::new_with_cell_parameters(
        mesh.clone(), &s.xs, &s.zone_map, &params0, &flux_boundary, DiffusionSettings::default(),
    )
    .unwrap();
    let xs = solver.xs_fields();
    let cmp = |name: &str, port: &[f64], up: Vec<f64>| {
        let mut worst = 0.0_f64;
        let mut at = 0;
        let mut differ = 0;
        for i in 0..port.len() {
            let r = if up[i] == 0.0 { (port[i] - up[i]).abs() } else { ((port[i] - up[i]) / up[i]).abs() };
            if r > 0.0 {
                differ += 1;
            }
            if r > worst {
                worst = r;
                at = i;
            }
        }
        println!("{name:18} differ {differ:5}/{} worst rel {worst:.3e} at cell {at} zone {} T {:.3} (port {:.17e} up {:.17e})", port.len(), s.zone_map[at], t0[at], port[at], up[at]);
    };
    for g in 0..6 {
        cmp(&format!("D{g}"), xs.d[g].internal.as_slice(), read(&format!("D{g}")));
        cmp(&format!("nuSigmaEff{g}"), xs.nu_sigma_f[g].internal.as_slice(), read(&format!("nuSigmaEff{g}")));
        cmp(&format!("sigmaPow{g}"), xs.sigma_pow[g].internal.as_slice(), read(&format!("sigmaPow{g}")));
        cmp(&format!("sigmaRemoval{g}"), xs.sigma_removal[g].internal.as_slice(), read(&format!("sigmaRemoval{g}")));
        cmp(&format!("chiPrompt{g}"), xs.chi_prompt[g].internal.as_slice(), read(&format!("chiPrompt{g}")));
        cmp(&format!("chiDelayed{g}"), xs.chi_delayed[g].internal.as_slice(), read(&format!("chiDelayed{g}")));
        cmp(&format!("IV{g}"), xs.inv_velocity[g].internal.as_slice(), read(&format!("IV{g}")));
    }
    for j in 0..6 {
        for i in 0..6 {
            cmp(&format!("sigmaFromTo_{j}_{i}"), xs.scattering[j][i].internal.as_slice(), read(&format!("sigmaFromTo_{j}_{i}")));
        }
    }
    cmp("BetaTot", xs.beta_tot.internal.as_slice(), read("BetaTot"));
}
