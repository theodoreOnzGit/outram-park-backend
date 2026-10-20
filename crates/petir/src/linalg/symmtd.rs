// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 Theodore Ong and the outram-park contributors.
//
// PORTED from the GNU Scientific Library (GSL) 2.8, commit
// cf180cd7fbd06039a577f9c9ff0b428784765ac1, read 2026-10-20.
// GPL-3.0-or-later (verified from each file's own header; see NOTICE).
//
//   linalg/symmtd.c     Copyright (C) 2001, 2007 Brian Gough;
//                       (C) 2019 Patrick Alken
//     :65   gsl_linalg_symmtd_decomp
//     :124  gsl_linalg_symmtd_unpack
//     :186  gsl_linalg_symmtd_unpack_T
//   linalg/householder.c Copyright (C) 1996-2000, 2004, 2007, 2010 Gerard
//                       Jungman, Brian Gough
//     :348  gsl_linalg_householder_left
//   cblas/source_symv.h, source_syr2.h, source_dot_r.h, source_axpy_r.h,
//   source_gemv_r.h, source_ger.h   Copyright (C) 1996-2000 Gerard Jungman
//     -- the row-major branches reached, inlined with the same loop order
//        and the same expressions.
//
// The Householder vector itself comes from `crate::linalg::householder::
// transform`, the existing port of `gsl_linalg_householder_transform`.

//! Symmetric tridiagonal decomposition `A = Q T Q^T` — GSL's
//! `linalg/symmtd.c`, the first stage of [`crate::linalg::eigen_symm`].
//!
//! # Lineage
//!
//! **Ported.** The CBLAS kernels upstream calls (`dsymv`, `ddot`, `daxpy`,
//! `dsyr2`, and `dgemv`/`dger` inside `householder_left`) are inlined with
//! their loop order and expressions unchanged.
//!
//! # Storage, as GSL stores it
//!
//! Only the **lower triangle** of the input is read or written. After
//! [`SymmTd::new`], the diagonal and first subdiagonal of
//! [`SymmTd::packed`] hold `T`, and column `i` below the subdiagonal holds the
//! tail of Householder vector `v_i` (its leading `1` implicit). The strict
//! upper triangle is the input's, untouched.
//!
//! # Deviations
//!
//! - **`tau` has `n - 1` entries and the last is `0.0`.** Upstream computes
//!   `n - 2` reflectors but its `tau` is `n - 1` long and is used as the
//!   workspace `x`, so GSL leaves a meaningless workspace value in
//!   `tau[n - 2]`. Nothing reads it; PETIR writes zero.
//! - **`n == 1` does nothing.** Upstream's `for (i = 0; i < N - 2; i++)` wraps
//!   `N - 2` round for `N == 1`; GSL's eigensolvers special-case `N == 1`
//!   before calling it, as [`crate::linalg::eigen_symm`] does.

use alloc::vec;
use alloc::vec::Vec;

use crate::linalg::elem::{acc, rd, wr};
use crate::linalg::householder;
use crate::linalg::Matrix;
use crate::{PetirError, Result};

/// A symmetric tridiagonal decomposition in GSL's packed layout.
#[derive(Debug, Clone, PartialEq)]
pub struct SymmTd {
    packed: Matrix,
    tau: Vec<f64>,
}

impl SymmTd {
    /// Decompose `a`, `gsl_linalg_symmtd_decomp` (`linalg/symmtd.c:65`).
    ///
    /// # Errors
    ///
    /// [`PetirError::NotSquare`] if `a` is not square.
    pub fn new(a: &Matrix) -> Result<Self> {
        let n = a.rows();
        if n != a.cols() {
            return Err(PetirError::NotSquare {
                rows: a.rows(),
                cols: n,
            });
        }
        let mut packed = a.clone();
        let mut tau = vec![0.0; n.saturating_sub(1)];
        decomp(&mut packed, &mut tau);
        Ok(Self { packed, tau })
    }

    /// GSL's packed matrix (see the module docs).
    pub fn packed(&self) -> &Matrix {
        &self.packed
    }

    /// The Householder coefficients, `n - 1` long (the last is unused).
    pub fn tau(&self) -> &[f64] {
        &self.tau
    }

    /// `(diag, subdiag)` of `T`, `gsl_linalg_symmtd_unpack_T`
    /// (`symmtd.c:186`). A copy; no arithmetic.
    pub fn unpack_t(&self) -> (Vec<f64>, Vec<f64>) {
        let n = self.packed.rows();
        let d = (0..n).map(|i| self.packed.get(i, i)).collect();
        let sd = (0..n.saturating_sub(1))
            .map(|i| self.packed.get(i + 1, i))
            .collect();
        (d, sd)
    }

    /// `(Q, diag, subdiag)`, `gsl_linalg_symmtd_unpack` (`symmtd.c:124`):
    /// `Q = Q_1 Q_2 ... Q_(n-2)` formed by applying the reflectors to the
    /// identity in reverse order.
    pub fn unpack(&self) -> Result<(Matrix, Vec<f64>, Vec<f64>)> {
        let n = self.packed.rows();
        let mut q = Matrix::zeros(n, n)?;
        for i in 0..n {
            q.set(i, i, 1.0);
        }
        // for (i = N - 2; i-- > 0;): i = N-3 down to 0
        for i in (0..n.saturating_sub(2)).rev() {
            let mut h = self.packed.column_from(i, i + 1);
            wr(&mut h, 0, 1.0);
            householder_left(rd(&self.tau, i), &h, &mut q, i + 1, i + 1);
        }
        let (d, sd) = self.unpack_t();
        Ok((q, d, sd))
    }
}

