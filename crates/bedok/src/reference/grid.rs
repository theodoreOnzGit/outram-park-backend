//! Grid geometry and the state-vector index convention.
//!
//! # Why this module exists first
//!
//! Every field in BEDOK — flux, fission source, power density, cross sections —
//! is stored as a flat vector indexed by a single rule. Getting that rule wrong
//! does not crash anything: it silently permutes the reactor, and the solver
//! converges happily to a wrong answer. It is the highest-risk single line in
//! the whole translation, so it is pinned here, once, with tests.
//!
//! # The convention
//!
//! Taken verbatim from Yan Ren's `main_exec_diff3d.m:176`, which is the
//! authoritative statement of how the MATLAB indexes its state vectors:
//!
//! ```text
//! idx = (g-1)*maxix*maxiy*maxiz + (ix-1)*maxiy*maxiz + (iy-1)*maxiz + iz
//! ```
//!
//! That is **group-major**, then `ix`, then `iy`, with `iz` varying fastest.
//! The MATLAB indices are 1-based; this port is 0-based, so the same rule
//! becomes:
//!
//! ```text
//! idx = g*nodes + ix*(ny*nz) + iy*nz + iz
//! ```
//!
//! with `nodes = nx*ny*nz`. The reference fixtures under
//! `tests/fixtures/*/` carry explicit `g,ix,iy,iz` columns using the **1-based**
//! MATLAB values, precisely so that this conversion is checked rather than
//! assumed.

use crate::error::{BedokError, Result};

/// The node grid a case is discretised on.
///
/// Holds only the counts. Physical dimensions live in [`Geometry`].
///
/// # Note on `nz`
///
/// A case constructor may *change* the axial node count relative to what the
/// user requested — `iaea3ds` raises `maxiz` from 18 to 19 when it appends the
/// axial reflector. Always read the grid back from the built case rather than
/// from the input parameters.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Grid {
    /// Number of nodes in x. MATLAB `params.maxix`.
    pub nx: usize,
    /// Number of nodes in y. MATLAB `params.maxiy`.
    pub ny: usize,
    /// Number of nodes in z. MATLAB `params.maxiz`.
    pub nz: usize,
    /// Number of energy groups. MATLAB `params.G`.
    pub ngroups: usize,
}

impl Grid {
    /// A grid with `nx` × `ny` × `nz` nodes and `ngroups` energy groups.
    ///
    /// # Errors
    ///
    /// [`BedokError::EmptyGrid`] if any dimension is zero — a zero-sized grid
    /// makes every index computation below meaningless, so it is rejected at
    /// construction rather than producing an empty solve.
    pub fn new(nx: usize, ny: usize, nz: usize, ngroups: usize) -> Result<Self> {
        if nx == 0 || ny == 0 || nz == 0 || ngroups == 0 {
            return Err(BedokError::EmptyGrid {
                nx,
                ny,
                nz,
                ngroups,
            });
        }
        Ok(Self {
            nx,
            ny,
            nz,
            ngroups,
        })
    }

    /// Number of spatial nodes, `nx*ny*nz`, ignoring energy groups.
    #[must_use]
    pub const fn nodes(&self) -> usize {
        self.nx * self.ny * self.nz
    }

    /// Length of a full state vector, `nodes * ngroups`.
    ///
    /// This is the length of `scalar_flux`, `fission_source` and `pwrdens` in
    /// the MATLAB — e.g. 10,982 for IAEA-3D (17×17×19 nodes, 2 groups).
    #[must_use]
    pub const fn state_len(&self) -> usize {
        self.nodes() * self.ngroups
    }

