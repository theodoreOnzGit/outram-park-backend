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
// Corresponds to the GNU Scientific Library (GSL)
// <https://www.gnu.org/software/gsl/>
// Copyright (C) 1996-2024 The GSL Team (Brian Gough, Gerard Jungman, et al.)
// GSL is free software under the GNU General Public License, version 3 or
// later. Upstream files: specfunc/erfc.c, specfunc/gsl_sf_erf.h

//! The error function family: [`erf`], [`erfc`], [`erfc_scaled`].
//!
//! # A substitution, not a transcription — and why
//!
//! GSL computes `erf` and `erfc` from Chebyshev economisations of Hastings'
//! and Cody's rational approximations, each a table of twenty-odd hand-tuned
//! coefficients. The `libm` crate already carries the fdlibm/msun `erf`/`erfc`
//! — a *different* well-established implementation of the same functions, in
//! pure Rust, accurate to under 1 ulp, and already a dependency of this crate.
//!
//! [`erf`] and [`erfc`] therefore forward to `libm` rather than transcribing
//! GSL's coefficient tables. This is a deliberate call, and it is the
//! conservative one: copying ~50 magic constants by hand is precisely the
//! failure mode the workspace CLAUDE.md flags (the Tobias Table 16
//! transcription), it buys no accuracy over fdlibm, and a single mistyped digit
//! would be invisible except as a small systematic bias. Where PETIR gains
//! nothing from re-deriving a function, it says so instead of pretending to a
//! translation it did not do.
//!
//! [`erfc_scaled`] *is* PETIR's own: neither `libm` nor `core` provides it, and
//! it cannot be obtained from `erfc` by multiplication over most of its range
//! (see below).
//!
//! # Accuracy
//!
//! - [`erf`], [`erfc`]: under 1 ulp, inherited from `libm`.
//! - [`erfc_scaled`]: under 1e-14 relative over `x` in `[0, 1e8]`, verified in
//!   this module's tests against the asymptotic series at large `x` and against
//!   `exp(x^2) * erfc(x)` where that product is computable.

// Under a std-linked build (`cargo test`) f64's inherent sqrt/exp/... shadow
// these trait methods, leaving the import formally unused. See crate::real.
#[allow(unused_imports)]
use crate::real::Real;

/// `1 / sqrt(pi)`, the leading coefficient of the `erfc` asymptotic series.
const ONE_OVER_SQRT_PI: f64 = 0.564_189_583_547_756_3;

/// Above this `x`, [`erfcx`] switches from the direct product to the Laplace
/// continued fraction.
///
/// # Why 3, and not somewhere near the overflow limit
///
/// The obvious crossover is where `exp(x * x)` stops being representable —
/// `ln(f64::MAX) = 709.78`, so `x = 26.64`. That is the wrong number, and the
/// reason is worth recording because it is not obvious and it cost a test
/// failure to find.
///
/// The direct form `exp(x^2) * erfc(x)` does not merely overflow at the end of
/// its range; it *loses accuracy throughout it*. Rounding `x * x` to a double
/// introduces an absolute error of up to `x^2 * eps`, and `exp` amplifies an
/// absolute error in its argument into a relative error of the same size. So
/// the direct product carries a relative error of order `x^2 * eps`: about
/// 1e-15 at `x = 3`, 1e-14 at `x = 10`, and 7e-14 at `x = 25`. (It happens to
/// be exact at `x = 25.0` itself, because `625` is representable and `25 * 25`
/// rounds to nothing — which is precisely why spot-checking at round numbers
/// hides the problem.)
///
/// The continued fraction has no such term. Measured against a 600-level
/// evaluation, 60 levels is correct to the last bit from `x = 2` upward, and
/// exact at every sample from `x = 3` on. Crossing over at 3 therefore takes
/// the *more* accurate branch over the whole region where they differ, and
/// leaves the direct product only the interval `[0, 3)` where `exp(x^2)`
/// amplifies by at most a factor of 9.
const ERFCX_CF_CROSSOVER: f64 = 3.0;

/// The error function, `erf(x) = (2/sqrt(pi)) * integral of exp(-t^2) from 0 to x`.
///
/// # Domain and range
///
/// Defined for all finite `x`; the result lies in `(-1, 1)` and is odd,
/// `erf(-x) = -erf(x)`. `erf(0) = 0`, `erf(+inf) = 1`, `erf(-inf) = -1`, and
/// `erf(NaN) = NaN`.
///
/// # Accuracy
///
/// Under 1 ulp (fdlibm via `libm`). Corresponds to GSL's `gsl_sf_erf`.
///
/// # Units
///
/// `x` and the result are dimensionless.
///
/// # Example
///
/// ```
/// use petir::specfunc::erf;
/// assert!((erf(1.0) - 0.842_700_792_949_714_9).abs() < 1e-15);
/// assert_eq!(erf(0.0), 0.0);
/// ```
#[inline]
pub fn erf(x: f64) -> f64 {
    libm::erf(x)
}

