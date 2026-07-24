//! Distributed conjugate-gradient solve over an MPI decomposition (bead op-57m).
//!
//! This is the parallel-Krylov core of MPI scale-out: it solves a symmetric
//! positive-definite linear system `A u = b` across ranks using only two
//! distributed primitives —
//!
//! - a **halo-exchanged matrix-vector product** ([`poisson_matvec`]): each rank
//!   applies the stencil to its slab using ghost values from its neighbours (via
//!   [`super::exchange_halo`]); and
//! - a **globally-reduced inner product** ([`distributed_dot`]): each rank sums
//!   its local contribution, then an [`all_reduce`](outram_park_mpi::Communicator::all_reduce)
//!   forms the global dot product.
//!
//! With those two operations the conjugate-gradient iteration ([`distributed_cg`])
//! is otherwise identical to the serial one, and produces the **same solution
//! regardless of rank count** — the module tests check this against
//! [`serial_cg`] across several partitions.
//!
//! The model operator is the 1-D shifted Poisson / Helmholtz stencil
//! `A u_i = (2 + shift) u_i - u_{i-1} - u_{i+1}` with homogeneous Dirichlet ghosts
//! (`u_{-1} = u_N = 0`). For `shift > 0` it is symmetric positive-definite and
//! well-conditioned, so CG converges — it stands in for the SPD systems a real
//! implicit discretisation produces.
//!
//! # Scope / human-review flags
//!
//! **Verification-only, untrusted AI draft.** This demonstrates a *distributed
//! linear solve* (parallel CG); wiring it as the linear stage of pflotran's actual
//! Newton solve on the real Jacobian, preconditioning, and 2-D/3-D (Cartesian) or
//! unstructured partitioning remain follow-ups under op-57m.

use outram_park_mpi::{Communicator, MpiResult, ReduceOp};

use super::Decomposition1D;

/// Apply the 1-D shifted-Poisson operator `A = (2+shift) I - L` to the distributed
/// vector `u` (this rank's slab), returning `A u` for the local cells.
///
/// Ghost values come from a halo exchange; at a physical domain end the ghost is
/// `0` (homogeneous Dirichlet). `shift >= 0`; `shift > 0` makes `A` SPD.
///
/// # Errors
/// Propagates any halo-exchange transport error.
pub fn poisson_matvec(
    comm: &Communicator,
    decomp: &Decomposition1D,
    u: &[f64],
    shift: f64,
) -> MpiResult<Vec<f64>> {
    let halo = super::exchange_halo(comm, decomp, u)?;
    let n = u.len();
    let mut out = vec![0.0; n];
    for i in 0..n {
        let west = if i > 0 { u[i - 1] } else { halo.left.unwrap_or(0.0) };
        let east = if i + 1 < n { u[i + 1] } else { halo.right.unwrap_or(0.0) };
        out[i] = (2.0 + shift) * u[i] - west - east;
    }
    Ok(out)
}

/// Global inner product of two distributed vectors: local dot, then an
/// `all_reduce(Sum)` so every rank gets the same global value.
///
/// # Errors
/// Propagates any collective transport error.
pub fn distributed_dot(comm: &Communicator, a: &[f64], b: &[f64]) -> MpiResult<f64> {
    let local: f64 = a.iter().zip(b).map(|(x, y)| x * y).sum();
    let global = comm.all_reduce(&[local], ReduceOp::Sum)?;
    Ok(global[0])
}

/// Solve `A u = b` for the shifted-Poisson operator by **distributed conjugate
/// gradient**, starting from `u = 0`. Returns this rank's solution slab and the
/// iteration count.
///
/// Convergence is on the global residual 2-norm (`sqrt(r·r) < tol`). Every rank
/// must call this collectively with matching `shift`/`tol`/`max_iter`.
///
/// # Errors
/// Propagates any transport error from the matvec or the reduced dot products.
pub fn distributed_cg(
    comm: &Communicator,
    decomp: &Decomposition1D,
    b: &[f64],
    shift: f64,
    tol: f64,
    max_iter: usize,
) -> MpiResult<(Vec<f64>, usize)> {
    let n = b.len();
    let mut x = vec![0.0; n];
    // r = b - A x = b (x = 0).
    let mut r = b.to_vec();
    let mut p = r.clone();
    let mut rs_old = distributed_dot(comm, &r, &r)?;

    let mut iters = 0;
    if rs_old.sqrt() < tol {
        return Ok((x, iters));
    }
    for k in 0..max_iter {
        iters = k + 1;
        let ap = poisson_matvec(comm, decomp, &p, shift)?;
        let p_dot_ap = distributed_dot(comm, &p, &ap)?;
        let alpha = rs_old / p_dot_ap;
        for i in 0..n {
            x[i] += alpha * p[i];
            r[i] -= alpha * ap[i];
        }
        let rs_new = distributed_dot(comm, &r, &r)?;
        if rs_new.sqrt() < tol {
            break;
        }
        let beta = rs_new / rs_old;
        for i in 0..n {
            p[i] = r[i] + beta * p[i];
        }
        rs_old = rs_new;
    }
    Ok((x, iters))
}

