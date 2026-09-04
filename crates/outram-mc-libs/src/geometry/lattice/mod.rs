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
//! special kind of universe fill: [`crate::geometry::geometry::Geometry`]
//! descends into it exactly as it would a nested universe.

use super::position::{Direction, Position};

/// Lattice type tag. Maps to `openmc::LatticeType`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LatticeType {
    Rect,
    Hex,
}

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
    fn is_3d(&self) -> bool {
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

/// Sentinel for an unused entry in a [`HexLattice`]'s "square" universe array —
/// the corner cells that fall outside the hexagon. Mirrors OpenMC's `C_NONE`
/// (`-1`) fill marker (`include/openmc/constants.h`).
pub const HEX_NONE: i32 = -1;

/// Orientation of a hexagonal lattice. Maps to `openmc::HexLattice::Orientation`
/// (`include/openmc/lattice.h:296`).
///
/// - [`HexOrientation::Y`] — two sides of every tile are parallel to the y-axis
///   (OpenMC default). Flat tile edges face ±x.
/// - [`HexOrientation::X`] — two sides parallel to the x-axis; the first element
///   of each ring starts along +x.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HexOrientation {
    /// Sides parallel to the y-axis (OpenMC default).
    Y,
    /// Sides parallel to the x-axis.
    X,
}

/// Floating-point coincidence tolerance for equal *distances*. Mirrors
/// `FP_COINCIDENT` (`include/openmc/constants.h:55`). Used by
/// [`HexLattice::get_indices`]'s boundary handling.
const FP_COINCIDENT: f64 = 1.0e-12;
/// Floating-point precision floor for "already on the edge" tests. Mirrors
/// `FP_PRECISION` (`include/openmc/constants.h:53`).
const FP_PRECISION: f64 = 1.0e-14;

