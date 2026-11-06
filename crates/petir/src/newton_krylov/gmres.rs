// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of PETIR, a component of OUTRAM PARK.
//
// PORTED (line by line, not lifted) from this workspace's
// `crates/outram-foam-basic-lib/src/linear_operator.rs`, commit
// 69f3f242a483b44335a35a5584538dd7df64e035, read 2026-11-01: the
// `LinearOperator` and `OperatorPreconditioner` contracts,
// `IdentityPreconditioner`, the helpers `norm2` / `dot` / `axpy` /
// `zero_rhs_result` / `true_relative_residual`, and `gmres_op`
// (restarted right-preconditioned GMRES(m), Saad & Schultz 1986), together
// with `KrylovSettings` / `KrylovResult` from that crate's `krylov/mod.rs`.
// GPL-3.0 (OUTRAM PARK contributors). `outram-foam-basic-lib` depends on
// PETIR, so PETIR cannot call it; gh:#1066 asks for `gmres_op` to be reused
// rather than duplicated, and porting it is the closest this dependency
// direction allows. See "Why a port, not a lift" below.
//
// PETIR is distributed in the hope that it will be useful, but WITHOUT ANY
// WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
// FOR A PARTICULAR PURPOSE.  See the GNU General Public License for more
// details.

//! Matrix-free restarted GMRES and the operator contracts it needs.
//!
//! # Why a port, not a lift
//!
//! `outram_foam_basic_lib::linear_operator::gmres_op` is the workspace's
//! matrix-free GMRES, and gh:#1066 asks for it to be reused. It cannot be
//! called from here (that crate depends on PETIR), and it cannot be lifted
//! verbatim: its file also holds `LduMatrix` glue, it indexes with `[]`
//! (forbidden in PETIR, `tests/no_panic_gate.rs`) and it calls the method
//! form `.sqrt()` (forbidden, gh:#1001). So every expression of `gmres_op`
//! is kept — modified Gram–Schmidt Arnoldi, the Givens rotations in the
//! same order, the `1e-300` happy-breakdown guard, the back substitution,
//! the end-of-cycle true-residual check, the flat (unblocked) `norm2` and
//! `dot` — and only the subscripts become `get`/`get_mut` and `sqrt` becomes
//! `Real::sqrt` (correctly rounded, the same bits as `f64::sqrt`).
//! **Not yet pinned bit for bit against the source**: a comparison test
//! would have to live in `outram-foam-basic-lib` (follow-up, see the V&V
//! record).
//!
//! # Deviation
//!
//! **DEVIATION — no `diagonal`.** Upstream's `LinearOperator` also requires
//! `diagonal`, for its Jacobi preconditioner. A Jacobian-free operator has no
//! cheap diagonal, so the method is dropped here, and the Jacobi
//! preconditioner with it.

use alloc::vec;
use alloc::vec::Vec;

#[allow(unused_imports)]
use crate::real::Real;

use crate::linalg::Matrix;

/// Settings for [`gmres`] — upstream's `KrylovSettings`
/// (`outram-foam-basic-lib/src/krylov/mod.rs`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct KrylovSettings {
    /// Relative residual target `||b - A x||_2 / ||b||_2`.
    pub tolerance: f64,
    /// Cap on total inner iterations (operator applications).
    pub max_iter: usize,
    /// Subspace dimension `m` of GMRES(m); `0` means unrestarted up to
    /// `max_iter`.
    pub restart: usize,
}

impl Default for KrylovSettings {
    /// Upstream's defaults: `tolerance = 1e-8`, `max_iter = 1000`,
    /// `restart = 30`.
    fn default() -> Self {
        Self {
            tolerance: 1.0e-8,
            max_iter: 1000,
            restart: 30,
        }
    }
}

/// Outcome of [`gmres`] — upstream's `KrylovResult`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct KrylovResult {
    /// Inner iterations (operator applications inside Arnoldi) performed.
    pub n_iterations: usize,
    /// The true relative residual `||b - A x||_2 / ||b||_2` of the returned
    /// iterate, recomputed from the operator; `INFINITY` if non-finite.
    pub final_residual: f64,
    /// `true` iff the tolerance was met.
    pub converged: bool,
}