/// Serial reference: the same shifted-Poisson CG solved on one rank over the whole
/// `n_global`-cell system, the correctness oracle for [`distributed_cg`].
pub fn serial_cg(
    n_global: usize,
    b: &[f64],
    shift: f64,
    tol: f64,
    max_iter: usize,
) -> (Vec<f64>, usize) {
    let matvec = |u: &[f64]| -> Vec<f64> {
        let mut out = vec![0.0; n_global];
        for i in 0..n_global {
            let west = if i > 0 { u[i - 1] } else { 0.0 };
            let east = if i + 1 < n_global { u[i + 1] } else { 0.0 };
            out[i] = (2.0 + shift) * u[i] - west - east;
        }
        out
    };
    let dot = |a: &[f64], c: &[f64]| -> f64 { a.iter().zip(c).map(|(x, y)| x * y).sum() };

    let mut x = vec![0.0; n_global];
    let mut r = b.to_vec();
    let mut p = r.clone();
    let mut rs_old = dot(&r, &r);
    let mut iters = 0;
    if rs_old.sqrt() < tol {
        return (x, 0);
    }
    for k in 0..max_iter {
        iters = k + 1;
        let ap = matvec(&p);
        let alpha = rs_old / dot(&p, &ap);
        for i in 0..n_global {
            x[i] += alpha * p[i];
            r[i] -= alpha * ap[i];
        }
        let rs_new = dot(&r, &r);
        if rs_new.sqrt() < tol {
            break;
        }
        let beta = rs_new / rs_old;
        for i in 0..n_global {
            p[i] = r[i] + beta * p[i];
        }
        rs_old = rs_new;
    }
    (x, iters)
}

#[cfg(test)]
mod tests {
    use super::*;
    use outram_park_mpi::run;

    #[test]
    fn distributed_cg_matches_serial_across_rank_counts() {
        let n = 48;
        let shift = 0.1_f64;
        let tol = 1e-10;
        // A smooth right-hand side.
        let b: Vec<f64> = (0..n).map(|i| ((i as f64) * 0.3).sin() + 1.0).collect();
        let (reference, _ref_iters) = serial_cg(n, &b, shift, tol, 1000);

        for p in [1, 2, 3, 4, 6, 8] {
            let b_ref = b.clone();
            let reference = reference.clone();
            let ok = run(p, move |c| {
                let d = Decomposition1D::new(n, c);
                let b_local = b_ref[d.start..d.start + d.local_len].to_vec();
                let (x_local, _) =
                    distributed_cg(c, &d, &b_local, shift, tol, 1000).unwrap();
                let expected = &reference[d.start..d.start + d.local_len];
                x_local
                    .iter()
                    .zip(expected)
                    .all(|(a, e)| (a - e).abs() < 1e-8)
            })
            .unwrap();
            assert!(ok.iter().all(|&b| b), "distributed CG != serial for p={p}");
        }
    }

    #[test]
    fn distributed_cg_actually_solves_the_system() {
        // Verify the residual b - A x is ~0 for the distributed solution (p=4).
        let n = 32;
        let shift = 0.5_f64;
        let b: Vec<f64> = (0..n).map(|i| (i % 5) as f64).collect();
        let residuals = run(4, move |c| {
            let d = Decomposition1D::new(n, c);
            let b_local = b[d.start..d.start + d.local_len].to_vec();
            let (x, _) = distributed_cg(c, &d, &b_local, shift, 1e-12, 1000).unwrap();
            let ax = poisson_matvec(c, &d, &x, shift).unwrap();
            // local max |b - A x|
            b_local
                .iter()
                .zip(&ax)
                .map(|(bi, axi)| (bi - axi).abs())
                .fold(0.0_f64, f64::max)
        })
        .unwrap();
        for res in residuals {
            assert!(res < 1e-8, "distributed CG left residual {res:e}");
        }
    }

    #[test]
    fn distributed_dot_matches_serial() {
        let n = 20;
        let a: Vec<f64> = (0..n).map(|i| i as f64).collect();
        let b: Vec<f64> = (0..n).map(|i| (n - i) as f64).collect();
        let serial: f64 = a.iter().zip(&b).map(|(x, y)| x * y).sum();
        let got = run(3, move |c| {
            let d = Decomposition1D::new(n, c);
            let al = a[d.start..d.start + d.local_len].to_vec();
            let bl = b[d.start..d.start + d.local_len].to_vec();
            distributed_dot(c, &al, &bl).unwrap()
        })
        .unwrap();
        // Every rank gets the same global dot, equal to the serial value.
        for g in got {
            assert!((g - serial).abs() < 1e-9);
        }
    }
}
