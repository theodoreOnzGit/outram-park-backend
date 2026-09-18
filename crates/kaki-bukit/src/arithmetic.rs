// PROVENANCE
//   Upstream project: CYCLUS <https://github.com/cyclus/cyclus>
//   Upstream file:    src/cyc_arithmetic.h, src/cyc_arithmetic.cc
//   Upstream commit:  d4faab7ce0566ccb8febfcf50915cdeedee59db0
//   Upstream licence: BSD-3-Clause
//   Copyright (c) 2010-2016, University of Wisconsin Computational Nuclear
//   Engineering Research Group. All rights reserved.
//
//   Independent Rust translation, (C) 2026 Theodore Ong and the outram-park
//   contributors, GPL-3.0-only. Relicensing is ONE-WAY.

//! Compensated summation, translated from `CycArithmetic`.
//!
//! Cyclus sums composition vectors constantly — every normalisation, every
//! mass balance, every `Material::Absorb`. Over a long simulation a naive
//! `fold(0.0, Add::add)` accumulates enough rounding error to make an
//! inventory visibly drift, so upstream sums with Kahan compensation after
//! sorting ascending. Both halves matter and both are preserved here.

use alloc::vec::Vec;

/// Kahan-compensated sum of `values`, ascending-sorted first.
///
/// # Method
///
/// Translated line-for-line from `CycArithmetic::KahanSum`, which itself cites
/// the Wikipedia description of the algorithm. Two things happen, in order:
///
///  1. **Sort ascending.** Adding the smallest magnitudes first keeps the
///     running sum as small as possible for as long as possible, so fewer
///     low-order bits of each addend fall off the end. Upstream does this with
///     `std::sort` on a copy.
///  2. **Kahan compensation.** A running compensation term `c` recovers the
///     low-order part lost from each addend and feeds it back into the next.
///
/// # A deliberate difference from upstream
///
/// Upstream sorts with `std::sort`, whose comparator is `operator<`. That is
/// undefined behaviour in the presence of NaN, because `<` is not a strict
/// weak ordering over floats. Here the sort uses [`f64::total_cmp`], which is
/// a total order over every `f64` including NaN and signed zeros. On NaN-free
/// input — which is every input upstream intends — the two orderings agree
/// exactly, so this is strictly a hardening, not a behavioural change.
///
/// # Examples
///
/// ```
/// use kaki_bukit::arithmetic::kahan_sum;
///
/// // A large value followed by many small ones: naive summation loses them.
/// let mut v = vec![1.0e16];
/// v.extend(core::iter::repeat(1.0).take(10));
/// assert_eq!(kahan_sum(&v), 1.0e16 + 10.0);
/// ```
#[must_use]
pub fn kahan_sum(values: &[f64]) -> f64 {
    let mut sorted: Vec<f64> = values.to_vec();
    sorted.sort_by(|a, b| a.total_cmp(b));

    let mut sum = 0.0f64;
    // A running compensation for lost low-order bits.
    let mut c = 0.0f64;
    for &v in &sorted {
        let y = v - c;
        let t = sum + y;
        // (t - sum) recovers the high-order part of y; subtracting y recovers
        // the negated low part, which is carried into the next iteration.
        c = (t - sum) - y;
        sum = t;
    }
    sum
}

/// Returns `values` sorted ascending. Upstream `CycArithmetic::sort_ascending`.
///
/// Exposed because upstream exposes it, and because a caller that sums the
/// same vector repeatedly can sort once and call [`kahan_sum_sorted`].
#[must_use]
pub fn sort_ascending(values: &[f64]) -> Vec<f64> {
    let mut v = values.to_vec();
    v.sort_by(|a, b| a.total_cmp(b));
    v
}

/// Kahan-compensated sum of already-ascending-sorted `values`.
///
/// This is [`kahan_sum`] without the sort and without the copy it needs. Use
/// it only when `values` is genuinely sorted ascending; passing unsorted input
/// is not unsound, but it gives up the error-reduction that the sort provides
/// and the result will differ from [`kahan_sum`] in the low-order bits.
#[must_use]
pub fn kahan_sum_sorted(values: &[f64]) -> f64 {
    let mut sum = 0.0f64;
    let mut c = 0.0f64;
    for &v in values {
        let y = v - c;
        let t = sum + y;
        c = (t - sum) - y;
        sum = t;
    }
    sum
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    #[test]
    fn recovers_small_addends_a_naive_sum_would_lose() {
        let mut v = vec![1.0e16];
        v.extend(core::iter::repeat_n(1.0, 10));

        // The naive sum in source order loses every 1.0.
        let naive: f64 = v.iter().sum();
        assert_eq!(naive, 1.0e16);

        assert_eq!(kahan_sum(&v), 1.0e16 + 10.0);
    }

    #[test]
    fn matches_exact_sum_on_representable_input() {
        let v = vec![0.5, 0.25, 0.125, 0.0625];
        assert_eq!(kahan_sum(&v), 0.9375);
    }

    #[test]
    fn empty_sums_to_zero() {
        assert_eq!(kahan_sum(&[]), 0.0);
    }

    #[test]
    fn is_order_independent_because_it_sorts() {
        let a = vec![1.0e16, 1.0, 1.0, -1.0e16];
        let b = vec![-1.0e16, 1.0, 1.0e16, 1.0];
        assert_eq!(kahan_sum(&a), kahan_sum(&b));
    }

    #[test]
    fn sorted_variant_agrees_when_input_is_sorted() {
        let v = sort_ascending(&[3.0, 1.0, 2.0]);
        assert_eq!(kahan_sum_sorted(&v), kahan_sum(&v));
    }
}
