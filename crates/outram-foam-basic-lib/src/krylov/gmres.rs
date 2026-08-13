// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
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

//! Restarted, right-preconditioned GMRES — GMRES(m).
//!
//! The generalised minimal residual method (Saad & Schultz, 1986) for
//! **nonsymmetric** `A x = b`. Each outer cycle builds an `m`-dimensional Krylov
//! subspace by Arnoldi (modified Gram-Schmidt) and minimises the residual 2-norm
//! over that subspace via incremental Givens rotations on the Hessenberg factor;
//! the cycle then **restarts** from the current iterate to bound storage at `m`
//! basis vectors. Total inner iterations are capped at `settings.max_iter`.
//!
//! # Right preconditioning
//!
//! This solver uses **right** preconditioning: it applies GMRES to
//! `A M^{-1} u = b` with `x = M^{-1} u`. The chief practical advantage is that the
//! Krylov residual `b − A M^{-1} u_k` *equals* the true residual `b − A x_k`, so
//! the cheap Givens-rotation residual estimate tracks the genuine residual and the
//! stopping test `||r||₂ / ||b||₂ ≤ tol` needs no correction. The preconditioner
//! is applied once per basis vector inside Arnoldi and once to the accumulated
//! correction at the end of each cycle. All vectors are dimensionless `f64` of
//! length `n_cells`.
//!
//! # Execution backend
//!
//! There is **one** implementation, [`gmres_prepared`], with the backend as a
//! parameter; [`gmres`] is the convenience adapter for a caller holding a bare
//! [`LduMatrix`], which builds the cell-gather index and runs on
//! [`ComputeBackend::Serial`]. The control flow — Arnoldi, the Givens rotations,
//! the `k x k` back substitution — stays on the host exactly as bead
//! `op-yvj.4.4` requires; only the `O(n)` and `O(n_faces)` kernels are
//! dispatched. See [`crate::krylov::bicgstab`]'s module documentation for the
//! determinism contract, which is identical here.
//!
//! # Where GMRES's cost sits, and why that matters for threading
//!
//! GMRES(m) does **one** sparse product per inner iteration but `j + 1` inner
//! products and `j + 1` `axpy`s at Arnoldi step `j`, so by the end of a cycle the
//! vector operations dominate — the opposite balance to BiCGStab, which is two
//! products against a fixed four inner products. Since the vector operations have
//! a size floor 64x higher than the product's
//! ([`VECOP_MIN_ELEMENTS`](crate::ldu_matrix::parallel::VECOP_MIN_ELEMENTS) =
//! 262 144 against
//! [`SPMV_MIN_CELLS`](crate::ldu_matrix::parallel::SPMV_MIN_CELLS) = 4 096),
//! GMRES has **less** to gain than BiCGStab from `CpuMulti` on a mid-sized mesh,
//! and more to gain on a very large one. That is a prediction of the dispatch
//! policy, and it is measured in `crate::krylov::hybrid_tests` rather than left
//! as a claim.

use std::sync::Arc;

use super::{KrylovResult, KrylovSettings, Preconditioner};
use crate::compute::ComputeBackend;
use crate::ldu_matrix::parallel::{axpy, dot, norm_l2, scale, HybridLdu};
use crate::ldu_matrix::LduMatrix;

/// Below this the Arnoldi vector norm is treated as a "happy breakdown" (the
/// Krylov space is exhausted / the solution lies exactly in the current space).
const HAPPY_TOL: f64 = 1.0e-300;

/// Solve `A x = b` with restarted right-preconditioned GMRES(m), serially, from
/// a bare [`LduMatrix`].
///
/// The convenience adapter over [`gmres_prepared`]: it builds the cell-gather
/// index and runs on [`ComputeBackend::Serial`]. In a solver loop prefer
/// [`gmres_prepared`], which reuses a caller-owned index and accepts a backend —
/// see [`crate::krylov::bicgstab`] for the same note in full.
///
/// # Arguments
/// - `a` — sparse system matrix (LDU). May be asymmetric.
/// - `b` — right-hand side, length `n_cells`.
/// - `x0` — optional initial guess; `None` means the zero vector.
/// - `precond` — preconditioner `M^{-1}` applied on the **right**.
/// - `settings` — `tolerance` (relative, on `||r||₂ / ||b||₂`), `max_iter` (total
///   inner-iteration cap), and `restart` = the subspace dimension `m`. A `restart`
///   of `0` is treated as `m = max_iter` (unrestarted up to the cap).
///
/// # Returns
/// `(x, result)` with the final iterate, its **true** relative residual
/// `||b − A x||₂ / ||b||₂`, and whether the tolerance was met. If `b` is exactly
/// zero, returns `x = 0`, `converged = true`, `0` iterations.
pub fn gmres(
    a: &LduMatrix,
    b: &[f64],
    x0: Option<&[f64]>,
    precond: &Preconditioner,
    settings: &KrylovSettings,
) -> (Vec<f64>, KrylovResult) {
    let ldu = HybridLdu::new(Arc::new(a.clone()));
    gmres_prepared(&ldu, b, x0, precond, settings, ComputeBackend::Serial)
}

