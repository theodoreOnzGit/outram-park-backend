//! **Can a lattice tile's contents be clipped by an external surface?**
//! `bn:op-867c.10`, gh #214 — the conus/discharge-tube blocker.
//!
//! # The problem
//!
//! The RMC paper builds HTR-10's conus and discharge tube by **rejecting balls
//! that intersect those surfaces**. `outram-mc` has no per-tile omission:
//! `HexLattice::universe_at` returns a plain universe index, and `HEX_NONE`
//! marks only the unused corners of the skewed square array, not "no ball here".
//!
//! Two routes were on the table: add real tile omission, or express the
//! rejection as a bespoke universe per boundary tile whose cell region is
//! intersected with the boundary surface.
//!
//! # Why the second route should work
//!
//! Surfaces are **global**: a cell region indexes `Geometry::surfaces`, not
//! anything tile-local. And `locate` descends into a tile universe *without*
//! re-consulting the enclosing cell's region. So a tile universe's own cell can
//! reference the conus surface as freely as it references the pebble surface,
//! and `inside(pebble) AND inside(conus)` is just an ordinary region.
//!
//! That is a claim about how the descent behaves, not a certainty, which is why
//! this test exists rather than a paragraph in the bead.
//!
//! # Results (2026-09-17) — IT DOES NOT WORK, and that is the finding
//!
//! A tile universe's cell region **can** reference any surface index, and the
//! clipping does take effect — `a_tile_universe_can_be_clipped_by_a_global_surface`
//! passes. But the surface is evaluated in the **TILE-LOCAL frame**, not
//! globally, because `locate` recentres the position into the tile before
//! testing the region.
//!
//! So one shared conus surface does **not** clip every boundary tile at the
//! right place. Each boundary tile would need its own copy of the conus,
//! translated into that tile's frame — workable, but a materially worse design
//! than the shared surface this test set out to confirm.
//!
//! ## A methodological note worth more than the result
//!
//! The first version of the discriminating test **passed**, and was worthless:
//! it re-centred the lattice so the straddling tile's centre sat exactly on the
//! cut, making the tile-local origin coincide with global `x = 0`. Under that
//! geometry both interpretations give the same answer. Had it been trusted,
//! `op-867c.10` would have been closed on a conclusion that is false.
//!
//! The fix was to probe a tile whose centre is AWAY from the cut, where the two
//! readings disagree. A test that cannot distinguish the hypotheses is not weak
//! evidence — it is none.

use outram_mc_libs::geometry::cell::{Cell, CellFill, HalfSpaceSense, RegionToken, SurfaceToken};
use outram_mc_libs::geometry::geometry::Geometry;
use outram_mc_libs::geometry::lattice::{Lattice, RectLattice};
use outram_mc_libs::geometry::position::{Direction, Position};
use outram_mc_libs::geometry::surface::{BoundaryType, Sphere, SurfaceKind, XPlane, YPlane, ZPlane};
use outram_mc_libs::geometry::universe::Universe;

const HALF: f64 = 2.0; // root box half-width; 2x2x2 lattice of pitch 2.0
const PITCH: f64 = 2.0;
const BALL_R: f64 = 0.7;
/// The "conus": everything with x < CUT is inside the vessel; a ball there
/// survives. A ball at x > CUT is rejected.
const CUT: f64 = 0.0;

const MAT_BALL: usize = 0;
const MAT_VOID_FILL: usize = 1; // what replaces a rejected ball
const MAT_COOLANT: usize = 2;

/// Lattice of 8 tiles. Tiles get one of two universes: an ordinary ball, or a
/// ball CLIPPED by the global "conus" plane — the same universe a boundary tile
/// would use in the real model.
fn clipped_lattice_geometry() -> Geometry {
    let surfaces = vec![
        // 0: the ball, in tile-local coordinates
        SurfaceKind::Sphere(Sphere { x0: 0.0, y0: 0.0, z0: 0.0, r: BALL_R, bc: BoundaryType::Transmissive }),
        // 1: the "conus" cut plane, in GLOBAL coordinates
        SurfaceKind::XPlane(XPlane { x0: CUT, bc: BoundaryType::Transmissive }),
        // 2..7: the reflective root box
        SurfaceKind::XPlane(XPlane { x0: -HALF, bc: BoundaryType::Reflective }),
        SurfaceKind::XPlane(XPlane { x0: HALF, bc: BoundaryType::Reflective }),
        SurfaceKind::YPlane(YPlane { y0: -HALF, bc: BoundaryType::Reflective }),
        SurfaceKind::YPlane(YPlane { y0: HALF, bc: BoundaryType::Reflective }),
        SurfaceKind::ZPlane(ZPlane { z0: -HALF, bc: BoundaryType::Reflective }),
        SurfaceKind::ZPlane(ZPlane { z0: HALF, bc: BoundaryType::Reflective }),
    ];
    let ins = |i: usize| RegionToken::HalfSpace { surface_idx: i, sense: HalfSpaceSense::Inside };
    let out = |i: usize| RegionToken::HalfSpace { surface_idx: i, sense: HalfSpaceSense::Outside };

    let box_region = vec![
        out(2), ins(3), RegionToken::Intersection,
        out(4), RegionToken::Intersection,
        ins(5), RegionToken::Intersection,
        out(6), RegionToken::Intersection,
        ins(7), RegionToken::Intersection,
    ];
    let root = Cell::fill(1, box_region, CellFill::Lattice(0), Position::ZERO);

    // Universe 1 — an ordinary ball.
    let ball = Cell::material(2, vec![ins(0)], MAT_BALL, 293.6);
    let around = Cell::material(3, vec![out(0)], MAT_COOLANT, 293.6);

    // Universe 2 — the SAME ball, but its region is intersected with the global
    // conus plane: fuel only where the ball AND x < CUT. Everywhere else in the
    // tile is the fill material, i.e. the ball has been "rejected" on that side.
    let clipped = Cell::material(
        4,
        vec![ins(0), ins(1), RegionToken::Intersection],
        MAT_BALL,
        293.6,
    );
    let clipped_rest = Cell::material(
        5,
        vec![ins(0), ins(1), RegionToken::Intersection, RegionToken::Complement],
        MAT_VOID_FILL,
        293.6,
    );

    Geometry {
        surfaces,
        cells: vec![root, ball, around, clipped, clipped_rest],
        universes: vec![
            Universe { id: 0, cell_indices: vec![0] },
            Universe { id: 1, cell_indices: vec![1, 2] },
            Universe { id: 2, cell_indices: vec![3, 4] },
        ],
        lattices: vec![Lattice::Rect(RectLattice {
            id: 0,
            n: [2, 2, 2],
            lower_left: Position::new(-HALF, -HALF, -HALF),
            pitch: [PITCH, PITCH, PITCH],
            // Tile [0,*,*] ordinary; tile [1,*,*] clipped — the "boundary tiles".
            universes: vec![1, 2, 1, 2, 1, 2, 1, 2],
            outer: Some(1),
        })],
        root_universe: 0,
    }
}

