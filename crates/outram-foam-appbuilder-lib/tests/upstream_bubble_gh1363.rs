//! Two-fluid Euler-Euler port (`outram_foam_multiphase::TwoFluidPimple`, the
//! hydrodynamic core of `ReactingTwoPhaseEulerFoam`) against ESI OpenFOAM-v2506
//! `twoPhaseEulerFoam` on a reduced bubbleColumn, per cell (gh:#1363, row 5).
//!
//! # Method
//! `verification_and_validation/upstream_openfoam_gh1363/bubble` holds the case
//! (a closed 0.15 x 0.30 m tank of 6 x 12 cells, water with an air cloud, drag
//! only, laminar, constant density, all-upwind, fixed dt) and
//! `bubble/reference/bubble_v2506_dt1e-3.txt.gz` /
//! `..._dt1e-4.txt.gz`: text dumps (17 significant digits) of the real ESI
//! solver (`driver/twoPhaseEulerDump`, `twoPhaseEulerFoam.C` with the energy
//! equations skipped and dumps added; the diff is committed) at every step:
//! `Kd`, `rAU`, `HbyA`, `phiHbyA`, `p_rgh`, the phase fluxes, `U`, `alpha`.
//! The port is built from the same polyMesh and the same `alpha.air`.
//!
//! # Results
//! See `verification_and_validation/reacting_two_phase_euler_foam_vs_openfoam.md`
//! (Round 2), where every number below is recorded and interpreted.

use miniz_oxide::inflate::decompress_to_vec;
use outram_foam_appbuilder_lib::io::field_reader::read_vol_scalar_field;
use outram_foam_appbuilder_lib::io::poly_mesh::read_poly_mesh;
use outram_foam_basic_lib::prelude::*;
use outram_foam_multiphase::two_fluid::{DragModel, Phase, TwoFluidSystem};
use outram_foam_multiphase::two_fluid_pimple::TwoFluidPimple;
use outram_foam_multiphase::two_fluid_pimple_esi::TwoFluidPimpleEsi;
use std::collections::BTreeMap;
use std::path::PathBuf;
use uom::si::dynamic_viscosity::pascal_second;
use uom::si::f64::{DynamicViscosity, Length, MassDensity};
use uom::si::length::meter;
use uom::si::mass_density::kilogram_per_cubic_meter;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("verification_and_validation/upstream_openfoam_gh1363/bubble")
}

/// (step, name) -> values, as dumped by the ESI solver.
type Reference = BTreeMap<(usize, String), Vec<f64>>;

fn read_reference(tag: &str) -> Reference {
    let path = root().join(format!("reference/bubble_v2506_{tag}.txt.gz"));
    let bytes = std::fs::read(path).expect("reference");
    let text = String::from_utf8(decompress_to_vec(&bytes[10..bytes.len() - 8]).expect("inflate"))
        .expect("utf8");
    let mut r = Reference::new();
    for line in text.lines() {
        let mut tok = line.split_whitespace();
        let step: usize = tok.next().unwrap().parse().unwrap();
        let name = tok.next().unwrap().to_string();
        r.insert((step, name), tok.map(|v| v.parse::<f64>().unwrap()).collect());
    }
    r
}

