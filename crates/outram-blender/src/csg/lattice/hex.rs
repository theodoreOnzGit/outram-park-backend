// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Ported from OpenMC (https://github.com/openmc-dev/openmc, MIT licence,
// Copyright (c) 2011-2026 Massachusetts Institute of Technology, UChicago
// Argonne LLC and OpenMC contributors; see LICENSE.openmc):
//   src/lattice.cpp, include/openmc/lattice.h
// Hexagonal lattice: indexing and distance.
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

//! Hexagonal lattice: orientation, indexing, local positions and tile
//! distance (split out of the lattice module, 2026-10-02).

#[allow(unused_imports)]
use crate::csg::position::{Direction, Position};
use super::hex_build::fill_level;

/// Sentinel for an unused entry in a [`HexLattice`]'s "square" universe array —
/// the corner cells that fall outside the hexagon. Mirrors OpenMC's `C_NONE`
/// (`-1`) fill marker (`include/openmc/constants.h`).
pub const HEX_NONE: i32 = -1;

/// Orientation of a hexagonal lattice. Maps to `openmc::HexLattice::Orientation`
/// (`include/openmc/lattice.h:296`).
///
/// - [`HexOrientation::Y`] — ~~two sides of every tile are parallel to the
///   y-axis (OpenMC default). Flat tile edges face ±x.~~ **CORRECTED
///   2026-09-25:** every tile has two faces **perpendicular** to the y-axis
///   (flat edges facing ±y, vertices pointing ±x), which is OpenMC's own
///   definition (`openmc/lattice.py`, `HexLattice.orientation`: *"the 'y'
///   orientation means that each lattice element has two faces that are
///   perpendicular to the y-axis"*) and what [`HexLattice`]'s centre
///   arithmetic builds: neighbour centres at `(0, ±pitch)` and
///   `(±sqrt(3)/2, ±1/2)·pitch`. OpenMC default. Found while building the
///   HTR-10 two-ball bed, whose B-layer balls sit on the 0-degree vertex
///   `(pitch/sqrt(3), 0)` and are drawn whole only because that is a vertex.
/// - [`HexOrientation::X`] — ~~two sides parallel to the x-axis~~ **CORRECTED
///   2026-09-25:** two faces perpendicular to the x-axis (flat edges facing
///   ±x; neighbour centres at `(±pitch, 0)`); the first element of each ring
///   starts along +x.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HexOrientation {
    /// Two faces of every tile perpendicular to the y-axis (OpenMC default).
    Y,
    /// Two faces of every tile perpendicular to the x-axis.
    X,
}

/// Floating-point coincidence tolerance for equal *distances*. Mirrors
/// `FP_COINCIDENT` (`include/openmc/constants.h:55`). Used by
/// [`HexLattice::get_indices`]'s boundary handling.
pub(crate) const FP_COINCIDENT: f64 = 1.0e-12;
/// Floating-point precision floor for "already on the edge" tests. Mirrors
/// `FP_PRECISION` (`include/openmc/constants.h:53`).
pub(crate) const FP_PRECISION: f64 = 1.0e-14;

/// Are two distances coincident within tolerance? Mirrors the inline
/// `coincident(d1, d2)` helper (`include/openmc/geometry.h:33`).
#[inline]
pub(crate) fn coincident(d1: f64, d2: f64) -> bool {
    (d1 - d2).abs() < FP_COINCIDENT
}

