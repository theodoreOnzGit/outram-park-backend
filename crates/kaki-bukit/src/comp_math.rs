// PROVENANCE
//   Upstream project: CYCLUS <https://github.com/cyclus/cyclus>
//   Upstream file:    src/comp_math.h, src/comp_math.cc
//   Upstream commit:  d4faab7ce0566ccb8febfcf50915cdeedee59db0
//   Upstream licence: BSD-3-Clause
//   Copyright (c) 2010-2016, University of Wisconsin Computational Nuclear
//   Engineering Research Group. All rights reserved.
//
//   Independent Rust translation, (C) 2026 Theodore Ong and the outram-park
//   contributors, GPL-3.0-only. Relicensing is ONE-WAY.

//! Component-wise arithmetic on composition vectors, translated from
//! `namespace cyclus::compmath`.
//!
//! A [`CompMap`] is a map from nuclide to a **dimensionless** quantity. It is
//! deliberately not normalised: upstream's own header says "In general
//! CompMaps are not assumed to be normalized to any particular value", and
//! most of the resource layer depends on that — [`normalize`] to a mass is how
//! a composition and a quantity get combined before they are added.
//!
//! Whether the quantity is an atom fraction or a mass fraction is **not**
//! recorded in the map. It is carried by the caller, and
//! [`Composition`](crate::composition::Composition) is the type that keeps the
//! two straight. Every function here is basis-agnostic.

use alloc::collections::BTreeMap;
use alloc::vec::Vec;

use crate::arithmetic::kahan_sum;
use crate::error::{CyclusError, Result};
use crate::limits::abs;
use crate::nuclide::Nuc;

/// A raw map from nuclide to dimensionless quantity. Upstream `CompMap`.
///
/// `BTreeMap` rather than a hash map, and that choice is load-bearing: it
/// reproduces `std::map`'s ordered iteration, so a composition sums, prints
/// and records in nuclide order on every run and every platform. A hash map
/// would make [`sum`]'s result depend on iteration order in the low-order
/// bits, which is exactly the reproducibility this crate is built to preserve.
pub type CompMap = BTreeMap<Nuc, f64>;

/// Component-wise addition. Upstream `compmath::Add`.
///
/// No normalisation is performed. Nuclides present in only one operand are
/// carried through unchanged.
///
/// # Examples
///
/// ```
/// use kaki_bukit::comp_math::{add, CompMap};
/// use kaki_bukit::nuclide::nuc;
///
/// let mut v1 = CompMap::new();
/// v1.insert(nuc::U235, 2.3);
/// v1.insert(nuc::U238, 1.3);
/// let mut v2 = CompMap::new();
/// v2.insert(nuc::U235, 1.1);
/// v2.insert(nuc::U238, 1.2);
///
/// let v3 = add(&v1, &v2);
/// assert!((v3[&nuc::U235] - 3.4).abs() < 1e-12);
/// assert!((v3[&nuc::U238] - 2.5).abs() < 1e-12);
/// ```
#[must_use]
pub fn add(v1: &CompMap, v2: &CompMap) -> CompMap {
    let mut out = v1.clone();
    for (&nuc, &qty) in v2 {
        *out.entry(nuc).or_insert(0.0) += qty;
    }
    out
}

/// Component-wise subtraction, `v1 - v2`. Upstream `compmath::Sub`.
///
/// No normalisation is performed, and **the result may contain negative
/// entries**. That is upstream behaviour and callers rely on it:
/// [`Material::extract_comp`](crate::material::Material::extract_comp)
/// subtracts and then applies a threshold, so a small negative left by
/// rounding is cleared by [`apply_threshold`] rather than rejected here.
#[must_use]
pub fn sub(v1: &CompMap, v2: &CompMap) -> CompMap {
    let mut out = v1.clone();
    for (&nuc, &qty) in v2 {
        *out.entry(nuc).or_insert(0.0) -= qty;
    }
    out
}

/// Sum of every quantity, without normalisation. Upstream `compmath::Sum`.
///
/// Uses [`kahan_sum`](crate::arithmetic::kahan_sum) — sorted, compensated —
/// exactly as upstream does. This is the single most-called numerical routine
/// in the crate, and the compensation is why a long simulation's mass balance
/// stays closed.
#[must_use]
pub fn sum(v: &CompMap) -> f64 {
    let values: Vec<f64> = v.values().copied().collect();
    kahan_sum(&values)
}

/// Zeroes — by removing — every entry whose magnitude is at or below
/// `threshold`. Upstream `compmath::ApplyThreshold`.
///
/// Upstream erases the entry rather than setting it to zero, and this does
/// too: a composition carrying a thousand nuclides at `1e-30` costs real time
/// in every later sum, and the distinction between "absent" and "present at
/// zero" is not one the resource layer makes.
///
/// # Errors
///
/// [`CyclusError::Value`] if `threshold` is negative, matching upstream's
/// `ValueError`.
pub fn apply_threshold(v: &mut CompMap, threshold: f64) -> Result<()> {
    if threshold < 0.0 {
        return Err(CyclusError::Value("composition threshold cannot be negative"));
    }
    v.retain(|_, qty| abs(*qty) > threshold);
    Ok(())
}

