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
// Copyright (C) 1996-2000 Gerard Jungman, Brian Gough
// GPL-3.0-or-later (verified from the per-file header of linalg/householder.c;
// see NOTICE).
//
//   linalg/householder.c:50   gsl_linalg_householder_transform
//   linalg/householder.c:162  gsl_linalg_householder_hm
//   linalg/householder.c:298  gsl_linalg_householder_hv

//! Householder reflections — GSL's `linalg/householder.c`.
//!
//! A Householder reflection is `P = I - tau v v^T`, chosen so that `P x` is a
//! multiple of `e_1`. Applying one to each column in turn is what reduces a
//! matrix to upper triangular form, which is the QR factorisation in
//! [`crate::linalg::qr`].
//!
//! # Storage convention, which matters when reading the code
//!
//! Upstream stores `v` with an **implicit leading 1**: after
//! [`transform`], `v[0]` holds `beta` (the new leading entry of the reduced
//! column), not `v[0]` itself, and every routine that *applies* the reflection
//! treats the first component as `1`. GSL's own comment warns about this, and
//! it is the single easiest thing to get wrong when reading these functions.
//! The loops below therefore start at index 1 and handle index 0 separately —
//! that is upstream's shape, not an optimisation.
//!
//! # Which branch of `hm` this is
//!
//! `gsl_linalg_householder_hm` has two implementations behind `#ifdef
//! USE_BLAS`, and **they round differently**: the BLAS branch accumulates
//! `w_j` with `ddot` over rows `1..m` and adds `A[0][j]` last, while the
//! plain branch seeds `w_j` with `A[0][j]` and adds the rest to it. GSL's
//! build system never defines `USE_BLAS` (checked in `configure.ac`,
//! `config.h.in` and the `Makefile.am`s — no definition), so the plain branch
//! is what GSL actually compiles, and it is what is ported here.
//!
//! # Units
//!
//! Bare dimensionless `f64`.

use crate::linalg::blas1;
use crate::linalg::Matrix;
use crate::scalar::{DBL_EPSILON as GSL_DBL_EPSILON, DBL_MIN as GSL_DBL_MIN};
#[allow(unused_imports)]
use crate::real::Real;

/// `GSL_SIGN`: `+1` for a non-negative argument, `-1` otherwise.
///
/// Note this is **not** `f64::signum`, which returns `-0.0`'s sign as `-1.0`
/// and maps `NaN` to `NaN`. Upstream's macro is a plain comparison and the
/// choice of branch at `alpha == 0.0` decides the sign of `beta`.
#[inline]
fn gsl_sign(x: f64) -> f64 {
    if x >= 0.0 {
        1.0
    } else {
        -1.0
    }
}

/// Replace `v` with a Householder vector annihilating `v[1..]`, returning
/// `tau`.
///
/// Translates `gsl_linalg_householder_transform` (`linalg/householder.c:50`).
/// On return `v[0]` holds `beta` and `v[1..]` the reflection vector's tail,
/// with the leading `1` implicit (see the module docs).
///
/// `tau` is `0.0` when no reflection is needed: a length-1 vector, or a tail
/// that is already exactly zero.
///
/// # The `DBL_MIN` branch is not decoration
///
/// `v[1..]` is scaled by `1 / (alpha - beta)`. When `alpha` and `beta` nearly
/// cancel that divisor underflows to a subnormal and the scaling overflows to
/// infinity. Upstream splits the scaling into two steps through
/// `DBL_EPSILON` in that case, which is a real guard against a real failure
/// and is ported rather than simplified away.
pub fn transform(v: &mut [f64]) -> f64 {
    let Some((alpha_slot, x)) = v.split_first_mut() else {
        // n == 0. Upstream cannot be called with an empty vector; an absent
        // reflection is tau = 0, which is the identity.
        return 0.0;
    };
    if x.is_empty() {
        return 0.0; // n == 1, tau = 0
    }

    let xnorm = blas1::nrm2(x);
    if xnorm == 0.0 {
        return 0.0; // tau = 0
    }

    let alpha = *alpha_slot;
    let beta = -gsl_sign(alpha) * alpha.hypot(xnorm);
    let tau = (beta - alpha) / beta;

    let s = alpha - beta;
    if s.abs() > GSL_DBL_MIN {
        blas1::scal(1.0 / s, x);
    } else {
        blas1::scal(GSL_DBL_EPSILON / s, x);
        blas1::scal(1.0 / GSL_DBL_EPSILON, x);
    }
    *alpha_slot = beta;

    tau
}

/// Apply `P = I - tau v v^T` from the left to the submatrix of `a` whose
/// top-left corner is `(row0, col0)`.
///
/// Translates `gsl_linalg_householder_hm` (`linalg/householder.c:162`), the
/// non-`USE_BLAS` branch — see the module docs for why that one.
///
/// `v` is indexed from the submatrix's first row, with `v[0]` implicitly `1`.
/// A `tau` of zero is the identity and returns immediately, as upstream does.
pub fn hm(tau: f64, v: &[f64], a: &mut Matrix, row0: usize, col0: usize) {
    if tau == 0.0 {
        return;
    }
    let rows = a.rows();
    let cols = a.cols();

    for j in col0..cols {
        // Compute w_j = sum_i A[i][j] v[i], with v[0] = 1 -- so the i = 0 term
        // is A[row0][j] itself and seeds the sum.
        let mut wj = a.get(row0, j);
        for (i, &vi) in (row0 + 1..rows).zip(v.iter().skip(1)) {
            wj += a.get(i, j) * vi;
        }

        // A[i][j] -= tau v[i] w_j, again with the i = 0 term separate.
        let a0j = a.get(row0, j);
        a.set(row0, j, a0j - tau * wj);
        for (i, &vi) in (row0 + 1..rows).zip(v.iter().skip(1)) {
            let aij = a.get(i, j);
            a.set(i, j, aij - tau * vi * wj);
        }
    }
}