/// Port set-up on the committed polyMesh and `0/alpha.air`.
fn setup_port() -> TwoFluidPimple {
    let mesh = read_poly_mesh(&root().join("constant/polyMesh")).expect("mesh");
    let n = mesh.n_cells;
    let alpha = read_vol_scalar_field(&root().join("0/alpha.air"), n).expect("alpha");
    let rho = |v: f64| MassDensity::new::<kilogram_per_cubic_meter>(v);
    let mu = |v: f64| DynamicViscosity::new::<pascal_second>(v);
    let dia = |v: f64| Length::new::<meter>(v);
    let air = Phase::new(mesh.clone(), "air", rho(1.2), mu(1.84e-5), dia(3e-3), 0.0).unwrap();
    let water = Phase::new(mesh.clone(), "water", rho(1000.0), mu(1e-3), dia(1e-4), 0.0).unwrap();
    let mut system = TwoFluidSystem::new(air, water).unwrap();
    for c in 0..n {
        system.dispersed.alpha_mut().internal[c] = alpha[c];
    }
    system.enforce_alpha_constraint();
    // No-slip on the walls only: an `empty` patch must keep its default
    // condition (a fixed-value condition there adds a wall shear on the 2-D
    // front and back faces; the first run of this test did, see the record).
    let kinds: Vec<PatchKind> = mesh.patches.iter().map(|p| p.kind).collect();
    for ph in [&mut system.dispersed, &mut system.continuous] {
        for (pi, pf) in ph.u_mut().boundary.iter_mut().enumerate() {
            if kinds[pi] == PatchKind::Wall {
                *pf = PatchField::fixed_value_vec(pf.values.len(), Vector3::ZERO);
            }
        }
    }
    let mut pimple = TwoFluidPimple::new(system, DragModel::SchillerNaumann);
    pimple.gravity = Vector3::new(0.0, -9.81, 0.0);
    pimple.n_correctors = 1;
    pimple.n_outer_correctors = 1;
    pimple.p_ref_cell = 0;
    pimple.solver_settings = SolverSettings { tolerance: 1e-14, max_iter: 200_000 };
    pimple
}

/// Worst |a - b|, the peak |b|, and the count of bit-identical entries.
fn compare(a: &[f64], b: &[f64]) -> (f64, f64, usize) {
    assert_eq!(a.len(), b.len());
    let mut worst = 0.0_f64;
    let mut peak = 0.0_f64;
    let mut same = 0;
    for (x, y) in a.iter().zip(b) {
        worst = worst.max((x - y).abs());
        peak = peak.max(y.abs());
        if x == y {
            same += 1;
        }
    }
    (worst, peak, same)
}

fn flat(v: &VolVectorField, n: usize) -> Vec<f64> {
    (0..n).flat_map(|c| [v.internal[c].x, v.internal[c].y, v.internal[c].z]).collect()
}

/// ESI-form stepper at pressure level 0 with `pMin` lowered (see the record).
fn new_esi() -> TwoFluidPimpleEsi {
    let mut s = TwoFluidPimpleEsi::new(setup_port(), 0.0).expect("esi");
    s.p_min = -1.0e9;
    s
}

/// Worst differences of one step against the ESI dump:
/// `[alpha (absolute), U.air / peak, U.water / peak, p / peak]`, with `p`
/// compared after removing the value of cell 0 on both sides (the closed
/// tank fixes `p` only up to a constant) and `p_esi = p_rgh + rho*gh`.
fn metrics(step: usize, port: &TwoFluidPimple, reference: &Reference) -> ([f64; 4], String) {
    let n = port.mesh.n_cells;
    let get = |name: &str| &reference[&(step, name.to_string())];
    let alpha: Vec<f64> = (0..n).map(|c| port.system.dispersed.alpha().internal[c]).collect();
    let ua = flat(port.system.dispersed.u(), n);
    let uw = flat(port.system.continuous.u(), n);
    let p_port: Vec<f64> = (0..n).map(|c| port.p.internal[c] - port.p.internal[0]).collect();
    let p_esi: Vec<f64> = {
        let (prgh, rho, gh) = (get("p_rgh"), get("rho"), get("gh"));
        (0..n).map(|c| (prgh[c] + rho[c] * gh[c]) - (prgh[0] + rho[0] * gh[0])).collect()
    };
    let (da, pa, sa) = compare(&alpha, get("alpha1"));
    let (dua, pua, sua) = compare(&ua, get("U1"));
    let (duw, puw, suw) = compare(&uw, get("U2"));
    let (dp, pp, sp) = compare(&p_port, &p_esi);
    let text = format!(
        "step {step:2}: alpha {da:.3e}/{pa:.3} [{sa}] | U.air {dua:.3e}/{pua:.3e} [{sua}] | U.water {duw:.3e}/{puw:.3e} [{suw}] | p {dp:.3e}/{pp:.3e} [{sp}]"
    );
    ([da, dua / pua, duw / puw, dp / pp], text)
}