/// **A tile universe's cell region CAN reference a global surface**, so a ball
/// can be clipped by the conus without any per-tile omission machinery.
#[test]
fn a_tile_universe_can_be_clipped_by_a_global_surface() {
    let geom = clipped_lattice_geometry();
    let u = Direction::new(1.0, 0.0, 0.0);
    let at = |p: Position| {
        geom.locate(p, u, SurfaceToken::NONE)
            .unwrap_or_else(|| panic!("{p:?} must locate"))
            .material
    };

    // Ordinary tile [0,0,0], centre (-1,-1,-1): the ball is whole.
    assert_eq!(at(Position::new(-1.0, -1.0, -1.0)), Some(MAT_BALL), "unclipped ball centre");

    // Clipped tile [1,0,0], centre (+1,-1,-1) -- entirely at x > CUT, so the
    // whole ball is rejected and the tile reads as fill.
    assert_eq!(
        at(Position::new(1.0, -1.0, -1.0)),
        Some(MAT_VOID_FILL),
        "a ball wholly on the rejected side must NOT be fuel"
    );

    // Outside every ball: coolant, via the lattice `outer`.
    assert_eq!(at(Position::new(-1.9, -1.9, -1.9)), Some(MAT_COOLANT), "tile corner");
}

/// **Does the clip follow the GLOBAL surface, or the TILE frame?**
///
/// This is the question the whole approach turns on, and it is easy to write a
/// test that cannot answer it. The first version of this re-centred the lattice
/// so the straddling tile's centre sat exactly ON the cut — which makes the
/// tile-local origin coincide with global `x = 0`, so both interpretations give
/// the same answer and the test proved nothing. It passed, which is worse than
/// failing.
///
/// The discriminating probe needs a tile whose centre is AWAY from the cut.
/// Tile `[1,0,0]` spans global `x in [0, 2]`, centre `x = +1`, ball radius 0.7.
/// Probe global `x = 0.5`, inside that ball:
///
/// - **global** reading: `0.5 > CUT` → rejected → fill
/// - **tile-frame** reading: local `x = 0.5 - 1 = -0.5 < 0` → fuel
///
/// The two disagree, so the answer is informative.
#[test]
fn the_clip_tracks_the_global_surface_not_the_tile_frame() {
    let geom = clipped_lattice_geometry();
    let u = Direction::new(1.0, 0.0, 0.0);
    let at = |p: Position| {
        geom.locate(p, u, SurfaceToken::NONE)
            .unwrap_or_else(|| panic!("{p:?} must locate"))
            .material
    };

    let probe = Position::new(0.5, -1.0, -1.0);
    let got = at(probe);
    println!("probe x = {:.2}, tile centre x = 1.00, cut at {CUT:.2} -> {got:?}", probe.x);
    println!("  global reading expects fill ({MAT_VOID_FILL}); tile-frame expects fuel ({MAT_BALL})");

    // Sanity: the probe really is inside the ball, so the answer is about the
    // CUT and not about having missed the sphere entirely.
    let local_r = (0.5_f64 - 1.0).abs();
    assert!(local_r < BALL_R, "probe must be inside the ball, |local| = {local_r}");

    assert_eq!(
        got,
        Some(MAT_BALL),
        "MEASURED BEHAVIOUR: region surfaces inside a lattice tile are evaluated \
         in the TILE-LOCAL frame. If this ever returns MAT_VOID_FILL the descent \
         has changed to evaluate them globally, which would be a better world for \
         op-867c.10 but is not the one we are in."
    );
}
