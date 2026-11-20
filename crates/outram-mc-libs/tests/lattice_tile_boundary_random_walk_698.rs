//! **The standalone random-walk probe of gh:#698** (the issue's own geometry-only
//! measurement, previously run only inside the issue).
//!
//! # Methodology
//!
//! A geometry-only random walk drives the transport loop's geometry path:
//! `locate_at_level` (the surface token applied at the crossing's level) ->
//! `distance_to_boundary` -> stream -> `cross_surface_in_frame` (surface) or a
//! 1e-8 cm nudge (lattice edge), with exponential flights of mean 0.7 cm and an
//! isotropic redirection in the xy plane at each collision; at most 200 events
//! per history, 20 000 histories per model. A history is LOST when `locate`
//! finds no cell. Both models are reflective on the outside, so no history can
//! legitimately leave.
//!
//! - Model A: a 2x2 `RectLattice` (pitch 1.26) of a pin universe (fuel
//!   cylinder r = 0.4 cm, moderator bounded by its OWN planes at +-0.63, which
//!   coincide with the tile edges); lattice `lower_left` x = y in {0, 1.26, 4.96,
//!   11.26, 78.96}.
//! - Model B: a reflective box at `C +- 0.63` filled with the same pin
//!   universe translated by `(C, C)`, with local planes at +-0.63; C in
//!   {0, 10, 123.456, 1000}.
//!
//! # Results
//!
//! | model | before the fix (old tie rule restored, 2026-11-20) | after the fix |
//! |---|---|---|
//! | A, `lower_left` 0 | 20 000 / 20 000 lost | **0** / 20 000 |
//! | A, `lower_left` 1.26, 4.96, 11.26, 78.96 | (first row aborted the run) | **0** / 20 000 each |
//! | B, C = 0 | 0 / 20 000 | **0** / 20 000 |
//! | B, C = 10 | 19 999 / 20 000 lost | **0** / 20 000 |
//! | B, C = 123.456, 1000 | (not reached) | **0** / 20 000 each |
//!
//! The "before" column reproduces the issue's own numbers (Model A: every
//! history lost; Model B: lost at C = 10 only) by restoring the old tie rule
//! (FP_REL 1e-14, surface preferred within 1e-9 cm) for the experiment, so the
//! probe can fail; the criterion is **zero lost histories**.

use outram_mc_libs::geometry::cell::{Cell, CellFill, HalfSpaceSense, RegionToken, SurfaceToken};
use outram_mc_libs::geometry::crossing::GeometryExt;
use outram_mc_libs::geometry::geometry::{Crossing, Geometry};
use outram_mc_libs::geometry::lattice::{Lattice, RectLattice};
use outram_mc_libs::geometry::position::{stream, Direction, Position};
use outram_mc_libs::geometry::surface::{BoundaryType, SurfaceKind, XPlane, YPlane, ZCylinder};
use outram_mc_libs::geometry::universe::Universe;

const PITCH: f64 = 1.26;
const H: f64 = 0.63;

struct Lcg(u64);
impl Lcg {
    fn next(&mut self) -> f64 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        ((self.0 >> 11) as f64 + 0.5) / (1u64 << 53) as f64
    }
}

fn hs(i: usize, sense: HalfSpaceSense) -> RegionToken {
    RegionToken::HalfSpace { surface_idx: i, sense }
}

/// Surfaces 0..=4: the pin universe's cylinder and its four local planes;
/// 5..=8: the reflective outer planes at x in [x0, x1], y in [y0, y1].
fn surfaces(x0: f64, x1: f64, y0: f64, y1: f64) -> Vec<SurfaceKind> {
    let t = BoundaryType::Transmissive;
    let r = BoundaryType::Reflective;
    vec![
        SurfaceKind::ZCylinder(ZCylinder { x0: 0.0, y0: 0.0, r: 0.4, bc: t }),
        SurfaceKind::XPlane(XPlane { x0: -H, bc: t }),
        SurfaceKind::XPlane(XPlane { x0: H, bc: t }),
        SurfaceKind::YPlane(YPlane { y0: -H, bc: t }),
        SurfaceKind::YPlane(YPlane { y0: H, bc: t }),
        SurfaceKind::XPlane(XPlane { x0, bc: r }),
        SurfaceKind::XPlane(XPlane { x0: x1, bc: r }),
        SurfaceKind::YPlane(YPlane { y0, bc: r }),
        SurfaceKind::YPlane(YPlane { y0: y1, bc: r }),
    ]
}

fn pin_cells() -> Vec<Cell> {
    use HalfSpaceSense::{Inside as I, Outside as O};
    let a = RegionToken::Intersection;
    vec![
        Cell::material(10, vec![hs(0, I)], 0, 293.6),
        Cell::material(
            11,
            vec![hs(0, O), hs(1, O), a, hs(2, I), a, hs(3, O), a, hs(4, I), a],
            1,
            293.6,
        ),
    ]
}

