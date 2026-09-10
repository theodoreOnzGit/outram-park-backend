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

/// **Which surface a particle is sitting on, and which side of it it is on.**
///
/// This is OpenMC's *signed surface token* (`Particle::surface_`,
/// `include/openmc/particle_data.h:437`; `SURFACE_NONE == 0`), expressed as a
/// named type instead of a signed integer with a magic zero.
///
/// # Why a particle needs to carry this
///
/// Immediately after a boundary crossing the particle sits **exactly on** a
/// surface, where `Surface::evaluate(r)` is ~0 and its sign is decided by
/// round-off rather than by geometry. A membership test that re-evaluates that
/// sign can therefore put the particle back in the cell it was *leaving*; the
/// next `distance_to_boundary` then finds no forward surface (the one it is on
/// is suppressed as coincident), the particle streams to infinity and the
/// history leaks. That is GitHub #168 — 85 % of source neutrons lost on a
/// concentric-shell pebble (measured 2026-09-10: leakage 0.846 per source
/// neutron, k_eff 0.236 where ~1.30 was expected).
///
/// Carrying the crossed surface **plus the side it ended up on** removes the
/// ambiguity entirely: [`Cell::contains`] takes the recorded sense as fact for
/// that one surface instead of re-deriving it. Mirrors `Region::contains_simple`
/// / `contains_complex` (`src/cell.cpp:1046`, `:1069`), where a region token
/// equal to `on_surface` is satisfied outright and its negation fails outright.
///
/// [`SurfaceToken::NONE`] means "not on any surface" — the state after a
/// collision, at birth, and for any standalone geometry query.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SurfaceToken {
    /// The particle is not sitting on any surface.
    None,
    /// The particle is on surface `surface_idx`, on the `sense` side of it.
    On {
        /// Index into the global surface array.
        surface_idx: usize,
        /// Which side of that surface the particle is on.
        sense: HalfSpaceSense,
    },
}

impl SurfaceToken {
    /// Not on any surface — OpenMC's `SURFACE_NONE`.
    pub const NONE: SurfaceToken = SurfaceToken::None;

    /// On surface `surface_idx`, on the `sense` side.
    #[inline]
    pub fn on(surface_idx: usize, sense: HalfSpaceSense) -> Self {
        Self::On { surface_idx, sense }
    }

    /// Whether this token names surface `surface_idx` (either sense) — the
    /// `coincident` test used when computing distances.
    #[inline]
    pub fn is_on(self, surface_idx: usize) -> bool {
        matches!(self, Self::On { surface_idx: i, .. } if i == surface_idx)
    }
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

