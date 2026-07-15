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

/// A hexagonal lattice. Maps to `openmc::HexLattice`.
///
/// **Status: data-only stub.** Hex indexing (`get_indices`/`distance`) is not yet
/// ported — see bead op-6tz.11. The struct fixes the vocabulary so the
/// `hexagonal_lattice` notebook test can name it while remaining `#[ignore]`d.
pub struct HexLattice {
    pub id: i32,
    pub n_rings: usize,
    pub n_axial: usize,
    pub center: Position,
    pub pitch: [f64; 2],
    pub universes: Vec<usize>,
}
