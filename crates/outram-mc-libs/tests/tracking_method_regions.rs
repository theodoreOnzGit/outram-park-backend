//! **Per-region tracking method and region-local majorants** — `bn:op-867c.1`
//! and `.2`, the foundation of hybrid delta/surface tracking (gh #214).
//!
//! NEW WORK, not a port: OpenMC is pure surface tracking and has no equivalent.
//!
//! # What is gated here
//!
//! 1. A region can declare delta tracking, and `locate` reports it.
//! 2. **Inheritance** — nested universes inside a delta region are delta too,
//!    without restating it.
//! 3. **Override** — a deeper region can declare surface tracking, carving a
//!    surface-tracked island out of a delta-tracked one. This is how an HTR-10
//!    control-rod channel sits inside a delta-tracked pebble bed, and it is the
//!    reason `Cell::tracking` is `Option<TrackingMethod>` rather than a bare
//!    enum: `None` (inherit) and `Some(Surface)` (override) must be different,
//!    or every nested universe silently resets its parent's choice.
//! 4. A region-local majorant bounds **only its own materials**, so an absorber
//!    elsewhere in the model cannot raise the cost inside it.
//!
//! Point 3 is the one that would fail silently. A model whose bed quietly
//! reverted to surface tracking would still produce the right `k` — delta
//! tracking is unbiased either way — and only be slower, which is exactly the
//! kind of defect that survives a passing test suite.
//!
//! # Results (2026-09-17)
//!
//! All five assertions pass. The majorant test measures a **100x** saving from
//! excluding one strong absorber on its synthetic three-material table — the
//! physically-sized figure for a real B4C rod against a real HTR-10 pebble is
//! **26.3x at the thermal peak**, measured separately in
//! `examples/majorant_absorber_price.rs`. The two are different problems and
//! are not expected to agree; this one exists to prove the SUBSETTING takes
//! effect, that one to price it.

use outram_mc_libs::geometry::cell::{
    Cell, CellFill, HalfSpaceSense, RegionToken, SurfaceToken, TrackingMethod,
};
use outram_mc_libs::geometry::geometry::Geometry;
use outram_mc_libs::geometry::position::{Direction, Position};
use outram_mc_libs::geometry::surface::{BoundaryType, Sphere, SurfaceKind, XPlane, YPlane, ZPlane};
use outram_mc_libs::geometry::universe::Universe;
use outram_mc_libs::material::material::{Material, NuclideComponent};
use outram_mc_libs::material::nuclide::Nuclide;
use outram_mc_libs::pebble_beds::delta_tracking::Majorant;

const BOX_HALF: f64 = 3.0;
const BED_R: f64 = 2.0;
const ROD_R: f64 = 0.5;

