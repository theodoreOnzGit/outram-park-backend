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

use super::cell::CellFill;
use super::lattice::RectLattice;
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
    /// Global surface index the particle currently sits on (`usize::MAX` if none),
    /// used for coincident-distance handling.
    pub on_surface: usize,
}

impl GeometryPath {
    /// The leaf (lowest) coordinate level — where the material fill lives.
    #[inline]
    pub fn leaf(&self) -> &Coord {
        self.levels.last().expect("a located path has at least one level")
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
pub struct Geometry {
    /// Global surface array; region tokens and `on_surface` index into it.
    pub surfaces: Vec<SurfaceKind>,
    /// Global cell array; universes and paths index into it.
    pub cells: Vec<Cell>,
    /// Global universe array; the root and every fill index into it.
    pub universes: Vec<Universe>,
    /// Global lattice array; lattice-fill cells index into it.
    pub lattices: Vec<RectLattice>,
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
    /// `on_surface` is the global surface index the particle sits on
    /// (`usize::MAX` if none); it is carried through into the returned path for
    /// coincident-distance handling. Returns `None` if the particle is in no cell
    /// at some level (a "lost" particle — outside the geometry).
    pub fn locate(&self, r: Position, u: Direction, on_surface: usize) -> Option<GeometryPath> {
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
            let i_cell = self.universes[level.universe].find_cell(level.r, &self.surfaces, &self.cells)?;
            level.cell = i_cell;
            let cell = &self.cells[i_cell];

            match cell.fill {
                CellFill::Material(m) => {
                    levels.push(level);
                    return Some(GeometryPath { levels, material: Some(m), on_surface });
                }
                CellFill::Void => {
                    levels.push(level);
                    return Some(GeometryPath { levels, material: None, on_surface });
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
        let mut best = BoundaryHit { distance: f64::INFINITY, crossing: Crossing::None, coord_level: 0 };

        for (i, coord) in path.levels.iter().enumerate() {
            let cell = &self.cells[coord.cell];
            let (d_surf, i_surf) =
                cell.distance_to_boundary(coord.r, coord.u, &self.surfaces, path.on_surface);
            if d_surf < best.distance * (1.0 - FP_REL) {
                best.distance = d_surf;
                best.crossing = if i_surf == usize::MAX { Crossing::None } else { Crossing::Surface(i_surf) };
                best.coord_level = i;
            }

            if let Some(l_idx) = coord.lattice {
                let (d_lat, _trans) = self.lattices[l_idx].distance(coord.r, coord.u);
                if d_lat < best.distance * (1.0 - FP_REL) {
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
        let path = self.locate(r, u, usize::MAX)?;
        let m = path.material?;
        Some(materials[m].macro_xs_total(e, nuclides))
    }

    /// Apply a surface crossing to a global position/direction and return the
    /// post-crossing state plus whether the particle survives.
    ///
    /// The particle is assumed already streamed to the surface at global `r`.
    /// A **reflective** surface reflects `u` about its outward normal; a
    /// **vacuum** surface kills the particle (leak); transmissive/lattice
    /// crossings pass through unchanged. The returned position is nudged a hair
    /// past the surface along the outgoing direction so the next `locate` lands
    /// unambiguously on the far side.
    ///
    /// Mirrors the boundary-condition dispatch in `cross_surface`
    /// (`src/surface.cpp` / `src/geometry.cpp`), reduced to the vacuum/reflective/
    /// transmissive cases this crate implements.
    pub fn cross_surface(&self, i_surf: usize, r: Position, u: Direction) -> (Position, Direction, bool) {
        const NUDGE: f64 = 1.0e-9; // cm; << feature size (~1 cm), >> f64 round-off
        let surf = &self.surfaces[i_surf];
        match surf.bc() {
            BoundaryType::Vacuum => (r, u, false),
            BoundaryType::Reflective | BoundaryType::White | BoundaryType::Periodic => {
                // White/Periodic are approximated as reflective (documented gap).
                let u_new = surf.reflect(r, u);
                (stream(r, u_new, NUDGE), u_new, true)
            }
            BoundaryType::Transmissive => (stream(r, u, NUDGE), u, true),
        }
    }
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
            SurfaceKind::ZCylinder(ZCylinder { x0: 0.0, y0: 0.0, r: r_fuel, bc: BoundaryType::Transmissive }),
            SurfaceKind::XPlane(XPlane { x0: -half, bc: BoundaryType::Reflective }),
            SurfaceKind::XPlane(XPlane { x0: half, bc: BoundaryType::Reflective }),
            SurfaceKind::YPlane(YPlane { y0: -half, bc: BoundaryType::Reflective }),
            SurfaceKind::YPlane(YPlane { y0: half, bc: BoundaryType::Reflective }),
        ];
        // Fuel cell: inside the cylinder.
        let fuel = Cell::material(1, vec![RegionToken::HalfSpace { surface_idx: 0, sense: HalfSpaceSense::Inside }], 0, 293.6);
        // Moderator cell: outside cylinder AND inside the four planes.
        let mod_region = vec![
            RegionToken::HalfSpace { surface_idx: 0, sense: HalfSpaceSense::Outside },
            RegionToken::HalfSpace { surface_idx: 1, sense: HalfSpaceSense::Outside }, // x > -half
            RegionToken::Intersection,
            RegionToken::HalfSpace { surface_idx: 2, sense: HalfSpaceSense::Inside },  // x < +half
            RegionToken::Intersection,
            RegionToken::HalfSpace { surface_idx: 3, sense: HalfSpaceSense::Outside }, // y > -half
            RegionToken::Intersection,
            RegionToken::HalfSpace { surface_idx: 4, sense: HalfSpaceSense::Inside },  // y < +half
            RegionToken::Intersection,
        ];
        let moder = Cell::material(2, mod_region, 1, 293.6);
        let cells = vec![fuel, moder];
        let universes = vec![Universe { id: 0, cell_indices: vec![0, 1] }];
        let geom = Geometry { surfaces, cells, universes, lattices: vec![], root_universe: 0 };

        // At the origin: inside the fuel.
        let p = geom.locate(Position::new(0.0, 0.0, 0.0), Direction::new(1.0, 0.0, 0.0), usize::MAX).unwrap();
        assert_eq!(p.material, Some(0), "origin is fuel");
        let hit = geom.distance_to_boundary(&p);
        assert!((hit.distance - r_fuel).abs() < 1e-9, "fuel→wall distance {}", hit.distance);
        assert_eq!(hit.crossing, Crossing::Surface(0));

        // Between cylinder and box: moderator.
        let p2 = geom.locate(Position::new(0.5, 0.0, 0.0), Direction::new(1.0, 0.0, 0.0), usize::MAX).unwrap();
        assert_eq!(p2.material, Some(1), "0.5 cm out is moderator");

        // Outside the box: lost.
        assert!(geom.locate(Position::new(1.0, 0.0, 0.0), Direction::new(1.0, 0.0, 0.0), usize::MAX).is_none());
    }

    /// A reflective XPlane must send a +x particle back along −x.
    #[test]
    fn reflective_plane_flips_direction() {
        let surfaces = vec![SurfaceKind::XPlane(XPlane { x0: 1.0, bc: BoundaryType::Reflective })];
        let cells = vec![Cell::material(1, vec![RegionToken::HalfSpace { surface_idx: 0, sense: HalfSpaceSense::Inside }], 0, 293.6)];
        let universes = vec![Universe { id: 0, cell_indices: vec![0] }];
        let geom = Geometry { surfaces, cells, universes, lattices: vec![], root_universe: 0 };
        let (_r, u, alive) = geom.cross_surface(0, Position::new(1.0, 0.0, 0.0), Direction::new(1.0, 0.0, 0.0));
        assert!(alive);
        assert!((u.u + 1.0).abs() < 1e-12, "reflected u.u = {}", u.u);
    }

    /// A sphere with a vacuum BC must kill the particle on crossing.
    #[test]
    fn vacuum_sphere_leaks() {
        let surfaces = vec![SurfaceKind::Sphere(Sphere { x0: 0.0, y0: 0.0, z0: 0.0, r: 5.0, bc: BoundaryType::Vacuum })];
        let cells = vec![Cell::material(1, vec![RegionToken::HalfSpace { surface_idx: 0, sense: HalfSpaceSense::Inside }], 0, 293.6)];
        let universes = vec![Universe { id: 0, cell_indices: vec![0] }];
        let geom = Geometry { surfaces, cells, universes, lattices: vec![], root_universe: 0 };
        let (_r, _u, alive) = geom.cross_surface(0, Position::new(5.0, 0.0, 0.0), Direction::new(1.0, 0.0, 0.0));
        assert!(!alive, "vacuum crossing should kill the particle");
    }
}
