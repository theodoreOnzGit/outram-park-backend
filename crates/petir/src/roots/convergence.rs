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
// Copyright (C) 1996-2000 Reid Priedhorsky, Brian Gough
// GPL-3.0-or-later (verified from the per-file headers; see NOTICE).
//
//   roots/convergence.c  gsl_root_test_interval -> test_interval
//                        gsl_root_test_delta    -> test_delta
//                        gsl_root_test_residual -> test_residual

//! Convergence tests for the one-dimensional root finders.
//!
//! # Why these are separate from the solvers
//!
//! GSL deliberately does not decide when you are done. The solver takes one
//! step; you decide whether that step was good enough. That separation is worth
//! preserving, because the right test depends on what the root is *for*:
//!
//! - [`test_interval`] — the bracket is narrow enough. The only one of the
//!   three that actually **guarantees** the root's location, because a sign
//!   change inside a bracket is a proof.
//! - [`test_delta`] — successive iterates stopped moving. Cheap, works for
//!   non-bracketing methods, and is **not** a guarantee: a slowly converging
//!   iteration can creep and satisfy this while still far from the root.
//! - [`test_residual`] — `|f(x)|` is small. Says nothing about `x` on its own:
//!   for a function with a shallow slope near its root, `|f|` can be `1e-15`
//!   while `x` is off by `1e-3`. Useful when small residual is the actual
//!   requirement, misleading when it is being used as a proxy for accuracy.
//!
//! # Mixed absolute and relative tolerance
//!
//! All three take `epsabs`, and the first two also take `epsrel`, combining
//! them as `epsabs + epsrel * scale`. That combination is what makes a single
//! tolerance work across magnitudes: a pure relative test cannot converge on a
//! root at zero, and a pure absolute test is meaningless for a root near
//! `1e12`. Pass `epsrel = 0.0` for a purely absolute test.

// Under a std-linked build (`cargo test`) f64's inherent abs shadows this
// trait, leaving the import formally unused. See crate::real.
#[allow(unused_imports)]
use crate::real::Real;

use crate::{PetirError, Result};

/// Whether a convergence test is satisfied.
///
/// Stands in for GSL's `GSL_SUCCESS` / `GSL_CONTINUE` return pair, which is a
/// distinction the C API makes by overloading the error channel. Making it a
/// separate type means "not converged yet" cannot be mistaken for a failure, or
/// silently ignored the way a discarded `int` can be.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Convergence {
    /// The test is satisfied; stop iterating.
    Converged,
    /// Not there yet; iterate again.
    Continue,
}

impl Convergence {
    /// `true` if the test is satisfied.
    pub fn is_converged(self) -> bool {
        matches!(self, Convergence::Converged)
    }
}

/// Test whether a bracketing interval is narrow enough.
///
/// Converged when `|x_upper - x_lower| < epsabs + epsrel * min(|x_lower|, |x_upper|)`,
/// where the relative term uses the smaller magnitude **only when both
/// endpoints have the same sign**. When the interval straddles zero the
/// relative term is dropped entirely — which is right, because "within 1 % of
/// a root that might be zero" is not a meaningful requirement.
///
/// # Errors
///
/// [`PetirError::Tolerance`] if either tolerance is negative;
/// [`PetirError::Invalid`] if `x_lower > x_upper`.
///
/// Translates GSL's `gsl_root_test_interval`.
///
/// # Example
///
/// ```
/// use petir::roots::{test_interval, Convergence};
/// assert_eq!(test_interval(1.0, 1.0 + 1e-9, 1e-6, 0.0).unwrap(), Convergence::Converged);
/// assert_eq!(test_interval(0.0, 1.0, 1e-6, 0.0).unwrap(), Convergence::Continue);
/// ```
pub fn test_interval(x_lower: f64, x_upper: f64, epsabs: f64, epsrel: f64) -> Result<Convergence> {
    if epsrel < 0.0 || epsabs < 0.0 {
        return Err(PetirError::Tolerance);
    }
    if x_lower > x_upper {
        return Err(PetirError::Invalid);
    }

    let abs_lower = x_lower.abs();
    let abs_upper = x_upper.abs();

    // The relative term only applies when the interval does not straddle zero.
    let min_abs = if (x_lower > 0.0 && x_upper > 0.0) || (x_lower < 0.0 && x_upper < 0.0) {
        if abs_lower < abs_upper {
            abs_lower
        } else {
            abs_upper
        }
    } else {
        0.0
    };

    let tolerance = epsabs + epsrel * min_abs;

    if (x_upper - x_lower).abs() < tolerance {
        Ok(Convergence::Converged)
    } else {
        Ok(Convergence::Continue)
    }
}

/// Test whether successive iterates have stopped moving.
///
/// Converged when `|x1 - x0| < epsabs + epsrel * |x1|`, or when the two are
/// bit-identical.
///
/// # What this does not tell you
///
/// That the iteration has *converged*, only that it has *stalled*. A method
/// converging linearly with ratio 0.999 satisfies this long before it reaches
/// the root. Where a guarantee is wanted, bracket the root and use
/// [`test_interval`] instead.
///
/// # Errors
///
/// [`PetirError::Tolerance`] if either tolerance is negative.
///
/// Translates GSL's `gsl_root_test_delta`.
pub fn test_delta(x1: f64, x0: f64, epsabs: f64, epsrel: f64) -> Result<Convergence> {
    if epsrel < 0.0 || epsabs < 0.0 {
        return Err(PetirError::Tolerance);
    }
    let tolerance = epsabs + epsrel * x1.abs();
    if (x1 - x0).abs() < tolerance || x1 == x0 {
        Ok(Convergence::Converged)
    } else {
        Ok(Convergence::Continue)
    }
}

