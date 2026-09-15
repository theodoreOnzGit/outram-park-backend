// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of PETIR, a component of OUTRAM PARK.
//
// PETIR is free software: you can redistribute it and/or modify it under the
// terms of the GNU General Public License as published by the Free Software
// Foundation, version 3 of the License.
//
// PETIR is distributed in the hope that it will be useful, but WITHOUT ANY
// WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
// FOR A PARTICULAR PURPOSE.  See the GNU General Public License for more
// details.
//
// You should have received a copy of the GNU General Public License along
// with PETIR.  If not, see <https://www.gnu.org/licenses/>.
//
// PORTED from the GNU Scientific Library (GSL) 2.8, commit
// cf180cd7fbd06039a577f9c9ff0b428784765ac1, read 2026-09-15.
// Copyright (C) 1996-2000 Gerard Jungman, Brian Gough (linalg/qr.c, author
// G. Jungman); Copyright (C) 1996-2000 Gerard Jungman (cblas/source_trsv_r.h).
// GPL-3.0-or-later (verified from the per-file headers; see NOTICE).
//
//   linalg/qr.c:104            gsl_linalg_QR_decomp_old
//   linalg/qr.c:150            gsl_linalg_QR_solve
//   linalg/qr.c:216            gsl_linalg_QR_lssolve
//   linalg/qr.c:362            gsl_linalg_QR_QTvec
//   linalg/qr.c:394            gsl_linalg_QR_Qvec
//   cblas/source_trsv_r.h      dtrsv, the Upper/NoTrans/NonUnit branch

//! Householder QR factorisation and least-squares solve — GSL's
//! `linalg/qr.c`.
//!
//! Factorises a general `m x n` matrix `A` as `A = Q R`, with `Q` orthogonal
//! and `R` upper triangular, and uses it to solve square systems and
//! overdetermined least-squares problems.
//!
//! # Why QR rather than the normal equations
//!
//! The obvious way to fit `m` samples to `n` basis functions is to solve
//! `A^T A c = A^T y`. Do not. Forming `A^T A` **squares the condition
//! number**, so a design matrix with a condition number of `1e8` — entirely
//! ordinary — produces a normal-equations system at `1e16`, which is beyond
//! what `f64` can resolve at all. QR works on `A` directly and its accuracy
//! depends on `cond(A)`, not its square. This is the whole reason
//! [`crate::cheb::ChebSeries::fit`] routes through here rather than through
//! the smaller amount of code the normal equations would need.
//!
//! # Which upstream decomposition this is
//!
//! GSL ships two: `gsl_linalg_QR_decomp`, a recursive Level-3 BLAS blocked
//! algorithm, and `gsl_linalg_QR_decomp_old`, the classical column-by-column
//! Householder sweep. **The classical one is ported here.** It produces the
//! same factorisation, it is the one whose structure a reader can check
//! against the textbook definition, and the blocked variant's advantage is
//! cache behaviour through a BLAS that this crate does not have and may not
//! acquire (see [`crate::linalg`]).
//!
//! # What is NOT here
//!
//! **Column pivoting, and therefore rank-deficient least squares.** GSL's
//! `linalg/qrpt.c` (`gsl_linalg_QRPT_decomp`, `_lssolve`, `_rank`) is the
//! source if that is wanted; it is vendored and unported. Without pivoting a
//! rank-deficient `A` gives a triangular factor with a zero — or
//! catastrophically small — diagonal entry, and back-substitution through it
//! produces numbers rather than an error. [`QrDecomposition::solve`] and
//! [`QrDecomposition::least_squares`] therefore check the diagonal and report
//! rather than returning that; see their docs. Tracked as `bn:op-4m4b`.
//!
//! Also absent: `Q` as an explicit matrix (`gsl_linalg_QR_unpack`), the rank-1
//! update (`gsl_linalg_QR_update`), and the condition estimator
//! (`gsl_linalg_QR_rcond`). None is needed by the fitting path.
//!
//! # Units
//!
//! Bare dimensionless `f64`.

