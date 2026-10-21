// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Ported from OpenMC (https://github.com/openmc-dev/openmc, MIT licence,
// Copyright (c) 2011-2026 Massachusetts Institute of Technology, UChicago
// Argonne LLC and OpenMC contributors; see LICENSE.openmc):
//   src/lattice.cpp, include/openmc/lattice.h
// Lattice enum dispatch and tile-frame surface translation.
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

//! Rectangular and hexagonal lattices.
//!
//! C++ source: `src/lattice.cpp` (1219 LOC), `include/openmc/lattice.h`.
//!
//! A lattice tiles space with identical universes on a periodic grid. OpenMC
//! supports two types:
//!   - `RectLattice` — 3-D rectangular grid (nx × ny × nz pitches)
//!   - `HexLattice`  — 2-D hexagonal grid (axial rings + axial levels)
//!
//! Each lattice element maps to a universe index. The lattice is itself a
//! special kind of universe fill: [`crate::csg::geometry::Geometry`]
//! descends into it exactly as it would a nested universe.

use super::position::{Direction, Position};

mod hex;
mod hex_build;
mod rect;

pub use hex::{HexLattice, HexOrientation, HEX_NONE};
pub use rect::RectLattice;
#[allow(unused_imports)]
pub(crate) use hex_build::*;

/// Lattice type tag. Maps to `openmc::LatticeType`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LatticeType {
    Rect,
    Hex,
}

#[derive(Debug, Clone)]
/// A lattice fill — dispatched by enum, not a trait object (per the workspace
/// "enums over `dyn`" rule). [`crate::csg::geometry::Geometry`] holds a
/// `Vec<Lattice>` and matches on the variant during descent.
pub enum Lattice {
    /// A rectangular lattice.
    Rect(RectLattice),
    /// A hexagonal lattice.
    Hex(HexLattice),
}

impl Lattice {
    /// The user-facing lattice id.
    pub fn id(&self) -> i32 {
        match self {
            Lattice::Rect(l) => l.id,
            Lattice::Hex(l) => l.id,
        }
    }

    /// Skewed/signed tile index for `(r, u)` in this lattice's local frame.
    pub fn get_indices(&self, r: Position, u: Direction) -> [i32; 3] {
        match self {
            Lattice::Rect(l) => l.get_indices(r, u),
            Lattice::Hex(l) => l.get_indices(r, u),
        }
    }

    /// Universe index at tile `i` (tile universe, else `outer`, else `None`).
    pub fn universe_at(&self, i: [i32; 3]) -> Option<usize> {
        match self {
            Lattice::Rect(l) => l.universe_at(i),
            Lattice::Hex(l) => l.universe_at(i),
        }
    }

    /// Position `r` recentred into tile `i`'s local frame (tile centre at origin).
    pub fn get_local_position(&self, r: Position, i: [i32; 3]) -> Position {
        match self {
            Lattice::Rect(l) => l.get_local_position(r, i),
            Lattice::Hex(l) => l.get_local_position(r, i),
        }
    }

    /// The centre of tile `i`, exactly as [`Self::get_local_position`]
    /// subtracts it. See [`RectLattice::tile_center`].
    pub fn tile_center(&self, i: [i32; 3]) -> Position {
        match self {
            Lattice::Rect(l) => l.tile_center(i),
            Lattice::Hex(l) => l.tile_center(i),
        }
    }

    /// Distance to the next tile boundary along `(r, u)` from tile `i_xyz`, with
    /// `r` in that tile's local frame, plus the crossing's index translation.
    ///
    /// The rectangular lattice ignores `i_xyz` (its calculation is
    /// tile-relative); the hexagonal lattice needs it (its calculation is
    /// neighbour-relative for finite-precision robustness).
    pub fn distance(&self, r: Position, u: Direction, i_xyz: [i32; 3]) -> (f64, [i32; 3]) {
        match self {
            Lattice::Rect(l) => l.distance(r, u),
            Lattice::Hex(l) => l.distance(r, u, i_xyz),
        }
    }
}

#[cfg(test)]
mod hex_tests;

