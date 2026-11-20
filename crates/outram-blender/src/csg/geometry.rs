// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Ported from OpenMC (https://github.com/openmc-dev/openmc, MIT licence,
// Copyright (c) 2011-2026 Massachusetts Institute of Technology, UChicago
// Argonne LLC and OpenMC contributors; see LICENSE.openmc):
//   src/geometry.cpp, include/openmc/geometry.h
// locate_particle (find_cell_inner) and distance_to_boundary.
// Moved here unchanged in substance from `outram-mc-libs`
// (`src/geometry/`) on 2026-10-02, GitHub issue #486: outram-blender owns the
// CSG description and its pure navigation kernel, outram-mc-libs keeps the
// transport-state work and re-exports these items under its old paths.
//
// This file is part of OUTRAM PARK.
//
// OUTRAM PARK is free software: you can redistribute it and/or modify it
// under the terms of the GNU General Public License as published by the
// Free Software Foundation, either version 3 of the License, or (at your
// option) any later version.
//
// OUTRAM PARK is distributed in the hope that it will be useful, but
// WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the GNU
// General Public License for more details.
//
// You should have received a copy of the GNU General Public License along
// with OUTRAM PARK.  If not, see <https://www.gnu.org/licenses/>.

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

#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
use crate::nostd_prelude::*;
use super::cell::{CellFill, SurfaceToken};
use super::lattice::Lattice;
use super::position::{Direction, Position};
use super::universe::Universe;
use crate::csg::cell::{Cell, TrackingMethod};
use super::surface::SurfaceKind;

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
    /// **Exact global -> local frame offset for this level**: `r` here equals
    /// the global position minus this, and a direction needs no transformation
    /// because every nested frame in this crate is a pure translation.
    ///
    /// Accumulated on the way down (`parent.offset + cell.translation`, plus the
    /// lattice tile centre for a lattice level) rather than recovered afterwards
    /// as `levels[0].r - levels[k].r`. That subtraction is catastrophic
    /// cancellation — with a probe at `y = -9` and a translation of `0.2` it
    /// returns `0.19999999999999929` — and the ~1e-16 error it leaves in the
    /// local coordinate is enough to put a crossing point exactly on
    /// `dot == 0.0` in `nudge_across`, flipping that branch and displacing the
    /// particle by `1e-9`, a 10^6 amplification. Carrying the offset removes the
    /// cancellation entirely. Found by `tests/cell_translation.rs`; it affects
    /// lattice tile centres too, not only non-zero cell translations.
    pub offset: Position,
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
    /// **How this point is to be transported** — the deepest
    /// [`TrackingMethod`] declared on the path from root to leaf.
    ///
    /// A region's method is inherited by everything nested inside it, so a
    /// delta-tracked bed makes its pebble and TRISO universes delta-tracked
    /// too, without each of them restating it. A deeper cell may override,
    /// which is how a surface-tracked control-rod channel is carved out of a
    /// delta-tracked bed.
    ///
    /// NEW WORK, no OpenMC counterpart — see [`TrackingMethod`] (`bn:op-867c.1`).
    pub tracking: TrackingMethod,
    /// Index into [`Self::levels`] of the cell that **declared** [`Self::tracking`],
    /// or `0` when nothing on the path declared anything (the default,
    /// surface-tracked case).
    ///
    /// This is what makes a delta region's *extent* knowable. Delta tracking
    /// must stop at the edge of the region that chose it, and
    /// [`Geometry::distance_to_boundary`] cannot answer that: it returns the
    /// nearest boundary at **any** level, which inside a finely divided bed is
    /// usually a pebble or TRISO surface far inside the region. Pair this with
    /// outram-mc-libs' `GeometryExt::distance_out_of_level`.
    ///
    /// NEW WORK, no OpenMC counterpart (`bn:op-867c.4`).
    pub tracking_level: usize,
    /// The coordinate level at which [`Self::on_surface`] names a surface, or
    /// `None` for "every level" (the behaviour of [`Geometry::locate`]). Set by
    /// [`Geometry::locate_at_level`] (gh:#698): a surface index names the same
    /// physical plane only at the level where the particle crossed it, so the
    /// token is not consulted at any other level.
    pub on_level: Option<usize>,
}