use alloc::vec::Vec;

use crate::linalg::householder;
use crate::linalg::Matrix;
use crate::{PetirError, Result};

#[allow(unused_imports)]
use crate::real::Real;

/// A matrix in factored `Q R` form.
///
/// Built by [`QrDecomposition::new`]. `Q` is held implicitly, as the packed
/// Householder vectors in the strict lower triangle of `qr` together with
/// `tau` — the same storage scheme LAPACK uses, and upstream's.
#[derive(Debug, Clone)]
pub struct QrDecomposition {
    /// `R` in the diagonal and upper triangle; the Householder vectors below.
    qr: Matrix,
    /// One reflection coefficient per column, `min(m, n)` of them.
    tau: Vec<f64>,
}

/// The outcome of a least-squares solve.
#[derive(Debug, Clone, PartialEq)]
pub struct LeastSquares {
    /// The coefficient vector minimising `||A x - b||`.
    pub solution: Vec<f64>,
    /// `b - A x`, the part of `b` the model cannot reach. Its norm is the
    /// residual norm; its *direction* is often more informative — a residual
    /// with visible structure means the basis is missing something, where a
    /// structureless one means you have reached the noise.
    pub residual: Vec<f64>,
}

impl QrDecomposition {
    /// Factorise `a` in place as `A = Q R`.
    ///
    /// Translates `gsl_linalg_QR_decomp_old` (`linalg/qr.c:104`): for each
    /// column in turn, build the Householder reflection that annihilates it
    /// below the diagonal, then apply that reflection to every column to its
    /// right.
    ///
    /// # Errors
    ///
    /// [`PetirError::Invalid`] for a zero-dimensioned matrix.
    ///
    /// # Example
    ///
    /// ```
    /// use petir::linalg::{Matrix, QrDecomposition};
    /// let a = Matrix::from_row_major(2, 2, vec![1.0, 2.0, 3.0, 4.0]).unwrap();
    /// let qr = QrDecomposition::new(a).unwrap();
    /// // [[1, 2], [3, 4]] x = [5, 11]  ->  x = [1, 2]
    /// let x = qr.solve(&[5.0, 11.0]).unwrap();
    /// assert!((x[0] - 1.0).abs() < 1e-12);
    /// assert!((x[1] - 2.0).abs() < 1e-12);
    /// ```
    pub fn new(a: Matrix) -> Result<Self> {
        let m = a.rows();
        let n = a.cols();
        if m == 0 || n == 0 {
            return Err(PetirError::Invalid);
        }

        let mut qr = a;
        let k = m.min(n);
        let mut tau: Vec<f64> = Vec::with_capacity(k);

        for i in 0..k {
            // Reduce column i, rows i..m, to a multiple of e_1. Row-major
            // storage makes that column strided, so it is copied out, reduced
            // and written back -- see Matrix::column_from.
            let mut c = qr.column_from(i, i);
            let tau_i = householder::transform(&mut c);
            qr.set_column_from(i, i, &c);
            tau.push(tau_i);

            // Apply the same reflection to the columns to the right.
            if i + 1 < n {
                householder::hm(tau_i, &c, &mut qr, i, i + 1);
            }
        }

        Ok(Self { qr, tau })
    }

    /// Rows of the factored matrix.
    pub fn rows(&self) -> usize {
        self.qr.rows()
    }

    /// Columns of the factored matrix.
    pub fn cols(&self) -> usize {
        self.qr.cols()
    }

