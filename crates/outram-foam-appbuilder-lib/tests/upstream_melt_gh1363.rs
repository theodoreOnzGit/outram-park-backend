//! `MeltFoam` + `SolidificationMelting` against ESI OpenFOAM-v2506's
//! `solidificationMeltingSource` fvOption inside a solid's energy equation,
//! per cell (gh:#1363, row 6).
//!
//! # Method
//! `verification_and_validation/upstream_openfoam_gh1363/melt` holds a 1-D
//! gallium-like slab (40 cells over 20 mm, hot wall 311 K, start 301 K,
//! `Tmelt` 302.78 K, `L` 80160 J/kg, fixed dt = 0.5 s, 200 steps) and
//! `melt/reference/melt_v2506.txt.gz`: the IEEE-754 bit patterns written by
//! `driver/meltDump.C` (`solid/solveSolid.H` of chtMultiRegionFoam on the
//! unmodified ESI libraries, the real option; see
//! `verification_and_validation/melt_foam_vs_openfoam.md`): cell T, h and the
//! option's liquid fraction at every step. The port gets
//! `solidus = liquidus = Tmelt` (ESI's single melting point), no gravity.
//!
//! # Results
//! See `verification_and_validation/melt_foam_vs_openfoam.md`.

use miniz_oxide::inflate::decompress_to_vec;
use outram_foam_appbuilder_lib::io::control_dict::ControlDict;
use outram_foam_appbuilder_lib::io::fv_schemes::FvSchemes;
use outram_foam_appbuilder_lib::io::fv_solution::FvSolution;
use outram_foam_appbuilder_lib::io::poly_mesh::read_poly_mesh;
use outram_foam_appbuilder_lib::solvers::melt_foam::MeltFoam;
use outram_foam_basic_lib::prelude::*;
use std::collections::BTreeMap;
use std::path::PathBuf;

const DT: f64 = 0.5;
const N_STEPS: usize = 200;
const T_MELT: f64 = 302.78;
const LATENT: f64 = 80160.0;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("verification_and_validation/upstream_openfoam_gh1363/melt")
}

type Fields = BTreeMap<String, Vec<f64>>;

struct Reference {
    steps: BTreeMap<usize, Fields>,
    constants: BTreeMap<String, f64>,
}

fn read_reference() -> Reference {
    let bytes = std::fs::read(root().join("reference/melt_v2506.txt.gz")).expect("reference");
    let text = String::from_utf8(decompress_to_vec(&bytes[10..bytes.len() - 8]).expect("inflate")).expect("utf8");
    let mut r = Reference { steps: BTreeMap::new(), constants: BTreeMap::new() };
    let bits = |h: &str| f64::from_bits(u64::from_str_radix(h, 16).unwrap());
    for line in text.lines() {
        let tok: Vec<&str> = line.split_whitespace().collect();
        match tok[0] {
            "S" => r.steps.entry(tok[1].parse().unwrap()).or_default().entry(tok[3].to_string()).or_default().push(bits(tok[5])),
            "C" => {
                r.constants.insert(tok[2].to_string(), bits(tok[3]));
            }
            _ => {}
        }
    }
    r
}

fn build_port(reference: &Reference) -> MeltFoam {
    let mesh = read_poly_mesh(&root().join("constant/polyMesh")).expect("polyMesh");
    let k = &reference.constants;
    let control = ControlDict { delta_t: DT, ..ControlDict::default() };
    let mut s = MeltFoam::new(mesh.clone(), control, FvSchemes::default(), FvSolution::default());
    s.t = VolScalarField::uniform("T", mesh.clone(), 301.0);
    let hot = mesh.patches.iter().position(|p| p.name == "hot").expect("hot");
    let far = mesh.patches.iter().position(|p| p.name == "far").expect("far");
    s.t.boundary[hot].bc = BoundaryCondition::FixedValue(311.0);
    for v in s.t.boundary[hot].values.iter_mut() {
        *v = 311.0;
    }
    s.t.boundary[far].bc = BoundaryCondition::ZeroGradient;
    s.alpha_thermal = VolScalarField::uniform("alphat", mesh.clone(), k["kappa"] / (k["rho"] * k["Cp"]));
    s.temperature_solver = SolverSettings { tolerance: 1e-16, max_iter: 10_000 };
    let coeffs = MeltFoam::boussinesq_coefficients(T_MELT, T_MELT, LATENT, k["Cp"], k["rho"], 0.0, 1.0e5);
    s.fv_models.push(FvModel::SolidificationMelting(SolidificationMelting::new(
        "melt",
        "U",
        "T",
        true,
        CellSelection::All,
        coeffs,
        Vector3::new(0.0, 0.0, 0.0),
        mesh.n_cells,
    )));
    s
}

#[derive(Debug, Default, Clone, Copy)]
struct Worst {
    t: f64,
    alpha: f64,
    identical: usize,
    total: usize,
    class_mismatch_steps: usize,
}

fn run(steps: usize) -> Worst {
    let reference = read_reference();
    let mut s = build_port(&reference);
    let mut w = Worst::default();
    for step in 1..=steps {
        s.step().expect("step");
        let up = &reference.steps[&step];
        let t = s.t.internal.as_slice();
        let a = s.liquid_fraction().expect("model").to_vec();
        for (p, u) in t.iter().zip(&up["T"]).chain(a.iter().zip(&up["alpha1"])) {
            w.total += 1;
            if p.to_bits() == u.to_bits() {
                w.identical += 1;
            }
        }
        if std::env::var("MELT_VERBOSE").is_ok() && step <= 4 {
            eprintln!("step {step} T port {:?} up {:?}", &t[..3], &up["T"][..3]);
            eprintln!("step {step} a port {:?} up {:?}", &a[..3], &up["alpha1"][..3]);
        }
        w.t = w.t.max(t.iter().zip(&up["T"]).fold(0.0_f64, |m, (p, u)| m.max((p - u).abs())));
        w.alpha = w.alpha.max(a.iter().zip(&up["alpha1"]).fold(0.0_f64, |m, (p, u)| m.max((p - u).abs())));
        let cls = |v: &[f64]| (v.iter().filter(|x| **x == 0.0).count(), v.iter().filter(|x| **x == 1.0).count());
        if cls(&a) != cls(&up["alpha1"]) {
            w.class_mismatch_steps += 1;
        }
    }
    w
}

#[test]
fn melting_slab_matches_upstream_solidification_melting_source() {
    let w = run(N_STEPS);
    eprintln!("melt vs upstream: {w:?}");
    // Pinned at about twice the measured worst (3.4e-11 K, 2.0e-12): the
    // prediction for alpha (1e-12) was missed by 2x, see the record.
    assert!(w.t < 7e-11, "{w:?}");
    assert!(w.alpha < 5e-12, "{w:?}");
    assert_eq!(w.class_mismatch_steps, 0, "{w:?}");
}