impl GeometryPath {
    /// The surface token to apply to the cell at coordinate level `level`:
    /// [`Self::on_surface`] where it names a surface, [`SurfaceToken::NONE`]
    /// at every other level when [`Self::on_level`] is set (gh:#698).
    #[inline]
    pub fn token_at(&self, level: usize) -> SurfaceToken {
        match self.on_level {
            Some(l) if l != level => SurfaceToken::NONE,
            _ => self.on_surface,
        }
    }

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

/// The whole CSG model — the flat arrays every geometry index refers to.
///
/// Read-only after construction; share across threads as `Arc<Geometry>`.
/// Maps to OpenMC's `model::{surfaces,cells,universes,lattices}` globals plus
/// `model::root_universe`.
#[derive(Debug, Clone)]
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
    /// **Ablate cell temperatures** (GitHub #313): a copy in which every cell
    /// is "unset" (`temperature: None`), so every material cell takes its
    /// material's temperature and `bind_cell_temperatures` changes nothing.
    ///
    /// Cell temperature is the transport default, as in OpenMC; this is the
    /// explicit, visible way to run without it.
    #[must_use]
    pub fn without_cell_temperatures(&self) -> Self {
        let mut g = self.clone();
        for c in &mut g.cells {
            c.temperature = None;
        }
        g
    }

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
        self.locate_impl(r, u, on_surface, None)
    }

    /// [`Self::locate`] with `on_surface` applied only at coordinate level
    /// `token_level` (gh:#698): the level of the crossing that produced the token
    /// (`BoundaryHit::coord_level`). OpenMC restarts the cell search at the
    /// crossing's own level (`cross_surface`, `particle.cpp:345`), so a surface
    /// index never constrains a cell in another level or another tile of a
    /// repeated universe, where it names a different physical plane. The returned
    /// path carries `on_level = Some(token_level)`, which the distance queries
    /// honour through [`GeometryPath::token_at`].
    pub fn locate_at_level(
        &self,
        r: Position,
        u: Direction,
        on_surface: SurfaceToken,
        token_level: usize,
    ) -> Option<GeometryPath> {
        self.locate_impl(r, u, on_surface, Some(token_level))
    }

    fn locate_impl(
        &self,
        r: Position,
        u: Direction,
        on_surface: SurfaceToken,
        token_level: Option<usize>,
    ) -> Option<GeometryPath> {
        let mut levels: Vec<Coord> = Vec::new();
        let mut level = Coord {
            universe: self.root_universe,
            cell: usize::MAX,
            r,
            u,
            lattice: None,
            lattice_index: [0; 3],
            offset: Position::ZERO,
        };

        // Inheritance: a cell that declares nothing (`None`) keeps whatever the
        // enclosing region chose, so a delta-tracked bed makes its pebble and
        // TRISO universes delta-tracked without each restating it. A cell that
        // declares `Some(..)` overrides — including `Some(Surface)`, which is
        // how a surface-tracked rod channel sits inside a delta-tracked bed.
        let mut tracking = TrackingMethod::Surface;
        let mut tracking_level = 0_usize;
        loop {
            let i_cell = self.universes[level.universe].find_cell(
                level.r,
                level.u,
                &self.surfaces,
                &self.cells,
                match token_level {
                    Some(l) if l != levels.len() => SurfaceToken::NONE,
                    _ => on_surface,
                },
            )?;
            level.cell = i_cell;
            let cell = &self.cells[i_cell];
            if let Some(declared) = cell.tracking {
                tracking = declared;
                // `levels` holds the ancestors; this cell's own level is next.
                tracking_level = levels.len();
            }

            match cell.fill {
                CellFill::Material(m) => {
                    levels.push(level);
                    return Some(GeometryPath {
                        levels,
                        material: Some(m),
                        on_surface,
                        tracking,
                        tracking_level,
                        on_level: token_level,
                    });
                }
                CellFill::Void => {
                    levels.push(level);
                    return Some(GeometryPath {
                        levels,
                        material: None,
                        on_surface,
                        tracking,
                        tracking_level,
                        on_level: token_level,
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
                        offset: level.offset + cell.translation,
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
                        offset: level.offset + cell.translation + lat.tile_center(idx),
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
    /// Ported from `distance_to_boundary` (`src/geometry.cpp:480-518`): each level
    /// contributes its cell's nearest bounding-surface distance and, if the level
    /// is a lattice tile, the distance to the next tile edge. A lower level wins
    /// only by `FP_REL_PRECISION` (1e-5) relative, so a boundary coincident with a
    /// higher-level one resolves to the higher level (gh:#698).
    ///
    /// ~~Because nested frames here are pure translations, the global
    /// `on_surface` index is valid at every level.~~ **CORRECTED 2026-11-19**
    /// (gh:#698): false for a universe repeated in a lattice, where surface index
    /// `i` names a different physical plane in every tile; that is why the
    /// higher-level boundary must win a coincident tie.
    ///
    /// (Doc comment restored 2026-10-02, GitHub #486: in the former
    /// outram-mc-libs file it sat above `distance_out_of_level`'s and was
    /// rendered as part of that method's docs.)
    pub fn distance_to_boundary(&self, path: &GeometryPath) -> BoundaryHit {
        // `distance_to_boundary`, OpenMC `src/geometry.cpp:480-518` (gh:#698).
        // Levels run from the highest (root) downwards. A lower level replaces
        // the best so far only if it is shorter by `FP_REL_PRECISION` relative,
        // so a lower-level surface coincident with a higher-level boundary (a
        // lattice tile edge, the parent cell's wall) never wins on round-off.
        // Within one level the surface wins only if it is shorter than the
        // lattice distance by `FP_COINCIDENT` (upstream's `d_surf < d_lat -
        // FP_COINCIDENT`), else the lattice edge.
        // ~~`FP_REL = 1e-14`, a surface preferred on a 1e-9 cm tie, and the
        // lower level winning on round-off~~ **CORRECTED 2026-11-19** (gh:#698).
        const FP_REL_PRECISION: f64 = 1.0e-5;
        let mut best = BoundaryHit {
            distance: f64::INFINITY,
            crossing: Crossing::None,
            coord_level: 0,
        };

        for (i, coord) in path.levels.iter().enumerate() {
            let cell = &self.cells[coord.cell];
            let (d_surf, i_surf) =
                cell.distance_to_boundary(coord.r, coord.u, &self.surfaces, path.token_at(i));
            let d_lat = match coord.lattice {
                Some(l_idx) => {
                    self.lattices[l_idx]
                        .distance(coord.r, coord.u, coord.lattice_index)
                        .0
                }
                None => f64::INFINITY,
            };
            let beats = |d: f64, best_d: f64| {
                best_d == f64::INFINITY || (best_d - d) / best_d >= FP_REL_PRECISION
            };

            if d_surf < d_lat - super::surface::FP_COINCIDENT {
                if beats(d_surf, best.distance) {
                    best.distance = d_surf;
                    best.crossing = if i_surf == usize::MAX {
                        Crossing::None
                    } else {
                        Crossing::Surface(i_surf)
                    };
                    best.coord_level = i;
                }
            } else if coord.lattice.is_some() && beats(d_lat, best.distance) {
                best.distance = d_lat;
                best.crossing = Crossing::Lattice;
                best.coord_level = i;
            }
        }
        best
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::csg::cell::{Cell, HalfSpaceSense, RegionToken};
    use crate::csg::surface::{BoundaryType, SurfaceKind};
    use crate::csg::surface::{XPlane, YPlane, ZCylinder};

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

    /// gh:#698: a pin universe bounded by its own planes at the tile edges, in a
    /// 2x2 lattice. At a point whose local `x = +h` plane and the lattice tile
    /// edge coincide, `distance_to_boundary` must return the LATTICE crossing, as
    /// OpenMC does (`d_surf < d_lat - FP_COINCIDENT` is false), not the tile-local
    /// plane whose index names a different plane in the next tile.
    #[test]
    fn coincident_tile_local_plane_loses_to_the_lattice_edge() {
        use crate::csg::cell::CellFill;
        use crate::csg::lattice::RectLattice;
        let pitch = 1.26;
        let h = 0.63;
        let plane = |x0: f64| SurfaceKind::XPlane(XPlane { x0, bc: BoundaryType::Transmissive });
        let yplane = |y0: f64| SurfaceKind::YPlane(YPlane { y0, bc: BoundaryType::Transmissive });
        let surfaces = vec![
            plane(0.0),
            plane(2.0 * pitch),
            yplane(0.0),
            yplane(2.0 * pitch),
            plane(-h),
            plane(h),
            yplane(-h),
            yplane(h),
        ];
        let hs = |i: usize, sense| RegionToken::HalfSpace { surface_idx: i, sense };
        let and = RegionToken::Intersection;
        let root = Cell::fill(
            1,
            vec![
                hs(0, HalfSpaceSense::Outside),
                hs(1, HalfSpaceSense::Inside),
                and,
                hs(2, HalfSpaceSense::Outside),
                and,
                hs(3, HalfSpaceSense::Inside),
                and,
            ],
            CellFill::Lattice(0),
            Position::ZERO,
        );
        let pin_mod = Cell::material(
            2,
            vec![
                hs(4, HalfSpaceSense::Outside),
                hs(5, HalfSpaceSense::Inside),
                and,
                hs(6, HalfSpaceSense::Outside),
                and,
                hs(7, HalfSpaceSense::Inside),
                and,
            ],
            0,
            293.6,
        );
        let lattice = RectLattice {
            id: 1,
            n: [2, 2, 1],
            lower_left: Position::new(0.0, 0.0, -0.5),
            pitch: [pitch, pitch, 1.0],
            universes: vec![1, 1, 1, 1],
            outer: None,
        };
        let geom = Geometry {
            surfaces,
            cells: vec![root, pin_mod],
            universes: vec![
                Universe { id: 0, cell_indices: vec![0] },
                Universe { id: 1, cell_indices: vec![1] },
            ],
            lattices: vec![crate::csg::lattice::Lattice::Rect(lattice)],
            root_universe: 0,
        };
        // Tile (0,0), local x = 0.37: the local x = +h plane and the tile edge are
        // both 0.26 cm away along +x.
        let p = geom
            .locate(
                Position::new(1.0, 0.63, 0.0),
                Direction::new(1.0, 0.0, 0.0),
                SurfaceToken::NONE,
            )
            .unwrap();
        let hit = geom.distance_to_boundary(&p);
        assert!((hit.distance - 0.26).abs() < 1e-12, "d = {}", hit.distance);
        assert_eq!(hit.crossing, Crossing::Lattice, "tile edge must beat the coincident local plane");
    }

    /// gh:#698 (token level): surface 0 is used by the ROOT cell (global `x > 0`)
    /// and by the pin universe in a lattice tile centred at x = 10.5 (local
    /// `x < 0`, i.e. a different physical plane at x = 10.5). A particle that has
    /// just crossed the tile-local plane moving to its inside carries the token
    /// (0, Inside). Applied at every level (`locate`) the token is also forced on
    /// the root cell, whose region wants Outside(0), so the search fails; applied
    /// only at the crossing's level (`locate_at_level`) the root cell is judged by
    /// the particle's real position and the particle is found in the inside pin
    /// cell.
    #[test]
    fn the_surface_token_applies_only_at_the_crossing_level() {
        use crate::csg::cell::CellFill;
        use crate::csg::lattice::{Lattice, RectLattice};
        let plane = |x0: f64| SurfaceKind::XPlane(XPlane { x0, bc: BoundaryType::Transmissive });
        let hs = |i: usize, sense| RegionToken::HalfSpace { surface_idx: i, sense };
        let (out, ins, and) = (HalfSpaceSense::Outside, HalfSpaceSense::Inside, RegionToken::Intersection);
        let root = Cell::fill(
            1,
            vec![hs(0, out), hs(1, ins), and],
            CellFill::Lattice(0),
            Position::ZERO,
        );
        let inside_pin = Cell::material(2, vec![hs(0, ins)], 7, 293.6);
        let outside_pin = Cell::material(3, vec![hs(0, out)], 8, 293.6);
        let geom = Geometry {
            surfaces: vec![plane(0.0), plane(20.0)],
            cells: vec![root, inside_pin, outside_pin],
            universes: vec![
                Universe { id: 0, cell_indices: vec![0] },
                Universe { id: 1, cell_indices: vec![1, 2] },
            ],
            lattices: vec![Lattice::Rect(RectLattice {
                id: 1,
                n: [1, 1, 1],
                lower_left: Position::new(10.0, -0.5, -0.5),
                pitch: [1.0, 1.0, 1.0],
                universes: vec![1],
                outer: None,
            })],
            root_universe: 0,
        };
        let r = Position::new(10.5 - 1.0e-9, 0.0, 0.0);
        let u = Direction::new(-1.0, 0.0, 0.0);
        let token = SurfaceToken::on(0, HalfSpaceSense::Inside);
        assert!(geom.locate(r, u, token).is_none(), "token at every level rejects the root cell");
        let p = geom.locate_at_level(r, u, token, 1).expect("found with the token at its own level");
        assert_eq!(p.material, Some(7), "the inside pin cell");
        assert_eq!(p.token_at(0), SurfaceToken::NONE);
        assert_eq!(p.token_at(1), token);
    }
}