fn root_region() -> Vec<RegionToken> {
    use HalfSpaceSense::{Inside as I, Outside as O};
    let a = RegionToken::Intersection;
    vec![hs(5, O), hs(6, I), a, hs(7, O), a, hs(8, I), a]
}

fn model_a(lower_left: f64) -> (Geometry, Position) {
    let (x0, x1) = (lower_left, lower_left + 2.0 * PITCH);
    let mut cells = vec![Cell::fill(1, root_region(), CellFill::Lattice(0), Position::ZERO)];
    cells.extend(pin_cells());
    let geom = Geometry {
        surfaces: surfaces(x0, x1, x0, x1),
        cells,
        universes: vec![
            Universe { id: 0, cell_indices: vec![0] },
            Universe { id: 1, cell_indices: vec![1, 2] },
        ],
        lattices: vec![Lattice::Rect(RectLattice {
            id: 1,
            n: [2, 2, 1],
            lower_left: Position::new(lower_left, lower_left, -0.5),
            pitch: [PITCH, PITCH, 1.0],
            universes: vec![1, 1, 1, 1],
            outer: None,
        })],
        root_universe: 0,
    };
    (geom, Position::new(lower_left + PITCH, lower_left + PITCH, 0.0))
}

fn model_b(c: f64) -> (Geometry, Position) {
    let mut cells = vec![Cell::fill(1, root_region(), CellFill::Universe(1), Position::new(c, c, 0.0))];
    cells.extend(pin_cells());
    let geom = Geometry {
        surfaces: surfaces(c - H, c + H, c - H, c + H),
        cells,
        universes: vec![
            Universe { id: 0, cell_indices: vec![0] },
            Universe { id: 1, cell_indices: vec![1, 2] },
        ],
        lattices: vec![],
        root_universe: 0,
    };
    (geom, Position::new(c, c, 0.0))
}

/// Lost histories out of `n`, walking as the transport loop does.
fn lost_histories(geom: &Geometry, centre: Position, half: f64, n: usize, seed: u64) -> usize {
    const NUDGE: f64 = 1.0e-8;
    let mut rng = Lcg(seed);
    let mut dummy_seed = seed;
    let mut lost = 0;
    for _ in 0..n {
        let mut r = Position::new(
            centre.x + half * (2.0 * rng.next() - 1.0),
            centre.y + half * (2.0 * rng.next() - 1.0),
            0.0,
        );
        let phi = std::f64::consts::TAU * rng.next();
        let mut u = Direction::new(phi.cos(), phi.sin(), 0.0);
        let (mut on, mut on_level) = (SurfaceToken::NONE, 0usize);
        for _ in 0..200 {
            let located = if on == SurfaceToken::NONE {
                geom.locate(r, u, on)
            } else {
                geom.locate_at_level(r, u, on, on_level)
            };
            let Some(path) = located else {
                lost += 1;
                break;
            };
            let hit = geom.distance_to_boundary(&path);
            let d_coll = -rng.next().ln() * 0.7;
            if d_coll < hit.distance {
                r = stream(r, u, d_coll);
                let phi = std::f64::consts::TAU * rng.next();
                u = Direction::new(phi.cos(), phi.sin(), 0.0);
                on = SurfaceToken::NONE;
                continue;
            }
            match hit.crossing {
                Crossing::Surface(i) => {
                    let r_hit = stream(r, u, hit.distance);
                    let x = geom.cross_surface_in_frame(i, &path, hit.coord_level, r_hit, u, &mut dummy_seed);
                    if !x.alive {
                        break;
                    }
                    r = x.r;
                    u = x.u;
                    on = x.on_surface;
                    on_level = hit.coord_level;
                }
                Crossing::Lattice => {
                    r = stream(stream(r, u, hit.distance), u, NUDGE);
                    on = SurfaceToken::NONE;
                }
                Crossing::None => break,
            }
        }
    }
    lost
}

#[test]
fn no_history_is_lost_in_a_lattice_of_tile_bounded_pins() {
    for ll in [0.0, 1.26, 4.96, 11.26, 78.96] {
        let (g, c) = model_a(ll);
        let lost = lost_histories(&g, c, PITCH - 1e-6, 20_000, 0x698a);
        println!("model A lower_left = {ll:>6}: lost {lost} / 20000");
        assert_eq!(lost, 0, "model A, lower_left {ll}");
    }
}

#[test]
fn no_history_is_lost_in_a_translated_tile_bounded_universe() {
    for cc in [0.0, 10.0, 123.456, 1000.0] {
        let (g, c) = model_b(cc);
        let lost = lost_histories(&g, c, H - 1e-6, 20_000, 0x698b);
        println!("model B C = {cc:>8}: lost {lost} / 20000");
        assert_eq!(lost, 0, "model B, C {cc}");
    }
}