/// Per-step metrics of the dev-form `solve_timestep`.
fn run_dev(tag: &str, dt: f64, steps: usize) -> Vec<([f64; 4], String)> {
    let reference = read_reference(tag);
    let mut port = setup_port();
    (1..=steps)
        .map(|step| {
            port.solve_timestep(dt).expect("step");
            metrics(step, &port, &reference)
        })
        .collect()
}

/// Per-step metrics of the ESI-form `solve_timestep_esi`, after `configure`.
fn run_esi(
    tag: &str,
    dt: f64,
    steps: usize,
    configure: impl Fn(&mut TwoFluidPimpleEsi),
) -> Vec<([f64; 4], String)> {
    let reference = read_reference(tag);
    let mut port = new_esi();
    configure(&mut port);
    (1..=steps)
        .map(|step| {
            port.solve_timestep_esi(dt).expect("step");
            metrics(step, &port.core, &reference)
        })
        .collect()
}

fn show(label: &str, rows: &[([f64; 4], String)]) {
    for (_, text) in rows {
        println!("[{label}] {text}");
    }
}

/// The ESI-form step reproduces ESI v2506 `twoPhaseEulerFoam` to round-off
/// accumulation for 10 steps at dt = 1e-3 (pinned at the P8 prediction
/// bounds, not at the measured values).
#[test]
fn esi_form_matches_esi_bubble_column_dt1e3() {
    let rows = run_esi("dt1e-3", 1e-3, 10, |_| {});
    show("esi-form dt1e-3", &rows);
    for (step, (m, _)) in rows.iter().enumerate() {
        assert!(m[0] < 1e-12, "step {}: alpha {:e}", step + 1, m[0]);
        assert!(m[1] < 1e-10 && m[2] < 1e-10, "step {}: U {:e} {:e}", step + 1, m[1], m[2]);
        assert!(m[3] < 1e-10, "step {}: p {:e}", step + 1, m[3]);
    }
}

#[test]
fn esi_form_matches_esi_bubble_column_dt1e4() {
    let rows = run_esi("dt1e-4", 1e-4, 10, |_| {});
    show("esi-form dt1e-4", &rows);
    for (step, (m, _)) in rows.iter().enumerate() {
        assert!(m[0] < 1e-12, "step {}: alpha {:e}", step + 1, m[0]);
        assert!(m[1] < 1e-10 && m[2] < 1e-10, "step {}: U {:e} {:e}", step + 1, m[1], m[2]);
        assert!(m[3] < 1e-10, "step {}: p {:e}", step + 1, m[3]);
    }
}

/// Ablations: each translated piece carries part of the agreement. Pinned as
/// lower bounds on the loss of agreement at step 2 (dt = 1e-3), measured:
/// without the MULES limiter alpha is off by 3.2e-4 (first-row cells lose air
/// through the wall-side relative flux); without the explicit stress term
/// `H` is off by 1.4e-5 (relative).
#[test]
fn esi_form_ablations_lose_the_agreement() {
    let no_limiter = run_esi("dt1e-3", 1e-3, 2, |s| s.mules_limiter = false);
    show("no limiter dt1e-3", &no_limiter);
    assert!(no_limiter[1].0[0] > 1e-5, "alpha {:e}", no_limiter[1].0[0]);
    let no_stress = run_esi("dt1e-3", 1e-3, 2, |s| s.explicit_stress = false);
    show("no explicit stress dt1e-3", &no_stress);
    assert!(no_stress[1].0[1] > 1e-9, "U.air {:e}", no_stress[1].0[1]);
}

