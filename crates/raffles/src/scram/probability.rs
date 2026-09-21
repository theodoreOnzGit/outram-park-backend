// ---------------------------------------------------------------------------
// Ported from SCRAM (a probabilistic risk analysis tool).
//
//   Upstream project: SCRAM — Olzhas Rakhimov
//   Upstream repo:    https://github.com/rakhimov/scram
//   Upstream file:    src/probability_analysis.cc, src/probability_analysis.h
//   Upstream commit:  b85b78940de38996eeffec54d946824bd4280a1c  (2019-07-03)
//   Accessed:         2026-09-21
//
//   Copyright (C) 2014-2018 Olzhas Rakhimov
//   Licensed under the GNU General Public License, version 3 or later.
//
// Same-licence port: SCRAM is GPL-3.0-or-later, RAFFLES is GPL-3.0-only.
//
// Translation notes: SCRAM's `CutSetProbabilityCalculator`,
// `RareEventCalculator` and `McubCalculator` are a small class hierarchy over
// a `Zbdd` of cut sets and a `Pdag::IndexMap<double>` of variable
// probabilities. Here they are free functions over slices, selected by the
// `Approximation` enum, per the workspace no-trait-objects rule. The `Exact`
// variant is NOT a port of a SCRAM class — upstream computes the exact value
// by BDD traversal; this is inclusion-exclusion over the same cut sets,
// which yields the same number for independent basic events and is verified
// against upstream's BDD result. A BDD *is* available in this crate --
// `super::bdd` -- and computes the same quantity with no cut-set ceiling and
// no cut sets at all; the two are checked against each other as well as
// against upstream.
// ---------------------------------------------------------------------------

//! Top-event probability from minimal cut sets.
//!
//! Three quantifications, and the difference between them matters more than
//! the arithmetic does — see [`Approximation`].

use crate::{RafflesError, Result};

/// A minimal cut set: the basic events that together cause the top event.
///
/// Members are **indices into the basic-event probability slice** passed
/// alongside, not probabilities themselves. An empty cut set is rejected: in
/// fault-tree semantics it would mean the top event occurs unconditionally,
/// which is a malformed tree rather than a probability-1 answer.
///
/// Upstream represents a cut set as `std::vector<int>` of *signed* variable
/// indices, where a negative index is a complement. SCRAM asserts
/// `member > 0` in the probability path — complemented literals never reach
/// it — so this type carries unsigned indices and the assertion becomes
/// unrepresentable rather than checked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CutSet {
    members: Vec<usize>,
}

impl CutSet {
    /// Builds a cut set from basic-event indices.
    ///
    /// Duplicate members are collapsed: a basic event appearing twice in one
    /// cut set is the same event, and `p * p` would double-count it.
    ///
    /// # Errors
    ///
    /// [`RafflesError::InvalidParameter`] if `members` is empty.
    pub fn new(members: &[usize]) -> Result<Self> {
        if members.is_empty() {
            return Err(RafflesError::InvalidParameter {
                parameter: "members".to_string(),
                value: 0.0,
                reason: "a cut set needs at least one basic event; an empty cut set would \
                         mean the top event occurs unconditionally"
                    .to_string(),
            });
        }
        let mut members = members.to_vec();
        members.sort_unstable();
        members.dedup();
        Ok(Self { members })
    }

    /// The basic-event indices, ascending and deduplicated.
    pub fn members(&self) -> &[usize] {
        &self.members
    }

    /// How many basic events are in this cut set — its **order**.
    ///
    /// Order 1 is a single point of failure.
    pub fn order(&self) -> usize {
        self.members.len()
    }
}

/// How to combine cut-set probabilities into a top-event probability.
///
/// The three differ in how they treat the overlap between cut sets — the
/// chance that two of them occur at once. Both approximations are **upper
/// bounds** for a coherent tree, and both are standard in PRA practice.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Approximation {
    /// Inclusion-exclusion over the cut sets: the exact probability of their
    /// union, for independent basic events.
    ///
    /// **Cost is `2^n - 1` terms in the number of cut sets `n`**, so this is
    /// for small trees and for checking the approximations, not for a full
    /// plant model. [`top_event_probability`] refuses more than
    /// [`EXACT_CUT_SET_LIMIT`] cut sets rather than hanging.
    ///
    /// **Prefer [`super::bdd::Bdd::probability`] for anything larger.** It
    /// computes the same quantity from the tree directly, with no cut-set
    /// ceiling — and on a **non-coherent** tree it computes a *different*,
    /// truer quantity, because minimal cut sets there are conservative. This
    /// variant is kept because it is an independent second route to the same
    /// number on a coherent tree, and the two are checked against each
    /// other.
    Exact,
    /// Rare-event approximation: the sum of the cut-set probabilities,
    /// clamped to 1.
    ///
    /// Ignores every overlap, so it over-counts. Good when the basic-event
    /// probabilities are small — the regime the name refers to — and
    /// increasingly poor as they rise. The clamp is upstream's:
    /// `return sum > 1 ? 1 : sum;`.
    RareEvent,
    /// Min-cut-upper-bound: `1 - prod(1 - p_i)` over the cut sets.
    ///
    /// Treats the cut sets as independent, which they are not when they share
    /// basic events. Tighter than [`Approximation::RareEvent`] and never
    /// exceeds 1 by construction, so it needs no clamp.
    Mcub,
}

