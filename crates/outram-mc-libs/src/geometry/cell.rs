/// CSG cells — regions bounded by surface half-spaces.
///
/// C++ source: `src/cell.cpp` (1861 LOC), `include/openmc/cell.h` (493 LOC).
///
/// A `Cell` is defined by a Boolean combination of surface half-spaces encoded
/// as a **Reverse Polish Notation (RPN)** token stream ([`RegionToken`]):
/// half-space operands are pushed, and the `Intersection` / `Union` /
/// `Complement` operators pop and combine them. A pure intersection cell — the
/// common case (fuel pin, moderator box) — is written as
/// `[HalfSpace, HalfSpace, Intersection, HalfSpace, Intersection, …]`.
///
/// A cell may be a **material cell** (filled with a `Material`) or a **fill
/// cell** (filled with a nested `Universe` or `Lattice`).
use super::position::{Direction, Position};
use super::surface::SurfaceKind;

/// Which side of a surface a half-space token selects.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HalfSpaceSense {
    /// Negative side, `evaluate(r) < 0` — the interior of a sphere/cylinder.
    Inside,
    /// Positive side, `evaluate(r) > 0` — the exterior.
    Outside,
}

/// One token in the RPN region definition. Maps to OpenMC's region token stream
/// (`src/cell.cpp`), but with the operators named rather than encoded as the
/// sentinel negative integers OpenMC uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegionToken {
    /// Half-space of surface `surface_idx` (index into the global surface array).
    HalfSpace {
        surface_idx: usize,
        sense: HalfSpaceSense,
    },
    /// Logical AND of the two operands below it on the stack.
    Intersection,
    /// Logical OR of the two operands below it on the stack.
    Union,
    /// Logical NOT of the single operand below it on the stack.
    Complement,
}

/// What fills a cell. Maps to OpenMC's `Cell::type_` / `Fill`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CellFill {
    /// Filled with a material (index into the materials list).
    Material(usize),
    /// Filled with a nested universe (index into the universe array).
    Universe(usize),
    /// Filled with a lattice (index into the lattice array).
    Lattice(usize),
    /// Void — no material, streams freely.
    Void,
}

#[derive(Debug, Clone)]
/// A CSG cell. Maps to `openmc::Cell`.
pub struct Cell {
    /// User-facing cell id (for reporting/tallies).
    pub id: i32,
    /// Region definition as an RPN token stream (see [`RegionToken`]).
    pub region: Vec<RegionToken>,
    /// What the cell is filled with.
    pub fill: CellFill,
    /// Temperature of this cell in Kelvin (passed to the Doppler XS lookup).
    pub temperature: f64,
    /// Rigid translation \[cm\] applied to a fill universe's local frame
    /// (`coord.r -= translation`). Zero for material cells and untranslated fills.
    /// Mirrors `Cell::translation_` in `src/cell.cpp`.
    pub translation: Position,
}

impl Cell {
    /// Build a material cell with no translation — the common leaf case.
    pub fn material(
        id: i32,
        region: Vec<RegionToken>,
        material_idx: usize,
        temperature: f64,
    ) -> Self {
        Self {
            id,
            region,
            fill: CellFill::Material(material_idx),
            temperature,
            translation: Position::ZERO,
        }
    }

    /// Build a fill cell (nested universe or lattice) with an optional translation.
    pub fn fill(id: i32, region: Vec<RegionToken>, fill: CellFill, translation: Position) -> Self {
        Self {
            id,
            region,
            fill,
            temperature: 293.6,
            translation,
        }
    }

    /// Whether position `r` lies inside this cell's region.
    ///
    /// Evaluates the RPN token stream over a boolean stack (mirrors the semantics
    /// of `Region::contains` in `src/cell.cpp`, generalised to explicit RPN so
    /// intersection, union and complement are all handled by one evaluator). A
    /// half-space pushes `sense_matches`; `Intersection`/`Union` pop two and push
    /// their AND/OR; `Complement` negates the top.
    ///
    /// `surfaces` is the global surface array the tokens index into. A malformed
    /// (stack-underflowing) region conservatively returns `false`.
    pub fn contains(&self, r: Position, surfaces: &[SurfaceKind]) -> bool {
        let mut stack: Vec<bool> = Vec::with_capacity(self.region.len());
        for tok in &self.region {
            match tok {
                RegionToken::HalfSpace { surface_idx, sense } => {
                    // `sense()` is true on the positive (outside) half-space.
                    let positive = surfaces[*surface_idx].sense(r);
                    let inside_token = match sense {
                        HalfSpaceSense::Outside => positive,
                        HalfSpaceSense::Inside => !positive,
                    };
                    stack.push(inside_token);
                }
                RegionToken::Intersection => {
                    let (Some(b), Some(a)) = (stack.pop(), stack.pop()) else {
                        return false;
                    };
                    stack.push(a && b);
                }
                RegionToken::Union => {
                    let (Some(b), Some(a)) = (stack.pop(), stack.pop()) else {
                        return false;
                    };
                    stack.push(a || b);
                }
                RegionToken::Complement => {
                    let Some(a) = stack.pop() else { return false };
                    stack.push(!a);
                }
            }
        }
        stack.pop().unwrap_or(false)
    }

    /// Distance along ray `(r, u)` to the nearest surface bounding this cell.
    ///
    /// Ported from `Region::distance` (`src/cell.cpp:947`): take the minimum
    /// `distance` over every half-space surface in the region (operators are
    /// skipped). `on_surface` is the global surface index the particle currently
    /// sits on (`usize::MAX` if none) — that surface is queried with the
    /// `coincident` flag so round-off cannot re-report a zero crossing.
    ///
    /// Returns `(distance, surface_idx)`; `surface_idx == usize::MAX` when no
    /// bounding surface is crossed (distance `INFINITY`).
    pub fn distance_to_boundary(
        &self,
        r: Position,
        u: Direction,
        surfaces: &[SurfaceKind],
        on_surface: usize,
    ) -> (f64, usize) {
        let mut min_dist = f64::INFINITY;
        let mut i_surf = usize::MAX;
        for tok in &self.region {
            let RegionToken::HalfSpace { surface_idx, .. } = tok else {
                continue;
            };
            let coincident = *surface_idx == on_surface;
            let d = surfaces[*surface_idx].distance(r, u, coincident);
            if d < min_dist {
                min_dist = d;
                i_surf = *surface_idx;
            }
        }
        (min_dist, i_surf)
    }
}
