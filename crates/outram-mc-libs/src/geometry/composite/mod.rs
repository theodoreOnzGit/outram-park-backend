// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Ported from OpenMC (https://github.com/openmc-dev/openmc, MIT licence,
// Copyright (c) 2011-2026 Massachusetts Institute of Technology, UChicago
// Argonne LLC and OpenMC contributors; see LICENSE.openmc):
//   openmc/model/surface_composite.py at commit 8f79d9d0 (2026-11-10)
//   openmc/surface.py (Plane.from_points, flip_normal, rotate, translate,
//   Cone/Cylinder._get_base_coeffs) at the same commit.
//
// This file is part of OUTRAM PARK. AGPL-3.0-only; see LICENSE.

//! **Composite surfaces**: OpenMC's `openmc.model` shapes that expand into
//! ordinary CSG half-spaces (GitHub #1389).
//!
//! Upstream is `openmc/model/surface_composite.py` at OpenMC `8f79d9d0`. Each
//! composite there builds a handful of primitive surfaces in `__init__` and
//! returns their combination from `__neg__` (inside) and `__pos__` (outside,
//! `~(-self)` unless overridden). This module does the same with this crate's
//! own types: a composite is built once, holds its primitive
//! [`SurfaceKind`]s, and gives its two half-spaces as RPN
//! [`RegionToken`] streams indexing those surfaces ([`CompositeRegion`]).
//! Nothing new is added to transport: the surfaces are the existing ones
//! (`outram_blender::csg::surface`), the regions are evaluated by the existing
//! [`Cell::contains`] and bounded by the existing
//! `Cell::distance_to_boundary` (which handles unions and complements,
//! `src/cell.cpp:950`).
//!
//! | composite | upstream class (`surface_composite.py`) | here |
//! |---|---|---|
//! | cylinder sector | `CylinderSector`, lines 71-237 | [`CylinderSector`] |
//! | isogonal octagon | `IsogonalOctagon`, lines 240-395 | [`IsogonalOctagon`] |
//! | one-sided cones | `X/Y/ZConeOneSided`, lines 737-897 | [`ConeOneSided`] |
//! | polygon | `Polygon`, lines 900-1369 | [`Polygon`] |
//! | cruciform prism | `CruciformPrism`, lines 1372-1454 | [`CruciformPrism`] |
//! | rectangular prism (rounded corners) | `RectangularPrism`, lines 1457-1585 | [`RectangularPrism`] |
//! | conical frustum | `ConicalFrustum`, lines 1752-1869 | [`ConicalFrustum`] |
//! | vessel | `Vessel`, lines 1872-1962 | [`Vessel`] |
//!
//! `RightCircularCylinder`, `RectangularParallelepiped`, `OrthogonalBox` and
//! `HexagonalPrism` are not here: the beginner model API covers them
//! (`model::shape`, `model::hex`; issue #1389's workspace check).
//!
//! # Units
//!
//! Raw `f64` **centimetres** and **degrees** (for angles), as upstream and as
//! the rest of the CSG layer (`crates/outram-mc-libs/CLAUDE.md`, "Units: raw
//! `f64`, not `uom`"): composites produce CSG surfaces, which take raw `f64`.
//!
//! # What is not carried over
//!
//! - **Translation and rotation of a built composite** (`CompositeSurface.
//!   translate/rotate`): not ported. `ConicalFrustum` uses upstream's
//!   rotation internally, as upstream does.
//! - **Periodic boundaries.** [`CompositeRegion::with_boundary`] refuses
//!   [`BoundaryType::Periodic`]: upstream pairs periodic planes by object
//!   reference (`RectangularPrism`), which the CSG surface types here do not
//!   carry.
//! - **`Polygon.offset`**: not ported.
//!
//! # Verification
//!
//! `tests/composite_surfaces_vs_openmc.rs`: point membership and surface
//! coefficients against OpenMC's own Python, stochastic volume against the
//! analytic volume, and slices drawn with the ported plotter
//! (`verification_and_validation/composite_surfaces/`).

mod cones;
mod polygon;
mod prisms;
#[cfg(test)]
mod tests;

pub use cones::{ConeOneSided, ConicalFrustum, Vessel};
pub use polygon::{Polygon, PolygonBasis};
pub use prisms::{CruciformPrism, CylinderSector, IsogonalOctagon, RectangularPrism};

use crate::geometry::cell::{Cell, HalfSpaceSense, RegionToken, SurfaceToken};
use crate::geometry::geometry::Geometry;
use crate::geometry::position::{Direction, Position};
use crate::geometry::surface::{BoundaryType, Plane, SurfaceKind};
use crate::geometry::universe::Universe;

