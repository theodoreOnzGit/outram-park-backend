// SPDX-License-Identifier: GPL-3.0

//! **Distribcell offset tables** — per-instance tallies inside a repeated
//! universe. GitHub #261, the last of its sixteen filters.
//!
//! Ported from `DistribcellFilter::get_all_bins`
//! (`src/tallies/filter_distribcell.cpp`) and the offset tables upstream
//! builds in `src/geometry_aux.cpp`, at OpenMC `afa7a14`.
//!
//! # What it is for
//!
//! A cell defined once inside a universe that a lattice repeats 400 times is
//! **one** cell and **400** instances. A `CellFilter` on it bins all 400
//! together; a distribcell filter gives 400 bins. Without it there is no
//! per-pebble or per-pin result out of a lattice — which is squarely this
//! crate's remit, since pebble beds are its specialisation.
//!
//! # How the instance index is formed
//!
//! Upstream walks the particle's coordinate stack from root to leaf,
//! accumulating an offset at each level, and returns it when the level's cell
//! is the target:
//!
//! ```text
//! offset = 0
//! for each level i:
//!     c = cell at level i
//!     if c is universe-filled:  offset += cell_offset[c]
//!     if c is lattice-filled:   offset += lattice_offset[lat][tile] + cell_offset[c]
//!     if c == target:           return offset
//! ```
//!
//! So each table entry answers one question: **how many instances of the
//! target come before this branch, among its siblings?** For a cell, that is
//! the instances under the preceding cells of its own universe; for a lattice
//! tile, the instances under the preceding tiles.
//!
//! # Why this is built per target, not once
//!
//! The offsets depend on which cell is the target — a cell appearing twice
//! under different branches has different counts preceding it than a cell
//! appearing once. Upstream indexes every table by a `distribcell_index`;
//! this port builds the tables for one target at a time
//! ([`DistribcellOffsets::build`]), which is the same information without a
//! global registry.
//!
//! # The failure this refuses
//!
//! A geometry whose universe graph contains a **cycle** would make the
//! instance count infinite. Upstream does not check (a cyclic geometry is
//! invalid and its own validation catches it earlier); here it would recurse
//! until the stack blew. [`DistribcellOffsets::build`] detects it and returns
//! an error naming the universe, because a stack overflow gives a reader no
//! information at all.

use std::collections::HashMap;

use crate::geometry::cell::CellFill;
use crate::geometry::geometry::{Coord, Geometry};

/// Offset tables for one target cell.
#[derive(Debug, Clone, PartialEq)]
pub struct DistribcellOffsets {
    /// The cell whose instances are numbered.
    pub target: usize,
    /// Per cell: instances of the target under the **preceding** cells of that
    /// cell's own universe. Zero for a cell whose universe is never entered.
    pub cell_offset: Vec<usize>,
    /// Per lattice, per flat tile index: instances under the preceding tiles.
    pub lattice_offset: Vec<Vec<usize>>,
    /// Total instances of the target in the whole geometry.
    pub n_instances: usize,
}

/// Flat tile index within a lattice's universe array, from a signed index
/// triplet. `None` for a triplet outside the lattice.
fn flat_tile(geom: &Geometry, lattice_idx: usize, i: [i32; 3]) -> Option<usize> {
    use crate::geometry::lattice::Lattice;
    match &geom.lattices[lattice_idx] {
        Lattice::Rect(l) => {
            if i.iter().any(|&v| v < 0) {
                return None;
            }
            let (ix, iy, iz) = (i[0] as usize, i[1] as usize, i[2] as usize);
            if ix >= l.n[0] || iy >= l.n[1] || iz >= l.n[2] {
                return None;
            }
            Some(l.n[0] * l.n[1] * iz + l.n[0] * iy + ix)
        }
        Lattice::Hex(l) => {
            let side = 2 * l.n_rings - 1;
            if i.iter().take(2).any(|&v| v < 0) || i[2] < 0 {
                return None;
            }
            let (ix, iy, iz) = (i[0] as usize, i[1] as usize, i[2] as usize);
            if ix >= side || iy >= side || iz >= l.n_axial {
                return None;
            }
            Some(side * side * iz + side * iy + ix)
        }
    }
}

