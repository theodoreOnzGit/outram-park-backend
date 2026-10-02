// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Ported from OpenMC (https://github.com/openmc-dev/openmc, MIT licence,
// Copyright (c) 2011-2026 Massachusetts Institute of Technology, UChicago
// Argonne LLC and OpenMC contributors; see LICENSE.openmc):
//   src/lattice.cpp, include/openmc/lattice.h
// Rectangular lattice.
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

//! Rectangular lattice (split out of the lattice module, 2026-10-02).

#[allow(unused_imports)]
use crate::csg::position::{Direction, Position};

#[derive(Debug, Clone)]
/// A rectangular lattice. Maps to `openmc::RectLattice`.
pub struct RectLattice {
    /// User-facing lattice id.
    pub id: i32,
    /// Number of grid cells in x, y, z (z = 1 for a 2-D lattice).
    pub n: [usize; 3],
    /// Lower-left corner of the lattice in cm.
    pub lower_left: Position,
    /// Pitch (cell width) in cm for each axis.
    pub pitch: [f64; 3],
    /// Universe index for each lattice element, row-major flat index
    /// `nx*ny*iz + nx*iy + ix`.
    pub universes: Vec<usize>,
    /// Universe filling the region outside the grid (`None` ⇒ no outer; a
    /// particle leaving the grid is lost). Maps to `Lattice::outer_`.
    pub outer: Option<usize>,
}

impl RectLattice {
    /// Whether this lattice has a third (z) dimension.
    #[inline]
    pub(crate) fn is_3d(&self) -> bool {
        self.n[2] > 1
    }

    /// Map a position to a (possibly out-of-range, signed) lattice index triplet.
    ///
    /// Ported from `RectLattice::get_indices` (`src/lattice.cpp:288`), including
    /// the coincidence handling: when the point sits on a tile boundary the index
    /// is resolved by the sign of the direction cosine `u`, so a particle just
    /// crossing into a tile is placed in the tile it is entering. Indices may be
    /// negative or ≥ `n` — call [`Self::are_valid_indices`] to test membership.
    pub fn get_indices(&self, r: Position, u: Direction) -> [i32; 3] {
        let idx = |num: f64, pitch: f64, dir: f64| -> i32 {
            let f = num / pitch;
            let close = f.round();
            if (f - close).abs() < 1.0e-12 {
                if dir > 0.0 {
                    close as i32
                } else {
                    close as i32 - 1
                }
            } else {
                f.floor() as i32
            }
        };
        let ix = idx(r.x - self.lower_left.x, self.pitch[0], u.u);
        let iy = idx(r.y - self.lower_left.y, self.pitch[1], u.v);
        let iz = if self.is_3d() {
            idx(r.z - self.lower_left.z, self.pitch[2], u.w)
        } else {
            0
        };
        [ix, iy, iz]
    }

    /// Whether a signed index triplet is inside the grid.
    /// Ported from `RectLattice::are_valid_indices` (`src/lattice.cpp:243`).
    #[inline]
    pub fn are_valid_indices(&self, i: [i32; 3]) -> bool {
        i[0] >= 0
            && (i[0] as usize) < self.n[0]
            && i[1] >= 0
            && (i[1] as usize) < self.n[1]
            && i[2] >= 0
            && (i[2] as usize) < self.n[2]
    }

    /// The universe index at tile `i` — the tile's universe if in range, else the
    /// `outer` universe if defined, else `None` (lost).
    pub fn universe_at(&self, i: [i32; 3]) -> Option<usize> {
        if self.are_valid_indices(i) {
            let flat =
                self.n[0] * self.n[1] * i[2] as usize + self.n[0] * i[1] as usize + i[0] as usize;
            self.universes.get(flat).copied()
        } else {
            self.outer
        }
    }

    /// Position of `r` recentred into the local frame of tile `i` (tile centre at
    /// the origin). Ported from `RectLattice::get_local_position`
    /// (`src/lattice.cpp:330`).
    pub fn get_local_position(&self, r: Position, i: [i32; 3]) -> Position {
        let mut out = r;
        out.x -= self.lower_left.x + (i[0] as f64 + 0.5) * self.pitch[0];
        out.y -= self.lower_left.y + (i[1] as f64 + 0.5) * self.pitch[1];
        if self.is_3d() {
            out.z -= self.lower_left.z + (i[2] as f64 + 0.5) * self.pitch[2];
        }
        out
    }

    /// **Exactly what [`Self::get_local_position`] subtracts** — the centre of
    /// tile `i` in this lattice's own frame, with `0.0` on any axis that
    /// `get_local_position` leaves untouched (`z` for a 2-D lattice).
    ///
    /// Returned so the global -> local frame offset can be **accumulated** as a
    /// particle descends, rather than reconstructed afterwards by subtracting
    /// two stored positions. That subtraction is catastrophic cancellation: for
    /// a tile centre of `0.2` under a probe at `y = -9` it returns
    /// `0.19999999999999929`, and the resulting ~1e-16 error in the local
    /// coordinate is enough to land a crossing point exactly on `dot == 0.0` in
    /// `nudge_across`, flipping that branch and moving the particle by `1e-9`.
    /// See `tests/cell_translation.rs`.
    pub fn tile_center(&self, i: [i32; 3]) -> Position {
        Position {
            x: self.lower_left.x + (i[0] as f64 + 0.5) * self.pitch[0],
            y: self.lower_left.y + (i[1] as f64 + 0.5) * self.pitch[1],
            z: if self.is_3d() {
                self.lower_left.z + (i[2] as f64 + 0.5) * self.pitch[2]
            } else {
                0.0
            },
        }
    }

    /// Distance \[cm\] to the next lattice-tile boundary along `(r, u)`, with `r`
    /// expressed in the current tile's local frame (tile centre at origin).
    ///
    /// Ported from `RectLattice::distance` (`src/lattice.cpp:252`): the oncoming
    /// tile edge is at `±½·pitch` in the sign of each direction cosine, and the
    /// returned distance is the minimum over the active axes. Also returns the
    /// tile-index translation `[±1,0,0]` etc. of the crossing.
    pub fn distance(&self, r: Position, u: Direction) -> (f64, [i32; 3]) {
        const FP: f64 = 1.0e-12;
        let x0 = (0.5 * self.pitch[0]).copysign(u.u);
        let y0 = (0.5 * self.pitch[1]).copysign(u.v);
        let mut d = f64::INFINITY;
        if u.u != 0.0 {
            d = d.min((x0 - r.x) / u.u);
        }
        if u.v != 0.0 {
            d = d.min((y0 - r.y) / u.v);
        }
        let mut z0 = 0.0;
        if self.is_3d() {
            z0 = (0.5 * self.pitch[2]).copysign(u.w);
            if u.w != 0.0 {
                d = d.min((z0 - r.z) / u.w);
            }
        }
        let mut trans = [0i32; 3];
        if u.u != 0.0 && (r.x + u.u * d - x0).abs() < FP {
            trans[0] = 1_f64.copysign(u.u) as i32;
        }
        if u.v != 0.0 && (r.y + u.v * d - y0).abs() < FP {
            trans[1] = 1_f64.copysign(u.v) as i32;
        }
        if self.is_3d() && u.w != 0.0 && (r.z + u.w * d - z0).abs() < FP {
            trans[2] = 1_f64.copysign(u.w) as i32;
        }
        (d, trans)
    }
}