    /// The smallest absolute value on `R`'s diagonal, relative to the largest.
    ///
    /// # What it is, and what it is not
    ///
    /// This is a **cheap rank indicator, not a condition number.** A value
    /// near zero means `R` is numerically singular and any solve through it is
    /// meaningless. A value comfortably away from zero does *not* prove the
    /// system is well conditioned — without column pivoting the diagonal of
    /// `R` is not ordered, and an unpivoted QR can hide rank deficiency from
    /// exactly this test. GSL's `gsl_linalg_QR_rcond` (unported) is the real
    /// estimator, and `linalg/qrpt.c` is the pivoted factorisation that would
    /// make the test sound.
    ///
    /// It is reported rather than merely used internally because a caller
    /// fitting data is entitled to see how close their design matrix came to
    /// the cliff.
    pub fn diagonal_ratio(&self) -> f64 {
        let k = self.rows().min(self.cols());
        let mut smallest = f64::INFINITY;
        let mut largest = 0.0_f64;
        for i in 0..k {
            let d = self.qr.get(i, i).abs();
            if d < smallest {
                smallest = d;
            }
            if d > largest {
                largest = d;
            }
        }
        if largest == 0.0 {
            return 0.0;
        }
        smallest / largest
    }

    /// The packed factorisation: `R` in the diagonal and upper triangle, the
    /// Householder vectors below it.
    ///
    /// Exposed, rather than kept private, so the factorisation can be checked
    /// against upstream's rather than taken on trust —
    /// `tests/gsl_qr_code_to_code.rs` compares this entry-for-entry against
    /// what GSL's compiled `gsl_linalg_QR_decomp_old` produces on the same
    /// input. A caller who wants `Q` as an explicit matrix wants
    /// `gsl_linalg_QR_unpack`, which is not ported.
    pub fn packed(&self) -> &Matrix {
        &self.qr
    }

    /// The Householder coefficients, one per factored column.
    ///
    /// Exposed for the same reason as [`Self::packed`].
    pub fn tau(&self) -> &[f64] {
        &self.tau
    }

    /// Estimated rank of `R`, by upstream's criterion.
    ///
    /// Translates `gsl_linalg_QRPT_rank` (`linalg/qrpt.c:586`): count the
    /// diagonal entries whose magnitude exceeds a tolerance. A negative `tol`
    /// selects upstream's default,
    /// `20 * (m + n) * 2^floor(log2(max|diag R|)) * DBL_EPSILON`.
    ///
    /// # Why an exact-zero test will not do
    ///
    /// Measured 2026-09-15 on the 3x2 matrix whose second column is exactly
    /// twice the first: `R` comes out with `R[0][0] = -3.74` and
    /// `R[1][1] = -1.26e-15`, **not zero**. Exact dependence in exact
    /// arithmetic is near-dependence after rounding, so `d == 0.0` catches
    /// nothing and the solve proceeds. It returned `[0.25, 0.375]` with a
    /// residual of 1e-16 — a genuine minimiser, but an arbitrary one: which
    /// of the infinitely many splits between two dependent columns you get is
    /// decided by rounding. That is why this is reported rather than returned.
    ///
    /// # This test is weaker here than it is upstream
    ///
    /// `gsl_linalg_QRPT_rank` is written for a **column-pivoted** `R`, whose
    /// diagonal decreases monotonically, so a small entry really does mark the
    /// rank. This factorisation is unpivoted (see the module docs), and an
    /// unpivoted `R` can in principle hide a rank deficiency behind a
    /// well-sized diagonal. Taken as a guard it is sound — everything it
    /// rejects is genuinely deficient — but it is not a proof of full rank.
    /// `linalg/qrpt.c` is the source if that is wanted; `bn:op-4m4b`.
    pub fn rank(&self, tol: f64) -> usize {
        let k = self.rows().min(self.cols());
        let mut absmax = 0.0_f64;
        for i in 0..k {
            let d = self.qr.get(i, i).abs();
            if d > absmax {
                absmax = d;
            }
        }
        if absmax == 0.0 {
            return 0;
        }

        let eps = if tol < 0.0 {
            // `(int)(log(absmax) / M_LN2)` -- C truncates toward zero, which
            // `as i32` also does. `absmax > 0` is established above, so the
            // logarithm is finite.
            let ee = (absmax.ln() / core::f64::consts::LN_2) as i32;
            20.0 * ((self.rows() + self.cols()) as f64)
                * libm::pow(2.0, f64::from(ee))
                * crate::scalar::DBL_EPSILON
        } else {
            tol
        };

        let mut r = 0usize;
        for i in 0..k {
            if self.qr.get(i, i).abs() > eps {
                r += 1;
            }
        }
        r
    }

