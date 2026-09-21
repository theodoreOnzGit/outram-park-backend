//! Minimal cut sets from a fault tree — the MOCUS algorithm.
//!
//! **This is NOT a port of SCRAM's `src/mocus.cc`, and it carries no
//! attribution header for that reason.** Upstream's MOCUS is a 108-line driver
//! over a ZBDD `CutSetContainer` operating on a `Pdag` that a 2,400-line
//! preprocessor has already rewritten; porting it means porting ZBDD, the
//! Boolean graph and the preprocessor — roughly 8,000 lines — and none of that
//! is here. What this module implements is the **classical top-down MOCUS
//! expansion** from the published literature:
//!
//! - J. B. Fussell and W. E. Vesely, "A new methodology for obtaining cut sets
//!   for fault trees", *Transactions of the American Nuclear Society* **15**
//!   (1972), 262-263.
//! - A. Rauzy, "New algorithms for fault trees analysis", *Reliability
//!   Engineering & System Safety* **40**(3) (1993), 203-211,
//!   [doi:10.1016/0951-8320(93)90060-C](https://doi.org/10.1016/0951-8320(93)90060-C)
//!   — for why a BDD/ZBDD formulation supersedes this one on large models.
//!
//! It is nonetheless **verified against upstream**: the cut sets it produces
//! are compared, set for set, against the products SCRAM reports for its own
//! input models. That comparison is worth more than a translation would be,
//! precisely because the two algorithms are unrelated — see
//! `crates/raffles/docs/scram-port-verification.md` and
//! `tests/scram_mocus_oracle.rs`.
//!
//! # What it does and does not handle
//!
//! **All eight connectives**, coherent and not. A negated gate is expanded
//! through its De Morgan dual, so complemented literals appear during
//! expansion; a partial set containing both a literal and its complement is
//! impossible and is dropped. At the end the negative literals are deleted and
//! the result minimised by absorption — which is what upstream does too, in
//! ZBDD form: `Zbdd::EliminateComplement` OR-merges the two branches of a
//! negative-index node (deleting the literal) and `Zbdd::Minimize` absorbs.
//!
//! **Minimal cut sets of a non-coherent tree are CONSERVATIVE**, and that is a
//! property of the definition, not of this implementation. Deleting negative
//! literals discards the information that some failure combinations require a
//! component to be *working*, so the cut sets describe a function that is
//! everywhere at least as large as the real one. Quantifying them gives an
//! **upper bound** on the top-event probability, not the probability. Use
//! prime implicants if you need the exact function — upstream has them behind
//! `--prime-implicants`, and they are not ported.
//!
//! **It is exponential in the worst case**, which is why Rauzy's paper exists.
//! [`minimal_cut_sets`] takes an order limit for that reason, and it is the
//! same knob upstream calls `limit_order`.
//!
//! **Upstream has a probability cut-off setting, and it does nothing.**
//! `Settings::cut_off_` defaults to `1e-8`, is settable from the CLI
//! (`--cut-off`) and from a project file, and is range-validated on the way
//! in — but its getter `Settings::cut_off()` has **no callers anywhere in
//! SCRAM 0.16.2**, so no product is ever discarded by probability. Checked by
//! running it, not only by reading: `--cut-off 0.5` on `Aralia/chinese`, whose
//! top-event probability is `1.17e-3`, leaves all 392 products in place and
//! the total unchanged.
//!
//! So there is nothing to port here, and truncating by order only is not a
//! divergence from upstream. Stated because the opposite is the natural
//! assumption from reading `settings.h`.

use std::collections::HashSet;

use super::fault_tree::{Arg, Connective, FaultTree, Gate};
use super::probability::CutSet;
use crate::{RafflesError, Result};

/// Default cap on the **order** of a generated cut set — how many basic events
/// it may contain.
///
/// 20, matching upstream SCRAM's `Settings::limit_order_` default, so a
/// comparison against SCRAM's reported products is like for like unless the
/// caller changes both.
pub const DEFAULT_LIMIT_ORDER: usize = 20;