/// Test whether the residual `|f|` is below `epsabs`.
///
/// # Read the caveat before using this as an accuracy test
///
/// A small residual does not imply an accurate root. Near a root of
/// multiplicity two, or wherever `f'` is small, `|f(x)|` falls off far faster
/// than `|x - root|`: `f(x) = (x - 1)^2` has `|f| = 1e-12` at `x = 1 + 1e-6`.
/// If what you need is an accurate `x`, this is the wrong test.
///
/// It is the right test when small residual is itself the requirement — for
/// instance when the root feeds a balance equation whose imbalance is what
/// matters.
///
/// # Errors
///
/// [`PetirError::Tolerance`] if `epsabs` is negative.
///
/// Translates GSL's `gsl_root_test_residual`.
pub fn test_residual(f: f64, epsabs: f64) -> Result<Convergence> {
    if epsabs < 0.0 {
        return Err(PetirError::Tolerance);
    }
    if f.abs() < epsabs {
        Ok(Convergence::Converged)
    } else {
        Ok(Convergence::Continue)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interval_test_converges_when_narrow() {
        assert_eq!(
            test_interval(1.0, 1.0 + 1e-9, 1e-6, 0.0).unwrap(),
            Convergence::Converged
        );
        assert_eq!(
            test_interval(0.0, 1.0, 1e-6, 0.0).unwrap(),
            Convergence::Continue
        );
    }

    /// The straddling-zero rule is the subtle part of this routine.
    #[test]
    fn relative_tolerance_is_dropped_when_the_interval_straddles_zero() {
        // Same sign: the relative term applies, using the SMALLER magnitude.
        // width 1.0, min|x| = 100 -> tolerance = 0.1 * 100 = 10 > 1 -> converged
        assert_eq!(
            test_interval(100.0, 101.0, 0.0, 0.1).unwrap(),
            Convergence::Converged
        );
        // Straddling zero: min_abs is forced to 0, so the relative term
        // contributes nothing and a pure-relative test can never converge.
        assert_eq!(
            test_interval(-100.0, 101.0, 0.0, 0.1).unwrap(),
            Convergence::Continue
        );
        // Negative endpoints are still "same sign".
        assert_eq!(
            test_interval(-101.0, -100.0, 0.0, 0.1).unwrap(),
            Convergence::Converged
        );
    }

    #[test]
    fn interval_test_rejects_bad_input() {
        assert_eq!(
            test_interval(0.0, 1.0, -1.0, 0.0).unwrap_err(),
            PetirError::Tolerance
        );
        assert_eq!(
            test_interval(0.0, 1.0, 0.0, -1.0).unwrap_err(),
            PetirError::Tolerance
        );
        assert_eq!(
            test_interval(1.0, 0.0, 1e-6, 0.0).unwrap_err(),
            PetirError::Invalid
        );
    }

    #[test]
    fn delta_test_converges_on_a_stalled_iteration() {
        assert_eq!(
            test_delta(1.0, 1.0 + 1e-12, 1e-9, 0.0).unwrap(),
            Convergence::Converged
        );
        assert_eq!(
            test_delta(1.0, 2.0, 1e-9, 0.0).unwrap(),
            Convergence::Continue
        );
        // Bit-identical iterates converge regardless of tolerance.
        assert_eq!(test_delta(3.0, 3.0, 0.0, 0.0).unwrap(), Convergence::Converged);
    }

    #[test]
    fn delta_tolerance_scales_with_the_iterate() {
        // 1 % of 1000 is 10, so a step of 5 counts as converged.
        assert_eq!(
            test_delta(1000.0, 1005.0, 0.0, 0.01).unwrap(),
            Convergence::Converged
        );
        // The same step at magnitude 1 does not.
        assert_eq!(
            test_delta(1.0, 6.0, 0.0, 0.01).unwrap(),
            Convergence::Continue
        );
    }

    #[test]
    fn residual_test_examines_only_the_function_value() {
        assert_eq!(test_residual(1e-12, 1e-9).unwrap(), Convergence::Converged);
        assert_eq!(test_residual(-1e-12, 1e-9).unwrap(), Convergence::Converged);
        assert_eq!(test_residual(1.0, 1e-9).unwrap(), Convergence::Continue);
        assert_eq!(
            test_residual(0.0, -1.0).unwrap_err(),
            PetirError::Tolerance
        );
    }

    /// Demonstrates the documented trap, so the doc comment is not just an
    /// assertion: a tiny residual coexisting with a poor root.
    #[test]
    fn a_small_residual_can_coexist_with_an_inaccurate_root() {
        // f(x) = (x - 1)^2 at x = 1 + 1e-6 gives f = 1e-12.
        let x = 1.0 + 1e-6_f64;
        let f = (x - 1.0) * (x - 1.0);
        assert_eq!(test_residual(f, 1e-9).unwrap(), Convergence::Converged);
        // ...yet x is wrong in the sixth decimal place.
        assert!((x - 1.0).abs() > 1e-7);
    }
}
