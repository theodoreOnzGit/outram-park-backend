//! **Does transport descend a lattice inside a lattice?** Depth-3 navigation,
//! which nothing in this crate has ever exercised.
//!
//! # Why this test exists
//!
//! The HTR-10 core model (`bn:op-867c`, gh #214) is a **hex bed lattice whose
//! tiles contain pebble universes whose fuel zones contain a TRISO lattice** —
//! three levels. A survey on 2026-09-17 found that `CellFill::Lattice` appears
//! in exactly four places crate-wide, and in every existing model the lattice
//! fills a **root** cell whose tile universes contain only material cells. The
//! deepest case asserted anywhere is
//! `openmc_notebooks::triso::triso_nested_lattice_geometry_navigation`, which
//! checks `levels.len() == 2`.
//!
//! So depth 3 was *expected* to work — `Geometry::locate` is an uncapped loop
//! over levels (`geometry.rs:186-243`) and `distance_to_boundary` handles a
//! lattice at any level (`:263-301`) — but was **never tested**. Building a
//! 24,000-tile core on an untested descent is how a whole phase of work turns
//! out to rest on nothing.
//!
//! There is specific reason for suspicion: the tie-break at `geometry.rs:280-295`
//! exists because of `op-6tz.34`, a systematic history under-count **in nested
//! lattice geometry**. That bug was at depth 2.
//!
//! # The model
//!
//! Deliberately shaped like HTR-10 rather than like a convenient abstraction:
//!
//! ```text
//! universe 0  root cell, reflective box              fill -> lattice 0
//!   lattice 0  2x2x2, pitch 2.0                      tiles -> universe 1
//!     universe 1  "pebble": sphere r=0.8             fill -> lattice 1
//!                 outside the sphere                 -> material 2 (coolant)
//!       lattice 1  2x2x2, pitch 0.8                  tiles -> universe 2
//!         universe 2  "TRISO": sphere r=0.15         -> material 0 (kernel)
//!                     outside                        -> material 1 (matrix)
//! ```
//!
//! # Results (2026-09-17) — depth 3 WORKS
//!
//! | Check | Result |
//! |---|---|
//! | `locate` level count at a nested kernel centre | **3** (root -> lattice 0 -> lattice 1) |
//! | leaf material | kernel, correct |
//! | level 1 / level 2 lattice ids | `Some(0)` / `Some(1)`, tile `[1,1,1]` both |
//! | coolant outside every pebble | correct, and at **depth 2**, not 3 |
//! | matrix inside the inner lattice | correct, at depth 3 |
//! | streaming from inside a kernel | stops at the kernel surface, 0.190000 cm |
//! | streaming in inner-lattice matrix | stops at the **inner tile edge**, 0.050000 cm |
//!
//! **Interpretation.** `Geometry::locate` descends a lattice inside a lattice
//! correctly, and `distance_to_boundary` sees the inner lattice's tile edge
//! rather than streaming through it to the next surface. The depth-3 navigation
//! the HTR-10 core model needs is therefore *present and now gated*, and
//! `op-6tz.34`'s under-count does not recur one level deeper.
//!
//! This says nothing about SCALE. The largest lattice ever built in this crate
//! is 37 hex tiles; HTR-10 needs ~24,000. Depth is retired as a risk here;
//! size is not.
//!
//! **A trap worth recording, because the first version of this test fell into
//! it.** The inner lattice must be sized to fit *inside* the pebble sphere, not
//! to its radius: a cube spanning `[-R, R]` has corners at `R*sqrt(3)`, well
//! outside the sphere, so probe points near the tile corners land in coolant at
//! depth 2 and the descent looks broken when it is not.

use outram_mc_libs::geometry::cell::{Cell, CellFill, HalfSpaceSense, RegionToken, SurfaceToken};
use outram_mc_libs::geometry::geometry::Geometry;
use outram_mc_libs::geometry::lattice::{Lattice, RectLattice};
use outram_mc_libs::geometry::position::{Direction, Position};
use outram_mc_libs::geometry::surface::{
    BoundaryType, Sphere, SurfaceKind, XPlane, YPlane, ZPlane,
};
use outram_mc_libs::geometry::universe::Universe;

const BOX_HALF: f64 = 2.0; // root box half-width
const OUTER_PITCH: f64 = 2.0; // lattice 0 cell size  (2x2x2 over [-2, 2])
const PEBBLE_R: f64 = 0.8;
// Inner lattice spans [-0.4, 0.4]^3, whose furthest corner is 0.4*sqrt(3) =
// 0.693 cm from the pebble centre — comfortably inside PEBBLE_R = 0.8, so every
// inner tile is wholly within the pebble. Sizing it to the sphere's RADIUS
// instead puts the cube corners OUTSIDE the sphere, which is how the first
// version of this test probed coolant and thought the descent had failed.
const INNER_PITCH: f64 = 0.4; // lattice 1 cell size  (2x2x2 over [-0.4, 0.4])
const INNER_HALF: f64 = 0.4;
const KERNEL_R: f64 = 0.1;

