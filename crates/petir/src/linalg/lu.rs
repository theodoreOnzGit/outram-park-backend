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
// Translated from the GNU Scientific Library (GSL)
// <https://www.gnu.org/software/gsl/>
// Copyright (C) 1996-2024 The GSL Team (Brian Gough, Gerard Jungman, et al.)
// GSL is free software under the GNU General Public License, version 3 or
// later. Upstream file: linalg/lu.c
//   `gsl_linalg_LU_det`    -> `det`
//   `gsl_linalg_LU_lndet`  -> `ln_det`
//   `gsl_linalg_LU_invert` -> `inverse`
//
// Built on `crate::linalg::square_matrix`'s Crout factorisation rather than on
// a second LU of our own: one factorisation in the crate means one set of
// pivoting decisions and one place for a defect to live.

//! Determinant, log-determinant and explicit inverse, from the LU factors.
//!
//! # Read this before calling [`inverse`]
//!
//! Almost nobody who wants `inverse(a)` actually wants it. If the next thing
//! you do is multiply by a vector, you want [`SquareMatrix::solve`]: it is
//! about `n` times cheaper and it is more accurate, because forming the inverse
//! explicitly and then multiplying rounds twice where solving rounds once.
//!
//! The legitimate uses are few — a covariance matrix whose individual entries
//! are the quantity of interest, an inverse that will be reused across very
//! many right-hand sides that are not available together, or handing a matrix
//! to an interface that demands one. [`inverse`] is provided for those, and
//! because GSL and Octave both have it, not as the default way to solve a
//! system.
//!
//! # The singularity sentinel, and what `det` can and cannot tell you
//!
//! The underlying Crout factorisation does not stop on a zero pivot: it writes
//! [`f64::EPSILON`] into that diagonal slot and continues, which is what
//! OpenFOAM does. Everything here detects that sentinel explicitly and reports
//! [`PetirError::Singular`] with the offending column.
//!
//! The consequence worth stating plainly: a determinant that comes back as a
//! very small number is **not** evidence of singularity, and one that comes
//! back exactly zero will not happen for a singular matrix — you get the error
//! instead. Neither is a conditioning test. A matrix can have a determinant of
//! `1.0` and be numerically hopeless (scale `[[1e-8, 0], [0, 1e8]]`), and a
//! well-conditioned matrix can have a determinant of `1e-300` purely from its
//! size. If the question is "can I trust this solve", the answer comes from the
//! residual `||Ax - b||`, not from the determinant.

use alloc::vec;
use alloc::vec::Vec;

// Under a std-linked build (`cargo test`) f64's inherent sqrt/exp/... shadow
// these trait methods, leaving the import formally unused. See crate::real.
#[allow(unused_imports)]
use crate::real::Real;

use crate::linalg::SquareMatrix;
use crate::{PetirError, Result};

/// Factorise `a`, returning the LU factors, the pivot list and the
/// permutation sign, or the column at which the matrix proved singular.
///
/// Shared by all three public routines so that they cannot disagree about
/// what "singular" means.
fn factorise(a: &SquareMatrix) -> Result<(SquareMatrix, Vec<usize>, f64)> {
    let n = a.n();
    if n == 0 {
        return Err(PetirError::Invalid);
    }
    let mut lu = a.clone();
    let pivot = lu.lu_decompose();

    // The factorisation writes exactly f64::EPSILON to a diagonal whose pivot
    // column was entirely zero; that sentinel is the singularity signal.
    for i in 0..n {
        if lu.get(i, i) == f64::EPSILON {
            return Err(PetirError::Singular { col: i });
        }
    }

    // Each recorded pivot that moved a row contributes a factor of -1.
    let mut sign = 1.0_f64;
    for (j, &p) in pivot.iter().enumerate() {
        if p != j {
            sign = -sign;
        }
    }
    Ok((lu, pivot, sign))
}

