//! High-level geometry navigation: particle location and boundary crossing.
//!
//! C++ source: `src/geometry.cpp` (495 LOC), `include/openmc/geometry.h`.
//!
//! These are the two innermost queries of the transport algorithm:
//!   1. [`Geometry::locate`] — descend the universe/lattice hierarchy from the
//!      root universe to find the leaf cell and its material at a point (ported
//!      from `find_cell_inner`, `src/geometry.cpp:102`).
//!   2. [`Geometry::distance_to_boundary`] — over every coordinate level, find
//!      the nearest surface **or** lattice-tile crossing (ported from
//!      `distance_to_boundary`, `src/geometry.cpp:361`).
//!
//! A [`Geometry`] owns the flat arrays every index refers to: `surfaces`,
//! `cells`, `universes`, `lattices`, plus the `root_universe`. It is read-only
//! after construction, so transport threads share it as `Arc<Geometry>`.

use super::cell::{CellFill, HalfSpaceSense, SurfaceToken};
use super::lattice::Lattice;
use super::position::{stream, Direction, Position};
use super::surface::{BoundaryType, SurfaceKind};
use super::universe::Universe;
use crate::geometry::cell::Cell;

/// One coordinate level in a located particle's nesting chain.
///
/// Mirrors an OpenMC `LocalCoord`: the universe searched at this level, the cell
/// found there, and the particle's position/direction expressed in that level's
/// local frame. `lattice` is `Some` when this level's universe was reached by
/// descending into a lattice (so a lattice-tile crossing is possible here).
#[derive(Debug, Clone, Copy)]
pub struct Coord {
    /// Universe index searched at this level.
    pub universe: usize,
    /// Cell index (global) found containing the particle at this level.
    pub cell: usize,
    /// Position \[cm\] in this level's local frame.
    pub r: Position,
    /// Direction (unit) in this level's local frame.
    pub u: Direction,
    /// Lattice index if this level was entered via a lattice, else `None`.
    pub lattice: Option<usize>,
    /// Lattice tile index `[ix, iy, iz]` for this level (only meaningful if
    /// `lattice` is `Some`).
    pub lattice_index: [i32; 3],
}

/// A fully located particle: its coordinate-level chain plus the leaf material.
pub struct GeometryPath {
    /// Coordinate levels from root (index 0) down to the material leaf.
    pub levels: Vec<Coord>,
    /// Leaf material index, or `None` for a void cell.
    pub material: Option<usize>,
    /// The surface the particle currently sits on and which side of it it is on
    /// ([`SurfaceToken::NONE`] if it is on none). Used for coincident-distance
    /// handling and for unambiguous cell membership after a crossing.
    pub on_surface: SurfaceToken,
}

impl GeometryPath {
    /// The leaf (lowest) coordinate level — where the material fill lives.
    #[inline]
    pub fn leaf(&self) -> &Coord {
        self.levels
            .last()
            .expect("a located path has at least one level")
    }
}

/// What the nearest boundary along a flight is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Crossing {
    /// A CSG surface with this global index is crossed.
    Surface(usize),
    /// A lattice-tile boundary is crossed (re-locate into the neighbouring tile).
    Lattice,
    /// No boundary within a finite distance (particle streams to infinity).
    None,
}

/// Result of a [`Geometry::distance_to_boundary`] query.
#[derive(Debug, Clone, Copy)]
pub struct BoundaryHit {
    /// Distance \[cm\] to the nearest boundary (`INFINITY` if none).
    pub distance: f64,
    /// What is crossed at that distance.
    pub crossing: Crossing,
    /// Coordinate level (index into [`GeometryPath::levels`]) of the crossing.
    pub coord_level: usize,
}

/// Post-crossing state returned by [`Geometry::cross_surface`].
///
/// Feed `r`, `u` and `on_surface` straight back into the transport loop's next
/// [`Geometry::locate`] call; `alive` is `false` only for a vacuum (leak)
/// crossing, where the other fields are the escape state.
#[derive(Debug, Clone, Copy)]
pub struct SurfaceCrossing {
    /// Position \[cm\] just across the surface (nudged off it — see
    /// [`Geometry::cross_surface`]).
    pub r: Position,
    /// Outgoing unit direction — reflected for a reflective surface, unchanged
    /// for a transmissive one.
    pub u: Direction,
    /// `false` if the crossing killed the particle (vacuum boundary = leak).
    pub alive: bool,
    /// The surface just crossed and **which side of it the particle is now on**.
    /// [`SurfaceToken::NONE`] for a vacuum crossing (the history is over).
    pub on_surface: SurfaceToken,
}