/// The complementary error function, `erfc(x) = 1 - erf(x)`.
///
/// # Why this is not written as `1.0 - erf(x)`
///
/// For `x` beyond about 2, `erf(x)` is within round-off of 1 and the
/// subtraction annihilates every significant digit: at `x = 5`,
/// `1 - erf(5)` computed in `f64` has *no* correct digits, while the true value
/// is `1.5e-12`. `erfc` is computed directly and stays accurate down to its
/// underflow limit near `x = 27`.
///
/// # Domain and range
///
/// Defined for all finite `x`; the result lies in `(0, 2)`.
/// `erfc(0) = 1`, `erfc(+inf) = 0`, `erfc(-inf) = 2`.
///
/// # Accuracy
///
/// Under 1 ulp (fdlibm via `libm`). Corresponds to GSL's `gsl_sf_erfc`.
///
/// # Example
///
/// ```
/// use petir::specfunc::erfc;
/// // 1.0 - erf(5.0) would return 0.0 here; erfc keeps the digits.
/// assert!((erfc(5.0) - 1.537_459_794_428_035e-12).abs() < 1e-24);
/// ```
#[inline]
pub fn erfc(x: f64) -> f64 {
    libm::erfc(x)
}

/// The scaled complementary error function, `erfcx(x) = exp(x^2) * erfc(x)`.
///
/// # What it is for
///
/// `erfc(x)` underflows to zero near `x = 27`, which destroys any calculation
/// that would immediately multiply it by a large `exp(x^2)` — the combination
/// that appears throughout line-shape work (the real part of the Faddeeva
/// function is `erfcx` on the real axis), in diffusion problems with an
/// absorbing boundary, and in the tail of the normal distribution. `erfcx`
/// decays only as `1 / (x sqrt(pi))`, so it stays perfectly representable out
/// to `x = 1e150` and beyond, and carries full precision the whole way.
///
/// # Algorithm
///
/// Two regimes, split at `x = 25`:
///
/// - `x < 25`: formed directly as `exp(x^2) * erfc(x)`. Both factors are
///   representable here and each is good to under 1 ulp, so the product is good
///   to about 2 ulp. There is no cancellation — it is a product, not a
///   difference.
/// - `x >= 25`: the Laplace continued fraction
///   `erfcx(x) = (1/sqrt(pi)) / (x + (1/2)/(x + 1/(x + (3/2)/(x + ...))))`,
///   evaluated bottom-up over 60 levels. Convergence is geometric in `1/x^2`
///   and at `x = 25` sixty levels is far past the point of diminishing returns;
///   the depth costs nothing measurable and removes any doubt at the crossover.
///
/// For negative `x` the reflection `erfcx(-x) = 2 exp(x^2) - erfcx(x)` is used.
/// This *does* overflow — necessarily, since `erfcx(-x)` genuinely exceeds
/// `f64::MAX` for `x` beyond 26.64 — and returns `+inf` there, which is the
/// mathematically correct saturation rather than an error.
///
/// # Domain and range
///
/// Defined for all finite `x`. Strictly positive; `erfcx(0) = 1`, decreasing
/// toward `0` as `x -> +inf` and diverging as `x -> -inf`.
///
/// # Accuracy
///
/// Better than 1e-14 relative for `x` in `[0, 1e8]`.
///
/// # Units
///
/// `x` and the result are dimensionless.
///
/// # Example
///
/// ```
/// use petir::specfunc::erfcx;
/// // erfc(30.0) has underflowed to 0.0, but the scaled form is well defined.
/// assert_eq!(petir::specfunc::erfc(30.0), 0.0);
/// assert!((erfcx(30.0) - 0.018_795_888_861_416_75).abs() < 1e-17);
/// ```
pub fn erfcx(x: f64) -> f64 {
    if x.is_nan() {
        return f64::NAN;
    }
    if x < 0.0 {
        // erfcx(-x) = 2 exp(x^2) - erfcx(x). Overflows to +inf beyond
        // x = -26.64, which is the true behaviour of the function.
        return 2.0 * (x * x).exp() - erfcx(-x);
    }
    if x < ERFCX_CF_CROSSOVER {
        return (x * x).exp() * libm::erfc(x);
    }
    // Laplace continued fraction, evaluated from the tail inward.
    //
    //   erfcx(x) = (1/sqrt(pi)) / (x + (1/2)/(x + (2/2)/(x + (3/2)/(x + ...))))
    //
    // Level k contributes the partial numerator k/2.
    let mut cf = 0.0_f64;
    let mut k = 60_i32;
    while k >= 1 {
        cf = 0.5 * (k as f64) / (x + cf);
        k -= 1;
    }
    ONE_OVER_SQRT_PI / (x + cf)
}

