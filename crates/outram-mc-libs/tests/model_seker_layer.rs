//! **Tutorial 7's model is the CSG it claims to be** (gh #943): one Şeker
//! layer of the HTR-10 pebble bed, an infinite stack of it, built through the
//! beginner API (`examples/tutorial_seker_layer.rs`, statements in
//! `examples/common/seker_layer_model.rs`) lowers to the hexagonal-lattice
//! CSG a hand-written model has.
//!
//! # Methodology
//!
//! The hand-built side is written here from **nee_soon's site table**
//! (`SekerCell::site_centre`, `SekerSite::ALL`: 23 ball sites, 13 owned by
//! the tile and 10 pieces of neighbours') with the low-level types only
//! (spheres, general planes, `HexLattice::from_rings`), independent of the
//! beginner API's translation of the owned balls into neighbour pieces. The
//! beginner model is assembled (no data processed). The nuclides of the
//! embedded LOW-tier library only, so this runs anywhere (it tests the
//! geometry and the lowering, not the data); materials are the beginner
//! model's own on both sides (tutorial 3 and 6's tests compare those to
//! hand-built ones to the ulp). The pebbles here are smeared (one material
//! per ball) so the hand-built tile stays 23 spheres; the TRISO fuel ball is
//! covered by `tests/model_pebble.rs`, and the full TRISO layer by the
//! third test below.
//!
//! 1. **The fuel rule.** The 13 balls take the core's low-discrepancy rule
//!    (`SekerBed::assign`) at 0.57: 7 fuel, in the order the rule gives.
//! 2. **Structure.** The 23 ball surfaces of the beginner model are exactly
//!    nee_soon's 23 sites (centre and radius to 1e-9 cm); the container's
//!    six faces are unit planes at the apothem with normals 30 degrees
//!    apart; its two ends are at +-h/2; all eight reflective; the lattice is
//!    one-ring and Y-oriented at the Şeker pitch.
//! 3. **Lookups.** 300 000 random points of the prism: the same material (by
//!    name) in both geometries.
//! 4. **Transport.** The CSG power iteration on the hand-built geometry
//!    (`run_keff_csg`) and the beginner run (`keff_with`,
//!    `MCSettings::Custom`) at the same seed, source box and data: `k` in
//!    every generation to the last bit, on both backends.
//! 5. **The TRISO layer against the site table.** The real model (tutorial
//!    5's fuel ball in 7 of the 13 places) assembled from the ENDF tape
//!    headers: the census counts 7 fuel and 6 moderator balls and
//!    7 x 8335 particles from the assembled geometry; the packing fraction
//!    is 13 balls over the tile; and 200 000 random points are classified by
//!    the 23-site table alone (inside which ball? how far from its centre?)
//!    and agree with what the geometry finds.
//!
//! # Results (2026-10-18)
//!
//! All pass; the counts and the `k` values are printed by the tests (not
//! physics results: LOW tier, tiny runs).

#[path = "../examples/common/seker_layer_model.rs"]
#[allow(dead_code)]
mod seker_layer_model;

use outram_mc_libs::geometry::cell::{
    Cell, CellFill, HalfSpaceSense, RegionToken, SurfaceToken, TrackingMethod,
};
use outram_mc_libs::geometry::geometry::Geometry;
use outram_mc_libs::geometry::lattice::{HexLattice, HexOrientation, Lattice as CsgLattice};
use outram_mc_libs::geometry::position::{Direction, Position};
use outram_mc_libs::geometry::surface::{BoundaryType, Plane, Sphere as CsgSphere, SurfaceKind, ZPlane};
use outram_mc_libs::geometry::universe::Universe;
use outram_mc_libs::material::nuclide::Nuclide;
use outram_mc_libs::model::prelude::*;
use outram_mc_libs::physics::keff::KeffSettings as LowKeffSettings;
use outram_mc_libs::physics::keff::{ComputeType, ThreadCount};
use outram_mc_libs::physics::transport_csg::run_keff_csg;
use seker_layer_model as seker;
use uom::si::volume::cubic_centimeter;