/// Which side of `surf` a particle leaving along `u_out` from the crossing point
/// `r` ends up on.
///
/// Purely geometric, and independent of any cell's region definition: the
/// outward normal points in the direction `evaluate` increases, so a particle
/// travelling with `u_out · n > 0` is heading into the positive (outside)
/// half-space. This is how OpenMC signs its surface token when the region is not
/// a simple intersection (`Region::distance_complex`, `src/cell.cpp:1013`), and
/// it is used here for every crossing because it needs no assumption about how
/// the region was written.
#[inline]
fn outgoing_side(surf: &SurfaceKind, r: Position, u_out: Direction, i_surf: usize) -> SurfaceToken {
    let n = surf.normal(r);
    let dot = u_out.u * n.u + u_out.v * n.v + u_out.w * n.w;
    let sense = if dot > 0.0 {
        HalfSpaceSense::Outside
    } else {
        HalfSpaceSense::Inside
    };
    SurfaceToken::on(i_surf, sense)
}

/// The whole CSG model — the flat arrays every geometry index refers to.
///
/// Read-only after construction; share across threads as `Arc<Geometry>`.
/// Maps to OpenMC's `model::{surfaces,cells,universes,lattices}` globals plus
/// `model::root_universe`.
pub struct Geometry {
    /// Global surface array; region tokens and `on_surface` index into it.
    pub surfaces: Vec<SurfaceKind>,
    /// Global cell array; universes and paths index into it.
    pub cells: Vec<Cell>,
    /// Global universe array; the root and every fill index into it.
    pub universes: Vec<Universe>,
    /// Global lattice array; lattice-fill cells index into it. Each entry is a
    /// [`Lattice`] enum ([`Lattice::Rect`] or [`Lattice::Hex`]).
    pub lattices: Vec<Lattice>,
    /// Index of the root universe tracking starts in.
    pub root_universe: usize,
}

impl Geometry {
    /// Locate the particle at global position `r` moving along `u`.
    ///
    /// Descends from the root universe: at each level it finds the containing
    /// cell; a `Material`/`Void` fill terminates the descent, a `Universe` fill
    /// recurses into that universe (applying the cell translation), and a
    /// `Lattice` fill resolves the tile index and recurses into the tile's
    /// universe (recentring the position to the tile). Ported from
    /// `find_cell_inner` (`src/geometry.cpp:102`).
    ///
    /// `on_surface` records which surface the particle sits on and on which side
    /// (see [`SurfaceToken`]); pass [`SurfaceToken::NONE`] for a standalone
    /// point query. It is used to resolve cell membership on that surface
    /// exactly — without it a particle sitting on a boundary it has just crossed
    /// can be re-located in the cell it was leaving — and is carried through into
    /// the returned path for coincident-distance handling. Returns `None` if the
    /// particle is in no cell at some level (a "lost" particle — outside the
    /// geometry).
    pub fn locate(
        &self,
        r: Position,
        u: Direction,
        on_surface: SurfaceToken,
    ) -> Option<GeometryPath> {
        let mut levels: Vec<Coord> = Vec::new();
        let mut level = Coord {
            universe: self.root_universe,
            cell: usize::MAX,
            r,
            u,
            lattice: None,
            lattice_index: [0; 3],
        };

        loop {
            let i_cell = self.universes[level.universe].find_cell(
                level.r,
                level.u,
                &self.surfaces,
                &self.cells,
                on_surface,
            )?;
            level.cell = i_cell;
            let cell = &self.cells[i_cell];

            match cell.fill {
                CellFill::Material(m) => {
                    levels.push(level);
                    return Some(GeometryPath {
                        levels,
                        material: Some(m),
                        on_surface,
                    });
                }
                CellFill::Void => {
                    levels.push(level);
                    return Some(GeometryPath {
                        levels,
                        material: None,
                        on_surface,
                    });
                }
                CellFill::Universe(u_idx) => {
                    let child = Coord {
                        universe: u_idx,
                        cell: usize::MAX,
                        r: level.r - cell.translation,
                        u: level.u,
                        lattice: None,
                        lattice_index: [0; 3],
                    };
                    levels.push(level);
                    level = child;
                }
                CellFill::Lattice(l_idx) => {
                    let lat = &self.lattices[l_idx];
                    let coord_r = level.r - cell.translation;
                    let idx = lat.get_indices(coord_r, level.u);
                    let uni = lat.universe_at(idx)?; // out of grid + no outer ⇒ lost
                    let local = lat.get_local_position(coord_r, idx);
                    let child = Coord {
                        universe: uni,
                        cell: usize::MAX,
                        r: local,
                        u: level.u,
                        lattice: Some(l_idx),
                        lattice_index: idx,
                    };
                    levels.push(level);
                    level = child;
                }
            }
        }
    }

