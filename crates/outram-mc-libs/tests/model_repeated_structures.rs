//! **Repeated structures in the beginner API lower to the existing CSG, bit
//! for bit** (gh #926, epic #925).
//!
//! # Methodology
//!
//! Each case builds a geometry twice: once through `model::Model` (shapes
//! filled with materials, `.containing`, `Cube`, `Lattice`,
//! `.boundary(Boundary::Reflective)`), and once by hand from the crate's
//! existing builders: `build_triso_particle` (the five-shell TRISO universe,
//! `outram_blender::csg::triso_particle`, with `TrisoRadii::HTR10`), the
//! six reflective planes of `pebble_beds::fhr_pebble::homogeneous_cube`, and
//! a CSG `RectLattice` as `tests/stats_move_fingerprints.rs` builds its
//! TRISO lattice. Then:
//!
//! 1. **Structure**: every surface (by its `Debug` form, which prints each
//!    `f64` exactly), every cell's region tokens and fill, every universe's
//!    cell list, the lattice (grid, lower-left, pitch, tile universes,
//!    outer) and the root are compared for equality. Only the integer cell
//!    and universe ids (labels, read by no transport code) are not compared.
//! 2. **Transport**: the CSG power iteration on the hand-built geometry
//!    (`run_keff_csg`) and the beginner run on the assembled model
//!    (`keff_with`, `MCSettings::Custom`) at the same seed must give the same
//!    `k` in every generation, to the last bit, on both backends (**since
//!    2026-10-18, gh #955**: the default multi-thread run against
//!    `ComputeType::CpuMultiThread`, `.single_thread()` against
//!    `ComputeType::CpuSingleThread`; ~~single-thread only~~ before). Data:
//!    the embedded LOW tier (`Nuclide::from_core`) on both sides, so this
//!    runs anywhere; it tests the geometry, not the data.
//! 3. **Counting from the geometry** (gh:#316: a lattice held 8240 TRISO
//!    while the model reported 8340): `AssembledModel::census` counts copies
//!    by walking the assembled universe/lattice graph, and its kernel volume
//!    is checked against an independent stochastic volume estimate
//!    (`geometry::volume_calc`, OpenMC's `VolumeCalculation`).
//!
//! # Results (2026-10-18)
//!
//! All pass: the unit cell and the 3 x 3 x 3 lattice are structurally equal
//! to the hand-built geometries and give identical `k` in every generation;
//! the census counts 1 and 27 particles and the packing fractions equal the
//! analytic `V_particle / a^3` to within 1e-15 relative; the stochastic
//! kernel volume agrees within its 4-sigma band. The k values are not
//! physics results (LOW tier, tiny runs).

use outram_mc_libs::geometry::cell::{Cell, CellFill, HalfSpaceSense, RegionToken};
use outram_mc_libs::geometry::geometry::Geometry;
use outram_mc_libs::geometry::lattice::{Lattice as CsgLattice, RectLattice};
use outram_mc_libs::geometry::position::Position;
use outram_mc_libs::geometry::surface::{BoundaryType, SurfaceKind};
use outram_mc_libs::geometry::triso_particle::{build_triso_particle, TrisoMaterials, TrisoRadii};
use outram_mc_libs::geometry::universe::Universe;
use outram_mc_libs::geometry::volume_calc::{BoundingBox, VolumeCalculation, VolumeDomain};
use outram_mc_libs::material::nuclide::Nuclide;
use outram_mc_libs::model::prelude::*;
use outram_mc_libs::pebble_beds::fhr_pebble::homogeneous_cube;
use outram_mc_libs::physics::keff::{ComputeType, KeffSettings as LowKeffSettings, ThreadCount};
use outram_mc_libs::physics::transport_csg::run_keff_csg;
use uom::si::f64::{Length, Ratio, ThermodynamicTemperature};
use uom::si::length::centimeter;
use uom::si::ratio::ratio;
use uom::si::thermodynamic_temperature::kelvin;
use uom::si::volume::cubic_centimeter;

const T: f64 = 293.6;

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
            ("si28".to_string(), tape(14028.0, 27.737450)),
        ],
    )
    .expect("five incident-neutron headers")
}

fn cm(x: f64) -> Length {
    Length::new::<centimeter>(x)
}

