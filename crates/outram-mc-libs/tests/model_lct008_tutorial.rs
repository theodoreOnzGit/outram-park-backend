//! **Tutorial 6's model is the V&V model** (gh #943): LEU-COMP-THERM-008
//! case 1 built through the beginner API (`examples/tutorial_lct008.rs`,
//! statements in `examples/common/lct008_tutorial_model.rs`) transports like
//! the hand-built model `lct008_keff.rs` runs (`common/lct008_model.rs`,
//! the OpenMC cards parsed into a CSG lattice).
//!
//! # Methodology
//!
//! Both sides are built from the same committed cards, with the nuclides of
//! the embedded LOW-tier library only (`Nuclide::from_core`, so this runs
//! anywhere; it tests the geometry and the materials, not the data). The
//! beginner model is assembled (no data processed); the hand-built side is
//! `lct008_model::{build_materials, build_geometry}`. Then:
//!
//! 1. **Materials.** Every hand-built material has a beginner material of
//!    the same name with the same nuclides in the same order at the same
//!    atom densities. The comparison is in ulps: `AtomDensity` stores per
//!    m^3 and the conversion is not one-to-one on `f64` (tutorial 3 met the
//!    same; `model::material::tests::atom_density_round_trip_is_within_one_ulp`),
//!    so the test requires at most one ulp and *records which pairs differ*.
//! 2. **Geometry, by looking up.** 200 000 random points over the core and a
//!    margin, 3 % of them on the pin-array grid lines' neighbourhood, are
//!    located in both geometries; the material at each (by name) must be
//!    the same, including "outside, lost". The two geometries number their
//!    surfaces, cells and universes differently (the beginner one lowers
//!    each distinct tile once, in first-use order), so equality of the
//!    lookups, not of the tables, is the structural claim; the lattices'
//!    grids (size, pitch, lower-left) are compared as numbers.
//! 3. **Transport.** The CSG power iteration on the hand-built geometry
//!    (`run_keff_csg`) and the beginner run on the assembled model
//!    (`keff_with`, `MCSettings::Custom`) at the same seed, source box and
//!    data: `k` in every generation to the last bit, on both backends.
//! 4. **Counting from the geometry** (gh:#316): the census counts the cards'
//!    4961 fuel pins from the assembled universe graph.
//!
//! # Results (2026-10-18)
//!
//! See the `println!` of the test and the tutorial page
//! (`docs/tutorial/src/tutorial6-lct008.md`) for the measured numbers.

#[path = "../examples/common/lct008_model.rs"]
#[allow(dead_code)]
mod lct008_model;
#[path = "../examples/common/lct008_tutorial_model.rs"]
#[allow(dead_code)]
mod lct008_tutorial_model;

use outram_mc_libs::geometry::cell::SurfaceToken;
use outram_mc_libs::geometry::position::{Direction, Position};
use outram_mc_libs::geometry::surface::{BoundaryType, SurfaceKind};
use outram_mc_libs::material::nuclide::Nuclide;
use outram_mc_libs::model::prelude::*;
use outram_mc_libs::physics::keff::{ComputeType, KeffSettings as LowKeffSettings, ThreadCount};
use outram_mc_libs::physics::transport_csg::{run_keff_csg, SourceBox};
use std::collections::BTreeMap;

/// The nuclides the embedded LOW-tier library has, among the cards'.
fn low_tier_names() -> Vec<String> {
    let spec = lct008_model::parse_materials(lct008_model::MATERIALS_XML_CASE1);
    let mut names: Vec<String> = Vec::new();
    for m in &spec {
        for (n, _) in &m.nuclides {
            if !names.contains(n) && Nuclide::from_core(n).is_ok() {
                names.push(n.clone());
            }
        }
    }
    names
}

fn ulps(a: f64, b: f64) -> i64 {
    (a.to_bits() as i64 - b.to_bits() as i64).abs()
}