#[derive(Debug, Clone)]
/// A hexagonal lattice. Maps to `openmc::HexLattice`.
///
/// C++ source: `src/lattice.cpp:456` (constructor) and the `HexLattice::*`
/// methods that follow it, `include/openmc/lattice.h:253`.
///
/// # What it represents
///
/// A hexagonal lattice tiles the plane with `3*n_rings*(n_rings-1) + 1`
/// hexagonal tiles arranged in `n_rings` concentric rings (the innermost "ring"
/// is the single central tile). Each tile maps to a universe index. Optionally
/// the lattice is stacked `n_axial` times along z.
///
/// # Indexing (this is the crux)
///
/// Internally OpenMC stores the tiles in a **skewed** `(2*n_rings-1) x
/// (2*n_rings-1)` *square* array, with the unused corner entries set to
/// [`HEX_NONE`]. A tile is addressed by a signed index triplet `[ix, iy, iz]`
/// where `ix, iy` are the two skewed lattice axes offset by `n_rings-1` (so the
/// central tile is `[n_rings-1, n_rings-1, 0]`) and `iz` is the axial level. The
/// flat storage index is
/// `(2*n_rings-1)^2 * iz + (2*n_rings-1) * iy + ix` (see
/// [`Self::flat_index`]). Membership in the hexagon (as opposed to a skipped
/// corner) is [`Self::are_valid_indices`].
///
/// Units: `center`/`pitch` in cm.
pub struct HexLattice {
    /// User-facing lattice id.
    pub id: i32,
    /// Orientation of the tiles (see [`HexOrientation`]).
    pub orientation: HexOrientation,
    /// Number of radial rings (the central tile is the innermost ring).
    pub n_rings: usize,
    /// Number of axial levels (`1` for a 2-D lattice).
    pub n_axial: usize,
    /// Lattice centre in cm. `z` is only used when `n_axial > 1`.
    pub center: Position,
    /// `[radial_pitch, axial_pitch]` in cm. `pitch[1]` is only used when 3-D.
    pub pitch: [f64; 2],
    /// Universe index for each tile in the skewed square array (row-major:
    /// `(2*n_rings-1)^2 * iz + (2*n_rings-1) * iy + ix`). Unused corners hold
    /// [`HEX_NONE`]. Valid entries are non-negative universe indices.
    pub universes: Vec<i32>,
    /// Universe filling everything outside the hexagon (`None` ⇒ a particle
    /// leaving the lattice is lost). Maps to `Lattice::outer_`.
    pub outer: Option<usize>,
}

impl HexLattice {
    /// Whether this lattice has a third (axial) dimension.
    #[inline]
    pub(crate) fn is_3d(&self) -> bool {
        self.n_axial > 1
    }

    /// Width of the skewed square array along each planar axis: `2*n_rings - 1`.
    #[inline]
    pub fn n_side(&self) -> usize {
        2 * self.n_rings - 1
    }

    /// Planar flat indices of the tiles in OpenMC's Python **ring order** —
    /// outermost ring first, each ring clockwise from the top, the centre
    /// last — i.e. the order `HexLattice.universes` lists them and
    /// `Lattice.get_unique_universes` (`openmc/lattice.py:110-136`) walks
    /// them. Recovered by pushing position numbers through the same
    /// [`fill_level`] walk [`Self::from_rings`] uses, so the two cannot drift.
    #[must_use]
    pub fn ring_order_flat_indices(&self) -> Vec<usize> {
        let n = self.n_rings;
        let mut next = 0usize;
        let rings: Vec<Vec<usize>> = (0..n)
            .map(|j| {
                let len = if j == n - 1 { 1 } else { 6 * (n - 1 - j) };
                (0..len)
                    .map(|_| {
                        next += 1;
                        next - 1
                    })
                    .collect()
            })
            .collect();
        let side = self.n_side();
        let mut probe = vec![HEX_NONE; side * side];
        fill_level(n, self.orientation, &rings, &mut probe);
        let mut order = vec![usize::MAX; next];
        for (flat, &pos) in probe.iter().enumerate() {
            if pos >= 0 {
                order[pos as usize] = flat;
            }
        }
        order
    }

    /// Flat storage index of tile `[ix, iy, iz]`. Ported from
    /// `HexLattice::get_flat_index` (`src/lattice.cpp:973`). The caller must have
    /// checked [`Self::are_valid_indices`] first.
    #[inline]
    pub fn flat_index(&self, i: [i32; 3]) -> usize {
        let n = self.n_side() as i32;
        (n * n * i[2] + n * i[1] + i[0]) as usize
    }

    /// Whether a signed index triplet addresses a real tile inside the hexagon.
    /// Ported from `HexLattice::are_valid_indices` (`src/lattice.cpp:725`).
    #[inline]
    pub fn are_valid_indices(&self, i: [i32; 3]) -> bool {
        let nr = self.n_rings as i32;
        i[0] >= 0
            && i[1] >= 0
            && i[2] >= 0
            && i[0] < 2 * nr - 1
            && i[1] < 2 * nr - 1
            && i[0] + i[1] > nr - 2
            && i[0] + i[1] < 3 * nr - 2
            && i[2] < self.n_axial as i32
    }