    /// Reject a factorisation whose columns are numerically dependent.
    ///
    /// Returns the index of the first diagonal entry that fails the rank test,
    /// which is the column a caller should look at.
    fn check_full_rank(&self) -> Result<()> {
        let n = self.cols();
        if self.rank(-1.0) >= self.rows().min(n) {
            return Ok(());
        }
        // Name the first offending column rather than just saying "singular".
        let k = self.rows().min(n);
        let mut absmax = 0.0_f64;
        for i in 0..k {
            let d = self.qr.get(i, i).abs();
            if d > absmax {
                absmax = d;
            }
        }
        let ee = if absmax > 0.0 {
            (absmax.ln() / core::f64::consts::LN_2) as i32
        } else {
            0
        };
        let eps = 20.0
            * ((self.rows() + n) as f64)
            * libm::pow(2.0, f64::from(ee))
            * crate::scalar::DBL_EPSILON;
        for i in 0..k {
            // The negation is deliberate, and clippy's `neg_cmp_op_on_partial_ord`
            // should not be taken up here: `abs() <= eps` is FALSE for a NaN
            // diagonal entry, which would let a factorisation of garbage pass as
            // full rank. `!(x > eps)` is true for NaN, so it is rejected --
            // matching upstream's `fabs(di) > eps` counting, where a NaN entry
            // likewise fails to increment the rank.
            if !(self.qr.get(i, i).abs() > eps) {
                return Err(PetirError::Singular { col: i });
            }
        }
        Err(PetirError::Singular { col: 0 })
    }

    /// Overwrite `v` with `Q^T v`.
    ///
    /// Translates `gsl_linalg_QR_QTvec` (`linalg/qr.c:362`): apply each stored
    /// reflection in turn to the trailing part of `v`.
    fn qt_vec(&self, v: &mut [f64]) -> Result<()> {
        let m = self.rows();
        if v.len() != m {
            return Err(PetirError::LengthMismatch {
                expected: m,
                found: v.len(),
            });
        }
        for (i, &ti) in self.tau.iter().enumerate() {
            let h = self.qr.column_from(i, i);
            let Some(w) = v.get_mut(i..) else {
                return Err(PetirError::Invalid);
            };
            householder::hv(ti, &h, w);
        }
        Ok(())
    }

    /// Overwrite `v` with `Q v`.
    ///
    /// Translates `gsl_linalg_QR_Qvec` (`linalg/qr.c:394`) — the same
    /// reflections as [`Self::qt_vec`], applied in the reverse order, which is
    /// what transposes an orthogonal product.
    fn q_vec(&self, v: &mut [f64]) -> Result<()> {
        let m = self.rows();
        if v.len() != m {
            return Err(PetirError::LengthMismatch {
                expected: m,
                found: v.len(),
            });
        }
        for (i, &ti) in self.tau.iter().enumerate().rev() {
            let h = self.qr.column_from(i, i);
            let Some(w) = v.get_mut(i..) else {
                return Err(PetirError::Invalid);
            };
            householder::hv(ti, &h, w);
        }
        Ok(())
    }

    /// Solve `R x = b` in place for the leading `n x n` block of `R`.
    ///
    /// Translates the `CblasRowMajor / CblasNoTrans / CblasUpper / NonUnit`
    /// branch of `cblas/source_trsv_r.h` — plain back-substitution, last
    /// unknown first.
    ///
    /// # Errors
    ///
    /// [`PetirError::Singular`] naming the column whose diagonal entry is
    /// zero. Upstream's `dtrsv` divides regardless and returns infinities;
    /// reporting is the deliberate deviation, because a silent infinity in a
    /// fit coefficient is indistinguishable from a real answer until it is
    /// far too late.
    fn r_solve_in_place(&self, x: &mut [f64]) -> Result<()> {
        let n = self.cols();
        if x.len() != n {
            return Err(PetirError::LengthMismatch {
                expected: n,
                found: x.len(),
            });
        }

        // Back-substitution: i descends over n-1 ..= 0.
        for i in (0..n).rev() {
            let mut tmp = x.get(i).copied().unwrap_or(f64::NAN);
            for (j, &xj) in (i + 1..n).zip(x.iter().skip(i + 1)) {
                tmp -= self.qr.get(i, j) * xj;
            }
            let diagonal = self.qr.get(i, i);
            if diagonal == 0.0 {
                return Err(PetirError::Singular { col: i });
            }
            if let Some(slot) = x.get_mut(i) {
                *slot = tmp / diagonal;
            }
        }
        Ok(())
    }