/// Root box; inside it a "bed" sphere declared DELTA-tracked; inside *that* a
/// "rod" sphere declared explicitly SURFACE-tracked. Mirrors the HTR-10 shape:
/// a delta-tracked bed with a surface-tracked control-rod channel through it.
fn geometry_with_rod_channel() -> Geometry {
    let surfaces = vec![
        SurfaceKind::Sphere(Sphere { x0: 0.0, y0: 0.0, z0: 0.0, r: BED_R, bc: BoundaryType::Transmissive }),
        SurfaceKind::Sphere(Sphere { x0: 0.0, y0: 0.0, z0: 0.0, r: ROD_R, bc: BoundaryType::Transmissive }),
        SurfaceKind::XPlane(XPlane { x0: -BOX_HALF, bc: BoundaryType::Reflective }),
        SurfaceKind::XPlane(XPlane { x0: BOX_HALF, bc: BoundaryType::Reflective }),
        SurfaceKind::YPlane(YPlane { y0: -BOX_HALF, bc: BoundaryType::Reflective }),
        SurfaceKind::YPlane(YPlane { y0: BOX_HALF, bc: BoundaryType::Reflective }),
        SurfaceKind::ZPlane(ZPlane { z0: -BOX_HALF, bc: BoundaryType::Reflective }),
        SurfaceKind::ZPlane(ZPlane { z0: BOX_HALF, bc: BoundaryType::Reflective }),
    ];
    let inside = |i: usize| RegionToken::HalfSpace { surface_idx: i, sense: HalfSpaceSense::Inside };
    let outside = |i: usize| RegionToken::HalfSpace { surface_idx: i, sense: HalfSpaceSense::Outside };

    let box_region = vec![
        outside(2), inside(3), RegionToken::Intersection,
        outside(4), RegionToken::Intersection,
        inside(5), RegionToken::Intersection,
        outside(6), RegionToken::Intersection,
        inside(7), RegionToken::Intersection,
    ];

    // Root: reflector, surface-tracked by default (declares nothing).
    let root_reflector = Cell::material(1, box_region.clone(), 2, 293.6);
    // The bed: a nested universe, DELTA-tracked.
    let bed = Cell::fill(2, vec![inside(0)], CellFill::Universe(1), Position::ZERO).delta_tracked(0);

    // Inside the bed universe: fuel everywhere (declares NOTHING -> inherits
    // delta), and a rod channel that declares surface tracking explicitly.
    let bed_fuel = Cell::material(3, vec![outside(1)], 0, 293.6);
    let rod = Cell::material(4, vec![inside(1)], 1, 293.6).surface_tracked();

    Geometry {
        surfaces,
        cells: vec![root_reflector, bed, bed_fuel, rod],
        universes: vec![
            Universe { id: 0, cell_indices: vec![1, 0] }, // bed tried before reflector
            Universe { id: 1, cell_indices: vec![3, 2] }, // rod tried before fuel
        ],
        lattices: vec![],
        root_universe: 0,
    }
}

/// A region that declares nothing is surface-tracked — the default, and what
/// every existing model in this crate silently relies on.
#[test]
fn an_undeclared_region_is_surface_tracked() {
    let geom = geometry_with_rod_channel();
    let u = Direction::new(1.0, 0.0, 0.0);
    let at = geom
        .locate(Position::new(2.5, 0.0, 0.0), u, SurfaceToken::NONE)
        .expect("outside the bed, inside the box");
    assert_eq!(at.material, Some(2), "should be the reflector");
    assert_eq!(
        at.tracking,
        TrackingMethod::Surface,
        "a region declaring nothing must be surface-tracked"
    );
}

/// A declared delta region reports delta, with its own majorant index.
#[test]
fn a_declared_delta_region_reports_delta() {
    let geom = geometry_with_rod_channel();
    let u = Direction::new(1.0, 0.0, 0.0);
    let at = geom
        .locate(Position::new(1.5, 0.0, 0.0), u, SurfaceToken::NONE)
        .expect("inside the bed, outside the rod");
    assert_eq!(at.material, Some(0), "should be bed fuel");
    assert_eq!(
        at.tracking,
        TrackingMethod::Delta { majorant: 0 },
        "inside a delta-tracked bed"
    );
}

/// **Inheritance.** `bed_fuel` declares nothing, sits inside a delta region,
/// and must be delta. Without this, every nested universe of a delta-tracked
/// bed would silently revert to surface tracking — a defect that changes no
/// answer and only costs speed, so nothing else would catch it.
#[test]
fn nested_regions_inherit_delta_without_restating_it() {
    let geom = geometry_with_rod_channel();
    let u = Direction::new(1.0, 0.0, 0.0);
    // Several points, all inside the bed universe, none of them declaring.
    for p in [
        Position::new(1.5, 0.0, 0.0),
        Position::new(0.0, 1.9, 0.0),
        Position::new(0.6, 0.6, 0.6),
    ] {
        let at = geom.locate(p, u, SurfaceToken::NONE).expect("locates");
        assert_eq!(
            at.tracking,
            TrackingMethod::Delta { majorant: 0 },
            "{p:?} is inside the delta-tracked bed and declares nothing, so it \
             must INHERIT delta. Surface here means inheritance is broken."
        );
    }
}

