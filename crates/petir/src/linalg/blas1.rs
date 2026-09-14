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
// later. Upstream file: blas/blas.c (the CBLAS level-1 wrappers) and the
// reference CBLAS `cblas_d*` kernels GSL dispatches to.
//   `gsl_blas_ddot`   -> `dot`     `gsl_blas_dnrm2`  -> `nrm2`
//   `gsl_blas_dasum`  -> `asum`    `gsl_blas_daxpy`  -> `axpy`
//   `gsl_blas_dscal`  -> `scal`    `gsl_blas_idamax` -> `iamax`
//   `gsl_blas_dswap`  -> `swap`

//! Level-1 BLAS: the vector kernels, in pure Rust.
//!
//! # Naming
//!
//! GSL's names without the `gsl_blas_d` prefix — `dot`, `nrm2`, `asum`, `axpy`,
//! `scal`, `iamax`, `swap`. The `d` (double) is dropped because PETIR has no
//! single-precision path to distinguish it from.
//!
//! # Why these are worth having as named functions
//!
//! Three of them are not the obvious loop:
//!
//! - [`nrm2`] scales before squaring, so that a vector containing `1e200` does
//!   not overflow on its way to a perfectly representable norm, and one
//!   containing `1e-200` does not underflow to zero.
//! - [`iamax`] follows the BLAS tie-breaking rule (**first** index of the
//!   maximum, not the last), which pivoting code depends on for reproducibility.
//! - [`axpy`] and [`scal`] short-circuit on `alpha == 0` and `alpha == 1`,
//!   matching the reference BLAS, which matters when they sit inside an inner
//!   loop that mostly passes those values.
//!
//! [`dot`] and [`asum`] are the obvious loop, and are here for completeness so
//! that a translated GSL routine reads the same as its source.
//!
//! # Units
//!
//! Bare dimensionless `f64` slices.

// Under a std-linked build (`cargo test`) f64's inherent sqrt/exp/... shadow
// these trait methods, leaving the import formally unused. See crate::real.
#[allow(unused_imports)]
use crate::real::Real;

use crate::{PetirError, Result};

/// Check two slices agree in length, the precondition of every binary kernel.
#[inline]
fn same_len(x: &[f64], y: &[f64]) -> Result<()> {
    if x.len() != y.len() {
        return Err(PetirError::LengthMismatch {
            expected: x.len(),
            found: y.len(),
        });
    }
    Ok(())
}

/// Inner product `x . y`.
///
/// # Errors
///
/// [`PetirError::LengthMismatch`] if the slices differ in length. GSL returns
/// `GSL_EBADLEN` here; PETIR reports both lengths so the caller can see which
/// side is wrong.
///
/// # Example
///
/// ```
/// use petir::linalg::dot;
/// assert_eq!(dot(&[1.0, 2.0, 3.0], &[4.0, 5.0, 6.0]).unwrap(), 32.0);
/// ```
pub fn dot(x: &[f64], y: &[f64]) -> Result<f64> {
    same_len(x, y)?;
    let mut acc = 0.0_f64;
    for (&xi, &yi) in x.iter().zip(y.iter()) {
        acc += xi * yi;
    }
    Ok(acc)
}

/// Euclidean norm `sqrt(sum x_i^2)`, computed without spurious overflow.
///
/// # Why not `dot(x, x).sqrt()`
///
/// Because that squares first. A vector holding `1e200` has a norm of `1e200`
/// — entirely representable — but `1e200 * 1e200` is `+inf`, so the naive
/// route returns infinity for a finite answer. The same happens downward: a
/// vector of `1e-200` entries has its squares underflow to zero and reports a
/// norm of exactly zero.
///
/// This is the standard BLAS `dnrm2` remedy: track the largest magnitude seen
/// so far and accumulate the sum of squares *relative to it*, rescaling when a
/// new maximum appears. One pass, no overflow, no underflow.
///
/// # Example
///
/// ```
/// use petir::linalg::nrm2;
/// assert_eq!(nrm2(&[3.0, 4.0]), 5.0);
/// // The naive sum of squares would return +inf here.
/// assert_eq!(nrm2(&[1e200, 0.0]), 1e200);
/// // ...and exactly 0.0 here.
/// assert_eq!(nrm2(&[1e-200, 0.0]), 1e-200);
/// ```
pub fn nrm2(x: &[f64]) -> f64 {
    if x.is_empty() {
        return 0.0;
    }
    if x.len() == 1 {
        return x[0].abs();
    }
    let mut scale = 0.0_f64;
    let mut ssq = 1.0_f64;
    for &xi in x {
        if xi == 0.0 {
            continue;
        }
        let ax = xi.abs();
        if scale < ax {
            // Rescale the running sum to the new, larger reference.
            let r = scale / ax;
            ssq = 1.0 + ssq * r * r;
            scale = ax;
        } else {
            let r = ax / scale;
            ssq += r * r;
        }
    }
    scale * ssq.sqrt()
}