    /// Distance to the nearest boundary — surface or lattice tile — over all
    /// coordinate levels of `path`.
    ///
    /// Ported from `distance_to_boundary` (`src/geometry.cpp:361`): each level
    /// contributes its cell's nearest bounding-surface distance and, if the level
    /// is a lattice tile, the distance to the next tile edge; the global minimum
    /// wins. Because nested frames here are pure translations (no rotation), the
    /// global `on_surface` index is valid for coincident checks at every level.
    pub fn distance_to_boundary(&self, path: &GeometryPath) -> BoundaryHit {
        const FP_REL: f64 = 1.0e-14;
        let mut best = BoundaryHit {
            distance: f64::INFINITY,
            crossing: Crossing::None,
            coord_level: 0,
        };

        for (i, coord) in path.levels.iter().enumerate() {
            let cell = &self.cells[coord.cell];
            let (d_surf, i_surf) =
                cell.distance_to_boundary(coord.r, coord.u, &self.surfaces, path.on_surface);
            if d_surf < best.distance * (1.0 - FP_REL) {
                best.distance = d_surf;
                best.crossing = if i_surf == usize::MAX {
                    Crossing::None
                } else {
                    Crossing::Surface(i_surf)
                };
                best.coord_level = i;
            }

            if let Some(l_idx) = coord.lattice {
                let (d_lat, _trans) =
                    self.lattices[l_idx].distance(coord.r, coord.u, coord.lattice_index);
                // Root-cause fix for op-6tz.34 (nested-lattice under-count). When a
                // lattice fills its enclosing cell exactly, the outermost tile edge
                // is coincident with the cell's bounding (reflective) surface. If
                // floating-point rounding lets the lattice edge win this tie, the
                // transport loop nudges the particle a hair PAST the tile edge —
                // which is also past the reflective wall — so it lands outside the
                // model region and `locate` leaks it, systematically under-counting
                // histories. A bounding surface is the harder boundary: require a
                // lattice crossing to beat the current best by a small ABSOLUTE
                // margin when the best is a coincident Surface, so the wall wins the
                // tie and the particle reflects instead of leaking. A genuine
                // interior tile crossing (no coincident surface) is unaffected.
                const COINCIDENT_ABS: f64 = 1.0e-9; // cm; the transport nudge scale
                let surface_tie = matches!(best.crossing, Crossing::Surface(_))
                    && (best.distance - d_lat).abs() < COINCIDENT_ABS;
                if d_lat < best.distance * (1.0 - FP_REL) && !surface_tie {
                    best.distance = d_lat;
                    best.crossing = Crossing::Lattice;
                    best.coord_level = i;
                }
            }
        }
        best
    }

    /// Total macroscopic cross section of the cell a point is in — a convenience
    /// for delta-tracking majorant lookups. `None` if the point is lost (outside
    /// the geometry) or in a void cell.
    ///
    /// `materials`/`nuclides` are the global arrays the leaf material indexes into.
    pub fn sigma_t_at(
        &self,
        r: Position,
        u: Direction,
        e: f64,
        materials: &[crate::material::material::Material],
        nuclides: &[crate::material::nuclide::Nuclide],
    ) -> Option<f64> {
        let path = self.locate(r, u, SurfaceToken::NONE)?;
        let m = path.material?;
        Some(materials[m].macro_xs_total(e, nuclides))
    }

    /// Apply a surface crossing to a global position/direction and return the
    /// post-crossing state.
    ///
    /// The particle is assumed already streamed to the surface at global `r`.
    /// A **reflective** surface reflects `u` about its outward normal; a
    /// **vacuum** surface kills the particle (leak); a transmissive crossing
    /// passes through unchanged. The returned position is nudged a hair
    /// **across the surface, along its normal**, and the returned
    /// [`SurfaceCrossing::on_surface`] records which side the particle ended up
    /// on, so the next [`Geometry::locate`] is unambiguous.
    ///
    /// # Why the outgoing side must be recorded, not re-derived
    ///
    /// After the crossing the particle sits (to within round-off) *on* the
    /// surface, where the sign of `Surface::evaluate` is decided by rounding
    /// rather than by geometry — worst at **grazing incidence on a curved
    /// surface**, where the tangential step dominates. A membership test that
    /// re-evaluates that sign can put the particle back in the cell it was
    /// leaving; the next `distance_to_boundary` then finds no forward surface
    /// (the one it sits on is suppressed as coincident), so the particle streams
    /// to infinity and the history leaks. On a concentric-shell pebble that lost
    /// 85 % of source neutrons (GitHub #168). The outgoing side is known exactly
    /// here — it is the sign of `u_out · n` — so it is recorded and carried,
    /// exactly as OpenMC carries its signed surface token
    /// (`src/particle.cpp:344`, and `surface() = -surface()` on reflection at
    /// `:795`).
    ///
    /// The nudge is a second, independent guard belonging to *this* crate's
    /// tracker (OpenMC does not nudge): it is taken along the surface **normal**
    /// so `evaluate` changes sign no matter how tangent `u_out` is, plus a step
    /// along `u_out` so a grazing particle makes tangential progress and does not
    /// re-hit the same point. See [`nudge_across`].
    ///
    /// Mirrors the boundary-condition dispatch in `Particle::cross_surface`
    /// (`src/particle.cpp:659`), reduced to the vacuum/reflective/transmissive
    /// cases this crate implements.
    pub fn cross_surface(&self, i_surf: usize, r: Position, u: Direction) -> SurfaceCrossing {
        let surf = &self.surfaces[i_surf];
        match surf.bc() {
            BoundaryType::Vacuum => SurfaceCrossing {
                r,
                u,
                alive: false,
                on_surface: SurfaceToken::NONE,
            },
            BoundaryType::Reflective | BoundaryType::White | BoundaryType::Periodic => {
                // White/Periodic are approximated as reflective (documented gap).
                // Compose the reflection off EVERY reflective surface coincident
                // with `r` (corner/edge handling — see
                // [`Geometry::compose_corner_reflection`]); for a lone wall this
                // reduces exactly to `surf.reflect(r, u)`.
                let u_new = self.compose_corner_reflection(i_surf, r, u);
                // The particle bounces back to the side it came from.
                let p = nudge_across(surf, r, u, u_new, false);
                SurfaceCrossing {
                    r: p,
                    u: u_new,
                    alive: true,
                    on_surface: outgoing_side(surf, r, u_new, i_surf),
                }
            }
            BoundaryType::Transmissive => {
                // The particle passes through to the far side.
                let p = nudge_across(surf, r, u, u, true);
                SurfaceCrossing {
                    r: p,
                    u,
                    alive: true,
                    on_surface: outgoing_side(surf, r, u, i_surf),
                }
            }
        }
    }