/// Ceiling on how many partially-expanded cut sets may be alive at once.
///
/// MOCUS is exponential, and on a real PRA model it does not merely get slow —
/// it exhausts memory. Hitting this returns an error naming the limit rather
/// than letting the process be killed, which is the difference between a
/// diagnosable failure and a disappearing job.
pub const EXPANSION_LIMIT: usize = 5_000_000;

/// Generates the minimal cut sets of a fault tree.
///
/// A **cut set** is a set of basic events whose joint occurrence causes the
/// top event; it is **minimal** if no proper subset of it is also a cut set.
/// The minimal cut sets are the complete enumeration of how the system fails,
/// and are what [`super::probability::top_event_probability`] and
/// [`super::importance::importance_factors`] consume.
///
/// `limit_order` discards any cut set with more than that many basic events;
/// pass [`DEFAULT_LIMIT_ORDER`] to match upstream SCRAM's default. Truncation
/// is a real approximation — the discarded sets contribute probability — so
/// lower it deliberately, not for speed alone.
///
/// The returned cut sets are sorted by order and then lexicographically by
/// member index, so two runs on the same tree compare equal.
///
/// # Errors
///
/// - [`RafflesError::InvalidParameter`] if `limit_order` is zero, or if the
///   expansion exceeds [`EXPANSION_LIMIT`] intermediate states.
///
/// # Example
///
/// ```
/// use raffles::scram::fault_tree::{Connective, FaultTreeBuilder};
/// use raffles::scram::mocus::{minimal_cut_sets, DEFAULT_LIMIT_ORDER};
///
/// let mut b = FaultTreeBuilder::new();
/// b.basic_event("ValveOne", 0.5).unwrap();
/// b.basic_event("PumpOne", 0.7).unwrap();
/// b.basic_event("ValveTwo", 0.5).unwrap();
/// b.basic_event("PumpTwo", 0.7).unwrap();
/// b.gate("TrainOne", Connective::Or, &["ValveOne", "PumpOne"]).unwrap();
/// b.gate("TrainTwo", Connective::Or, &["ValveTwo", "PumpTwo"]).unwrap();
/// b.gate("TopEvent", Connective::And, &["TrainOne", "TrainTwo"]).unwrap();
/// let model = b.build("TopEvent").unwrap();
///
/// // Two redundant trains: every failure needs one event from each.
/// let cut_sets = minimal_cut_sets(model.tree(), DEFAULT_LIMIT_ORDER).unwrap();
/// assert_eq!(cut_sets.len(), 4);
/// assert!(cut_sets.iter().all(|c| c.order() == 2));
/// ```
pub fn minimal_cut_sets(tree: &FaultTree, limit_order: usize) -> Result<Vec<CutSet>> {
    if limit_order == 0 {
        return Err(RafflesError::InvalidParameter {
            parameter: "limit_order".to_string(),
            value: 0.0,
            reason: "an order limit of zero admits no cut set at all; use at least 1".to_string(),
        });
    }

    // A partially-expanded set: the literals settled so far, positive and
    // complemented kept apart, and the gates still to expand with the sign
    // each was reached under. Every component is sorted and deduplicated so
    // that two routes to the same intermediate state compare equal and are
    // visited once -- on a tree with shared subtrees that is the difference
    // between exponential and tractable.
    type Partial = (Vec<usize>, Vec<usize>, Vec<(usize, bool)>);

    let start: Partial = (Vec::new(), Vec::new(), vec![(tree.top(), false)]);
    let mut seen: HashSet<Partial> = HashSet::new();
    seen.insert(start.clone());
    let mut pending: Vec<Partial> = vec![start];
    let mut complete: Vec<Vec<usize>> = Vec::new();

    while let Some((positive, negative, gates)) = pending.pop() {
        let Some((&(gate, negated), rest)) = gates.split_first() else {
            // No gates left: the negative literals have done their work
            // constraining the expansion and are now deleted, exactly as
            // upstream's `EliminateComplement` deletes them. What survives is
            // a cut set in the conservative sense -- see the module doc.
            complete.push(positive);
            continue;
        };
        let rest = rest.to_vec();

        for branch in branches(&tree.gates()[gate], negated) {
            let mut positive = positive.clone();
            let mut negative = negative.clone();
            let mut gates = rest.clone();
            for (arg, arg_negated) in branch {
                match arg {
                    Arg::BasicEvent(e) if arg_negated => negative.push(e),
                    Arg::BasicEvent(e) => positive.push(e),
                    Arg::Gate(g) => gates.push((g, arg_negated)),
                }
            }
            for v in [&mut positive, &mut negative] {
                v.sort_unstable();
                v.dedup();
            }
            gates.sort_unstable();
            gates.dedup();

            // A set requiring an event both to occur and not to occur is the
            // empty function: it is no implicant at all, and dropping it here
            // is what keeps the deletion above from inventing a cut set.
            if positive.iter().any(|e| negative.binary_search(e).is_ok()) {
                continue;
            }
            // Truncate here rather than after expansion: the positive literals
            // only grow, and they alone decide the final order, so a partial
            // set already over the limit can never come back under it.
            if positive.len() > limit_order {
                continue;
            }

            let next: Partial = (positive, negative, gates);
            if seen.insert(next.clone()) {
                if seen.len() > EXPANSION_LIMIT {
                    return Err(RafflesError::InvalidParameter {
                        parameter: "tree".to_string(),
                        value: seen.len() as f64,
                        reason: format!(
                            "cut-set expansion exceeded {EXPANSION_LIMIT} intermediate states. \
                             MOCUS is exponential; lower `limit_order`. If you need the \
                             top-event PROBABILITY rather than the cut sets, \
                             `scram::bdd::Bdd` computes it without enumerating them"
                        ),
                    });
                }
                pending.push(next);
            }
        }
    }

    Ok(minimize(complete))
}

