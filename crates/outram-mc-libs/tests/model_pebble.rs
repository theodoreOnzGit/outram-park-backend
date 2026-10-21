//! **A pebble's fuel zone in the beginner API lowers to the existing
//! builders, bit for bit** (gh #926, tutorial 5 on #943).
//!
//! # Methodology
//!
//! Each case builds a small pebble (a 0.8 cm fuel zone of HTR-10 TRISO
//! particles, `TrisoRadii::HTR10`, in a 1.0 cm reflective graphite shell)
//! twice: through `model::Model`, and by hand from the crate's existing
//! builders. Then, as `tests/model_repeated_structures.rs` does:
//!
//! 1. **Structure**: every surface (`Debug`, exact `f64`), cell region, fill,
//!    translation and tracking, universe cell list, lattice and root are
//!    compared for equality (integer ids excepted).
//! 2. **Transport**: the hand-built geometry through the ~~single-thread~~
//!    CSG driver (`run_keff_csg`, or `run_keff_csg_hybrid` with the same
//!    majorants for a delta-tracked zone) against the beginner run
//!    (`keff_with`, `MCSettings::Custom`) at the same seed: the same `k` in
//!    every generation, to the last bit, on both backends (**since
//!    2026-10-18, gh #955**: the default multi-thread run against
//!    `ComputeType::CpuMultiThread`, `.single_thread()` against
//!    `ComputeType::CpuSingleThread`). Data: the embedded LOW tier on both
//!    sides (this tests the geometry, not the data).
//! 3. **Counting from the geometry** (gh:#316): the census's particle count
//!    against the existing packers' own lists, and the packing fraction it
//!    realises against the one asked for.
//!
//! The hand-built paths:
//!
//! - **Regular lattice**: `build_triso_particle` (the TRISO universe), the
//!   covering grid and whole-particle keep rule of the HTR-10 core's TRISO
//!   lattice (`nee_soon::htr10_rmc::core_shell::TrisoBuild::new` and
//!   `::lattice`; written out here because `nee_soon` depends on this crate,
//!   not the other way), an empty matrix universe as `outer`. Its particle
//!   count must equal `sphere_packing::cubic_array_in_ball`'s.
//! - **Random**: `dh_universe::pack_in_ball` (the packing `DhUniverse::pebble`
//!   uses), `build_triso_particle`, and `pebble_beds::triso_lattice::
//!   create_triso_lattice` (OpenMC's `create_triso_lattice`), with the zone's
//!   cell delta-tracked. And an **independent** check that shares no
//!   lowering code: `DhUniverse::pebble` with the same seed and fraction must
//!   answer the same material as the lowered CSG at every one of 20 000
//!   random points.
//!
//! # Results (2026-10-18)
//!
//! All pass. Regular: structure equal, per-generation `k` identical, 267
//! particles = `cubic_array_in_ball`'s count. Random: 272 particles drawn as
//! 836 tile copies, each counted once. Random: structure equal,
//! per-generation `k` identical with delta tracking, and 20 000 of 20 000
//! points agree with `DhUniverse`'s own geometry. The counts and fractions
//! are printed by the tests; the `k` values are not physics results (LOW
//! tier, tiny runs).

use petir::mathf::RealMath;
use outram_mc_libs::dh_universe::{pack_in_ball, DhTreatment, DhUniverse, PebbleParams};
use outram_mc_libs::geometry::cell::{
    Cell, CellFill, HalfSpaceSense, RegionToken, SurfaceToken, TrackingMethod,
};
use outram_mc_libs::geometry::geometry::Geometry;
use outram_mc_libs::geometry::lattice::{Lattice as CsgLattice, RectLattice};
use outram_mc_libs::geometry::position::{Direction, Position};
use outram_mc_libs::geometry::surface::{BoundaryType, Sphere as CsgSphere, SurfaceKind, XPlane};
use outram_mc_libs::geometry::triso_particle::{build_triso_particle, TrisoMaterials, TrisoRadii};
use outram_mc_libs::geometry::universe::Universe;
use outram_mc_libs::geometry::volume_calc::{BoundingBox, VolumeCalculation, VolumeDomain};
use outram_mc_libs::material::nuclide::Nuclide;
use outram_mc_libs::model::prelude::*;
use outram_mc_libs::model::{Arrangement, RowKind};
use outram_mc_libs::pebble_beds::fhr_pebble::TrisoSpec;
use outram_mc_libs::pebble_beds::sphere_packing::cubic_array_in_ball;
use outram_mc_libs::pebble_beds::triso_lattice::{create_triso_lattice, TrisoLatticeBase};
use outram_mc_libs::physics::keff::KeffSettings as LowKeffSettings;
use outram_mc_libs::physics::keff::{ComputeType, ThreadCount};
use outram_mc_libs::physics::transport_csg::{run_keff_csg, run_keff_csg_hybrid};
use uom::si::f64::{Length, Ratio, ThermodynamicTemperature};
use uom::si::length::centimeter;
use uom::si::ratio::ratio;
use uom::si::thermodynamic_temperature::kelvin;
use uom::si::volume::cubic_centimeter;