const MAT_KERNEL: usize = 0;
const MAT_MATRIX: usize = 1;
const MAT_COOLANT: usize = 2;

/// Build the three-level geometry described in the module docs.
fn depth3_geometry() -> Geometry {
    let surfaces = vec![
        // 0: TRISO kernel, in the innermost universe's local frame
        SurfaceKind::Sphere(Sphere {
            x0: 0.0,
            y0: 0.0,
            z0: 0.0,
            r: KERNEL_R,
            bc: BoundaryType::Transmissive,
        }),
        // 1: pebble surface, in the mid universe's local frame
        SurfaceKind::Sphere(Sphere {
            x0: 0.0,
            y0: 0.0,
            z0: 0.0,
            r: PEBBLE_R,
            bc: BoundaryType::Transmissive,
        }),
        // 2..7: the reflective root box
        SurfaceKind::XPlane(XPlane { x0: -BOX_HALF, bc: BoundaryType::Reflective }),
        SurfaceKind::XPlane(XPlane { x0: BOX_HALF, bc: BoundaryType::Reflective }),
        SurfaceKind::YPlane(YPlane { y0: -BOX_HALF, bc: BoundaryType::Reflective }),
        SurfaceKind::YPlane(YPlane { y0: BOX_HALF, bc: BoundaryType::Reflective }),
        SurfaceKind::ZPlane(ZPlane { z0: -BOX_HALF, bc: BoundaryType::Reflective }),
        SurfaceKind::ZPlane(ZPlane { z0: BOX_HALF, bc: BoundaryType::Reflective }),
    ];

    let inside = |i: usize| RegionToken::HalfSpace { surface_idx: i, sense: HalfSpaceSense::Inside };
    let outside =
        |i: usize| RegionToken::HalfSpace { surface_idx: i, sense: HalfSpaceSense::Outside };

    // Root box: outside the low planes, inside the high ones.
    let box_region = vec![
        outside(2),
        inside(3),
        RegionToken::Intersection,
        outside(4),
        RegionToken::Intersection,
        inside(5),
        RegionToken::Intersection,
        outside(6),
        RegionToken::Intersection,
        inside(7),
        RegionToken::Intersection,
    ];

    // LEVEL 0 — root cell filled by the outer lattice.
    let root = Cell::fill(1, box_region, CellFill::Lattice(0), Position::ZERO);

    // LEVEL 1 — the "pebble": inside the sphere is ANOTHER LATTICE. This is the
    // cell that makes the test depth 3 rather than depth 2.
    let pebble_interior = Cell::fill(2, vec![inside(1)], CellFill::Lattice(1), Position::ZERO);
    let coolant = Cell::material(3, vec![outside(1)], MAT_COOLANT, 293.6);

    // LEVEL 2 — the "TRISO" leaf.
    let kernel = Cell::material(4, vec![inside(0)], MAT_KERNEL, 293.6);
    let matrix = Cell::material(5, vec![outside(0)], MAT_MATRIX, 293.6);

    let outer_lattice = RectLattice {
        id: 0,
        n: [2, 2, 2],
        lower_left: Position::new(-BOX_HALF, -BOX_HALF, -BOX_HALF),
        pitch: [OUTER_PITCH, OUTER_PITCH, OUTER_PITCH],
        universes: vec![1; 8],
        outer: Some(1),
    };
    let inner_lattice = RectLattice {
        id: 1,
        n: [2, 2, 2],
        lower_left: Position::new(-INNER_HALF, -INNER_HALF, -INNER_HALF),
        pitch: [INNER_PITCH, INNER_PITCH, INNER_PITCH],
        universes: vec![2; 8],
        // Anything inside the pebble but outside the inner lattice bounds is
        // matrix — the same role `outer` plays in the real TRISO model.
        outer: Some(2),
    };

    Geometry {
        surfaces,
        cells: vec![root, pebble_interior, coolant, kernel, matrix],
        universes: vec![
            Universe { id: 0, cell_indices: vec![0] },
            Universe { id: 1, cell_indices: vec![1, 2] },
            Universe { id: 2, cell_indices: vec![3, 4] },
        ],
        lattices: vec![Lattice::Rect(outer_lattice), Lattice::Rect(inner_lattice)],
        root_universe: 0,
    }
}

