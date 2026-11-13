//! `ChtMultiRegionFoam` against upstream OpenFOAM-v2506 `chtMultiRegionFoam` on
//! a two-region case (frozen-flow fluid at rest + solid), per cell (gh:#1363,
//! row 4).
//!
//! # Method
//! `verification_and_validation/upstream_openfoam_gh1363/cht` holds the case
//! (ESI blockMesh polyMeshes of a 20x5x1 water region and a 10x5x1 solid region
//! joined by `mappedWall` patches, the `bottomWater` / `heater` properties of the
//! `multiRegionHeater` tutorial, fixed dt = 0.1 s, 50 steps) and
//! `cht/reference/cht_v2506.txt.gz`: the IEEE-754 bit patterns written by
//! `driver/chtDump.C` (`chtMultiRegionFoam.C` and its `.H` files on the
//! unmodified ESI libraries and the real
//! `compressible::turbulentTemperatureRadCoupledMixed`; see
//! `verification_and_validation/cht_multi_region_foam_vs_openfoam.md` for the
//! substitutions): cell T and h of both regions, the interface face
//! temperatures, the heat into each region through the interface, and upstream's
//! own kappa / Cp / rho, at every step. The port is built from the same meshes
//! with those constants.
//!
//! # Results
//! See `verification_and_validation/cht_multi_region_foam_vs_openfoam.md`.

use miniz_oxide::inflate::decompress_to_vec;
use outram_foam_appbuilder_lib::io::control_dict::ControlDict;
use outram_foam_appbuilder_lib::io::fv_schemes::FvSchemes;
use outram_foam_appbuilder_lib::io::fv_solution::FvSolution;
use outram_foam_appbuilder_lib::io::poly_mesh::read_poly_mesh;
use outram_foam_appbuilder_lib::solvers::cht_multi_region_foam::{ChtMultiRegionFoam, CoupledTemperature, RegionId, SolidRegion};
use outram_foam_appbuilder_lib::solvers::energy::{EnergyEquation, IncompressibleThermo};
use outram_foam_appbuilder_lib::solvers::pimple_foam::PimpleFoam;
use outram_foam_basic_lib::prelude::*;
use std::collections::BTreeMap;
use std::path::PathBuf;
use uom::si::power::watt;
use uom::si::f64::TemperatureInterval;
use uom::si::temperature_interval::kelvin as kelvin_interval;

const DT: f64 = 0.1;
const N_STEPS: usize = 50;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("verification_and_validation/upstream_openfoam_gh1363/cht")
}

type Fields = BTreeMap<String, Vec<f64>>;

struct Reference {
    /// step -> "f T", "s T", "f Tf", ... -> values
    steps: BTreeMap<usize, Fields>,
    /// step -> scalar name -> value
    scalars: BTreeMap<usize, BTreeMap<String, f64>>,
}

fn read_reference() -> Reference {
    let bytes = std::fs::read(root().join("reference/cht_v2506.txt.gz")).expect("reference");
    let text = String::from_utf8(decompress_to_vec(&bytes[10..bytes.len() - 8]).expect("inflate")).expect("utf8");
    let mut r = Reference { steps: BTreeMap::new(), scalars: BTreeMap::new() };
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
            _ => {}
        }
    }
    r
}

#[derive(Debug, Clone, Copy)]
struct Cmp {
    identical: usize,
    n: usize,
    worst_abs: f64,
}

fn compare(port: &[f64], up: &[f64]) -> Cmp {
    assert_eq!(port.len(), up.len(), "length");
    let mut c = Cmp { identical: 0, n: port.len(), worst_abs: 0.0 };
    for (a, b) in port.iter().zip(up) {
        if a.to_bits() == b.to_bits() {
            c.identical += 1;
        }
        c.worst_abs = c.worst_abs.max((a - b).abs());
    }
    c
}

fn patch_index(mesh: &FvMesh, name: &str) -> usize {
    mesh.patches.iter().position(|p| p.name == name).unwrap_or_else(|| panic!("no patch {name}"))
}

