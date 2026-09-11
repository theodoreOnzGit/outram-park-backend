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

//! Discretisation-agnostic linear-algebra contract, plus Krylov solvers written
//! against it.
//!
//! # What belongs in this module
//!
//! The [`LinearOperator`] contract — "I am an `n x n` linear map you can apply
//! to a vector, and I can hand you my diagonal" — and the
//! [`OperatorPreconditioner`] contract, together with the three matrix-free
//! Krylov drivers [`cg_op`], [`bicgstab_op`] and [`gmres_op`] that need nothing
//! more than those two contracts.
//!
//! # What does NOT belong here
//!
//! Any storage format. This module stores no coefficients of its own: it is the
//! seam at which a *representation* (LDU for the face-addressed finite-volume
//! side, CSR for a finite-element side) meets the *solvers*.
//!
//! # Why it exists
//!
//! Before this module every Krylov entry point in the crate
//! ([`crate::krylov::gmres`], [`crate::krylov::bicgstab`],
//! [`crate::ldu_matrix::solvers::conjugate_gradient`], GAMG, Gauss-Seidel) took
//! a concrete `&LduMatrix`. `LduMatrix` is a *good* representation for a
//! finite-volume mesh — one coefficient per internal face, owner/neighbour
//! addressing straight off the mesh — and a poor one for a finite-element
//! stiffness matrix, whose degree-of-freedom graph is denser than the cell graph
//! and has no face to hang a coefficient on.
//!
//! A finite-element crate therefore had exactly two bad options: reformulate
//! structural mechanics as finite volume so it could reuse the solvers, or
//! reimplement the solvers so it could keep its own matrix. This module is the
//! third option (GitHub issue #175): **one Krylov layer, two matrix layouts.**
//!
//! # Additive by construction
//!
//! Nothing in this module changes any existing function. The existing
//! finite-volume solvers keep their `&LduMatrix` signatures and their exact
//! behaviour; the functions here are *new* generic siblings. [`LduMatrix`]
//! implements [`LinearOperator`], so the finite-volume side may use them too,
//! but is not obliged to.
//!
//! # No trait objects
//!
//! Per the workspace design rules, [`LinearOperator`] and
//! [`OperatorPreconditioner`] are used **only as generic bounds**
//! (`fn cg_op<A: LinearOperator, M: OperatorPreconditioner>`), never as
//! `Box<dyn LinearOperator>`. Each call site monomorphises to one concrete
//! matrix type, so the matrix-vector product inlines exactly as it would have
//! done in a hand-written solver.
//!
//! # Units
//!
//! Everything here is dimensionless `f64`. A Krylov subspace mixes residuals,
//! search directions and solution increments that share no single physical
//! dimension, so no `uom` typing is applied — exactly as in
//! [`crate::krylov`]. Apply units at the layer that assembles the matrix.
//!
//! # Example
//!
//! ```rust
//! use outram_foam_basic_lib::krylov::KrylovSettings;
//! use outram_foam_basic_lib::ldu_matrix::LduMatrix;
//! use outram_foam_basic_lib::linear_operator::{cg_op, JacobiOperatorPreconditioner};
//!
//! // Symmetric positive-definite 3-cell chain.
//! let mut a = LduMatrix::new(3, vec![0, 1], vec![1, 2]);
//! a.diag = vec![4.0, 4.0, 4.0];
//! a.lower = vec![-1.0, -1.0];
//! a.upper = vec![-1.0, -1.0];
//! let b = vec![1.0, 2.0, 3.0];
//!
//! let m = JacobiOperatorPreconditioner::new(&a);
//! let settings = KrylovSettings { tolerance: 1e-12, max_iter: 100, restart: 30 };
//! let (x, result) = cg_op(&a, &b, None, &m, &settings);
//!
//! assert!(result.converged);
//! let mut ax = vec![0.0; 3];
//! outram_foam_basic_lib::linear_operator::LinearOperator::apply(&a, &x, &mut ax);
//! for i in 0..3 {
//!     assert!((ax[i] - b[i]).abs() < 1e-9);
//! }
//! ```

