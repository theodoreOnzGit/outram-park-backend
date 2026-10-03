// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Ported from OpenMC (https://github.com/openmc-dev/openmc, MIT licence,
// Copyright (c) 2011-2026 Massachusetts Institute of Technology, UChicago
// Argonne LLC and OpenMC contributors; see LICENSE.openmc):
//   src/universe.cpp, include/openmc/universe.h
// Universe: an ordered list of cells.
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

//! Universe hierarchy — the nesting mechanism for CSG geometry.
//!
//! C++ source: `src/universe.cpp` (217 LOC), `include/openmc/universe.h`.
//!
//! A `Universe` is a collection of `Cell`s. Particle tracking starts in the root
//! universe and recursively descends into fill universes/lattices to locate the
//! leaf cell a particle inhabits (see [`crate::csg::geometry::Geometry`]).
//!
//! Key operation: [`Universe::find_cell`] — given a local position, return the
//! first cell in this universe that contains it.

use super::cell::{Cell, SurfaceToken};
use super::position::{Direction, Position};
use super::surface::SurfaceKind;

#[derive(Debug, Clone)]
/// A universe — an ordered list of cells searched top-to-bottom.
/// Maps to `openmc::Universe`.
pub struct Universe {
    /// User-facing universe id.
    pub id: i32,
    /// Indices into the global cell array, in search order.
    pub cell_indices: Vec<usize>,
}

impl Universe {
    /// Find the first cell in this universe that contains a particle at `r`
    /// heading along `u` (both in this universe's local frame).
    ///
    /// Ported from `Universe::find_cell` (`src/universe.cpp:40`): iterate the
    /// universe's cells in order and return the first whose region contains the
    /// point. Returns the **global cell index**, or `None` if the point is in no
    /// cell of this universe (a geometry "lost particle").
    ///
    /// `on_surface` is the surface the particle is sitting on, if any — it
    /// disambiguates membership for a particle that has just crossed a boundary
    /// and would otherwise be re-selected into the cell it was leaving. See
    /// [`Cell::contains`] and [`SurfaceToken`]. Pass [`SurfaceToken::NONE`] for a
    /// standalone point query. Because every nested frame in this crate is a
    /// pure translation, the global token stays valid at every level.
    pub fn find_cell(
        &self,
        r: Position,
        u: Direction,
        surfaces: &[SurfaceKind],
        cells: &[Cell],
        on_surface: SurfaceToken,
    ) -> Option<usize> {
        for &i_cell in &self.cell_indices {
            if cells[i_cell].contains(r, u, surfaces, on_surface) {
                return Some(i_cell);
            }
        }
        None
    }
}
