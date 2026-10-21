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
// cf180cd7fbd06039a577f9c9ff0b428784765ac1, read 2026-09-14.
// Copyright (C) 2001, 2007 Brian Gough, Gerard Jungman
// GPL-3.0-or-later (verified from the per-file headers; see NOTICE).
//
//   linalg/tridiag.c  solve_tridiag                  -> solve_symm_tridiag
//                     gsl_linalg_solve_symm_tridiag  -> solve_symm_tridiag

//! Symmetric tridiagonal linear solver — GSL's `linalg/tridiag.c`.
//!
//! # Why this exists separately from [`SquareMatrix`]
//!
//! A tridiagonal system of order `n` has `3n - 2` non-zero entries, not `n^2`.
//! Solving it with a dense LU would cost `O(n^3)` time and `O(n^2)` memory to
//! get the same answer an `O(n)` sweep gives — and on a microcontroller the
//! memory is the binding constraint, not the time. For the cubic spline in
//! [`crate::interp`], which builds a tridiagonal system with one row per knot,
//! that is the difference between a usable routine and an unusable one.
//!
//! [`SquareMatrix`]: crate::linalg::SquareMatrix
//!
//! # Algorithm
//!
//! `A = L D L^T` — a Cholesky-like factorisation specialised to the
//! tridiagonal case, with `gamma` the sub-diagonal of `L` and `alpha` the
//! diagonal of `D` — followed by forward and back substitution. One pass each
//! way, `O(n)` throughout.
//!
//! # No pivoting, and what that means
//!
//! There is none, so a zero pivot is reported rather than worked around:
//! [`crate::PetirError::ZeroDivide`]. For the **diagonally dominant** systems
//! this is used on — which the natural cubic spline's always is, since its
//! diagonal is `2(h_i + h_{i+1})` against off-diagonals `h_{i+1}` — pivoting is
//! provably unnecessary. Hand it an indefinite system and it can fail on a
//! matrix a pivoting solver would handle; that is upstream's trade, kept.
//!
//! # Units
//!
//! Bare dimensionless `f64`.

use alloc::vec;
use alloc::vec::Vec;

use crate::zip::zip_flat;
use crate::{PetirError, Result};