/// A square linear map that can be applied to a vector.
///
/// Port of upstream's contract minus `diagonal` (see the module docs). Used
/// only as a generic bound, never as a trait object.
#[diagnostic::on_unimplemented(
    message = "`{Self}` cannot be used as a GMRES operator: it does not implement `petir::newton_krylov::LinearOperator`",
    label = "not a linear operator",
    note = "implemented by `petir::linalg::Matrix` (dense) and `petir::newton_krylov::MffdOperator` (Jacobian-free)",
    note = "to add your own, implement `n_rows`, `n_cols` and `apply`"
)]
pub trait LinearOperator {
    /// Number of rows; equals the length of `y` in [`apply`](Self::apply).
    fn n_rows(&self) -> usize;
    /// Number of columns; equals the length of `x`. Must equal `n_rows`.
    fn n_cols(&self) -> usize;
    /// Write `y = A x`, overwriting `y`.
    fn apply(&self, x: &[f64], y: &mut [f64]);
    /// Write `r = b - A x`; the default applies the operator and subtracts.
    fn residual(&self, x: &[f64], b: &[f64], r: &mut [f64]) {
        self.apply(x, r);
        for (ri, &bi) in r.iter_mut().zip(b.iter()) {
            *ri = bi - *ri;
        }
    }
}

/// An approximate inverse `M^{-1} ~ A^{-1}`, applied as `z = M^{-1} r`.
///
/// Port of upstream's `OperatorPreconditioner`. Must be linear and
/// deterministic.
#[diagnostic::on_unimplemented(
    message = "`{Self}` cannot be used as a GMRES preconditioner: it does not implement `petir::newton_krylov::OperatorPreconditioner`",
    label = "not a preconditioner",
    note = "implemented by `IdentityPreconditioner` and `DenseLuPreconditioner`",
    note = "pass `&IdentityPreconditioner` for an unpreconditioned solve"
)]
pub trait OperatorPreconditioner {
    /// Write `z = M^{-1} r`, overwriting `z`.
    fn apply(&self, r: &[f64], z: &mut [f64]);
}

/// No preconditioning: `z = r`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct IdentityPreconditioner;

impl OperatorPreconditioner for IdentityPreconditioner {
    fn apply(&self, r: &[f64], z: &mut [f64]) {
        z.copy_from_slice(r);
    }
}

/// A dense matrix is a linear operator (row-major product).
impl LinearOperator for Matrix {
    fn n_rows(&self) -> usize {
        self.rows()
    }
    fn n_cols(&self) -> usize {
        self.cols()
    }
    fn apply(&self, x: &[f64], y: &mut [f64]) {
        for (yi, row) in y.iter_mut().zip(self.row_iter()) {
            let mut s = 0.0_f64;
            for (&a, &xj) in row.iter().zip(x.iter()) {
                s += a * xj;
            }
            *yi = s;
        }
    }
}

// ── shared helpers (upstream's, subscripts replaced) ─────────────────────────

/// Element `i`, or `0.0` if out of range (never, by construction here).
#[inline]
fn at(v: &[f64], i: usize) -> f64 {
    v.get(i).copied().unwrap_or(0.0)
}

/// Store `x` at `i` if in range.
#[inline]
fn put(v: &mut [f64], i: usize, x: f64) {
    if let Some(s) = v.get_mut(i) {
        *s = x;
    }
}

/// Euclidean norm, summed flat (upstream `norm2`).
fn norm2(v: &[f64]) -> f64 {
    Real::sqrt(v.iter().map(|x| x * x).sum::<f64>())
}

/// Euclidean inner product, summed flat (upstream `dot`).
fn dot(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b.iter()).map(|(x, y)| x * y).sum()
}

/// `y += alpha * x` (upstream `axpy`).
fn axpy(alpha: f64, x: &[f64], y: &mut [f64]) {
    for (yi, &xi) in y.iter_mut().zip(x.iter()) {
        *yi += alpha * xi;
    }
}

/// The `b == 0` answer (upstream `zero_rhs_result`).
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

/// True relative residual of `x` (upstream `true_relative_residual`).
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

