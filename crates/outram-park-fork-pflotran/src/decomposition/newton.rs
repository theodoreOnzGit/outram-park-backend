//! Distributed Newton solver for nonlinear systems (bead op-gj5).
//!
//! The RICHARDS / GENERAL flow modes are **nonlinear** (van Genuchten curves,
//! upstream-weighted mobility, a pressure-dependent EOS), so distributing them
//! needs more than a single linear solve: a Newton loop whose residual norm is a
//! global reduction and whose Jacobian is solved distributed each iteration. This
//! module provides that outer loop, [`distributed_newton`], built on the
//! distributed BiCGStab and `all_reduce` primitives.
//!
//! Given a residual closure `F(x)` (this rank's owned-cell residual, doing its own
//! halo exchange) and a Jacobian-rows closure returning the local tridiagonal
//! `(diag, west, east)`, each iteration:
//!
//! 1. evaluate `F(x)` and its global 2-norm (`sqrt(all_reduce(F·F))`);
//! 2. assemble the distributed Jacobian ([`super::ldu::DistributedLduMatrix1D::from_rows`]);
//! 3. solve `J·δx = −F` with distributed BiCGStab;
//! 4. update `x ← x + δx`.
//!
//! The module test drives a genuinely nonlinear 1-D reaction–diffusion problem and
//! confirms the distributed Newton reproduces a serial Newton solve cell-for-cell.
//!
//! # Scope / human-review flags
//!
//! Verification-only, untrusted AI draft. This is the distributed nonlinear-solve
//! *framework*; wiring pflotran's exact RICHARDS residual (van Genuchten
//! saturation/rel-perm + EOS + upstream mobility + gravity) and Jacobian into it —
//! the last op-gj5 step — requires faithfully replicating those curves per rank
//! and is a follow-up. Plain Newton (no line search / damping); 1-D tridiagonal.

use outram_park_mpi::{Communicator, MpiError, MpiResult};

use super::krylov::distributed_dot;
use super::ldu::DistributedLduMatrix1D;
use super::Decomposition1D;

/// Solve a nonlinear system `F(x) = 0` by distributed Newton, starting from `x0`.
///
/// - `residual(x) -> F` returns this rank's owned-cell residual slab.
/// - `jac_rows(x) -> (diag, west, east)` returns this rank's local tridiagonal
///   Jacobian rows (`west[i]` = ∂F_i/∂x_{i-1}, `east[i]` = ∂F_i/∂x_{i+1}), each
///   length `decomp.local_len`, with `0` for off-domain couplings.
///
/// Converges when the global residual 2-norm falls below `abs_tol`. Returns the
/// solution slab and the Newton iteration count.
///
/// # Errors
/// Propagates any transport error from the closures or the distributed linear
/// solve; [`MpiError::InvalidArgument`] if a closure returns the wrong length.
#[allow(clippy::too_many_arguments)]
pub fn distributed_newton<R, J>(
    comm: &Communicator,
    decomp: &Decomposition1D,
    mut x: Vec<f64>,
    residual: R,
    jac_rows: J,
    abs_tol: f64,
    max_newton: usize,
    lin_tol: f64,
    lin_max: usize,
) -> MpiResult<(Vec<f64>, usize)>
where
    R: Fn(&[f64]) -> MpiResult<Vec<f64>>,
    J: Fn(&[f64]) -> MpiResult<(Vec<f64>, Vec<f64>, Vec<f64>)>,
{
    let l = decomp.local_len;
    if x.len() != l {
        return Err(MpiError::InvalidArgument(format!(
            "distributed_newton: x0 length {} != local_len {l}",
            x.len()
        )));
    }
    let mut iters = 0;
    for k in 0..max_newton {
        iters = k + 1;
        let f = residual(&x)?;
        let fnorm = distributed_dot(comm, &f, &f)?.sqrt();
        if fnorm < abs_tol {
            return Ok((x, k)); // converged (k iterations completed before this check)
        }
        let (diag, west, east) = jac_rows(&x)?;
        let jac = DistributedLduMatrix1D::from_rows(decomp, diag, west, east)
            .map_err(|e| MpiError::Transport(format!("newton jacobian: {e}")))?;
        let neg_f: Vec<f64> = f.iter().map(|v| -v).collect();
        let (dx, _) = jac.solve_bicgstab(comm, &neg_f, lin_tol, lin_max)?;
        for i in 0..l {
            x[i] += dx[i];
        }
    }
    // Final convergence check after the last update.
    let f = residual(&x)?;
    if distributed_dot(comm, &f, &f)?.sqrt() < abs_tol {
        return Ok((x, iters));
    }
    Ok((x, iters))
}

#[cfg(test)]
mod tests {
    use super::*;
    use outram_park_mpi::run;

    // A nonlinear 1-D reaction-diffusion residual with homogeneous Dirichlet ends:
    //   F_i(u) = shift*u_i + u_i^3 + D*( (u_i - u_west) + (u_i - u_east) ) - b_i
    // (u_west/u_east are the neighbour values, 0 at a domain end). The cubic term
    // makes it genuinely nonlinear; the Jacobian is tridiagonal with
    //   diag_i = shift + 3 u_i^2 + D*(deg_i),  west_i = -D, east_i = -D.
    const SHIFT: f64 = 0.5;
    const D: f64 = 1.0;

