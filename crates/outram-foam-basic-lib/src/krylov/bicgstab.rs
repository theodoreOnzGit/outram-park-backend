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

//! Preconditioned BiCGStab (bi-conjugate gradient stabilised).
//!
//! A short-recurrence Krylov method for **nonsymmetric** linear systems `A x = b`
//! (van der Vorst, 1992). Unlike CG it does not require `A` to be symmetric
//! positive-definite, and unlike GMRES it uses fixed (constant) work and storage
//! per iteration. It can, however, break down (the underlying bi-Lanczos process
//! can divide by a near-zero quantity); this implementation detects the two
//! classic breakdowns (`rho ≈ 0`, `omega ≈ 0`) and any `NaN`, and returns the
//! best iterate found so far with `converged = false` rather than propagating
//! garbage.
//!
//! Preconditioning is applied on both search directions (`p̂ = M^{-1} p`,
//! `ŝ = M^{-1} s`), i.e. right preconditioning of the stabilised recurrence. All
//! vectors are dimensionless `f64` of length `n_cells`.

use super::vecops::{axpy, dot, nrm2};
use super::{KrylovResult, KrylovSettings, Preconditioner};
use crate::ldu_matrix::LduMatrix;

/// Below this magnitude a BiCGStab scalar (`rho`, `omega`, or a denominator) is
/// treated as a breakdown rather than being divided by.
const BREAKDOWN_TOL: f64 = 1.0e-30;

/// Solve `A x = b` with preconditioned BiCGStab.
///
/// # Arguments
/// - `a` — the sparse system matrix (LDU); its `multiply` is the only SpMV used.
/// - `b` — right-hand side, length `n_cells`.
/// - `x0` — optional initial guess; `None` means the zero vector.
/// - `precond` — preconditioner `M^{-1}` (identity / Jacobi / ILU(0)).
/// - `settings` — tolerance (relative, on `||r||₂ / ||b||₂`) and `max_iter`;
///   `restart` is ignored by BiCGStab.
///
/// # Returns
/// `(x, result)` where `x` is the best iterate and `result` reports the iteration
/// count, the **true** relative residual `||b − A x||₂ / ||b||₂`, and whether the
/// tolerance was met. If `b` is exactly zero, returns `x = 0`, `converged = true`,
/// `0` iterations. On breakdown or `NaN`, returns the best-so-far iterate with
/// `converged = false`.
pub fn bicgstab(
    a: &LduMatrix,
    b: &[f64],
    x0: Option<&[f64]>,
    precond: &Preconditioner,
    settings: &KrylovSettings,
) -> (Vec<f64>, KrylovResult) {
    let n = a.n_cells;
    let bnorm = nrm2(b);

    // Trivial system: b == 0 -> x == 0 is the exact solution.
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

    let mut x = match x0 {
        Some(g) => g.to_vec(),
        None => vec![0.0; n],
    };

    // r = b - A x
    let mut r = a.residual(&x, b);
    let rhat0 = r.clone(); // fixed shadow residual r̂0

    // Track the best iterate by relative residual.
    let mut best_x = x.clone();
    let mut best_rel = nrm2(&r) / bnorm;

    if best_rel <= settings.tolerance {
        return (
            best_x,
            KrylovResult {
                n_iterations: 0,
                final_residual: best_rel,
                converged: true,
            },
        );
    }

    let mut rho_old: f64 = 1.0;
    let mut alpha: f64 = 1.0;
    let mut omega: f64 = 1.0;
    let mut v = vec![0.0; n];
    let mut p = vec![0.0; n];
    let mut phat = vec![0.0; n];
    let mut shat = vec![0.0; n];

    let mut iters = 0usize;
    let mut converged = false;

    while iters < settings.max_iter {
        iters += 1;

        let rho = dot(&rhat0, &r);
        if !rho.is_finite() || rho.abs() < BREAKDOWN_TOL {
            break; // rho breakdown
        }

        // beta = (rho/rho_old) * (alpha/omega)
        if omega.abs() < BREAKDOWN_TOL {
            break;
        }
        let beta = (rho / rho_old) * (alpha / omega);

        // p = r + beta*(p - omega*v)
        for i in 0..n {
            p[i] = r[i] + beta * (p[i] - omega * v[i]);
        }

        // phat = M^{-1} p ; v = A phat
        precond.apply(&p, &mut phat);
        v = a.multiply(&phat);

        let rhat_v = dot(&rhat0, &v);
        if !rhat_v.is_finite() || rhat_v.abs() < BREAKDOWN_TOL {
            break;
        }
        alpha = rho / rhat_v;

        // s = r - alpha*v
        let mut s = r.clone();
        axpy(-alpha, &v, &mut s);

        // Early convergence on the half-step: x += alpha*phat.
        let s_rel = nrm2(&s) / bnorm;
        if s_rel <= settings.tolerance {
            axpy(alpha, &phat, &mut x);
            let rel = nrm2(&a.residual(&x, b)) / bnorm;
            if rel < best_rel {
                best_rel = rel;
                best_x = x.clone();
            }
            converged = best_rel <= settings.tolerance;
            break;
        }

        // shat = M^{-1} s ; t = A shat
        precond.apply(&s, &mut shat);
        let t = a.multiply(&shat);

        let tt = dot(&t, &t);
        if !tt.is_finite() || tt.abs() < BREAKDOWN_TOL {
            // omega undefined: still take the alpha step before bailing.
            axpy(alpha, &phat, &mut x);
            let rel = nrm2(&a.residual(&x, b)) / bnorm;
            if rel < best_rel {
                best_rel = rel;
                best_x = x.clone();
            }
            break;
        }
        omega = dot(&t, &s) / tt;

        // x += alpha*phat + omega*shat
        axpy(alpha, &phat, &mut x);
        axpy(omega, &shat, &mut x);

        // r = s - omega*t
        r = s;
        axpy(-omega, &t, &mut r);

        let rel = nrm2(&r) / bnorm;
        if !rel.is_finite() {
            break; // NaN/inf guard: keep best-so-far
        }
        if rel < best_rel {
            best_rel = rel;
            best_x = x.clone();
        }
        if best_rel <= settings.tolerance {
            converged = true;
            break;
        }

        if omega.abs() < BREAKDOWN_TOL {
            break; // omega breakdown
        }
        rho_old = rho;
    }

    // Report the TRUE residual of the returned iterate.
    let true_rel = nrm2(&a.residual(&best_x, b)) / bnorm;
    let true_rel = if true_rel.is_finite() { true_rel } else { best_rel };
    (
        best_x,
        KrylovResult {
            n_iterations: iters,
            final_residual: true_rel,
            converged: converged && true_rel <= settings.tolerance,
        },
    )
}