/// Scales every quantity so the total is `val`. Upstream `compmath::Normalize`.
///
/// A no-op when the sum already equals `val`, and — importantly — also a no-op
/// when the sum is zero, since there is no scale factor that makes an empty
/// composition total `val`. Upstream guards the same way, and silently: an
/// all-zero composition is a legitimate state for a fully depleted buffer.
pub fn normalize(v: &mut CompMap, val: f64) {
    let s = sum(v);
    if s != val && s != 0.0 {
        let mult = val / s;
        for qty in v.values_mut() {
            *qty *= mult;
        }
    }
}

/// `true` if every key is a valid nuclide. Upstream `compmath::ValidNucs`.
#[must_use]
pub fn valid_nucs(v: &CompMap) -> bool {
    v.keys().all(|n| n.is_nuclide())
}

/// The first invalid nuclide key, if any.
///
/// Not an upstream function. `valid_nucs` answering only yes/no means a caller
/// that wants to *report* the problem has to re-scan for it, and every caller
/// in this crate does want to report it. Returning the offending key here is
/// the "human interface layer" rule applied to an error path.
#[must_use]
pub fn first_invalid_nuc(v: &CompMap) -> Option<Nuc> {
    v.keys().copied().find(|n| !n.is_nuclide())
}

/// `true` if no quantity is negative. Upstream `compmath::AllPositive`.
///
/// Note the name is upstream's and is mildly misleading in both languages:
/// the test is `>= 0`, so an all-zero composition passes.
#[must_use]
pub fn all_positive(v: &CompMap) -> bool {
    v.values().all(|&q| q >= 0.0)
}

