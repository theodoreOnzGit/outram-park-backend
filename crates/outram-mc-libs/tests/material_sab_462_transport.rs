//! GitHub #462: migrating a driver from the nuclide-level binding
//! (`Nuclide::with_thermal_scattering`) to the per-material association
//! (`assign_material_sab`) must not move a result when the bound nuclide
//! appears only in the materials that name the table.
//!
//! Methodology: a three-region sphere in CORE data: a U-235 / H-1 / O-16
//! core (material 0), an O-16 shell with no hydrogen (material 1) and a
//! water reflector H-1 / O-16 (material 2). The old way binds H-1 (entry 1)
//! to H in H2O (embedded LEAPR deck, 293.6 K) and both H-bearing materials
//! reference it. The new way loads H-1 free and names the table for the same
//! two materials with `MaterialSab::for_materials_containing`. The same seed
//! and settings are run through `run_keff_csg_seq`; the criterion is
//! bit-identical `k` per generation, and the check also asserts that the
//! shell never touches a bound entry.
//!
//! Result (2026-11-14): recorded on #462.

#![cfg(feature = "std")] // needs files, threads or reports (gh:#1605)

use njoy_outram_park_fork::leapr::decks::SabMaterial;
use njoy_outram_park_fork::reference_data::reference_endf;
use outram_mc_libs::material::speed::SpeedTier;
use outram_mc_libs::geometry::cell::{Cell, HalfSpaceSense, RegionToken};
use outram_mc_libs::geometry::geometry::Geometry;
use outram_mc_libs::geometry::position::Position;
use outram_mc_libs::geometry::surface::{BoundaryType, Sphere, SurfaceKind};
use outram_mc_libs::geometry::universe::Universe;
use outram_mc_libs::material::material::{Material, NuclideComponent};
use outram_mc_libs::material::material_sab::{assign_material_sab, MaterialSab};
use outram_mc_libs::material::nuclide::Nuclide;
use outram_mc_libs::material::thermal::ThermalScattering;
use outram_mc_libs::physics::keff::KeffSettings;
use outram_mc_libs::physics::compute::ThreadCount;
use outram_mc_libs::physics::transport_csg::{run_keff_csg, run_keff_csg_hybrid, run_keff_csg_par, run_keff_csg_seq, SourceBox};

const T: f64 = 293.6;
const U235: usize = 0;
const H1: usize = 1;
const O16: usize = 2;

fn sphere(r: f64, bc: BoundaryType) -> SurfaceKind {
    SurfaceKind::Sphere(Sphere { x0: 0.0, y0: 0.0, z0: 0.0, r, bc })
}

fn comp(v: &[(usize, f64)]) -> Vec<NuclideComponent> {
    v.iter().map(|&(nuclide_idx, atom_density)| NuclideComponent { nuclide_idx, atom_density }).collect()
}

fn model() -> (Geometry, Vec<Material>, Vec<Nuclide>, ThermalScattering, SourceBox) {
    model_with(false)
}

/// `endf`: H-1 and O-16 reconstructed from the ENDF/B-VIII.0 tapes (the
/// pointwise tier the benchmark examples use), tsl-HinH2O from its tape.
fn model_with(endf: bool) -> (Geometry, Vec<Material>, Vec<Nuclide>, ThermalScattering, SourceBox) {
    let (nuclides, table) = if endf {
        let load = |name: &str, file: &str| {
            let p = reference_endf(file).expect("tape");
            Nuclide::from_endf_file_with_speed(&p, name, T, SpeedTier::default()).unwrap()
        };
        let tsl = reference_endf("tsl-HinH2O.endf").expect("tsl tape");
        (
            vec![
                Nuclide::from_core("U235").unwrap(),
                load("H1", "n-001_H_001-ENDF8.0-Beta6.endf"),
                load("O16", "n-008_O_016-ENDF8.0.endf"),
            ],
            ThermalScattering::from_endf_file(tsl.to_str().unwrap(), 1, T, "c_H_in_H2O").unwrap(),
        )
    } else {
        (
            vec![
                Nuclide::from_core("U235").unwrap(),
                Nuclide::from_core("H1").unwrap(),
                Nuclide::from_core("O16").unwrap(),
            ],
            ThermalScattering::from_embedded_leapr_deck(SabMaterial::HInH2O, T, "HInH2O").unwrap(),
        )
    };
    let mat = |id, name: &str, components| Material { id, name: name.into(), temperature: T, components };
    let materials = vec![
        mat(1, "solution", comp(&[(U235, 1.7e-3), (H1, 5.9e-2), (O16, 3.3e-2)])),
        mat(2, "shell (no H)", comp(&[(O16, 4.0e-2)])),
        mat(3, "reflector", comp(&[(H1, 6.7e-2), (O16, 3.3e-2)])),
    ];
    let inside = |s| RegionToken::HalfSpace { surface_idx: s, sense: HalfSpaceSense::Inside };
    let outside = |s| RegionToken::HalfSpace { surface_idx: s, sense: HalfSpaceSense::Outside };
    let geom = Geometry {
        surfaces: vec![
            sphere(14.0, BoundaryType::Transmissive),
            sphere(15.0, BoundaryType::Transmissive),
            sphere(40.0, BoundaryType::Vacuum),
        ],
        cells: vec![
            Cell::material(1, vec![inside(0)], 0, T),
            Cell::material(2, vec![outside(0), inside(1)], 1, T),
            Cell::material(3, vec![outside(1), inside(2)], 2, T),
        ],
        universes: vec![Universe { id: 0, cell_indices: vec![0, 1, 2] }],
        lattices: vec![],
        root_universe: 0,
    };
    let h = 14.0 / 3f64.sqrt();
    let src = SourceBox { lower: Position::new(-h, -h, -h), upper: Position::new(h, h, h) };
    (geom, materials, nuclides, table, src)
}