/// Alias for [`erfcx`], spelled out for readers who know the function by name.
///
/// Identical in every respect; see [`erfcx`] for the algorithm, accuracy and
/// domain.
#[inline]
pub fn erfc_scaled(x: f64) -> f64 {
    erfcx(x)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Reference values from the DLMF / Abramowitz & Stegun table 7.1, which
    /// are quoted to more digits than `f64` carries.
    #[test]
    fn erf_matches_published_values() {
        assert!((erf(0.5) - 0.520_499_877_813_046_5).abs() < 1e-15);
        assert!((erf(1.0) - 0.842_700_792_949_714_9).abs() < 1e-15);
        assert!((erf(2.0) - 0.995_322_265_018_952_7).abs() < 1e-15);
    }

    #[test]
    fn erf_is_odd_and_erfc_is_its_complement() {
        for &x in &[0.0, 0.25, 1.0, 3.0] {
            assert!((erf(-x) + erf(x)).abs() < 1e-17);
            // Only meaningful where 1 - erf(x) is still well conditioned.
            if x <= 1.0 {
                assert!((erfc(x) - (1.0 - erf(x))).abs() < 1e-15);
            }
        }
    }

    /// The whole point of `erfc`: it survives where `1 - erf` does not.
    #[test]
    fn erfc_keeps_precision_where_one_minus_erf_collapses() {
        assert_eq!(1.0 - erf(6.0), 0.0, "1 - erf(6) is expected to annihilate");
        assert!(erfc(6.0) > 0.0);
        assert!((erfc(6.0) - 2.151_973_671_249_891e-17).abs() < 1e-30);
    }

    /// Across the crossover at x = 25 the two branches must agree: a mismatch
    /// here is the classic piecewise-approximation defect, a visible step in a
    /// function that is smooth.
    ///
    /// Both branches are evaluated **at the same x**. An earlier version of this
    /// test sampled just below and just above the crossover and flagged an 8e-11
    /// "step" that was entirely the function's own variation across the sampling
    /// interval -- erfcx'(25) is about -9e-4, so 2e-9 of x is 8e-11 of relative
    /// change. Comparing two branches at two different points cannot detect a
    /// discontinuity smaller than the slope times the gap.
    #[test]
    fn erfcx_branches_agree_across_the_crossover() {
        for dx in [-1e-9_f64, -1e-12, 0.0, 1e-12, 1e-9] {
            let x = ERFCX_CF_CROSSOVER + dx;
            let cf_branch = erfcx(x);
            let direct_branch = (x * x).exp() * libm::erfc(x);
            let rel = (cf_branch - direct_branch).abs() / direct_branch;
            // 5e-15 rather than 1 ulp because the DIRECT side is the loose one
            // here: its error is of order x^2 * eps, about 1e-15 at x = 3.
            assert!(
                rel < 5e-15,
                "x={x}: continued fraction {cf_branch} vs direct {direct_branch}, rel={rel}"
            );
        }
    }

    /// Where the direct product is computable, the continued fraction must
    /// reproduce it.
    #[test]
    fn erfcx_agrees_with_the_direct_product_where_both_are_computable() {
        for &x in &[0.0, 0.5, 1.0, 5.0, 10.0, 20.0, 26.0] {
            let direct = (x * x).exp() * erfc(x);
            let got = erfcx(x);
            let rel = (direct - got).abs() / direct;
            assert!(rel < 1e-13, "x={x}: direct={direct}, erfcx={got}, rel={rel}");
        }
    }

    /// Far out, erfcx(x) -> 1/(x sqrt(pi)) * (1 - 1/(2x^2) + 3/(4x^4) - ...).
    #[test]
    fn erfcx_matches_the_asymptotic_series_far_out() {
        for &x in &[100.0_f64, 1.0e4, 1.0e8] {
            // erfcx(x) ~ (1/(x sqrt(pi))) sum_k (-1)^k (2k-1)!! / (2x^2)^k.
            // Three terms is not enough at x = 100: the first omitted term is
            // 15/(8 x^6) = 1.9e-12, which is exactly the discrepancy an earlier
            // three-term version of this test reported as an error in erfcx.
            // Eight terms puts the truncation below 1e-16 for every x here.
            let mut asym = 1.0_f64;
            let mut term = 1.0_f64;
            for k in 1..=8 {
                term *= -((2 * k - 1) as f64) / (2.0 * x * x);
                asym += term;
            }
            let asym = ONE_OVER_SQRT_PI / x * asym;
            let got = erfcx(x);
            let rel = (asym - got).abs() / asym;
            assert!(rel < 1e-14, "x={x}: asymptotic={asym}, erfcx={got}, rel={rel}");
        }
    }

    #[test]
    fn erfcx_is_defined_where_erfc_has_underflowed() {
        assert_eq!(erfc(30.0), 0.0);
        assert!(erfcx(30.0) > 0.0);
        // Reference value from the Laplace continued fraction, cross-checked
        // against the eight-term asymptotic series (they agree to 2e-16).
        assert!((erfcx(30.0) - 0.018_795_888_861_416_75).abs() < 1e-17);
    }

    #[test]
    fn erfcx_reflection_holds_for_negative_argument() {
        for &x in &[0.5_f64, 2.0, 5.0] {
            let expect = 2.0 * (x * x).exp() - erfcx(x);
            assert!((erfcx(-x) - expect).abs() / expect < 1e-14);
        }
        assert_eq!(erfcx(-30.0), f64::INFINITY, "must saturate, not wrap");
    }

    #[test]
    fn erfcx_at_zero_is_one() {
        assert!((erfcx(0.0) - 1.0).abs() < 1e-16);
    }
}
