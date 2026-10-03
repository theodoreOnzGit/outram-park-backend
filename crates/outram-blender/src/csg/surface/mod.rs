// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Ported from OpenMC (https://github.com/openmc-dev/openmc, MIT licence,
// Copyright (c) 2011-2026 Massachusetts Institute of Technology, UChicago
// Argonne LLC and OpenMC contributors; see LICENSE.openmc):
//   src/surface.cpp, include/openmc/surface.h
// Surface types, sense / distance / normal / reflection.
// Moved here unchanged in substance from `outram-mc-libs`
// (`src/geometry/`) on 2026-10-02, GitHub issue #486: outram-blender owns the
// CSG description and its pure navigation kernel, outram-mc-libs keeps the
// transport-state work and re-exports these items under its old paths.
//
// This file is part of OUTRAM PARK.
//
// OUTRAM PARK is free software: you can redistribute it and/or modify it
// under the terms of the GNU General Public License as published by the
// Free Software Foundation, either version 3 of the License, or (at your
// option) any later version.
//
// OUTRAM PARK is distributed in the hope that it will be useful, but
// WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the GNU
// General Public License for more details.
//
// You should have received a copy of the GNU General Public License along
// with OUTRAM PARK.  If not, see <https://www.gnu.org/licenses/>.

//! Quadric surfaces for CSG geometry.
//!
//! C++ source: `src/surface.cpp` (1422 LOC), `include/openmc/surface.h` (419 LOC).
//!
//! OpenMC supports: XPlane, YPlane, ZPlane, Plane (general), XCylinder,
//! YCylinder, ZCylinder, Sphere, XCone, YCone, ZCone, Quadric, Torus{X,Y,Z}.
//!
//! Each surface implements two core methods:
//!   - `evaluate(r)` — signed "sense" function; negative = inside, positive = outside
//!   - `distance(r, u, coincident)` — distance to surface intersection along ray
//!
//! Boundary conditions: Transmissive, Vacuum, Reflective, Periodic, White.

use super::position::{Direction, Position};

mod quadric;
mod torus;
#[cfg(test)]
mod tests;

pub use quadric::{
    Plane, Quadric, Sphere, XCone, XCylinder, XPlane, YCone, YCylinder, YPlane, ZCone, ZCylinder,
    ZPlane,
};
pub use torus::{XTorus, YTorus, ZTorus};

/// Tolerance \[dimensionless, in the units of `evaluate`\] within which a point
/// counts as sitting **on** a surface rather than to one side of it.
///
/// `FP_COINCIDENT` in `include/openmc/constants.h:55`. Inside this band the sign
/// of `evaluate` is decided by floating-point round-off rather than by geometry,
/// so [`SurfaceKind::sense`] switches to the direction of travel instead.
pub const FP_COINCIDENT: f64 = 1.0e-12;

/// Surface boundary condition type.  Maps to `openmc::BoundaryType`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoundaryType {
    Transmissive,
    Vacuum,
    Reflective,
    Periodic,
    White,
}

/// Trait all surfaces must implement.  Maps to the virtual `Surface` base class.
pub trait Surface: Send + Sync {
    /// Evaluate the surface equation at `r`. Negative = inside the surface.
    fn evaluate(&self, r: Position) -> f64;

    /// Smallest positive distance along ray `(r, u)` to this surface.
    /// Returns `f64::INFINITY` if no intersection.
    /// `coincident` hints that `r` is already on this surface.
    fn distance(&self, r: Position, u: Direction, coincident: bool) -> f64;

    /// Outward unit normal at point `r` (assumes `r` is on the surface).
    fn normal(&self, r: Position) -> Direction;

    /// Reflect direction `u` off this surface at position `r`.
    #[inline]
    fn reflect(&self, r: Position, u: Direction) -> Direction {
        let n = self.normal(r);
        let dot = u.u * n.u + u.v * n.v + u.w * n.w;
        Direction::new(
            u.u - 2.0 * dot * n.u,
            u.v - 2.0 * dot * n.v,
            u.w - 2.0 * dot * n.w,
        )
    }
}