/// Materials in first-use order kernel, buffer, PyC, SiC, (PyC), matrix:
/// low-level indices 0, 1, 2, 3, 2, 4. Densities are illustrative (this
/// test is about geometry), in atoms per barn-cm.
struct Mats {
    uo2: Material,
    buffer: Material,
    pyc: Material,
    sic: Material,
    matrix: Material,
}

fn mats() -> Mats {
    let d = atoms_per_barn_cm;
    Mats {
        uo2: Material::from_atom_densities([(U235, d(0.0047)), (U238, d(0.0188)), (O16, d(0.047))])
            .named("UO2"),
        buffer: Material::from_atom_densities([(C12, d(0.0551))]).named("buffer"),
        pyc: Material::from_atom_densities([(C12, d(0.0953))]).named("PyC"),
        sic: Material::from_atom_densities([(Si28, d(0.0478)), (C12, d(0.0478))]).named("SiC"),
        matrix: Material::from_atom_densities([(C12, d(0.0867))]).named("matrix"),
    }
}

/// The TRISO particle, kernel outwards, one `.containing` per statement.
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

const HAND_MATS: TrisoMaterials = TrisoMaterials {
    kernel: 0,
    buffer: 1,
    ipyc: 2,
    sic: 3,
    opyc: 2,
    matrix: 4,
};

/// `homogeneous_cube`'s six reflective planes and region, renumbered to
/// start at surface `base`.
fn reflective_cube(half_width: f64, base: usize) -> (Vec<SurfaceKind>, Vec<RegionToken>) {
    let g = homogeneous_cube(half_width, 0, T);
    let region = g.cells[0]
        .region
        .iter()
        .map(|t| match *t {
            RegionToken::HalfSpace { surface_idx, sense } => RegionToken::HalfSpace {
                surface_idx: surface_idx + base,
                sense,
            },
            other => other,
        })
        .collect();
    (g.surfaces, region)
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
        assert_eq!(
            a.temperature.to_bits(),
            b.temperature.to_bits(),
            "cell {i} T"
        );
        assert_eq!(
            format!("{:?}", a.translation),
            format!("{:?}", b.translation)
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
                assert_eq!(a.universes, b.universes);
                assert_eq!(a.outer, b.outer);
            }
            _ => panic!("rect lattices expected"),
        }
    }
}

/// LOW-tier nuclides in the assembled model's own nuclide order (C-12 is
/// served by the embedded natural carbon, which is all this needs: the same
/// data on both sides).
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

fn assert_same_k(a: &AssembledModel, hand: &Geometry) -> f64 {
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
    let reference_seq = run_keff_csg(hand, &a.materials, &nuclides, a.source_box, &low, None);
    let low_par = LowKeffSettings {
        compute: ComputeType::CpuMultiThread(ThreadCount::Fixed(3)),
        ..low.clone()
    };
    let reference = run_keff_csg(hand, &a.materials, &nuclides, a.source_box, &low_par, None);
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
    assert_eq!(run.keff().mean.to_bits(), reference.k_mean.to_bits());
    let single = a
        .keff_with(&nuclides, &custom.clone().single_thread())
        .expect("runs on one thread");
    assert_eq!(
        single.k_by_cycle, reference_seq.k_by_generation,
        "single thread: per-generation k must be identical bit for bit"
    );
    println!(
        "bit for bit: model {} vs hand-built {:.5} ± {:.5}",
        run.keff(),
        reference.k_mean,
        reference.k_std
    );
    reference.k_mean
}

fn model_with(shape: Shape) -> Model {
    let mut m = Model::new(header_only_library());
    m.add(shape);
    m.set_temperature(Temperature::Uniform(
        ThermodynamicTemperature::new::<kelvin>(T),
    ));
    m
}

/// HTR-10's packing fraction: 8335 particles of r = 0.0455 cm in a 2.5 cm
/// fuel zone (`tests/cubic_triso_array.rs`, `TrisoSpec::HTR10_LI2014`).
fn htr10_pf() -> f64 {
    8335.0 * (0.0455_f64 / 2.5).powi(3)
}