const T: f64 = 300.15;

/// One of nee_soon's 23 ball sites (`SekerSite::ALL`), by its own formulas.
#[derive(Clone, Copy, Debug)]
enum Nee {
    Bottom(u8),
    Top(u8),
    Inner(u8),
    Outer(u8),
    Entering(u8),
}

/// `SekerCell::site_centre`, for 6 cm balls, written out again here from the
/// source (`bed.rs`): polar helpers, the flower, `d/sqrt(3)`,
/// `2 d/sqrt(3)` and `pitch - 2 d/sqrt(3)`.
fn nee_centre(site: Nee) -> [f64; 3] {
    let d = 6.0_f64;
    let apothem = d * (std::f64::consts::PI / 6.0).cos() + 0.5 * d;
    let pitch = 2.0 * apothem;
    let h = 2.0 * (d * (2.0_f64 / 3.0).sqrt());
    let polar = |rho: f64, deg: f64, z: f64| {
        let t = deg.to_radians();
        [rho * t.cos(), rho * t.sin(), z]
    };
    let flower = |k: u8, z: f64| {
        if k == 0 {
            [0.0, 0.0, z]
        } else {
            polar(d, 60.0 * f64::from(k - 1), z)
        }
    };
    let s3 = 3.0_f64.sqrt();
    match site {
        Nee::Bottom(k) => flower(k, -0.5 * h),
        Nee::Top(k) => flower(k, 0.5 * h),
        Nee::Inner(k) => polar(d / s3, 90.0 + 120.0 * f64::from(k), 0.0),
        Nee::Outer(k) => polar(2.0 * d / s3, 270.0 + 120.0 * f64::from(k), 0.0),
        Nee::Entering(k) => polar(pitch - 2.0 * d / s3, 90.0 + 120.0 * f64::from(k), 0.0),
    }
}

/// The 23 sites, and for each the index (into the tile's 13 owned balls, in
/// `seker::own_sites()` order) of the ball it is a piece of: a top flower is
/// its bottom flower in the next layer, an entering ball is the neighbour's
/// `Outer` of the same `k`.
fn nee_sites() -> Vec<(Nee, usize)> {
    let mut v = Vec::new();
    for k in 0..7 {
        v.push((Nee::Bottom(k), usize::from(k)));
    }
    for k in 0..7 {
        v.push((Nee::Top(k), usize::from(k)));
    }
    for k in 0..3 {
        v.push((Nee::Inner(k), 7 + usize::from(k)));
    }
    for k in 0..3 {
        v.push((Nee::Outer(k), 10 + usize::from(k)));
    }
    for k in 0..3 {
        v.push((Nee::Entering(k), 10 + usize::from(k)));
    }
    v
}

