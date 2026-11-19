//! GitHub #467: `run_keff_csg` returned `k = 0 ± 0` when the fission bank
//! died before the active generations. OpenMC aborts there
//! (`synchronize_bank`, `src/eigenvalue.cpp:122`, `fatal_error("No fission
//! sites banked ...")`); `try_run_keff_csg` now returns `EmptyFissionBank`
//! and `run_keff_csg` panics with the same message.
//!
//! Methodology: a 0.3 cm bare HEU sphere (Godiva composition, CORE data),
//! far below the 8.74 cm critical radius, run with 3 histories per
//! generation. Seeds are scanned until one loses its bank before the last
//! generation; that seed must give `Err` and a panic, never a `k`. A control
//! (the 8.74 cm sphere at 200 histories) must give `Ok` and the same `k`,
//! bit for bit, as the reviewed driver `run_keff_csg_seq`.
//!
//! Result (2026-11-14): see the issue for the seed found and the numbers.
//!
//! Second part (GitHub #467, remaining entry points): `try_run_keff_csg_seq`,
//! `_par` and `_hybrid` report the same failure on the same tiny sphere, and
//! on the 8.74 cm control return `Ok` with `k` bit-identical to the plain
//! drivers. A bank that empties in the LAST generation (one generation
//! requested) is an error too.

#![cfg(feature = "std")] // needs files, threads or reports (gh:#1605)

use outram_mc_libs::geometry::cell::{Cell, HalfSpaceSense, RegionToken};
use outram_mc_libs::geometry::geometry::Geometry;
use outram_mc_libs::geometry::position::Position;
use outram_mc_libs::geometry::surface::{BoundaryType, Sphere, SurfaceKind};
use outram_mc_libs::geometry::universe::Universe;
use outram_mc_libs::material::material::{Material, NuclideComponent};
use outram_mc_libs::material::nuclide::Nuclide;
use outram_mc_libs::physics::keff::KeffSettings;
use outram_mc_libs::physics::compute::ThreadCount;
use outram_mc_libs::physics::transport_csg::{
    run_keff_csg, run_keff_csg_hybrid, run_keff_csg_par, run_keff_csg_seq, try_run_keff_csg,
    try_run_keff_csg_hybrid, try_run_keff_csg_par, try_run_keff_csg_seq, SourceBox,
};

fn heu_sphere(r: f64) -> (Geometry, Vec<Material>, Vec<Nuclide>, SourceBox) {
    let nuclides = vec![
        Nuclide::from_core("U234").unwrap(),
        Nuclide::from_core("U235").unwrap(),
        Nuclide::from_core("U238").unwrap(),
    ];
    let heu = Material {
        id: 1,
        name: "HEU".into(),
        temperature: 293.6,
        components: vec![
            NuclideComponent { nuclide_idx: 0, atom_density: 4.9184e-4 },
            NuclideComponent { nuclide_idx: 1, atom_density: 4.4994e-2 },
            NuclideComponent { nuclide_idx: 2, atom_density: 2.4984e-3 },
        ],
    };
    let geom = Geometry {
        surfaces: vec![SurfaceKind::Sphere(Sphere {
            x0: 0.0,
            y0: 0.0,
            z0: 0.0,
            r,
            bc: BoundaryType::Vacuum,
        })],
        cells: vec![Cell::material(
            1,
            vec![RegionToken::HalfSpace { surface_idx: 0, sense: HalfSpaceSense::Inside }],
            0,
            293.6,
        )],
        universes: vec![Universe { id: 0, cell_indices: vec![0] }],
        lattices: vec![],
        root_universe: 0,
    };
    let h = r / 3f64.sqrt();
    let sb = SourceBox { lower: Position::new(-h, -h, -h), upper: Position::new(h, h, h) };
    (geom, vec![heu], nuclides, sb)
}

#[test]
fn an_empty_fission_bank_is_an_error_not_k_zero() {
    let (geom, mats, nucs, sb) = heu_sphere(0.3);
    let mut found = None;
    for seed in 1..=50_u64 {
        let settings = KeffSettings {
            n_particles: 3,
            n_inactive: 2,
            n_active: 3,
            seed,
            ..Default::default()
        };
        if let Err(e) = try_run_keff_csg(&geom, &mats, &nucs, sb, &settings, None) {
            assert!(e.generations_completed < e.generations_requested);
            assert_eq!(e.generations_requested, 5);
            assert!(e.to_string().contains("No fission sites banked"));
            println!(
                "seed {seed}: bank empty after {} of {} generations; partial k_mean {}",
                e.generations_completed, e.generations_requested, e.partial.k_mean
            );
            found = Some(settings);
            break;
        }
    }
    let settings = found.expect("a 0.3 cm HEU sphere at 3 histories keeps its bank for 50 seeds");
    let panicked = std::panic::catch_unwind(|| {
        run_keff_csg(&geom, &mats, &nucs, sb, &settings, None)
    });
    assert!(panicked.is_err(), "run_keff_csg must not return a k for an empty bank");
}