/// The axis a composite is parallel to (upstream's `axis='x'|'y'|'z'`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Axis {
    X,
    Y,
    Z,
}

/// Why a composite could not be built. Each message is upstream's
/// `ValueError` text (or says which upstream check it stands for).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompositeError {
    /// The upstream class whose check failed, e.g. `"CylinderSector"`.
    pub composite: &'static str,
    /// What was wrong.
    pub reason: String,
}

impl CompositeError {
    fn new(composite: &'static str, reason: impl Into<String>) -> Self {
        Self {
            composite,
            reason: reason.into(),
        }
    }
}

impl std::fmt::Display for CompositeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.composite, self.reason)
    }
}

impl std::error::Error for CompositeError {}

/// **A composite surface expanded into primitive surfaces and two regions.**
///
/// `surfaces` are the composite's own primitive surfaces, in upstream's
/// `_surface_names` order where upstream has one. `negative` (inside, upstream
/// `-composite`) and `positive` (outside, `+composite`) are RPN token streams
/// whose `surface_idx` index **`surfaces`**, not a global array; use
/// [`Self::append_to`] to place them in a model.
#[derive(Debug, Clone)]
pub struct CompositeRegion {
    /// The primitive surfaces.
    pub surfaces: Vec<SurfaceKind>,
    /// Inside, `-composite`, as RPN over `surfaces`.
    pub negative: Vec<RegionToken>,
    /// Outside, `+composite`, as RPN over `surfaces`.
    pub positive: Vec<RegionToken>,
    /// Which surfaces take a boundary condition from
    /// [`Self::with_boundary`]. Upstream's `boundary_type` setter skips the
    /// disambiguation planes of the one-sided cones and the vessel
    /// (`CompositeSurface.boundary_type`, lines 46-53) and a polygon's internal
    /// facets (only `on_boundary` facets are in its `_surface_names`).
    takes_boundary: Vec<bool>,
}

impl CompositeRegion {
    /// Copy the surfaces onto the end of `global` and return
    /// `(negative, positive)` re-indexed into it.
    pub fn append_to(&self, global: &mut Vec<SurfaceKind>) -> (Vec<RegionToken>, Vec<RegionToken>) {
        let offset = global.len();
        global.extend(self.surfaces.iter().cloned());
        (
            shift_tokens(&self.negative, offset),
            shift_tokens(&self.positive, offset),
        )
    }

    /// Whether `r` \[cm\] is in the `side` half-space of the composite
    /// (`Inside` = `-composite`). A standalone point query through the
    /// transport's own [`Cell::contains`], with the direction fixed at
    /// `(1,1,1)/sqrt(3)` (as `volume_calc`) to break a tie on a surface.
    pub fn contains(&self, r: Position, side: HalfSpaceSense) -> bool {
        let tokens = match side {
            HalfSpaceSense::Inside => &self.negative,
            HalfSpaceSense::Outside => &self.positive,
        };
        let cell = Cell::material(0, tokens.clone(), 0, 293.6);
        let s = 1.0 / 3.0_f64.sqrt();
        cell.contains(
            r,
            Direction::new(s, s, s),
            &self.surfaces,
            SurfaceToken::NONE,
        )
    }

    /// Set `bc` on every surface that takes the composite's boundary
    /// condition (upstream `CompositeSurface.boundary_type` setter, lines
    /// 46-53). [`BoundaryType::Periodic`] is refused: upstream pairs periodic
    /// planes by reference, which these surface types do not carry.
    pub fn with_boundary(mut self, bc: BoundaryType) -> Result<Self, CompositeError> {
        // `PeriodicWith(j)` names ONE partner; stamping it on every surface of
        // the composite would pair them all with `j`. Refused too (2026-11-14,
        // GitHub #259).
        if matches!(bc, BoundaryType::Periodic | BoundaryType::PeriodicWith(_)) {
            return Err(CompositeError::new(
                "CompositeSurface",
                "periodic boundaries are not supported on composite surfaces here",
            ));
        }
        for (s, takes) in self.surfaces.iter_mut().zip(&self.takes_boundary) {
            if *takes {
                set_bc(s, bc);
            }
        }
        Ok(self)
    }