/// Sum of absolute values, `sum |x_i|` — the vector 1-norm.
///
/// # Example
///
/// ```
/// use petir::linalg::asum;
/// assert_eq!(asum(&[1.0, -2.0, 3.0]), 6.0);
/// ```
pub fn asum(x: &[f64]) -> f64 {
    x.iter().map(|v| v.abs()).sum()
}

/// `y <- alpha * x + y`, in place.
///
/// # Errors
///
/// [`PetirError::LengthMismatch`] if the slices differ in length.
///
/// # Example
///
/// ```
/// use petir::linalg::axpy;
/// let mut y = [1.0, 1.0, 1.0];
/// axpy(2.0, &[1.0, 2.0, 3.0], &mut y).unwrap();
/// assert_eq!(y, [3.0, 5.0, 7.0]);
/// ```
pub fn axpy(alpha: f64, x: &[f64], y: &mut [f64]) -> Result<()> {
    same_len(x, y)?;
    if alpha == 0.0 {
        // Matches the reference BLAS: a zero multiplier leaves y untouched,
        // rather than adding 0.0 and turning a -0.0 or a NaN in x into noise.
        return Ok(());
    }
    if alpha == 1.0 {
        for (yi, &xi) in y.iter_mut().zip(x.iter()) {
            *yi += xi;
        }
        return Ok(());
    }
    for (yi, &xi) in y.iter_mut().zip(x.iter()) {
        *yi += alpha * xi;
    }
    Ok(())
}

/// `x <- alpha * x`, in place.
///
/// # Example
///
/// ```
/// use petir::linalg::scal;
/// let mut x = [1.0, 2.0, 3.0];
/// scal(3.0, &mut x);
/// assert_eq!(x, [3.0, 6.0, 9.0]);
/// ```
pub fn scal(alpha: f64, x: &mut [f64]) {
    if alpha == 1.0 {
        return;
    }
    for xi in x.iter_mut() {
        *xi *= alpha;
    }
}

/// Index of the **first** element of largest absolute value.
///
/// # Why "first" is part of the contract
///
/// BLAS specifies the lowest index among ties, and pivoting code relies on it:
/// two runs of the same factorisation must choose the same pivot row, or the
/// resulting `L` and `U` differ and results stop being bit-reproducible. A
/// `max_by` written with `>=` instead of `>` would silently take the *last*
/// maximum and break that.
///
/// Returns `None` for an empty slice — GSL returns `0`, which is
/// indistinguishable from a genuine first-element maximum.
///
/// # Example
///
/// ```
/// use petir::linalg::iamax;
/// assert_eq!(iamax(&[1.0, -5.0, 3.0]), Some(1));
/// assert_eq!(iamax(&[2.0, -2.0]), Some(0), "ties take the lowest index");
/// assert_eq!(iamax(&[]), None);
/// ```
pub fn iamax(x: &[f64]) -> Option<usize> {
    if x.is_empty() {
        return None;
    }
    let mut best = 0_usize;
    let mut best_abs = x[0].abs();
    for (i, &xi) in x.iter().enumerate().skip(1) {
        let a = xi.abs();
        // Strictly greater: ties keep the earlier index.
        if a > best_abs {
            best_abs = a;
            best = i;
        }
    }
    Some(best)
}

