// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 Theodore Ong and the outram-park contributors.
//
// PORTED from the GNU Scientific Library (GSL) 2.8, commit
// cf180cd7fbd06039a577f9c9ff0b428784765ac1, read 2026-10-20.
// GPL-3.0-or-later (verified from each file's own header; see NOTICE).
//
//   linalg/cholesky.c   Copyright (C) 2000 Thomas Walter;
//                       (C) 2000-2007 Brian Gough, Gerard Jungman;
//                       (C) 2016, 2019 Patrick Alken
//     :87   gsl_linalg_cholesky_decomp1
//     :103  gsl_linalg_cholesky_solve   (via :132 gsl_linalg_cholesky_svx)
//     :218  gsl_linalg_cholesky_invert
//     :624  cholesky_decomp_L2          (the gaxpy algorithm, G&VL 4.2.1)
//   linalg/invtri.c     Copyright (C) 2016-2019 Patrick Alken
//     :37   gsl_linalg_tri_invert, :75 triangular_inverse_L2 (lower branch)
//   linalg/trimult.c    Copyright (C) 2019 Patrick Alken
//     :39   gsl_linalg_tri_LTL, :62 triangular_multsymm_L2 (lower branch)
//   cblas/source_trsv_r.h, source_trmv_r.h, source_gemv_r.h, source_dot_r.h,
//   source_scal_r.h     Copyright (C) 1996-2000 Gerard Jungman
//     -- the row-major branches each call above reaches, inlined with the
//        same loop order and the same expressions.
//   matrix/swap_source.c:235  gsl_matrix_transpose_tricpy (copy only)

//! Cholesky factorisation `A = L L^T` of a symmetric positive-definite
//! matrix — GSL's `linalg/cholesky.c`.
//!
//! # Lineage
//!
//! **Ported.** Every loop below is GSL's, including the CBLAS kernels it
//! calls, which are inlined at the call site with their loop order and
//! expressions unchanged (`temp += x * a`, then `y += -1.0 * temp`, and so on)
//! so that the rounding is upstream's.
//!
//! # Storage, as GSL stores it
//!
//! [`Cholesky::packed`] is GSL's `LLT` matrix: the factor `L` in the lower
//! triangle *including the diagonal*, and the **original** `A` in the strict
//! upper triangle (GSL 2.2 onwards, `gsl_linalg_cholesky_decomp1`). Only the
//! **lower triangle of the input is read**: upstream's first act is to copy
//! it over the upper triangle, so an asymmetric input is silently symmetrised
//! from below, exactly as in GSL.
//!
//! # DEVIATION: the Level-2 algorithm at every size
//!
//! For `n <= 24` (`CROSSOVER_CHOLESKY`, `linalg/recurse.h`) GSL itself runs
//! the Level-2 routines ported here. Above that it switches to a recursive
//! Level-3 blocking (`cholesky_decomp_L3`, `triangular_inverse_L3`,
//! `triangular_multsymm_L3`) built on `dtrsm`, `dsyrk` and `dtrmm`. That
//! computes the same factor in a different order, so the rounding differs.
//! PETIR does not port the blocked form — it is a cache optimisation with no
//! numerical purpose, and it needs four Level-3 CBLAS kernels — and uses the
//! Level-2 algorithm at every size. The effect was **measured**, not assumed:
//! see `tests/gsl_cholesky_code_to_code.rs`, whose 30x30 case is above the
//! crossover.
//!
//! # Not ported
//!
//! `decomp2`/`svx2` (diagonal scaling), `decomp_unit`, `rcond` (needs GSL's
//! `invnorm1` Hager estimator) and the matrix right-hand-side `solve_mat`.
//! GSL has **no** Cholesky determinant routine, so none is offered here; use
//! [`crate::linalg::det`] (LU), which is also what Octave's `det` does.
//!
//! # Units
//!
//! Bare dimensionless `f64`.

use alloc::vec::Vec;

#[allow(unused_imports)]
use crate::real::Real;

use crate::linalg::elem::{rd, wr};
use crate::linalg::Matrix;
use crate::{PetirError, Result};

/// A Cholesky factorisation in GSL's packed `LLT` layout.
#[derive(Debug, Clone, PartialEq)]
pub struct Cholesky {
    llt: Matrix,
}

