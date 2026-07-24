//! MPI-style domain decomposition and halo exchange (bead op-v6s.15.9).
//!
//! PFLOTRAN scales out by partitioning the grid across MPI ranks and exchanging a
//! one-cell **halo** (ghost layer) between neighbouring subdomains each iteration,
//! so every rank can evaluate its stencil using up-to-date neighbour values. This
//! module provides that pattern for a **structured 1-D chain of cells**, driven by
//! the pure-Rust [`outram_park_mpi`] transport — the first slice of distributed
//! scale-out.
//!
//! # What is here
//!
//! - [`Decomposition1D`] — a balanced contiguous partition of `n_global` cells
//!   across the ranks of a communicator: each rank's cell range and its left/right
//!   neighbour ranks.
//! - [`exchange_halo`] — one nearest-neighbour halo swap: send each subdomain's
//!   edge cell to its neighbour and receive the neighbour's edge into a ghost slot.
//! - [`jacobi_smooth_distributed`] — a worked example: distributed Jacobi
//!   relaxation of the 1-D Laplace problem with Dirichlet ends, which reproduces
//!   the serial result exactly (see the module tests). This stands in for the real
//!   stencil kernels (diffusion, conduction, the transport operator) that the halo
//!   exchange will feed.
//!
//! # Scope / human-review flags
//!
//! **Verification-only, untrusted AI draft** (workspace `RESPONSIBLE_USE.md`).
//! This is the halo-exchange foundation, **not** a fully MPI-parallel Newton
//! solve: the implicit RICHARDS/transport Jacobian is still assembled and solved
//! serially per rank. Decomposing the global linear solve (distributed
//! matrix-vector products + a parallel Krylov method) is a follow-up. The
//! partition is 1-D (structured slabs); unstructured / multi-dimensional
//! partitioning is future work.

pub mod krylov;

use outram_park_mpi::{Communicator, MpiResult};

/// A balanced contiguous 1-D partition of `n_global` cells over a communicator.
///
/// Rank `r` of `p` owns a contiguous block of cells; blocks differ in length by at
/// most one when `p` does not divide `n_global`. Each rank knows its global cell
/// offset, its local length, and the ranks of its left/right neighbours (or
/// `None` at a physical domain end).
#[derive(Debug, Clone)]
pub struct Decomposition1D {
    /// Total number of cells in the global domain.
    pub n_global: usize,
    /// Number of ranks the domain is split over.
    pub n_ranks: i32,
    /// This rank's id in the communicator.
    pub rank: i32,
    /// Global index of this rank's first (leftmost) owned cell.
    pub start: usize,
    /// Number of cells this rank owns.
    pub local_len: usize,
    /// Left-neighbour rank, or `None` if this subdomain touches the low domain end.
    pub left: Option<i32>,
    /// Right-neighbour rank, or `None` if this subdomain touches the high domain end.
    pub right: Option<i32>,
}

impl Decomposition1D {
    /// Build the partition for this rank from the global cell count and its
    /// communicator. Uses the standard balanced split: the first `n_global % p`
    /// ranks get one extra cell.
    pub fn new(n_global: usize, comm: &Communicator) -> Self {
        let p = comm.size();
        let r = comm.rank();
        let base = n_global / p as usize;
        let rem = n_global % p as usize;
        let r_us = r as usize;
        // Ranks [0, rem) get base+1 cells; the rest get base.
        let local_len = base + if r_us < rem { 1 } else { 0 };
        let start = r_us * base + r_us.min(rem);
        let left = if r > 0 { Some(r - 1) } else { None };
        let right = if r < p - 1 { Some(r + 1) } else { None };
        Decomposition1D {
            n_global,
            n_ranks: p,
            rank: r,
            start,
            local_len,
            left,
            right,
        }
    }

    /// Global cell index of this rank's `local`-th owned cell.
    pub fn global_index(&self, local: usize) -> usize {
        self.start + local
    }
}