    /// The universe index at tile `i` — the tile's universe if it is a valid,
    /// filled tile, else the `outer` universe if defined, else `None` (lost).
    ///
    /// Mirrors the lattice-descent fallback in `find_cell` / `cross_lattice`
    /// (`src/geometry.cpp`): out-of-hexagon or unused-corner tiles resolve to
    /// `outer_`.
    pub fn universe_at(&self, i: [i32; 3]) -> Option<usize> {
        if self.are_valid_indices(i) {
            match self.universes.get(self.flat_index(i)).copied() {
                Some(u) if u >= 0 => Some(u as usize),
                _ => self.outer,
            }
        } else {
            self.outer
        }
    }

    /// The planar (and axial, if 3-D) offset of tile `i`'s centre from the
    /// global origin, so that `get_local_position(r, i) = r - center_offset(i)`.
    /// Split out from `get_local_position` so [`Self::distance`] can reconstruct
    /// the lattice-frame position from a tile-local one.
    pub(crate) fn center_offset(&self, i: [i32; 3]) -> Position {
        let nr = self.n_rings as f64;
        let p = self.pitch[0];
        let (ix, iy) = (i[0] as f64, i[1] as f64);
        let mut off = Position::ZERO;
        match self.orientation {
            HexOrientation::Y => {
                off.x = self.center.x + 3.0_f64.sqrt() / 2.0 * (ix - nr + 1.0) * p;
                off.y = self.center.y + (iy - nr + 1.0) * p + (ix - nr + 1.0) * p / 2.0;
            }
            HexOrientation::X => {
                off.x = self.center.x + (ix - nr + 1.0) * p + (iy - nr + 1.0) * p / 2.0;
                off.y = self.center.y + 3.0_f64.sqrt() / 2.0 * (iy - nr + 1.0) * p;
            }
        }
        if self.is_3d() {
            off.z = self.center.z - (0.5 * self.n_axial as f64 - i[2] as f64 - 0.5) * self.pitch[1];
        }
        off
    }

    /// Position of `r` recentred into the local frame of tile `i` (tile centre at
    /// the origin). Ported from `HexLattice::get_local_position`
    /// (`src/lattice.cpp:981`). The axial component is only shifted for a 3-D
    /// lattice.
    pub fn get_local_position(&self, r: Position, i: [i32; 3]) -> Position {
        let off = self.center_offset(i);
        Position {
            x: r.x - off.x,
            y: r.y - off.y,
            z: if self.is_3d() { r.z - off.z } else { r.z },
        }
    }

    /// **Exactly what [`Self::get_local_position`] subtracts** — see
    /// [`RectLattice::tile_center`] for why this is exposed rather than
    /// recovered by subtraction.
    pub fn tile_center(&self, i: [i32; 3]) -> Position {
        let off = self.center_offset(i);
        Position {
            x: off.x,
            y: off.y,
            z: if self.is_3d() { off.z } else { 0.0 },
        }
    }