use crate::krylov::{KrylovResult, KrylovSettings};
use crate::ldu_matrix::LduMatrix;

/// A square linear map `A` that can be applied to a vector.
///
/// This is the single contract the discretisation-agnostic Krylov solvers in
/// this module need. Implement it for any sparse (or dense, or matrix-free)
/// representation and [`cg_op`] / [`bicgstab_op`] / [`gmres_op`] work on it
/// unchanged.
///
/// # Contract
///
/// - [`n_rows`](Self::n_rows) and [`n_cols`](Self::n_cols) must be equal and
///   constant for the lifetime of the operator: these solvers are square-system
///   solvers.
/// - [`apply`](Self::apply) must be **linear** and **deterministic**: the same
///   `x` must give bitwise the same `y`, and `A(u + v) = Au + Av` to rounding.
///   A Krylov method that is handed a non-linear or noisy operator does not
///   merely converge slowly, it converges to the wrong answer.
/// - [`diagonal`](Self::diagonal) must write `A[i][i]` into `d[i]`.
///
/// # Units
///
/// Dimensionless. `x` and `y` are bare `f64` slices in whatever units the
/// assembling layer chose; this contract neither knows nor enforces them.
///
/// # Implemented by
///
/// [`LduMatrix`] in this crate (face-addressed, finite-volume), and
/// `farrer_park::sparse::CsrMatrix` (row-compressed, finite-element).
#[diagnostic::on_unimplemented(
    message = "`{Self}` cannot be used as a Krylov system matrix: it does not implement `LinearOperator`",
    label = "not a linear operator",
    note = "implemented by `outram_foam_basic_lib::ldu_matrix::LduMatrix` (finite volume) and `farrer_park::sparse::CsrMatrix` (finite element)",
    note = "to add your own, implement `n_rows`, `n_cols`, `apply` and `diagonal`"
)]
pub trait LinearOperator {
    /// Number of rows of `A` (dimensionless count). Equals the length of `y` in
    /// [`apply`](Self::apply).
    fn n_rows(&self) -> usize;

    /// Number of columns of `A` (dimensionless count). Equals the length of `x`
    /// in [`apply`](Self::apply). Must equal [`n_rows`](Self::n_rows).
    fn n_cols(&self) -> usize;

    /// Write the matrix-vector product `y = A x`.
    ///
    /// `x` has length [`n_cols`](Self::n_cols), `y` has length
    /// [`n_rows`](Self::n_rows) and is **fully overwritten** (implementations
    /// must not accumulate into it).
    fn apply(&self, x: &[f64], y: &mut [f64]);

    /// Write the main diagonal `A[i][i]` into `d`, which has length
    /// [`n_rows`](Self::n_rows).
    ///
    /// Used to build a Jacobi preconditioner without exposing the storage
    /// format. A structurally-zero diagonal entry must be written as `0.0`;
    /// [`JacobiOperatorPreconditioner`] is the layer that decides what to do
    /// about it.
    fn diagonal(&self, d: &mut [f64]);

    /// Write the residual `r = b - A x`.
    ///
    /// Provided so an implementation with a fused kernel can override it; the
    /// default applies the operator and subtracts. `x`, `b` and `r` all have
    /// length `n`. `r` is fully overwritten.
    fn residual(&self, x: &[f64], b: &[f64], r: &mut [f64]) {
        self.apply(x, r);
        for i in 0..r.len() {
            r[i] = b[i] - r[i];
        }
    }
}