    /// Solve the square system `A x = b`.
    ///
    /// Translates `gsl_linalg_QR_solve` (`linalg/qr.c:150`): form `Q^T b`,
    /// then back-substitute through `R`.
    ///
    /// # Errors
    ///
    /// - [`PetirError::NotSquare`] if the factored matrix is not square. Use
    ///   [`Self::least_squares`] for an overdetermined system.
    /// - [`PetirError::LengthMismatch`] if `b` does not have `m` entries.
    /// - [`PetirError::Singular`] if the columns are numerically dependent,
    ///   naming the first column that fails [`Self::rank`]'s test.
    pub fn solve(&self, b: &[f64]) -> Result<Vec<f64>> {
        let m = self.rows();
        let n = self.cols();
        if m != n {
            return Err(PetirError::NotSquare { rows: m, cols: n });
        }
        if b.len() != m {
            return Err(PetirError::LengthMismatch {
                expected: m,
                found: b.len(),
            });
        }
        self.check_full_rank()?;
        let mut x = b.to_vec();
        self.qt_vec(&mut x)?;
        self.r_solve_in_place(&mut x)?;
        Ok(x)
    }

    /// Least-squares solution of the overdetermined system `A x = b`.
    ///
    /// Translates `gsl_linalg_QR_lssolve` (`linalg/qr.c:216`). With `A = Q R`
    /// and `Q` orthogonal, `||A x - b|| = ||R x - Q^T b||`; the first `n`
    /// rows of that can be driven to zero and the rest cannot, so the
    /// minimiser comes from back-substituting the leading `n` entries of
    /// `Q^T b`, and the remainder *is* the residual, rotated back by `Q`.
    ///
    /// # Errors
    ///
    /// - [`PetirError::Invalid`] if `m < n` — an underdetermined system has no
    ///   unique least-squares solution, and upstream rejects it too.
    /// - [`PetirError::LengthMismatch`] if `b` does not have `m` entries.
    /// - [`PetirError::Singular`] if `A`'s columns are numerically dependent,
    ///   naming the first column that fails [`Self::rank`]'s test. See that
    ///   method for why an exact-zero test would not catch this, and the
    ///   module docs for what a pivoted factorisation would add.
    ///
    /// # Example
    ///
    /// ```
    /// use petir::linalg::{Matrix, QrDecomposition};
    /// // Fit y = a + b t to three collinear points: t = 0, 1, 2; y = 1, 3, 5.
    /// let a = Matrix::from_row_major(3, 2, vec![1.0, 0.0, 1.0, 1.0, 1.0, 2.0]).unwrap();
    /// let fit = QrDecomposition::new(a).unwrap().least_squares(&[1.0, 3.0, 5.0]).unwrap();
    /// assert!((fit.solution[0] - 1.0).abs() < 1e-12);
    /// assert!((fit.solution[1] - 2.0).abs() < 1e-12);
    /// // The points are exactly collinear, so nothing is left over.
    /// assert!(fit.residual.iter().all(|r| r.abs() < 1e-12));
    /// ```
    pub fn least_squares(&self, b: &[f64]) -> Result<LeastSquares> {
        let m = self.rows();
        let n = self.cols();
        if m < n {
            return Err(PetirError::Invalid);
        }
        if b.len() != m {
            return Err(PetirError::LengthMismatch {
                expected: m,
                found: b.len(),
            });
        }

        self.check_full_rank()?;

        // residual <- Q^T b
        let mut residual = b.to_vec();
        self.qt_vec(&mut residual)?;

        // x <- leading n entries, then solve R x = x.
        let Some(head) = residual.get(..n) else {
            return Err(PetirError::Invalid);
        };
        let mut solution = head.to_vec();
        self.r_solve_in_place(&mut solution)?;

        // residual <- Q (Q^T b - R x): zero the leading n entries, which are
        // exactly the part R can reach, and rotate the rest back.
        for slot in residual.iter_mut().take(n) {
            *slot = 0.0;
        }
        self.q_vec(&mut residual)?;

        Ok(LeastSquares { solution, residual })
    }
}