// ── Enum dispatch over the concrete surfaces ─────────────────────────────────
//
// Per the workspace design rules (enums over trait objects), CSG navigation
// dispatches over this closed set by `match` rather than `Box<dyn Surface>`.
// The `Surface` trait above stays as the compiler-enforced contract each
// concrete surface satisfies.

#[derive(Debug, Clone)]
/// A CSG quadric surface — the closed set the geometry navigator dispatches over.
///
/// Wraps each concrete surface struct. Maps to the OpenMC `Surface` polymorphic
/// hierarchy (`src/surface.cpp`), realised here as an enum so `match` gives
/// exhaustiveness and rust-analyzer go-to-definition on every variant.
pub enum SurfaceKind {
    XPlane(XPlane),
    YPlane(YPlane),
    ZPlane(ZPlane),
    Plane(Plane),
    Sphere(Sphere),
    XCylinder(XCylinder),
    YCylinder(YCylinder),
    ZCylinder(ZCylinder),
    XCone(XCone),
    YCone(YCone),
    ZCone(ZCone),
    Quadric(Quadric),
    XTorus(XTorus),
    YTorus(YTorus),
    ZTorus(ZTorus),
}

impl SurfaceKind {
    /// Signed surface sense at `r`: negative inside, positive outside.
    /// Delegates to the wrapped surface's [`Surface::evaluate`].
    #[inline]
    pub fn evaluate(&self, r: Position) -> f64 {
        match self {
            Self::XPlane(s) => s.evaluate(r),
            Self::YPlane(s) => s.evaluate(r),
            Self::ZPlane(s) => s.evaluate(r),
            Self::Plane(s) => s.evaluate(r),
            Self::Sphere(s) => s.evaluate(r),
            Self::XCylinder(s) => s.evaluate(r),
            Self::YCylinder(s) => s.evaluate(r),
            Self::ZCylinder(s) => s.evaluate(r),
            Self::XCone(s) => s.evaluate(r),
            Self::YCone(s) => s.evaluate(r),
            Self::ZCone(s) => s.evaluate(r),
            Self::Quadric(s) => s.evaluate(r),
            Self::XTorus(s) => s.evaluate(r),
            Self::YTorus(s) => s.evaluate(r),
            Self::ZTorus(s) => s.evaluate(r),
        }
    }

    /// Boolean sense used by cell membership: `true` = positive (outside) half-space.
    ///
    /// Ported from `Surface::sense` (`src/surface.cpp:117`). Normally this is
    /// just the sign of [`SurfaceKind::evaluate`], but **within
    /// [`FP_COINCIDENT`] of the surface the sign is round-off, not geometry**,
    /// so the side is decided from the direction of travel relative to the
    /// outward normal instead: a particle moving along `+n` is leaving the
    /// negative side, i.e. it is on the positive side.
    ///
    /// This matters wherever a particle sits on a surface — the state
    /// immediately after a boundary crossing, and at grazing incidence on a
    /// curved surface, where the evaluated sign flips essentially at random.
    /// For the surface a particle is *known* to be on, prefer the recorded
    /// [`crate::csg::cell::SurfaceToken`], which is exact; this is the
    /// fallback for every other surface.
    ///
    /// - `r` — position \[cm\].
    /// - `u` — unit direction of travel.
    #[inline]
    pub fn sense(&self, r: Position, u: Direction) -> bool {
        let f = self.evaluate(r);
        if f.abs() < FP_COINCIDENT {
            let n = self.normal(r);
            return u.u * n.u + u.v * n.v + u.w * n.w > 0.0;
        }
        f > 0.0
    }