    /// Flat index of node `(ix, iy, iz)` in group `g`, all **0-based**.
    ///
    /// Implements `g*nodes + ix*ny*nz + iy*nz + iz` — the 0-based form of the
    /// MATLAB convention documented at module level.
    ///
    /// # Panics
    ///
    /// In debug builds, if any index is out of range. An out-of-range index is
    /// a programming error, and silently wrapping it would reintroduce exactly
    /// the class of bug this module exists to prevent.
    #[must_use]
    pub fn index(&self, g: usize, ix: usize, iy: usize, iz: usize) -> usize {
        debug_assert!(g < self.ngroups, "group {g} >= {}", self.ngroups);
        debug_assert!(ix < self.nx, "ix {ix} >= {}", self.nx);
        debug_assert!(iy < self.ny, "iy {iy} >= {}", self.ny);
        debug_assert!(iz < self.nz, "iz {iz} >= {}", self.nz);
        g * self.nodes() + ix * (self.ny * self.nz) + iy * self.nz + iz
    }

    /// Inverse of [`index`](Self::index): flat index back to `(g, ix, iy, iz)`,
    /// all 0-based.
    ///
    /// # Errors
    ///
    /// [`BedokError::IndexOutOfRange`] if `idx >= state_len()`.
    pub fn unindex(&self, idx: usize) -> Result<(usize, usize, usize, usize)> {
        if idx >= self.state_len() {
            return Err(BedokError::IndexOutOfRange {
                idx,
                len: self.state_len(),
            });
        }
        let nodes = self.nodes();
        let g = idx / nodes;
        let rem = idx % nodes;
        let ix = rem / (self.ny * self.nz);
        let rem = rem % (self.ny * self.nz);
        let iy = rem / self.nz;
        let iz = rem % self.nz;
        Ok((g, ix, iy, iz))
    }

    /// Flat index from the **1-based** indices used by the MATLAB and written
    /// into the reference fixtures.
    ///
    /// This is the single place the 1-based → 0-based conversion happens.
    /// Fixture loaders must go through it rather than subtracting one by hand
    /// at each call site.
    ///
    /// # Errors
    ///
    /// [`BedokError::IndexOutOfRange`] if any index is zero (1-based indices
    /// start at one) or exceeds its dimension.
    pub fn index_from_matlab(&self, g: usize, ix: usize, iy: usize, iz: usize) -> Result<usize> {
        if g == 0 || ix == 0 || iy == 0 || iz == 0 {
            return Err(BedokError::IndexOutOfRange { idx: 0, len: 1 });
        }
        if g > self.ngroups || ix > self.nx || iy > self.ny || iz > self.nz {
            return Err(BedokError::IndexOutOfRange {
                idx: idx_for_message(g, ix, iy, iz),
                len: self.state_len(),
            });
        }
        Ok(self.index(g - 1, ix - 1, iy - 1, iz - 1))
    }
}

/// Packs a 1-based coordinate into a single number purely for error messages.
fn idx_for_message(g: usize, ix: usize, iy: usize, iz: usize) -> usize {
    g.wrapping_mul(1_000_000)
        .wrapping_add(ix.wrapping_mul(10_000))
        .wrapping_add(iy.wrapping_mul(100))
        .wrapping_add(iz)
}