    fn distributed_residual(
        comm: &Communicator,
        decomp: &Decomposition1D,
        u: &[f64],
        b: &[f64],
    ) -> MpiResult<Vec<f64>> {
        let halo = super::super::exchange_halo(comm, decomp, u)?;
        let l = u.len();
        let mut f = vec![0.0; l];
        for i in 0..l {
            let gi = decomp.global_index(i);
            let has_w = gi > 0;
            let has_e = gi + 1 < decomp.n_global;
            let uw = if i > 0 {
                u[i - 1]
            } else {
                halo.left.unwrap_or(0.0)
            };
            let ue = if i + 1 < l {
                u[i + 1]
            } else {
                halo.right.unwrap_or(0.0)
            };
            let mut fi = SHIFT * u[i] + u[i] * u[i] * u[i] - b[i];
            if has_w {
                fi += D * (u[i] - uw);
            }
            if has_e {
                fi += D * (u[i] - ue);
            }
            f[i] = fi;
        }
        Ok(f)
    }

    fn distributed_jac(decomp: &Decomposition1D, u: &[f64]) -> (Vec<f64>, Vec<f64>, Vec<f64>) {
        let l = u.len();
        let (mut diag, mut west, mut east) = (vec![0.0; l], vec![0.0; l], vec![0.0; l]);
        for i in 0..l {
            let gi = decomp.global_index(i);
            let has_w = gi > 0;
            let has_e = gi + 1 < decomp.n_global;
            let mut dg = SHIFT + 3.0 * u[i] * u[i];
            if has_w {
                dg += D;
                west[i] = -D;
            }
            if has_e {
                dg += D;
                east[i] = -D;
            }
            diag[i] = dg;
        }
        (diag, west, east)
    }

    /// Serial Newton on the whole system, the correctness oracle.
    fn serial_newton(n: usize, b: &[f64], tol: f64, max_it: usize) -> Vec<f64> {
        let mut u = vec![0.0; n];
        for _ in 0..max_it {
            // residual
            let mut f = vec![0.0; n];
            for i in 0..n {
                let uw = if i > 0 { u[i - 1] } else { 0.0 };
                let ue = if i + 1 < n { u[i + 1] } else { 0.0 };
                let mut fi = SHIFT * u[i] + u[i] * u[i] * u[i] - b[i];
                if i > 0 {
                    fi += D * (u[i] - uw);
                }
                if i + 1 < n {
                    fi += D * (u[i] - ue);
                }
                f[i] = fi;
            }
            let fnorm = f.iter().map(|v| v * v).sum::<f64>().sqrt();
            if fnorm < tol {
                break;
            }
            // tridiagonal Jacobian
            let mut diag = vec![0.0; n];
            let mut off = vec![0.0; n]; // -D on both sub/super
            for i in 0..n {
                let mut dg = SHIFT + 3.0 * u[i] * u[i];
                if i > 0 {
                    dg += D;
                }
                if i + 1 < n {
                    dg += D;
                }
                diag[i] = dg;
                off[i] = -D;
            }
            // Thomas algorithm solve J dx = -f (constant -D off-diagonals).
            let mut cp = vec![0.0; n];
            let mut dp = vec![0.0; n];
            cp[0] = if n > 1 { off[0] / diag[0] } else { 0.0 };
            dp[0] = -f[0] / diag[0];
            for i in 1..n {
                let m = diag[i] - off[i] * cp[i - 1];
                cp[i] = if i + 1 < n { off[i] / m } else { 0.0 };
                dp[i] = (-f[i] - off[i] * dp[i - 1]) / m;
            }
            let mut dx = vec![0.0; n];
            dx[n - 1] = dp[n - 1];
            for i in (0..n - 1).rev() {
                dx[i] = dp[i] - cp[i] * dx[i + 1];
            }
            for i in 0..n {
                u[i] += dx[i];
            }
        }
        u
    }

    #[test]
    fn distributed_newton_matches_serial_on_nonlinear_diffusion() {
        let n = 40;
        let tol = 1e-10;
        let b: Vec<f64> = (0..n).map(|i| 1.0 + ((i as f64) * 0.2).sin()).collect();
        let reference = serial_newton(n, &b, tol, 100);

        for p in [1, 2, 3, 4, 5] {
            let b = b.clone();
            let reference = reference.clone();
            let ok = run(p, move |comm| {
                let dd = Decomposition1D::new(n, comm);
                let b_local = b[dd.start..dd.start + dd.local_len].to_vec();
                let (u, _iters) = distributed_newton(
                    comm,
                    &dd,
                    vec![0.0; dd.local_len],
                    |x| distributed_residual(comm, &dd, x, &b_local),
                    |x| Ok(distributed_jac(&dd, x)),
                    tol,
                    100,
                    1e-12,
                    5000,
                )
                .unwrap();
                let expected = &reference[dd.start..dd.start + dd.local_len];
                u.iter().zip(expected).all(|(a, e)| (a - e).abs() < 1e-7)
            })
            .unwrap();
            assert!(
                ok.iter().all(|&b| b),
                "distributed Newton != serial for p={p}"
            );
        }
    }

    #[test]
    fn distributed_newton_drives_residual_to_zero() {
        let n = 24;
        let b: Vec<f64> = (0..n).map(|i| (i % 3) as f64 + 0.5).collect();
        let res = run(3, move |comm| {
            let dd = Decomposition1D::new(n, comm);
            let b_local = b[dd.start..dd.start + dd.local_len].to_vec();
            let (u, _) = distributed_newton(
                comm,
                &dd,
                vec![0.0; dd.local_len],
                |x| distributed_residual(comm, &dd, x, &b_local),
                |x| Ok(distributed_jac(&dd, x)),
                1e-12,
                100,
                1e-13,
                5000,
            )
            .unwrap();
            let f = distributed_residual(comm, &dd, &u, &b_local).unwrap();
            distributed_dot(comm, &f, &f).unwrap().sqrt()
        })
        .unwrap();
        assert!(res[0] < 1e-9, "distributed Newton residual {}", res[0]);
    }
}