/// **The headline check: `locate` must descend THREE levels.**
///
/// The centre of the model sits at the corner shared by all eight outer tiles,
/// so probe a point unambiguously inside one tile, inside that tile's pebble,
/// and inside one of that pebble's TRISO kernels.
#[test]
fn locate_descends_three_lattice_levels() {
    let geom = depth3_geometry();
    let u = Direction::new(1.0, 0.0, 0.0);

    // Centre of outer tile [1,1,1] is (+1, +1, +1). In that tile's local frame
    // the pebble is centred at the tile centre, and the inner lattice's tile
    // [1,1,1] is centred at (+0.2, +0.2, +0.2) locally — a TRISO kernel centre.
    let kernel_centre = Position::new(1.0 + 0.2, 1.0 + 0.2, 1.0 + 0.2);
    let at = geom
        .locate(kernel_centre, u, SurfaceToken::NONE)
        .expect("a point inside a nested TRISO kernel must locate");

    println!(
        "levels = {}, material = {:?}",
        at.levels.len(),
        at.material
    );
    for (i, lvl) in at.levels.iter().enumerate() {
        println!(
            "  level {i}: lattice = {:?}, index = {:?}",
            lvl.lattice, lvl.lattice_index
        );
    }

    assert_eq!(
        at.levels.len(),
        3,
        "expected root -> outer-lattice tile -> inner-lattice tile, got {} levels. \
         If this is 2, transport does NOT descend a lattice inside a lattice, and \
         the HTR-10 core model (gh #214) has an unscoped prerequisite.",
        at.levels.len()
    );
    assert_eq!(at.material, Some(MAT_KERNEL), "should be in a TRISO kernel");
    assert_eq!(at.levels[1].lattice, Some(0), "level 1 is the outer lattice");
    assert_eq!(at.levels[2].lattice, Some(1), "level 2 is the INNER lattice");
}

/// Each of the three levels must resolve to the right material, so the descent
/// is checked at every depth rather than only at the leaf.
#[test]
fn every_level_resolves_to_its_own_material() {
    let geom = depth3_geometry();
    let u = Direction::new(1.0, 0.0, 0.0);
    let probe = |p: Position| {
        geom.locate(p, u, SurfaceToken::NONE)
            .unwrap_or_else(|| panic!("{p:?} must locate"))
    };

    // Outside every pebble, still inside the root box -> coolant, depth 2.
    let coolant = probe(Position::new(1.9, 1.9, 1.9));
    assert_eq!(coolant.material, Some(MAT_COOLANT), "tile corner is coolant");
    assert_eq!(coolant.levels.len(), 2, "coolant sits at depth 2, not 3");

    // Inside a pebble, inside the inner lattice, but outside the kernel -> matrix.
    let matrix = probe(Position::new(1.0 + 0.2 + KERNEL_R + 0.05, 1.2, 1.2));
    assert_eq!(matrix.material, Some(MAT_MATRIX), "just outside a kernel is matrix");
    assert_eq!(matrix.levels.len(), 3, "matrix is still inside both lattices");

    // Inside a kernel -> fuel.
    let kernel = probe(Position::new(1.2, 1.2, 1.2));
    assert_eq!(kernel.material, Some(MAT_KERNEL));
}

/// **`distance_to_boundary` must see the INNER lattice edge.**
///
/// This is the half of the descent that `locate` alone does not exercise, and
/// it is the half `op-6tz.34` was a bug in: a particle streaming inside a
/// nested lattice must stop at the inner tile edge, not sail through it to the
/// next surface it happens to find.
#[test]
fn streaming_stops_at_the_inner_lattice_edge() {
    let geom = depth3_geometry();
    let u = Direction::new(1.0, 0.0, 0.0);

    // Start just inside a kernel, heading +x. The first thing in the way is the
    // kernel surface itself.
    let start = Position::new(1.2 - KERNEL_R + 0.01, 1.2, 1.2);
    let at = geom.locate(start, u, SurfaceToken::NONE).expect("locates");
    let d = geom.distance_to_boundary(&at);
    println!("distance from inside kernel to first boundary = {:.6} cm", d.distance);
    assert!(
        d.distance > 0.0 && d.distance < 2.0 * KERNEL_R + 1.0e-9,
        "first crossing should be the kernel surface, got {:.6}",
        d.distance
    );

    // Now start in the matrix of the inner tile, heading +x. Inner tile [1,1,1]
    // spans local x in [0.0, 0.4], i.e. global [1.0, 1.4], so from global 1.35
    // the inner-lattice tile edge is 0.05 cm away.
    let in_matrix = Position::new(1.35, 1.2, 1.2);
    let at2 = geom.locate(in_matrix, u, SurfaceToken::NONE).expect("locates");
    let d2 = geom.distance_to_boundary(&at2);
    println!(
        "from inner-lattice matrix at x=1.35, next boundary in {:.6} cm (levels {})",
        d2.distance,
        at2.levels.len()
    );
    assert!(
        d2.distance.is_finite() && d2.distance > 0.0,
        "a particle in a nested lattice must have a finite next boundary, got {:?}",
        d2.distance
    );
    assert!(
        d2.distance <= 0.05 + 1.0e-9,
        "should stop at the inner lattice tile edge ~0.05 cm away, not stream past \
         it: got {:.6} cm. A too-large distance here is the op-6tz.34 signature.",
        d2.distance
    );
}