/// `gsl_linalg_symmtd_decomp`'s loop, on `a` in place.
pub(crate) fn decomp(a: &mut Matrix, tau: &mut [f64]) {
    let n = a.rows();
    for i in 0..n.saturating_sub(2) {
        // v = A(i+1.., i)
        let mut v = a.column_from(i, i + 1);
        let tau_i = householder::transform(&mut v);
        let len = n - i - 1;
        let o = i + 1;

        if tau_i != 0.0 {
            let ei = rd(&v, 0);
            wr(&mut v, 0, 1.0);

            // x = tau * A * v: dsymv(Lower, tau_i, m, v, 0.0, x),
            // source_symv.h RowMajor/Lower branch. beta == 0 zeroes y first.
            let mut x = vec![0.0_f64; len];
            for r in (0..len).rev() {
                let temp1 = tau_i * rd(&v, r);
                let mut temp2 = 0.0_f64;
                acc(&mut x, r, temp1 * a.get(o + r, o + r));
                for j in 0..r {
                    let arj = a.get(o + r, o + j);
                    acc(&mut x, j, temp1 * arj);
                    temp2 += rd(&v, j) * arj;
                }
                acc(&mut x, r, tau_i * temp2);
            }

            // w = x - (1/2) tau (x' v) v
            let mut xv = 0.0_f64;
            for k in 0..len {
                xv += rd(&x, k) * rd(&v, k);
            }
            let alpha = -0.5 * tau_i * xv;
            for k in 0..len {
                acc(&mut x, k, alpha * rd(&v, k));
            }

            // A = A - v w' - w v': dsyr2(Lower, -1.0, v, x, m),
            // source_syr2.h RowMajor/Lower branch.
            for r in 0..len {
                let tmp1 = -1.0 * rd(&v, r);
                let tmp2 = -1.0 * rd(&x, r);
                for j in 0..=r {
                    let cur = a.get(o + r, o + j);
                    a.set(o + r, o + j, cur + (tmp1 * rd(&x, j) + tmp2 * rd(&v, j)));
                }
            }

            wr(&mut v, 0, ei);
        }
        a.set_column_from(i, i + 1, &v);
        wr(tau, i, tau_i);
    }
}

/// `gsl_linalg_householder_left` (`linalg/householder.c:348`): apply
/// `H = I - tau v v^T` from the left to the trailing submatrix of `a` whose
/// top-left corner is `(r0, c0)`. `v.len()` is the submatrix's row count.
///
/// `work := A^T v` by `dgemv(Trans)`, then `A := A - tau v work^T` by `dger`,
/// each inlined from its RowMajor branch.
pub(crate) fn householder_left(tau: f64, v: &[f64], a: &mut Matrix, r0: usize, c0: usize) {
    if tau == 0.0 {
        return;
    }
    let m = v.len();
    let n = a.cols().saturating_sub(c0);
    // dgemv(CblasTrans, 1.0, A, v, 0.0, work): beta == 0 zeroes work, then
    // temp = alpha * X[j]; if temp != 0: Y[i] += temp * A[j][i]
    let mut work = vec![0.0_f64; n];
    for jx in 0..m {
        let temp = 1.0 * rd(v, jx);
        if temp != 0.0 {
            for iy in 0..n {
                acc(&mut work, iy, temp * a.get(r0 + jx, c0 + iy));
            }
        }
    }
    // dger(-tau, v, work, A): tmp = alpha * X[i]; A[i][j] += Y[j] * tmp
    for i in 0..m {
        let tmp = -tau * rd(v, i);
        for j in 0..n {
            let cur = a.get(r0 + i, c0 + j);
            a.set(r0 + i, c0 + j, cur + rd(&work, j) * tmp);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sym4() -> Matrix {
        Matrix::from_row_major(
            4,
            4,
            vec![
                4.0, 1.0, -2.0, 2.0, 1.0, 2.0, 0.0, 1.0, -2.0, 0.0, 3.0, -2.0, 2.0, 1.0, -2.0, -1.0,
            ],
        )
        .unwrap()
    }

    /// `Q T Q^T` reproduces `A`, and `Q` is orthogonal.
    #[test]
    fn q_t_qt_reproduces_a() {
        let a = sym4();
        let td = SymmTd::new(&a).unwrap();
        let (q, d, sd) = td.unpack().unwrap();
        assert_eq!(td.tau().len(), 3);
        let n = 4;
        for i in 0..n {
            for j in 0..n {
                let mut s = 0.0;
                let mut qq = 0.0;
                for k in 0..n {
                    qq += q.get(i, k) * q.get(j, k);
                    for l in 0..n {
                        let t = if k == l {
                            d[k]
                        } else if k + 1 == l {
                            sd[k]
                        } else if l + 1 == k {
                            sd[l]
                        } else {
                            0.0
                        };
                        s += q.get(i, k) * t * q.get(j, l);
                    }
                }
                assert!((s - a.get(i, j)).abs() < 1e-13, "({i},{j})");
                let e = if i == j { 1.0 } else { 0.0 };
                assert!((qq - e).abs() < 1e-14);
            }
        }
    }

    #[test]
    fn a_non_square_input_is_refused() {
        assert!(matches!(
            SymmTd::new(&Matrix::zeros(2, 3).unwrap()),
            Err(PetirError::NotSquare { .. })
        ));
    }
}