/// Reconstruct `A` from a factorisation, for testing.
///
/// `A = Q R`, formed by applying the stored reflections to each column of the
/// upper triangle. Only used by this module's tests, and by
/// `tests/gsl_qr.rs`, which is why it is `pub(crate)` rather than public: a
/// caller who wants `Q` explicitly wants `gsl_linalg_QR_unpack`, which is not
/// ported.
#[cfg(test)]
pub(crate) fn reconstruct(qr: &QrDecomposition) -> Result<Matrix> {
    let m = qr.rows();
    let n = qr.cols();
    let mut out = Matrix::zeros(m, n)?;
    for j in 0..n {
        // Column j of R, zero below the diagonal.
        let mut col: Vec<f64> = (0..m)
            .map(|i| if i <= j { qr.qr.get(i, j) } else { 0.0 })
            .collect();
        qr.q_vec(&mut col)?;
        out.set_column_from(j, 0, &col);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::zip::zip_flat;
    use alloc::vec;

    fn max_abs_diff(a: &Matrix, b: &Matrix) -> f64 {
        let mut worst = 0.0_f64;
        for (ra, rb) in zip_flat!(a.row_iter(), b.row_iter()) {
            for (&x, &y) in ra.iter().zip(rb.iter()) {
                let d = (x - y).abs();
                if d > worst {
                    worst = d;
                }
            }
        }
        worst
    }

    /// The factorisation must reproduce the matrix it came from.
    ///
    /// # Methodology
    ///
    /// Factor a 4x3 matrix with no particular structure, then form `Q R` by
    /// applying the stored reflections to the columns of `R` and compare
    /// entry-wise against the original.
    ///
    /// # Results
    ///
    /// Maximum entry-wise difference 1.8e-15 on 2026-09-15, against entries of
    /// order 1-10. Interpretation: the packed Householder storage and the
    /// reflection application agree, to rounding.
    #[test]
    fn q_times_r_reproduces_the_original_matrix() {
        let data = vec![
            1.0, 2.0, 3.0, //
            4.0, 5.0, 6.0, //
            7.0, 8.0, 10.0, //
            2.0, -1.0, 4.0,
        ];
        let a = Matrix::from_row_major(4, 3, data).unwrap();
        let qr = QrDecomposition::new(a.clone()).unwrap();
        let back = reconstruct(&qr).unwrap();
        let worst = max_abs_diff(&a, &back);
        assert!(worst < 1e-13, "Q R differs from A by {worst}");
    }

    /// A square solve must reproduce a known solution.
    #[test]
    fn a_square_system_solves_exactly() {
        // [[2, 1], [1, 3]] x = [5, 10]  ->  x = [1, 3]
        let a = Matrix::from_row_major(2, 2, vec![2.0, 1.0, 1.0, 3.0]).unwrap();
        let x = QrDecomposition::new(a)
            .unwrap()
            .solve(&[5.0, 10.0])
            .unwrap();
        assert!((x.first().copied().unwrap_or(0.0) - 1.0).abs() < 1e-12);
        assert!((x.get(1).copied().unwrap_or(0.0) - 3.0).abs() < 1e-12);
    }

    /// The least-squares residual must be orthogonal to every column of `A` --
    /// that orthogonality IS the normal equation, and it is the property that
    /// distinguishes a minimiser from any other vector.
    ///
    /// # Results
    ///
    /// `|A^T r|` at most 3.6e-15 on an over-determined 5x3 system whose
    /// entries are of order 1-25, 2026-09-15.
    #[test]
    fn the_least_squares_residual_is_orthogonal_to_the_columns() {
        let data = vec![
            1.0, 1.0, 1.0, //
            1.0, 2.0, 4.0, //
            1.0, 3.0, 9.0, //
            1.0, 4.0, 16.0, //
            1.0, 5.0, 25.0,
        ];
        let a = Matrix::from_row_major(5, 3, data).unwrap();
        let b = vec![1.1, 3.9, 9.2, 15.8, 25.1];
        let qr = QrDecomposition::new(a.clone()).unwrap();
        let fit = qr.least_squares(&b).unwrap();

        for j in 0..a.cols() {
            let mut dot = 0.0_f64;
            for (i, &r) in fit.residual.iter().enumerate() {
                dot += a.get(i, j) * r;
            }
            assert!(dot.abs() < 1e-12, "column {j} not orthogonal: {dot}");
        }
    }

    /// `residual` must actually be `b - A x`, not merely something orthogonal.
    #[test]
    fn the_reported_residual_equals_b_minus_ax() {
        let data = vec![1.0, 0.0, 1.0, 1.0, 1.0, 2.0, 1.0, 3.0];
        let a = Matrix::from_row_major(4, 2, data).unwrap();
        let b = vec![1.0, 2.5, 4.5, 7.5];
        let fit = QrDecomposition::new(a.clone())
            .unwrap()
            .least_squares(&b)
            .unwrap();
        let ax = a.mul_vec(&fit.solution).unwrap();
        for (&bi, &axi, &ri) in zip_flat!(b.iter(), ax.iter(), fit.residual.iter()) {
            assert!(
                (bi - axi - ri).abs() < 1e-12,
                "residual {ri} != b - Ax = {}",
                bi - axi
            );
        }
    }

    /// An exactly rank-deficient system is REPORTED, not answered with
    /// infinities.
    ///
    /// Column 2 is twice column 1, so `R` has an exact zero on its diagonal
    /// and upstream's `dtrsv` would divide by it.
    #[test]
    fn a_rank_deficient_system_is_reported() {
        let data = vec![1.0, 2.0, 2.0, 4.0, 3.0, 6.0];
        let a = Matrix::from_row_major(3, 2, data).unwrap();
        let qr = QrDecomposition::new(a).unwrap();
        assert!(
            matches!(
                qr.least_squares(&[1.0, 2.0, 3.0]),
                Err(PetirError::Singular { .. })
            ),
            "a dependent column must report, not return infinities"
        );
        assert!(
            qr.diagonal_ratio() < 1e-15,
            "the rank indicator should be at the floor, got {}",
            qr.diagonal_ratio()
        );
    }

    #[test]
    fn malformed_input_is_reported() {
        let a = Matrix::from_row_major(2, 3, vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0]).unwrap();
        let wide = QrDecomposition::new(a).unwrap();
        // m < n: underdetermined, no unique least-squares solution.
        assert!(matches!(
            wide.least_squares(&[1.0, 2.0]),
            Err(PetirError::Invalid)
        ));

        let sq = Matrix::from_row_major(2, 2, vec![1.0, 0.0, 0.0, 1.0]).unwrap();
        let qr = QrDecomposition::new(sq).unwrap();
        assert!(matches!(
            qr.solve(&[1.0]),
            Err(PetirError::LengthMismatch { .. })
        ));

        let tall = Matrix::from_row_major(3, 2, vec![1.0, 0.0, 0.0, 1.0, 1.0, 1.0]).unwrap();
        let qr_tall = QrDecomposition::new(tall).unwrap();
        assert!(matches!(
            qr_tall.solve(&[1.0, 2.0, 3.0]),
            Err(PetirError::NotSquare { rows: 3, cols: 2 })
        ));
    }
}