    /// **A two-cell model of this composite alone**: cell id 1 is the inside,
    /// filled with material 0, and cell id 2 the outside, material 1, both in
    /// the root universe. What the drawings and the stochastic-volume check
    /// are run on, so they see exactly the regions transport would.
    pub fn two_cell_geometry(&self) -> Geometry {
        Geometry {
            surfaces: self.surfaces.clone(),
            cells: vec![
                Cell::material(1, self.negative.clone(), 0, 293.6),
                Cell::material(2, self.positive.clone(), 1, 293.6),
            ],
            universes: vec![Universe {
                id: 0,
                cell_indices: vec![0, 1],
            }],
            lattices: Vec::new(),
            root_universe: 0,
        }
    }
}

/// **Any composite**, for code that holds several kinds in one list. Each
/// variant wraps the composite's own type.
#[derive(Debug, Clone)]
pub enum Composite {
    CylinderSector(CylinderSector),
    IsogonalOctagon(IsogonalOctagon),
    ConeOneSided(ConeOneSided),
    Polygon(Polygon),
    CruciformPrism(CruciformPrism),
    RectangularPrism(RectangularPrism),
    ConicalFrustum(ConicalFrustum),
    Vessel(Vessel),
}

impl Composite {
    /// The expanded surfaces and regions of the wrapped composite.
    pub fn region(&self) -> &CompositeRegion {
        match self {
            Self::CylinderSector(c) => c.region(),
            Self::IsogonalOctagon(c) => c.region(),
            Self::ConeOneSided(c) => c.region(),
            Self::Polygon(c) => c.region(),
            Self::CruciformPrism(c) => c.region(),
            Self::RectangularPrism(c) => c.region(),
            Self::ConicalFrustum(c) => c.region(),
            Self::Vessel(c) => c.region(),
        }
    }
}

// ── Region assembly (RPN), the stand-in for openmc.Region operators ─────────

/// Builds a [`CompositeRegion`] surface by surface. `half` gives the RPN of a
/// single half-space; `and`/`or`/`not` combine RPN fragments the way
/// `openmc.Intersection`, `openmc.Union` and `openmc.Complement` do.
#[derive(Debug, Default)]
struct Builder {
    surfaces: Vec<SurfaceKind>,
    takes_boundary: Vec<bool>,
}

impl Builder {
    /// Add a surface that takes the composite's boundary condition; returns its index.
    fn add(&mut self, s: SurfaceKind) -> usize {
        self.add_with(s, true)
    }

    /// Add a surface, saying whether it takes the boundary condition.
    fn add_with(&mut self, s: SurfaceKind, takes_boundary: bool) -> usize {
        self.surfaces.push(s);
        self.takes_boundary.push(takes_boundary);
        self.surfaces.len() - 1
    }

    /// Finish with `negative`; `positive` is upstream's default `~(-self)`.
    fn finish(self, negative: Vec<RegionToken>) -> CompositeRegion {
        let positive = not(negative.clone());
        self.finish_with(negative, positive)
    }

    /// Finish with both half-spaces given (a composite that overrides `__pos__`).
    fn finish_with(
        self,
        negative: Vec<RegionToken>,
        positive: Vec<RegionToken>,
    ) -> CompositeRegion {
        CompositeRegion {
            surfaces: self.surfaces,
            negative,
            positive,
            takes_boundary: self.takes_boundary,
        }
    }
}

/// `-s` (inside, negative half-space) of surface `i`.
fn neg(i: usize) -> Vec<RegionToken> {
    vec![RegionToken::HalfSpace {
        surface_idx: i,
        sense: HalfSpaceSense::Inside,
    }]
}

/// `+s` (outside, positive half-space) of surface `i`.
fn pos(i: usize) -> Vec<RegionToken> {
    vec![RegionToken::HalfSpace {
        surface_idx: i,
        sense: HalfSpaceSense::Outside,
    }]
}

/// Join fragments with a binary operator, left to right.
fn join(parts: Vec<Vec<RegionToken>>, op: RegionToken) -> Vec<RegionToken> {
    let mut out = Vec::new();
    for (k, p) in parts.into_iter().enumerate() {
        out.extend(p);
        if k > 0 {
            out.push(op);
        }
    }
    out
}

/// `a & b & ...` (`openmc.Intersection`).
fn and(parts: Vec<Vec<RegionToken>>) -> Vec<RegionToken> {
    join(parts, RegionToken::Intersection)
}

/// `a | b | ...` (`openmc.Union`).
fn or(parts: Vec<Vec<RegionToken>>) -> Vec<RegionToken> {
    join(parts, RegionToken::Union)
}

/// `~a` (`openmc.Complement`).
fn not(mut a: Vec<RegionToken>) -> Vec<RegionToken> {
    a.push(RegionToken::Complement);
    a
}