/// The alternative ways a gate can be satisfied, under the sign it was reached
/// with.
///
/// Each branch is a conjunction: every `(arg, negated)` in it must hold. The
/// branches themselves are alternatives. A negated gate is expanded through
/// its De Morgan dual rather than by building an explicit negation-normal
/// form first, which is what upstream's preprocessor does instead.
///
/// The negated `atleast` is the one worth spelling out: *not* (at least `k` of
/// `n`) is *at most* `k - 1` of them, which is *at least* `n - k + 1` of their
/// complements. That is why the negated case takes combinations of size
/// `n - min + 1` rather than `min`.
fn branches(gate: &Gate, negated: bool) -> Vec<Vec<(Arg, bool)>> {
    let args = gate.args();
    // All arguments, each carrying `sign`, as a single conjunctive branch.
    let all = |sign: bool| vec![args.iter().map(|a| (*a, sign)).collect::<Vec<_>>()];
    // One branch per argument, each carrying `sign`.
    let each = |sign: bool| args.iter().map(|a| vec![(*a, sign)]).collect::<Vec<_>>();
    // One branch per `k`-combination, every member carrying `sign`.
    let choose = |k: usize, sign: bool| {
        combinations(args, k)
            .into_iter()
            .map(|c| c.into_iter().map(|a| (a, sign)).collect())
            .collect::<Vec<Vec<_>>>()
    };

    match (gate.connective(), negated) {
        // AND: everything, or -- negated, by De Morgan -- any one negated.
        (Connective::And, false) | (Connective::Nand, true) => all(false),
        (Connective::And, true) | (Connective::Nand, false) => each(true),
        // OR: any one, or -- negated -- everything negated.
        (Connective::Or, false) | (Connective::Nor, true) => each(false),
        (Connective::Or, true) | (Connective::Nor, false) => all(true),
        // NULL is a pass-through, so it is AND over its single argument.
        (Connective::Null, sign) => all(sign),
        // NOT flips whatever sign it was reached with.
        (Connective::Not, sign) => all(!sign),
        // At least `min` of `n`; negated, at least `n - min + 1` complements.
        (Connective::Atleast { min }, false) => choose(min, false),
        (Connective::Atleast { min }, true) => choose(args.len() - min + 1, true),
        // XOR(a, b) is `a and not b` or `not a and b`; its negation is IFF,
        // `both` or `neither`. Arity is fixed at two by `FaultTreeBuilder`.
        (Connective::Xor, false) => vec![
            vec![(args[0], false), (args[1], true)],
            vec![(args[0], true), (args[1], false)],
        ],
        (Connective::Xor, true) => vec![
            vec![(args[0], false), (args[1], false)],
            vec![(args[0], true), (args[1], true)],
        ],
    }
}