#[test]
fn per_material_association_is_bit_identical_to_the_nuclide_binding_when_only_bound_materials_share_it() {
    bit_identical(false);
}

#[test]
fn the_same_on_pointwise_endf_data() {
    if reference_endf("tsl-HinH2O.endf").is_none() {
        println!("SKIP: reference-data/endf not available");
        return;
    }
    bit_identical(true);
}

fn bit_identical(endf: bool) {
    let (geom, materials, nuclides, table, src) = model_with(endf);
    let settings = KeffSettings {
        n_particles: 300,
        n_inactive: 3,
        n_active: 6,
        seed: 7,
        temperature_k: T,
        ..Default::default()
    };

    // Old: H-1 bound at its own entry; solution and reflector reference it.
    let mut old_nucs = nuclides.clone();
    #[allow(deprecated)]
    {
        old_nucs[H1] = old_nucs[H1].clone().with_thermal_scattering(table.clone());
    }
    let old = run_keff_csg_seq(&geom, &materials, &old_nucs, &[], None, src, &settings, None, &[], None);

    // New: the same two materials name the table.
    let entries = MaterialSab::for_materials_containing(&materials, H1, 0, &[]);
    assert_eq!(entries.iter().map(|e| e.material).collect::<Vec<_>>(), vec![0, 2]);
    let (new_mats, new_nucs) =
        assign_material_sab(&materials, &nuclides, std::slice::from_ref(&table), &entries).unwrap();
    assert!(new_nucs[..3].iter().all(|n| !n.has_thermal_scattering()), "originals stay free");
    assert!(new_nucs[3].has_thermal_scattering());
    assert!(
        new_mats[1].components.iter().all(|c| !new_nucs[c.nuclide_idx].has_thermal_scattering()),
        "the hydrogen-free shell references no bound entry"
    );
    let new = run_keff_csg_seq(&geom, &new_mats, &new_nucs, &[], None, src, &settings, None, &[], None);

    println!(
        "old (nuclide-level) k = {:.5} +/- {:.5}; new (per-material) k = {:.5} +/- {:.5}",
        old.k_mean, old.k_std, new.k_mean, new.k_std
    );
    assert_eq!(old.k_by_generation, new.k_by_generation);
    assert_eq!(old.k_mean.to_bits(), new.k_mean.to_bits());

    // The other drivers too (hst009_keff runs `run_keff_csg`).
    let tc = ThreadCount::Fixed(2);
    let op = run_keff_csg_par(&geom, &materials, &old_nucs, &[], None, src, &settings, None, &[], None, tc);
    let np = run_keff_csg_par(&geom, &new_mats, &new_nucs, &[], None, src, &settings, None, &[], None, tc);
    assert_eq!(op.k_by_generation, np.k_by_generation, "par");
    let oh = run_keff_csg_hybrid(&geom, &materials, &old_nucs, &[], None, src, &settings, None);
    let nh = run_keff_csg_hybrid(&geom, &new_mats, &new_nucs, &[], None, src, &settings, None);
    assert_eq!(oh.k_by_generation, nh.k_by_generation, "hybrid");
    let oc = run_keff_csg(&geom, &materials, &old_nucs, src, &settings, None);
    let nc = run_keff_csg(&geom, &new_mats, &new_nucs, src, &settings, None);
    assert_eq!(oc.k_by_generation, nc.k_by_generation, "run_keff_csg");
}

#[test]
fn for_materials_containing_honours_the_exceptions() {
    let (_, materials, _, _, _) = model();
    let all = MaterialSab::for_materials_containing(&materials, H1, 0, &[]);
    let but_reflector = MaterialSab::for_materials_containing(&materials, H1, 0, &[2]);
    assert_eq!(all.len(), 2);
    assert_eq!(but_reflector.len(), 1);
    assert_eq!(but_reflector[0].material, 0);
    assert!(MaterialSab::for_materials_containing(&materials, U235, 0, &[0]).is_empty());
}