/// An approximate inverse `M^{-1} ~ A^{-1}` usable by the generic Krylov
/// drivers.
///
/// # Contract
///
/// [`apply`](Self::apply) writes `z = M^{-1} r`, with `r` and `z` the same
/// length as the system. It must be **linear** and **deterministic** for the
/// same reasons [`LinearOperator::apply`] must be, and for [`cg_op`] it must
/// additionally be **symmetric positive definite** — preconditioned CG is only
/// a conjugate-gradient method in the `M^{-1}`-inner product, and an
/// unsymmetric `M` silently destroys that.
///
/// # Units
///
/// Dimensionless, as [`LinearOperator`].
#[diagnostic::on_unimplemented(
    message = "`{Self}` cannot be used as a Krylov preconditioner: it does not implement `OperatorPreconditioner`",
    label = "not a preconditioner",
    note = "implemented by `IdentityPreconditioner`, `JacobiOperatorPreconditioner`, and `outram_foam_basic_lib::krylov::Preconditioner`",
    note = "pass `&IdentityPreconditioner` for an unpreconditioned solve"
)]
pub trait OperatorPreconditioner {
    /// Write `z = M^{-1} r`. Both slices have the system length; `z` is fully
    /// overwritten.
    fn apply(&self, r: &[f64], z: &mut [f64]);
}

/// No preconditioning: `M = I`, so `z = r`.
///
/// The correct choice when the system is already well conditioned, and the
/// baseline any other preconditioner must beat.
#[derive(Debug, Clone, Copy, Default)]
pub struct IdentityPreconditioner;

impl OperatorPreconditioner for IdentityPreconditioner {
    fn apply(&self, r: &[f64], z: &mut [f64]) {
        z.copy_from_slice(r);
    }
}

/// Diagonal (Jacobi) preconditioning built from any [`LinearOperator`]:
/// `z[i] = r[i] / A[i][i]`.
///
/// Stores the reciprocal diagonal, so applying it is one multiply per row.
/// Cannot break down: a diagonal entry whose magnitude is below `1e-300` is
/// treated as `1.0`, matching [`crate::krylov::JacobiPreconditioner`].
///
/// # Units
///
/// The stored reciprocals carry the inverse of whatever units the matrix
/// entries had; treated as dimensionless `f64` here.
#[derive(Debug, Clone)]
pub struct JacobiOperatorPreconditioner {
    /// `1 / A[i][i]` per row, length `n`.
    inv_diag: Vec<f64>,
}

impl JacobiOperatorPreconditioner {
    /// Extract and invert the diagonal of `a`.
    ///
    /// Cost: one [`LinearOperator::diagonal`] call plus `n` divisions.
    pub fn new<A: LinearOperator + ?Sized>(a: &A) -> Self {
        let n = a.n_rows();
        let mut d = vec![0.0; n];
        a.diagonal(&mut d);
        for v in d.iter_mut() {
            *v = if v.abs() > 1.0e-300 { 1.0 / *v } else { 1.0 };
        }
        Self { inv_diag: d }
    }

    /// The stored reciprocal diagonal `1 / A[i][i]`, length `n`.
    pub fn reciprocal_diagonal(&self) -> &[f64] {
        &self.inv_diag
    }
}

impl OperatorPreconditioner for JacobiOperatorPreconditioner {
    fn apply(&self, r: &[f64], z: &mut [f64]) {
        for i in 0..z.len() {
            z[i] = r[i] * self.inv_diag[i];
        }
    }
}

impl LinearOperator for LduMatrix {
    fn n_rows(&self) -> usize {
        self.n_cells
    }

    fn n_cols(&self) -> usize {
        self.n_cells
    }

    /// Face-addressed sparse multiply, delegating to
    /// [`LduMatrix::multiply`] so there is exactly one implementation of the
    /// finite-volume kernel.
    fn apply(&self, x: &[f64], y: &mut [f64]) {
        let prod = self.multiply(x);
        y.copy_from_slice(&prod);
    }

    fn diagonal(&self, d: &mut [f64]) {
        d.copy_from_slice(&self.diag);
    }

    /// Delegates to [`LduMatrix::residual`], the crate's existing fused kernel.
    fn residual(&self, x: &[f64], b: &[f64], r: &mut [f64]) {
        let res = LduMatrix::residual(self, x, b);
        r.copy_from_slice(&res);
    }
}