#[test]
fn a_surviving_bank_is_unchanged_bit_for_bit() {
    let (geom, mats, nucs, sb) = heu_sphere(8.74);
    let settings = KeffSettings {
        n_particles: 200,
        n_inactive: 2,
        n_active: 3,
        seed: 3,
        ..Default::default()
    };
    let ok = try_run_keff_csg(&geom, &mats, &nucs, sb, &settings, None).expect("bank survives");
    let reference = run_keff_csg_seq(&geom, &mats, &nucs, &[], None, sb, &settings, None, &[], None);
    assert_eq!(ok.k_by_generation, reference.k_by_generation);
    assert_eq!(ok.k_mean.to_bits(), reference.k_mean.to_bits());
}

fn tiny(seed: u64) -> KeffSettings {
    KeffSettings { n_particles: 3, n_inactive: 2, n_active: 3, seed, ..Default::default() }
}

fn control() -> KeffSettings {
    KeffSettings { n_particles: 200, n_inactive: 2, n_active: 3, seed: 3, ..Default::default() }
}

#[test]
fn seq_reports_the_empty_bank_like_try_run_keff_csg() {
    let (geom, mats, nucs, sb) = heu_sphere(0.3);
    let mut errs = 0;
    for seed in 1..=50_u64 {
        let s = tiny(seed);
        let a = try_run_keff_csg_seq(&geom, &mats, &nucs, &[], None, sb, &s, None, &[], None);
        let b = try_run_keff_csg(&geom, &mats, &nucs, sb, &s, None);
        // Same run (single thread), so same verdict, same generation count.
        match (a, b) {
            (Err(a), Err(b)) => {
                assert_eq!(a.generations_completed, b.generations_completed);
                assert!(a.to_string().contains("No fission sites banked"));
                errs += 1;
            }
            (Ok(a), Ok(b)) => assert_eq!(a.k_mean.to_bits(), b.k_mean.to_bits()),
            _ => panic!("seed {seed}: seq and try_run_keff_csg disagree"),
        }
    }
    assert!(errs > 0, "no seed of 50 lost its bank");
}

#[test]
fn par_reports_the_empty_bank() {
    let (geom, mats, nucs, sb) = heu_sphere(0.3);
    let tc = ThreadCount::Fixed(2);
    let err = (1..=50_u64)
        .find_map(|seed| {
            try_run_keff_csg_par(&geom, &mats, &nucs, &[], None, sb, &tiny(seed), None, &[], None, tc)
                .err()
        })
        .expect("a 0.3 cm sphere at 3 histories loses its bank under the parallel driver");
    assert!(err.generations_completed < err.generations_requested);
    assert!(err.to_string().contains("No fission sites banked"));

    let (geom, mats, nucs, sb) = heu_sphere(8.74);
    let ok = try_run_keff_csg_par(&geom, &mats, &nucs, &[], None, sb, &control(), None, &[], None, tc)
        .expect("bank survives");
    let plain = run_keff_csg_par(&geom, &mats, &nucs, &[], None, sb, &control(), None, &[], None, tc);
    assert_eq!(ok.k_by_generation, plain.k_by_generation);
    assert_eq!(ok.k_mean.to_bits(), plain.k_mean.to_bits());
}

#[test]
fn hybrid_reports_the_empty_bank() {
    let (geom, mats, nucs, sb) = heu_sphere(0.3);
    let err = (1..=50_u64)
        .find_map(|seed| {
            try_run_keff_csg_hybrid(&geom, &mats, &nucs, &[], None, sb, &tiny(seed), None).err()
        })
        .expect("a 0.3 cm sphere at 3 histories loses its bank under the hybrid driver");
    assert!(err.to_string().contains("No fission sites banked"));

    let (geom, mats, nucs, sb) = heu_sphere(8.74);
    let ok = try_run_keff_csg_hybrid(&geom, &mats, &nucs, &[], None, sb, &control(), None)
        .expect("bank survives");
    let plain = run_keff_csg_hybrid(&geom, &mats, &nucs, &[], None, sb, &control(), None);
    assert_eq!(ok.k_by_generation, plain.k_by_generation);
    assert_eq!(ok.k_mean.to_bits(), plain.k_mean.to_bits());
}

#[test]
fn seq_control_is_bit_identical() {
    let (geom, mats, nucs, sb) = heu_sphere(8.74);
    let ok = try_run_keff_csg_seq(&geom, &mats, &nucs, &[], None, sb, &control(), None, &[], None)
        .expect("bank survives");
    let plain = run_keff_csg_seq(&geom, &mats, &nucs, &[], None, sb, &control(), None, &[], None);
    assert_eq!(ok.k_by_generation, plain.k_by_generation);
    assert_eq!(ok.k_mean.to_bits(), plain.k_mean.to_bits());
}

/// One generation requested: the bank of the only (= last) generation is
/// empty, the driver ends "normally" with `k = 0`, and that is an error.
#[test]
fn a_bank_that_dies_in_the_last_generation_is_an_error() {
    let (geom, mats, nucs, sb) = heu_sphere(0.3);
    let s = KeffSettings { n_particles: 3, n_inactive: 0, n_active: 1, seed: 1, ..Default::default() };
    let plain = run_keff_csg_seq(&geom, &mats, &nucs, &[], None, sb, &s, None, &[], None);
    assert_eq!(plain.k_by_generation, vec![0.0], "setup: the single generation banks nothing");
    let e = try_run_keff_csg(&geom, &mats, &nucs, sb, &s, None).unwrap_err();
    assert_eq!((e.generations_completed, e.generations_requested), (0, 1));
}