const T: f64 = 293.6;
const R_ZONE: f64 = 0.8;
const R_PEBBLE: f64 = 1.0;
const R_P: f64 = 0.0455;
const OFFSET: [f64; 3] = [0.13, 0.37, 0.71];
const PITCH: f64 = 0.19;
const PF: f64 = 0.05;
const SEED: u64 = 4_243;

fn cm(x: f64) -> Length {
    Length::new::<centimeter>(x)
}

/// A library of incident-neutron headers (masses only): enough to assemble.
fn header_only_library() -> NuclearData {
    let endf = |x: f64| {
        let e = x.abs().r_log10().floor() as i32;
        format!("{:.6}{e:+}", x / 10f64.r_powi(e))
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
            ("si28".to_string(), tape(14028.0, 27.737450)),
        ],
    )
    .expect("five incident-neutron headers")
}

struct Mats {
    uo2: Material,
    buffer: Material,
    pyc: Material,
    sic: Material,
    matrix: Material,
    shell: Material,
}

/// Illustrative densities (atoms per barn-cm); low-level indices in first-use
/// order: kernel 0, buffer 1, PyC 2, SiC 3, matrix 4, shell 5.
fn mats() -> Mats {
    let d = atoms_per_barn_cm;
    Mats {
        uo2: Material::from_atom_densities([(U235, d(0.0047)), (U238, d(0.0188)), (O16, d(0.047))])
            .named("UO2"),
        buffer: Material::from_atom_densities([(C12, d(0.0551))]).named("buffer"),
        pyc: Material::from_atom_densities([(C12, d(0.0953))]).named("PyC"),
        sic: Material::from_atom_densities([(Si28, d(0.0478)), (C12, d(0.0478))]).named("SiC"),
        matrix: Material::from_atom_densities([(C12, d(0.0867))]).named("matrix"),
        shell: Material::from_atom_densities([(C12, d(0.0870))]).named("shell"),
    }
}

const HAND_MATS: TrisoMaterials = TrisoMaterials {
    kernel: 0,
    buffer: 1,
    ipyc: 2,
    sic: 3,
    opyc: 2,
    matrix: 4,
};

fn particle(m: &Mats) -> Shape {
    let r = TrisoRadii::HTR10;
    let kernel = Sphere::radius(cm(r.kernel)).filled_with(m.uo2.clone());
    let buffer = Sphere::radius(cm(r.buffer))
        .filled_with(m.buffer.clone())
        .containing(kernel);
    let ipyc = Sphere::radius(cm(r.ipyc))
        .filled_with(m.pyc.clone())
        .containing(buffer);
    let sic = Sphere::radius(cm(r.sic))
        .filled_with(m.sic.clone())
        .containing(ipyc);
    Sphere::radius(cm(r.opyc))
        .filled_with(m.pyc.clone())
        .containing(sic)
}

fn model_with(shape: Shape) -> Model {
    let mut m = Model::new(header_only_library());
    m.add(shape);
    m.set_temperature(Temperature::Uniform(
        ThermodynamicTemperature::new::<kelvin>(T),
    ));
    m
}

fn pebble(m: &Mats, zone: impl Into<Shape>) -> Shape {
    Sphere::radius(cm(R_PEBBLE))
        .filled_with(m.shell.clone())
        .containing(zone)
        .boundary(Boundary::Reflective)
}

fn half(s: usize, sense: HalfSpaceSense) -> RegionToken {
    RegionToken::HalfSpace {
        surface_idx: s,
        sense,
    }
}

fn sphere(r: f64, bc: BoundaryType) -> SurfaceKind {
    SurfaceKind::Sphere(CsgSphere {
        x0: 0.0,
        y0: 0.0,
        z0: 0.0,
        r,
        bc,
    })
}