/// Are two distances coincident within tolerance? Mirrors the inline
/// `coincident(d1, d2)` helper (`include/openmc/geometry.h:33`).
#[inline]
fn coincident(d1: f64, d2: f64) -> bool {
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
    fn is_3d(&self) -> bool {
        self.n_axial > 1
    }

    /// Width of the skewed square array along each planar axis: `2*n_rings - 1`.
    #[inline]
    fn n_side(&self) -> usize {
        2 * self.n_rings - 1
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
    fn center_offset(&self, i: [i32; 3]) -> Position {
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
    /// [`crate::geometry::geometry::Geometry`] descent stores the *tile-local*
    /// position, so `r_local` is passed here and the lattice-frame position is
    /// reconstructed via `r_local + center_offset(i_xyz)` (an exact inverse of
    /// [`Self::get_local_position`]).
    pub fn distance(&self, r_local: Position, u: Direction, i_xyz: [i32; 3]) -> (f64, [i32; 3]) {
        // Reconstruct the lattice-frame position (inverse of get_local_position).
        let off = self.center_offset(i_xyz);
        let r = Position {
            x: r_local.x + off.x,
            y: r_local.y + off.y,
            z: if self.is_3d() {
                r_local.z + off.z
            } else {
                r_local.z
            },
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

    /// Build a 2-D hexagonal lattice from the user-facing **ring** description,
    /// mirroring the OpenMC Python `HexLattice.universes = [[ring], …]` setter.
    ///
    /// `rings` is ordered **outermost ring first**, and within each ring the
    /// elements are listed clockwise starting at the "top" (see the notebook's
    /// `show_indices`). The outer ring has `6*(n_rings-1)` elements, the next
    /// `6*(n_rings-2)`, …, and the innermost "ring" is the single central tile.
    /// Each entry is a universe index. This routine walks the same skewed index
    /// path OpenMC's `fill_lattice_y` / `fill_lattice_x`
    /// (`src/lattice.cpp:546,598`) uses so the stored array matches OpenMC
    /// exactly. `outer` fills everything outside the hexagon.
    ///
    /// Panics if the ring sizes are inconsistent with a hexagon of
    /// `rings.len()` rings.
    pub fn from_rings(
        id: i32,
        orientation: HexOrientation,
        center: Position,
        radial_pitch: f64,
        rings: &[Vec<usize>],
        outer: Option<usize>,
    ) -> Self {
        let n_rings = rings.len();
        assert!(n_rings >= 1, "a hex lattice needs at least one ring");
        // Validate ring sizes: outer→inner is 6(n-1), 6(n-2), …, 1.
        for (k, ring) in rings.iter().enumerate() {
            let expected = if k == n_rings - 1 {
                1
            } else {
                6 * (n_rings - 1 - k)
            };
            assert_eq!(
                ring.len(),
                expected,
                "ring {k} (outer-first) has {} elements, expected {expected} for a {n_rings}-ring hex lattice",
                ring.len()
            );
        }
        let n_side = 2 * n_rings - 1;
        let mut universes = vec![HEX_NONE; n_side * n_side];
        // Geometry-derived placement (op-6tz.38 fix): map each ring's elements
        // onto the skewed tiles at that hex-radius via [`fill_level`], so the
        // fill round-trips through `get_indices`/`universe_at`. (The old
        // row-order `fill_lattice_x/y` walk mis-placed ring-order input.)
        fill_level(n_rings, orientation, rings, &mut universes);

        HexLattice {
            id,
            orientation,
            n_rings,
            n_axial: 1,
            center,
            pitch: [radial_pitch, 0.0],
            universes,
            outer,
        }
    }

    /// Build a **3-D** hexagonal lattice: `levels.len()` axially-stacked copies
    /// of a hexagonal ring fill, one full ring description per axial level.
    ///
    /// Mirrors OpenMC's 3-D `HexLattice` input, where `universes` is nested one
    /// level deeper than the 2-D case — `[axial_level][ring][element]` — and the
    /// C++ `fill_lattice_x`/`fill_lattice_y` (`src/lattice.cpp:546,598`) wrap the
    /// 2-D ring walk in an outer axial `m` loop. Each axial level is an
    /// independent 2-D hexagonal fill written into its own `(2*n_rings-1)^2`
    /// slice of [`Self::universes`] at flat offset `(2*n_rings-1)^2 * iz` (see
    /// [`Self::flat_index`]). The 2-D [`Self::from_rings`] is the special case
    /// `levels.len() == 1`; a single-level call here reproduces its planar layout
    /// (only `n_axial`/`pitch[1]` differ).
    ///
    /// # Parameters
    /// - `id`, `orientation`, `center`, `outer` — as [`Self::from_rings`];
    ///   `center.z` is the axial centre of the whole stack, in cm.
    /// - `radial_pitch` — tile flat-to-flat pitch in cm (`pitch[0]`).
    /// - `axial_pitch` — height of one axial level in cm (`pitch[1]`).
    /// - `levels` — one entry per axial level. **`levels[0]` is the bottom level**
    ///   (`iz = 0`, lowest z; its centre sits at `center.z - (n_axial-1)/2 ·
    ///   axial_pitch`), matching the internal `iz` convention of
    ///   [`Self::get_indices`]/[`Self::center_offset`]/[`Self::distance`]. This is
    ///   the **reverse** of OpenMC's Python display (top-first) — flip the outer
    ///   list when porting a Python case. Every level must have the same ring
    ///   count; within a level the ring/element order is that of
    ///   [`Self::from_rings`] (outermost ring first, single central tile last).
    ///
    /// Panics if `levels` is empty, if the levels disagree on ring count, or if
    /// any ring size is inconsistent with a hexagon of that ring count.
    pub fn from_rings_3d(
        id: i32,
        orientation: HexOrientation,
        center: Position,
        radial_pitch: f64,
        axial_pitch: f64,
        levels: &[Vec<Vec<usize>>],
        outer: Option<usize>,
    ) -> Self {
        let n_axial = levels.len();
        assert!(n_axial >= 1, "a hex lattice needs at least one axial level");
        let n_rings = levels[0].len();
        assert!(n_rings >= 1, "a hex lattice needs at least one ring");

        let n_side = 2 * n_rings - 1;
        let block = n_side * n_side;
        let mut universes = vec![HEX_NONE; block * n_axial];

        for (iz, rings) in levels.iter().enumerate() {
            assert_eq!(
                rings.len(),
                n_rings,
                "axial level {iz} has {} rings, expected {n_rings} (all levels must have the same ring count)",
                rings.len()
            );
            // Validate ring sizes: outer→inner is 6(n-1), 6(n-2), …, 1.
            for (k, ring) in rings.iter().enumerate() {
                let expected = if k == n_rings - 1 {
                    1
                } else {
                    6 * (n_rings - 1 - k)
                };
                assert_eq!(
                    ring.len(),
                    expected,
                    "axial level {iz} ring {k} (outer-first) has {} elements, expected {expected} for a {n_rings}-ring hex lattice",
                    ring.len()
                );
            }
            let slice = &mut universes[iz * block..(iz + 1) * block];
            // Geometry-derived per-level placement (op-6tz.38 fix); see from_rings.
            fill_level(n_rings, orientation, rings, slice);
        }

        HexLattice {
            id,
            orientation,
            n_rings,
            n_axial,
            center,
            pitch: [radial_pitch, axial_pitch],
            universes,
            outer,
        }
    }
}

/// Planar centre-of-tile coordinates `(x, y)` of skewed-axial coordinates
/// `(a, b)` for the given orientation, in flat-to-flat pitch units with the
/// central tile at `center = 0`.
fn tile_xy(a: i32, b: i32, orientation: HexOrientation) -> (f64, f64) {
    let (a, b) = (a as f64, b as f64);
    let s3 = 3.0_f64.sqrt();
    match orientation {
        HexOrientation::Y => (s3 / 2.0 * a, b + a / 2.0),
        HexOrientation::X => (a + b / 2.0, s3 / 2.0 * b),
    }
}

/// Hexagonal ring index (0 = centre) of skewed-axial coordinates `(a, b)`.
///
/// This is the cube-coordinate hex distance `(|a| + |b| + |a+b|)/2` — the
/// number of tiles crossed on a straight walk from the centre. A tile is inside
/// an `n_rings` hexagon iff its ring index is `<= n_rings - 1`, which is exactly
/// [`HexLattice::are_valid_indices`] expressed on `(a, b)`.
fn tile_ring(a: i32, b: i32) -> usize {
    ((a.abs() + b.abs() + (a + b).abs()) / 2) as usize
}

/// Flat storage slots of every tile in ring `k` (0 = centre), ordered by
/// increasing planar angle **starting from +x and proceeding
/// counter-clockwise**.
///
/// This is the geometry-derived placement order [`HexLattice::from_rings`] /
/// [`HexLattice::from_rings_3d`] use to map a user ring's elements onto skewed
/// tiles, replacing the row-order walk of [`fill_lattice_y`] / [`fill_lattice_x`]
/// that caused bead op-6tz.38. Ring `k` (for `k >= 1`) yields exactly `6*k`
/// slots; ring `0` yields the single central slot. The returned slots are a
/// disjoint partition of all hexagon tiles across `k = 0..n_rings`, so the fill
/// is a bijection and hence round-trip-correct.
///
/// See the OpenMC-fidelity caveat on [`HexLattice::from_rings`]: the exact
/// angular start/rotation is best-effort, not verified bit-identical to OpenMC.
fn ring_slots(n_rings: usize, k: usize, orientation: HexOrientation) -> Vec<usize> {
    let nr = n_rings as i32;
    let n_side = (2 * nr - 1) as usize;
    let mut tiles: Vec<(f64, usize)> = Vec::new();
    for iy in 0..n_side as i32 {
        for ix in 0..n_side as i32 {
            let (a, b) = (ix - (nr - 1), iy - (nr - 1));
            if tile_ring(a, b) != k {
                continue;
            }
            let (x, y) = tile_xy(a, b, orientation);
            let mut ang = y.atan2(x);
            if ang < 0.0 {
                ang += std::f64::consts::TAU;
            }
            tiles.push((ang, n_side * iy as usize + ix as usize));
        }
    }
    // All tiles in one ring have distinct angles, so this is a total order.
    tiles.sort_by(|p, q| p.0.total_cmp(&q.0));
    tiles.into_iter().map(|(_, slot)| slot).collect()
}

/// Write one axial level's ring-nested universes into its `(2*n_rings-1)^2`
/// skewed slice using the geometry-derived [`ring_slots`] placement.
///
/// `rings` is outer-ring-first (as accepted by [`HexLattice::from_rings`]); the
/// outer ring has hex-radius `n_rings-1` and the innermost (single-tile) ring
/// radius `0`. `out` is a single already-`HEX_NONE`-filled block. The caller
/// (`from_rings`/`from_rings_3d`) validates ring sizes inline before calling.
fn fill_level(n_rings: usize, orientation: HexOrientation, rings: &[Vec<usize>], out: &mut [i32]) {
    for (j, ring) in rings.iter().enumerate() {
        // Outer-first input → hex-radius counts down; centre (k = 0) is last.
        let k = n_rings - 1 - j;
        let slots = ring_slots(n_rings, k, orientation);
        debug_assert_eq!(
            ring.len(),
            slots.len(),
            "ring {j} (radius {k}) size {} != tile count {}",
            ring.len(),
            slots.len()
        );
        for (&elem, &slot) in ring.iter().zip(slots.iter()) {
            out[slot] = elem as i32;
        }
    }
}

/// Fill the skewed universe array for a `'y'`-orientation hex lattice from the
/// flattened ring-order input. Ported from `HexLattice::fill_lattice_y`
/// (`src/lattice.cpp:598`), for a single axial level (2-D).
///
/// No longer used by `from_rings`/`from_rings_3d` (op-6tz.38 replaced the
/// row-order walk with the geometry-derived [`fill_level`]); retained for
/// reference against the C++ source.
#[allow(dead_code)]
fn fill_lattice_y(n_rings: usize, univ: &[i32], out: &mut [i32]) {
    let nr = n_rings as i32;
    let n_side = (2 * nr - 1) as usize;
    let mut input_index = 0usize;
    let mut i_x: i32 = 1;
    let mut i_a: i32 = nr - 1;

    // Upper triangular region (first n_rings-1 rows of input).
    for k in 0..(nr - 1) {
        i_x -= 1;
        for _ in 0..(k + 1) {
            let indx = n_side * (i_a + nr - 1) as usize + (i_x + nr - 1) as usize;
            out[indx] = univ[input_index];
            input_index += 1;
            i_x += 2;
            i_a -= 1;
        }
        i_x -= 2 * (k + 1);
        i_a += k + 1;
    }

    // Middle square region (next 2*n_rings-1 rows).
    for k in 0..(2 * nr - 1) {
        if k % 2 == 0 {
            i_x -= 1;
        } else {
            i_x += 1;
            i_a -= 1;
        }
        for _ in 0..(nr - (k % 2)) {
            let indx = n_side * (i_a + nr - 1) as usize + (i_x + nr - 1) as usize;
            out[indx] = univ[input_index];
            input_index += 1;
            i_x += 2;
            i_a -= 1;
        }
        i_x -= 2 * (nr - (k % 2));
        i_a += nr - (k % 2);
    }

    // Lower triangular region.
    for k in 0..(nr - 1) {
        i_x += 1;
        i_a -= 1;
        for _ in 0..(nr - k - 1) {
            let indx = n_side * (i_a + nr - 1) as usize + (i_x + nr - 1) as usize;
            out[indx] = univ[input_index];
            input_index += 1;
            i_x += 2;
            i_a -= 1;
        }
        i_x -= 2 * (nr - k - 1);
        i_a += nr - k - 1;
    }
}

/// Fill the skewed universe array for an `'x'`-orientation hex lattice. Ported
/// from `HexLattice::fill_lattice_x` (`src/lattice.cpp:546`), single axial level.
///
/// No longer used by `from_rings`/`from_rings_3d` (op-6tz.38 replaced the
/// row-order walk with the geometry-derived [`fill_level`]); retained for
/// reference against the C++ source.
#[allow(dead_code)]
fn fill_lattice_x(n_rings: usize, univ: &[i32], out: &mut [i32]) {
    let nr = n_rings as i32;
    let n_side = (2 * nr - 1) as usize;
    let mut input_index = 0usize;
    let mut i_a: i32 = -(nr - 1);
    let mut i_y: i32 = nr - 1;

    // Upper region (first n_rings-1 rows).
    for k in 0..(nr - 1) {
        for _ in 0..(k + nr) {
            let indx = n_side * (i_y + nr - 1) as usize + (i_a + nr - 1) as usize;
            out[indx] = univ[input_index];
            input_index += 1;
            i_a += 1;
        }
        i_a = -(nr - 1);
        i_y -= 1;
    }

    // Lower region (centerline downward).
    for k in 0..nr {
        i_a = -(nr - 1) + k;
        for _ in 0..(2 * nr - k - 1) {
            let indx = n_side * (i_y + nr - 1) as usize + (i_a + nr - 1) as usize;
            out[indx] = univ[input_index];
            input_index += 1;
            i_a += 1;
        }
        i_y -= 1;
    }
}

#[derive(Debug, Clone)]
/// A lattice fill — dispatched by enum, not a trait object (per the workspace
/// "enums over `dyn`" rule). [`crate::geometry::geometry::Geometry`] holds a
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