/// The largest number of cut sets [`Approximation::Exact`] will accept.
///
/// Inclusion-exclusion is `2^n - 1` terms; at 20 cut sets that is about a
/// million, which is a second or so, and every step beyond doubles it.
pub const EXACT_CUT_SET_LIMIT: usize = 20;

/// Probability that one cut set occurs — the product of its members'
/// probabilities.
///
/// Assumes the basic events are **independent**. This is upstream's
/// `CutSetProbabilityCalculator::Calculate`.
///
/// # Errors
///
/// [`RafflesError::InvalidParameter`] if a member index is out of range for
/// `event_probabilities`, or if any referenced probability is outside `[0, 1]`
/// or not finite.
pub fn cut_set_probability(cut_set: &CutSet, event_probabilities: &[f64]) -> Result<f64> {
    let mut product = 1.0;
    for &member in &cut_set.members {
        let p = *event_probabilities
            .get(member)
            .ok_or_else(|| RafflesError::InvalidParameter {
                parameter: "cut set member".to_string(),
                value: member as f64,
                reason: format!(
                    "basic-event index {member} is out of range for {} probabilities",
                    event_probabilities.len()
                ),
            })?;
        if !(0.0..=1.0).contains(&p) || !p.is_finite() {
            return Err(RafflesError::InvalidParameter {
                parameter: "basic-event probability".to_string(),
                value: p,
                reason: format!("probability of basic event {member} must lie in [0, 1]"),
            });
        }
        product *= p;
    }
    Ok(product)
}

/// Top-event probability from the minimal cut sets.
///
/// `event_probabilities[i]` is the probability of basic event `i`; cut-set
/// members index into it. Basic events are assumed **independent**.
///
/// # Errors
///
/// [`RafflesError::InvalidParameter`] if `cut_sets` is empty, if any member
/// index or probability is invalid (see [`cut_set_probability`]), or if
/// [`Approximation::Exact`] is asked for more than [`EXACT_CUT_SET_LIMIT`] cut
/// sets.
pub fn top_event_probability(
    cut_sets: &[CutSet],
    event_probabilities: &[f64],
    approximation: Approximation,
) -> Result<f64> {
    if cut_sets.is_empty() {
        return Err(RafflesError::InvalidParameter {
            parameter: "cut_sets".to_string(),
            value: 0.0,
            reason: "a fault tree with no cut sets has no failure mode; the top-event \
                     probability is undefined rather than zero"
                .to_string(),
        });
    }

    match approximation {
        Approximation::RareEvent => {
            let mut sum = 0.0;
            for cut_set in cut_sets {
                sum += cut_set_probability(cut_set, event_probabilities)?;
            }
            // Upstream: `return sum > 1 ? 1 : sum;`
            Ok(if sum > 1.0 { 1.0 } else { sum })
        }
        Approximation::Mcub => {
            let mut m = 1.0;
            for cut_set in cut_sets {
                m *= 1.0 - cut_set_probability(cut_set, event_probabilities)?;
            }
            Ok(1.0 - m)
        }
        Approximation::Exact => {
            if cut_sets.len() > EXACT_CUT_SET_LIMIT {
                return Err(RafflesError::InvalidParameter {
                    parameter: "cut_sets".to_string(),
                    value: cut_sets.len() as f64,
                    reason: format!(
                        "inclusion-exclusion is 2^n terms; {} cut sets exceeds the limit of \
                         {EXACT_CUT_SET_LIMIT}. Use `scram::bdd::Bdd::probability`, which \
                         has no such ceiling, or an approximation",
                        cut_sets.len()
                    ),
                });
            }
            inclusion_exclusion(cut_sets, event_probabilities)
        }
    }
}

/// Exact probability of the union of the cut sets, by inclusion-exclusion.
///
/// Sums over every non-empty subset of the cut sets, with sign `(-1)^(k+1)`
/// for a subset of size `k`. The intersection of several cut sets is the union
/// of their members — every event in any of them must occur — so its
/// probability is the product over that merged set, which is why duplicates
/// must be collapsed.
fn inclusion_exclusion(cut_sets: &[CutSet], event_probabilities: &[f64]) -> Result<f64> {
    let n = cut_sets.len();
    let mut total = 0.0;
    // Validate up front so an out-of-range index is reported once, not 2^n times.
    for cut_set in cut_sets {
        cut_set_probability(cut_set, event_probabilities)?;
    }
    for mask in 1u32..(1u32 << n) {
        let mut merged: Vec<usize> = Vec::new();
        for (i, cut_set) in cut_sets.iter().enumerate() {
            if mask & (1 << i) != 0 {
                merged.extend_from_slice(&cut_set.members);
            }
        }
        merged.sort_unstable();
        merged.dedup();
        let mut product = 1.0;
        for member in merged {
            product *= event_probabilities[member];
        }
        if mask.count_ones() % 2 == 1 {
            total += product;
        } else {
            total -= product;
        }
    }
    Ok(total)
}