impl OperatorPreconditioner for crate::krylov::Preconditioner {
    /// Bridges the existing finite-volume preconditioner enum onto the generic
    /// contract, so an `LduMatrix` caller can use [`cg_op`] / [`gmres_op`] /
    /// [`bicgstab_op`] with ILU(0) as well as Jacobi.
    fn apply(&self, r: &[f64], z: &mut [f64]) {
        crate::krylov::Preconditioner::apply(self, r, z)
    }
}

// ── shared helpers ──────────────────────────────────────────────────────────

/// Euclidean norm, summed flat (no blocking), so results are reproducible
/// independent of problem size.
fn norm2(v: &[f64]) -> f64 {
    v.iter().map(|x| x * x).sum::<f64>().sqrt()
}

/// Euclidean inner product, summed flat.
fn dot(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b.iter()).map(|(x, y)| x * y).sum()
}

/// `y += alpha * x`.
fn axpy(alpha: f64, x: &[f64], y: &mut [f64]) {
    for i in 0..y.len() {
        y[i] += alpha * x[i];
    }
}

/// The trivial `b == 0` answer shared by all three drivers.
fn zero_rhs_result(n: usize) -> (Vec<f64>, KrylovResult) {
    (
        vec![0.0; n],
        KrylovResult {
            n_iterations: 0,
            final_residual: 0.0,
            converged: true,
        },
    )
}

/// Compute the true relative residual of `x`, and the work buffer it used.
fn true_relative_residual<A: LinearOperator + ?Sized>(
    a: &A,
    x: &[f64],
    b: &[f64],
    bnorm: f64,
    work: &mut [f64],
) -> f64 {
    a.residual(x, b, work);
    norm2(work) / bnorm
}

// ── generic Krylov drivers ──────────────────────────────────────────────────

/// Preconditioned conjugate gradients for a **symmetric positive-definite**
/// operator.
///
/// The right solver for a finite-element stiffness matrix, which is symmetric
/// by construction (`K = integral of B^T D B` with `D` symmetric), and for the
/// finite-volume pressure Poisson equation.
///
/// # Not a replacement for [`crate::ldu_matrix::solvers::conjugate_gradient`]
///
/// That function is the finite-volume path and keeps its DIC preconditioner and
/// its `&LduMatrix` signature unchanged. This one is generic over the operator
/// and takes the preconditioner from the caller. Neither calls the other.
///
/// # Arguments
///
/// - `a` — the SPD system operator. Behaviour on a non-symmetric or indefinite
///   operator is undefined (CG may stagnate or diverge); use [`gmres_op`] or
///   [`bicgstab_op`] there.
/// - `b` — right-hand side, length `a.n_rows()`, caller's units.
/// - `x0` — optional initial guess; `None` means the zero vector. Pass the
///   previous Newton iterate to warm-start.
/// - `precond` — SPD preconditioner `M^{-1}`.
/// - `settings` — `tolerance` (relative, on `||b - A x||_2 / ||b||_2`) and
///   `max_iter`. `restart` is ignored.
///
/// # Returns
///
/// `(x, result)`. `result.final_residual` is the **true** relative residual of
/// the returned iterate, recomputed from `a` and `b`, never an internal
/// estimate. An exactly-zero `b` returns `x = 0`, converged, `0` iterations.
///
/// # Cost
///
/// One [`LinearOperator::apply`] and one [`OperatorPreconditioner::apply`] per
/// iteration, plus `O(n)` vector work. Storage `O(n)`.
pub fn cg_op<A, M>(
    a: &A,
    b: &[f64],
    x0: Option<&[f64]>,
    precond: &M,
    settings: &KrylovSettings,
) -> (Vec<f64>, KrylovResult)
where
    A: LinearOperator + ?Sized,
    M: OperatorPreconditioner + ?Sized,
{
    let n = a.n_rows();
    debug_assert_eq!(b.len(), n, "cg_op: rhs length mismatch");
    let bnorm = norm2(b);
    if bnorm == 0.0 {
        return zero_rhs_result(n);
    }

    let mut x = match x0 {
        Some(g) => g.to_vec(),
        None => vec![0.0; n],
    };
    let mut r = vec![0.0; n];
    a.residual(&x, b, &mut r);

    let mut rel = norm2(&r) / bnorm;
    if rel <= settings.tolerance {
        return (
            x,
            KrylovResult {
                n_iterations: 0,
                final_residual: rel,
                converged: true,
            },
        );
    }

    let mut z = vec![0.0; n];
    precond.apply(&r, &mut z);
    let mut p = z.clone();
    let mut rz = dot(&r, &z);
    let mut ap = vec![0.0; n];

    let mut n_iter = 0usize;
    let mut converged = false;

    for iter in 0..settings.max_iter {
        a.apply(&p, &mut ap);
        let pap = dot(&p, &ap);
        if !pap.is_finite() || pap.abs() < 1.0e-300 {
            n_iter = iter;
            break;
        }
        let alpha = rz / pap;
        axpy(alpha, &p, &mut x);
        axpy(-alpha, &ap, &mut r);

        rel = norm2(&r) / bnorm;
        n_iter = iter + 1;
        if !rel.is_finite() {
            break;
        }
        if rel <= settings.tolerance {
            converged = true;
            break;
        }

        precond.apply(&r, &mut z);
        let rz_new = dot(&r, &z);
        if rz.abs() < 1.0e-300 {
            break;
        }
        let beta = rz_new / rz;
        rz = rz_new;
        for i in 0..n {
            p[i] = z[i] + beta * p[i];
        }
    }

    let final_rel = true_relative_residual(a, &x, b, bnorm, &mut r);
    let final_rel = if final_rel.is_finite() {
        final_rel
    } else {
        f64::INFINITY
    };
    (
        x,
        KrylovResult {
            n_iterations: n_iter,
            final_residual: final_rel,
            converged: converged && final_rel <= settings.tolerance,
        },
    )
}