/// `nee_soon::htr10_rmc::core_shell::TrisoBuild::new`'s `triso_axis`: tiles
/// per axis and the lower edge of a grid covering `[-r, r]`.
fn covering_axis(r: f64, pitch: f64, off: f64) -> (usize, f64) {
    let k_lo = (-r / pitch - off + 0.5).floor();
    let k_hi = (r / pitch - off - 0.5).ceil();
    ((k_hi - k_lo) as usize + 1, (k_lo + off - 0.5) * pitch)
}

fn assert_same_geometry(ours: &Geometry, hand: &Geometry) {
    assert_eq!(ours.surfaces.len(), hand.surfaces.len(), "surface count");
    for (i, (a, b)) in ours.surfaces.iter().zip(&hand.surfaces).enumerate() {
        assert_eq!(format!("{a:?}"), format!("{b:?}"), "surface {i}");
    }
    assert_eq!(ours.cells.len(), hand.cells.len(), "cell count");
    for (i, (a, b)) in ours.cells.iter().zip(&hand.cells).enumerate() {
        assert_eq!(a.region, b.region, "cell {i} region");
        assert_eq!(a.fill, b.fill, "cell {i} fill");
        assert_eq!(a.temperature.to_bits(), b.temperature.to_bits());
        assert_eq!(
            format!("{:?}", a.translation),
            format!("{:?}", b.translation),
            "cell {i} translation"
        );
        assert_eq!(a.tracking, b.tracking, "cell {i} tracking");
    }
    let lists = |g: &Geometry| {
        g.universes
            .iter()
            .map(|u| u.cell_indices.clone())
            .collect::<Vec<_>>()
    };
    assert_eq!(lists(ours), lists(hand), "universe cell lists");
    assert_eq!(ours.root_universe, hand.root_universe, "root");
    assert_eq!(ours.lattices.len(), hand.lattices.len(), "lattice count");
    for (a, b) in ours.lattices.iter().zip(&hand.lattices) {
        match (a, b) {
            (CsgLattice::Rect(a), CsgLattice::Rect(b)) => {
                assert_eq!(a.n, b.n);
                assert_eq!(format!("{:?}", a.lower_left), format!("{:?}", b.lower_left));
                assert_eq!(a.pitch.map(f64::to_bits), b.pitch.map(f64::to_bits));
                assert_eq!(a.universes, b.universes, "tile universes");
                assert_eq!(a.outer, b.outer);
            }
            _ => panic!("rect lattices expected"),
        }
    }
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

/// The hand-built geometry through the low-level driver (hybrid when the
/// model has delta regions, with the model's own majorants) against the
/// beginner run: identical `k` in every generation.
fn assert_same_k(a: &AssembledModel, hand: &Geometry) {
    let nuclides = low_tier(a);
    let (n, inactive, active, seed) = (300, 4, 8, 11);
    let low = LowKeffSettings {
        n_particles: n,
        n_inactive: inactive,
        n_active: active,
        temperature_k: T,
        seed,
        ..LowKeffSettings::default()
    };
    let majorants = a.majorants(&nuclides);
    let reference_on = |low: &LowKeffSettings| {
        if majorants.is_empty() {
            run_keff_csg(hand, &a.materials, &nuclides, a.source_box, low, None)
        } else {
            run_keff_csg_hybrid(
                hand,
                &a.materials,
                &nuclides,
                &majorants,
                None,
                a.source_box,
                low,
                None,
            )
        }
    };
    let reference = reference_on(&LowKeffSettings {
        compute: ComputeType::CpuMultiThread(ThreadCount::Fixed(3)),
        ..low.clone()
    });
    let reference_seq = reference_on(&low);
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
    assert_eq!(
        single.provenance.backend.contains("hybrid"),
        !majorants.is_empty()
    );
    assert_eq!(
        run.provenance.backend.contains("hybrid"),
        !majorants.is_empty(),
        "the provenance names the backend that ran"
    );
    println!(
        "bit for bit: model {} vs hand-built {:.5} ± {:.5} ({} majorants)",
        run.keff(),
        reference.k_mean,
        reference.k_std,
        majorants.len()
    );
}

/// The surfaces and cells every hand-built pebble ends with: the zone's
/// sphere and fill cell, then the reflective shell and its cell.
fn close_pebble(
    surfaces: &mut Vec<SurfaceKind>,
    cells: &mut Vec<Cell>,
    tracking: Option<TrackingMethod>,
) -> (usize, usize) {
    surfaces.push(sphere(R_ZONE, BoundaryType::Transmissive));
    let zone = surfaces.len() - 1;
    surfaces.push(sphere(R_PEBBLE, BoundaryType::Reflective));
    let shell = surfaces.len() - 1;
    let mut fill = Cell::fill(
        0,
        vec![half(zone, HalfSpaceSense::Inside)],
        CellFill::Lattice(0),
        Position::ZERO,
    );
    fill.tracking = tracking;
    cells.push(fill);
    cells.push(Cell::material(
        0,
        vec![
            half(zone, HalfSpaceSense::Outside),
            half(shell, HalfSpaceSense::Inside),
            RegionToken::Intersection,
        ],
        5,
        T,
    ));
    (cells.len() - 2, cells.len() - 1)
}

/// The regular lattice of the HTR-10 core, by hand, in a 0.8 cm zone.
fn hand_regular(tracking: Option<TrackingMethod>) -> (Geometry, usize) {
    let triso = build_triso_particle(Position::ZERO, TrisoRadii::HTR10, HAND_MATS, T, 1, 0, 0);
    let mut surfaces = triso.surfaces.clone();
    surfaces.push(SurfaceKind::XPlane(XPlane {
        x0: 1.0e30,
        bc: BoundaryType::Transmissive,
    }));
    let mut cells = triso.cells.clone();
    cells.push(Cell::material(
        0,
        vec![
            half(5, HalfSpaceSense::Inside),
            half(5, HalfSpaceSense::Outside),
            RegionToken::Union,
        ],
        HAND_MATS.matrix,
        T,
    ));
    let axes = OFFSET.map(|o| covering_axis(R_ZONE, PITCH, o));
    // TrisoBuild::lattice's loop: keep a particle where it lies wholly in.
    let r_keep = R_ZONE - R_P;
    let c = |lo: f64, n: usize| lo + (n as f64 + 0.5) * PITCH;
    let mut tiles = Vec::new();
    for k in 0..axes[2].0 {
        for j in 0..axes[1].0 {
            for i in 0..axes[0].0 {
                let (x, y, z) = (c(axes[0].1, i), c(axes[1].1, j), c(axes[2].1, k));
                tiles.push(if x * x + y * y + z * z <= r_keep * r_keep {
                    0
                } else {
                    1
                });
            }
        }
    }
    let kept = tiles.iter().filter(|&&u| u == 0).count();
    let (fill, shell) = close_pebble(&mut surfaces, &mut cells, tracking);
    let g = Geometry {
        surfaces,
        cells,
        universes: vec![
            triso.universe.clone(),
            Universe {
                id: 1,
                cell_indices: vec![6],
            },
            Universe {
                id: 2,
                cell_indices: vec![fill, shell],
            },
        ],
        lattices: vec![CsgLattice::Rect(RectLattice {
            id: 1,
            n: axes.map(|a| a.0),
            lower_left: Position::new(axes[0].1, axes[1].1, axes[2].1),
            pitch: [PITCH; 3],
            universes: tiles,
            outer: Some(1),
        })],
        root_universe: 2,
    };
    (g, kept)
}

/// **A lattice in a sphere, whole-particle rejection**: the HTR-10 core's
/// TRISO lattice, bit for bit, and its census.
#[test]
fn lattice_in_a_sphere_lowers_to_the_htr10_core_lattice() {
    let m = mats();
    let tile = Cube::side(cm(PITCH))
        .filled_with(m.matrix.clone())
        .containing(particle(&m));
    let zone = Sphere::radius(cm(R_ZONE))
        .filled_with(m.matrix.clone())
        .containing_lattice(Lattice::covering(tile, OFFSET));
    let a = model_with(pebble(&m, zone)).assemble().expect("assembles");
    let (hand, kept) = hand_regular(None);
    assert_same_geometry(&a.geometry, &hand);
    assert!(
        a.delta_regions().is_empty(),
        "surface tracked: no delta region"
    );
    assert_same_k(&a, &hand);

    // Counted from the geometry, against the existing packer's own list.
    let census = a.census().expect("census");
    println!("{census}");
    let expected = cubic_array_in_ball(R_P, R_ZONE, PITCH, OFFSET).len();
    let p = &census.packings[0];
    assert_eq!((p.particles, kept), (expected, expected));
    assert_eq!(census.rows[0].copies, expected, "kernels placed");
    let v_p = 4.0 / 3.0 * std::f64::consts::PI * R_P.r_powi(3);
    let v_zone = 4.0 / 3.0 * std::f64::consts::PI * R_ZONE.r_powi(3);
    let realised = p.realised.get::<ratio>();
    assert!((realised / (expected as f64 * v_p / v_zone) - 1.0).abs() < 1e-12);
    assert!((p.requested.get::<ratio>() / (v_p / PITCH.r_powi(3)) - 1.0).abs() < 1e-12);
    assert!(matches!(
        p.arrangement,
        Arrangement::RegularLattice {
            offset: Some(OFFSET),
            ..
        }
    ));
    println!(
        "regular: {} particles, realised {:.6} %, asked (infinite array) {:.6} %",
        p.particles,
        100.0 * realised,
        100.0 * p.requested.get::<ratio>()
    );
    // The pool carries the matrix exactly; the whole pebble's volume closes.
    let pool = census
        .rows
        .iter()
        .find(|r| r.kind == RowKind::Pool)
        .unwrap();
    let pool_v = pool.volume().get::<cubic_centimeter>();
    assert!((pool_v / (v_zone - expected as f64 * v_p) - 1.0).abs() < 1e-12);
    assert!(census
        .rows
        .iter()
        .filter(|r| matches!(r.kind, RowKind::Pooled { .. }))
        .all(|r| r.volume().get::<cubic_centimeter>() == 0.0));
    let v_pebble = 4.0 / 3.0 * std::f64::consts::PI * R_PEBBLE.r_powi(3);
    let total = census.total_volume().get::<cubic_centimeter>();
    assert!((total / v_pebble - 1.0).abs() < 1e-12, "{total}");

    // The kernel volume against an independent stochastic estimate.
    let calc = VolumeCalculation {
        domain: VolumeDomain::Material,
        ids: vec![0],
        box_: BoundingBox {
            lower: Position::new(-R_PEBBLE, -R_PEBBLE, -R_PEBBLE),
            upper: Position::new(R_PEBBLE, R_PEBBLE, R_PEBBLE),
        },
        n_samples: 2_000_000,
        master_seed: 926,
    };
    let (v, _) = calc.execute(&a.geometry);
    let exact = census.rows[0].volume().get::<cubic_centimeter>();
    let z = (v[0].volume - exact) / v[0].std_dev;
    println!(
        "kernel volume: census {exact:.6e} cm3, stochastic {:.6e} ± {:.1e} ({z:+.2} sigma)",
        v[0].volume, v[0].std_dev
    );
    assert!(z.abs() < 4.0, "{z}");

    // The same grid given by size (copies_of, odd, centred: a tile at the
    // centre) is cubic_array_in_ball at offset 0.
    let tile = Cube::side(cm(PITCH))
        .filled_with(m.matrix.clone())
        .containing(particle(&m));
    let n = 2 * (R_ZONE / PITCH).ceil() as usize + 1;
    let zone = Sphere::radius(cm(R_ZONE))
        .filled_with(m.matrix.clone())
        .containing_lattice(Lattice::copies_of(tile, [n, n, n]));
    let a = model_with(pebble(&m, zone)).assemble().unwrap();
    let got = a.census().unwrap().packings[0].particles;
    assert_eq!(got, cubic_array_in_ball(R_P, R_ZONE, PITCH, [0.0; 3]).len());
}

/// **A `DhRegion` with a lattice packing** is the same lattice, with the
/// zone's cell delta-tracked over the zone's materials.
#[test]
fn a_lattice_packed_dh_region_is_the_lattice_delta_tracked() {
    let m = mats();
    let zone = DhRegion::new(Sphere::radius(cm(R_ZONE)), m.matrix.clone())
        .with_particles(
            particle(&m),
            Packing::Lattice {
                pitch: cm(PITCH),
                offset: OFFSET,
            },
        )
        .delta_tracking();
    let a = model_with(pebble(&m, zone)).assemble().expect("assembles");
    let (hand, _) = hand_regular(Some(TrackingMethod::Delta { majorant: 0 }));
    assert_same_geometry(&a.geometry, &hand);
    assert_eq!(a.delta_regions(), &[vec![0, 1, 2, 3, 4]]);
    assert_same_k(&a, &hand);
    let census = a.census().unwrap();
    assert!(census.packings[0].delta_tracked);

    // Packing::LatticeOfCount places exactly the count asked for.
    let n = census.packings[0].particles;
    let zone = DhRegion::new(Sphere::radius(cm(R_ZONE)), m.matrix.clone())
        .with_particles(
            particle(&m),
            Packing::LatticeOfCount {
                particles: n,
                offset: OFFSET,
            },
        )
        .delta_tracking();
    let a = model_with(pebble(&m, zone)).assemble().expect("assembles");
    assert_eq!(a.census().unwrap().packings[0].particles, n);
}

/// The particle universe of the random packing: `build_triso_particle`'s, and
/// the tracking lattice OpenMC's `create_triso_lattice` builds over the
/// centres `pack_in_ball` gives.
fn hand_random() -> (Geometry, usize) {
    let packed = pack_in_ball(R_P, R_ZONE, PF, SEED).expect("packs");
    let centres: Vec<Position> = packed
        .packing
        .spheres()
        .iter()
        .map(|s| Position::new(s.center.x + 0.0, s.center.y + 0.0, s.center.z + 0.0))
        .collect();
    let triso = build_triso_particle(Position::ZERO, TrisoRadii::HTR10, HAND_MATS, T, 1, 0, 0);
    let v_p = 4.0 / 3.0 * std::f64::consts::PI * R_P.r_powi(3);
    let p = (v_p / PF).r_cbrt();
    let axes = [0.5; 3].map(|o| covering_axis(R_ZONE, p, o));
    let t = create_triso_lattice(
        &centres,
        R_P,
        0,
        Position::new(axes[0].1 + 0.0, axes[1].1 + 0.0, axes[2].1 + 0.0),
        [p; 3],
        axes.map(|a| a.0),
        HAND_MATS.matrix,
        T,
        TrisoLatticeBase {
            surface: 5,
            cell: 6,
            universe: 1,
        },
        1,
    );
    assert_eq!(t.partly_outside, 0);
    let mut surfaces = triso.surfaces.clone();
    surfaces.extend(t.surfaces);
    let mut cells = triso.cells.clone();
    cells.extend(t.cells);
    let (fill, shell) = close_pebble(
        &mut surfaces,
        &mut cells,
        Some(TrackingMethod::Delta { majorant: 0 }),
    );
    let mut universes = vec![triso.universe.clone()];
    universes.extend(t.universes);
    universes.push(Universe {
        id: 0,
        cell_indices: vec![fill, shell],
    });
    let root = universes.len() - 1;
    (
        Geometry {
            surfaces,
            cells,
            universes,
            lattices: vec![CsgLattice::Rect(t.lattice)],
            root_universe: root,
        },
        centres.len(),
    )
}

fn random_zone(m: &Mats) -> DhRegion {
    DhRegion::new(Sphere::radius(cm(R_ZONE)), m.matrix.clone())
        .with_particles(
            particle(m),
            Packing::Random {
                fraction: Ratio::new::<ratio>(PF),
                seed: SEED,
            },
        )
        .delta_tracking()
}

/// **Random packing in a `DhRegion`**: `pack_in_ball` + `build_triso_particle`
/// + `create_triso_lattice`, delta-tracked, bit for bit; counted once each
/// even where a particle is drawn in two tiles.
#[test]
fn random_packing_lowers_to_pack_in_ball_and_create_triso_lattice() {
    let m = mats();
    let a = model_with(pebble(&m, random_zone(&m)))
        .assemble()
        .expect("assembles");
    let (hand, n) = hand_random();
    assert_same_geometry(&a.geometry, &hand);
    assert_same_k(&a, &hand);

    let census = a.census().expect("census");
    let p = &census.packings[0];
    println!("{census}");
    assert_eq!(p.particles, n, "each particle once, cut by tiles or not");
    assert_eq!(census.rows[0].copies, n, "kernels");
    let copies_in_tiles = a
        .geometry
        .cells
        .iter()
        .filter(|c| c.fill == CellFill::Universe(0))
        .count();
    assert!(
        copies_in_tiles > n,
        "some particles straddle tile faces ({copies_in_tiles} copies of {n})"
    );
    let realised = p.realised.get::<ratio>();
    assert!((realised / PF - 1.0).abs() <= (2e-3_f64).max((R_P / R_ZONE).r_powi(3) / PF));
    assert_eq!(p.requested.get::<ratio>(), PF);
    assert!(matches!(
        p.arrangement,
        Arrangement::Random { seed: SEED, .. }
    ));
    println!(
        "random: {n} particles ({copies_in_tiles} tile copies), realised {:.6} %, asked {:.6} %",
        100.0 * realised,
        100.0 * PF
    );
    let v_pebble = 4.0 / 3.0 * std::f64::consts::PI * R_PEBBLE.r_powi(3);
    let total = census.total_volume().get::<cubic_centimeter>();
    assert!((total / v_pebble - 1.0).abs() < 1e-12);
}

/// **Independent check**: `DhUniverse::pebble` (its own packing call, its own
/// point query `ExplicitTrisoPebble::material_at`, no CSG) with the same
/// seed and fraction answers the same material as the lowered CSG pebble at
/// 20 000 random points.
#[test]
fn the_random_pebble_is_dh_universes_pebble_point_for_point() {
    let m = mats();
    let a = model_with(pebble(&m, random_zone(&m))).assemble().unwrap();
    let r = TrisoRadii::HTR10;
    let lm = &a.materials;
    // DhUniverse's order: kernel, buffer, IPyC, SiC, OPyC, matrix, shell.
    let dh_to_low = [0, 1, 2, 3, 2, 4, 5];
    let params = PebbleParams {
        spec: TrisoSpec {
            kernel: r.kernel,
            buffer: r.buffer,
            ipyc: r.ipyc,
            sic: r.sic,
            opyc: r.opyc,
            packing_fraction: PF,
        },
        fuel_zone_radius: R_ZONE,
        pebble_radius: R_PEBBLE,
        coolant_radius: None,
        materials: dh_to_low.iter().map(|&i| lm[i].clone()).collect(),
        seed: SEED,
    };
    let dh = DhUniverse::pebble(params, DhTreatment::DeltaTracking).expect("DH pebble");
    let census = a.census().unwrap();
    assert_eq!(dh.particle_count(), census.packings[0].particles);
    let realised = census.packings[0].realised.get::<ratio>();
    assert!((dh.packing_fraction() / realised - 1.0).abs() < 1e-12);
    let mut seed = 0x926_u64;
    let mut next = || {
        seed = seed
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        (seed >> 11) as f64 / (1u64 << 53) as f64
    };
    let (mut agree, mut in_particles) = (0, 0);
    for _ in 0..20_000 {
        let p = loop {
            let p = [2.0 * next() - 1.0, 2.0 * next() - 1.0, 2.0 * next() - 1.0];
            if p.iter().map(|x| x * x).sum::<f64>() < 1.0 {
                break Position::new(p[0] * R_PEBBLE, p[1] * R_PEBBLE, p[2] * R_PEBBLE);
            }
        };
        let csg = a
            .geometry
            .locate(p, Direction::new(1.0, 0.0, 0.0), SurfaceToken::NONE)
            .and_then(|q| q.material);
        let dh = dh.material_at(p).map(|i| dh_to_low[i]);
        assert_eq!(csg, dh, "material at {p:?}");
        agree += 1;
        if !matches!(csg, Some(4) | Some(5)) {
            in_particles += 1;
        }
    }
    println!("{agree} of 20000 points agree ({in_particles} inside particles)");
    assert!(in_particles > 100, "the sample reaches the particles");
}

/// Every bad input is refused with a reason, before any transport.
#[test]
fn bad_packings_are_refused_with_reasons() {
    let m = mats();
    let refuse = |zone: Shape, why: &str| {
        let e = model_with(pebble(&m, zone))
            .assemble()
            .err()
            .unwrap_or_else(|| panic!("should refuse: {why}"))
            .to_string();
        assert!(e.contains(why), "{e}");
    };
    let random = |pf: f64| {
        DhRegion::new(Sphere::radius(cm(R_ZONE)), m.matrix.clone()).with_particles(
            particle(&m),
            Packing::Random {
                fraction: Ratio::new::<ratio>(pf),
                seed: 1,
            },
        )
    };
    refuse(random(PF).into(), "needs its tracking");
    refuse(
        DhRegion::new(Sphere::radius(cm(R_ZONE)), m.matrix.clone())
            .delta_tracking()
            .into(),
        "needs its particles",
    );
    refuse(random(0.45).delta_tracking().into(), "unreachable");
    refuse(random(0.0).delta_tracking().into(), "unreachable");
    refuse(random(1e-6).delta_tracking().into(), "fewer than one");
    let big = Sphere::radius(cm(0.9)).filled_with(m.uo2.clone());
    refuse(
        DhRegion::new(Sphere::radius(cm(R_ZONE)), m.matrix.clone())
            .with_particles(
                big,
                Packing::Random {
                    fraction: Ratio::new::<ratio>(PF),
                    seed: 1,
                },
            )
            .delta_tracking()
            .into(),
        "larger than the region",
    );
    let moved = Sphere::radius(cm(R_P))
        .centred_at(cm(0.1), cm(0.0), cm(0.0))
        .filled_with(m.uo2.clone());
    refuse(
        DhRegion::new(Sphere::radius(cm(R_ZONE)), m.matrix.clone())
            .with_particles(
                moved,
                Packing::Random {
                    fraction: Ratio::new::<ratio>(PF),
                    seed: 1,
                },
            )
            .delta_tracking()
            .into(),
        "centred at its own origin",
    );
    refuse(
        DhRegion::new(Sphere::radius(cm(R_ZONE)), m.matrix.clone())
            .with_particles(
                particle(&m),
                Packing::Lattice {
                    pitch: cm(0.05),
                    offset: OFFSET,
                },
            )
            .delta_tracking()
            .into(),
        "smaller than the particle's diameter",
    );
    refuse(
        DhRegion::new(Sphere::radius(cm(R_ZONE)), m.matrix.clone())
            .with_particles(
                particle(&m),
                Packing::LatticeOfCount {
                    particles: 0,
                    offset: OFFSET,
                },
            )
            .delta_tracking()
            .into(),
        "zero particles",
    );
    // Symmetric offsets move the count in shells: 8335 is unreachable at
    // [0.5; 3] in HTR-10's zone (tests/cubic_triso_array.rs), and the
    // message names the nearest count.
    let e =
        Cube::for_whole_particles_in_sphere(&particle(&m), Sphere::radius(cm(2.5)), 8335, [0.5; 3])
            .unwrap_err()
            .to_string();
    assert!(e.contains("nearest"), "{e}");
    assert!(Cube::for_whole_particles_in_sphere(
        &particle(&m),
        Sphere::radius(cm(0.04)),
        1,
        [0.0; 3]
    )
    .is_err());

    // Lattices in spheres: tiles of another material, a grid that does not
    // cover, a covering lattice in a box.
    let tile = |mat: &Material| {
        Cube::side(cm(PITCH))
            .filled_with(mat.clone())
            .containing(particle(&m))
    };
    refuse(
        Sphere::radius(cm(R_ZONE))
            .filled_with(m.matrix.clone())
            .containing_lattice(Lattice::covering(tile(&m.shell), OFFSET)),
        "sphere's own material",
    );
    refuse(
        Sphere::radius(cm(R_ZONE))
            .filled_with(m.matrix.clone())
            .containing_lattice(Lattice::copies_of(tile(&m.matrix), [3, 3, 3])),
        "does not cover",
    );
    let e = model_with(
        Cube::side(cm(1.0))
            .filled_with(m.matrix.clone())
            .containing_lattice(Lattice::covering(tile(&m.matrix), OFFSET)),
    )
    .assemble()
    .unwrap_err()
    .to_string();
    assert!(e.contains("fills a sphere"), "{e}");
    let bad = Lattice::covering(tile(&m.matrix), [f64::NAN, 0.0, 0.0]);
    refuse(
        Sphere::radius(cm(R_ZONE))
            .filled_with(m.matrix.clone())
            .containing_lattice(bad),
        "must be finite",
    );
}

/// **Delta tracking is on whenever a `DhRegion` asks for it** (the
/// correct-physics-is-default pattern: construct through the ordinary path,
/// assert what is there), and diagnostics, which are scored under surface
/// tracking only, are refused on such a model rather than silently missing
/// the region.
#[test]
fn a_dh_region_is_delta_tracked_and_refuses_diagnostics() {
    let m = mats();
    let mut model = Model::new(header_only_library());
    let zone_c = model.component("Fuel zone");
    let zone = random_zone(&m).in_component(zone_c);
    model.add(pebble(&m, zone));
    model.set_temperature(Temperature::Uniform(
        ThermodynamicTemperature::new::<kelvin>(T),
    ));
    let a = model.assemble().unwrap();
    let delta: Vec<_> = a
        .geometry
        .cells
        .iter()
        .filter(|c| matches!(c.tracking, Some(TrackingMethod::Delta { .. })))
        .collect();
    assert_eq!(delta.len(), 1, "the zone's fill cell");
    assert!(matches!(delta[0].fill, CellFill::Lattice(_)));
    let census = a.census().unwrap();
    assert_eq!(census.packings[0].component, Some(zone_c));
    assert!(
        census.copies(zone_c) > 0,
        "matrix cells carry the component"
    );
    let nuclides = low_tier(&a);
    assert_eq!(a.majorants(&nuclides).len(), 1);
    let e = a
        .keff_with(
            &nuclides,
            &KeffSettings::new(MCSettings::Smoke, Some(Diagnostics::default())),
        )
        .err()
        .expect("refused")
        .to_string();
    assert!(e.contains("surface tracking only"), "{e}");
}
