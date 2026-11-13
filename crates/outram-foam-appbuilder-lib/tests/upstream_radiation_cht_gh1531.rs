//! `ChtMultiRegionFoam` with opt-in P1 radiation in the fluid, against upstream
//! OpenFOAM-v2506 `chtMultiRegionFoam` with `radiationModel P1` (gh:#1531, epic
//! gh:#1379).
//!
//! # Method
//! The gh:#1363 two-region case (meshes in
//! `verification_and_validation/upstream_openfoam_gh1363/cht`) with P1 radiation on
//! in the fluid (a = e = 50 /m, wall emissivities 0.9 / 0.8 / 0.5, `G` starting at 0),
//! run by `upstream_radiation_gh1531/driver/chtRadDump.C` (the gh:#1363 driver, whose
//! transcribed `EEqn.H` already holds `rad.Sh` and `rad.correct()`, plus dumps of `G`,
//! `qr` and `G` on the patches) into `reference/cht_rad_v2506.txt.gz`: bit patterns per
//! step, and the PCG iteration count of each `G` solve. The port replays those counts
//! with tolerance 0 so the comparison is of arithmetic, not of the stopping rule.
//! Predictions: `upstream_radiation_gh1531/PREDICTIONS.md`; results: `README.md` there.

use miniz_oxide::inflate::decompress_to_vec;
use outram_foam_appbuilder_lib::io::control_dict::ControlDict;
use outram_foam_appbuilder_lib::io::fv_schemes::FvSchemes;
use outram_foam_appbuilder_lib::io::fv_solution::FvSolution;
use outram_foam_appbuilder_lib::io::poly_mesh::read_poly_mesh;
use outram_foam_appbuilder_lib::solvers::cht_multi_region_foam::{ChtMultiRegionFoam, CoupledTemperature, RegionId, SolidRegion};
use outram_foam_appbuilder_lib::solvers::energy::{EnergyEquation, IncompressibleThermo};
use outram_foam_appbuilder_lib::solvers::pimple_foam::PimpleFoam;
use outram_foam_basic_lib::prelude::*;
use outram_foam_basic_lib::radiation::{ConstantAbsorptionEmission, P1Model, P1PatchModel};
use std::collections::BTreeMap;
use std::path::PathBuf;

const DT: f64 = 0.1;
const N_STEPS: usize = 50;
const A: f64 = 50.0;

fn case_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("verification_and_validation/upstream_openfoam_gh1363/cht")
}

fn rad_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("verification_and_validation/upstream_radiation_gh1531")
}

type Fields = BTreeMap<String, Vec<f64>>;

struct Reference {
    steps: BTreeMap<usize, Fields>,
    scalars: BTreeMap<usize, BTreeMap<String, f64>>,
    /// step -> PCG iterations of the G solve
    g_iterations: BTreeMap<usize, usize>,
}

fn read_reference(file: &str) -> Reference {
    let bytes = std::fs::read(rad_root().join("reference").join(file)).expect("reference");
    let text = String::from_utf8(decompress_to_vec(&bytes[10..bytes.len() - 8]).expect("inflate")).expect("utf8");
    let mut r = Reference { steps: BTreeMap::new(), scalars: BTreeMap::new(), g_iterations: BTreeMap::new() };
    let bits = |h: &str| f64::from_bits(u64::from_str_radix(h, 16).unwrap());
    for line in text.lines() {
        let tok: Vec<&str> = line.split_whitespace().collect();
        match tok[0] {
            "S" => {
                let step: usize = tok[1].parse().unwrap();
                r.steps.entry(step).or_default().entry(format!("{} {}", tok[2], tok[3])).or_default().push(bits(tok[5]));
            }
            "C" => {
                r.scalars.entry(tok[1].parse().unwrap()).or_default().insert(tok[2].to_string(), bits(tok[3]));
            }
            "I" => {
                r.g_iterations.insert(tok[1].parse().unwrap(), tok[2].parse().unwrap());
            }
            _ => {}
        }
    }
    r
}

fn patch_index(mesh: &FvMesh, name: &str) -> usize {
    mesh.patches.iter().position(|p| p.name == name).unwrap_or_else(|| panic!("no patch {name}"))
}