#[test]
fn the_fuel_rule_gives_seven_of_thirteen_in_the_orders_it_names() {
    let flags = seker::fuel_flags(seker::FUEL_BALL_FRACTION);
    // Flower: centre (n = 0), then the six on a circle by angle -120, -60, 0,
    // 60, 120, 180 degrees = k 5, 6, 1, 2, 3, 4 (n = 1..6): fuel at n = 1, 3, 5
    // = k 5, 1, 3. Central: Inner by angle -150, -30, 90 = k 1, 2, 0
    // (n = 7, 8, 9): fuel at 7, 8 = k 1, 2; Outer by angle -90, 30, 150 =
    // k 0, 1, 2 (n = 10, 11, 12): fuel at 10, 12 = k 0, 2.
    let expected = vec![
        false, true, false, true, false, true, false, // flower k = 0..6
        false, true, true, // Inner
        true, false, true, // Outer
    ];
    assert_eq!(flags, expected);
    assert_eq!(flags.iter().filter(|f| **f).count(), 7);
    // The same rule at other fractions: counts as it says.
    assert_eq!(seker::fuel_flags(0.0).iter().filter(|f| **f).count(), 0);
    assert_eq!(seker::fuel_flags(1.0).iter().filter(|f| **f).count(), 13);
    for n in 0..=13 {
        assert_eq!(
            seker::fuel_flags_for_count(n)
                .iter()
                .filter(|f| **f)
                .count(),
            n
        );
    }
    // The cell's own numbers: 13 balls make 0.6448 of the tile.
    let ball_v = 4.0 / 3.0 * std::f64::consts::PI * 27.0;
    let tile_v = 2.0 * 3.0_f64.sqrt() * seker::apothem_cm().powi(2) * seker::height_cm();
    assert!((13.0 * ball_v / tile_v - 0.6448).abs() < 5e-5);
    assert!((seker::apothem_cm() - 8.196_152).abs() < 1e-6);
    assert!((seker::height_cm() - 9.797_959).abs() < 1e-6);
    // 13 owned sites, each a distinct place inside the tile.
    let sites = seker::own_sites();
    assert_eq!(sites.len(), 13);
    for s in &sites {
        let [x, y, z] = seker::site_centre(*s);
        assert!(z.abs() <= 0.5 * seker::height_cm() + 1e-12);
        assert!(x.hypot(y) < seker::pitch_cm());
    }
}

/// A library of incident-neutron headers (masses only): enough to assemble.
fn header_only_library() -> NuclearData {
    let endf = |x: f64| {
        let e = x.abs().log10().floor() as i32;
        format!("{:.6}{e:+}", x / 10f64.powi(e))
    };
    let tape = |za: f64, awr: f64| {
        format!(
            " TPID\n{:>11}{:>11}{:>11}{:>11}{:>11}{:>11}\n{:>11}{:>11}{:>11}{:>11}{:>11}{:>11}\n{:>11}{:>11}{:>11}{:>11}{:>11}{:>11}\n",
            endf(za),
            endf(awr),
            0, 0, 0, 0, "0.0", "0.0", 0, 0, 0, 6, "1.0", "2.0+7", 0, 0, 10, 8
        )
        .into_bytes()
    };
    NuclearData::from_endf_bytes(
        "headers only",
        [
            ("u235".to_string(), tape(92235.0, 233.0248)),
            ("u238".to_string(), tape(92238.0, 236.0058)),
            ("o16".to_string(), tape(8016.0, 15.857510)),
            ("c12".to_string(), tape(6012.0, 11.896910)),
        ],
    )
    .expect("four incident-neutron headers")
}

/// The test's materials: LOW-tier nuclides only. The gas stands in for helium
/// (no He in the embedded library): a thin carbon, "gas".
fn small_materials() -> (seker::Materials, Material) {
    let d = atoms_per_barn_cm;
    let mix = Material::from_atom_densities([
        (U235, d(0.0016)),
        (U238, d(0.0064)),
        (O16, d(0.016)),
        (C12, d(0.05)),
    ])
    .named("fuel mix");
    let shell = Material::from_atom_densities([(C12, d(0.0870))]).named("shell graphite");
    let gas = Material::from_atom_densities([(C12, d(1.0e-6))]).named("gas");
    (
        seker::Materials {
            uo2: mix.clone(),
            buffer: shell.clone(),
            pyc: shell.clone(),
            sic: shell.clone(),
            matrix: shell.clone(),
            shell,
            helium: gas,
        },
        mix,
    )
}

fn low_tier(a: &AssembledModel) -> Vec<Nuclide> {
    a.nuclides
        .iter()
        .map(|k| {
            let name = match k.isotope.name().as_str() {
                "C12" => "C0".to_string(),
                other => other.to_string(),
            };
            Nuclide::from_core(&name).expect("CORE nuclide")
        })
        .collect()
}