/// Every `k`-subset of `args`, preserving the declared order within each.
///
/// The minimal cut sets of an at-least gate are exactly its `min`-combinations:
/// any `min` of the arguments occurring is sufficient, and no fewer is.
fn combinations(args: &[Arg], k: usize) -> Vec<Vec<Arg>> {
    let mut out = Vec::new();
    let mut current = Vec::with_capacity(k);
    fn walk(args: &[Arg], k: usize, start: usize, current: &mut Vec<Arg>, out: &mut Vec<Vec<Arg>>) {
        if current.len() == k {
            out.push(current.clone());
            return;
        }
        // Stop early once too few arguments remain to finish a combination.
        for i in start..=args.len().saturating_sub(k - current.len()) {
            current.push(args[i]);
            walk(args, k, i + 1, current, out);
            current.pop();
        }
    }
    walk(args, k, 0, &mut current, &mut out);
    out
}

/// Discards every cut set that strictly contains another — the minimisation
/// step, by absorption.
///
/// Sorting by order first means a superset is always compared against sets
/// already known to be no larger, so one pass suffices.
fn minimize(mut sets: Vec<Vec<usize>>) -> Vec<CutSet> {
    sets.sort_unstable();
    sets.dedup();
    sets.sort_by(|a, b| a.len().cmp(&b.len()).then_with(|| a.cmp(b)));

    let mut kept: Vec<Vec<usize>> = Vec::new();
    for candidate in sets {
        // An empty cut set means the top event is unconditional; it absorbs
        // everything, and `CutSet::new` refuses it, so it is dropped here with
        // the rest of the non-minimal sets rather than being silently kept.
        if candidate.is_empty() {
            continue;
        }
        if !kept.iter().any(|k| is_subset(k, &candidate)) {
            kept.push(candidate);
        }
    }
    kept.into_iter()
        .map(|members| CutSet::new(&members).expect("non-empty by construction"))
        .collect()
}