/// Solve `A x = b` with restarted right-preconditioned GMRES(m) on a chosen
/// [`ComputeBackend`].
///
/// **This is the implementation**; [`gmres`] is a thin adapter onto it.
///
/// # Determinism
///
/// Bitwise identical on [`ComputeBackend::Serial`] and
/// [`ComputeBackend::CpuMulti`] at any thread count — identical iterates,
/// identical Givens residual estimates, identical iteration count. Every kernel
/// it uses carries that guarantee individually.
///
/// # Arguments
///
/// As [`gmres`], plus:
/// - `ldu` — the prepared sparse system, replacing `a`.
/// - `backend` — requested execution backend; degrades rather than failing when
///   unavailable.
///
/// # Storage
///
/// `O(m · n_cells)` for the Arnoldi basis, unchanged by the backend: this
/// function threads the kernels, it does not change the algorithm.
///
/// # Example
///
/// ```rust
/// use std::sync::Arc;
/// use outram_foam_basic_lib::compute::ComputeBackend;
/// use outram_foam_basic_lib::ldu_matrix::LduMatrix;
/// use outram_foam_basic_lib::ldu_matrix::parallel::HybridLdu;
/// use outram_foam_basic_lib::krylov::{gmres_prepared, KrylovSettings, Preconditioner};
///
/// let mut a = LduMatrix::new(4, vec![0, 1, 2], vec![1, 2, 3]);
/// a.diag = vec![4.0; 4];
/// a.lower = vec![-1.0; 3];
/// a.upper = vec![-2.0; 3];
/// let precond = Preconditioner::ilu0(&a);
/// let ldu = HybridLdu::new(Arc::new(a));
/// let b = vec![1.0, 2.0, 3.0, 4.0];
/// let settings = KrylovSettings::default();
///
/// let (x_ser, r_ser) =
///     gmres_prepared(&ldu, &b, None, &precond, &settings, ComputeBackend::Serial);
/// let (x_par, r_par) =
///     gmres_prepared(&ldu, &b, None, &precond, &settings, ComputeBackend::CpuMulti);
///
/// assert!(r_ser.converged);
/// assert_eq!(x_ser, x_par);
/// assert_eq!(r_ser.n_iterations, r_par.n_iterations);
/// ```
pub fn gmres_prepared(
    ldu: &HybridLdu,
    b: &[f64],
    x0: Option<&[f64]>,
    precond: &Preconditioner,
    settings: &KrylovSettings,
    backend: ComputeBackend,
) -> (Vec<f64>, KrylovResult) {
    gmres_impl(ldu, b, x0, precond, settings, backend, &mut Vec::new())
}