/// Physical extents and per-node dimensions of a case.
///
/// Mirrors Yan Ren's `geometry` struct. All lengths are in centimetres, the
/// unit the benchmark specifications and the MATLAB both use; `uom` types are
/// deliberately **not** used inside the reference translation, so that the
/// arithmetic stays line-for-line comparable with the original. The
/// `substituted` path is where typed quantities belong.
#[derive(Debug, Clone)]
pub struct Geometry {
    /// The node grid.
    pub grid: Grid,
    /// Total core extent in x \[cm\]. MATLAB `geometry.Xtot`.
    pub x_total: f64,
    /// Total core extent in y \[cm\]. MATLAB `geometry.Ytot`.
    pub y_total: f64,
    /// Total core extent in z \[cm\]. MATLAB `geometry.Ztot`.
    pub z_total: f64,
    /// Node width in x \[cm\], **one entry per spatial node** (length
    /// [`Grid::nodes`]). MATLAB `geometry.Lx`.
    ///
    /// Index with `ix*(ny*nz) + iy*nz + iz`, i.e. [`Grid::index`] with `g = 0`.
    ///
    /// # Not one per axis index
    ///
    /// An earlier version of this doc comment said "one per x index". That was
    /// wrong, and it was caught independently by three separate ports of the
    /// MATLAB — see `neacrpa2.m:43`, `sigmavalupd3d_handler.m:57` and
    /// `driftflux6_solverstatic3d.m:63`, all of which index these per node. The
    /// distinction matters because an axially varying mesh is exactly the case
    /// a per-axis reading would silently get wrong.
    pub lx: Vec<f64>,
    /// Node width in y \[cm\], one entry per spatial node. MATLAB
    /// `geometry.Ly`. Indexed as [`lx`](Self::lx).
    pub ly: Vec<f64>,
    /// Node height in z \[cm\], one entry per spatial node. MATLAB
    /// `geometry.Lz`. Indexed as [`lx`](Self::lx).
    pub lz: Vec<f64>,
    /// Node volume \[cm³\], one per spatial node. MATLAB `geometry.Vi`.
    pub volume: Vec<f64>,
    /// Material index per spatial node, 1-based as in the MATLAB.
    /// MATLAB `geometry.whichsigma`.
    pub which_sigma: Vec<usize>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn iaea3d_grid() -> Grid {
        // The real IAEA-3D shape: note nz = 19, not the 18 requested, because
        // the case constructor appends an axial reflector node.
        Grid::new(17, 17, 19, 2).expect("valid grid")
    }

    #[test]
    fn state_len_matches_the_matlab_field_length() {
        // Octave reports scalar_flux as [10982 x 5] and pwrdens as [10982 x 1]
        // for IAEA-3D. 10982 = 17*17*19*2.
        assert_eq!(iaea3d_grid().state_len(), 10_982);
        assert_eq!(iaea3d_grid().nodes(), 5_491);
    }

    #[test]
    fn index_round_trips_through_unindex() {
        let grid = iaea3d_grid();
        for idx in 0..grid.state_len() {
            let (g, ix, iy, iz) = grid.unindex(idx).expect("in range");
            assert_eq!(grid.index(g, ix, iy, iz), idx, "round trip failed at {idx}");
        }
    }

    #[test]
    fn iz_varies_fastest_then_iy_then_ix_then_group() {
        let grid = iaea3d_grid();
        // Adjacent iz are adjacent in memory.
        assert_eq!(grid.index(0, 0, 0, 1) - grid.index(0, 0, 0, 0), 1);
        // Stepping iy skips a whole z column.
        assert_eq!(grid.index(0, 0, 1, 0) - grid.index(0, 0, 0, 0), grid.nz);
        // Stepping ix skips a whole y-z plane.
        assert_eq!(
            grid.index(0, 1, 0, 0) - grid.index(0, 0, 0, 0),
            grid.ny * grid.nz
        );
        // Stepping group skips the entire spatial mesh.
        assert_eq!(
            grid.index(1, 0, 0, 0) - grid.index(0, 0, 0, 0),
            grid.nodes()
        );
    }

    #[test]
    fn matlab_indices_convert_to_the_same_place() {
        let grid = iaea3d_grid();
        // First entry of the fixtures is g=1, ix=1, iy=1, iz=1 -> flat 0.
        assert_eq!(grid.index_from_matlab(1, 1, 1, 1).expect("valid"), 0);
        // Last entry maps to the final slot.
        assert_eq!(
            grid.index_from_matlab(2, 17, 17, 19).expect("valid"),
            grid.state_len() - 1
        );
    }

    #[test]
    fn matlab_indices_reject_zero_and_overflow() {
        let grid = iaea3d_grid();
        assert!(grid.index_from_matlab(0, 1, 1, 1).is_err());
        assert!(grid.index_from_matlab(1, 18, 1, 1).is_err());
        assert!(grid.index_from_matlab(3, 1, 1, 1).is_err());
    }

    #[test]
    fn empty_dimensions_are_rejected() {
        assert!(Grid::new(0, 17, 19, 2).is_err());
        assert!(Grid::new(17, 17, 19, 0).is_err());
    }
}