/// **Tutorial 4's geometry**: one TRISO particle in a reflective cube of
/// matrix, against `build_triso_particle` plus `homogeneous_cube`'s planes.
#[test]
fn unit_cell_lowers_to_build_triso_particle_in_a_reflective_cube() {
    let m = mats();
    let p = particle(&m);
    let side = Cube::with_packing_fraction(&p, Ratio::new::<ratio>(htr10_pf()));
    let a_cm = side.sides_cm()[0];
    let cell = side
        .filled_with(m.matrix.clone())
        .containing(p)
        .boundary(Boundary::Reflective);
    let assembled = model_with(cell).assemble().expect("assembles");

    // By hand: the TRISO universe's five shells, then the cube, then the
    // matrix cell = outside(OPyC) ∩ inside(cube).
    let triso = build_triso_particle(Position::ZERO, TrisoRadii::HTR10, HAND_MATS, T, 1, 0, 0);
    let (planes, cube_region) = reflective_cube(0.5 * a_cm, 5);
    let mut surfaces = triso.surfaces.clone();
    surfaces.extend(planes);
    let mut cells: Vec<Cell> = triso.cells[..5].to_vec();
    let mut matrix_region = vec![RegionToken::HalfSpace {
        surface_idx: 4,
        sense: HalfSpaceSense::Outside,
    }];
    matrix_region.extend(cube_region);
    matrix_region.push(RegionToken::Intersection);
    cells.push(Cell::material(6, matrix_region, HAND_MATS.matrix, T));
    let hand = Geometry {
        surfaces,
        cells,
        universes: vec![Universe {
            id: 0,
            cell_indices: (0..6).collect(),
        }],
        lattices: vec![],
        root_universe: 0,
    };
    assert_same_geometry(&assembled.geometry, &hand);
    assert!(assembled.geometry.surfaces[5..].iter().all(|s| matches!(
        s,
        SurfaceKind::XPlane(p) if p.bc == BoundaryType::Reflective
    ) || matches!(s, SurfaceKind::YPlane(p) if p.bc == BoundaryType::Reflective)
        || matches!(s, SurfaceKind::ZPlane(p) if p.bc == BoundaryType::Reflective)));
    assert_eq!(
        assembled.materials.len(),
        5,
        "IPyC and OPyC share one material"
    );
    assert_same_k(&assembled, &hand);

    // Counted from the geometry: one particle, at exactly the packing
    // fraction asked for.
    let census = assembled.census().expect("census");
    println!("{census}");
    let kernel_cell = &census.rows[0];
    assert_eq!((kernel_cell.cell, kernel_cell.copies), (0, 1));
    let particle_v: f64 = census.rows[..5]
        .iter()
        .map(|r| r.volume().get::<cubic_centimeter>())
        .sum();
    let pf = particle_v / census.total_volume().get::<cubic_centimeter>();
    assert!((pf / htr10_pf() - 1.0).abs() < 1e-13, "pf {pf}");
}