/// `true` if `v1` and `v2` agree to within a **relative** `threshold`.
/// Upstream `compmath::AlmostEq`.
///
/// # Method
///
/// Upstream cites a note on floating-point comparison and implements
/// "almost equal if `|x-y| < |x|*eps` and `|x-y| < |y|*eps`" — a relative
/// test, not an absolute one, so it behaves sensibly across the many orders of
/// magnitude a composition spans. Two structural conditions come first: the
/// maps must be the same size, and every key of `v1` must be present in `v2`.
///
/// # A preserved upstream quirk
///
/// When either quantity is exactly zero, upstream falls into a branch that
/// tests `|diff| > |diff| * threshold`. For `threshold < 1` and a non-zero
/// `diff` that is always true, so the comparison reports *not equal* — meaning
/// a nuclide present at zero in one map and non-zero in the other never
/// compares equal regardless of how small the difference is. That is
/// reproduced here rather than "fixed", because a caller's tolerance may have
/// been tuned against it and silently changing a comparison is how a
/// regression gets shipped. It is called out in
/// `docs/port-notes.md` as a candidate defect to raise upstream.
///
/// # Errors
///
/// [`CyclusError::Value`] if `threshold` is negative.
pub fn almost_eq(v1: &CompMap, v2: &CompMap, threshold: f64) -> Result<bool> {
    if threshold < 0.0 {
        return Err(CyclusError::Value("comparison threshold cannot be negative"));
    }
    if v1.len() != v2.len() {
        return Ok(false);
    }
    if v1.is_empty() && v2.is_empty() {
        return Ok(true);
    }

    for (&nuc, &subtrahend) in v1 {
        let Some(&minuend) = v2.get(&nuc) else {
            return Ok(false);
        };
        let diff = minuend - subtrahend;
        if abs(minuend) == 0.0 || abs(subtrahend) == 0.0 {
            // Upstream's degenerate branch, preserved verbatim; see the note
            // in this function's docs.
            if abs(diff) > abs(diff) * threshold {
                return Ok(false);
            }
        } else if abs(diff) > abs(minuend) * threshold || abs(diff) > abs(subtrahend) * threshold {
            return Ok(false);
        }
    }
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::nuclide::nuc;
    use alloc::vec;

    fn comp(entries: &[(Nuc, f64)]) -> CompMap {
        entries.iter().copied().collect()
    }

    #[test]
    fn add_matches_the_upstream_header_example() {
        let v1 = comp(&[(nuc::U235, 2.3), (nuc::U238, 1.3)]);
        let v2 = comp(&[(nuc::U235, 1.1), (nuc::U238, 1.2)]);
        let v3 = add(&v1, &v2);
        assert!((v3[&nuc::U235] - 3.4).abs() < 1e-12);
        assert!((v3[&nuc::U238] - 2.5).abs() < 1e-12);
    }

    #[test]
    fn add_carries_through_nuclides_present_in_only_one_operand() {
        let v1 = comp(&[(nuc::U235, 1.0)]);
        let v2 = comp(&[(nuc::O16, 2.0)]);
        let v3 = add(&v1, &v2);
        assert_eq!(v3.len(), 2);
        assert_eq!(v3[&nuc::U235], 1.0);
        assert_eq!(v3[&nuc::O16], 2.0);
    }

    #[test]
    fn sub_may_leave_negative_entries() {
        let v1 = comp(&[(nuc::U235, 1.0)]);
        let v2 = comp(&[(nuc::U235, 3.0)]);
        assert_eq!(sub(&v1, &v2)[&nuc::U235], -2.0);
    }

    #[test]
    fn sum_is_compensated() {
        let v = comp(&[(nuc::U235, 1.0e16), (nuc::U238, 1.0), (nuc::O16, 1.0)]);
        assert_eq!(sum(&v), 1.0e16 + 2.0);
    }

    #[test]
    fn normalize_scales_to_the_requested_total() {
        let mut v = comp(&[(nuc::U235, 3.0), (nuc::U238, 7.0)]);
        normalize(&mut v, 1.0);
        assert!((v[&nuc::U235] - 0.3).abs() < 1e-12);
        assert!((v[&nuc::U238] - 0.7).abs() < 1e-12);
        assert!((sum(&v) - 1.0).abs() < 1e-12);
    }

    #[test]
    fn normalize_leaves_an_all_zero_composition_alone() {
        let mut v = comp(&[(nuc::U235, 0.0)]);
        normalize(&mut v, 1.0);
        assert_eq!(v[&nuc::U235], 0.0);
    }

    #[test]
    fn apply_threshold_erases_rather_than_zeroing() {
        let mut v = comp(&[(nuc::U235, 1.0), (nuc::U238, 1e-30), (nuc::O16, -1e-30)]);
        apply_threshold(&mut v, 1e-20).unwrap();
        assert_eq!(v.len(), 1);
        assert!(v.contains_key(&nuc::U235));
    }

    #[test]
    fn apply_threshold_rejects_a_negative_threshold() {
        let mut v = comp(&[(nuc::U235, 1.0)]);
        assert_eq!(
            apply_threshold(&mut v, -1.0),
            Err(CyclusError::Value("composition threshold cannot be negative"))
        );
    }

    #[test]
    fn valid_nucs_rejects_an_element_id() {
        assert!(valid_nucs(&comp(&[(nuc::U235, 1.0)])));
        let bad = comp(&[(nuc::U_NATURAL, 1.0)]);
        assert!(!valid_nucs(&bad));
        assert_eq!(first_invalid_nuc(&bad), Some(nuc::U_NATURAL));
    }

    #[test]
    fn all_positive_admits_zero() {
        assert!(all_positive(&comp(&[(nuc::U235, 0.0)])));
        assert!(!all_positive(&comp(&[(nuc::U235, -1e-9)])));
    }

    #[test]
    fn almost_eq_is_relative() {
        let a = comp(&[(nuc::U235, 1.0e9)]);
        let b = comp(&[(nuc::U235, 1.0e9 + 1.0)]);
        // 1 part in 1e9 — within a 1e-6 relative tolerance.
        assert!(almost_eq(&a, &b, 1e-6).unwrap());
        assert!(!almost_eq(&a, &b, 1e-12).unwrap());
    }

    #[test]
    fn almost_eq_requires_the_same_key_set() {
        let a = comp(&[(nuc::U235, 1.0)]);
        let b = comp(&[(nuc::U238, 1.0)]);
        assert!(!almost_eq(&a, &b, 1.0).unwrap());
        assert!(almost_eq(&CompMap::new(), &CompMap::new(), 0.0).unwrap());
    }

    #[test]
    fn almost_eq_preserves_the_upstream_zero_branch() {
        // Documented quirk: a zero on either side never compares equal to a
        // non-zero, however tight the difference.
        let a = comp(&[(nuc::U235, 0.0)]);
        let b = comp(&[(nuc::U235, 1e-300)]);
        assert!(!almost_eq(&a, &b, 0.5).unwrap());
        // Two exact zeros do compare equal, because diff is then 0.
        let z = comp(&[(nuc::U235, 0.0)]);
        assert!(almost_eq(&a, &z, 0.5).unwrap());
    }

    #[test]
    fn iteration_is_deterministic_in_nuclide_order() {
        let v = comp(&[(nuc::U238, 1.0), (nuc::O16, 2.0), (nuc::U235, 3.0)]);
        let keys: Vec<Nuc> = v.keys().copied().collect();
        assert_eq!(keys, vec![nuc::O16, nuc::U235, nuc::U238]);
    }
}