/// The determinant of `a`.
///
/// Computed as the product of the `U` diagonal times the sign of the row
/// permutation.
///
/// # Overflow
///
/// The determinant of a large matrix overflows easily — the product of 200
/// diagonal entries of magnitude 10 is `1e200`, and of 400 such entries is
/// `+inf`. Where the magnitude is what matters rather than the sign, use
/// [`ln_det`], which cannot overflow.
///
/// # Errors
///
/// - [`PetirError::Singular`] if a pivot column vanished, naming the column.
/// - [`PetirError::Invalid`] for a zero-order matrix.
///
/// # Example
///
/// ```
/// use petir::linalg::{det, SquareMatrix};
/// // [[1, 2], [3, 4]] has determinant 1*4 - 2*3 = -2.
/// let mut a = SquareMatrix::new(2);
/// a.set(0, 0, 1.0); a.set(0, 1, 2.0);
/// a.set(1, 0, 3.0); a.set(1, 1, 4.0);
/// assert!((det(&a).unwrap() + 2.0).abs() < 1e-12);
/// ```
pub fn det(a: &SquareMatrix) -> Result<f64> {
    let (lu, _pivot, sign) = factorise(a)?;
    let mut d = sign;
    for i in 0..lu.n() {
        d *= lu.get(i, i);
    }
    Ok(d)
}

/// The natural logarithm of `|det(a)|`.
///
/// # Why this exists separately
///
/// Log-determinants are what actually appear in practice — in a Gaussian
/// log-likelihood, in an entropy, in a Bayesian evidence — and they appear
/// there precisely *because* the determinant itself is unrepresentable. This
/// accumulates `ln|u_ii|` term by term, so a matrix whose determinant is
/// `1e-4000` reports `-9210.34` rather than `0.0`, and one whose determinant is
/// `1e4000` reports `+9210.34` rather than `+inf`.
///
/// The sign is necessarily discarded; recover it from [`det`] when it is small
/// enough to compute, or from the permutation parity.
///
/// # Errors
///
/// As [`det`].
///
/// # Example
///
/// ```
/// use petir::linalg::{ln_det, SquareMatrix};
/// // A diagonal matrix of 1e-200 entries: det = 1e-600, which is not a f64.
/// let mut a = SquareMatrix::new(3);
/// for i in 0..3 { a.set(i, i, 1e-200); }
/// let l = ln_det(&a).unwrap();
/// assert!((l - 3.0 * 1e-200_f64.ln()).abs() < 1e-9);
/// ```
pub fn ln_det(a: &SquareMatrix) -> Result<f64> {
    let (lu, _pivot, _sign) = factorise(a)?;
    let mut acc = 0.0_f64;
    for i in 0..lu.n() {
        acc += lu.get(i, i).abs().ln();
    }
    Ok(acc)
}