/// Apply `P = I - tau v v^T` to the vector `w`, in place.
///
/// Translates `gsl_linalg_householder_hv` (`linalg/householder.c:298`).
/// `v[0]` is implicitly `1`, so `v' w = w[0] + v[1..]' w[1..]`.
pub fn hv(tau: f64, v: &[f64], w: &mut [f64]) {
    if tau == 0.0 {
        return;
    }
    let (Some((_, v1)), Some((w0_slot, w1))) = (v.split_first(), w.split_first_mut()) else {
        return;
    };

    // d1 = v(2:n)' w(2:n); d = v'w = w(1) + d1 since v(1) = 1.
    let mut d1 = 0.0_f64;
    for (&vi, &wi) in v1.iter().zip(w1.iter()) {
        d1 += vi * wi;
    }
    let d = *w0_slot + d1;

    // w = w - tau v (v'w)
    *w0_slot -= tau * d;
    let alpha = -tau * d;
    if alpha != 0.0 {
        // Upstream reaches `gsl_blas_daxpy`, which returns early for a zero
        // multiplier rather than adding 0.0 -- kept, because adding 0.0 would
        // turn a -0.0 in w1 into +0.0.
        for (wi, &vi) in w1.iter_mut().zip(v1.iter()) {
            *wi += alpha * vi;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;
    use alloc::vec::Vec;

    /// After `transform`, applying the reflection to the original vector must
    /// annihilate everything below the first entry, leaving `beta` on top.
    ///
    /// # Methodology
    ///
    /// Take `x = [3, 4]`, whose norm is exactly 5. Run `transform`, then apply
    /// the resulting reflection to a fresh copy of `x` with [`hv`] and check
    /// the tail is zero and the head is `beta`.
    ///
    /// # Results
    ///
    /// `beta = -5` exactly (the sign is opposite `alpha = 3`, per `GSL_SIGN`),
    /// and the reflected tail is `0` to within 1e-15. Interpretation: the
    /// reflection is the one upstream intends, including the sign convention
    /// that avoids cancellation.
    #[test]
    fn a_reflection_annihilates_the_tail_it_was_built_from() {
        let x = vec![3.0_f64, 4.0];
        let mut v = x.clone();
        let tau = transform(&mut v);

        let beta = v.first().copied().unwrap_or(f64::NAN);
        assert!(
            (beta + 5.0).abs() < 1e-15,
            "beta should be -|x| = -5, got {beta}"
        );

        let mut w = x.clone();
        hv(tau, &v, &mut w);
        let head = w.first().copied().unwrap_or(f64::NAN);
        let tail = w.get(1).copied().unwrap_or(f64::NAN);
        assert!(
            (head - beta).abs() < 1e-14,
            "head {head} should be beta {beta}"
        );
        assert!(tail.abs() < 1e-15, "tail should be annihilated, got {tail}");
    }

    /// A reflection is orthogonal, so it preserves the Euclidean norm.
    #[test]
    fn a_reflection_preserves_the_norm() {
        let x = vec![1.0_f64, -2.0, 0.5, 4.0];
        let mut v = x.clone();
        let tau = transform(&mut v);
        let mut w = vec![2.0_f64, 1.0, -1.0, 3.0];
        let before = blas1::nrm2(&w);
        hv(tau, &v, &mut w);
        let after = blas1::nrm2(&w);
        assert!(
            (before - after).abs() < 1e-13,
            "norm {before} -> {after} under an orthogonal map"
        );
    }

    /// Degenerate inputs return tau = 0, the identity, exactly as upstream.
    #[test]
    fn degenerate_inputs_give_the_identity() {
        assert_eq!(transform(&mut []), 0.0);
        assert_eq!(transform(&mut [2.0]), 0.0);
        // An already-annihilated tail needs no reflection.
        let mut v = vec![7.0, 0.0, 0.0];
        assert_eq!(transform(&mut v), 0.0);
        assert_eq!(v, vec![7.0, 0.0, 0.0], "v is left untouched when tau = 0");
    }

    /// `hm` applied to a one-column matrix must agree with `hv` on the same
    /// data -- they are the same reflection, and upstream implements them as
    /// separate loops.
    #[test]
    fn hm_and_hv_agree_on_a_single_column() {
        let x = vec![2.0_f64, -1.0, 3.0];
        let mut v = x.clone();
        let tau = transform(&mut v);

        let mut w = vec![1.0_f64, 5.0, -2.0];
        let mut a = Matrix::from_row_major(3, 1, w.clone()).unwrap();

        hv(tau, &v, &mut w);
        hm(tau, &v, &mut a, 0, 0);

        let got: Vec<f64> = (0..3).map(|i| a.get(i, 0)).collect();
        for (&expected, &actual) in w.iter().zip(got.iter()) {
            assert!(
                (expected - actual).abs() < 1e-14,
                "hv gave {expected}, hm gave {actual}"
            );
        }
    }

    /// `GSL_SIGN` is not `f64::signum`, and the difference is load-bearing.
    #[test]
    fn gsl_sign_treats_zero_as_positive() {
        assert_eq!(gsl_sign(0.0), 1.0);
        assert_eq!(
            gsl_sign(-0.0),
            1.0,
            "GSL_SIGN tests >= 0, so -0.0 is positive"
        );
        assert_eq!((-0.0_f64).signum(), -1.0, "...unlike signum, for contrast");
        assert_eq!(gsl_sign(-3.0), -1.0);
    }
}