    /// Map a position + direction to a (possibly out-of-range) skewed index
    /// triplet. Ported from `HexLattice::get_indices` (`src/lattice.cpp:877`),
    /// including the Voronoi nearest-centre refinement and the on-boundary
    /// direction tie-break.
    ///
    /// The returned indices may address an unused corner or lie outside the
    /// hexagon — test with [`Self::are_valid_indices`] / resolve with
    /// [`Self::universe_at`].
    pub fn get_indices(&self, r: Position, u: Direction) -> [i32; 3] {
        let p = self.pitch[0];

        // Offset by the lattice centre.
        let mut r_o = Position {
            x: r.x - self.center.x,
            y: r.y - self.center.y,
            z: r.z,
        };
        if self.is_3d() {
            r_o.z -= self.center.z;
        }

        // Axial index (with coincidence handling).
        let mut iz: i32 = 0;
        if self.is_3d() {
            let iz_ = r_o.z / self.pitch[1] + 0.5 * self.n_axial as f64;
            let iz_close = iz_.round();
            iz = if coincident(iz_, iz_close) {
                if u.w > 0.0 {
                    iz_close as i32
                } else {
                    iz_close as i32 - 1
                }
            } else {
                iz_.floor() as i32
            };
        }

        // Planar indices in the skewed basis — good to within a 2x2 candidate block.
        let (mut i0, mut i1): (i32, i32) = match self.orientation {
            HexOrientation::Y => {
                let alpha = r_o.y - r_o.x / 3.0_f64.sqrt();
                (
                    (r_o.x / (0.5 * 3.0_f64.sqrt() * p)).floor() as i32,
                    (alpha / p).floor() as i32,
                )
            }
            HexOrientation::X => {
                let alpha = r_o.y - r_o.x * 3.0_f64.sqrt();
                (
                    (-alpha / (3.0_f64.sqrt() * p)).floor() as i32,
                    (r_o.y / (0.5 * 3.0_f64.sqrt() * p)).floor() as i32,
                )
            }
        };
        // Offset so the centre tile is (n_rings-1, n_rings-1) and indices stay ≥ 0.
        i0 += self.n_rings as i32 - 1;
        i1 += self.n_rings as i32 - 1;

        // Voronoi refinement over the 2x2 candidate block: pick the tile whose
        // centre `r` is closest to, with the on-boundary tie broken by which tile
        // the direction `u` points into (lowest dot product wins).
        let mut i0_chg = 0;
        let mut i1_chg = 0;
        let mut d_min = f64::INFINITY;
        let mut dp_min = f64::INFINITY;
        for i in 0..2 {
            for j in 0..2 {
                let cand = [i0 + j, i1 + i, iz];
                let r_t = self.get_local_position(r, cand);
                let d = r_t.x * r_t.x + r_t.y * r_t.y;
                let on_boundary = coincident(1.0, d_min / d);
                if d < d_min || on_boundary {
                    let inv = d.sqrt();
                    let dp = u.u * (r_t.x / inv) + u.v * (r_t.y / inv);
                    if on_boundary && dp > dp_min {
                        continue;
                    }
                    d_min = d;
                    i0_chg = j;
                    i1_chg = i;
                    dp_min = dp;
                }
            }
        }
        [i0 + i0_chg, i1 + i1_chg, iz]
    }

