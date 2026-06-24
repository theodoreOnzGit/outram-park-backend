/// CSG cells — regions bounded by surface half-spaces.
///
/// C++ source: `src/cell.cpp` (1861 LOC), `include/openmc/cell.h` (493 LOC).
///
/// A `Cell` is defined by a Boolean combination of surface half-spaces encoded
/// as a Reverse Polish Notation (RPN) token stream.  Tokens are:
///   - Positive integer n  → inside surface n (half-space −)
///   - Negative integer n  → outside surface n (half-space +)
///   - INTERSECTION (−1)  → logical AND
///   - UNION        (−2)  → logical OR
///   - COMPLEMENT   (−3)  → logical NOT
///
/// A cell may be a **material cell** (filled with a `Material`) or a
/// **fill cell** (filled with a nested `Universe` or `Lattice`).

use super::position::Position;
use super::surface::Surface;

/// Token in the RPN region definition.  Maps to OpenMC's region token encoding.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegionToken {
    /// Half-space: surface index, positive = outside, negative = inside.
    HalfSpace { surface_idx: usize, sense: HalfSpaceSense },
    Intersection,
    Union,
    Complement,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HalfSpaceSense { Inside, Outside }

/// What fills a cell.  Maps to OpenMC's `Cell::type_`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CellFill {
    /// Filled with a material (index into the materials list).
    Material(usize),
    /// Filled with a nested universe (index).
    Universe(usize),
    /// Filled with a lattice (index).
    Lattice(usize),
    Void,
}

/// A CSG cell.  Maps to `openmc::Cell`.
pub struct Cell {
    pub id: i32,
    pub region: Vec<RegionToken>,
    pub fill: CellFill,
    /// Temperature of this cell in eV (1 eV ≈ 11604 K).
    pub temperature: f64,
}

impl Cell {
    /// Evaluate the region definition at position `r` using the provided surfaces.
    ///
    /// Returns `true` if `r` is inside this cell.
    /// TODO: port the RPN stack evaluator from `cell.cpp::contains()`.
    pub fn contains(&self, r: Position, surfaces: &[Box<dyn Surface>]) -> bool {
        let _ = (r, surfaces);
        todo!("Cell::contains: port RPN evaluator from src/cell.cpp")
    }

    /// Distance to the nearest surface bounding this cell along ray `(r, u)`.
    ///
    /// Returns `(distance, surface_idx)`.
    /// TODO: port from `cell.cpp::distance()`.
    pub fn distance_to_boundary(
        &self,
        r: super::position::Position,
        u: super::position::Direction,
        surfaces: &[Box<dyn Surface>],
    ) -> (f64, usize) {
        let _ = (r, u, surfaces);
        todo!("Cell::distance_to_boundary: port from src/cell.cpp")
    }
}