/// The ghost values a rank receives from its neighbours in a halo exchange.
///
/// `left`/`right` are the neighbour's adjacent edge cell value, or `None` when
/// this subdomain touches the corresponding physical domain end (where a boundary
/// condition, not a neighbour value, applies).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Halo {
    /// Value of the left neighbour's rightmost cell (`None` at the low domain end).
    pub left: Option<f64>,
    /// Value of the right neighbour's leftmost cell (`None` at the high domain end).
    pub right: Option<f64>,
}

/// Tag namespaces for the two halo directions (kept distinct so a left-going and a
/// right-going message can never be confused).
const TAG_TO_LEFT: i32 = 101;
const TAG_TO_RIGHT: i32 = 102;

/// Perform one nearest-neighbour halo exchange for the subdomain field `local`.
///
/// Each rank sends its leftmost owned cell to its left neighbour and its rightmost
/// to its right neighbour, and receives the neighbours' adjacent edge cells into a
/// [`Halo`]. Sends are posted before receives; because the transport buffers
/// eagerly, a full neighbour chain exchanges without deadlock.
///
/// # Errors
/// Propagates any [`outram_park_mpi`] transport error.
///
/// # Panics
/// Does not panic; an empty `local` slab is only valid when the rank has no
/// neighbours (a degenerate single-rank-with-zero-cells case), handled by sending
/// nothing.
pub fn exchange_halo(comm: &Communicator, decomp: &Decomposition1D, local: &[f64]) -> MpiResult<Halo> {
    // Post sends first (buffered), then receive from each neighbour.
    if let Some(l) = decomp.left {
        let first = *local.first().expect("subdomain with a left neighbour is non-empty");
        comm.send(&[first], l, TAG_TO_LEFT)?;
    }
    if let Some(r) = decomp.right {
        let last = *local.last().expect("subdomain with a right neighbour is non-empty");
        comm.send(&[last], r, TAG_TO_RIGHT)?;
    }
    // My left neighbour sent its rightmost cell to its right (me) with TAG_TO_RIGHT.
    let left = match decomp.left {
        Some(l) => Some(comm.recv::<f64>(l, TAG_TO_RIGHT)?.0[0]),
        None => None,
    };
    // My right neighbour sent its leftmost cell to its left (me) with TAG_TO_LEFT.
    let right = match decomp.right {
        Some(r) => Some(comm.recv::<f64>(r, TAG_TO_LEFT)?.0[0]),
        None => None,
    };
    Ok(Halo { left, right })
}

/// Distributed Jacobi relaxation of the 1-D Laplace problem `u'' = 0` on
/// `[0, n_global)` with Dirichlet ends `u[0] = left_bc`, `u[n_global-1] = right_bc`,
/// run for `iterations` sweeps over a [`Decomposition1D`].
///
/// Each sweep: exchange the halo, then set every interior cell to the average of
/// its two neighbours (using ghost values at subdomain edges and the fixed BC at
/// the true domain ends). Returns this rank's converged local slab. Because Jacobi
/// is order-independent within a sweep, the distributed result is **bit-identical**
/// to the serial computation ([`jacobi_smooth_serial`]) — the module tests assert
/// this across several rank counts.
///
/// # Errors
/// Propagates any halo-exchange transport error.
pub fn jacobi_smooth_distributed(
    comm: &Communicator,
    decomp: &Decomposition1D,
    left_bc: f64,
    right_bc: f64,
    iterations: usize,
) -> MpiResult<Vec<f64>> {
    let n = decomp.local_len;
    // Initialise: BC at the global ends, 0 elsewhere.
    let mut u = vec![0.0_f64; n];
    if decomp.left.is_none() && n > 0 {
        u[0] = left_bc;
    }
    if decomp.right.is_none() && n > 0 {
        u[n - 1] = right_bc;
    }

    for _ in 0..iterations {
        let halo = exchange_halo(comm, decomp, &u)?;
        let mut next = u.clone();
        for i in 0..n {
            let global = decomp.global_index(i);
            // Fixed Dirichlet cells never change.
            if global == 0 || global == decomp.n_global - 1 {
                continue;
            }
            // Left neighbour value: interior cell, else the halo ghost.
            let west = if i > 0 { u[i - 1] } else { halo.left.expect("interior edge has a left ghost") };
            let east = if i + 1 < n { u[i + 1] } else { halo.right.expect("interior edge has a right ghost") };
            next[i] = 0.5 * (west + east);
        }
        u = next;
    }
    Ok(u)
}