/// `gsl_matrix_transpose_tricpy(CblasLower, CblasUnit, A, A)`: copy the strict
/// lower triangle onto the strict upper one. No arithmetic.
fn lower_to_upper(a: &mut Matrix) {
    let n = a.rows();
    for i in 0..n {
        for j in 0..i {
            a.set(j, i, a.get(i, j));
        }
    }
}

impl Cholesky {
    /// Factorise `a`, `gsl_linalg_cholesky_decomp1` (`linalg/cholesky.c:87`).
    ///
    /// # Errors
    ///
    /// - [`PetirError::NotSquare`] if `a` is not square (`GSL_ENOTSQR`).
    /// - [`PetirError::Domain`] if a pivot is `<= 0`, i.e. the matrix is not
    ///   positive definite (`GSL_EDOM`, `cholesky.c:653`).
    ///
    /// # Example
    ///
    /// ```
    /// use petir::linalg::{Cholesky, Matrix};
    /// let a = Matrix::from_row_major(2, 2, vec![4.0, 2.0, 2.0, 3.0]).unwrap();
    /// let c = Cholesky::new(&a).unwrap();
    /// // L = [[2, 0], [1, sqrt(2)]]
    /// assert_eq!(c.packed().get(0, 0), 2.0);
    /// assert_eq!(c.packed().get(1, 0), 1.0);
    /// let x = c.solve(&[2.0, 1.0]).unwrap();
    /// assert!((x[0] - 0.5).abs() < 1e-15 && x[1].abs() < 1e-15);
    /// ```
    pub fn new(a: &Matrix) -> Result<Self> {
        let n = a.rows();
        if n != a.cols() {
            return Err(PetirError::NotSquare {
                rows: a.rows(),
                cols: a.cols(),
            });
        }
        let mut llt = a.clone();
        lower_to_upper(&mut llt);
        decomp_l2(&mut llt)?;
        Ok(Self { llt })
    }

    /// Order of the factorised matrix.
    pub fn n(&self) -> usize {
        self.llt.rows()
    }

    /// GSL's packed `LLT`: `L` on and below the diagonal, the original `A`
    /// strictly above it.
    pub fn packed(&self) -> &Matrix {
        &self.llt
    }

    /// The factor `L` alone, with zeros above the diagonal.
    pub fn l(&self) -> Matrix {
        let n = self.n();
        let mut l = self.llt.clone();
        for i in 0..n {
            for j in (i + 1)..n {
                l.set(i, j, 0.0);
            }
        }
        l
    }

    /// Solve `A x = b`, `gsl_linalg_cholesky_solve` (`cholesky.c:103`):
    /// forward substitution with `L`, then back substitution with `L^T`.
    ///
    /// # Errors
    ///
    /// [`PetirError::LengthMismatch`] if `b.len() != n` (`GSL_EBADLEN`).
    pub fn solve(&self, b: &[f64]) -> Result<Vec<f64>> {
        let n = self.n();
        if b.len() != n {
            return Err(PetirError::LengthMismatch {
                expected: n,
                found: b.len(),
            });
        }
        let mut x = b.to_vec();
        trsv_lower_notrans(&self.llt, &mut x);
        trsv_lower_trans(&self.llt, &mut x);
        Ok(x)
    }

    /// The inverse `A^{-1}`, `gsl_linalg_cholesky_invert`
    /// (`cholesky.c:218`): invert `L`, form `L^{-T} L^{-1}`, then mirror the
    /// lower triangle onto the upper. The result is exactly symmetric.
    ///
    /// # Errors
    ///
    /// [`PetirError::Singular`] if a diagonal entry of `L` is exactly zero
    /// (`GSL_ESING` from `triangular_singular`, `invtri.c:233`). A
    /// successfully constructed factor never has one, so this is unreachable
    /// in practice and is kept because upstream checks it.
    pub fn invert(&self) -> Result<Matrix> {
        let n = self.n();
        let mut t = self.llt.clone();
        // triangular_singular (invtri.c:233)
        for i in 0..n {
            if t.get(i, i) == 0.0 {
                return Err(PetirError::Singular { col: i });
            }
        }
        tri_invert_lower_l2(&mut t);
        tri_ltl_l2(&mut t);
        lower_to_upper(&mut t);
        Ok(t)
    }
}