    /// Compose the specular reflections of every reflective-type surface
    /// coincident with the crossing point `r`, returning the outgoing direction.
    ///
    /// # Physical principle
    ///
    /// Specular reflections **compose at a corner/edge**. When a particle streams
    /// into a point that lies on the primary reflective surface `i_surf` *and*
    /// (within `CORNER_TOL`) on one or more other Reflective / White / Periodic
    /// surfaces, reflecting off only `i_surf` leaves the particle still headed
    /// into the neighbouring wall(s). Reflecting off **all** coincident reflective
    /// surfaces in a single event sends it back out of the corner immediately; at
    /// a right-angle corner this is the familiar retroreflection (every involved
    /// direction component negated at once).
    ///
    /// # Why this crate needs it (robustness fix, not a port)
    ///
    /// This crate's surface tracker advances to a boundary and then nudges a fixed
    /// [`NUDGE`](Self::cross_surface) (`1e-9 cm`) along the outgoing direction.
    /// Near a corner a grazing particle otherwise "ping-pongs": it alternately
    /// re-crosses the perpendicular walls at sub-nudge distances, advancing only
    /// ~`NUDGE` *along* the wall per event, so clearing an O(1 cm) feature can take
    /// ~10^5–10^9 events — past `MAX_EVENTS`, where the history is capped and
    /// leaked (a small negative bias), or previously hung. Composing the corner
    /// reflection makes the particle leave in one event and removes the bias. This
    /// is a robustness fix for *this crate's fixed-nudge tracker*: OpenMC advances
    /// exactly to each surface token and never accumulates a sub-nudge residual, so
    /// it has no directly corresponding routine — only the physics (reflections
    /// compose at a corner) is mirrored.
    ///
    /// # Selection and composition
    ///
    /// A surface `j != i_surf` is included only when the *incoming* direction
    /// actually **crosses** it (`|u · n_j| > 0`, judged on the incoming `u`) — the
    /// same condition that made `i_surf` a crossing in the first place. A wall the
    /// particle grazes exactly parallel to (`u · n_j = 0`) is skipped (reflecting
    /// off it would be a no-op anyway). The sign of `u · n_j` is deliberately *not*
    /// used: an axis-aligned plane's [`SurfaceKind::normal`] always points along the
    /// +axis regardless of which side bounds the cell, so a min-side wall (cell on
    /// the +sense side, exited with `u · n_j < 0`) must be treated identically to a
    /// max-side wall — testing only the sign would leave min-side corners leaking.
    /// A particle exiting a convex corner from inside the cell crosses every
    /// coincident bounding wall, so composing all their reflections is exactly the
    /// retroreflection that sends it back inside. The common lone-wall case has no
    /// other coincident surface, so this reduces to exactly `surf.reflect(r, u)`
    /// (bit-identical prior behaviour).
    ///
    /// Reflections are composed by applying each surface's [`SurfaceKind::reflect`]
    /// in turn. For mutually orthogonal walls (axis-aligned cube / lattice-cell
    /// corners — the realistic case) the order is immaterial and the result is the
    /// exact retroreflection of the crossed components. For a corner of
    /// non-orthogonal reflective surfaces the composition is order-dependent and
    /// only approximate; such geometries do not arise in the current verification
    /// set.
    ///
    /// `r` is the crossing point; coincidence is judged by
    /// `|SurfaceKind::evaluate(r)| <= CORNER_TOL`.
    fn compose_corner_reflection(&self, i_surf: usize, r: Position, u: Direction) -> Direction {
        /// Coincidence tolerance \[cm\] for a shared corner/edge — the transport
        /// nudge scale, so a sub-nudge ping-pong pair is always caught while any
        /// physically distinct wall (>> 1e-9 cm away) is not.
        const CORNER_TOL: f64 = 1.0e-9;

        // Always reflect off the primary (crossed) surface.
        let mut u_new = self.surfaces[i_surf].reflect(r, u);

        for (j, surf) in self.surfaces.iter().enumerate() {
            if j == i_surf {
                continue;
            }
            if !matches!(
                surf.bc(),
                BoundaryType::Reflective | BoundaryType::White | BoundaryType::Periodic
            ) {
                continue;
            }
            // Coincident with the crossing point?
            if surf.evaluate(r).abs() > CORNER_TOL {
                continue;
            }
            // Reflect off any coincident wall the particle actually crosses (has a
            // non-zero velocity component through). Sign-agnostic: min-side and
            // max-side walls share the same +axis geometric normal, so both must be
            // handled. A grazing wall (`u·n = 0`) is a no-op and is skipped.
            let n = surf.normal(r);
            let dot = u.u * n.u + u.v * n.v + u.w * n.w;
            if dot != 0.0 {
                u_new = surf.reflect(r, u_new);
            }
        }
        u_new
    }
}