    /// Smallest positive distance along ray `(r, u)` to this surface, or
    /// `INFINITY` if it is not crossed. `coincident` hints `r` sits on the surface.
    #[inline]
    pub fn distance(&self, r: Position, u: Direction, coincident: bool) -> f64 {
        match self {
            Self::XPlane(s) => s.distance(r, u, coincident),
            Self::YPlane(s) => s.distance(r, u, coincident),
            Self::ZPlane(s) => s.distance(r, u, coincident),
            Self::Plane(s) => s.distance(r, u, coincident),
            Self::Sphere(s) => s.distance(r, u, coincident),
            Self::XCylinder(s) => s.distance(r, u, coincident),
            Self::YCylinder(s) => s.distance(r, u, coincident),
            Self::ZCylinder(s) => s.distance(r, u, coincident),
            Self::XCone(s) => s.distance(r, u, coincident),
            Self::YCone(s) => s.distance(r, u, coincident),
            Self::ZCone(s) => s.distance(r, u, coincident),
            Self::Quadric(s) => s.distance(r, u, coincident),
            Self::XTorus(s) => s.distance(r, u, coincident),
            Self::YTorus(s) => s.distance(r, u, coincident),
            Self::ZTorus(s) => s.distance(r, u, coincident),
        }
    }

    /// Outward unit normal at `r` (assumes `r` lies on the surface).
    #[inline]
    pub fn normal(&self, r: Position) -> Direction {
        match self {
            Self::XPlane(s) => s.normal(r),
            Self::YPlane(s) => s.normal(r),
            Self::ZPlane(s) => s.normal(r),
            Self::Plane(s) => s.normal(r),
            Self::Sphere(s) => s.normal(r),
            Self::XCylinder(s) => s.normal(r),
            Self::YCylinder(s) => s.normal(r),
            Self::ZCylinder(s) => s.normal(r),
            Self::XCone(s) => s.normal(r),
            Self::YCone(s) => s.normal(r),
            Self::ZCone(s) => s.normal(r),
            Self::Quadric(s) => s.normal(r),
            Self::XTorus(s) => s.normal(r),
            Self::YTorus(s) => s.normal(r),
            Self::ZTorus(s) => s.normal(r),
        }
    }

    /// Specular reflection of direction `u` off this surface at `r`.
    #[inline]
    pub fn reflect(&self, r: Position, u: Direction) -> Direction {
        match self {
            Self::XPlane(s) => s.reflect(r, u),
            Self::YPlane(s) => s.reflect(r, u),
            Self::ZPlane(s) => s.reflect(r, u),
            Self::Plane(s) => s.reflect(r, u),
            Self::Sphere(s) => s.reflect(r, u),
            Self::XCylinder(s) => s.reflect(r, u),
            Self::YCylinder(s) => s.reflect(r, u),
            Self::ZCylinder(s) => s.reflect(r, u),
            Self::XCone(s) => s.reflect(r, u),
            Self::YCone(s) => s.reflect(r, u),
            Self::ZCone(s) => s.reflect(r, u),
            Self::Quadric(s) => s.reflect(r, u),
            Self::XTorus(s) => s.reflect(r, u),
            Self::YTorus(s) => s.reflect(r, u),
            Self::ZTorus(s) => s.reflect(r, u),
        }
    }

    /// This surface's boundary condition.
    #[inline]
    pub fn bc(&self) -> BoundaryType {
        match self {
            Self::XPlane(s) => s.bc,
            Self::YPlane(s) => s.bc,
            Self::ZPlane(s) => s.bc,
            Self::Plane(s) => s.bc,
            Self::Sphere(s) => s.bc,
            Self::XCylinder(s) => s.bc,
            Self::YCylinder(s) => s.bc,
            Self::ZCylinder(s) => s.bc,
            Self::XCone(s) => s.bc,
            Self::YCone(s) => s.bc,
            Self::ZCone(s) => s.bc,
            Self::Quadric(s) => s.bc,
            Self::XTorus(s) => s.bc,
            Self::YTorus(s) => s.bc,
            Self::ZTorus(s) => s.bc,
        }
    }
}