/// `cholesky_decomp_L2` (`linalg/cholesky.c:624`), with the `dgemv` and
/// `dscal` it calls inlined.
fn decomp_l2(a: &mut Matrix) -> Result<()> {
    let n = a.rows();
    for j in 0..n {
        if j > 0 {
            // gsl_blas_dgemv(CblasNoTrans, -1.0, A(j:n, 0:j), A(j, 0:j), 1.0,
            // A(j:n, j)) -- source_gemv_r.h, RowMajor/NoTrans branch.
            // beta == 1 leaves y alone; alpha == -1 is applied as `alpha * temp`.
            for i in 0..(n - j) {
                let mut temp = 0.0_f64;
                for k in 0..j {
                    temp += a.get(j, k) * a.get(j + i, k);
                }
                a.set(j + i, j, a.get(j + i, j) + -1.0 * temp);
            }
        }

        let mut ajj = a.get(j, j);
        if ajj <= 0.0 {
            return Err(PetirError::Domain);
        }
        ajj = Real::sqrt(ajj);
        // gsl_vector_scale(&v, 1.0 / ajj) -> cblas_dscal: X *= alpha
        let alpha = 1.0 / ajj;
        for i in j..n {
            a.set(i, j, a.get(i, j) * alpha);
        }
    }
    Ok(())
}

/// `cblas_dtrsv(RowMajor, Lower, NoTrans, NonUnit)`: forward substitution
/// (`source_trsv_r.h`, second branch). `a` is square and `x.len() == n`.
pub(crate) fn trsv_lower_notrans(a: &Matrix, x: &mut [f64]) {
    let n = a.rows();
    if n == 0 {
        return;
    }
    wr(x, 0, rd(x, 0) / a.get(0, 0));
    for i in 1..n {
        let mut tmp = rd(x, i);
        for j in 0..i {
            tmp -= a.get(i, j) * rd(x, j);
        }
        wr(x, i, tmp / a.get(i, i));
    }
}

/// `cblas_dtrsv(RowMajor, Lower, Trans, NonUnit)`: back substitution with
/// `A^T` (`source_trsv_r.h`, fourth branch).
pub(crate) fn trsv_lower_trans(a: &Matrix, x: &mut [f64]) {
    let n = a.rows();
    if n == 0 {
        return;
    }
    wr(x, n - 1, rd(x, n - 1) / a.get(n - 1, n - 1));
    // for (i = N - 1; i > 0 && i--;)
    for i in (0..(n - 1)).rev() {
        let mut tmp = rd(x, i);
        for j in (i + 1)..n {
            tmp -= a.get(j, i) * rd(x, j);
        }
        wr(x, i, tmp / a.get(i, i));
    }
}

/// `triangular_inverse_L2(CblasLower, CblasNonUnit, T)` (`invtri.c:75`), with
/// `dtrmv(Lower, NoTrans, NonUnit)` and `dscal` inlined.
fn tri_invert_lower_l2(t: &mut Matrix) {
    let n = t.rows();
    for i in 0..n {
        let j = n - i - 1;
        let tjj = 1.0 / t.get(j, j);
        t.set(j, j, tjj);
        let ajj = -tjj;

        if j < n - 1 {
            // m = T(j+1.., j+1..), v = T(j+1.., j); len = n - j - 1.
            let len = n - j - 1;
            let o = j + 1;
            // source_trmv_r.h, RowMajor/NoTrans/Lower branch:
            //   for (i = N; i > 0 && i--;) { temp = sum_{jj<ii} X[jj]*A[ii][jj];
            //     X[ii] = temp + X[ii] * A[ii][ii]; }
            for ii in (0..len).rev() {
                let mut temp = 0.0_f64;
                for jj in 0..ii {
                    temp += t.get(o + jj, j) * t.get(o + ii, o + jj);
                }
                t.set(o + ii, j, temp + t.get(o + ii, j) * t.get(o + ii, o + ii));
            }
            // dscal(ajj, v)
            for ii in 0..len {
                t.set(o + ii, j, t.get(o + ii, j) * ajj);
            }
        }
    }
}