    /// Whether a particle at `r` heading along `u` lies inside this cell's region.
    ///
    /// Evaluates the RPN token stream over a boolean stack (mirrors the semantics
    /// of `Region::contains` in `src/cell.cpp:1035`, generalised to explicit RPN so
    /// intersection, union and complement are all handled by one evaluator). A
    /// half-space pushes `sense_matches`; `Intersection`/`Union` pop two and push
    /// their AND/OR; `Complement` negates the top.
    ///
    /// # The `on_surface` override (why `u` and `on_surface` are arguments)
    ///
    /// For the one surface named by `on_surface` the recorded side is taken as
    /// **fact** and the surface equation is not evaluated at all: a half-space
    /// asking for that side is satisfied, one asking for the other side fails.
    /// Ported from `Region::contains_simple` (`src/cell.cpp:1046`), where
    /// `token == on_surface` and `-token == on_surface` short-circuit the sense
    /// test. Without it, a particle sitting exactly on a surface it has just
    /// crossed can be re-located in the cell it was leaving (see
    /// [`SurfaceToken`]).
    ///
    /// Every *other* surface is judged by [`SurfaceKind::sense`], which falls
    /// back to the direction of travel relative to the outward normal when the
    /// point is within `FP_COINCIDENT` of the surface — hence `u`.
    ///
    /// `surfaces` is the global surface array the tokens index into. A malformed
    /// (stack-underflowing) region conservatively returns `false`. Pass
    /// [`SurfaceToken::NONE`] for a standalone point query.
    pub fn contains(
        &self,
        r: Position,
        u: Direction,
        surfaces: &[SurfaceKind],
        on_surface: SurfaceToken,
    ) -> bool {
        let mut stack: Vec<bool> = Vec::with_capacity(self.region.len());
        for tok in &self.region {
            match tok {
                RegionToken::HalfSpace { surface_idx, sense } => {
                    let inside_token = match on_surface {
                        // The particle is on this very surface: its side was
                        // recorded at the crossing, so trust it rather than
                        // re-deriving a sign that round-off owns.
                        SurfaceToken::On {
                            surface_idx: i,
                            sense: on_sense,
                        } if i == *surface_idx => on_sense == *sense,
                        // `sense()` is true on the positive (outside) half-space.
                        _ => {
                            let positive = surfaces[*surface_idx].sense(r, u);
                            match sense {
                                HalfSpaceSense::Outside => positive,
                                HalfSpaceSense::Inside => !positive,
                            }
                        }
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
    /// Ported from `Region::distance` (`src/cell.cpp:950`): take the minimum
    /// `distance` over every half-space surface in the region (operators are
    /// skipped). `on_surface` names the surface the particle currently sits on
    /// (see [`SurfaceToken`]) — that surface is queried with the `coincident`
    /// flag so round-off cannot re-report a zero crossing.
    ///
    /// Returns `(distance, surface_idx)`; `surface_idx == usize::MAX` when no
    /// bounding surface is crossed (distance `INFINITY`).
    pub fn distance_to_boundary(
        &self,
        r: Position,
        u: Direction,
        surfaces: &[SurfaceKind],
        on_surface: SurfaceToken,
    ) -> (f64, usize) {
        let mut min_dist = f64::INFINITY;
        let mut i_surf = usize::MAX;
        for tok in &self.region {
            let RegionToken::HalfSpace { surface_idx, .. } = tok else {
                continue;
            };
            let coincident = on_surface.is_on(*surface_idx);
            let d = surfaces[*surface_idx].distance(r, u, coincident);
            if d < min_dist {
                min_dist = d;
                i_surf = *surface_idx;
            }
        }
        (min_dist, i_surf)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::surface::{BoundaryType, Sphere};

    /// **GitHub #168 regression, at the level it is decided.**
    ///
    /// A particle sitting exactly on a shell's outer surface, heading out, must
    /// not be found in that shell — regardless of which way round-off happens to
    /// push `Surface::evaluate`. Both a point a hair *inside* the surface and a
    /// point a hair *outside* it are tested with the same outgoing
    /// [`SurfaceToken`]: the token decides, so both answer the same way.
    ///
    /// Geometry: spheres of radius 1 and 2; the shell is `outside(0) & inside(1)`.
    #[test]
    fn on_surface_token_overrides_a_roundoff_sense() {
        let sphere = |r: f64| {
            SurfaceKind::Sphere(Sphere {
                x0: 0.0,
                y0: 0.0,
                z0: 0.0,
                r,
                bc: BoundaryType::Transmissive,
            })
        };
        let surfaces = vec![sphere(1.0), sphere(2.0)];
        let shell = Cell::material(
            1,
            vec![
                RegionToken::HalfSpace {
                    surface_idx: 0,
                    sense: HalfSpaceSense::Outside,
                },
                RegionToken::HalfSpace {
                    surface_idx: 1,
                    sense: HalfSpaceSense::Inside,
                },
                RegionToken::Intersection,
            ],
            0,
            293.6,
        );
        let outward = Direction::new(1.0, 0.0, 0.0);
        // Having crossed surface 1 outward, the particle is on its Outside.
        let leaving = SurfaceToken::on(1, HalfSpaceSense::Outside);

        for eps in [-1.0e-12, 0.0, 1.0e-12] {
            let r = Position::new(2.0 + eps, 0.0, 0.0);
            assert!(
                !shell.contains(r, outward, &surfaces, leaving),
                "a particle recorded on the OUTSIDE of surface 1 must never be                  found back in the shell (eps={eps}) — GH #168 regressed"
            );
            // The opposite token puts it back in, at the same coordinates.
            assert!(
                shell.contains(
                    r,
                    outward,
                    &surfaces,
                    SurfaceToken::on(1, HalfSpaceSense::Inside)
                ),
                "the Inside token must select the shell (eps={eps})"
            );
        }

        // `is_on` names the surface regardless of side — it is the coincident
        // test `distance_to_boundary` uses.
        assert!(leaving.is_on(1));
        assert!(!leaving.is_on(0));
        assert!(!SurfaceToken::NONE.is_on(1));
    }

    /// With no token, a point safely away from every surface still resolves
    /// normally — the override must not change ordinary membership.
    #[test]
    fn membership_away_from_surfaces_is_unaffected() {
        let surfaces = vec![SurfaceKind::Sphere(Sphere {
            x0: 0.0,
            y0: 0.0,
            z0: 0.0,
            r: 1.0,
            bc: BoundaryType::Transmissive,
        })];
        let ball = Cell::material(
            1,
            vec![RegionToken::HalfSpace {
                surface_idx: 0,
                sense: HalfSpaceSense::Inside,
            }],
            0,
            293.6,
        );
        let u = Direction::new(1.0, 0.0, 0.0);
        assert!(ball.contains(
            Position::new(0.5, 0.0, 0.0),
            u,
            &surfaces,
            SurfaceToken::NONE
        ));
        assert!(!ball.contains(
            Position::new(1.5, 0.0, 0.0),
            u,
            &surfaces,
            SurfaceToken::NONE
        ));
    }
}