/// **A 3 x 3 x 3 lattice of that unit cell** in a reflective box, against a
/// CSG `RectLattice` of `build_triso_particle` universes.
#[test]
fn triso_lattice_lowers_to_a_hand_built_rect_lattice() {
    let m = mats();
    let p = particle(&m);
    let side = Cube::with_packing_fraction(&p, Ratio::new::<ratio>(htr10_pf()));
    let a = side.sides_cm()[0];
    let tile = side.filled_with(m.matrix.clone()).containing(p);
    let lattice = Lattice::copies_of(tile, [3, 3, 3]);
    let block = Cube::side(cm(3.0 * a))
        .filled_with(m.matrix.clone())
        .containing_lattice(lattice)
        .boundary(Boundary::Reflective);
    let assembled = model_with(block).assemble().expect("assembles");

    // By hand. Universe 0: the TRISO (its matrix cell is outside(OPyC),
    // bounded by the lattice). Surfaces 5..10: the container. Universe 1:
    // the lattice's outer, matrix everywhere (-5 ∪ +5). Universe 2: root,
    // the container filled with the lattice.
    let triso = build_triso_particle(Position::ZERO, TrisoRadii::HTR10, HAND_MATS, T, 0, 0, 0);
    let (planes, cube_region) = reflective_cube(1.5 * a, 5);
    let mut surfaces = triso.surfaces.clone();
    surfaces.extend(planes);
    let mut cells = triso.cells.clone();
    cells.push(Cell::material(
        7,
        vec![
            RegionToken::HalfSpace {
                surface_idx: 5,
                sense: HalfSpaceSense::Inside,
            },
            RegionToken::HalfSpace {
                surface_idx: 5,
                sense: HalfSpaceSense::Outside,
            },
            RegionToken::Union,
        ],
        HAND_MATS.matrix,
        T,
    ));
    cells.push(Cell::fill(
        8,
        cube_region,
        CellFill::Lattice(0),
        Position::ZERO,
    ));
    let hand = Geometry {
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
                cell_indices: vec![7],
            },
        ],
        lattices: vec![CsgLattice::Rect(RectLattice {
            id: 1,
            n: [3, 3, 3],
            lower_left: Position::new(-1.5 * a, -1.5 * a, -1.5 * a),
            pitch: [a; 3],
            universes: vec![0; 27],
            outer: Some(1),
        })],
        root_universe: 2,
    };
    assert_same_geometry(&assembled.geometry, &hand);
    let k_lattice = assert_same_k(&assembled, &hand);

    // Counted from the assembled geometry, not from the 3x3x3 we asked for.
    let census = assembled.census().expect("census");
    println!("{census}");
    assert_eq!(census.rows[0].copies, 27, "27 kernels placed");
    assert_eq!(
        census.rows[6].copies, 1,
        "the outer universe is reached once"
    );
    assert!(census.rows[6].volume_each.get::<cubic_centimeter>().abs() < 1e-15);
    let total = census.total_volume().get::<cubic_centimeter>();
    assert!((total / (27.0 * a.powi(3)) - 1.0).abs() < 1e-13);
    let particle_v: f64 = census.rows[..5]
        .iter()
        .map(|r| r.volume().get::<cubic_centimeter>())
        .sum();
    assert!((particle_v / total / htr10_pf() - 1.0).abs() < 1e-13);

    // An independent estimate of the kernel volume: OpenMC's stochastic
    // volume calculation on the assembled geometry, by material.
    let h = 1.5 * a;
    let calc = VolumeCalculation {
        domain: VolumeDomain::Material,
        ids: vec![0],
        box_: BoundingBox {
            lower: Position::new(-h, -h, -h),
            upper: Position::new(h, h, h),
        },
        n_samples: 2_000_000,
        master_seed: 926,
    };
    let (v, _) = calc.execute(&assembled.geometry);
    let exact = census.rows[0].volume().get::<cubic_centimeter>();
    let z = (v[0].volume - exact) / v[0].std_dev;
    println!(
        "kernel volume: census {exact:.6e} cm3, stochastic {:.6e} ± {:.1e} ({z:+.2} sigma)",
        v[0].volume, v[0].std_dev
    );
    assert!(z.abs() < 4.0, "{z}");

    // The lattice of identical reflective cells is the same infinite array
    // as the single reflective cell: the two k must agree statistically
    // (not bitwise: the flights differ). Tiny runs, so a loose 5 sigma.
    let k_cell = {
        let m = mats();
        let p = particle(&m);
        let cell = Cube::with_packing_fraction(&p, Ratio::new::<ratio>(htr10_pf()))
            .filled_with(m.matrix.clone())
            .containing(p)
            .boundary(Boundary::Reflective);
        let a = model_with(cell).assemble().unwrap();
        a.keff_with(
            &low_tier(&a),
            &KeffSettings::new(
                MCSettings::Custom(CustomRun {
                    particles: 300,
                    inactive: 4,
                    active: 8,
                    seed: 11,
                }),
                None,
            ),
        )
        .unwrap()
        .keff()
    };
    println!("k: unit cell {k_cell}, lattice {k_lattice:.5}");
    assert!(((k_cell.mean - k_lattice) / (2.0_f64.sqrt() * k_cell.std)).abs() < 5.0);
}

