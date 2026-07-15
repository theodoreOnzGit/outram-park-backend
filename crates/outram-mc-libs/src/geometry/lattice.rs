/// Rectangular and hexagonal lattices.
///
/// C++ source: `src/lattice.cpp` (1219 LOC), `include/openmc/lattice.h`.
///
/// A lattice tiles space with identical universes on a periodic grid. OpenMC
/// supports two types:
///   - `RectLattice` — 3-D rectangular grid (nx × ny × nz pitches)
///   - `HexLattice`  — 2-D hexagonal grid (axial rings + axial levels)
///
/// Each lattice element maps to a universe index. The lattice is itself a
/// special kind of universe fill: [`crate::geometry::geometry::Geometry`]
/// descends into it exactly as it would a nested universe.

use super::position::{Direction, Position};

/// Lattice type tag. Maps to `openmc::LatticeType`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LatticeType { Rect, Hex }

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
    fn is_3d(&self) -> bool { self.n[2] > 1 }

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
                if dir > 0.0 { close as i32 } else { close as i32 - 1 }
            } else {
                f.floor() as i32
            }
        };
        let ix = idx(r.x - self.lower_left.x, self.pitch[0], u.u);
        let iy = idx(r.y - self.lower_left.y, self.pitch[1], u.v);
        let iz = if self.is_3d() { idx(r.z - self.lower_left.z, self.pitch[2], u.w) } else { 0 };
        [ix, iy, iz]
    }

    /// Whether a signed index triplet is inside the grid.
    /// Ported from `RectLattice::are_valid_indices` (`src/lattice.cpp:243`).
    #[inline]
    pub fn are_valid_indices(&self, i: [i32; 3]) -> bool {
        i[0] >= 0 && (i[0] as usize) < self.n[0]
            && i[1] >= 0 && (i[1] as usize) < self.n[1]
            && i[2] >= 0 && (i[2] as usize) < self.n[2]
    }

    /// The universe index at tile `i` — the tile's universe if in range, else the
    /// `outer` universe if defined, else `None` (lost).
    pub fn universe_at(&self, i: [i32; 3]) -> Option<usize> {
        if self.are_valid_indices(i) {
            let flat = self.n[0] * self.n[1] * i[2] as usize
                + self.n[0] * i[1] as usize
                + i[0] as usize;
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
        if u.u != 0.0 { d = d.min((x0 - r.x) / u.u); }
        if u.v != 0.0 { d = d.min((y0 - r.y) / u.v); }
        let mut z0 = 0.0;
        if self.is_3d() {
            z0 = (0.5 * self.pitch[2]).copysign(u.w);
            if u.w != 0.0 { d = d.min((z0 - r.z) / u.w); }
        }
        let mut trans = [0i32; 3];
        if u.u != 0.0 && (r.x + u.u * d - x0).abs() < FP { trans[0] = 1_f64.copysign(u.u) as i32; }
        if u.v != 0.0 && (r.y + u.v * d - y0).abs() < FP { trans[1] = 1_f64.copysign(u.v) as i32; }
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
        let mut r_o = Position { x: r.x - self.center.x, y: r.y - self.center.y, z: r.z };
        if self.is_3d() {
            r_o.z -= self.center.z;
        }

        // Axial index (with coincidence handling).
        let mut iz: i32 = 0;
        if self.is_3d() {
            let iz_ = r_o.z / self.pitch[1] + 0.5 * self.n_axial as f64;
            let iz_close = iz_.round();
            iz = if coincident(iz_, iz_close) {
                if u.w > 0.0 { iz_close as i32 } else { iz_close as i32 - 1 }
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
            z: if self.is_3d() { r_local.z + off.z } else { r_local.z },
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
            trans = if beta_dir > 0.0 { [1, 0, 0] } else { [-1, 0, 0] };
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
                trans = if gamma_dir > 0.0 { [1, -1, 0] } else { [-1, 1, 0] };
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
                trans = if delta_dir > 0.0 { [0, 1, 0] } else { [0, -1, 0] };
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
            let expected = if k == n_rings - 1 { 1 } else { 6 * (n_rings - 1 - k) };
            assert_eq!(
                ring.len(),
                expected,
                "ring {k} (outer-first) has {} elements, expected {expected} for a {n_rings}-ring hex lattice",
                ring.len()
            );
        }
        // Flatten outermost-first into the order OpenMC's fill routines consume.
        let flat: Vec<i32> = rings.iter().flatten().map(|&u| u as i32).collect();

        let n_side = 2 * n_rings - 1;
        let mut universes = vec![HEX_NONE; n_side * n_side];
        match orientation {
            HexOrientation::Y => fill_lattice_y(n_rings, &flat, &mut universes),
            HexOrientation::X => fill_lattice_x(n_rings, &flat, &mut universes),
        }

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
}

/// Fill the skewed universe array for a `'y'`-orientation hex lattice from the
/// flattened ring-order input. Ported from `HexLattice::fill_lattice_y`
/// (`src/lattice.cpp:598`), for a single axial level (2-D).
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
mod hex_tests {
    use super::*;

    /// A 4-ring, y-orientation lattice matching the `hexagonal-lattice` notebook:
    /// centre (0,0), pitch 1.25, big-pin (universe index 1) at the first element
    /// of every ring, regular pin (index 0) elsewhere, outer = 2.
    fn notebook_lattice() -> HexLattice {
        let outer_ring: Vec<usize> = std::iter::once(1).chain(std::iter::repeat(0).take(17)).collect();
        let ring_1: Vec<usize> = std::iter::once(1).chain(std::iter::repeat(0).take(11)).collect();
        let ring_2: Vec<usize> = std::iter::once(1).chain(std::iter::repeat(0).take(5)).collect();
        let inner_ring: Vec<usize> = vec![1];
        HexLattice::from_rings(
            4,
            HexOrientation::Y,
            Position::ZERO,
            1.25,
            &[outer_ring, ring_1, ring_2, inner_ring],
            Some(2),
        )
    }

    /// The hexagon holds `3*nr*(nr-1)+1` real tiles; the rest of the skewed square
    /// array is [`HEX_NONE`]. For 4 rings that is 37 valid tiles.
    #[test]
    fn valid_tile_count_matches_hexagon_formula() {
        let lat = notebook_lattice();
        let nr = lat.n_rings as i32;
        let n = lat.n_side() as i32;
        let mut valid = 0;
        for iy in 0..n {
            for ix in 0..n {
                if lat.are_valid_indices([ix, iy, 0]) {
                    valid += 1;
                }
            }
        }
        assert_eq!(valid, (3 * nr * (nr - 1) + 1) as i32, "4-ring hexagon should have 37 tiles");
        assert_eq!(valid, 37);
    }

    /// Round-trip: placing a particle exactly at a tile centre must recover that
    /// tile's skewed index via [`HexLattice::get_indices`]. This exercises the
    /// centre-offset geometry and the Voronoi nearest-centre refinement together.
    #[test]
    fn get_indices_recovers_every_tile_center() {
        let lat = notebook_lattice();
        let n = lat.n_side() as i32;
        let u = Direction::new(1.0, 0.0, 0.0);
        for iy in 0..n {
            for ix in 0..n {
                let i = [ix, iy, 0];
                if !lat.are_valid_indices(i) {
                    continue;
                }
                // Tile centre in the lattice (global) frame = center + center_offset.
                let c = lat.center_offset(i);
                let got = lat.get_indices(Position::new(c.x, c.y, 0.0), u);
                assert_eq!(got, i, "tile centre of {i:?} located as {got:?}");
            }
        }
    }

    /// The notebook fills the first element of each of the 4 rings with the big
    /// pin (index 1) and every other tile with the regular pin (index 0): 4 big
    /// pins, 33 regular pins.
    #[test]
    fn ring_fill_counts_match_notebook() {
        let lat = notebook_lattice();
        let big = lat.universes.iter().filter(|&&u| u == 1).count();
        let pin = lat.universes.iter().filter(|&&u| u == 0).count();
        let none = lat.universes.iter().filter(|&&u| u == HEX_NONE).count();
        assert_eq!(big, 4, "one big pin per ring");
        assert_eq!(pin, 33, "remaining tiles are regular pins");
        assert_eq!(big + pin + none, lat.universes.len());
        // The central tile is the innermost ring's single element → a big pin.
        let centre = [lat.n_rings as i32 - 1, lat.n_rings as i32 - 1, 0];
        assert_eq!(lat.universe_at(centre), Some(1), "central tile is the big pin");
    }

    /// Geometry of the y-orientation tile: its flat faces have outward normals at
    /// ±30°, ±90°, ±150° (never along ±x). So a particle leaving the central tile
    ///   - along +y (the delta face normal) exits at exactly half the flat-to-flat
    ///     pitch (`pitch/2`), crossing into `[0,+1,0]`;
    ///   - along +x exits through the +30° (beta) face at `pitch/2 / cos(30°)`,
    ///     crossing into `[+1,0,0]`.
    /// Both are checked against the ported `HexLattice::distance`.
    #[test]
    fn distance_from_center_matches_hex_face_geometry() {
        let lat = notebook_lattice();
        let centre = [lat.n_rings as i32 - 1, lat.n_rings as i32 - 1, 0];
        let half = 0.5 * lat.pitch[0];

        // +y → delta face → exactly half-pitch, cross into [0,+1,0].
        let (dy, ty) = lat.distance(Position::ZERO, Direction::new(0.0, 1.0, 0.0), centre);
        assert!((dy - half).abs() < 1e-9, "+y distance {dy} should be half-pitch {half}");
        assert_eq!(ty, [0, 1, 0], "+y crosses into the +delta neighbour");

        // +x → beta face at 30° → half-pitch / cos(30°), cross into [+1,0,0].
        let (dx, tx) = lat.distance(Position::ZERO, Direction::new(1.0, 0.0, 0.0), centre);
        let expect_x = half / (30.0_f64.to_radians().cos());
        assert!((dx - expect_x).abs() < 1e-9, "+x distance {dx} should be {expect_x}");
        assert_eq!(tx, [1, 0, 0], "+x crosses into the +beta neighbour");
    }

    /// Tiles outside the hexagon (unused corners) and beyond the grid resolve to
    /// the `outer` universe.
    #[test]
    fn outside_hexagon_resolves_to_outer() {
        let lat = notebook_lattice();
        // Corner [0,0] of the skewed array is outside the hexagon (0+0 <= nr-2).
        assert!(!lat.are_valid_indices([0, 0, 0]));
        assert_eq!(lat.universe_at([0, 0, 0]), Some(2), "corner → outer");
        // Far away in the lattice frame → outer.
        let far = lat.get_indices(Position::new(100.0, 100.0, 0.0), Direction::new(1.0, 0.0, 0.0));
        assert_eq!(lat.universe_at(far), Some(2), "far point → outer");
    }
}