/// **Override.** The rod channel declares surface tracking and must win over
/// the delta-tracked bed enclosing it. This is the HTR-10 case the whole design
/// exists for: a B4C rod inside the bed must not be delta-tracked, because it
/// is what poisons the majorant in the first place.
#[test]
fn a_deeper_region_overrides_an_inherited_delta_region() {
    let geom = geometry_with_rod_channel();
    let u = Direction::new(1.0, 0.0, 0.0);
    let at = geom
        .locate(Position::new(0.2, 0.0, 0.0), u, SurfaceToken::NONE)
        .expect("inside the rod");
    assert_eq!(at.material, Some(1), "should be the rod");
    assert_eq!(
        at.tracking,
        TrackingMethod::Surface,
        "an explicit Some(Surface) inside a delta region must override it. If \
         this reports Delta, `Cell::tracking` has collapsed back to a bare enum \
         and `None` is no longer distinguishable from an explicit override."
    );
}

// --------------------------------------------------------------------------
// Region-local majorant (op-867c.2)
// --------------------------------------------------------------------------

/// Load B-10 from the committed ENDF tape — fast (~30 ms) and a strong
/// absorber, so the subset difference below is unmissable.
fn boron() -> Option<Vec<Nuclide>> {
    let p = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../reference-data/endf/n-005_B_010-ENDF8.0.endf");
    p.exists().then(|| ())?;
    Nuclide::from_endf_file(&p, "B10", 293.6, 1.0e-3).ok().map(|n| vec![n])
}

/// **A region-local majorant must bound only the region's own materials.**
///
/// Three materials of increasing boron density. A majorant over the two weak
/// ones must be strictly below one that also sees the strong one — which is
/// the whole point: an absorber outside a region cannot raise the cost inside
/// it.
///
/// Tape-gated rather than synthetic. A version of this test using materials
/// with no cross sections would compare 0 against 0 and pass while proving
/// nothing.
#[test]
fn a_region_local_majorant_ignores_materials_outside_its_index_list() {
    let Some(nucs) = boron() else {
        eprintln!("SKIP: reference-data/endf/n-005_B_010 not in this checkout");
        return;
    };
    let mat = |id: i32, dens: f64| Material {
        id,
        name: format!("m{id}"),
        components: vec![NuclideComponent { nuclide_idx: 0, atom_density: dens }],
        temperature: 293.6,
    };
    // m3 is 100x m1 — the "control rod" of this miniature.
    let all = vec![mat(1, 1.0e-5), mat(2, 1.0e-4), mat(3, 1.0e-2)];
    let grid: Vec<f64> = (0..24).map(|i| 1.0e-3 * 10.0_f64.powf(i as f64 * 0.5)).collect();

    let weak_only = Majorant::over_indices(&all, &[0, 1], &nucs, &grid, 0.3);
    let by_hand = Majorant::from_materials(&all[..2], &nucs, &grid, 0.3);
    let with_absorber = Majorant::from_materials(&all, &nucs, &grid, 0.3);

    let mut worst_ratio = 0.0_f64;
    for &e in &grid {
        assert_eq!(
            weak_only.at(e),
            by_hand.at(e),
            "over_indices must equal from_materials on the same subset, at {e:e} eV"
        );
        assert!(
            weak_only.at(e) <= with_absorber.at(e) + 1.0e-30,
            "a subset majorant can never exceed the full one, at {e:e} eV"
        );
        if weak_only.at(e) > 0.0 {
            worst_ratio = worst_ratio.max(with_absorber.at(e) / weak_only.at(e));
        }
    }
    println!("excluding the strong absorber saves up to {worst_ratio:.1}x in tracking steps");
    assert!(
        worst_ratio > 10.0,
        "the strong absorber should dominate the global majorant by >10x, got \
         {worst_ratio:.2}x — if this is ~1 the subsetting is not taking effect"
    );

    // A stale index is IGNORED, not a panic: a majorant that is too SMALL is the
    // dangerous direction, and a stale index must not produce one by aborting.
    let with_stale = Majorant::over_indices(&all, &[0, 1, 99], &nucs, &grid, 0.3);
    for &e in &grid {
        assert_eq!(with_stale.at(e), by_hand.at(e), "stale index ignored");
    }
}