/// Preconditioned BiCGStab for a general (possibly non-symmetric) operator.
///
/// The generic sibling of [`crate::krylov::bicgstab`]: same algorithm and same
/// breakdown guards, but over [`LinearOperator`] instead of `&LduMatrix`, and
/// with the preconditioner supplied by the caller rather than built from an
/// `LduMatrix`. The existing function is untouched.
///
/// # Arguments and returns
///
/// As [`cg_op`], except that `a` need not be symmetric. `settings.restart` is
/// ignored.
///
/// # Cost
///
/// Two [`LinearOperator::apply`] and two [`OperatorPreconditioner::apply`] per
/// iteration; fixed `O(n)` storage.
pub fn bicgstab_op<A, M>(
    a: &A,
    b: &[f64],
    x0: Option<&[f64]>,
    precond: &M,
    settings: &KrylovSettings,
) -> (Vec<f64>, KrylovResult)
where
    A: LinearOperator + ?Sized,
    M: OperatorPreconditioner + ?Sized,
{
    let n = a.n_rows();
    debug_assert_eq!(b.len(), n, "bicgstab_op: rhs length mismatch");
    let bnorm = norm2(b);
    if bnorm == 0.0 {
        return zero_rhs_result(n);
    }

    let mut x = match x0 {
        Some(g) => g.to_vec(),
        None => vec![0.0; n],
    };
    let mut r = vec![0.0; n];
    a.residual(&x, b, &mut r);
    let r0 = r.clone();

    let mut rho = 1.0;
    let mut alpha = 1.0;
    let mut omega = 1.0;
    let mut p = vec![0.0; n];
    let mut v = vec![0.0; n];
    let mut y = vec![0.0; n];
    let mut z = vec![0.0; n];
    let mut s = vec![0.0; n];
    let mut t = vec![0.0; n];

    let mut n_iter = 0usize;
    let mut converged = norm2(&r) / bnorm <= settings.tolerance;

    if !converged {
        for iter in 0..settings.max_iter {
            n_iter = iter + 1;
            let rho_new = dot(&r0, &r);
            if rho_new.abs() < 1.0e-300 || !rho_new.is_finite() {
                break; // breakdown
            }
            let beta = (rho_new / rho) * (alpha / omega);
            rho = rho_new;
            for i in 0..n {
                p[i] = r[i] + beta * (p[i] - omega * v[i]);
            }
            precond.apply(&p, &mut y);
            a.apply(&y, &mut v);
            let r0v = dot(&r0, &v);
            if r0v.abs() < 1.0e-300 || !r0v.is_finite() {
                break;
            }
            alpha = rho / r0v;
            for i in 0..n {
                s[i] = r[i] - alpha * v[i];
            }
            axpy(alpha, &y, &mut x);
            if norm2(&s) / bnorm <= settings.tolerance {
                converged = true;
                break;
            }
            precond.apply(&s, &mut z);
            a.apply(&z, &mut t);
            let tt = dot(&t, &t);
            if tt.abs() < 1.0e-300 || !tt.is_finite() {
                break;
            }
            omega = dot(&t, &s) / tt;
            axpy(omega, &z, &mut x);
            for i in 0..n {
                r[i] = s[i] - omega * t[i];
            }
            let rel = norm2(&r) / bnorm;
            if !rel.is_finite() {
                break;
            }
            if rel <= settings.tolerance {
                converged = true;
                break;
            }
            if omega.abs() < 1.0e-300 {
                break;
            }
        }
    }

    let final_rel = true_relative_residual(a, &x, b, bnorm, &mut r);
    let final_rel = if final_rel.is_finite() {
        final_rel
    } else {
        f64::INFINITY
    };
    (
        x,
        KrylovResult {
            n_iterations: n_iter,
            final_residual: final_rel,
            converged: converged && final_rel <= settings.tolerance,
        },
    )
}