/// `triangular_multsymm_L2(CblasLower, T)` (`trimult.c:62`): `T <- L^T L` in
/// the lower triangle, with `ddot`, `dgemv(Trans)` and `dscal` inlined.
fn tri_ltl_l2(t: &mut Matrix) {
    let n = t.rows();
    for i in 0..n {
        let tii = t.get(i, i);
        if i < n - 1 {
            // ddot(v1, v1), v1 = T(i.., i): r += X * Y
            let mut tmp = 0.0_f64;
            for k in i..n {
                tmp += t.get(k, i) * t.get(k, i);
            }
            t.set(i, i, tmp);

            if i > 0 {
                // dgemv(CblasTrans, 1.0, m = T(i+1.., 0..i), x = T(i+1.., i),
                //       beta = Tii, y = T(i, 0..i))
                let len_x = n - i - 1;
                let len_y = i;
                // form y := beta * y
                if tii == 0.0 {
                    for iy in 0..len_y {
                        t.set(i, iy, 0.0);
                    }
                } else if tii != 1.0 {
                    for iy in 0..len_y {
                        t.set(i, iy, t.get(i, iy) * tii);
                    }
                }
                // RowMajor/Trans branch: temp = alpha * X[j]; Y[i] += temp * A[j][i]
                for jx in 0..len_x {
                    let temp = 1.0 * t.get(i + 1 + jx, i);
                    if temp != 0.0 {
                        for iy in 0..len_y {
                            t.set(i, iy, t.get(i, iy) + temp * t.get(i + 1 + jx, iy));
                        }
                    }
                }
            }
        } else {
            // dscal(Tii, row N-1): every entry of the last row, diagonal included
            for k in 0..n {
                t.set(n - 1, k, t.get(n - 1, k) * tii);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    fn spd3() -> Matrix {
        Matrix::from_row_major(
            3,
            3,
            vec![4.0, 12.0, -16.0, 12.0, 37.0, -43.0, -16.0, -43.0, 98.0],
        )
        .unwrap()
    }

    /// The textbook example has an exact integer factor:
    /// `L = [[2,0,0],[6,1,0],[-8,5,3]]`.
    #[test]
    fn the_textbook_example_factors_exactly() {
        let c = Cholesky::new(&spd3()).unwrap();
        let l = c.l();
        let want = [2.0, 0.0, 0.0, 6.0, 1.0, 0.0, -8.0, 5.0, 3.0];
        assert_eq!(l.as_slice(), &want[..]);
        // upper triangle of the packed form keeps the original A
        assert_eq!(c.packed().get(0, 1), 12.0);
        assert_eq!(c.packed().get(1, 2), -43.0);
    }

    #[test]
    fn solve_and_invert_reproduce_the_identity() {
        let a = spd3();
        let c = Cholesky::new(&a).unwrap();
        let x = c.solve(&[1.0, 2.0, 3.0]).unwrap();
        let r = a.mul_vec(&x).unwrap();
        for (ri, bi) in r.iter().zip([1.0, 2.0, 3.0]) {
            assert!((ri - bi).abs() < 1e-11);
        }
        let inv = c.invert().unwrap();
        for i in 0..3 {
            for j in 0..3 {
                let mut s = 0.0;
                for k in 0..3 {
                    s += a.get(i, k) * inv.get(k, j);
                }
                let e = if i == j { 1.0 } else { 0.0 };
                assert!((s - e).abs() < 1e-11, "({i},{j}) {s}");
                assert_eq!(inv.get(i, j), inv.get(j, i));
            }
        }
    }

    #[test]
    fn refusals_follow_gsl() {
        let indefinite = Matrix::from_row_major(2, 2, vec![1.0, 2.0, 2.0, 1.0]).unwrap();
        assert_eq!(Cholesky::new(&indefinite), Err(PetirError::Domain));
        let rect = Matrix::zeros(2, 3).unwrap();
        assert!(matches!(
            Cholesky::new(&rect),
            Err(PetirError::NotSquare { .. })
        ));
        let c = Cholesky::new(&spd3()).unwrap();
        assert!(matches!(
            c.solve(&[1.0]),
            Err(PetirError::LengthMismatch { .. })
        ));
    }
}
