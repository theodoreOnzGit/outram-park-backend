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

use super::vecops::{axpy, dot, nrm2, scal};
use super::{KrylovResult, KrylovSettings, Preconditioner};
use crate::ldu_matrix::LduMatrix;

/// Below this the Arnoldi vector norm is treated as a "happy breakdown" (the
/// Krylov space is exhausted / the solution lies exactly in the current space).
const HAPPY_TOL: f64 = 1.0e-300;

/// Solve `A x = b` with restarted right-preconditioned GMRES(m).
///
/// # Arguments
/// - `a` — sparse system matrix (LDU); its `multiply` is the only SpMV used.
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
    let n = a.n_cells;
    let bnorm = nrm2(b);

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

    // Outer (restart) loop.
    'outer: while total_iters < settings.max_iter {
        // r = b - A x ; beta = ||r||
        let r = a.residual(&x, b);
        let beta = nrm2(&r);
        let rel = beta / bnorm;
        if rel <= tol {
            converged = true;
            break;
        }

        // Arnoldi basis V (m+1 vectors), Hessenberg H stored column-wise.
        let mut v: Vec<Vec<f64>> = Vec::with_capacity(m + 1);
        let mut v0 = r;
        scal(1.0 / beta, &mut v0);
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
            let mut mz = vec![0.0; n];
            precond.apply(&v[j], &mut mz);
            let mut w = a.multiply(&mz);

            // Modified Gram-Schmidt against existing basis.
            let mut hcol = vec![0.0f64; j + 2];
            for i in 0..=j {
                let hij = dot(&w, &v[i]);
                hcol[i] = hij;
                axpy(-hij, &v[i], &mut w);
            }
            let hnext = nrm2(&w);
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

            // Extend the basis unless we hit a happy breakdown.
            if hnext > HAPPY_TOL {
                let mut vnew = w;
                scal(1.0 / hnext, &mut vnew);
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
            y[i] = if diag.abs() > HAPPY_TOL { sum / diag } else { 0.0 };
        }

        // Correction in the (un-preconditioned) Krylov space: z = Σ y_i v_i.
        let mut z = vec![0.0f64; n];
        for i in 0..k {
            axpy(y[i], &v[i], &mut z);
        }
        // Apply the preconditioner once: x += M^{-1} z.
        let mut mz = vec![0.0; n];
        precond.apply(&z, &mut mz);
        axpy(1.0, &mz, &mut x);

        // Check the true residual after the update.
        let true_rel = nrm2(&a.residual(&x, b)) / bnorm;
        if !true_rel.is_finite() {
            break 'outer; // NaN/inf guard
        }
        if true_rel <= tol {
            converged = true;
            break;
        }
    }

    let final_rel = nrm2(&a.residual(&x, b)) / bnorm;
    let final_rel = if final_rel.is_finite() { final_rel } else { 1.0 };
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