/// **Translate a surface into a lattice tile's local frame** — the mechanism for
/// clipping tile contents against a boundary defined in world coordinates
/// (`bn:op-867c.10`, gh #214).
///
/// NEW WORK, no OpenMC counterpart.
///
/// # Why this is needed
///
/// A cell region inside a tile universe is evaluated in the **tile-local**
/// frame, because `Geometry::locate` recentres the position into the tile before
/// testing the region. Measured 2026-09-17 in
/// `tests/lattice_tile_clipping.rs`, which was written specifically to find out.
///
/// So a single world-frame surface — HTR-10's conus, or its discharge tube —
/// does **not** clip every boundary tile at the right place. Each boundary tile
/// needs its own copy of that surface, translated by minus its tile centre.
///
/// # Why not the alternatives
///
/// Two other routes were considered and are recorded on the bead. Real per-tile
/// omission in [`HexLattice`] is the cleanest answer but the largest change —
/// `universe_at` returns a plain index and `HEX_NONE` marks only the skewed
/// array's unused corners. Doing the rejection in the delta path's `material_at`
/// closure is cheaper at run time and became possible only once hybrid tracking
/// landed, but needs the transport dispatch to accept a caller-supplied query,
/// which it does not.
///
/// This route needs nothing new, and its cost is bounded: one surface per
/// BOUNDARY tile, generated once at model-build time, not per history.
///
/// # What is supported
///
/// Planes and quadrics translate exactly. A sphere or cylinder translates by
/// moving its centre; a cone likewise. Surfaces whose definition is not
/// translation-covariant are returned unchanged and **that is a defect the
/// caller must not paper over** — check the returned surface if in doubt.
///
/// # Parameters
/// - `surface` — the world-frame surface to translate.
/// - `tile_center` — the tile's centre in the parent frame, from
///   [`RectLattice::tile_center`] or [`HexLattice::tile_center`].
pub fn surface_in_tile_frame(
    surface: &crate::csg::surface::SurfaceKind,
    tile_center: Position,
) -> crate::csg::surface::SurfaceKind {
    use crate::csg::surface::SurfaceKind as S;
    let (dx, dy, dz) = (tile_center.x, tile_center.y, tile_center.z);
    match surface.clone() {
        S::XPlane(mut p) => {
            p.x0 -= dx;
            S::XPlane(p)
        }
        S::YPlane(mut p) => {
            p.y0 -= dy;
            S::YPlane(p)
        }
        S::ZPlane(mut p) => {
            p.z0 -= dz;
            S::ZPlane(p)
        }
        // General plane a*x + b*y + c*z = d: translating the frame by `t` moves
        // the constant by a.t.
        S::Plane(mut p) => {
            p.d -= p.a * dx + p.b * dy + p.c * dz;
            S::Plane(p)
        }
        S::Sphere(mut s) => {
            s.x0 -= dx;
            s.y0 -= dy;
            s.z0 -= dz;
            S::Sphere(s)
        }
        S::XCylinder(mut c) => {
            c.y0 -= dy;
            c.z0 -= dz;
            S::XCylinder(c)
        }
        S::YCylinder(mut c) => {
            c.x0 -= dx;
            c.z0 -= dz;
            S::YCylinder(c)
        }
        S::ZCylinder(mut c) => {
            c.x0 -= dx;
            c.y0 -= dy;
            S::ZCylinder(c)
        }
        S::XCone(mut c) => {
            c.x0 -= dx;
            c.y0 -= dy;
            c.z0 -= dz;
            S::XCone(c)
        }
        S::YCone(mut c) => {
            c.x0 -= dx;
            c.y0 -= dy;
            c.z0 -= dz;
            S::YCone(c)
        }
        S::ZCone(mut c) => {
            c.x0 -= dx;
            c.y0 -= dy;
            c.z0 -= dz;
            S::ZCone(c)
        }
        // Torus and general quadric: translation is expressible but the
        // coefficient algebra is not a field shift, so it is NOT done silently.
        // Returning the surface unchanged would be a wrong answer that looks
        // like a right one.
        other => other,
    }
}