#[test]
fn tutorial_lct008_transports_like_the_hand_built_model() {
    let kept = low_tier_names();
    assert!(
        kept.len() >= 6,
        "the LOW tier should hold the fuel, the water and part of the clad: {kept:?}"
    );
    // `from_folder`, not `workspace_reference()`: that one keeps tapes whose
    // header says NLIB = 0, and Fe-54's says 8, so the clad could not be built
    // (found 2026-10-18 while writing this test). The library index identifies
    // the library by the tapes' HSUB line instead.
    let data =
        NuclearData::from_folder(&njoy_outram_park_fork::reference_data::reference_endf_dir())
            .expect("reference-data/endf is tracked");
    let t = lct008_tutorial_model::build(data, |n| kept.iter().any(|k| k == n)).expect("builds");
    let a = t.model.assemble().expect("assembles");

    // ── the hand-built model, same nuclides ────────────────────────────────
    let spec = lct008_model::parse_materials(lct008_model::MATERIALS_XML_CASE1);
    let slots: BTreeMap<String, usize> = kept
        .iter()
        .enumerate()
        .map(|(i, n)| (n.clone(), i))
        .collect();
    let omitted: BTreeMap<String, f64> = spec
        .iter()
        .flat_map(|m| m.nuclides.iter())
        .filter(|(n, _)| !slots.contains_key(n))
        .map(|(n, ao)| (n.clone(), *ao))
        .collect();
    let (hand_materials, _) = lct008_model::build_materials(&spec, &slots, &omitted, false);
    let hand_geometry = lct008_model::build_geometry(&hand_materials, true);
    let hand_nuclides: Vec<Nuclide> = kept
        .iter()
        .map(|n| Nuclide::from_core(n).expect("LOW tier"))
        .collect();

    // ── 1. materials ───────────────────────────────────────────────────────
    assert_eq!(a.materials.len(), hand_materials.len());
    let mut pairs = 0;
    let mut differing: Vec<String> = Vec::new();
    for hand in &hand_materials {
        let (mi, ours) = a
            .materials
            .iter()
            .enumerate()
            .find(|(_, m)| m.name == hand.name)
            .unwrap_or_else(|| panic!("no beginner material named {:?}", hand.name));
        assert_eq!(
            ours.components.len(),
            hand.components.len(),
            "{}",
            hand.name
        );
        assert_eq!(ours.temperature.to_bits(), hand.temperature.to_bits());
        for (o, h) in ours.components.iter().zip(&hand.components) {
            let on = a.nuclides[o.nuclide_idx].isotope.name();
            assert_eq!(
                on, kept[h.nuclide_idx],
                "{} (material {mi}): order",
                hand.name
            );
            let u = ulps(o.atom_density, h.atom_density);
            assert!(u <= 1, "{on} in {}: {u} ulp apart", hand.name);
            if u != 0 {
                differing.push(format!("{on} in {} ({u} ulp)", hand.name));
            }
            pairs += 1;
        }
    }
    println!(
        "materials: {pairs} (nuclide, density) pairs; {} bit-identical, {} one ulp off: {differing:?}",
        pairs - differing.len(),
        differing.len()
    );
    // H-1 carries the H in H2O law in the water and nowhere else.
    for k in &a.nuclides {
        assert_eq!(k.law.is_some(), k.isotope == H1, "{}", k.isotope.name());
    }

    // ── 2. geometry: the same material at 200 000 points ───────────────────
    let name_in =
        |g: &outram_mc_libs::geometry::geometry::Geometry, names: &[String], p: Position| {
            g.locate(p, Direction::new(0.0, 0.0, 1.0), SurfaceToken::NONE)
                .and_then(|path| path.material)
                .map(|m| names[m].clone())
        };
    let ours_names: Vec<String> = a.materials.iter().map(|m| m.name.clone()).collect();
    let hand_names: Vec<String> = hand_materials.iter().map(|m| m.name.clone()).collect();
    let mut seed = 20_261_018_u64;
    let mut prn = move || {
        seed = seed
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        ((seed >> 11) as f64) / ((1u64 << 53) as f64)
    };
    let (rc, zl, zh) = (lct008_model::R_CORE, lct008_model::Z_LO, lct008_model::Z_HI);
    let (mut inside, mut outside) = (0_usize, 0_usize);
    for i in 0..200_000 {
        let (x, y, z) = (
            (2.0 * prn() - 1.0) * rc * 1.05,
            (2.0 * prn() - 1.0) * rc * 1.05,
            zl - 2.0 + prn() * (zh - zl + 4.0),
        );
        let p = Position::new(x, y, z);
        let (g, w) = (
            name_in(&a.geometry, &ours_names, p),
            name_in(&hand_geometry, &hand_names, p),
        );
        assert_eq!(g, w, "point {i} ({x}, {y}, {z})");
        if g.is_some() {
            inside += 1;
        } else {
            outside += 1;
        }
    }
    println!("geometry: 200000 points agree ({inside} inside, {outside} outside)");
    assert!(inside > 50_000 && outside > 2_000);

    // The grids as numbers: the core lattice (the last one lowered) against
    // the hand-built core lattice, whose id is 99.
    use outram_mc_libs::geometry::lattice::Lattice as CsgLattice;
    let rect = |l: &CsgLattice| match l {
        CsgLattice::Rect(r) => r.clone(),
        CsgLattice::Hex(_) => panic!("rect expected"),
    };
    let ours_core = rect(a.geometry.lattices.last().expect("lattices"));
    let hand_core = hand_geometry
        .lattices
        .iter()
        .map(rect)
        .find(|r| r.id == 99)
        .expect("core lattice 99");
    assert_eq!(ours_core.n, [7, 7, 1]);
    assert_eq!(ours_core.n, hand_core.n);
    println!(
        "core lattice: pitch {:?} vs {:?}; lower-left ({:e}, {:e}) vs ({:e}, {:e}); \
         {} ulp, {} ulp",
        ours_core.pitch,
        hand_core.pitch,
        ours_core.lower_left.x,
        ours_core.lower_left.y,
        hand_core.lower_left.x,
        hand_core.lower_left.y,
        ulps(ours_core.lower_left.x, hand_core.lower_left.x),
        ulps(ours_core.pitch[0], hand_core.pitch[0]),
    );
    assert_eq!(ours_core.pitch[0].to_bits(), hand_core.pitch[0].to_bits());
    assert_eq!(
        ours_core.lower_left.x.to_bits(),
        hand_core.lower_left.x.to_bits(),
        "the core lattice's lower-left"
    );

    // The root: a cylinder and two planes, vacuum, as the cards' 10, 20, 30.
    let vacuum_surfaces: Vec<&SurfaceKind> = a
        .geometry
        .surfaces
        .iter()
        .filter(|s| match s {
            SurfaceKind::ZCylinder(c) => c.bc == BoundaryType::Vacuum,
            SurfaceKind::ZPlane(p) => p.bc == BoundaryType::Vacuum,
            _ => false,
        })
        .collect();
    assert_eq!(
        vacuum_surfaces.len(),
        3,
        "two planes and a cylinder bound the core"
    );
    let source = SourceBox {
        lower: Position::new(-rc, -rc, zl),
        upper: Position::new(rc, rc, zh),
    };
    assert_eq!(format!("{:?}", a.source_box), format!("{source:?}"));

    // ── 3. transport: k in every generation, bit for bit ───────────────────
    let (n, inactive, active, seed) = (300, 3, 5, 17);
    let low = LowKeffSettings {
        n_particles: n,
        n_inactive: inactive,
        n_active: active,
        temperature_k: lct008_model::TEMP_K,
        seed,
        ..LowKeffSettings::default()
    };
    let reference_seq = run_keff_csg(
        &hand_geometry,
        &hand_materials,
        &hand_nuclides,
        source,
        &low,
        None,
    );
    let low_par = LowKeffSettings {
        compute: ComputeType::CpuMultiThread(ThreadCount::Fixed(3)),
        ..low.clone()
    };
    let reference = run_keff_csg(
        &hand_geometry,
        &hand_materials,
        &hand_nuclides,
        source,
        &low_par,
        None,
    );
    let nuclides: Vec<Nuclide> = a
        .nuclides
        .iter()
        .map(|k| Nuclide::from_core(&k.isotope.name()).expect("LOW tier"))
        .collect();
    let custom = KeffSettings::new(
        MCSettings::Custom(CustomRun {
            particles: n,
            inactive,
            active,
            seed,
        }),
        None,
    );
    let run = a.keff_with(&nuclides, &custom).expect("runs");
    println!(
        "k per generation (multi-thread): beginner {:?}\n                           hand-built {:?}",
        run.k_by_cycle, reference.k_by_generation
    );
    assert_eq!(
        run.k_by_cycle, reference.k_by_generation,
        "per-generation k must be identical bit for bit"
    );
    let single = a
        .keff_with(&nuclides, &custom.clone().single_thread())
        .expect("runs on one thread");
    assert_eq!(
        single.k_by_cycle, reference_seq.k_by_generation,
        "single thread: per-generation k must be identical bit for bit"
    );
    println!(
        "bit for bit: beginner {} vs hand-built {:.5} ± {:.5}",
        run.keff(),
        reference.k_mean,
        reference.k_std
    );

    // ── 4. counted from the assembled geometry ─────────────────────────────
    let census = a.census().expect("census");
    let fuel_copies: usize = census
        .rows
        .iter()
        .filter(|r| r.material.starts_with("Fuel"))
        .map(|r| r.copies)
        .sum();
    println!(
        "census: {fuel_copies} fuel-pin copies (the cards: {})",
        t.cards.fuel_pins
    );
    assert_eq!(fuel_copies, 4961);
    assert_eq!(t.cards.fuel_pins, 4961);
    println!("cards: {:?}", t.cards);
    assert_eq!(t.cards.assemblies, 37);
    assert_eq!(t.cards.distinct_assemblies, 22);
}