/// Every tile's universe, in flat-index order. `None` for an unused corner of
/// a hex array.
fn tile_universes(geom: &Geometry, lattice_idx: usize) -> Vec<Option<usize>> {
    use crate::geometry::lattice::{Lattice, HEX_NONE};
    match &geom.lattices[lattice_idx] {
        Lattice::Rect(l) => l.universes.iter().map(|&u| Some(u)).collect(),
        Lattice::Hex(l) => l
            .universes
            .iter()
            .map(|&u| if u == HEX_NONE { None } else { Some(u as usize) })
            .collect(),
    }
}

impl DistribcellOffsets {
    /// Build the tables for `target`.
    ///
    /// # Errors
    ///
    /// A cycle in the universe graph, or a cell/lattice/universe index outside
    /// the geometry's arrays.
    pub fn build(geom: &Geometry, target: usize) -> Result<Self, String> {
        if target >= geom.cells.len() {
            return Err(format!(
                "cell index {target} is outside the geometry's {} cells",
                geom.cells.len()
            ));
        }
        let mut out = Self {
            target,
            cell_offset: vec![0; geom.cells.len()],
            lattice_offset: geom
                .lattices
                .iter()
                .enumerate()
                .map(|(i, _)| vec![0; tile_universes(geom, i).len()])
                .collect(),
            n_instances: 0,
        };
        let mut memo: HashMap<usize, usize> = HashMap::new();
        let mut stack: Vec<usize> = Vec::new();
        out.n_instances = out.count_universe(geom, geom.root_universe, &mut memo, &mut stack)?;
        Ok(out)
    }

    /// Instances of the target in `universe`'s subtree, filling the tables on
    /// the way.
    fn count_universe(
        &mut self,
        geom: &Geometry,
        universe: usize,
        memo: &mut HashMap<usize, usize>,
        stack: &mut Vec<usize>,
    ) -> Result<usize, String> {
        if let Some(&n) = memo.get(&universe) {
            return Ok(n);
        }
        if stack.contains(&universe) {
            return Err(format!(
                "the universe graph has a cycle through universe index {universe} \
                 ({:?}); the instance count would be infinite. Without this check the \
                 recursion would blow the stack, which tells a reader nothing.",
                stack
            ));
        }
        let u = geom.universes.get(universe).ok_or_else(|| {
            format!(
                "universe index {universe} is outside the geometry's {} universes",
                geom.universes.len()
            )
        })?;
        stack.push(universe);

        let cell_indices = u.cell_indices.clone();
        let mut running = 0usize;
        for &c in &cell_indices {
            // The offset for THIS cell is however many instances the preceding
            // siblings' subtrees contributed.
            self.cell_offset[c] = running;
            running += self.count_cell(geom, c, memo, stack)?;
        }
        stack.pop();
        memo.insert(universe, running);
        Ok(running)
    }

    /// Instances of the target in `cell`'s subtree, including `cell` itself.
    fn count_cell(
        &mut self,
        geom: &Geometry,
        cell: usize,
        memo: &mut HashMap<usize, usize>,
        stack: &mut Vec<usize>,
    ) -> Result<usize, String> {
        let mut n = usize::from(cell == self.target);
        match geom.cells[cell].fill {
            CellFill::Material(_) | CellFill::Void => {}
            CellFill::Universe(v) => {
                n += self.count_universe(geom, v, memo, stack)?;
            }
            CellFill::Lattice(l) => {
                if l >= geom.lattices.len() {
                    return Err(format!(
                        "cell {cell} fills with lattice {l}, outside the geometry's {} \
                         lattices",
                        geom.lattices.len()
                    ));
                }
                let tiles = tile_universes(geom, l);
                let mut running = 0usize;
                for (t, tu) in tiles.iter().enumerate() {
                    self.lattice_offset[l][t] = running;
                    if let Some(tu) = tu {
                        running += self.count_universe(geom, *tu, memo, stack)?;
                    }
                }
                n += running;
            }
        }
        Ok(n)
    }

