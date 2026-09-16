// PROVENANCE
//   Upstream project: CYCLUS <https://github.com/cyclus/cyclus>
//   Upstream file:    src/cyc_limits.h.in
//   Upstream commit:  d4faab7ce0566ccb8febfcf50915cdeedee59db0
//   Upstream licence: BSD-3-Clause
//   Copyright (c) 2010-2016, University of Wisconsin Computational Nuclear
//   Engineering Research Group. All rights reserved.
//
//   Independent Rust translation, (C) 2026 Theodore Ong and the outram-park
//   contributors, GPL-3.0-only. Relicensing is ONE-WAY.

//! Floating-point tolerances, translated from `cyc_limits.h.in`.
//!
//! # Why these are constants here and mutable globals upstream
//!
//! Upstream declares `extern double cy_eps` and `extern double cy_eps_rsrc` —
//! process-global mutable state, settable from the Python bindings at run
//! time. That does not survive the translation for two reasons, and the
//! difference is worth stating because it is a genuine behavioural change:
//!
//!  1. A mutable global would need a `static mut` or a lock, and in `no_std`
//!     there is no `OnceLock` to reach for.
//!  2. A tolerance that can change underneath a running simulation makes two
//!     runs of the same input irreproducible, which defeats the point of
//!     building this on PETIR's bit-identical arithmetic in the first place.
//!
//! The values are upstream's compiled-in defaults, taken from
//! `CMakeLists.txt`'s `CY_NEAR_ZERO`/`cy_eps` initialisation. A caller who
//! needs a different tolerance passes it explicitly — every function in this
//! crate that compares against a tolerance takes it as an argument, and these
//! constants are what the convenience wrappers supply.

/// Generic epsilon. Upstream `cyclus::eps()`, default `1e-6`.
///
/// Used for comparing dimensionless quantities — preferences, capacities,
/// exchange-graph flows.
pub const EPS: f64 = 1e-6;

/// Resource epsilon. Upstream `cyclus::eps_rsrc()`, default `1e-6` kg.
///
/// Used for comparing resource quantities in kilograms. It is a separate
/// constant from [`EPS`] upstream — and stays separate here — because the two
/// are dimensionally different things that merely happen to share a value, and
/// collapsing them would hide that from anyone who later wants to tighten one.
pub const EPS_RSRC: f64 = 1e-6;

/// Distance in ULPs within which two floats count as equal.
///
/// Upstream `float_ulp_eq = 2`. This governs the exclusive-order arithmetic in
/// the exchange solver, where upstream's comment is emphatic that the careful
/// float comparison "is vital for preventing false positive constraint
/// violations w.r.t. exclusivity-related capacity". See
/// [`float_distance`](crate::limits::float_distance).
pub const FLOAT_ULP_EQ: f64 = 2.0;

/// Maximum value for a function modifier (upstream `kModifierLimit`, `1e10`).
pub const MODIFIER_LIMIT: f64 = 1e10;

/// Upstream `CY_LARGE_INT`. A stand-in for "unbounded" in integer-valued
/// capacities; upstream sets it from CMake to the largest `int` it can use
/// without overflow in downstream arithmetic.
pub const CY_LARGE_INT: i32 = i32::MAX;

/// Upstream `CY_LARGE_DOUBLE`. A stand-in for "unbounded" in real-valued
/// capacities.
///
/// Note that upstream's greedy solver *also* compares group capacities against
/// `std::numeric_limits<double>::max()` directly, as its special case for an
/// unlimited capacity. [`UNLIMITED`] is that value; the two are deliberately
/// distinct and must not be merged.
pub const CY_LARGE_DOUBLE: f64 = 1e299;

/// Upstream `CY_NEAR_ZERO`.
pub const CY_NEAR_ZERO: f64 = 1e-8;

/// The sentinel an exchange capacity carries to mean "unbounded".
///
/// This is `std::numeric_limits<double>::max()`, which the greedy solver tests
/// for by exact equality in both [`capacity`](crate::exchange::greedy) and its
/// capacity-update step. Exact-equality comparison of a float is normally a
/// defect; here it is upstream's deliberate sentinel protocol and is preserved
/// as such.
pub const UNLIMITED: f64 = f64::MAX;