/// The serial reference for [`jacobi_smooth_distributed`]: the same Jacobi sweeps
/// over the whole `n_global`-cell array on one rank, used as the correctness oracle.
pub fn jacobi_smooth_serial(
    n_global: usize,
    left_bc: f64,
    right_bc: f64,
    iterations: usize,
) -> Vec<f64> {
    let mut u = vec![0.0_f64; n_global];
    if n_global > 0 {
        u[0] = left_bc;
        u[n_global - 1] = right_bc;
    }
    for _ in 0..iterations {
        let mut next = u.clone();
        for i in 1..n_global.saturating_sub(1) {
            next[i] = 0.5 * (u[i - 1] + u[i + 1]);
        }
        u = next;
    }
    u
}

#[cfg(test)]
mod tests {
    use super::*;
    use outram_park_mpi::run;

    #[test]
    fn partition_is_balanced_and_covers_the_domain() {
        // 10 cells over 3 ranks -> lengths 4,3,3; contiguous, no gaps/overlaps.
        let cover = run(3, |c| {
            let d = Decomposition1D::new(10, c);
            (d.rank, d.start, d.local_len, d.left, d.right)
        })
        .unwrap();
        assert_eq!(cover[0], (0, 0, 4, None, Some(1)));
        assert_eq!(cover[1], (1, 4, 3, Some(0), Some(2)));
        assert_eq!(cover[2], (2, 7, 3, Some(1), None));
        // Contiguous tiling: each start == previous start + previous len.
        assert_eq!(cover[1].1, cover[0].1 + cover[0].2);
        assert_eq!(cover[2].1, cover[1].1 + cover[1].2);
        assert_eq!(cover[2].1 + cover[2].2, 10);
    }

    #[test]
    fn halo_exchange_moves_edge_values_to_neighbours() {
        // Each rank fills its slab with its rank id; after exchange, the left ghost
        // is the left neighbour's id and the right ghost the right neighbour's id.
        let halos = run(4, |c| {
            let d = Decomposition1D::new(8, c); // 2 cells each
            let local = vec![c.rank() as f64; d.local_len];
            exchange_halo(c, &d, &local).unwrap()
        })
        .unwrap();
        assert_eq!(halos[0], Halo { left: None, right: Some(1.0) });
        assert_eq!(halos[1], Halo { left: Some(0.0), right: Some(2.0) });
        assert_eq!(halos[2], Halo { left: Some(1.0), right: Some(3.0) });
        assert_eq!(halos[3], Halo { left: Some(2.0), right: None });
    }

    #[test]
    fn distributed_jacobi_matches_serial_across_rank_counts() {
        let n = 24;
        let (left_bc, right_bc, iters) = (100.0_f64, 10.0_f64, 200);
        let reference = jacobi_smooth_serial(n, left_bc, right_bc, iters);

        for p in [1, 2, 3, 4, 6] {
            // Each rank checks its own slab against the serial reference slice, so
            // the test needs no equal-count gather and works for uneven splits.
            let ok = run(p, |c| {
                let d = Decomposition1D::new(n, c);
                let local = jacobi_smooth_distributed(c, &d, left_bc, right_bc, iters).unwrap();
                let expected = &reference[d.start..d.start + d.local_len];
                local
                    .iter()
                    .zip(expected)
                    .all(|(a, b)| (a - b).abs() < 1e-12)
            })
            .unwrap();
            assert!(ok.iter().all(|&b| b), "distributed != serial for p={p}");
        }
    }

    #[test]
    fn jacobi_converges_toward_the_linear_profile() {
        // The exact steady solution of u''=0 with u(0)=0, u(N-1)=(N-1) is u(i)=i.
        // Enough sweeps should approach it closely at the midpoint.
        let n = 9;
        let u = jacobi_smooth_serial(n, 0.0, (n - 1) as f64, 5000);
        assert!((u[(n - 1) / 2] - ((n - 1) / 2) as f64).abs() < 1e-6);
    }
}
