/// Universe hierarchy — the nesting mechanism for CSG geometry.
///
/// C++ source: `src/universe.cpp` (217 LOC), `include/openmc/universe.h`.
///
/// A `Universe` is a collection of `Cell`s. Particle tracking starts in the root
/// universe and recursively descends into fill universes/lattices to locate the
/// leaf cell a particle inhabits (see [`crate::geometry::geometry::Geometry`]).
///
/// Key operation: [`Universe::find_cell`] — given a local position, return the
/// first cell in this universe that contains it.

use super::cell::Cell;
use super::position::Position;
use super::surface::SurfaceKind;

/// A universe — an ordered list of cells searched top-to-bottom.
/// Maps to `openmc::Universe`.
pub struct Universe {
    /// User-facing universe id.
    pub id: i32,
    /// Indices into the global cell array, in search order.
    pub cell_indices: Vec<usize>,
}

impl Universe {
    /// Find the first cell in this universe that contains `r` (in this universe's
    /// local frame).
    ///
    /// Ported from `Universe::find_cell` (`src/universe.cpp:40`): iterate the
    /// universe's cells in order and return the first whose region contains the
    /// point. Returns the **global cell index**, or `None` if the point is in no
    /// cell of this universe (a geometry "lost particle").
    pub fn find_cell(&self, r: Position, surfaces: &[SurfaceKind], cells: &[Cell]) -> Option<usize> {
        for &i_cell in &self.cell_indices {
            if cells[i_cell].contains(r, surfaces) {
                return Some(i_cell);
            }
        }
        None
    }
}