/// The port, built from the case's meshes and upstream's own constants.
fn build_port(reference: &Reference) -> ChtMultiRegionFoam {
    let k = &reference.scalars[&0];
    let mf = read_poly_mesh(&root().join("constant/fluid/polyMesh")).expect("fluid polyMesh");
    let ms = read_poly_mesh(&root().join("constant/solid/polyMesh")).expect("solid polyMesh");
    let control = ControlDict { delta_t: DT, ..ControlDict::default() };
    let mut sol = FvSolution::default();
    sol.pimple.frozen_flow = true;
    let mut f = PimpleFoam::new(mf.clone(), control, FvSchemes::default(), sol);
    let mut e = EnergyEquation::uniform(mf.clone(), IncompressibleThermo::si(k["fluid_rho"], k["fluid_Cp"], k["fluid_kappa"]), 300.0);
    e.solver = SolverSettings { tolerance: 1e-16, max_iter: 10_000 };
    e.set_patch("left", BoundaryCondition::FixedValue(300.0));
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

/// Per-step worst differences of the run, over all fields.
#[derive(Debug, Default, Clone, Copy)]
struct Worst {
    t_cells: f64,
    t_faces: f64,
    q_rel: f64,
    identical: usize,
    total: usize,
}

fn run(cht: &mut ChtMultiRegionFoam, reference: &Reference, steps: usize) -> Worst {
    let mut w = Worst::default();
    for step in 1..=steps {
        cht.step().expect("step");
        let up = &reference.steps[&step];
        let sc = &reference.scalars[&step];
        let tf = cht.fluids[0].energy.as_ref().unwrap();
        let cells = [("f T", tf.t.internal.as_slice().to_vec()), ("s T", cht.solids[0].energy.t.internal.as_slice().to_vec())];
        for (name, port) in &cells {
            let c = compare(port, &up[*name]);
            if std::env::var("CHT_VERBOSE").is_ok() {
                eprintln!("step {step} {name}: worst {:.3e} identical {}/{}", c.worst_abs, c.identical, c.n);
            }
            w.t_cells = w.t_cells.max(c.worst_abs);
            w.identical += c.identical;
            w.total += c.n;
        }
        let pf = cht.interfaces[0].pairing.patch_a;
        let ps = cht.interfaces[0].pairing.patch_b;
        let faces = [("f Tf", tf.t.boundary[pf].values.as_slice().to_vec()), ("s Tf", cht.solids[0].energy.t.boundary[ps].values.as_slice().to_vec())];
        for (name, port) in &faces {
            let c = compare(port, &up[*name]);
            w.t_faces = w.t_faces.max(c.worst_abs);
        }
        let qf = cht.interface_heat_in(cht.interfaces[0].a).get::<watt>();
        let qs = cht.interface_heat_in(cht.interfaces[0].b).get::<watt>();
        if step == 1 && cht.energy_outer_correctors == 1 {
            // The fluid is solved first against the solid's initial 300 K: no
            // flux. Upstream's is exactly 0; the port's is 6e-32 W of round-off
            // (the prediction said "exactly 0"; recorded as missed by that).
            assert_eq!(sc["fluid_Qin"], 0.0);
            assert!(qf.abs() < 1e-30, "{qf}");
        }
        for (port, name) in [(qf, "fluid_Qin"), (qs, "solid_Qin")] {
            let want = sc[name];
            let scale = sc["fluid_Qin"].abs().max(sc["solid_Qin"].abs());
            if scale > 0.0 {
                w.q_rel = w.q_rel.max((port - want).abs() / scale);
            } else {
                assert_eq!(port, 0.0, "step {step} {name}");
            }
        }
    }
    w
}

#[test]
fn two_region_cht_matches_upstream_chtmultiregionfoam() {
    let reference = read_reference();
    let mut cht = build_port(&reference);
    let w = run(&mut cht, &reference, N_STEPS);
    eprintln!("cht vs upstream: {w:?}");
    // Pinned at about twice the measured worst (1.05e-11 K cells, 2.7e-11 K
    // faces, 7.7e-13 of the flux): the floor is the rounding of the T-form
    // primary variable against upstream's h-form, see the record. The
    // prediction (1e-12 K) was missed by 10x and is recorded as such.
    assert!(w.t_cells < 2e-11, "{w:?}");
    assert!(w.t_faces < 5e-11, "{w:?}");
    assert!(w.q_rel < 2e-12, "{w:?}");
}

/// Prediction 4: thirty converged outer passes are not upstream's single pass.
#[test]
fn converged_outer_passes_differ_from_one_upstream_pass() {
    let reference = read_reference();
    let mut cht = build_port(&reference);
    cht.energy_outer_correctors = 30;
    cht.energy_outer_tolerance = TemperatureInterval::new::<kelvin_interval>(0.0);
    let w = run(&mut cht, &reference, 5);
    eprintln!("30 passes vs one upstream pass, steps 1-5: {w:?}");
    assert!(w.t_cells > 1e-4, "{w:?}");
}