/// Exchange the contents of two vectors.
///
/// # Errors
///
/// [`PetirError::LengthMismatch`] if the slices differ in length.
///
/// # Example
///
/// ```
/// use petir::linalg::swap;
/// let mut a = [1.0, 2.0];
/// let mut b = [3.0, 4.0];
/// swap(&mut a, &mut b).unwrap();
/// assert_eq!(a, [3.0, 4.0]);
/// assert_eq!(b, [1.0, 2.0]);
/// ```
pub fn swap(x: &mut [f64], y: &mut [f64]) -> Result<()> {
    if x.len() != y.len() {
        return Err(PetirError::LengthMismatch {
            expected: x.len(),
            found: y.len(),
        });
    }
    for (xi, yi) in x.iter_mut().zip(y.iter_mut()) {
        core::mem::swap(xi, yi);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dot_matches_the_hand_computed_value() {
        assert_eq!(dot(&[1.0, 2.0, 3.0], &[4.0, 5.0, 6.0]).unwrap(), 32.0);
        assert_eq!(dot(&[], &[]).unwrap(), 0.0);
    }

    #[test]
    fn dot_reports_a_length_mismatch_rather_than_truncating() {
        let e = dot(&[1.0, 2.0], &[1.0]).unwrap_err();
        assert_eq!(
            e,
            PetirError::LengthMismatch {
                expected: 2,
                found: 1
            }
        );
    }

    #[test]
    fn nrm2_matches_the_pythagorean_answer() {
        assert_eq!(nrm2(&[3.0, 4.0]), 5.0);
        assert_eq!(nrm2(&[]), 0.0);
        assert_eq!(nrm2(&[-7.0]), 7.0);
        let v = [1.0, 2.0, 3.0, 4.0];
        assert!((nrm2(&v) - 30.0_f64.sqrt()).abs() < 1e-15);
    }

    /// The reason `nrm2` exists rather than `dot(x, x).sqrt()`.
    #[test]
    fn nrm2_survives_the_overflow_and_underflow_the_naive_form_does_not() {
        // Naive: 1e200^2 = inf.
        assert!(dot(&[1e200, 0.0], &[1e200, 0.0]).unwrap().is_infinite());
        assert_eq!(nrm2(&[1e200, 0.0]), 1e200);

        // Naive: 1e-200^2 underflows to 0.
        assert_eq!(dot(&[1e-200, 0.0], &[1e-200, 0.0]).unwrap(), 0.0);
        assert_eq!(nrm2(&[1e-200, 0.0]), 1e-200);

        // And the scaled form still gets the ordinary answer right when the
        // entries span a wide range.
        let got = nrm2(&[3e200, 4e200]);
        assert!((got - 5e200).abs() / 5e200 < 1e-15, "got {got}");
    }

    #[test]
    fn nrm2_is_invariant_to_ordering_and_sign() {
        let a = nrm2(&[1.0, -2.0, 3.0, -4.0]);
        let b = nrm2(&[4.0, 3.0, 2.0, 1.0]);
        assert!((a - b).abs() < 1e-15);
    }

    #[test]
    fn asum_is_the_one_norm() {
        assert_eq!(asum(&[1.0, -2.0, 3.0]), 6.0);
        assert_eq!(asum(&[]), 0.0);
    }

    #[test]
    fn axpy_accumulates_in_place() {
        let mut y = [1.0, 1.0, 1.0];
        axpy(2.0, &[1.0, 2.0, 3.0], &mut y).unwrap();
        assert_eq!(y, [3.0, 5.0, 7.0]);
    }

    /// The alpha == 0 short circuit is a behavioural contract, not just an
    /// optimisation: it must leave `y` bit-identical, NaNs in `x` included.
    #[test]
    fn axpy_with_zero_alpha_leaves_y_untouched_even_against_nan() {
        let mut y = [1.0, -0.0, 3.0];
        axpy(0.0, &[f64::NAN, f64::INFINITY, 1.0], &mut y).unwrap();
        assert_eq!(y[0], 1.0);
        assert!(y[1].is_sign_negative(), "-0.0 must survive untouched");
        assert_eq!(y[2], 3.0);
    }

    #[test]
    fn axpy_with_unit_alpha_is_plain_addition() {
        let mut y = [1.0, 2.0];
        axpy(1.0, &[10.0, 20.0], &mut y).unwrap();
        assert_eq!(y, [11.0, 22.0]);
    }

    #[test]
    fn axpy_reports_a_length_mismatch() {
        let mut y = [1.0];
        assert!(axpy(1.0, &[1.0, 2.0], &mut y).is_err());
    }

    #[test]
    fn scal_multiplies_in_place_and_short_circuits_on_one() {
        let mut x = [1.0, 2.0, 3.0];
        scal(3.0, &mut x);
        assert_eq!(x, [3.0, 6.0, 9.0]);
        let mut y = [-0.0, 1.0];
        scal(1.0, &mut y);
        assert!(y[0].is_sign_negative(), "alpha == 1 must not touch -0.0");
        let mut z = [1.0, 2.0];
        scal(0.0, &mut z);
        assert_eq!(z, [0.0, 0.0]);
    }

    /// Tie-breaking is the whole contract here.
    #[test]
    fn iamax_returns_the_first_index_among_ties() {
        assert_eq!(iamax(&[1.0, -5.0, 3.0]), Some(1));
        assert_eq!(iamax(&[2.0, -2.0]), Some(0));
        assert_eq!(iamax(&[-2.0, 2.0, -2.0]), Some(0));
        assert_eq!(iamax(&[7.0]), Some(0));
        assert_eq!(iamax(&[]), None);
    }

    #[test]
    fn iamax_compares_magnitudes_not_values() {
        assert_eq!(iamax(&[1.0, -100.0, 50.0]), Some(1));
    }

    #[test]
    fn swap_exchanges_both_ways() {
        let mut a = [1.0, 2.0];
        let mut b = [3.0, 4.0];
        swap(&mut a, &mut b).unwrap();
        assert_eq!(a, [3.0, 4.0]);
        assert_eq!(b, [1.0, 2.0]);
        assert!(swap(&mut a, &mut [0.0]).is_err());
    }
}