fn half(s: usize, sense: HalfSpaceSense) -> RegionToken {
    RegionToken::HalfSpace {
        surface_idx: s,
        sense,
    }
}

/// The hand-built geometry from the site table: 23 spheres, 6 planes and 2
/// ends (all reflective), the 23 piece cells, the helium cell, an `outer`
/// universe and the container's fill cell holding a one-ring `HexLattice`.
fn hand_built(a: &AssembledModel, fuel: &[bool]) -> Geometry {
    let m = |name: &str| {
        a.materials
            .iter()
            .position(|x| x.name == name)
            .unwrap_or_else(|| panic!("no material {name}"))
    };
    let (fuel_m, mod_m, gas_m) = (m("fuel mix"), m("shell graphite"), m("gas"));
    let sites = nee_sites();
    let mut surfaces: Vec<SurfaceKind> = sites
        .iter()
        .map(|(s, _)| {
            let c = nee_centre(*s);
            SurfaceKind::Sphere(CsgSphere {
                x0: c[0],
                y0: c[1],
                z0: c[2],
                r: 3.0,
                bc: BoundaryType::Transmissive,
            })
        })
        .collect();
    let apothem = seker::apothem_cm();
    let h = seker::height_cm();
    let s3 = 0.866_025_403_784_438_6_f64;
    let normals = [
        [0.0, 1.0],
        [s3, 0.5],
        [-s3, 0.5],
        [0.0, -1.0],
        [-s3, -0.5],
        [s3, -0.5],
    ];
    let first_plane = surfaces.len();
    for n in normals {
        surfaces.push(SurfaceKind::Plane(Plane {
            a: n[0],
            b: n[1],
            c: 0.0,
            d: apothem,
            bc: BoundaryType::Reflective,
        }));
    }
    surfaces.push(SurfaceKind::ZPlane(ZPlane {
        z0: -0.5 * h,
        bc: BoundaryType::Reflective,
    }));
    surfaces.push(SurfaceKind::ZPlane(ZPlane {
        z0: 0.5 * h,
        bc: BoundaryType::Reflective,
    }));
    // Cells 0..23: the pieces. Cell 23: helium outside every ball.
    let mut cells: Vec<Cell> = sites
        .iter()
        .enumerate()
        .map(|(i, (_, ball))| {
            let mat = if fuel[*ball] { fuel_m } else { mod_m };
            Cell::material(i as i32 + 1, vec![half(i, HalfSpaceSense::Inside)], mat, T)
        })
        .collect();
    let mut region = vec![
        half(0, HalfSpaceSense::Outside),
        half(1, HalfSpaceSense::Outside),
        RegionToken::Intersection,
    ];
    for i in 2..sites.len() {
        region.push(half(i, HalfSpaceSense::Outside));
        region.push(RegionToken::Intersection);
    }
    cells.push(Cell::material(24, region, gas_m, T));
    let tile = Universe {
        id: 0,
        cell_indices: (0..24).collect(),
    };
    // The outer universe (never reached), and the container.
    cells.push(Cell::material(
        25,
        vec![
            half(first_plane, HalfSpaceSense::Inside),
            half(first_plane, HalfSpaceSense::Outside),
            RegionToken::Union,
        ],
        gas_m,
        T,
    ));
    let outer = Universe {
        id: 1,
        cell_indices: vec![24],
    };
    let mut inside = vec![
        half(first_plane, HalfSpaceSense::Inside),
        half(first_plane + 1, HalfSpaceSense::Inside),
        RegionToken::Intersection,
    ];
    for k in 2..6 {
        inside.push(half(first_plane + k, HalfSpaceSense::Inside));
        inside.push(RegionToken::Intersection);
    }
    inside.push(half(first_plane + 6, HalfSpaceSense::Outside));
    inside.push(RegionToken::Intersection);
    inside.push(half(first_plane + 7, HalfSpaceSense::Inside));
    inside.push(RegionToken::Intersection);
    cells.push(Cell::fill(26, inside, CellFill::Lattice(0), Position::ZERO));
    let root = Universe {
        id: 2,
        cell_indices: vec![25],
    };
    let lattice = HexLattice::from_rings(
        1,
        HexOrientation::Y,
        Position::ZERO,
        2.0 * apothem,
        &[vec![0]],
        Some(1),
    );
    Geometry {
        surfaces,
        cells,
        universes: vec![tile, outer, root],
        lattices: vec![CsgLattice::Hex(lattice)],
        root_universe: 2,
    }
}