/// Move a particle sitting on surface `surf` (at `r`) decisively onto the
/// correct side of it, then along its outgoing direction.
///
/// - `u_in` is the **incoming** direction (used only for its sign relative to
///   the surface normal — which way the particle was crossing).
/// - `u_out` is the direction the particle leaves with (`= u_in` for a
///   transmissive crossing, the reflected direction for a reflective one).
/// - `through`: `true` for a transmissive crossing (end up on the *far* side),
///   `false` for a reflective one (bounce back to the *incoming* side).
///
/// The normal-direction step is what makes this robust at grazing incidence on
/// a curved surface — see [`Geometry::cross_surface`]. `Surface::evaluate`
/// increases along `+normal`, so stepping `±normal·NUDGE` flips its sign in the
/// intended direction no matter how tangent `u_out` is; the extra `u_out` step
/// keeps a grazing particle from re-hitting the same point.
fn nudge_across(
    surf: &SurfaceKind,
    r: Position,
    u_in: Direction,
    u_out: Direction,
    through: bool,
) -> Position {
    const NUDGE: f64 = 1.0e-9;
    let n = surf.normal(r);
    let dot = u_in.u * n.u + u_in.v * n.v + u_in.w * n.w;
    // Preserve prior behaviour for the (non-physical) exactly-tangent case.
    if dot == 0.0 {
        return stream(r, u_out, NUDGE);
    }
    // `+normal` raises `evaluate`. Transmissive: end up where evaluate has the
    // sign of the crossing direction (`dot`). Reflective: the opposite sign.
    let side = if through { dot.signum() } else { -dot.signum() };
    let across = Direction::new(n.u * side, n.v * side, n.w * side);
    let p = stream(r, u_out, NUDGE);
    stream(p, across, NUDGE)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::cell::{Cell, HalfSpaceSense, RegionToken};
    use crate::geometry::surface::{Sphere, XPlane, YPlane, ZCylinder};

    /// Two concentric regions in one universe: fuel inside a cylinder, moderator
    /// outside it but inside a reflective square box. Verifies `locate` picks the
    /// right cell/material on both sides of the cylinder, and that
    /// `distance_to_boundary` returns the cylinder wall from inside the fuel.
    #[test]
    fn locate_pincell_two_region() {
        let r_fuel = 0.4;
        let half = 0.63;
        let surfaces = vec![
            SurfaceKind::ZCylinder(ZCylinder {
                x0: 0.0,
                y0: 0.0,
                r: r_fuel,
                bc: BoundaryType::Transmissive,
            }),
            SurfaceKind::XPlane(XPlane {
                x0: -half,
                bc: BoundaryType::Reflective,
            }),
            SurfaceKind::XPlane(XPlane {
                x0: half,
                bc: BoundaryType::Reflective,
            }),
            SurfaceKind::YPlane(YPlane {
                y0: -half,
                bc: BoundaryType::Reflective,
            }),
            SurfaceKind::YPlane(YPlane {
                y0: half,
                bc: BoundaryType::Reflective,
            }),
        ];
        // Fuel cell: inside the cylinder.
        let fuel = Cell::material(
            1,
            vec![RegionToken::HalfSpace {
                surface_idx: 0,
                sense: HalfSpaceSense::Inside,
            }],
            0,
            293.6,
        );
        // Moderator cell: outside cylinder AND inside the four planes.
        let mod_region = vec![
            RegionToken::HalfSpace {
                surface_idx: 0,
                sense: HalfSpaceSense::Outside,
            },
            RegionToken::HalfSpace {
                surface_idx: 1,
                sense: HalfSpaceSense::Outside,
            }, // x > -half
            RegionToken::Intersection,
            RegionToken::HalfSpace {
                surface_idx: 2,
                sense: HalfSpaceSense::Inside,
            }, // x < +half
            RegionToken::Intersection,
            RegionToken::HalfSpace {
                surface_idx: 3,
                sense: HalfSpaceSense::Outside,
            }, // y > -half
            RegionToken::Intersection,
            RegionToken::HalfSpace {
                surface_idx: 4,
                sense: HalfSpaceSense::Inside,
            }, // y < +half
            RegionToken::Intersection,
        ];
        let moder = Cell::material(2, mod_region, 1, 293.6);
        let cells = vec![fuel, moder];
        let universes = vec![Universe {
            id: 0,
            cell_indices: vec![0, 1],
        }];
        let geom = Geometry {
            surfaces,
            cells,
            universes,
            lattices: vec![],
            root_universe: 0,
        };

        // At the origin: inside the fuel.
        let p = geom
            .locate(
                Position::new(0.0, 0.0, 0.0),
                Direction::new(1.0, 0.0, 0.0),
                SurfaceToken::NONE,
            )
            .unwrap();
        assert_eq!(p.material, Some(0), "origin is fuel");
        let hit = geom.distance_to_boundary(&p);
        assert!(
            (hit.distance - r_fuel).abs() < 1e-9,
            "fuel→wall distance {}",
            hit.distance
        );
        assert_eq!(hit.crossing, Crossing::Surface(0));

        // Between cylinder and box: moderator.
        let p2 = geom
            .locate(
                Position::new(0.5, 0.0, 0.0),
                Direction::new(1.0, 0.0, 0.0),
                SurfaceToken::NONE,
            )
            .unwrap();
        assert_eq!(p2.material, Some(1), "0.5 cm out is moderator");

        // Outside the box: lost.
        assert!(geom
            .locate(
                Position::new(1.0, 0.0, 0.0),
                Direction::new(1.0, 0.0, 0.0),
                SurfaceToken::NONE
            )
            .is_none());
    }

    /// A reflective XPlane must send a +x particle back along −x.
    #[test]
    fn reflective_plane_flips_direction() {
        let surfaces = vec![SurfaceKind::XPlane(XPlane {
            x0: 1.0,
            bc: BoundaryType::Reflective,
        })];
        let cells = vec![Cell::material(
            1,
            vec![RegionToken::HalfSpace {
                surface_idx: 0,
                sense: HalfSpaceSense::Inside,
            }],
            0,
            293.6,
        )];
        let universes = vec![Universe {
            id: 0,
            cell_indices: vec![0],
        }];
        let geom = Geometry {
            surfaces,
            cells,
            universes,
            lattices: vec![],
            root_universe: 0,
        };
        let crossed = geom.cross_surface(
            0,
            Position::new(1.0, 0.0, 0.0),
            Direction::new(1.0, 0.0, 0.0),
        );
        assert!(crossed.alive);
        assert!(
            (crossed.u.u + 1.0).abs() < 1e-12,
            "reflected u.u = {}",
            crossed.u.u
        );
        // The particle bounced back to the inside (negative) half-space.
        assert_eq!(
            crossed.on_surface,
            SurfaceToken::on(0, HalfSpaceSense::Inside)
        );
    }

    /// A sphere with a vacuum BC must kill the particle on crossing.
    #[test]
    fn vacuum_sphere_leaks() {
        let surfaces = vec![SurfaceKind::Sphere(Sphere {
            x0: 0.0,
            y0: 0.0,
            z0: 0.0,
            r: 5.0,
            bc: BoundaryType::Vacuum,
        })];
        let cells = vec![Cell::material(
            1,
            vec![RegionToken::HalfSpace {
                surface_idx: 0,
                sense: HalfSpaceSense::Inside,
            }],
            0,
            293.6,
        )];
        let universes = vec![Universe {
            id: 0,
            cell_indices: vec![0],
        }];
        let geom = Geometry {
            surfaces,
            cells,
            universes,
            lattices: vec![],
            root_universe: 0,
        };
        let crossed = geom.cross_surface(
            0,
            Position::new(5.0, 0.0, 0.0),
            Direction::new(1.0, 0.0, 0.0),
        );
        assert!(!crossed.alive, "vacuum crossing should kill the particle");
    }

    // ── GitHub #168 / op-mzvp.2.11 regression ─────────────────────────────
    //
    // A concentric-shell geometry — the shape that exposed the surface-tracking
    // defect. Four spherical regions, the outermost reflective, so nothing can
    // legitimately escape:
    //
    //   surface 0: r = 1  (transmissive)   cell 0: inside(0)               ball
    //   surface 1: r = 2  (transmissive)   cell 1: outside(0) & inside(1)  shell
    //   surface 2: r = 3  (transmissive)   cell 2: outside(1) & inside(2)  shell
    //   surface 3: r = 4  (reflective)     cell 3: outside(2) & inside(3)  shell
    //
    // The failure mode this guards against: a particle that has just crossed an
    // internal sphere sits exactly on it, `locate` re-derives the sign of
    // `evaluate` there, picks the cell it was LEAVING, and the next
    // `distance_to_boundary` finds no forward surface (the coincident one is
    // suppressed, the other bounds the cell behind it) — so the particle streams
    // to infinity and the history leaks.
    fn concentric_shells() -> Geometry {
        let tr = BoundaryType::Transmissive;
        let sphere = |r: f64, bc| {
            SurfaceKind::Sphere(Sphere {
                x0: 0.0,
                y0: 0.0,
                z0: 0.0,
                r,
                bc,
            })
        };
        let inside = |i| RegionToken::HalfSpace {
            surface_idx: i,
            sense: HalfSpaceSense::Inside,
        };
        let outside = |i| RegionToken::HalfSpace {
            surface_idx: i,
            sense: HalfSpaceSense::Outside,
        };
        let shell = |id, i_in, i_out, mat| {
            Cell::material(
                id,
                vec![outside(i_in), inside(i_out), RegionToken::Intersection],
                mat,
                293.6,
            )
        };
        Geometry {
            surfaces: vec![
                sphere(1.0, tr),
                sphere(2.0, tr),
                sphere(3.0, tr),
                sphere(4.0, BoundaryType::Reflective),
            ],
            cells: vec![
                Cell::material(1, vec![inside(0)], 0, 293.6),
                shell(2, 0, 1, 1),
                shell(3, 1, 2, 2),
                shell(4, 2, 3, 3),
            ],
            universes: vec![Universe {
                id: 0,
                cell_indices: vec![0, 1, 2, 3],
            }],
            lattices: vec![],
            root_universe: 0,
        }
    }

    /// **GitHub #168 regression.** Crossing every internal transmissive boundary
    /// of a concentric-shell geometry, in **both** directions, must land the
    /// particle in the adjacent cell — never back in the one it left — and must
    /// leave a finite forward boundary distance.
    ///
    /// Swept over near-radial *and* strongly grazing incidence: the grazing rays
    /// are the ones that broke, because there the tangential motion dominates and
    /// the sign of `evaluate` at the crossing point is pure round-off.
    #[test]
    fn crossing_a_shell_boundary_lands_in_the_adjacent_cell() {
        let geom = concentric_shells();
        // (surface index, radius, cell inside it, cell outside it)
        let boundaries = [
            (0usize, 1.0, 0usize, 1usize),
            (1, 2.0, 1, 2),
            (2, 3.0, 2, 3),
        ];
        // Radial through near-tangent. `tan` is the tangential component per unit
        // radial component: 1e6 is ~1e-6 rad off tangent.
        let tangential = [0.0, 1.0, 1.0e3, 1.0e6];

        for (i_surf, radius, cell_in, cell_out) in boundaries {
            for tan in tangential {
                for outward in [true, false] {
                    let radial = if outward { 1.0 } else { -1.0 };
                    // Hit point on the +x axis; radial along x, tangential along y.
                    let r_hit = Position::new(radius, 0.0, 0.0);
                    let u = Direction::from_unnormalised(radial, tan, 0.0);

                    let crossed = geom.cross_surface(i_surf, r_hit, u);
                    assert!(crossed.alive, "internal boundary must be transmissive");

                    let expect_cell = if outward { cell_out } else { cell_in };
                    let path = geom
                        .locate(crossed.r, crossed.u, crossed.on_surface)
                        .unwrap_or_else(|| {
                            panic!(
                                "lost after crossing surface {i_surf} (r={radius}, tan={tan}, \
                                 outward={outward}) — GH #168 regressed"
                            )
                        });
                    assert_eq!(
                        path.leaf().cell,
                        expect_cell,
                        "crossing surface {i_surf} (r={radius}, tan={tan}, outward={outward}) \
                         landed in cell {} not {expect_cell} — GH #168 regressed",
                        path.leaf().cell
                    );

                    // And the new cell must present a finite forward boundary:
                    // an INFINITY here is exactly how the leak started.
                    let hit = geom.distance_to_boundary(&path);
                    assert!(
                        hit.distance.is_finite(),
                        "no forward boundary from cell {expect_cell} after crossing surface \
                         {i_surf} (tan={tan}, outward={outward}) — GH #168 regressed"
                    );
                    assert!(
                        matches!(hit.crossing, Crossing::Surface(_)),
                        "expected a surface crossing ahead, got {:?}",
                        hit.crossing
                    );
                }
            }
        }
    }

    /// **GitHub #168 regression.** The outermost reflective sphere must send the
    /// particle back into the outer shell with its side recorded as `Inside`,
    /// at grazing incidence as well as head-on.
    #[test]
    fn reflecting_off_the_outer_sphere_stays_in_the_outer_shell() {
        let geom = concentric_shells();
        for tan in [0.0, 1.0, 1.0e3, 1.0e6] {
            let r_hit = Position::new(4.0, 0.0, 0.0);
            let u = Direction::from_unnormalised(1.0, tan, 0.0);
            let crossed = geom.cross_surface(3, r_hit, u);
            assert!(
                crossed.alive,
                "reflective sphere must not kill the particle"
            );
            assert_eq!(
                crossed.on_surface,
                SurfaceToken::on(3, HalfSpaceSense::Inside),
                "reflection must record the inward side (tan={tan})"
            );
            let path = geom
                .locate(crossed.r, crossed.u, crossed.on_surface)
                .unwrap_or_else(|| panic!("lost after reflecting (tan={tan}) — GH #168 regressed"));
            assert_eq!(
                path.leaf().cell,
                3,
                "must stay in the outer shell (tan={tan})"
            );
            assert!(
                geom.distance_to_boundary(&path).distance.is_finite(),
                "no forward boundary after reflecting (tan={tan}) — GH #168 regressed"
            );
        }
    }

    /// **GitHub #168 regression, end to end.** A full `run_keff_csg` power
    /// iteration over an all-reflective concentric-shell geometry must lose
    /// essentially no neutrons.
    ///
    /// # Methodology
    ///
    /// The [`concentric_shells`] model with fissile Godiva-like HEU in every
    /// region (so a lost history is not masked by absorption), transported
    /// through [`crate::physics::transport_csg::run_keff_csg_reactor_physics`]
    /// with the leakage spectrum enabled — 400 particles, 5 inactive + 10 active
    /// generations. Every surface but the outermost is transmissive and the
    /// outermost is reflective, so the *only* way to score leakage is a tracking
    /// failure. Pass criterion: total leakage < 1e-3 per source neutron.
    ///
    /// This is a **harness conservation check, not physics V&V** — it constrains
    /// the tracker, not the eigenvalue.
    ///
    /// # Results
    ///
    /// Measured 2026-09-10. With the two fixes below disabled — the
    /// [`SurfaceToken`] membership override and the direction-aware
    /// [`SurfaceKind::sense`] fallback — this geometry leaks **0.0860** per
    /// source neutron (k_eff 2.03432 ± 0.01644). With both in place it leaks
    /// **exactly 0.0** (k_eff 2.22599 ± 0.01072).
    ///
    /// The same disabled-mechanism run reproduces the reported 0.8463
    /// leakage on the four-region pebble
    /// ([`crate::pebble_beds::fhr_pebble`]'s
    /// `reflective_pebble_transport_does_not_leak`), so it is a faithful
    /// emulation of the pre-fix tracker rather than an approximation of it.
    /// The pebble is the more sensitive gate of the two; this test is here
    /// because it is minimal and needs no fissile-shell tuning to reproduce.
    #[test]
    fn reflective_concentric_shells_do_not_leak() {
        use crate::material::material::{Material, NuclideComponent};
        use crate::material::nuclide::Nuclide;
        use crate::physics::keff::KeffSettings;
        use crate::physics::transport_csg::{run_keff_csg_reactor_physics, SourceBox};
        use crate::tally::filter::EnergyFilter;
        use crate::tally::tally::{ScoreType, Tally, TallyBin};

        let nuclides = vec![
            Nuclide::from_core("U235").unwrap(),
            Nuclide::from_core("U238").unwrap(),
        ];
        let heu = |id| Material {
            id,
            name: "HEU".into(),
            temperature: 293.6,
            components: vec![
                NuclideComponent {
                    nuclide_idx: 0,
                    atom_density: 4.4994e-2,
                },
                NuclideComponent {
                    nuclide_idx: 1,
                    atom_density: 2.4984e-3,
                },
            ],
        };
        let materials = vec![heu(1), heu(2), heu(3), heu(4)];

        let geom = concentric_shells();
        let edges = vec![0.0, 0.625, 2.0e7];
        let mut tally = Tally {
            id: 0,
            name: "shells".into(),
            filters: vec![Box::new(EnergyFilter {
                bins: edges.clone(),
            })],
            scores: vec![ScoreType::Flux],
            bins: vec![TallyBin::default(); 2],
        };
        let mut leak = vec![TallyBin::default(); edges.len() - 1];
        let settings = KeffSettings {
            n_particles: 400,
            n_inactive: 5,
            n_active: 10,
            ..KeffSettings::default()
        };
        let source = SourceBox {
            lower: Position::new(-3.5, -3.5, -3.5),
            upper: Position::new(3.5, 3.5, 3.5),
        };
        let res = run_keff_csg_reactor_physics(
            &geom, &materials, &nuclides, source, &settings, &mut tally, &edges, &mut leak,
        );
        let leaked = leak
            .iter()
            .map(|b| b.mean(settings.n_active as u64))
            .sum::<f64>()
            / settings.n_particles as f64;
        eprintln!(
            "[GH #168] concentric shells: k_eff = {:.5} +/- {:.5}, leakage = {leaked:.3e}",
            res.k_mean, res.k_std
        );
        assert!(
            leaked < 1.0e-3,
            "all-reflective concentric shells leaked {leaked} per source neutron — GH #168 regressed"
        );
    }
}
