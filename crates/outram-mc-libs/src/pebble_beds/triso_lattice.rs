//! **Randomly packed particles in a tracking lattice**: a port of OpenMC's
//! `openmc.model.create_triso_lattice` (gh #926, tutorial 5 on #943).
//!
//! # Provenance
//!
//! OpenMC (MIT), `openmc/model/triso.py`: `create_triso_lattice` (line 805)
//! and `TRISO.classify` (line 74), with `RectLattice.find_element` /
//! `get_local_coordinates` (`openmc/lattice.py:502`, `:528`); commit
//! `d7d3284a1b13d7cae020e0d8fea9f1e60d91b18b` of the local checkout at
//! `/home/teddy0/Documents/research/openmc/`. Copyright (c) 2011-2025
//! Massachusetts Institute of Technology, UChicago Argonne LLC, and OpenMC
//! contributors; MIT licence (see `geometry::virtual_lattice` for the full
//! notice). This Rust translation is GPL-3.0-only.
//!
//! # What it does
//!
//! Randomly packed particles do not sit one per lattice tile. OpenMC's answer
//! is a **tracking lattice**: a rectangular grid laid over the particles, in
//! which each tile's universe holds a copy of **every particle that overlaps
//! the tile** (by its bounding box, `TRISO.classify`), each copy a cell
//! `inside(sphere)` filled with the particle's universe and translated to the
//! particle's centre in tile-local coordinates; a background cell (`outside`
//! every copy) is the matrix. A particle crossing a tile face is in both
//! tiles, and each tile draws only its own piece, so the particle is whole.
//! It changes only how fast a neutron finds the next surface (a few spheres
//! per tile instead of all of them), never the geometry.
//!
//! # Where this differs from upstream, and why
//!
//! - **Tiles that no particle touches share one universe**, which is also the
//!   lattice's `outer` (upstream makes a fresh universe per empty tile and
//!   another for `outer`). Same geometry, fewer cells.
//! - **An empty region here is a far plane's `-p ∪ +p`** (`x = 1e30` cm):
//!   in this crate an empty token stream is FALSE, not "everywhere"
//!   (nee_soon's TRISO matrix universe records the hole that left), so the
//!   background of an empty tile needs a region that holds every point. It
//!   is the first surface of the output, made once.
//! - Particles whose bounding box leaves the lattice are counted
//!   ([`TrisoLattice::partly_outside`]) where upstream warns.

use crate::geometry::cell::{Cell, CellFill, HalfSpaceSense, RegionToken};
use crate::geometry::lattice::RectLattice;
use crate::geometry::position::Position;
use crate::geometry::surface::{BoundaryType, Sphere, SurfaceKind, XPlane};
use crate::geometry::universe::Universe;

/// Where the output starts in the caller's arrays, so it can be appended
/// as is: every surface, cell and universe index in the output is absolute.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TrisoLatticeBase {
    /// Index of the first surface the output adds.
    pub surface: usize,
    /// Index of the first cell the output adds.
    pub cell: usize,
    /// Index of the first universe the output adds.
    pub universe: usize,
}

/// What [`create_triso_lattice`] builds, to append to a geometry.
#[derive(Debug, Clone)]
pub struct TrisoLattice {
    /// New surfaces: the far plane, then one sphere per particle copy.
    pub surfaces: Vec<SurfaceKind>,
    /// New cells: the empty tile's background, then per occupied tile its
    /// background cell and its particle copies.
    pub cells: Vec<Cell>,
    /// New universes: the empty universe (also `outer`), then one per
    /// occupied tile, in natural order (x fastest).
    pub universes: Vec<Universe>,
    /// The lattice (`universes` index the caller's universe array).
    pub lattice: RectLattice,
    /// Absolute indices of the background (matrix) cells.
    pub background_cells: Vec<usize>,
    /// Absolute indices of the particle-copy cells (filled with the
    /// particle universe).
    pub copy_cells: Vec<usize>,
    /// Particles whose bounding box leaves the lattice (upstream warns
    /// "partially or completely outside of the lattice"); their copies in
    /// the missing tiles are not made.
    pub partly_outside: usize,
}