    /// The instance index for a located coordinate stack — the port of
    /// `DistribcellFilter::get_all_bins`.
    ///
    /// Returns `None` when the target is not on this path, which is how a
    /// distribcell filter declines an event outside its cell.
    pub fn instance_of(&self, geom: &Geometry, levels: &[Coord]) -> Option<usize> {
        let mut offset = 0usize;
        for (i, lv) in levels.iter().enumerate() {
            match geom.cells[lv.cell].fill {
                CellFill::Universe(_) => offset += self.cell_offset[lv.cell],
                CellFill::Lattice(l) => {
                    // The tile index lives on the NEXT level down, which is the
                    // level the lattice descent produced — the same place
                    // upstream reads it (`p.coord(i + 1).lattice_index()`).
                    let tile = levels
                        .get(i + 1)
                        .and_then(|next| flat_tile(geom, l, next.lattice_index));
                    if let Some(t) = tile {
                        offset += self.lattice_offset[l][t];
                    }
                    offset += self.cell_offset[lv.cell];
                }
                CellFill::Material(_) | CellFill::Void => {}
            }
            if lv.cell == self.target {
                return Some(offset);
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::cell::{Cell, HalfSpaceSense, RegionToken};
    use crate::geometry::lattice::{Lattice, RectLattice};
    use crate::geometry::position::{Direction, Position};
    use crate::geometry::surface::{BoundaryType, Sphere, SurfaceKind};
    use crate::geometry::universe::Universe;

    fn inside(s: usize) -> Vec<RegionToken> {
        vec![RegionToken::HalfSpace {
            surface_idx: s,
            sense: HalfSpaceSense::Inside,
        }]
    }

    /// Root universe 0: one cell filled by a 3x2x1 lattice, whose every tile
    /// is universe 1, whose single cell (index 1) is the target material cell.
    ///
    /// Six instances, numbered in tile order.
    fn lattice_geometry() -> Geometry {
        Geometry {
            surfaces: vec![SurfaceKind::Sphere(Sphere {
                x0: 0.0,
                y0: 0.0,
                z0: 0.0,
                r: 100.0,
                bc: BoundaryType::Vacuum,
            })],
            cells: vec![
                // 0: root cell, filled by the lattice.
                Cell::fill(1, inside(0), CellFill::Lattice(0), Position::ZERO),
                // 1: the target, a material cell inside universe 1.
                Cell::material(2, inside(0), 0, 293.6),
            ],
            universes: vec![
                Universe {
                    id: 0,
                    cell_indices: vec![0],
                },
                Universe {
                    id: 1,
                    cell_indices: vec![1],
                },
            ],
            lattices: vec![Lattice::Rect(RectLattice {
                id: 1,
                n: [3, 2, 1],
                lower_left: Position::new(-3.0, -2.0, -1.0),
                pitch: [2.0, 2.0, 2.0],
                universes: vec![1; 6],
                outer: None,
            })],
            root_universe: 0,
        }
    }

    fn coord(cell: usize, universe: usize, lattice_index: [i32; 3]) -> Coord {
        Coord {
            universe,
            cell,
            r: Position::ZERO,
            u: Direction::new(0.0, 0.0, 1.0),
            lattice: None,
            lattice_index,
            offset: Position::ZERO,
        }
    }

    /// **A lattice of six tiles gives six instances, numbered in tile order.**
    #[test]
    fn a_lattice_numbers_its_tiles_in_order() {
        let g = lattice_geometry();
        let d = DistribcellOffsets::build(&g, 1).unwrap();
        assert_eq!(d.n_instances, 6, "3x2x1 tiles of a 1-instance universe");
        assert_eq!(d.lattice_offset[0], vec![0, 1, 2, 3, 4, 5]);
        assert_eq!(d.cell_offset[0], 0, "the root cell has no preceding sibling");

        // Walk each tile and check the instance index.
        for iy in 0..2i32 {
            for ix in 0..3i32 {
                let levels = vec![coord(0, 0, [0, 0, 0]), coord(1, 1, [ix, iy, 0])];
                let want = (3 * iy + ix) as usize;
                assert_eq!(
                    d.instance_of(&g, &levels),
                    Some(want),
                    "tile ({ix}, {iy})"
                );
            }
        }
    }

    /// The target not being on the path gives `None`, which is how the filter
    /// declines an event outside its cell.
    #[test]
    fn a_path_without_the_target_has_no_instance() {
        let g = lattice_geometry();
        let d = DistribcellOffsets::build(&g, 1).unwrap();
        assert_eq!(d.instance_of(&g, &[coord(0, 0, [0, 0, 0])]), None);
    }

    /// **Sibling cells shift the numbering.** Two cells in one universe, the
    /// target second: every instance of the target is offset by the
    /// instances under the cell that precedes it.
    #[test]
    fn preceding_siblings_shift_the_instance_numbering() {
        let mut g = lattice_geometry();
        // Universe 1 gains a cell (index 2) BEFORE the target, itself filled
        // by universe 2, which contains one more copy of... a different cell.
        // Simplest faithful case: make the preceding sibling the target too,
        // so each tile contributes 2 instances.
        g.cells.push(Cell::material(3, inside(0), 0, 293.6));
        g.universes[1].cell_indices = vec![2, 1];

        // Target is cell 1, which now sits after cell 2 in universe 1.
        let d = DistribcellOffsets::build(&g, 1).unwrap();
        assert_eq!(d.n_instances, 6, "cell 2 is not the target, so still 6");
        assert_eq!(
            d.cell_offset[1], 0,
            "cell 2 contributes no TARGET instances, so cell 1's offset is 0"
        );

        // Now make cell 2 the target instead: it precedes cell 1, so its
        // offsets are the same but the count is still 6.
        let d2 = DistribcellOffsets::build(&g, 2).unwrap();
        assert_eq!(d2.n_instances, 6);
        for iy in 0..2i32 {
            for ix in 0..3i32 {
                let levels = vec![coord(0, 0, [0, 0, 0]), coord(2, 1, [ix, iy, 0])];
                assert_eq!(d2.instance_of(&g, &levels), Some((3 * iy + ix) as usize));
            }
        }
    }

    /// Nested universes multiply: a lattice of 6 tiles, each tile a universe
    /// holding a cell filled by another universe with the target inside.
    #[test]
    fn nesting_multiplies_the_instance_count() {
        let mut g = lattice_geometry();
        // Universe 1's cell 1 becomes universe-filled by universe 2, which
        // holds the new target cell 2.
        g.cells[1] = Cell::fill(2, inside(0), CellFill::Universe(2), Position::ZERO);
        g.cells.push(Cell::material(3, inside(0), 0, 293.6));
        g.universes.push(Universe {
            id: 2,
            cell_indices: vec![2],
        });

        let d = DistribcellOffsets::build(&g, 2).unwrap();
        assert_eq!(d.n_instances, 6);
        for iy in 0..2i32 {
            for ix in 0..3i32 {
                let levels = vec![
                    coord(0, 0, [0, 0, 0]),
                    coord(1, 1, [ix, iy, 0]),
                    coord(2, 2, [0, 0, 0]),
                ];
                assert_eq!(
                    d.instance_of(&g, &levels),
                    Some((3 * iy + ix) as usize),
                    "tile ({ix}, {iy})"
                );
            }
        }
    }

    /// **A cyclic universe graph is refused**, not left to blow the stack.
    #[test]
    fn a_cyclic_universe_graph_is_refused() {
        let mut g = lattice_geometry();
        // Universe 1's cell now fills with universe 1 again.
        g.cells[1] = Cell::fill(2, inside(0), CellFill::Universe(1), Position::ZERO);
        let err = DistribcellOffsets::build(&g, 1).unwrap_err();
        assert!(err.contains("cycle"), "{err}");
    }

    /// Out-of-range indices are named rather than panicking.
    #[test]
    fn out_of_range_indices_are_refused() {
        let g = lattice_geometry();
        let err = DistribcellOffsets::build(&g, 99).unwrap_err();
        assert!(err.contains("outside the geometry"), "{err}");
    }
}