/// The explicit inverse of `a`.
///
/// One factorisation followed by `n` back-substitutions against the columns of
/// the identity — `O(n^3)`, the same order as the factorisation itself.
///
/// **Prefer [`SquareMatrix::solve`]** unless the inverse's entries are
/// themselves the answer; see this module's documentation for why.
///
/// # Errors
///
/// - [`PetirError::Singular`] if a pivot column vanished, naming the column.
/// - [`PetirError::Invalid`] for a zero-order matrix.
///
/// # Example
///
/// ```
/// use petir::linalg::{inverse, SquareMatrix};
/// let mut a = SquareMatrix::new(2);
/// a.set(0, 0, 4.0); a.set(0, 1, 7.0);
/// a.set(1, 0, 2.0); a.set(1, 1, 6.0);
/// let inv = inverse(&a).unwrap();
/// // 1/det * [[6, -7], [-2, 4]] with det = 10
/// assert!((inv.get(0, 0) - 0.6).abs() < 1e-12);
/// assert!((inv.get(0, 1) + 0.7).abs() < 1e-12);
/// assert!((inv.get(1, 0) + 0.2).abs() < 1e-12);
/// assert!((inv.get(1, 1) - 0.4).abs() < 1e-12);
/// ```
pub fn inverse(a: &SquareMatrix) -> Result<SquareMatrix> {
    let (lu, pivot, _sign) = factorise(a)?;
    let n = lu.n();
    let mut inv = SquareMatrix::new(n);
    for col in 0..n {
        // Back-substitute against the col-th column of the identity.
        let mut e = vec![0.0_f64; n];
        e[col] = 1.0;
        lu.lu_back_substitute(&pivot, &mut e);
        for (row, &value) in e.iter().enumerate() {
            inv.set(row, col, value);
        }
    }
    Ok(inv)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a matrix from a row-major literal, for readable tests.
    fn mat(n: usize, rows: &[f64]) -> SquareMatrix {
        let mut m = SquareMatrix::new(n);
        for i in 0..n {
            for j in 0..n {
                m.set(i, j, rows[i * n + j]);
            }
        }
        m
    }

    #[test]
    fn det_matches_the_closed_form_for_small_matrices() {
        // 1x1
        assert!((det(&mat(1, &[7.0])).unwrap() - 7.0).abs() < 1e-12);
        // 2x2: ad - bc
        assert!((det(&mat(2, &[1.0, 2.0, 3.0, 4.0])).unwrap() + 2.0).abs() < 1e-12);
        // 3x3, expanded by hand: 1(5*9-6*8) - 2(4*9-6*7) + 3(4*8-5*7) = 0 ->
        // use a non-singular one instead.
        let d = det(&mat(3, &[2.0, -1.0, 0.0, -1.0, 2.0, -1.0, 0.0, -1.0, 2.0])).unwrap();
        assert!((d - 4.0).abs() < 1e-12, "tridiagonal(2,-1) det = 4, got {d}");
    }

    /// The permutation sign is the easiest thing to get wrong and the hardest
    /// to notice: it flips the answer without changing its magnitude.
    #[test]
    fn det_gets_the_permutation_sign_right() {
        // Swapping two rows of the identity gives determinant -1.
        let swapped = mat(2, &[0.0, 1.0, 1.0, 0.0]);
        assert!((det(&swapped).unwrap() + 1.0).abs() < 1e-12);
        // A 3-cycle is an even permutation: determinant +1.
        let cycle = mat(3, &[0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 1.0, 0.0, 0.0]);
        assert!((det(&cycle).unwrap() - 1.0).abs() < 1e-12);
        // Identity is +1.
        let id = mat(3, &[1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0]);
        assert!((det(&id).unwrap() - 1.0).abs() < 1e-12);
    }

    #[test]
    fn det_of_a_triangular_matrix_is_the_diagonal_product() {
        let upper = mat(3, &[2.0, 5.0, -1.0, 0.0, 3.0, 7.0, 0.0, 0.0, -4.0]);
        assert!((det(&upper).unwrap() + 24.0).abs() < 1e-11);
    }

    #[test]
    fn det_reports_singularity_with_the_offending_column() {
        // Second row is twice the first: rank 1.
        let s = mat(2, &[1.0, 2.0, 2.0, 4.0]);
        match det(&s) {
            Err(PetirError::Singular { col }) => assert_eq!(col, 1),
            other => panic!("expected Singular, got {other:?}"),
        }
        // An all-zero matrix fails at the very first column.
        let z = mat(2, &[0.0, 0.0, 0.0, 0.0]);
        assert!(matches!(det(&z), Err(PetirError::Singular { col: 0 })));
    }

    #[test]
    fn zero_order_matrices_are_rejected() {
        let e = SquareMatrix::new(0);
        assert_eq!(det(&e).unwrap_err(), PetirError::Invalid);
        assert_eq!(ln_det(&e).unwrap_err(), PetirError::Invalid);
        assert_eq!(inverse(&e).unwrap_err(), PetirError::Invalid);
    }

    #[test]
    fn ln_det_agrees_with_the_log_of_det_where_both_are_computable() {
        let a = mat(3, &[4.0, 1.0, 0.0, 1.0, 3.0, 1.0, 0.0, 1.0, 2.0]);
        let d = det(&a).unwrap();
        let l = ln_det(&a).unwrap();
        assert!((l - d.abs().ln()).abs() < 1e-12, "det={d}, ln_det={l}");
    }

    /// The reason `ln_det` exists: it must survive where `det` overflows or
    /// underflows to nothing.
    #[test]
    fn ln_det_survives_where_det_is_unrepresentable() {
        let n = 4;
        let mut tiny = SquareMatrix::new(n);
        for i in 0..n {
            tiny.set(i, i, 1e-200);
        }
        assert_eq!(det(&tiny).unwrap(), 0.0, "1e-800 underflows to zero");
        let l = ln_det(&tiny).unwrap();
        assert!((l - 4.0 * 1e-200_f64.ln()).abs() < 1e-9, "ln_det={l}");

        let mut huge = SquareMatrix::new(n);
        for i in 0..n {
            huge.set(i, i, 1e200);
        }
        assert!(det(&huge).unwrap().is_infinite());
        assert!(ln_det(&huge).unwrap().is_finite());
    }

    #[test]
    fn inverse_reproduces_the_two_by_two_closed_form() {
        let a = mat(2, &[4.0, 7.0, 2.0, 6.0]);
        let inv = inverse(&a).unwrap();
        for (i, j, want) in [
            (0usize, 0usize, 0.6_f64),
            (0, 1, -0.7),
            (1, 0, -0.2),
            (1, 1, 0.4),
        ] {
            assert!(
                (inv.get(i, j) - want).abs() < 1e-12,
                "inv[{i}][{j}] = {}, want {want}",
                inv.get(i, j)
            );
        }
    }

    /// The defining property, checked directly rather than against a
    /// hand-computed inverse: A * A^-1 must be the identity.
    #[test]
    fn inverse_times_the_original_is_the_identity() {
        let a = mat(
            4,
            &[
                4.0, -2.0, 1.0, 0.5, //
                -2.0, 5.0, -1.0, 1.0, //
                1.0, -1.0, 3.0, -0.5, //
                0.5, 1.0, -0.5, 2.0,
            ],
        );
        let inv = inverse(&a).unwrap();
        for i in 0..4 {
            for j in 0..4 {
                let mut acc = 0.0_f64;
                for k in 0..4 {
                    acc += a.get(i, k) * inv.get(k, j);
                }
                let want = if i == j { 1.0 } else { 0.0 };
                assert!(
                    (acc - want).abs() < 1e-11,
                    "(A A^-1)[{i}][{j}] = {acc}, want {want}"
                );
            }
        }
    }

    #[test]
    fn inverse_of_the_inverse_is_the_original() {
        let a = mat(3, &[2.0, 1.0, 0.0, 1.0, 3.0, 1.0, 0.0, 1.0, 2.0]);
        let back = inverse(&inverse(&a).unwrap()).unwrap();
        for i in 0..3 {
            for j in 0..3 {
                assert!((back.get(i, j) - a.get(i, j)).abs() < 1e-10);
            }
        }
    }

    #[test]
    fn inverse_reports_singularity_rather_than_returning_infinities() {
        let s = mat(3, &[1.0, 2.0, 3.0, 2.0, 4.0, 6.0, 1.0, 1.0, 1.0]);
        assert!(matches!(inverse(&s), Err(PetirError::Singular { .. })));
    }

    /// Cross-check against the crate's own solver: the inverse's columns are
    /// exactly the solutions against the identity's columns.
    #[test]
    fn inverse_columns_agree_with_solve() {
        let a = mat(3, &[3.0, 1.0, 2.0, 1.0, 4.0, 0.5, 2.0, 0.5, 5.0]);
        let inv = inverse(&a).unwrap();
        for col in 0..3 {
            let mut e = vec![0.0_f64; 3];
            e[col] = 1.0;
            let x = a.solve(&e).unwrap();
            for row in 0..3 {
                assert!(
                    (inv.get(row, col) - x[row]).abs() < 1e-12,
                    "column {col} disagrees with solve at row {row}"
                );
            }
        }
    }
}