#[test]
fn the_beginner_layer_lowers_to_the_hand_built_hexagonal_lattice() {
    let (mats, mix) = small_materials();
    let fuel = seker::fuel_flags(seker::FUEL_BALL_FRACTION);
    let layer = seker::build(
        header_only_library(),
        &mats,
        &seker::FuelBall::Smeared(mix),
        &fuel,
        T,
    );
    let a = layer.model.assemble().expect("assembles");
    let hand = hand_built(&a, &fuel);

    // ── 2. structure ───────────────────────────────────────────────────────
    assert_eq!(
        a.geometry.surfaces.len(),
        hand.surfaces.len(),
        "surface count"
    );
    assert_eq!(a.geometry.cells.len(), hand.cells.len(), "cell count");
    assert_eq!(a.geometry.universes.len(), hand.universes.len());
    assert_eq!(a.geometry.lattices.len(), 1);
    // The 23 ball surfaces are nee_soon's 23 sites (as a set).
    let spheres = |g: &Geometry| -> Vec<[f64; 4]> {
        let mut v: Vec<[f64; 4]> = g
            .surfaces
            .iter()
            .filter_map(|s| match s {
                SurfaceKind::Sphere(p) => Some([p.x0, p.y0, p.z0, p.r]),
                _ => None,
            })
            .collect();
        v.sort_by(|p, q| {
            p.iter()
                .zip(q)
                .map(|(x, y)| ((x * 1e6).round() as i64).cmp(&((y * 1e6).round() as i64)))
                .find(|o| o.is_ne())
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        v
    };
    let (ours, theirs) = (spheres(&a.geometry), spheres(&hand));
    assert_eq!(ours.len(), 23);
    for (p, q) in ours.iter().zip(&theirs) {
        for k in 0..4 {
            assert!((p[k] - q[k]).abs() < 1e-9, "{p:?} against {q:?}");
        }
    }
    // The container: six unit planes at the apothem, 30 degrees apart, and two
    // ends, all reflective.
    let planes: Vec<&Plane> = a
        .geometry
        .surfaces
        .iter()
        .filter_map(|s| match s {
            SurfaceKind::Plane(p) => Some(p),
            _ => None,
        })
        .collect();
    assert_eq!(planes.len(), 6);
    let mut angles: Vec<f64> = planes
        .iter()
        .map(|p| {
            assert!((p.a.hypot(p.b) - 1.0).abs() < 1e-15 && p.c == 0.0);
            assert!((p.d - seker::apothem_cm()).abs() < 1e-12, "apothem");
            assert_eq!(p.bc, BoundaryType::Reflective);
            p.b.atan2(p.a).to_degrees().rem_euclid(360.0)
        })
        .collect();
    angles.sort_by(f64::total_cmp);
    for (k, ang) in angles.iter().enumerate() {
        assert!(
            (ang - (30.0 + 60.0 * k as f64)).abs() < 1e-9,
            "face {k} at {ang} degrees"
        );
    }
    let ends: Vec<f64> = a
        .geometry
        .surfaces
        .iter()
        .filter_map(|s| match s {
            SurfaceKind::ZPlane(p) => {
                assert_eq!(p.bc, BoundaryType::Reflective);
                Some(p.z0)
            }
            _ => None,
        })
        .collect();
    assert_eq!(ends.len(), 2);
    assert!((ends[0] + 0.5 * seker::height_cm()).abs() < 1e-12);
    assert!((ends[1] - 0.5 * seker::height_cm()).abs() < 1e-12);
    match &a.geometry.lattices[0] {
        CsgLattice::Hex(l) => {
            assert_eq!((l.n_rings, l.n_axial), (1, 1));
            assert_eq!(l.orientation, HexOrientation::Y);
            assert_eq!(l.pitch[0].to_bits(), (2.0 * seker::apothem_cm()).to_bits());
            assert_eq!(l.universes.len(), 1);
        }
        CsgLattice::Rect(_) => panic!("a hexagonal lattice"),
    }
    let delta = a
        .geometry
        .cells
        .iter()
        .filter(|c| matches!(c.tracking, Some(TrackingMethod::Delta { .. })))
        .count();
    assert_eq!(delta, 0, "the smeared balls are surface-tracked");

    // ── 3. lookups ─────────────────────────────────────────────────────────
    let names = |g: &Geometry, p: Position| {
        g.locate(p, Direction::new(0.0, 0.0, 1.0), SurfaceToken::NONE)
            .and_then(|path| path.material)
    };
    let ours_names: Vec<String> = a.materials.iter().map(|m| m.name.clone()).collect();
    let mut seed = 20_261_018_u64;
    let mut prn = move || {
        seed = seed
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        ((seed >> 11) as f64) / ((1u64 << 53) as f64)
    };
    let (apothem, h) = (seker::apothem_cm(), seker::height_cm());
    let r_circ = apothem / (std::f64::consts::PI / 6.0).cos();
    let (mut inside, mut outside, mut in_ball) = (0_usize, 0_usize, 0_usize);
    for i in 0..300_000 {
        let (x, y, z) = (
            (2.0 * prn() - 1.0) * r_circ * 1.05,
            (2.0 * prn() - 1.0) * apothem * 1.05,
            (2.0 * prn() - 1.0) * h * 0.55,
        );
        let p = Position::new(x, y, z);
        let (g, w) = (names(&a.geometry, p), names(&hand, p));
        assert_eq!(
            g.map(|m| ours_names[m].clone()),
            w.map(|m| ours_names[m].clone()),
            "point {i} ({x}, {y}, {z})"
        );
        if g.is_some() {
            inside += 1;
            if g.map(|m| ours_names[m].as_str()) != Some("gas") {
                in_ball += 1;
            }
        } else {
            outside += 1;
        }
    }
    println!(
        "lookups: 300000 points agree ({inside} in the layer, {in_ball} of them in a ball piece, {outside} outside)"
    );
    assert!(inside > 100_000 && outside > 10_000 && in_ball > 40_000);

    // ── 4. transport: k in every generation, bit for bit ───────────────────
    let nuclides = low_tier(&a);
    let (n, inactive, active, seed) = (300, 4, 8, 11);
    let low = LowKeffSettings {
        n_particles: n,
        n_inactive: inactive,
        n_active: active,
        temperature_k: T,
        seed,
        ..LowKeffSettings::default()
    };
    let reference_seq = run_keff_csg(&hand, &a.materials, &nuclides, a.source_box, &low, None);
    let reference = run_keff_csg(
        &hand,
        &a.materials,
        &nuclides,
        a.source_box,
        &LowKeffSettings {
            compute: ComputeType::CpuMultiThread(ThreadCount::Fixed(3)),
            ..low.clone()
        },
        None,
    );
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
    // No neutron was lost to a corner or a face of the hexagon.
    assert!(run.k_by_cycle.iter().all(|k| k.is_finite() && *k > 0.0));
}

/// Which of the 23 sites' balls holds `p` (strictly inside), if any, and its
/// distance from the centre.
fn ball_at(p: [f64; 3]) -> Option<(usize, f64)> {
    nee_sites().iter().enumerate().find_map(|(i, (s, _))| {
        let c = nee_centre(*s);
        let d = ((p[0] - c[0]).powi(2) + (p[1] - c[1]).powi(2) + (p[2] - c[2]).powi(2)).sqrt();
        (d < 3.0).then_some((i, d))
    })
}

#[test]
fn the_triso_layer_counts_what_the_paper_counts_and_matches_the_site_table() {
    let data =
        NuclearData::from_folder(&njoy_outram_park_fork::reference_data::reference_endf_dir())
            .expect("reference-data/endf is tracked");
    let mats = seker::materials(T);
    let fuel = seker::fuel_flags(seker::FUEL_BALL_FRACTION);
    let layer = seker::build(data, &mats, &seker::FuelBall::Triso, &fuel, T);
    let c = layer.components;
    let a = layer.model.assemble().expect("assembles");

    // ── counted from the geometry ──────────────────────────────────────────
    let census = a.census().expect("census");
    let (fuel_balls, moderator_balls) = (census.copies(c.shell), census.copies(c.moderator));
    let particles: usize = census.packings.iter().map(|p| p.particles).sum();
    let tile_v = census.total_volume().get::<cubic_centimeter>();
    let helium_v = census.volume(c.helium).get::<cubic_centimeter>();
    let packing = 1.0 - helium_v / tile_v;
    println!(
        "census: {fuel_balls} fuel + {moderator_balls} moderator balls, {particles} particles, packing {packing:.6} of the tile ({tile_v:.4} cm3)"
    );
    assert_eq!((fuel_balls, moderator_balls), (7, 6));
    assert_eq!(particles, 7 * seker::PARTICLES_PER_PEBBLE);
    assert_eq!(census.packings.len(), 7, "one packed zone per fuel ball");
    let ball_v = 4.0 / 3.0 * std::f64::consts::PI * 27.0;
    let tile_expected = 2.0 * 3.0_f64.sqrt() * seker::apothem_cm().powi(2) * seker::height_cm();
    assert!((tile_v / tile_expected - 1.0).abs() < 1e-12, "{tile_v}");
    assert!((packing - 13.0 * ball_v / tile_v).abs() < 1e-9, "{packing}");
    assert!((packing - seker::SEKER_INTERIOR_PACKING).abs() < 5e-5);
    // Ten pieces of balls come from the neighbours and the layer above: 10
    // more fuel-or-moderator cells that the census leaves out, and 10 more
    // packed zones of which the fuel ones are pieces too.
    let ball_spheres = a
        .geometry
        .surfaces
        .iter()
        .filter(|s| matches!(s, SurfaceKind::Sphere(p) if (p.r - 3.0).abs() < 1e-12))
        .count();
    assert_eq!(ball_spheres, 23);
    // The delta-tracked zones: one per fuel piece (the 7 fuel balls own 7;
    // the pieces from outside are fuel where the ball is).
    let fuel_pieces = nee_sites().iter().filter(|(_, b)| fuel[*b]).count();
    assert_eq!(a.delta_regions().len(), fuel_pieces);
    println!("{fuel_pieces} fuel pieces of the 23 are delta-tracked zones");

    // ── lookups against the site table alone ───────────────────────────────
    let names: Vec<String> = a.resolved.iter().map(|r| r.name.clone()).collect();
    let find = |p: [f64; 3]| {
        a.geometry
            .locate(
                Position::new(p[0], p[1], p[2]),
                Direction::new(0.0, 0.0, 1.0),
                SurfaceToken::NONE,
            )
            .and_then(|path| path.material)
            .map(|m| names[m].as_str())
    };
    let mut seed = 20_261_019_u64;
    let mut prn = move || {
        seed = seed
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        ((seed >> 11) as f64) / ((1u64 << 53) as f64)
    };
    let (apothem, h) = (seker::apothem_cm(), seker::height_cm());
    let r_circ = apothem / (std::f64::consts::PI / 6.0).cos();
    let sites = nee_sites();
    let tile_class = |p: [f64; 3]| -> Option<&'static str> {
        // Wholly inside the prism? (the geometry is bounded by it)
        let n = [
            [0.0, 1.0],
            [0.866_025_403_784_438_6, 0.5],
            [-0.866_025_403_784_438_6, 0.5],
        ];
        if n.iter()
            .any(|v| (v[0] * p[0] + v[1] * p[1]).abs() >= apothem)
            || p[2].abs() >= 0.5 * h
        {
            return None;
        }
        Some(match ball_at(p) {
            None => "helium",
            Some((i, d)) => {
                let fuel_ball = fuel[sites[i].1];
                if !fuel_ball || d >= seker::FUEL_ZONE_R_CM {
                    "shell graphite"
                } else {
                    "zone"
                }
            }
        })
    };
    let (mut tested, mut zone_points, mut shell_points, mut helium_points) = (0, 0, 0, 0);
    for i in 0..200_000 {
        let p = [
            (2.0 * prn() - 1.0) * r_circ,
            (2.0 * prn() - 1.0) * apothem,
            (2.0 * prn() - 1.0) * 0.5 * h,
        ];
        let Some(class) = tile_class(p) else { continue };
        let got = find(p).unwrap_or("lost");
        match class {
            "zone" => {
                zone_points += 1;
                assert!(
                    matches!(
                        got,
                        "matrix graphite"
                            | "UO2 kernel"
                            | "buffer carbon"
                            | "pyrolytic carbon"
                            | "silicon carbide"
                    ),
                    "point {i} {p:?}: a fuel zone, found {got}"
                );
            }
            "shell graphite" => {
                shell_points += 1;
                assert_eq!(got, "shell graphite", "point {i} {p:?}");
            }
            _ => {
                helium_points += 1;
                assert_eq!(got, "helium", "point {i} {p:?}");
            }
        }
        tested += 1;
    }
    println!(
        "lookups against the 23-site table: {tested} points agree ({zone_points} in a fuel zone, {shell_points} in graphite, {helium_points} in helium)"
    );
    assert!(tested > 120_000 && zone_points > 5_000 && shell_points > 20_000);
}

/// **A delta-tracked zone cut by the layer's own faces loses no history**
/// (found 2026-10-18 writing tutorial 7). The flower balls are cut through
/// their centres by the reflective top and bottom, and the balls that cross a
/// side face are cut by it, so the fuel zone of such a ball extends past the
/// model. `fly_delta_region` sampled virtual-collision sites along the whole
/// flight, found none placeable past the face and scored the history a leak:
/// k came out 0.005 and the population died in the third generation (real
/// data, Smoke). Sites past the nearest surface cannot matter (a collision
/// there is discarded) and are now treated as void. Pinned here on the LOW
/// tier with the TRISO fuel ball: every generation keeps its population and k
/// is of order 1 rather than of order 0.01.
#[test]
fn a_delta_zone_cut_by_the_layer_faces_does_not_lose_histories() {
    let (mats, _) = small_materials();
    let fuel = seker::fuel_flags(seker::FUEL_BALL_FRACTION);
    let layer = seker::build(
        header_only_library(),
        &mats,
        &seker::FuelBall::Triso,
        &fuel,
        T,
    );
    let a = layer.model.assemble().expect("assembles");
    assert!(!a.delta_regions().is_empty());
    let nuclides = low_tier(&a);
    let custom = KeffSettings::new(
        MCSettings::Custom(CustomRun {
            particles: 400,
            inactive: 3,
            active: 5,
            seed: 7,
        }),
        None,
    );
    let run = a
        .keff_with(&nuclides, &custom)
        .expect("the population lives");
    println!("k per generation: {:?}", run.k_by_cycle);
    assert!(
        run.k_by_cycle.iter().all(|k| *k > 0.3),
        "{:?}",
        run.k_by_cycle
    );
}