    /// Distance \[cm\] to the next lattice-tile boundary along `(r, u)`, plus the
    /// index translation `[±1, …]` of the crossing.
    ///
    /// Ported from `HexLattice::distance` (`src/lattice.cpp:736`). OpenMC does
    /// this calculation relative to the *neighbour* tile centres (not the current
    /// tile) for finite-precision robustness, so it needs the current tile index
    /// `i_xyz` and the position in the **lattice** frame. This crate's
    /// [`crate::csg::geometry::Geometry`] descent stores the *tile-local*
    /// position, so `r_local` is passed here and the lattice-frame position is
    /// reconstructed via `r_local + center_offset(i_xyz)` (an exact inverse of
    /// [`Self::get_local_position`]).
    pub fn distance(&self, r_local: Position, u: Direction, i_xyz: [i32; 3]) -> (f64, [i32; 3]) {
        // Reconstruct the lattice-frame position (inverse of get_local_position).
        let off = self.center_offset(i_xyz);
        // x and y are reconstructed into the LATTICE frame, because the
        // beta/gamma/delta tests below run `get_local_position` against
        // NEIGHBOUR tile centres and so need a lattice-frame position.
        //
        // z is deliberately left TILE-LOCAL. The axial test at the end of this
        // function compares z against `+/- 0.5 * pitch[1]`, which is a
        // tile-local half-height; feeding it a lattice-frame z makes the
        // comparison wrong by the tile's own z offset, so every tile except the
        // one sitting at offset zero returns a NEGATIVE distance and the
        // neutron steps backwards. That is why the defect was invisible at
        // `n_axial == 1` (offset zero) and grew with the layer count.
        //
        // This mirrors OpenMC, which builds exactly this hybrid at the CALL
        // site rather than inside the function (`src/geometry.cpp:459-467`):
        //
        //     Position r_hex {p.coord(i - 1).r()};   // parent: lattice frame
        //     r_hex -= cell_above->translation_;
        //     r_hex.z = coord.r().z;                 // current: tile-local z
        //     lattice_distance = lat.distance(r_hex, u, coord.lattice_index());
        //
        // Doing it here keeps this crate's single tile-local calling convention
        // (see this function's doc comment) instead of pushing the special case
        // into `Geometry::distance_to_boundary`.
        let r = Position {
            x: r_local.x + off.x,
            y: r_local.y + off.y,
            z: r_local.z,
        };

        let s3 = 3.0_f64.sqrt();
        let (beta_dir, gamma_dir, delta_dir) = match self.orientation {
            HexOrientation::Y => (u.u * s3 / 2.0 + u.v / 2.0, u.u * s3 / 2.0 - u.v / 2.0, u.v),
            HexOrientation::X => (u.u, u.u / 2.0 - u.v * s3 / 2.0, u.u / 2.0 + u.v * s3 / 2.0),
        };

        let mut d = f64::INFINITY;
        let mut trans = [0i32; 3];

        // beta direction.
        let edge = -(0.5 * self.pitch[0]).copysign(beta_dir);
        let i_t = if beta_dir > 0.0 {
            [i_xyz[0] + 1, i_xyz[1], i_xyz[2]]
        } else {
            [i_xyz[0] - 1, i_xyz[1], i_xyz[2]]
        };
        let r_t = self.get_local_position(r, i_t);
        let beta = match self.orientation {
            HexOrientation::Y => r_t.x * s3 / 2.0 + r_t.y / 2.0,
            HexOrientation::X => r_t.x,
        };
        if (beta - edge).abs() > FP_PRECISION && beta_dir != 0.0 {
            d = (edge - beta) / beta_dir;
            trans = if beta_dir > 0.0 {
                [1, 0, 0]
            } else {
                [-1, 0, 0]
            };
        }

        // gamma direction.
        let edge = -(0.5 * self.pitch[0]).copysign(gamma_dir);
        let i_t = if gamma_dir > 0.0 {
            [i_xyz[0] + 1, i_xyz[1] - 1, i_xyz[2]]
        } else {
            [i_xyz[0] - 1, i_xyz[1] + 1, i_xyz[2]]
        };
        let r_t = self.get_local_position(r, i_t);
        let gamma = match self.orientation {
            HexOrientation::Y => r_t.x * s3 / 2.0 - r_t.y / 2.0,
            HexOrientation::X => r_t.x / 2.0 - r_t.y * s3 / 2.0,
        };
        if (gamma - edge).abs() > FP_PRECISION && gamma_dir != 0.0 {
            let this_d = (edge - gamma) / gamma_dir;
            if this_d < d {
                trans = if gamma_dir > 0.0 {
                    [1, -1, 0]
                } else {
                    [-1, 1, 0]
                };
                d = this_d;
            }
        }

        // delta direction.
        let edge = -(0.5 * self.pitch[0]).copysign(delta_dir);
        let i_t = if delta_dir > 0.0 {
            [i_xyz[0], i_xyz[1] + 1, i_xyz[2]]
        } else {
            [i_xyz[0], i_xyz[1] - 1, i_xyz[2]]
        };
        let r_t = self.get_local_position(r, i_t);
        let delta = match self.orientation {
            HexOrientation::Y => r_t.y,
            HexOrientation::X => r_t.x / 2.0 + r_t.y * s3 / 2.0,
        };
        if (delta - edge).abs() > FP_PRECISION && delta_dir != 0.0 {
            let this_d = (edge - delta) / delta_dir;
            if this_d < d {
                trans = if delta_dir > 0.0 {
                    [0, 1, 0]
                } else {
                    [0, -1, 0]
                };
                d = this_d;
            }
        }

        // Top and bottom (axial) faces.
        if self.is_3d() {
            let z = r.z;
            let z0 = (0.5 * self.pitch[1]).copysign(u.w);
            if (z - z0).abs() > FP_PRECISION && u.w != 0.0 {
                let this_d = (z0 - z) / u.w;
                if this_d < d {
                    d = this_d;
                    trans = if u.w > 0.0 { [0, 0, 1] } else { [0, 0, -1] };
                }
            }
        }

        (d, trans)
    }
}