/// **OpenMC's `create_triso_lattice`** (see the module docs).
///
/// - `centres`: the particle centres, in the frame the lattice is placed in.
/// - `particle_radius`: the radius of the region each copy fills (the
///   particle's outermost sphere).
/// - `particle_universe`: the universe of one particle centred at its origin
///   (its layers, and a matrix cell outside them so a point on the outer
///   surface is never lost, as `build_triso_particle`'s matrix cell does).
/// - `lower_left`, `pitch`, `shape`: the lattice.
/// - `background`: the matrix material index; `temperature` \[K\] for the
///   cells.
/// - `base`, `lattice_id`: where the output starts (see
///   [`TrisoLatticeBase`]).
#[allow(clippy::too_many_arguments)]
pub fn create_triso_lattice(
    centres: &[Position],
    particle_radius: f64,
    particle_universe: usize,
    lower_left: Position,
    pitch: [f64; 3],
    shape: [usize; 3],
    background: usize,
    temperature: f64,
    base: TrisoLatticeBase,
    lattice_id: i32,
) -> TrisoLattice {
    let n_tiles = shape[0] * shape[1] * shape[2];
    let mut lattice = RectLattice {
        id: lattice_id,
        n: shape,
        lower_left,
        pitch,
        universes: Vec::new(),
        outer: None,
    };
    // `TRISO.classify` + the `triso_locations` dictionary: per tile (natural
    // order), the particles whose bounding box overlaps it.
    let mut per_tile: Vec<Vec<usize>> = vec![Vec::new(); n_tiles];
    let mut partly_outside = 0;
    let element = |p: [f64; 3]| -> [i64; 3] {
        let ll = [lower_left.x, lower_left.y, lower_left.z];
        std::array::from_fn(|i| ((p[i] - ll[i]) / pitch[i]).floor() as i64)
    };
    for (k, c) in centres.iter().enumerate() {
        let c = [c.x, c.y, c.z];
        let lo = element(std::array::from_fn(|i| c[i] - particle_radius));
        let hi = element(std::array::from_fn(|i| c[i] + particle_radius));
        let mut outside = false;
        for iz in lo[2]..=hi[2] {
            for iy in lo[1]..=hi[1] {
                for ix in lo[0]..=hi[0] {
                    let i = [ix, iy, iz];
                    if (0..3).any(|a| i[a] < 0 || i[a] >= shape[a] as i64) {
                        outside = true;
                        continue;
                    }
                    let flat =
                        shape[0] * shape[1] * iz as usize + shape[0] * iy as usize + ix as usize;
                    per_tile[flat].push(k);
                }
            }
        }
        if outside {
            partly_outside += 1;
        }
    }

    let half = |s: usize, sense: HalfSpaceSense| RegionToken::HalfSpace {
        surface_idx: s,
        sense,
    };
    let mut surfaces = vec![SurfaceKind::XPlane(XPlane {
        x0: 1.0e30,
        bc: BoundaryType::Transmissive,
    })];
    let everywhere = vec![
        half(base.surface, HalfSpaceSense::Inside),
        half(base.surface, HalfSpaceSense::Outside),
        RegionToken::Union,
    ];
    let mut cells = vec![Cell::material(
        base.cell as i32 + 1,
        everywhere,
        background,
        temperature,
    )];
    let mut universes = vec![Universe {
        id: base.universe as i32,
        cell_indices: vec![base.cell],
    }];
    let empty = base.universe;
    let mut background_cells = vec![base.cell];
    let mut copy_cells = Vec::new();
    lattice.outer = Some(empty);
    lattice.universes = vec![empty; n_tiles];

    for (flat, members) in per_tile.iter().enumerate() {
        if members.is_empty() {
            continue;
        }
        let i = [
            (flat % shape[0]) as i32,
            ((flat / shape[0]) % shape[1]) as i32,
            (flat / (shape[0] * shape[1])) as i32,
        ];
        let centre = lattice.tile_center(i);
        // One sphere per copy, at the particle's tile-local centre
        // (`get_local_coordinates`), and its cell: inside the sphere,
        // filled with the particle universe, translated to that centre.
        let mut outside_all = Vec::new();
        let mut copies = Vec::new();
        for &k in members {
            let c = centres[k];
            let local = Position::new(c.x - centre.x, c.y - centre.y, c.z - centre.z);
            surfaces.push(SurfaceKind::Sphere(Sphere {
                x0: local.x,
                y0: local.y,
                z0: local.z,
                r: particle_radius,
                bc: BoundaryType::Transmissive,
            }));
            let s = base.surface + surfaces.len() - 1;
            outside_all.push(half(s, HalfSpaceSense::Outside));
            if outside_all.len() > 1 {
                outside_all.push(RegionToken::Intersection);
            }
            copies.push((s, local));
        }
        // The background first, then the copies (upstream's order).
        let mut cell_indices = Vec::with_capacity(1 + copies.len());
        let bg = base.cell + cells.len();
        cells.push(Cell::material(
            bg as i32 + 1,
            outside_all,
            background,
            temperature,
        ));
        background_cells.push(bg);
        cell_indices.push(bg);
        for (s, local) in copies {
            let ci = base.cell + cells.len();
            cells.push(Cell::fill(
                ci as i32 + 1,
                vec![half(s, HalfSpaceSense::Inside)],
                CellFill::Universe(particle_universe),
                local,
            ));
            copy_cells.push(ci);
            cell_indices.push(ci);
        }
        let u = base.universe + universes.len();
        universes.push(Universe {
            id: u as i32,
            cell_indices,
        });
        lattice.universes[flat] = u;
    }

    TrisoLattice {
        surfaces,
        cells,
        universes,
        lattice,
        background_cells,
        copy_cells,
        partly_outside,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::cell::SurfaceToken;
    use crate::geometry::geometry::Geometry;
    use crate::geometry::position::Direction;

    /// Two particles, one straddling a tile face: each tile holds a copy of
    /// every particle touching it, the empty tiles share the `outer`
    /// universe, and a point anywhere in either particle finds the particle
    /// material, whichever tile it is in.
    #[test]
    fn a_particle_on_a_tile_face_is_in_both_tiles_and_whole() {
        // Universe 0: the particle (material 1) and matrix outside it
        // (material 0), as `build_triso_particle` writes them.
        let r = 0.1;
        let particle_surfaces = vec![SurfaceKind::Sphere(Sphere {
            x0: 0.0,
            y0: 0.0,
            z0: 0.0,
            r,
            bc: BoundaryType::Transmissive,
        })];
        let hs = |sense| RegionToken::HalfSpace {
            surface_idx: 0,
            sense,
        };
        let particle_cells = vec![
            Cell::material(1, vec![hs(HalfSpaceSense::Inside)], 1, 300.0),
            Cell::material(2, vec![hs(HalfSpaceSense::Outside)], 0, 300.0),
        ];
        let centres = [
            Position::new(0.0, 0.25, 0.25),
            Position::new(0.75, 0.75, 0.25),
        ];
        let t = create_triso_lattice(
            &centres,
            r,
            0,
            Position::new(-0.5, 0.0, 0.0),
            [0.5; 3],
            [3, 2, 1],
            0,
            300.0,
            TrisoLatticeBase {
                surface: 1,
                cell: 2,
                universe: 1,
            },
            1,
        );
        assert_eq!(t.partly_outside, 0);
        // Particle 0 sits on the face x = 0 between tiles (0,0) and (1,0):
        // two copies. Particle 1 is inside tile (2,1).
        assert_eq!(t.copy_cells.len(), 3);
        assert_eq!(t.lattice.universes, vec![2, 3, 1, 1, 1, 4]);
        assert_eq!(t.lattice.outer, Some(1));
        assert_eq!(t.background_cells.len(), 4);
        // The copy in tile (0,0) is centred at x = +0.25 there.
        let SurfaceKind::Sphere(s) = t.surfaces[1] else {
            panic!()
        };
        assert_eq!((s.x0, s.y0, s.z0), (0.25, 0.0, 0.0));

        // Assemble and look the material up.
        let mut surfaces = particle_surfaces;
        surfaces.extend(t.surfaces.clone());
        let mut cells = particle_cells;
        cells.extend(t.cells.clone());
        // The container: a box of planes would do; here a root cell filled
        // with the lattice over "everywhere" (the far plane).
        let far = 1;
        cells.push(Cell::fill(
            99,
            vec![
                RegionToken::HalfSpace {
                    surface_idx: far,
                    sense: HalfSpaceSense::Inside,
                },
                RegionToken::HalfSpace {
                    surface_idx: far,
                    sense: HalfSpaceSense::Outside,
                },
                RegionToken::Union,
            ],
            CellFill::Lattice(0),
            Position::ZERO,
        ));
        let mut universes = vec![Universe {
            id: 0,
            cell_indices: vec![0, 1],
        }];
        universes.extend(t.universes.clone());
        universes.push(Universe {
            id: 9,
            cell_indices: vec![cells.len() - 1],
        });
        let g = Geometry {
            surfaces,
            cells,
            universes,
            lattices: vec![crate::geometry::lattice::Lattice::Rect(t.lattice.clone())],
            root_universe: 5,
        };
        let mat = |x: f64, y: f64, z: f64| {
            g.locate(
                Position::new(x, y, z),
                Direction::new(1.0, 0.0, 0.0),
                SurfaceToken::NONE,
            )
            .and_then(|p| p.material)
        };
        // Both halves of the straddling particle, the other particle, matrix.
        assert_eq!(mat(-0.05, 0.25, 0.25), Some(1));
        assert_eq!(mat(0.05, 0.25, 0.25), Some(1));
        assert_eq!(mat(0.75, 0.80, 0.25), Some(1));
        assert_eq!(mat(0.25, 0.25, 0.25), Some(0));
        assert_eq!(mat(-0.25, 0.75, 0.25), Some(0), "an empty tile");
    }
}