/// Solve a symmetric tridiagonal system `A x = b`.
///
/// `diag` holds the `n` diagonal entries; `offdiag` holds the `n - 1`
/// off-diagonal entries, which by symmetry serve as both the sub- and
/// super-diagonal.
///
/// # Errors
///
/// - [`PetirError::Invalid`] if `diag` is empty.
/// - [`PetirError::LengthMismatch`] if `offdiag` is not exactly one shorter
///   than `diag`, or if `b` does not match `diag`.
/// - [`PetirError::ZeroDivide`] if a pivot vanishes. With no pivoting there is
///   no recovery, so this is reported rather than producing infinities — see
///   the module note on diagonal dominance.
///
/// # Example
///
/// ```
/// use petir::linalg::solve_symm_tridiag;
/// // [[2, 1, 0], [1, 2, 1], [0, 1, 2]] x = [3, 4, 3]  ->  x = [1, 1, 1]
/// let x = solve_symm_tridiag(&[2.0, 2.0, 2.0], &[1.0, 1.0], &[3.0, 4.0, 3.0]).unwrap();
/// for xi in &x {
///     assert!((xi - 1.0).abs() < 1e-12);
/// }
/// ```
pub fn solve_symm_tridiag(diag: &[f64], offdiag: &[f64], b: &[f64]) -> Result<Vec<f64>> {
    let n = diag.len();
    if n == 0 {
        // GSL: "matrix size must be positive", GSL_EBADLEN.
        return Err(PetirError::Invalid);
    }
    if offdiag.len() + 1 != n {
        return Err(PetirError::LengthMismatch {
            expected: n - 1,
            found: offdiag.len(),
        });
    }
    if b.len() != n {
        return Err(PetirError::LengthMismatch {
            expected: n,
            found: b.len(),
        });
    }

    // A = L D L^T, with lower_diag(L) = gamma and diag(D) = alpha.
    //
    // Every recurrence below carries its `i - 1` term in a local and grows its
    // output by `push`, rather than reading back through a subscript. The
    // arithmetic is unchanged from `linalg/tridiag.c`; what is gone is the
    // possibility of an out-of-range index, which on a `no_std` target is a
    // dead device rather than a stack trace (see tests/no_panic_gate.rs).
    let mut gamma: Vec<f64> = Vec::with_capacity(n.saturating_sub(1));
    let mut alpha: Vec<f64> = Vec::with_capacity(n);

    // `n == 0` is rejected above, so `split_first` always succeeds; the `else`
    // arm exists because the compiler cannot see that, and reports rather than
    // panics.
    let Some((&d0, diag_tail)) = diag.split_first() else {
        return Err(PetirError::Invalid);
    };
    if d0 == 0.0 {
        return Err(PetirError::ZeroDivide);
    }

    // DEVIATION FROM UPSTREAM, and a deliberate one.
    //
    // GSL computes `gamma[0] = offdiag[0] / alpha[0]` unconditionally. For a
    // 1x1 system `offdiag` is EMPTY, so that reads one past the end of the
    // array -- undefined behaviour in C, which in practice returns whatever
    // was in memory and is then never used.
    //
    // A 1x1 system needs no factorisation at all, so it returns directly. The
    // length checks above already guarantee `b.len() == n == 1`.
    let Some((&e0, offdiag_tail)) = offdiag.split_first() else {
        let Some(&b0) = b.first() else {
            return Err(PetirError::Invalid);
        };
        return Ok(vec![b0 / d0]);
    };

    alpha.push(d0);
    let mut g_prev = e0 / d0;
    gamma.push(g_prev);

    // Upstream's `for (i = 1; i < n - 1; i++)`. At step `i` the zip supplies
    // `diag[i]`, `offdiag[i - 1]` and `offdiag[i]`; it runs `n - 2` times
    // because `offdiag_tail` is the shortest of the three.
    for (&di, &e_prev, &ei) in zip_flat!(diag_tail.iter(), offdiag.iter(), offdiag_tail.iter()) {
        let a = di - e_prev * g_prev;
        if a == 0.0 {
            return Err(PetirError::ZeroDivide);
        }
        alpha.push(a);
        g_prev = ei / a;
        gamma.push(g_prev);
    }

    // The last diagonal entry: `alpha[n-1] = diag[n-1] - offdiag[n-2] * gamma[n-2]`.
    // `g_prev` is `gamma[n - 2]` on exit from the loop above.
    let (Some(&d_last), Some(&e_last)) = (diag.last(), offdiag.last()) else {
        return Err(PetirError::Invalid);
    };
    let a_last = d_last - e_last * g_prev;
    if a_last == 0.0 {
        return Err(PetirError::ZeroDivide);
    }
    alpha.push(a_last);

    // Forward substitution: z[0] = b[0], z[i] = b[i] - gamma[i-1] * z[i-1].
    let Some((&b0, b_tail)) = b.split_first() else {
        return Err(PetirError::Invalid);
    };
    let mut z: Vec<f64> = Vec::with_capacity(n);
    let mut z_prev = b0;
    z.push(z_prev);
    for (&bi, &g) in zip_flat!(b_tail.iter(), gamma.iter()) {
        z_prev = bi - g * z_prev;
        z.push(z_prev);
    }
    let c: Vec<f64> = zip_flat!(z.iter(), alpha.iter())
        .map(|(&zi, &ai)| zi / ai)
        .collect();

    // Back substitution: x[n-1] = c[n-1], x[i] = c[i] - gamma[i] * x[i+1].
    // Built back-to-front and reversed once, so the `i + 1` term is a local.
    let Some((&c_last, c_head)) = c.split_last() else {
        return Err(PetirError::Invalid);
    };
    let mut x: Vec<f64> = Vec::with_capacity(n);
    let mut x_next = c_last;
    x.push(x_next);
    for (&ci, &g) in zip_flat!(c_head.iter().rev(), gamma.iter().rev()) {
        x_next = ci - g * x_next;
        x.push(x_next);
    }
    x.reverse();

    Ok(x)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Multiply a symmetric tridiagonal matrix by a vector, for residual checks.
    fn matvec(diag: &[f64], offdiag: &[f64], x: &[f64]) -> Vec<f64> {
        let n = diag.len();
        let mut out = vec![0.0_f64; n];
        for i in 0..n {
            out[i] = diag[i] * x[i];
            if i > 0 {
                out[i] += offdiag[i - 1] * x[i - 1];
            }
            if i + 1 < n {
                out[i] += offdiag[i] * x[i + 1];
            }
        }
        out
    }

    #[test]
    fn solves_a_hand_checkable_system() {
        let x = solve_symm_tridiag(&[2.0, 2.0, 2.0], &[1.0, 1.0], &[3.0, 4.0, 3.0]).unwrap();
        for xi in &x {
            assert!((xi - 1.0).abs() < 1e-12, "got {x:?}");
        }
    }

    #[test]
    fn a_one_by_one_system_is_just_a_division() {
        let x = solve_symm_tridiag(&[4.0], &[], &[8.0]).unwrap();
        assert_eq!(x.len(), 1);
        assert!((x[0] - 2.0).abs() < 1e-15);
    }

    /// The real check: the residual, on a system with no hand-known answer.
    #[test]
    fn the_residual_vanishes_on_a_larger_system() {
        let n = 40;
        let diag: Vec<f64> = (0..n).map(|i| 4.0 + (i as f64) * 0.01).collect();
        let offdiag: Vec<f64> = (0..n - 1).map(|i| -1.0 - (i as f64) * 0.001).collect();
        let b: Vec<f64> = (0..n).map(|i| ((i as f64) * 0.37).sin()).collect();

        let x = solve_symm_tridiag(&diag, &offdiag, &b).unwrap();
        let ax = matvec(&diag, &offdiag, &x);

        for i in 0..n {
            assert!(
                (ax[i] - b[i]).abs() < 1e-12,
                "row {i}: (Ax)[i] = {}, b[i] = {}",
                ax[i],
                b[i]
            );
        }
    }

    /// Cross-check against the dense solver: two independent routes to the
    /// same answer.
    #[test]
    fn agrees_with_the_dense_lu_solver() {
        use crate::linalg::SquareMatrix;
        let diag = [4.0, 5.0, 6.0, 7.0];
        let offdiag = [1.0, -2.0, 0.5];
        let b = [1.0, 2.0, 3.0, 4.0];

        let sparse = solve_symm_tridiag(&diag, &offdiag, &b).unwrap();

        let mut m = SquareMatrix::new(4);
        for i in 0..4 {
            m.set(i, i, diag[i]);
            if i > 0 {
                m.set(i, i - 1, offdiag[i - 1]);
            }
            if i + 1 < 4 {
                m.set(i, i + 1, offdiag[i]);
            }
        }
        let dense = m.solve(&b).unwrap();

        for i in 0..4 {
            assert!(
                (sparse[i] - dense[i]).abs() < 1e-11,
                "row {i}: tridiag {} vs dense {}",
                sparse[i],
                dense[i]
            );
        }
    }

    #[test]
    fn malformed_input_is_reported() {
        assert_eq!(
            solve_symm_tridiag(&[], &[], &[]).unwrap_err(),
            PetirError::Invalid
        );
        // offdiag must be exactly one shorter than diag.
        assert!(matches!(
            solve_symm_tridiag(&[1.0, 2.0], &[1.0, 1.0], &[1.0, 1.0]),
            Err(PetirError::LengthMismatch { .. })
        ));
        // b must match diag.
        assert!(matches!(
            solve_symm_tridiag(&[1.0, 2.0], &[1.0], &[1.0]),
            Err(PetirError::LengthMismatch { .. })
        ));
    }

    /// A vanishing pivot is reported rather than producing infinities.
    #[test]
    fn a_zero_pivot_is_reported() {
        assert_eq!(
            solve_symm_tridiag(&[0.0, 1.0], &[1.0], &[1.0, 1.0]).unwrap_err(),
            PetirError::ZeroDivide
        );
    }
}