/// Whether every member of `small` appears in `large`; both are sorted.
fn is_subset(small: &[usize], large: &[usize]) -> bool {
    let mut it = large.iter();
    small.iter().all(|s| it.any(|l| l == s))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scram::fault_tree::FaultTreeBuilder;
    use std::collections::BTreeSet;

    /// Cut sets as sets of names, so a comparison cannot silently depend on
    /// index order.
    fn named(
        model: &crate::scram::fault_tree::FaultTreeModel,
        limit: usize,
    ) -> BTreeSet<BTreeSet<String>> {
        minimal_cut_sets(model.tree(), limit)
            .unwrap()
            .iter()
            .map(|c| {
                c.members()
                    .iter()
                    .map(|&i| model.basic_event_names()[i].clone())
                    .collect()
            })
            .collect()
    }

    fn expect(sets: &[&[&str]]) -> BTreeSet<BTreeSet<String>> {
        sets.iter()
            .map(|s| s.iter().map(|n| n.to_string()).collect())
            .collect()
    }

    /// **Methodology.** A `null` gate is a pass-through: it must change the
    /// tree's shape without changing its cut sets. Checked by chaining two of
    /// them above an OR and comparing against the bare OR.
    ///
    /// **Why this is a unit test and not an oracle comparison.** It used to
    /// be one: upstream's `input/ThreeLevels/top.xml` is exactly this shape
    /// (two transfer gates above an OR of `A` and `B`) and SCRAM's products
    /// for it were compared against this port's on 2026-09-21, agreeing on
    /// both cut sets. That model was then **dropped from the fixture** when
    /// `extract_oracle.sh` gained its multi-result guard — it defines three
    /// fault trees in one file, and the report gives no way to attribute a
    /// product to a top event. The evidence was real; it is no longer
    /// regenerable, so it is not claimed. **`Connective::Null` has no
    /// upstream-oracle coverage in the committed fixture.**
    ///
    /// **Result** (2026-09-21): `{A}`, `{B}` with and without the two
    /// pass-through gates.
    #[test]
    fn a_null_gate_passes_its_argument_through_unchanged() {
        let mut b = FaultTreeBuilder::new();
        b.basic_event("A", 0.1).unwrap();
        b.basic_event("B", 0.2).unwrap();
        b.gate("Bottom", Connective::Or, &["A", "B"]).unwrap();
        b.gate("Middle", Connective::Null, &["Bottom"]).unwrap();
        b.gate("Top", Connective::Null, &["Middle"]).unwrap();
        let chained = b.build("Top").unwrap();

        let mut b = FaultTreeBuilder::new();
        b.basic_event("A", 0.1).unwrap();
        b.basic_event("B", 0.2).unwrap();
        b.gate("Bottom", Connective::Or, &["A", "B"]).unwrap();
        let bare = b.build("Bottom").unwrap();

        let want = expect(&[&["A"], &["B"]]);
        assert_eq!(named(&chained, DEFAULT_LIMIT_ORDER), want);
        assert_eq!(named(&bare, DEFAULT_LIMIT_ORDER), want);
    }

    /// **Methodology.** The minimal cut sets of `atleast min=k` over `n`
    /// arguments are exactly its `k`-combinations, and the two boundary cases
    /// must degenerate: `k = 1` behaves as OR and `k = n` as AND. Upstream
    /// accepts both rather than rewriting them, and so does this.
    ///
    /// **Result** (2026-09-21): 2-of-3 gives the three pairs; 1-of-3 gives the
    /// three singletons; 3-of-3 gives the single triple. `HIPPS` covers the
    /// general case against SCRAM (`tests/scram_mocus_oracle.rs`); the two
    /// boundaries have no oracle model and are checked here only.
    #[test]
    fn an_atleast_gate_expands_to_its_combinations() {
        let build = |min: usize| {
            let mut b = FaultTreeBuilder::new();
            for n in ["A", "B", "C"] {
                b.basic_event(n, 0.1).unwrap();
            }
            b.gate("Top", Connective::Atleast { min }, &["A", "B", "C"])
                .unwrap();
            b.build("Top").unwrap()
        };
        assert_eq!(
            named(&build(2), DEFAULT_LIMIT_ORDER),
            expect(&[&["A", "B"], &["A", "C"], &["B", "C"]])
        );
        assert_eq!(
            named(&build(1), DEFAULT_LIMIT_ORDER),
            expect(&[&["A"], &["B"], &["C"]])
        );
        assert_eq!(
            named(&build(3), DEFAULT_LIMIT_ORDER),
            expect(&[&["A", "B", "C"]])
        );
    }

    /// **Methodology.** Absorption, in the case that actually catches a
    /// missing minimisation step: an OR whose arguments are `A` and an AND of
    /// `A` and `B`. The cut set `{A, B}` is a cut set but not a minimal one,
    /// because `{A}` already suffices.
    ///
    /// A generator without absorption returns both and still quantifies to
    /// roughly the right answer under the rare-event approximation, which is
    /// why the totals agreeing does not imply this.
    ///
    /// **Result** (2026-09-21): `{A}` only.
    #[test]
    fn a_non_minimal_cut_set_is_absorbed() {
        let mut b = FaultTreeBuilder::new();
        b.basic_event("A", 0.1).unwrap();
        b.basic_event("B", 0.2).unwrap();
        b.gate("Both", Connective::And, &["A", "B"]).unwrap();
        b.gate("Top", Connective::Or, &["A", "Both"]).unwrap();
        let model = b.build("Top").unwrap();
        assert_eq!(named(&model, DEFAULT_LIMIT_ORDER), expect(&[&["A"]]));
    }

    /// **Methodology.** A basic event reached twice along an AND path is one
    /// event, not two: `AND(A, OR(A, B))` has the single minimal cut set
    /// `{A}`, since `A` alone satisfies both arms.
    ///
    /// This is the idempotence that makes the probability layer's duplicate
    /// collapsing matter — `p * p` would be the wrong answer, and a generator
    /// that emitted `{A, A}` would produce it.
    ///
    /// **Result** (2026-09-21): `{A}` only, of order 1.
    #[test]
    fn a_repeated_basic_event_collapses_rather_than_squaring() {
        let mut b = FaultTreeBuilder::new();
        b.basic_event("A", 0.1).unwrap();
        b.basic_event("B", 0.2).unwrap();
        b.gate("Either", Connective::Or, &["A", "B"]).unwrap();
        b.gate("Top", Connective::And, &["A", "Either"]).unwrap();
        let model = b.build("Top").unwrap();
        let sets = minimal_cut_sets(model.tree(), DEFAULT_LIMIT_ORDER).unwrap();
        assert_eq!(sets.len(), 1);
        assert_eq!(sets[0].order(), 1);
        assert_eq!(named(&model, DEFAULT_LIMIT_ORDER), expect(&[&["A"]]));
    }

    /// **Methodology.** The results must not depend on the order arguments
    /// were declared in. Checked by building the same tree with its OR
    /// arguments reversed and comparing the named cut sets.
    ///
    /// **Result** (2026-09-21): identical, and the returned order is itself
    /// deterministic — the cut sets come back sorted by order then by member
    /// index, which is what lets the oracle tests compare them directly.
    #[test]
    fn the_result_does_not_depend_on_declaration_order() {
        let build = |reversed: bool| {
            let mut b = FaultTreeBuilder::new();
            for n in ["A", "B", "C", "D"] {
                b.basic_event(n, 0.1).unwrap();
            }
            let one = if reversed { ["B", "A"] } else { ["A", "B"] };
            let two = if reversed { ["D", "C"] } else { ["C", "D"] };
            b.gate("One", Connective::Or, &one).unwrap();
            b.gate("Two", Connective::Or, &two).unwrap();
            b.gate("Top", Connective::And, &["One", "Two"]).unwrap();
            b.build("Top").unwrap()
        };
        assert_eq!(
            named(&build(false), DEFAULT_LIMIT_ORDER),
            named(&build(true), DEFAULT_LIMIT_ORDER)
        );
        let sets = minimal_cut_sets(build(true).tree(), DEFAULT_LIMIT_ORDER).unwrap();
        let mut sorted = sets.clone();
        sorted.sort_by(|a, b| {
            a.order()
                .cmp(&b.order())
                .then_with(|| a.members().cmp(b.members()))
        });
        assert_eq!(sets, sorted, "the returned order should already be sorted");
    }

    /// **Methodology.** A wide AND of ORs is the standard blow-up case:
    /// `AND` of `n` two-argument `OR`s has exactly `2^n` minimal cut sets,
    /// every one of order `n`. At `n = 12` that is 4,096 — enough to show the
    /// expansion is not accidentally quadratic in something, and small enough
    /// to run in a normal test.
    ///
    /// It also pins the truncation semantics at the extreme: at any limit
    /// below `n` the answer must be empty, not partial.
    ///
    /// **Result** (2026-09-21): 4,096 cut sets, all of order 12, in well under
    /// a second in release mode; empty at limit 11.
    #[test]
    fn a_wide_and_of_ors_produces_exactly_two_to_the_n_cut_sets() {
        const N: usize = 12;
        let mut b = FaultTreeBuilder::new();
        let mut gate_names = Vec::new();
        for i in 0..N {
            let (a, c) = (format!("A{i}"), format!("C{i}"));
            b.basic_event(&a, 0.1).unwrap();
            b.basic_event(&c, 0.1).unwrap();
            let g = format!("G{i}");
            b.gate(&g, Connective::Or, &[&a, &c]).unwrap();
            gate_names.push(g);
        }
        let refs: Vec<&str> = gate_names.iter().map(|s| s.as_str()).collect();
        b.gate("Top", Connective::And, &refs).unwrap();
        let model = b.build("Top").unwrap();

        let sets = minimal_cut_sets(model.tree(), DEFAULT_LIMIT_ORDER).unwrap();
        assert_eq!(sets.len(), 1 << N);
        assert!(sets.iter().all(|c| c.order() == N));
        assert!(minimal_cut_sets(model.tree(), N - 1).unwrap().is_empty());
    }
}
