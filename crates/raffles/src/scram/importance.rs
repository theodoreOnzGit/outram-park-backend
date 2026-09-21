// ---------------------------------------------------------------------------
// Ported from SCRAM (a probabilistic risk analysis tool).
//
//   Upstream project: SCRAM — Olzhas Rakhimov
//   Upstream repo:    https://github.com/rakhimov/scram
//   Upstream file:    src/importance_analysis.cc, src/importance_analysis.h
//   Upstream commit:  b85b78940de38996eeffec54d946824bd4280a1c  (2019-07-03)
//   Accessed:         2026-09-21
//
//   Copyright (C) 2014-2018 Olzhas Rakhimov
//   Licensed under the GNU General Public License, version 3 or later.
//
// Same-licence port: SCRAM is GPL-3.0-or-later, RAFFLES is GPL-3.0-only.
//
// Translation notes: the five derived factors are ported verbatim from
// `ImportanceAnalyzerBase::Analyze` — see each field's doc comment for the
// upstream line. The Birnbaum factor (MIF) is NOT ported as upstream computes
// it: SCRAM differentiates the BDD (`CalculateMif` walking ite vertices), and
// no BDD is ported here. This module computes the same quantity by its
// definition, `P(top | event) - P(top | not event)`, evaluated on the cut
// sets. The two agree exactly for a coherent tree, which is checked against
// upstream's own numbers in the tests.
// ---------------------------------------------------------------------------

//! Which basic events matter — the five standard importance measures.
//!
//! A top-event probability says how likely failure is. It does not say what to
//! fix. These five measures rank the basic events, and they rank them
//! *differently* because they ask different questions — see
//! [`ImportanceFactors`].

use super::probability::{top_event_probability, Approximation, CutSet};
use crate::{RafflesError, Result};

/// Relative width of the band around the RRW singularity treated as exactly
/// singular.
///
/// `RRW = p_total / (p_total - p * MIF)` diverges when the event sits in every
/// cut set. Upstream tests that denominator for exact equality with zero;
/// this port cannot, because it computes both quantities by a different route
/// and lands within a few ulp rather than on the point. `1e-12` is far wider
/// than the ulp-scale noise and far narrower than any physically meaningful
/// denominator.
pub const SINGULARITY_TOLERANCE: f64 = 1e-12;

/// The five importance measures for one basic event, plus its occurrence
/// count.
///
/// All five are dimensionless. They are **not** interchangeable rankings: a
/// component can be top by one measure and unremarkable by another, which is
/// the reason PRA reports all of them rather than picking one.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ImportanceFactors {
    /// How many cut sets contain this event.
    ///
    /// A purely structural count — it ignores probabilities entirely. Upstream
    /// calls it `occurrence`.
    pub occurrence: usize,
    /// **Birnbaum marginal importance factor** — `P(top | event) - P(top | not
    /// event)`.
    ///
    /// The sensitivity of the top-event probability to this event: how much
    /// the answer moves between the event being certain and impossible. It
    /// does **not** depend on the event's own probability, which is why a very
    /// reliable component can still have a large MIF.
    pub mif: f64,
    /// **Critical importance factor** — `p * MIF / p_total`.
    ///
    /// The fraction of top-event probability attributable to this event being
    /// critical. Unlike MIF this *does* weight by the event's own probability,
    /// so it answers "where is the risk actually coming from". Upstream:
    /// `imp.cif = p_var * imp.mif / p_total;`
    pub cif: f64,
    /// **Fussell-Vesely diagnosis importance factor** — `p * RAW`.
    ///
    /// Upstream computes it exactly this way:
    /// `imp.dif = p_var * imp.raw;`. Note this is SCRAM's definition and is
    /// ported as such; other PRA codes define Fussell-Vesely as the fraction
    /// of top-event probability from cut sets containing the event, which is
    /// numerically different. **If you are comparing against another tool,
    /// check which definition it uses before concluding anything disagrees.**
    pub dif: f64,
    /// **Risk achievement worth** — `1 + (1 - p) * MIF / p_total`.
    ///
    /// How much worse the top event gets if this event is made certain. A
    /// large RAW marks a component whose continued reliability is load-bearing
    /// — the argument for maintaining it. Upstream:
    /// `imp.raw = 1 + (1 - p_var) * imp.mif / p_total;`
    pub raw: f64,
    /// **Risk reduction worth** — `p_total / (p_total - p * MIF)`.
    ///
    /// How much better the top event gets if this event is made impossible.
    /// A large RRW marks a component worth improving — the argument for
    /// investing in it. RRW is bounded below by 1 by construction.
    ///
    /// **This is the one field that deliberately disagrees with upstream, and
    /// only at the singularity.** The denominator vanishes exactly when the
    /// event lies in *every* cut set — a single point of failure — because
    /// then `P(top | not event) = 0`, removing the event removes all risk, and
    /// the ratio diverges. This port returns [`f64::INFINITY`], which states
    /// that. SCRAM guards the division with exact float equality
    /// (`if (p_total != p_var * imp.mif)`) and, when it holds, leaves `rrw` at
    /// its zero-initialised value — so **upstream reports `RRW = 0`**, a value
    /// RRW cannot otherwise take.
    ///
    /// The guard is also why this port cannot copy it: upstream lands on the
    /// singular point exactly only because its BDD traversal happens to
    /// produce bit-identical values. Computing the same quantity another way
    /// lands a few ulp off, falls through `!=`, and divides by ~1e-19. That
    /// was measured, not hypothesised — `Theatre/theatre` `Mains_Fail` gave
    /// **-4.77e15** before the tolerance guard replaced the equality test.
    /// See [`SINGULARITY_TOLERANCE`] and
    /// `tests/scram_oracle_suite.rs::rrw_diverges_from_upstream_only_at_the_singularity`.
    pub rrw: f64,
}