/// Re-index a token stream's half-spaces by `offset`.
fn shift_tokens(tokens: &[RegionToken], offset: usize) -> Vec<RegionToken> {
    tokens
        .iter()
        .map(|t| match *t {
            RegionToken::HalfSpace { surface_idx, sense } => RegionToken::HalfSpace {
                surface_idx: surface_idx + offset,
                sense,
            },
            other => other,
        })
        .collect()
}

/// Set a surface's boundary condition.
fn set_bc(s: &mut SurfaceKind, bc: BoundaryType) {
    match s {
        SurfaceKind::XPlane(p) => p.bc = bc,
        SurfaceKind::YPlane(p) => p.bc = bc,
        SurfaceKind::ZPlane(p) => p.bc = bc,
        SurfaceKind::Plane(p) => p.bc = bc,
        SurfaceKind::Sphere(p) => p.bc = bc,
        SurfaceKind::XCylinder(p) => p.bc = bc,
        SurfaceKind::YCylinder(p) => p.bc = bc,
        SurfaceKind::ZCylinder(p) => p.bc = bc,
        SurfaceKind::XCone(p) => p.bc = bc,
        SurfaceKind::YCone(p) => p.bc = bc,
        SurfaceKind::ZCone(p) => p.bc = bc,
        SurfaceKind::Quadric(p) => p.bc = bc,
        SurfaceKind::XTorus(p) => p.bc = bc,
        SurfaceKind::YTorus(p) => p.bc = bc,
        SurfaceKind::ZTorus(p) => p.bc = bc,
    }
}

// ── Small numpy stand-ins, written in numpy's operation order ───────────────

/// `np.cross(a, b)` for 3-vectors.
fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

/// `np.dot(a, b)` for 3-vectors **as numpy evaluates it**: an FMA chain,
/// `fma(a2, b2, fma(a1, b1, a0 b0))`.
///
/// `np.dot` on float64 vectors calls BLAS `ddot`; numpy's wheels ship
/// OpenBLAS, whose x86_64 kernel accumulates with fused multiply-adds.
/// Measured 2026-11-12 (numpy 2.4.6) on `CylinderSector(1, 3, 20, 130,
/// center=(0.2, 0.3), axis='x')`: `np.dot(n, p1)` for `plane2.d` gave
/// `0.6920903430595148`, the FMA chain gives the same, and a left-to-right
/// sum (also the exactly rounded value) gives `...147`, 1 ulp apart. So the
/// coefficients OpenMC's Python hands its C++ code are FMA-chain dot
/// products, and this follows them (`f64::mul_add` is a correctly rounded
/// FMA on every target). Pinned by
/// `tests/composite_surfaces_vs_openmc.rs::surface_coefficients_match_openmc_python`.
fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[2].mul_add(b[2], a[1].mul_add(b[1], a[0] * b[0]))
}

/// `a - b` for 3-vectors.
fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

/// `np.linalg.norm(a)`: `sqrt(dot(a, a))`.
fn norm(a: [f64; 3]) -> f64 {
    dot(a, a).sqrt()
}

/// `openmc.Plane.from_points(p1, p2, p3)` (`openmc/surface.py`, lines
/// 767-806): normal `n = (p2 - p1) x (p3 - p1)`, `d = n . p1`. `None` when the
/// points lie on a line (upstream's `ValueError`, `np.allclose(n, 0)`).
fn plane_from_points(p1: [f64; 3], p2: [f64; 3], p3: [f64; 3]) -> Option<Plane> {
    let n = cross(sub(p2, p1), sub(p3, p1));
    if n.iter().all(|v| v.abs() <= 1e-8) {
        return None;
    }
    Some(Plane {
        a: n[0],
        b: n[1],
        c: n[2],
        d: dot(n, p1),
        bc: BoundaryType::Transmissive,
    })
}

/// `Plane.flip_normal()` (`openmc/surface.py`, lines 808-813).
fn flip_normal(p: &mut Plane) {
    p.a = -p.a;
    p.b = -p.b;
    p.c = -p.c;
    p.d = -p.d;
}

/// Reorder a point to the composite's axis: `p[:] = p[coord_map]`, i.e.
/// `out[i] = p[map[i]]`.
fn remap(p: [f64; 3], map: [usize; 3]) -> [f64; 3] {
    [p[map[0]], p[map[1]], p[map[2]]]
}

/// Transmissive boundary, upstream's default.
const T: BoundaryType = BoundaryType::Transmissive;
