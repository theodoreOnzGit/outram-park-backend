//! Cartesian process topologies.
//!
//! A Cartesian topology maps the ranks of a communicator onto an N-dimensional
//! grid so a stencil code can find neighbours by coordinate rather than by hand.
//! [`Communicator::cart_create`] (mirrors `MPI_Cart_create`) builds a
//! [`CartesianComm`] over the first `dims.product()` ranks; on it,
//! [`CartesianComm::coords`] / [`CartesianComm::rank`] convert between a rank and
//! its grid coordinates (row-major: the last dimension varies fastest, as in MPI),
//! and [`CartesianComm::shift`] (mirrors `MPI_Cart_shift`) returns the
//! `(source, dest)` ranks for a shift along one dimension — exactly the pair a
//! halo exchange needs.
//!
//! Dimensions may be **periodic** (wrap-around, e.g. a torus) or not; at a
//! non-periodic edge a shift's off-grid neighbour is `None` (MPI's
//! `MPI_PROC_NULL`).
//!
//! # Provenance
//!
//! MPI-3.1 Cartesian-topology semantics (`MPI_Cart_create`/`_coords`/`_rank`/
//! `_shift`), row-major coordinate ordering. Reordering for locality
//! (`MPI_Cart_create`'s `reorder` flag) is not performed — ranks keep their
//! identity. Untrusted AI draft, verification-only.

use crate::communicator::Communicator;
use crate::error::{MpiError, MpiResult};

/// A communicator with an attached N-dimensional Cartesian grid topology.
///
/// Wraps the underlying [`Communicator`] (the first `dims.product()` ranks of the
/// parent) plus the grid `dims` and per-dimension `periods` (wrap-around) flags.
#[derive(Clone)]
pub struct CartesianComm {
    comm: Communicator,
    dims: Vec<i32>,
    periods: Vec<bool>,
}

impl Communicator {
    /// Create a Cartesian topology of shape `dims` (with per-dimension `periods`
    /// wrap-around flags) over the first `dims.product()` ranks of this
    /// communicator (mirrors `MPI_Cart_create` with `reorder = false`).
    ///
    /// Ranks `0..dims.product()` become members and receive `Some(CartesianComm)`;
    /// any surplus ranks receive `Ok(None)` (MPI's `MPI_COMM_NULL`). Collective:
    /// every rank of this communicator must call it with identical `dims`/`periods`.
    ///
    /// # Errors
    /// [`MpiError::InvalidArgument`] if `dims` and `periods` differ in length, a
    /// dimension is `<= 0`, or the grid needs more ranks than the communicator has.
    pub fn cart_create(
        &self,
        dims: &[i32],
        periods: &[bool],
    ) -> MpiResult<Option<CartesianComm>> {
        if dims.len() != periods.len() {
            return Err(MpiError::InvalidArgument(format!(
                "cart_create: dims (len {}) and periods (len {}) must match",
                dims.len(),
                periods.len()
            )));
        }
        if dims.iter().any(|&d| d <= 0) {
            return Err(MpiError::InvalidArgument(
                "cart_create: every dimension must be > 0".into(),
            ));
        }
        let needed: i64 = dims.iter().map(|&d| d as i64).product();
        if needed > self.size() as i64 {
            return Err(MpiError::InvalidArgument(format!(
                "cart_create: grid needs {needed} ranks but the communicator has {}",
                self.size()
            )));
        }
        // Members are ranks [0, needed); split them off (keyed by rank so the new
        // ranks match the old order, i.e. the Cartesian rank == parent rank).
        let color = if (self.rank() as i64) < needed {
            0
        } else {
            Communicator::UNDEFINED
        };
        let sub = self.split(color, self.rank())?;
        Ok(sub.map(|comm| CartesianComm {
            comm,
            dims: dims.to_vec(),
            periods: periods.to_vec(),
        }))
    }
}

impl CartesianComm {
    /// The underlying communicator (for point-to-point and collectives).
    pub fn comm(&self) -> &Communicator {
        &self.comm
    }

    /// The grid dimensions.
    pub fn dims(&self) -> &[i32] {
        &self.dims
    }

    /// The per-dimension periodicity (wrap-around) flags.
    pub fn periods(&self) -> &[bool] {
        &self.periods
    }

    /// This rank's own grid coordinates.
    pub fn my_coords(&self) -> Vec<i32> {
        self.coords(self.comm.rank())
    }

    /// Grid coordinates of `rank` (row-major: the last dimension varies fastest),
    /// mirroring `MPI_Cart_coords`.
    pub fn coords(&self, rank: i32) -> Vec<i32> {
        let mut rem = rank;
        let mut coords = vec![0; self.dims.len()];
        for i in (0..self.dims.len()).rev() {
            coords[i] = rem % self.dims[i];
            rem /= self.dims[i];
        }
        coords
    }

    /// Rank at the given grid `coords`, or `None` if any coordinate is out of range
    /// on a non-periodic dimension (a periodic coordinate wraps), mirroring
    /// `MPI_Cart_rank`.
    pub fn rank(&self, coords: &[i32]) -> Option<i32> {
        if coords.len() != self.dims.len() {
            return None;
        }
        let mut r = 0;
        for i in 0..self.dims.len() {
            let d = self.dims[i];
            let c = if self.periods[i] {
                coords[i].rem_euclid(d)
            } else if coords[i] < 0 || coords[i] >= d {
                return None;
            } else {
                coords[i]
            };
            r = r * d + c;
        }
        Some(r)
    }

    /// The `(source, dest)` ranks for a shift of `disp` cells along dimension
    /// `direction` (mirrors `MPI_Cart_shift`).
    ///
    /// `dest` is the neighbour `disp` steps *up* this dimension (the rank this one
    /// sends to); `source` is `disp` steps *down* (the rank it receives from).
    /// Either is `None` at a non-periodic edge (MPI's `MPI_PROC_NULL`).
    ///
    /// # Panics
    /// Does not panic; an out-of-range `direction` yields `(None, None)`.
    pub fn shift(&self, direction: usize, disp: i32) -> (Option<i32>, Option<i32>) {
        if direction >= self.dims.len() {
            return (None, None);
        }
        let coords = self.my_coords();
        let mut up = coords.clone();
        up[direction] += disp;
        let mut down = coords;
        down[direction] -= disp;
        (self.rank(&down), self.rank(&up))
    }
}