/// Computes the five importance measures for one basic event.
///
/// `event` indexes into `event_probabilities`, as cut-set members do.
/// `approximation` selects how the top-event probability is quantified;
/// **use [`Approximation::Exact`] to reproduce upstream SCRAM**, whose
/// importance analysis runs on the exact BDD value.
///
/// # Errors
///
/// [`RafflesError::InvalidParameter`] if `event` is out of range, if the cut
/// sets or probabilities are invalid (see
/// [`super::probability::cut_set_probability`]), or if the total probability
/// is zero — every factor divides by it, so the ranking is undefined rather
/// than infinite.
pub fn importance_factors(
    event: usize,
    cut_sets: &[CutSet],
    event_probabilities: &[f64],
    approximation: Approximation,
) -> Result<ImportanceFactors> {
    let p_var = *event_probabilities
        .get(event)
        .ok_or_else(|| RafflesError::InvalidParameter {
            parameter: "event".to_string(),
            value: event as f64,
            reason: format!(
                "basic-event index {event} is out of range for {} probabilities",
                event_probabilities.len()
            ),
        })?;

    let p_total = top_event_probability(cut_sets, event_probabilities, approximation)?;
    if p_total == 0.0 {
        return Err(RafflesError::InvalidParameter {
            parameter: "p_total".to_string(),
            value: 0.0,
            reason: "the top-event probability is zero, so every importance measure divides \
                     by zero; the ranking is undefined"
                .to_string(),
        });
    }

    // Birnbaum, by its definition rather than by upstream's BDD derivative.
    let mut with = event_probabilities.to_vec();
    with[event] = 1.0;
    let mut without = event_probabilities.to_vec();
    without[event] = 0.0;
    let p_with = top_event_probability(cut_sets, &with, approximation)?;
    let p_without = top_event_probability(cut_sets, &without, approximation)?;
    let mif = p_with - p_without;

    let occurrence = cut_sets
        .iter()
        .filter(|c| c.members().contains(&event))
        .count();

    // Upstream `ImportanceAnalyzerBase::Analyze`, verbatim in order.
    let cif = p_var * mif / p_total;
    let raw = 1.0 + (1.0 - p_var) * mif / p_total;
    let dif = p_var * raw;
    // Upstream guards this with exact float equality:
    //   if (p_total != p_var * imp.mif) imp.rrw = p_total / (p_total - ...);
    // That guard is fragile, and this port cannot use it. SCRAM reaches the
    // singular point exactly because its BDD traversal happens to produce a
    // bit-identical value; computing the same quantity a different way (here,
    // inclusion-exclusion plus a differenced MIF) lands a few ulp away, falls
    // straight through `!=`, and divides by a denominator of order 1e-19.
    // Measured on Theatre/theatre `Mains_Fail`, that produced an RRW of
    // -4.77e15 — not merely imprecise but the wrong sign, for a quantity
    // bounded below by 1.
    //
    // So the singularity is detected by relative magnitude instead. See
    // `tests/scram_oracle_suite.rs` for the case and the reasoning.
    let denominator = p_total - p_var * mif;
    let rrw = if denominator.abs() <= p_total * SINGULARITY_TOLERANCE {
        f64::INFINITY
    } else {
        p_total / denominator
    };

    Ok(ImportanceFactors {
        occurrence,
        mif,
        cif,
        dif,
        raw,
        rrw,
    })
}