/// Absolute value, `no_std`-safe.
///
/// `f64::abs` is a `std`-only inherent method. Rather than depend on `libm`
/// directly for one sign flip, this does it arithmetically.
#[inline]
#[must_use]
pub fn abs(x: f64) -> f64 {
    if x < 0.0 {
        -x
    } else {
        x
    }
}

/// Returns `true` if `d` is less than `-EPS`. Upstream `cyclus::IsNegative`.
///
/// Note this is *not* `d < 0.0`: a value in `(-EPS, 0)` is treated as zero,
/// which is what lets the resource layer tolerate the rounding that
/// accumulates over a long simulation without reporting a negative inventory.
#[inline]
#[must_use]
pub fn is_negative(d: f64) -> bool {
    d < -EPS
}

/// Returns `true` if `d1` and `d2` are within [`EPS`]. Upstream
/// `cyclus::AlmostEq`.
#[inline]
#[must_use]
pub fn almost_eq(d1: f64, d2: f64) -> bool {
    abs(d1 - d2) < EPS
}

/// The number of representable `f64` values between `a` and `b`, signed.
///
/// This is the translation of `boost::math::float_distance`, which upstream
/// uses in exactly two places — [`Arc`](crate::exchange::graph::Arc)
/// construction and the greedy solver's exclusive-order adjustment — and
/// nowhere else. Boost is not available here and pulling in a floating-point
/// utility crate for one function would be a heavier dependency than the
/// function.
///
/// # How it works
///
/// IEEE-754 binary64 is ordered such that, for two finite values of the same
/// sign, reinterpreting the bit patterns as `i64` and subtracting gives
/// exactly the count of representable values between them. Values of opposite
/// sign are handled by mapping each to a monotone key first. Boost documents
/// the same construction.
///
/// # Returns
///
/// `b - a` measured in ULPs, positive when `b > a`. Returns `0.0` when either
/// argument is NaN, which matches how the two call sites behave: a NaN
/// quantity there is already a bug upstream of this comparison, and returning
/// `0` makes it surface as a rejected match rather than a panic.
///
/// # Examples
///
/// ```
/// use outram_park_fork_cyclus::limits::float_distance;
///
/// assert_eq!(float_distance(1.0, 1.0), 0.0);
/// assert_eq!(float_distance(1.0, f64::from_bits(1.0f64.to_bits() + 1)), 1.0);
/// assert_eq!(float_distance(f64::from_bits(1.0f64.to_bits() + 1), 1.0), -1.0);
/// ```
#[must_use]
pub fn float_distance(a: f64, b: f64) -> f64 {
    if a.is_nan() || b.is_nan() {
        return 0.0;
    }
    // Map each value onto a monotonically increasing i64 key. For a
    // non-negative float the raw bit pattern is already monotone; for a
    // negative one it is monotone decreasing, so it is reflected about
    // i64::MIN.
    #[inline]
    fn key(x: f64) -> i64 {
        let bits = x.to_bits() as i64;
        if bits < 0 {
            i64::MIN - bits
        } else {
            bits
        }
    }
    (key(b) - key(a)) as f64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_negative_tolerates_epsilon_sized_rounding() {
        assert!(!is_negative(-EPS / 2.0));
        assert!(is_negative(-EPS * 2.0));
        assert!(!is_negative(0.0));
        assert!(!is_negative(1.0));
    }

    #[test]
    fn float_distance_counts_representable_values() {
        assert_eq!(float_distance(1.0, 1.0), 0.0);
        let next = f64::from_bits(1.0f64.to_bits() + 1);
        assert_eq!(float_distance(1.0, next), 1.0);
        assert_eq!(float_distance(next, 1.0), -1.0);
    }

    #[test]
    fn float_distance_crosses_zero_symmetrically() {
        // +0.0 and -0.0 are distinct bit patterns but adjacent in the ordering.
        assert_eq!(float_distance(-0.0, 0.0), 0.0);
        let tiny = f64::from_bits(1); // smallest positive subnormal
        assert_eq!(float_distance(-tiny, tiny), 2.0);
    }

    #[test]
    fn float_distance_is_zero_for_nan() {
        assert_eq!(float_distance(f64::NAN, 1.0), 0.0);
        assert_eq!(float_distance(1.0, f64::NAN), 0.0);
    }

    #[test]
    fn almost_eq_matches_upstream_tolerance() {
        assert!(almost_eq(1.0, 1.0 + EPS / 2.0));
        assert!(!almost_eq(1.0, 1.0 + EPS * 2.0));
    }
}
