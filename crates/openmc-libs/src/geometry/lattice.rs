/// Rectangular and hexagonal lattices.
///
/// C++ source: `src/lattice.cpp` (1219 LOC), `include/openmc/lattice.h`.
///
/// A lattice tiles the space with identical universes arranged in a periodic
/// grid.  OpenMC supports two types:
///   - `RectLattice` — 3D rectangular grid (nx × ny × nz pitches)
///   - `HexLattice`  — 2D hexagonal grid (axial rings + axial levels)
///
/// Each lattice element maps to a universe index.  The lattice is itself
/// treated as a special kind of universe fill.

use super::position::Position;

/// Lattice type tag.  Maps to `openmc::LatticeType`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LatticeType { Rect, Hex }

/// A rectangular lattice.  Maps to `openmc::RectLattice`.
pub struct RectLattice {
    pub id: i32,
    /// Number of grid cells in x, y, z.
    pub n: [usize; 3],
    /// Lower-left corner of the lattice in cm.
    pub lower_left: Position,
    /// Pitch (cell width) in cm for each axis.
    pub pitch: [f64; 3],
    /// Universe index for each lattice element, row-major `[z][y][x]`.
    pub universes: Vec<usize>,
}

impl RectLattice {
    /// Map a position to a lattice index triplet `[ix, iy, iz]`.
    /// Returns `None` if `r` is outside the lattice extent.
    /// TODO: port from `lattice.cpp::get_indices()`.
    pub fn get_indices(&self, r: Position) -> Option<[usize; 3]> {
        let _ = r;
        todo!("RectLattice::get_indices: port from src/lattice.cpp")
    }
}

/// A hexagonal lattice.  Maps to `openmc::HexLattice`.
pub struct HexLattice {
    pub id: i32,
    pub n_rings: usize,
    pub n_axial: usize,
    pub center: Position,
    pub pitch: [f64; 2],
    pub universes: Vec<usize>,
}
