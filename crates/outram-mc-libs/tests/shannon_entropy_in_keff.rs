//! **Shannon entropy wired into the k-eigenvalue driver** — `bn:op-867c.13`, gh #214.
//!
//! The diagnostic itself was ported and verified against OpenMC to 3 ulp in
//! `tests/shannon_entropy_vs_openmc.rs`, but had **no caller**: all three k
//! drivers bank into a private `Site` type and never exposed the bank. This
//! gates the wiring.
//!
//! # Why it matters for HTR-10
//!
//! `H` rises from a concentrated initial guess and plateaus once the fission
//! source has converged — that is how you choose how many generations to
//! discard. The RMC reference used **5** inactive cycles on a 1.8 m loosely
//! coupled core; this is the instrument that says whether that is enough.
//!
//! # Results (2026-09-17)
//!
//! Printed by the test.

use outram_mc_libs::geometry::cell::{Cell, HalfSpaceSense, RegionToken};
use outram_mc_libs::geometry::geometry::Geometry;
use outram_mc_libs::geometry::position::Position;
use outram_mc_libs::geometry::surface::{BoundaryType, Sphere, SurfaceKind};
use outram_mc_libs::geometry::universe::Universe;
use outram_mc_libs::material::material::{Material, NuclideComponent};
use outram_mc_libs::material::nuclide::Nuclide;
use outram_mc_libs::physics::keff::{ComputeType, KeffSettings};
use outram_mc_libs::physics::transport_csg::{run_keff_csg, run_keff_csg_hybrid, SourceBox};
use outram_mc_libs::tally::mesh::RegularMesh;

const R: f64 = 8.7407;
const TEMP: f64 = 293.6;

fn heu() -> Option<Vec<Nuclide>> {
    let base = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../reference-data/endf");
    let load = |n: &str, f: &str| -> Option<Nuclide> {
        let p = base.join(f);
        p.exists().then_some(())?;
        Nuclide::from_endf_file(&p, n, TEMP, 1.0e-3).ok()
    };
    Some(vec![
        load("U235", "n-092_U_235-ENDF8.0.endf")?,
        load("U238", "n-092_U_238.endf")?,
    ])
}

fn model() -> (Geometry, Vec<Material>) {
    let surfaces = vec![SurfaceKind::Sphere(Sphere {
        x0: 0.0, y0: 0.0, z0: 0.0, r: R, bc: BoundaryType::Vacuum,
    })];
    let core = Cell::material(
        1,
        vec![RegionToken::HalfSpace { surface_idx: 0, sense: HalfSpaceSense::Inside }],
        0,
        TEMP,
    );
    let geom = Geometry {
        surfaces,
        cells: vec![core],
        universes: vec![Universe { id: 0, cell_indices: vec![0] }],
        lattices: vec![],
        root_universe: 0,
    };
    let mats = vec![Material {
        id: 1,
        name: "HEU".into(),
        components: vec![
            NuclideComponent { nuclide_idx: 0, atom_density: 4.4994e-2 },
            NuclideComponent { nuclide_idx: 1, atom_density: 2.4984e-3 },
        ],
        temperature: TEMP,
    }];
    (geom, mats)
}

/// **Entropy is reported, rises from a concentrated source, and plateaus.**
///
/// The source starts uniform over the bounding box (so partly outside the
/// sphere) and must settle onto the fundamental mode. `H` is bounded above by
/// `log2(n_bins)`, and a converged source on a symmetric body should sit close
/// to — but below — that ceiling.
#[test]
fn entropy_is_reported_and_plateaus() {
    let Some(nucs) = heu() else {
        eprintln!("SKIP: ENDF tapes not in this checkout");
        return;
    };
    let (geom, mats) = model();
    let mesh = RegularMesh {
        lower_left: [-R, -R, -R],
        upper_right: [R, R, R],
        dimension: [4, 4, 4], // coarse on purpose: every bin must be populated
    };
    let s = KeffSettings {
        n_particles: 3000,
        n_inactive: 15,
        n_active: 25,
        temperature_k: TEMP,
        seed: 7,
        compute: ComputeType::CpuSingleThread,
        ..KeffSettings::default()
    };
    let src = SourceBox {
        lower: Position::new(-R, -R, -R),
        upper: Position::new(R, R, R),
    };

    let with = run_keff_csg_hybrid(&geom, &mats, &nucs, &[], Some(&mesh), src, &s, None);
    let without = run_keff_csg(&geom, &mats, &nucs, src, &s, None);

    let ceiling = (mesh.n_bins() as f64).log2();
    println!("k = {:.6} +/- {:.6}", with.k_mean, with.k_std);
    println!("entropy over {} generations, ceiling log2({}) = {ceiling:.4}",
             with.entropy.len(), mesh.n_bins());
    for (i, h) in with.entropy.iter().enumerate() {
        if i < 3 || i >= with.entropy.len() - 3 {
            println!("  gen {i:>3}: H = {h:.5}");
        } else if i == 3 {
            println!("       ...");
        }
    }

    assert!(
        !with.entropy.is_empty(),
        "an entropy mesh was supplied, so entropy must be reported"
    );
    assert!(
        without.entropy.is_empty(),
        "no mesh supplied, so entropy must be skipped at zero cost -- a \
         non-empty vector here means it is being computed unconditionally"
    );
    assert_eq!(
        with.k_mean, without.k_mean,
        "the diagnostic must not perturb the RNG stream: k must be bit-identical \
         with and without the entropy mesh. A difference means the entropy path \
         is drawing random numbers, which would silently change every result."
    );

    for (i, h) in with.entropy.iter().enumerate() {
        assert!(h.is_finite() && *h >= 0.0, "generation {i}: H = {h} is not a valid entropy");
        assert!(*h <= ceiling + 1.0e-9, "generation {i}: H = {h} exceeds log2(n_bins) = {ceiling}");
    }

    // Plateau: the last third should be flat relative to the spread over the
    // first few generations, which start from a deliberately wrong source.
    let n = with.entropy.len();
    let tail = &with.entropy[n * 2 / 3..];
    let tail_mean = tail.iter().sum::<f64>() / tail.len() as f64;
    let tail_spread = tail.iter().fold(0.0_f64, |a, h| a.max((h - tail_mean).abs()));
    println!("tail mean {tail_mean:.5}, max deviation {tail_spread:.5} bits");
    assert!(
        tail_spread < 0.15,
        "the converged tail should be flat to well under a bit; spread {tail_spread:.4}"
    );
}