/// The P1 model of the case: constant a = e, Marshak walls with `boundaryRadiationProperties`
/// emissivities, `frontAndBack` empty.
fn p1_model(mesh: &FvMesh, a: f64) -> P1Model {
    let patches = mesh
        .patches
        .iter()
        .map(|p| match p.name.as_str() {
            "left" => P1PatchModel::Marshak { emissivity: 0.9 },
            "interface" => P1PatchModel::Marshak { emissivity: 0.8 },
            "walls" => P1PatchModel::Marshak { emissivity: 0.5 },
            _ => P1PatchModel::ZeroGradient,
        })
        .collect();
    P1Model::new(ConstantAbsorptionEmission { absorptivity: a, emissivity: a, emission_source: 0.0 }, patches)
}

fn build_port(reference: &Reference, radiation: bool, solver_freq: usize) -> ChtMultiRegionFoam {
    let k = &reference.scalars[&0];
    let mf = read_poly_mesh(&case_root().join("constant/fluid/polyMesh")).expect("fluid polyMesh");
    let ms = read_poly_mesh(&case_root().join("constant/solid/polyMesh")).expect("solid polyMesh");
    let control = ControlDict { delta_t: DT, ..ControlDict::default() };
    let mut sol = FvSolution::default();
    sol.pimple.frozen_flow = true;
    let mut f = PimpleFoam::new(mf.clone(), control, FvSchemes::default(), sol);
    let mut e = EnergyEquation::uniform(mf.clone(), IncompressibleThermo::si(k["fluid_rho"], k["fluid_Cp"], k["fluid_kappa"]), 300.0);
    e.solver = SolverSettings { tolerance: 1e-16, max_iter: 10_000 };
    e.set_patch("left", BoundaryCondition::FixedValue(300.0));
    if radiation {
        e = e.with_radiation(p1_model(&mf, A), 0.0).expect("radiation");
        e.radiation.as_mut().unwrap().solver_freq = solver_freq;
    }
    f.energy = Some(e);
    let mut s = SolidRegion::new("solid", ms.clone(), IncompressibleThermo::si(k["solid_rho"], k["solid_Cp"], k["solid_kappa"]), 300.0);
    s.energy.solver = SolverSettings { tolerance: 1e-16, max_iter: 10_000 };
    s.energy.set_patch("right", BoundaryCondition::FixedValue(360.0));
    let (pf, ps) = (patch_index(&mf, "interface"), patch_index(&ms, "interface"));
    let c = CoupledTemperature::new(RegionId::Fluid(0), mf, pf, RegionId::Solid(0), ms, ps);
    let mut cht = ChtMultiRegionFoam::new(vec![f], vec![s], vec![c]).expect("port");
    cht.solid_delta_t = uom::si::f64::Time::new::<uom::si::time::second>(DT);
    cht.energy_outer_correctors = 1;
    cht
}

fn worst_abs(port: &[f64], up: &[f64]) -> (f64, usize) {
    assert_eq!(port.len(), up.len(), "length");
    let mut w = 0.0_f64;
    let mut same = 0;
    for (a, b) in port.iter().zip(up) {
        w = w.max((a - b).abs());
        if a.to_bits() == b.to_bits() {
            same += 1;
        }
    }
    (w, same)
}

#[derive(Debug, Default, Clone, Copy)]
struct Worst {
    t_fluid: f64,
    t_solid: f64,
    g_rel: f64,
    qr_rel: f64,
    gb_rel: f64,
    /// step of the first G difference above 1e-12 relative, 0 if none
    first_g_step: usize,
    /// mean fluid dT at step 1
    mean_dt_step1: f64,
}