/// Restarted right-preconditioned GMRES(m) for a general operator.
///
/// The generic sibling of [`crate::krylov::gmres`], algorithmically identical
/// (modified Gram-Schmidt Arnoldi, Givens rotations, happy-breakdown guard at
/// `1e-300`) but over [`LinearOperator`]. The existing function is untouched.
///
/// # Arguments and returns
///
/// As [`cg_op`], plus `settings.restart` = the subspace dimension `m`; `0` is
/// treated as `m = max_iter` (unrestarted up to the cap).
///
/// # Cost
///
/// One [`LinearOperator::apply`] and one [`OperatorPreconditioner::apply`] per
/// inner iteration; storage `O(m n)` for the Arnoldi basis.
pub fn gmres_op<A, M>(
    a: &A,
    b: &[f64],
    x0: Option<&[f64]>,
    precond: &M,
    settings: &KrylovSettings,
) -> (Vec<f64>, KrylovResult)
where
    A: LinearOperator + ?Sized,
    M: OperatorPreconditioner + ?Sized,
{
    /// Below this an Arnoldi vector norm is a happy breakdown. Same value as
    /// `crate::krylov::gmres::HAPPY_TOL`.
    const HAPPY_TOL: f64 = 1.0e-300;

    let n = a.n_rows();
    debug_assert_eq!(b.len(), n, "gmres_op: rhs length mismatch");
    let bnorm = norm2(b);
    if bnorm == 0.0 {
        return zero_rhs_result(n);
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
    let mut mz = vec![0.0; n];
    let mut r = vec![0.0; n];

    while total_iters < settings.max_iter {
        a.residual(&x, b, &mut r);
        let beta = norm2(&r);
        if beta / bnorm <= tol {
            converged = true;
            break;
        }

        let mut v: Vec<Vec<f64>> = Vec::with_capacity(m + 1);
        let mut v0 = r.clone();
        for e in v0.iter_mut() {
            *e /= beta;
        }
        v.push(v0);

        let mut cs = vec![0.0f64; m];
        let mut sn = vec![0.0f64; m];
        let mut g = vec![0.0f64; m + 1];
        g[0] = beta;
        let mut hcols: Vec<Vec<f64>> = Vec::with_capacity(m);
        let mut k = 0usize;

        for j in 0..m {
            if total_iters >= settings.max_iter {
                break;
            }
            total_iters += 1;

            precond.apply(&v[j], &mut mz);
            let mut w = vec![0.0; n];
            a.apply(&mz, &mut w);

            let mut hcol = vec![0.0f64; j + 2];
            for i in 0..=j {
                let hij = dot(&w, &v[i]);
                hcol[i] = hij;
                axpy(-hij, &v[i], &mut w);
            }
            let hnext = norm2(&w);
            hcol[j + 1] = hnext;

            for i in 0..j {
                let temp = cs[i] * hcol[i] + sn[i] * hcol[i + 1];
                hcol[i + 1] = -sn[i] * hcol[i] + cs[i] * hcol[i + 1];
                hcol[i] = temp;
            }

            let (c, s) = {
                let (h1, h2) = (hcol[j], hcol[j + 1]);
                if h2 == 0.0 {
                    (1.0, 0.0)
                } else {
                    let den = (h1 * h1 + h2 * h2).sqrt();
                    (h1 / den, h2 / den)
                }
            };
            cs[j] = c;
            sn[j] = s;
            hcol[j] = c * hcol[j] + s * hcol[j + 1];
            hcol[j + 1] = 0.0;
            let g_temp = c * g[j] + s * g[j + 1];
            g[j + 1] = -s * g[j] + c * g[j + 1];
            g[j] = g_temp;

            hcols.push(hcol);
            k = j + 1;

            let resid = g[j + 1].abs() / bnorm;
            if hnext > HAPPY_TOL {
                let mut vnew = w;
                for e in vnew.iter_mut() {
                    *e /= hnext;
                }
                v.push(vnew);
            }
            if resid <= tol || hnext <= HAPPY_TOL {
                break;
            }
        }

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

        let mut zc = vec![0.0f64; n];
        for i in 0..k {
            axpy(y[i], &v[i], &mut zc);
        }
        precond.apply(&zc, &mut mz);
        axpy(1.0, &mz, &mut x);

        let true_rel = true_relative_residual(a, &x, b, bnorm, &mut r);
        if !true_rel.is_finite() {
            break;
        }
        if true_rel <= tol {
            converged = true;
            break;
        }
    }

    let final_rel = true_relative_residual(a, &x, b, bnorm, &mut r);
    let final_rel = if final_rel.is_finite() {
        final_rel
    } else {
        f64::INFINITY
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::krylov::{bicgstab, gmres, Preconditioner};

    /// SPD tridiagonal LduMatrix helper.
    fn spd_tridiag(n: usize, d: f64, e: f64) -> LduMatrix {
        let owner: Vec<usize> = (0..n - 1).collect();
        let neigh: Vec<usize> = (1..n).collect();
        let mut a = LduMatrix::new(n, owner, neigh);
        a.diag = vec![d; n];
        a.lower = vec![e; n - 1];
        a.upper = vec![e; n - 1];
        a
    }

    fn nonsym_tridiag(n: usize, d: f64, lo: f64, up: f64) -> LduMatrix {
        let owner: Vec<usize> = (0..n - 1).collect();
        let neigh: Vec<usize> = (1..n).collect();
        let mut a = LduMatrix::new(n, owner, neigh);
        a.diag = vec![d; n];
        a.lower = vec![lo; n - 1];
        a.upper = vec![up; n - 1];
        a
    }

    fn rel_err(x: &[f64], y: &[f64]) -> f64 {
        let num: f64 = x.iter().zip(y).map(|(a, b)| (a - b) * (a - b)).sum();
        let den: f64 = y.iter().map(|b| b * b).sum();
        (num / den).sqrt()
    }

    /// `LduMatrix`'s `LinearOperator::apply` must agree bit-for-bit with its
    /// own `multiply`, so routing through the contract cannot perturb the
    /// finite-volume path.
    #[test]
    fn ldu_operator_apply_matches_multiply_bitwise() {
        let a = nonsym_tridiag(7, 4.0, -1.0, -2.0);
        let x: Vec<f64> = (0..7).map(|i| 0.3 * i as f64 - 1.1).collect();
        let direct = a.multiply(&x);
        let mut via = vec![0.0; 7];
        LinearOperator::apply(&a, &x, &mut via);
        assert_eq!(direct, via);

        let b: Vec<f64> = (0..7).map(|i| i as f64).collect();
        let direct_r = LduMatrix::residual(&a, &x, &b);
        let mut via_r = vec![0.0; 7];
        LinearOperator::residual(&a, &x, &b, &mut via_r);
        assert_eq!(direct_r, via_r);
    }

    /// `cg_op` on an SPD system reproduces the known solution.
    #[test]
    fn cg_op_spd_known_solution() {
        let n = 25;
        let a = spd_tridiag(n, 2.0, -1.0);
        let x_true: Vec<f64> = (0..n).map(|i| ((i + 1) as f64).sqrt()).collect();
        let b = a.multiply(&x_true);
        let s = KrylovSettings {
            tolerance: 1e-12,
            max_iter: 500,
            restart: 30,
        };
        let m = JacobiOperatorPreconditioner::new(&a);
        let (x, r) = cg_op(&a, &b, None, &m, &s);
        assert!(r.converged, "cg_op did not converge: {:?}", r);
        assert!(rel_err(&x, &x_true) < 1e-9, "rel err {}", rel_err(&x, &x_true));
    }

    /// The generic drivers must reach the same answer as the existing
    /// finite-volume ones on a non-symmetric system.
    #[test]
    fn generic_drivers_agree_with_existing_fv_solvers() {
        let n = 30;
        let a = nonsym_tridiag(n, 4.0, -1.0, -2.0);
        let b: Vec<f64> = (0..n).map(|i| (i as f64) - 12.5).collect();
        let s = KrylovSettings {
            tolerance: 1e-10,
            max_iter: 500,
            restart: 30,
        };

        let pc = Preconditioner::ilu0(&a);
        let (x_old_b, r_old_b) = bicgstab(&a, &b, None, &pc, &s);
        let (x_new_b, r_new_b) = bicgstab_op(&a, &b, None, &pc, &s);
        assert!(r_old_b.converged && r_new_b.converged);
        assert!(rel_err(&x_new_b, &x_old_b) < 1e-8);

        let (x_old_g, r_old_g) = gmres(&a, &b, None, &pc, &s);
        let (x_new_g, r_new_g) = gmres_op(&a, &b, None, &pc, &s);
        assert!(r_old_g.converged && r_new_g.converged);
        assert!(rel_err(&x_new_g, &x_old_g) < 1e-8);
    }

    /// Zero right-hand side short-circuits in all three drivers.
    #[test]
    fn zero_rhs_shortcircuits() {
        let a = spd_tridiag(6, 3.0, -1.0);
        let b = vec![0.0; 6];
        let s = KrylovSettings::default();
        let m = IdentityPreconditioner;
        for (x, r) in [
            cg_op(&a, &b, None, &m, &s),
            bicgstab_op(&a, &b, None, &m, &s),
            gmres_op(&a, &b, None, &m, &s),
        ] {
            assert!(r.converged && r.n_iterations == 0);
            assert!(x.iter().all(|v| *v == 0.0));
        }
    }

    /// A warm start that is already the answer costs zero iterations.
    #[test]
    fn warm_start_is_honoured() {
        let a = spd_tridiag(10, 2.0, -0.5);
        let x_true: Vec<f64> = (0..10).map(|i| i as f64 * 0.25).collect();
        let b = a.multiply(&x_true);
        let s = KrylovSettings {
            tolerance: 1e-10,
            max_iter: 100,
            restart: 20,
        };
        let m = JacobiOperatorPreconditioner::new(&a);
        let (_, r) = cg_op(&a, &b, Some(&x_true), &m, &s);
        assert!(r.converged);
        assert_eq!(r.n_iterations, 0);
    }
}