/// Restarted right-preconditioned GMRES(m) — port of upstream `gmres_op`.
///
/// Solves `A x = b` as `A M^{-1} u = b`, `x = M^{-1} u`, so the Givens
/// residual estimate is the true residual. Each cycle: modified Gram–Schmidt
/// Arnoldi, Givens rotations, a `k x k` back substitution, then the true
/// residual is recomputed from `A` and the cycle restarts from the new `x`.
/// Stops when `||b - A x||_2 / ||b||_2 <= settings.tolerance`, on a happy
/// breakdown, or after `settings.max_iter` inner iterations. `x0` is the
/// initial guess (zero if `None`).
///
/// Returns the iterate and a [`KrylovResult`]; non-convergence is reported in
/// the result, not as an error (upstream's contract).
///
/// # Example
///
/// ```
/// use petir::linalg::Matrix;
/// use petir::newton_krylov::{gmres, IdentityPreconditioner, KrylovSettings};
/// let a = Matrix::from_row_major(2, 2, vec![4.0, 1.0, 2.0, 3.0]).unwrap();
/// let (x, r) = gmres(&a, &[1.0, 2.0], None, &IdentityPreconditioner, &KrylovSettings::default());
/// assert!(r.converged);
/// assert!((4.0 * x[0] + x[1] - 1.0).abs() < 1e-8);
/// ```
pub fn gmres<A, M>(
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
    /// Below this an Arnoldi vector norm is a happy breakdown (upstream).
    const HAPPY_TOL: f64 = 1.0e-300;

    let n = a.n_rows();
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
        put(&mut g, 0, beta);
        let mut hcols: Vec<Vec<f64>> = Vec::with_capacity(m);
        let mut k = 0usize;

        for j in 0..m {
            if total_iters >= settings.max_iter {
                break;
            }
            total_iters += 1;

            let Some(vj) = v.get(j) else {
                break;
            };
            precond.apply(vj, &mut mz);
            let mut w = vec![0.0; n];
            a.apply(&mz, &mut w);

            let mut hcol = vec![0.0f64; j + 2];
            for (i, vi) in v.iter().enumerate().take(j + 1) {
                let hij = dot(&w, vi);
                put(&mut hcol, i, hij);
                axpy(-hij, vi, &mut w);
            }
            let hnext = norm2(&w);
            put(&mut hcol, j + 1, hnext);

            for i in 0..j {
                let (ci, si) = (at(&cs, i), at(&sn, i));
                let (hi, hi1) = (at(&hcol, i), at(&hcol, i + 1));
                let temp = ci * hi + si * hi1;
                put(&mut hcol, i + 1, -si * hi + ci * hi1);
                put(&mut hcol, i, temp);
            }

            let (c, s) = {
                let (h1, h2) = (at(&hcol, j), at(&hcol, j + 1));
                if h2 == 0.0 {
                    (1.0, 0.0)
                } else {
                    let den = Real::sqrt(h1 * h1 + h2 * h2);
                    (h1 / den, h2 / den)
                }
            };
            put(&mut cs, j, c);
            put(&mut sn, j, s);
            let (hj, hj1) = (at(&hcol, j), at(&hcol, j + 1));
            put(&mut hcol, j, c * hj + s * hj1);
            put(&mut hcol, j + 1, 0.0);
            let (gj, gj1) = (at(&g, j), at(&g, j + 1));
            let g_temp = c * gj + s * gj1;
            put(&mut g, j + 1, -s * gj + c * gj1);
            put(&mut g, j, g_temp);

            hcols.push(hcol);
            k = j + 1;

            let resid = at(&g, j + 1).abs() / bnorm;
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
            let mut sum = at(&g, i);
            for l in (i + 1)..k {
                let hli = hcols.get(l).map_or(0.0, |c| at(c, i));
                sum -= hli * at(&y, l);
            }
            let diag = hcols.get(i).map_or(0.0, |c| at(c, i));
            put(
                &mut y,
                i,
                if diag.abs() > HAPPY_TOL {
                    sum / diag
                } else {
                    0.0
                },
            );
        }

        let mut zc = vec![0.0f64; n];
        for (&yi, vi) in y.iter().zip(v.iter()) {
            axpy(yi, vi, &mut zc);
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

    fn tridiag(n: usize) -> Matrix {
        let mut a = Matrix::zeros(n, n).unwrap();
        for i in 0..n {
            a.set(i, i, 4.0);
            if i + 1 < n {
                a.set(i, i + 1, -1.0);
                a.set(i + 1, i, -2.0); // nonsymmetric
            }
        }
        a
    }

    #[test]
    fn solves_a_nonsymmetric_system_with_and_without_restart() {
        let n = 40;
        let a = tridiag(n);
        let xs: Vec<f64> = (0..n).map(|i| (i as f64 * 0.3).sin()).collect();
        let mut b = vec![0.0; n];
        a.apply(&xs, &mut b);
        for restart in [0, 5, 40] {
            let s = KrylovSettings {
                tolerance: 1e-12,
                max_iter: 2000,
                restart,
            };
            let (x, r) = gmres(&a, &b, None, &IdentityPreconditioner, &s);
            assert!(r.converged, "restart {restart}: {r:?}");
            for (xi, ei) in x.iter().zip(xs.iter()) {
                assert!((xi - ei).abs() < 1e-9);
            }
        }
    }

    #[test]
    fn zero_rhs_returns_zero_immediately() {
        let a = tridiag(3);
        let (x, r) = gmres(
            &a,
            &[0.0; 3],
            None,
            &IdentityPreconditioner,
            &KrylovSettings::default(),
        );
        assert_eq!(x, vec![0.0; 3]);
        assert!(r.converged && r.n_iterations == 0);
    }

    #[test]
    fn iteration_cap_is_reported_as_not_converged() {
        let a = tridiag(30);
        let b = vec![1.0; 30];
        let s = KrylovSettings {
            tolerance: 1e-14,
            max_iter: 2,
            restart: 30,
        };
        let (_x, r) = gmres(&a, &b, None, &IdentityPreconditioner, &s);
        assert!(!r.converged);
        assert_eq!(r.n_iterations, 2);
    }
}
