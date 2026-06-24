/// Universe hierarchy — the nesting mechanism for CSG geometry.
///
/// C++ source: `src/universe.cpp` (217 LOC), `include/openmc/universe.h`.
///
/// A `Universe` is a collection of `Cell`s.  OpenMC starts particle tracking
/// in the root universe (index 0) and recursively descends into fill universes
/// to locate which cell a particle inhabits.
///
/// Key operation: `find_cell(r, u)` — given a position and direction, returns
/// the leaf cell containing `r` and the coordinate transform to that cell's
/// local frame.

use super::position::{Direction, Position};

/// A universe — an ordered list of cells searched top-to-bottom.
/// Maps to `openmc::Universe`.
pub struct Universe {
    pub id: i32,
    /// Indices into the global cell array, in search order.
    pub cell_indices: Vec<usize>,
}

impl Universe {
    /// Find the cell in this universe that contains `r`.
    ///
    /// Returns the cell index, or `None` if `r` is not in any cell.
    /// TODO: port from `universe.cpp::find_cell()`.
    pub fn find_cell(&self, r: Position, u: Direction, _surfaces: &[Box<dyn super::surface::Surface>], _cells: &[super::cell::Cell]) -> Option<usize> {
        let _ = (r, u);
        todo!("Universe::find_cell: port from src/universe.cpp")
    }
}