/// [`gmres_prepared`] with the relative-residual history captured.
///
/// `history` is cleared and then receives the Givens residual estimate
/// `|g[j + 1]| / ||b||₂` after every completed inner iteration — which, because
/// this solver is right-preconditioned, is the *true* relative residual rather
/// than an estimate of it. Crate-internal, for the backend-parity V&V in
/// `crate::krylov::hybrid_tests`; see
/// [`crate::krylov::bicgstab_impl`](super::bicgstab::bicgstab_impl) for why the
/// history is what a parity gate must compare.
pub(crate) fn gmres_impl(
    ldu: &HybridLdu,
    b: &[f64],
    x0: Option<&[f64]>,
    precond: &Preconditioner,
    settings: &KrylovSettings,
    backend: ComputeBackend,
    history: &mut Vec<f64>,
) -> (Vec<f64>, KrylovResult) {
    let n = ldu.matrix().n_cells;
    history.clear();
    let bnorm = norm_l2(b, backend);

    if bnorm == 0.0 {
        return (
            vec![0.0; n],
            KrylovResult {
                n_iterations: 0,
                final_residual: 0.0,
                converged: true,
            },
        );
    }

    let m = if settings.restart == 0 {
        settings.max_iter.max(1)
    } else {
        settings.restart
    };

    let mut x = match x0 {
        Some(g) => g.to_vec(),
        None => vec![0.0; n],
    };

    let tol = settings.tolerance;
    let mut total_iters = 0usize;
    let mut converged = false;
    // Reused across cycles so the preconditioner application allocates once.
    let mut mz = vec![0.0; n];

    // Outer (restart) loop.
    'outer: while total_iters < settings.max_iter {
        // r = b - A x ; beta = ||r||
        let r = ldu.residual(&x, b, backend);
        let beta = norm_l2(&r, backend);
        let rel = beta / bnorm;
        if rel <= tol {
            converged = true;
            break;
        }

        // Arnoldi basis V (m+1 vectors), Hessenberg H stored column-wise.
        let mut v: Vec<Vec<f64>> = Vec::with_capacity(m + 1);
        let mut v0 = r;
        scale(1.0 / beta, &mut v0, backend);
        v.push(v0);

        // Givens rotation coefficients and the rotated RHS g (size m+1).
        let mut cs = vec![0.0f64; m];
        let mut sn = vec![0.0f64; m];
        let mut g = vec![0.0f64; m + 1];
        g[0] = beta;
        // H[j] holds column j: entries h[0..=j+1].
        let mut hcols: Vec<Vec<f64>> = Vec::with_capacity(m);

        let mut k = 0usize; // number of completed Arnoldi steps this cycle
        for j in 0..m {
            if total_iters >= settings.max_iter {
                break;
            }
            total_iters += 1;

            // w = A M^{-1} v_j   (right preconditioning)
            precond.apply_on(&v[j], &mut mz, backend);
            let mut w = vec![0.0; n];
            ldu.spmv_into(&mz, &mut w, backend);

            // Modified Gram-Schmidt against existing basis.
            let mut hcol = vec![0.0f64; j + 2];
            for i in 0..=j {
                let hij = dot(&w, &v[i], backend);
                hcol[i] = hij;
                axpy(-hij, &v[i], &mut w, backend);
            }
            let hnext = norm_l2(&w, backend);
            hcol[j + 1] = hnext;

            // Apply previous Givens rotations to the new column.
            for i in 0..j {
                let temp = cs[i] * hcol[i] + sn[i] * hcol[i + 1];
                hcol[i + 1] = -sn[i] * hcol[i] + cs[i] * hcol[i + 1];
                hcol[i] = temp;
            }

            // Compute and apply the new Givens rotation to eliminate hcol[j+1].
            let (c, s) = givens(hcol[j], hcol[j + 1]);
            cs[j] = c;
            sn[j] = s;
            hcol[j] = c * hcol[j] + s * hcol[j + 1];
            hcol[j + 1] = 0.0;
            // Rotate the RHS.
            let g_temp = c * g[j] + s * g[j + 1];
            g[j + 1] = -s * g[j] + c * g[j + 1];
            g[j] = g_temp;

            hcols.push(hcol);
            k = j + 1;

            // Residual estimate = |g[j+1]| (exact for right preconditioning).
            let resid = g[j + 1].abs() / bnorm;
            history.push(resid);

            // Extend the basis unless we hit a happy breakdown.
            if hnext > HAPPY_TOL {
                let mut vnew = w;
                scale(1.0 / hnext, &mut vnew, backend);
                v.push(vnew);
            }

            if resid <= tol || hnext <= HAPPY_TOL {
                break;
            }
        }

        // Solve the k×k upper-triangular system H y = g by back substitution.
        let mut y = vec![0.0f64; k];
        for i in (0..k).rev() {
            let mut sum = g[i];
            for l in (i + 1)..k {
                sum -= hcols[l][i] * y[l];
            }
            let diag = hcols[i][i];
            y[i] = if diag.abs() > HAPPY_TOL {
                sum / diag
            } else {
                0.0
            };
        }

        // Correction in the (un-preconditioned) Krylov space: z = Σ y_i v_i.
        let mut z = vec![0.0f64; n];
        for i in 0..k {
            axpy(y[i], &v[i], &mut z, backend);
        }
        // Apply the preconditioner once: x += M^{-1} z.
        precond.apply_on(&z, &mut mz, backend);
        axpy(1.0, &mz, &mut x, backend);

        // Check the true residual after the update.
        let true_rel = norm_l2(&ldu.residual(&x, b, backend), backend) / bnorm;
        if !true_rel.is_finite() {
            break 'outer; // NaN/inf guard
        }
        if true_rel <= tol {
            converged = true;
            break;
        }
    }

    let final_rel = norm_l2(&ldu.residual(&x, b, backend), backend) / bnorm;
    let final_rel = if final_rel.is_finite() {
        final_rel
    } else {
        1.0
    };
    (
        x,
        KrylovResult {
            n_iterations: total_iters,
            final_residual: final_rel,
            converged: converged && final_rel <= tol,
        },
    )
}

/// Compute the Givens rotation `(c, s)` that zeroes `b` in `[a; b]`.
///
/// Returns `(c, s)` such that `[c s; -s c] · [a; b] = [r; 0]` with
/// `r = sqrt(a² + b²)`. Uses a scaling-free direct form (inputs here are already
/// well scaled). For `a = b = 0` returns `(1, 0)` (identity).
fn givens(a: f64, b: f64) -> (f64, f64) {
    if b == 0.0 {
        (1.0, 0.0)
    } else if a == 0.0 {
        (0.0, 1.0)
    } else {
        let r = a.hypot(b);
        (a / r, b / r)
    }
}