/// Intermediates of the ESI-form step against the ESI dump, in the order the
/// ESI solver computes them: worst |diff| / peak for each, asserted.
fn trace_check(tag: &str, dt: f64, step: usize, bound: f64) -> Vec<String> {
    let reference = read_reference(tag);
    let mut port = new_esi();
    port.record_trace = true;
    for _ in 0..step {
        port.solve_timestep_esi(dt).expect("step");
    }
    let mut lines = Vec::new();
    for (name, vals) in &port.trace {
        let r = &reference[&(step, name.clone())];
        let (mine, theirs): (Vec<f64>, Vec<f64>) = if name == "p_rgh" {
            (vals.iter().map(|v| v - vals[0]).collect(), r.iter().map(|v| v - r[0]).collect())
        } else {
            (vals.clone(), r.clone())
        };
        let (w, pk, same) = compare(&mine, &theirs);
        let rel = w / pk.max(1e-300);
        lines.push(format!("{name:12} worst {w:.3e}  peak {pk:.3e}  rel {rel:.2e}  identical {same}/{}", vals.len()));
        assert!(rel < bound, "{name}: {rel:e}");
    }
    lines
}

#[test]
fn esi_form_intermediates_match_step1_and_step2() {
    for l in trace_check("dt1e-3", 1e-3, 1, 1e-10) {
        println!("[trace step1] {l}");
    }
    for l in trace_check("dt1e-3", 1e-3, 2, 1e-10) {
        println!("[trace step2] {l}");
    }
}

/// The dev-form `solve_timestep` is NOT the ESI algorithm (see the record):
/// pinned as a gap, so that the day it is made to agree this test fails and
/// the record is updated.
#[test]
fn dev_form_differs_from_esi_bubble_column() {
    let rows = run_dev("dt1e-3", 1e-3, 10);
    show("dev-form dt1e-3", &rows);
    show("dev-form dt1e-4", &run_dev("dt1e-4", 1e-4, 10));
    assert!(rows[0].0[1] > 0.2, "step 1 U.air {:e}", rows[0].0[1]);
    assert!(rows[9].0[2] > 0.3, "step 10 U.water {:e}", rows[9].0[2]);
    assert!(rows[9].0[0] > 1e-3, "step 10 alpha {:e}", rows[9].0[0]);
}

/// Refusal by name: a patch that is not Wall or Empty is not translated.
#[test]
fn esi_form_refuses_unsupported_patches() {
    let mut core = setup_port();
    // an inlet-like patch kind
    let mut mesh_patches = core.mesh.patches.clone();
    mesh_patches[0].kind = PatchKind::Patch;
    let mut mesh = (*core.mesh).clone();
    mesh.patches = mesh_patches;
    core.mesh = std::sync::Arc::new(mesh);
    assert!(TwoFluidPimpleEsi::new(core, 0.0).is_err());
}

/// Constant virtual-mass coefficient (`Cvm = 0.5`, the tutorial value): the
/// ESI-form step with `virtual_mass_coeff` against the same case run with
/// `virtualMass ( (air in water) { type constantCoefficient; Cvm 0.5; } ... )`
/// (`CVM=1 scripts/run_bubble.sh`). The dev form has no virtual mass.
#[test]
fn esi_form_virtual_mass_matches_esi_bubble_column() {
    let rows = run_esi("dt1e-3_vm", 1e-3, 10, |s| s.virtual_mass_coeff = Some(0.5));
    show("esi-form Cvm 0.5 dt1e-3", &rows);
    for (step, (m, _)) in rows.iter().enumerate() {
        assert!(m[0] < 1e-12, "step {}: alpha {:e}", step + 1, m[0]);
        assert!(m[1] < 1e-10 && m[2] < 1e-10, "step {}: U {:e} {:e}", step + 1, m[1], m[2]);
        assert!(m[3] < 1e-10, "step {}: p {:e}", step + 1, m[3]);
    }
    // ablation: the same reference with the term switched off must disagree
    let off = run_esi("dt1e-3_vm", 1e-3, 1, |_| {});
    show("esi-form Cvm 0 vs Cvm 0.5 reference", &off);
    assert!(off[0].0[1] > 0.05, "U.air without Vm {:e}", off[0].0[1]);
}