/// Rows written top-down lower to the CSG lattice's lower-left order, and
/// an empty tile is matrix everywhere (the far-plane region).
#[test]
fn rows_top_down_place_tiles_where_the_picture_shows_them() {
    let m = mats();
    let fuel_tile = Cube::side(cm(0.2))
        .filled_with(m.matrix.clone())
        .containing(particle(&m));
    let empty_tile = Cube::side(cm(0.2)).filled_with(m.buffer.clone());
    // Fuel top-left only.
    let lattice =
        Lattice::from_rows_top_down(vec![fuel_tile, empty_tile], vec![vec![0, 1], vec![1, 1]]);
    let block = Cuboid::sides(cm(0.4), cm(0.4), cm(0.2))
        .filled_with(m.matrix.clone())
        .containing_lattice(lattice);
    let a = model_with(block).assemble().unwrap();
    let mat = |x: f64, y: f64| {
        a.geometry
            .locate(
                Position::new(x, y, 0.0),
                outram_mc_libs::geometry::position::Direction::new(1.0, 0.0, 0.0),
                outram_mc_libs::geometry::cell::SurfaceToken::NONE,
            )
            .and_then(|p| p.material)
    };
    let names: Vec<&str> = a.resolved.iter().map(|r| r.name.as_str()).collect();
    let at = |x, y| mat(x, y).map(|i| names[i]);
    assert_eq!(
        at(-0.1, 0.1),
        Some("UO2"),
        "top-left tile holds the particle"
    );
    assert_eq!(at(0.1, 0.1), Some("buffer"));
    assert_eq!(at(-0.1, -0.1), Some("buffer"));
    assert_eq!(at(-0.1 + 0.08, 0.1), Some("matrix"), "outside the particle");
    let census = a.census().unwrap();
    let fuel_copies: usize = census
        .rows
        .iter()
        .filter(|r| r.material == "UO2")
        .map(|r| r.copies)
        .sum();
    assert_eq!(fuel_copies, 1);
    let empty = census
        .rows
        .iter()
        .find(|r| r.material == "buffer" && r.copies == 3);
    assert!(empty.is_some(), "three empty tiles: {census}");
}

/// **The default run is multi-threaded, and its answer does not depend on
/// the number of threads** (gh #955). Tutorial 4's unit cell, run through
/// the beginner API with the reactor-physics diagnostics (extra tallies,
/// leakage grids, IFP kinetics, mesh maps) inside rayon pools of 1, 2 and 4
/// threads: the per-generation k and entropy, and every diagnostic except
/// the wall time, are equal bit for bit; the provenance records the pool
/// size. The single-thread run is a different (statistically equivalent)
/// estimate, recorded as such.
#[test]
fn the_run_gives_the_same_k_and_diagnostics_on_any_number_of_threads() {
    let m = mats();
    let p = particle(&m);
    let cell = Cube::with_packing_fraction(&p, Ratio::new::<ratio>(htr10_pf()))
        .filled_with(m.matrix.clone())
        .containing(p)
        .boundary(Boundary::Reflective);
    let a = model_with(cell).assemble().unwrap();
    let nuclides = low_tier(&a);
    let settings = KeffSettings::new(
        MCSettings::Custom(CustomRun {
            particles: 300,
            inactive: 4,
            active: 12,
            seed: 955,
        }),
        Some(Diagnostics::reactor_physics()),
    )
    .silent();
    let on = |threads: usize| {
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(threads)
            .build()
            .unwrap();
        let mut run = pool.install(|| a.keff_with(&nuclides, &settings)).unwrap();
        assert_eq!(run.provenance.threads, threads);
        assert!(run.provenance.backend.contains("multi-thread"));
        for (_, r) in run.diagnostics.iter_mut() {
            if let MCDiagnosticResult::RunHealth(h) = r {
                assert!(h.transport_seconds.is_some());
                h.transport_seconds = None;
            }
        }
        run
    };
    let one = on(1);
    for t in [2, 4] {
        let other = on(t);
        assert_eq!(one.k_by_cycle, other.k_by_cycle, "{t} threads: k");
        assert_eq!(
            one.entropy_by_cycle, other.entropy_by_cycle,
            "{t} threads: H"
        );
        assert_eq!(
            one.diagnostics, other.diagnostics,
            "{t} threads: diagnostics"
        );
    }
    assert!(one.kinetics().is_ok(), "IFP ran in parallel");
    let single = a
        .keff_with(&nuclides, &settings.clone().single_thread())
        .unwrap();
    assert_eq!(single.provenance.threads, 1);
    assert_ne!(
        single.k_by_cycle, one.k_by_cycle,
        "different random streams"
    );
    let z = (single.keff().mean - one.keff().mean)
        / (single.keff().std.powi(2) + one.keff().std.powi(2)).sqrt();
    println!(
        "threads: k {} (any pool) vs {} (single thread), {z:+.2} sigma",
        one.keff(),
        single.keff()
    );
    assert!(z.abs() < 5.0, "{z}");
}