/// Run `steps` steps, replaying ESI's G iteration counts, and compare with the reference.
fn run(cht: &mut ChtMultiRegionFoam, reference: &Reference, steps: usize, verbose: bool) -> Worst {
    let mut w = Worst::default();
    for step in 1..=steps {
        // ESI solves G only on some steps with solverFreq > 1; replay its count where it solved.
        if let (Some(r), Some(n)) = (cht.fluids[0].energy.as_mut().unwrap().radiation.as_mut(), reference.g_iterations.get(&step)) {
            r.settings = SolverSettings { tolerance: 0.0, max_iter: *n };
        }
        cht.step().expect("step");
        let up = &reference.steps[&step];
        let e = cht.fluids[0].energy.as_ref().unwrap();
        let (tf, _) = worst_abs(e.t.internal.as_slice(), &up["f T"]);
        let (ts, _) = worst_abs(cht.solids[0].energy.t.internal.as_slice(), &up["s T"]);
        w.t_fluid = w.t_fluid.max(tf);
        w.t_solid = w.t_solid.max(ts);
        if step == 1 {
            let n = e.t.internal.len() as f64;
            w.mean_dt_step1 = e.t.internal.as_slice().iter().sum::<f64>() / n - 300.0;
        }
        let mut gr = 0.0;
        let mut qrr = 0.0_f64;
        let mut gbr = 0.0_f64;
        if let Some(r) = e.radiation.as_ref() {
            let g = r.state.g.internal.as_slice();
            let gmax = up["f G"].iter().fold(0.0_f64, |m, v| m.max(v.abs()));
            gr = worst_abs(g, &up["f G"]).0 / gmax;
            for name in ["left", "interface", "walls"] {
                let pi = patch_index(&e.t.mesh, name);
                let qmax = up[&format!("f qr_{name}")].iter().fold(0.0_f64, |m, v| m.max(v.abs())).max(1e-300);
                qrr = qrr.max(worst_abs(r.state.qr[pi].as_slice(), &up[&format!("f qr_{name}")]).0 / qmax);
                gbr = gbr.max(worst_abs(r.state.g.boundary[pi].values.as_slice(), &up[&format!("f Gb_{name}")]).0 / gmax);
            }
            w.g_rel = w.g_rel.max(gr);
            w.qr_rel = w.qr_rel.max(qrr);
            w.gb_rel = w.gb_rel.max(gbr);
            if w.first_g_step == 0 && gr > 1e-12 {
                w.first_g_step = step;
            }
        }
        if verbose {
            eprintln!("step {step:2}: T fluid {tf:.2e} solid {ts:.2e}  G rel {gr:.2e} qr rel {qrr:.2e} Gb rel {gbr:.2e}");
        }
    }
    w
}

fn verbose() -> bool {
    std::env::var("RAD_VERBOSE").is_ok()
}

/// The coupled case: P1 in the fluid, 50 steps, against ESI.
#[test]
fn coupled_p1_matches_upstream_chtmultiregionfoam() {
    let reference = read_reference("cht_rad_v2506.txt.gz");
    let mut cht = build_port(&reference, true, 1);
    let w = run(&mut cht, &reference, N_STEPS, verbose());
    eprintln!("cht+P1 vs ESI v2506: {w:?}");
    // Prediction 1: the mean step-1 fluid dT is -2.197e-3 K within 2 % (derived before the run).
    assert!((w.mean_dt_step1 / -2.197e-3 - 1.0).abs() < 0.02, "{w:?}");
    assert!(w.t_fluid < 5e-11, "{w:?}");
    assert!(w.t_solid < 2e-11, "{w:?}");
    assert!(w.g_rel < 1e-9, "{w:?}");
    assert!(w.gb_rel < 1e-9, "{w:?}");
    assert!(w.qr_rel < 1e-8, "{w:?}");
}

/// Ablation (prediction 2): the same port without radiation differs from ESI-with-radiation by
/// more than the 1e-3 K that radiation moves T by, and by less than the 0.12 K bound.
#[test]
fn radiation_off_differs_from_radiation_on_by_the_radiation() {
    let reference = read_reference("cht_rad_v2506.txt.gz");
    let mut cht = build_port(&reference, false, 1);
    assert!(cht.fluids[0].energy.as_ref().unwrap().radiation.is_none());
    let mut worst = 0.0_f64;
    for step in 1..=N_STEPS {
        cht.step().expect("step");
        let e = cht.fluids[0].energy.as_ref().unwrap();
        worst = worst.max(worst_abs(e.t.internal.as_slice(), &reference.steps[&step]["f T"]).0);
    }
    eprintln!("no-radiation port vs ESI with radiation: worst fluid T difference {worst:.4e} K");
    assert!(worst > 1e-3 && worst < 0.12, "{worst}");
}

/// `solverFreq 3` (ESI re-run with it): G is solved on steps 1, 3, 6, 9, ... and held in between;
/// the port's `begin_time_step` / `correct` rule gives the same fields.
#[test]
fn solver_freq_three_matches_upstream() {
    let reference = read_reference("cht_rad_f3_v2506.txt.gz");
    assert_eq!(reference.g_iterations.keys().take(4).copied().collect::<Vec<_>>(), vec![1, 3, 6, 9]);
    let mut cht = build_port(&reference, true, 3);
    let w = run(&mut cht, &reference, N_STEPS, verbose());
    eprintln!("cht+P1 solverFreq 3 vs ESI v2506: {w:?}");
    assert!(w.t_fluid < 5e-11 && w.t_solid < 2e-11, "{w:?}");
    assert!(w.g_rel < 1e-9 && w.gb_rel < 1e-9 && w.qr_rel < 1e-8, "{w:?}");
}
