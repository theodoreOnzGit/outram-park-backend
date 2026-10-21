// ---------------------------------------------------------------------------
// Original RAFFLES code, written for the port of Adolphus Lye's
// "Computing with Confidence" (https://github.com/Adolphus8/Computing-with-Confidence,
// "Coherent systems", accessed 2026-10-20; gh:#987). The gate-by-gate
// evaluation pattern follows Case studies 1-4 of those scripts; the p-box
// arithmetic it calls is the pba.r port in `pba.rs`, which carries pba.r's
// GPL-3.0 attribution. Licence: AGPL-3.0-only with the rest of the crate.
// AI-assisted draft; not reviewed by a human or the crate owner.
// ---------------------------------------------------------------------------

//! # Fault trees whose basic events are confidence boxes
//!
//! [`evaluate_pbox_tree`] takes a [`FaultTree`] and one [`Pbox`] per basic
//! event (usually a c-box from [`super::pba::kn`]) and propagates them
//! **gate by gate**, with the dependence assumption chosen explicitly for each
//! gate ([`GateDependence`]). The tree's structure is the SCRAM
//! [`FaultTree`]; its point probabilities are ignored.
//!
//! ## What "gate by gate" costs you
//!
//! A gate combines its arguments as if they were separate quantities. If a
//! basic event (or a sub-gate) feeds the tree through more than one path, the
//! independent combination double-counts it. So:
//!
//! - under [`EventDependence::Unknown`] a repeated event is **safe**: Fréchet
//!   bounds hold for every joint distribution, including "the same event
//!   twice";
//! - under [`EventDependence::Independent`] a repeated event is **refused**
//!   ([`RepeatedEvents::Reject`], the default). [`RepeatedEvents::IndependentCopies`]
//!   evaluates it anyway, treating each occurrence as an independent copy.
//!   That is what the "Boolean function" route of the Case study 3 script
//!   computes, and it is **not** the structure-function answer.
//!
//! [`evaluate_cbox_edges`] is the other route for repeated events: Lye et al.
//! (2024) evaluate the tree once with every basic event replaced by its
//! left edge and once by its right edge and take the envelope (Case study 4,
//! `e1f = env.pbox(e1f_L, e1f_R)`). It applies to coherent trees and to
//! c-boxes, whose edges are distributions; it is not justified for an
//! arbitrary p-box.
//!
//! `Xor` is refused (not monotone in the probabilities). `Atleast` is
//! supported under unknown dependence only (as the OR of the AND of every
//! `min`-subset, at most [`MAX_ATLEAST_SUBSETS`] subsets), because the subsets
//! share events.

use std::collections::HashMap;

use super::fault_tree::{Arg, Connective, FaultTree};
use super::pba::{
    and_independent, and_unknown, complement, envelope, or_independent, or_unknown, point,
};
use crate::imprecise::{EventDependence, Pbox};
use crate::{RafflesError, Result};

/// Upper limit on the number of `min`-subsets an unknown-dependence
/// `Atleast` gate expands into.
pub const MAX_ATLEAST_SUBSETS: usize = 200;

/// Which dependence assumption each gate uses.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GateDependence {
    /// The same assumption at every gate.
    Uniform(EventDependence),
    /// One assumption per gate, indexed as [`FaultTree::gates`].
    PerGate(Vec<EventDependence>),
}

/// What to do when a basic event or sub-gate feeds the tree twice.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RepeatedEvents {
    /// Refuse if any gate would combine a repeated event as independent.
    Reject,
    /// Treat every occurrence as an independent copy (the script's Boolean
    /// route). Knowingly approximate.
    IndependentCopies,
}

/// Options for [`evaluate_pbox_tree`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PboxTreeOptions {
    /// Dependence assumption per gate.
    pub dependence: GateDependence,
    /// Repeated-event policy.
    pub repeated: RepeatedEvents,
}

impl PboxTreeOptions {
    /// The same dependence everywhere, repeated events rejected.
    pub fn uniform(dependence: EventDependence) -> Self {
        Self {
            dependence: GateDependence::Uniform(dependence),
            repeated: RepeatedEvents::Reject,
        }
    }
}

/// A p-box for every gate of the tree.
#[derive(Debug, Clone, PartialEq)]
pub struct PboxTreeResult {
    gates: Vec<Pbox>,
    top: usize,
}

impl PboxTreeResult {
    /// The p-box of the top event.
    pub fn top(&self) -> &Pbox {
        &self.gates[self.top]
    }

    /// The p-box of gate `index` (indexed as [`FaultTree::gates`]).
    pub fn gate(&self, index: usize) -> Option<&Pbox> {
        self.gates.get(index)
    }
}

fn bad(parameter: &str, value: f64, reason: String) -> RafflesError {
    RafflesError::InvalidParameter {
        parameter: parameter.to_string(),
        value,
        reason,
    }
}

/// Whether any basic event or gate reachable from the top is the argument of
/// more than one gate input slot.
fn has_repeated(tree: &FaultTree) -> bool {
    let mut seen_gates = vec![false; tree.gates().len()];
    let mut stack = vec![tree.top()];
    seen_gates[tree.top()] = true;
    let mut counts: HashMap<(bool, usize), usize> = HashMap::new();
    while let Some(g) = stack.pop() {
        for arg in tree.gates()[g].args() {
            match arg {
                Arg::BasicEvent(i) => *counts.entry((false, *i)).or_insert(0) += 1,
                Arg::Gate(j) => {
                    *counts.entry((true, *j)).or_insert(0) += 1;
                    if !seen_gates[*j] {
                        seen_gates[*j] = true;
                        stack.push(*j);
                    }
                }
                Arg::Constant(_) => {}
            }
        }
    }
    counts.values().any(|c| *c > 1)
}

fn dependence_of(options: &PboxTreeOptions, gate: usize, gates: usize) -> Result<EventDependence> {
    match &options.dependence {
        GateDependence::Uniform(d) => Ok(*d),
        GateDependence::PerGate(v) => {
            if v.len() != gates {
                return Err(RafflesError::DimensionMismatch {
                    expected: gates,
                    found: v.len(),
                });
            }
            Ok(v[gate])
        }
    }
}

fn combine(a: &Pbox, b: &Pbox, is_and: bool, d: EventDependence) -> Result<Pbox> {
    match (is_and, d) {
        (true, EventDependence::Independent) => and_independent(a, b),
        (false, EventDependence::Independent) => or_independent(a, b),
        (true, EventDependence::Unknown) => and_unknown(a, b),
        (false, EventDependence::Unknown) => or_unknown(a, b),
    }
}

fn fold(items: &[Pbox], is_and: bool, d: EventDependence) -> Result<Pbox> {
    let mut acc = items[0].clone();
    for next in &items[1..] {
        acc = combine(&acc, next, is_and, d)?;
    }
    Ok(acc)
}

fn subsets(n: usize, k: usize) -> Vec<Vec<usize>> {
    fn rec(start: usize, n: usize, k: usize, cur: &mut Vec<usize>, out: &mut Vec<Vec<usize>>) {
        if cur.len() == k {
            out.push(cur.clone());
            return;
        }
        for i in start..n {
            cur.push(i);
            rec(i + 1, n, k, cur, out);
            cur.pop();
        }
    }
    let mut out = Vec::new();
    rec(0, n, k, &mut Vec::new(), &mut out);
    out
}

fn gate_value(connective: Connective, inputs: &[Pbox], d: EventDependence) -> Result<Pbox> {
    match connective {
        Connective::And => fold(inputs, true, d),
        Connective::Or => fold(inputs, false, d),
        Connective::Null => Ok(inputs[0].clone()),
        Connective::Not => complement(&inputs[0]),
        Connective::Nand => complement(&fold(inputs, true, d)?),
        Connective::Nor => complement(&fold(inputs, false, d)?),
        Connective::Xor => Err(bad(
            "connective",
            0.0,
            "Xor is not monotone in the input probabilities; gate-wise p-box \
             propagation is not implemented for it"
                .to_string(),
        )),
        Connective::Atleast { min } => {
            if d == EventDependence::Independent {
                return Err(bad(
                    "dependence",
                    min as f64,
                    "an at-least gate shares events between its k-subsets, so it cannot be \
                     combined as independent; use EventDependence::Unknown"
                        .to_string(),
                ));
            }
            let combos = subsets(inputs.len(), min);
            if combos.len() > MAX_ATLEAST_SUBSETS {
                return Err(bad(
                    "min",
                    combos.len() as f64,
                    format!(
                        "at-least expansion has {} subsets (limit {MAX_ATLEAST_SUBSETS})",
                        combos.len()
                    ),
                ));
            }
            let mut terms = Vec::with_capacity(combos.len());
            for c in combos {
                let picked: Vec<Pbox> = c.iter().map(|i| inputs[*i].clone()).collect();
                terms.push(fold(&picked, true, d)?);
            }
            fold(&terms, false, d)
        }
    }
}

fn evaluate_inner(
    tree: &FaultTree,
    basic: &[Pbox],
    options: &PboxTreeOptions,
) -> Result<Vec<Option<Pbox>>> {
    if basic.len() != tree.basic_event_count() {
        return Err(RafflesError::DimensionMismatch {
            expected: tree.basic_event_count(),
            found: basic.len(),
        });
    }
    let levels = basic.first().map_or(2, |p| p.levels());
    let n_gates = tree.gates().len();
    let mut memo: Vec<Option<Pbox>> = vec![None; n_gates];
    // Post-order over the DAG (acyclic by construction of FaultTree).
    let mut stack = vec![(tree.top(), false)];
    while let Some((g, expanded)) = stack.pop() {
        if memo[g].is_some() {
            continue;
        }
        let gate = &tree.gates()[g];
        if !expanded {
            stack.push((g, true));
            for arg in gate.args() {
                if let Arg::Gate(j) = arg {
                    if memo[*j].is_none() {
                        stack.push((*j, false));
                    }
                }
            }
            continue;
        }
        let inputs: Vec<Pbox> = gate
            .args()
            .iter()
            .map(|a| match a {
                Arg::BasicEvent(i) => Ok(basic[*i].clone()),
                Arg::Gate(j) => Ok(memo[*j].clone().expect("child evaluated first")),
                Arg::Constant(v) => point(if *v { 1.0 } else { 0.0 }, levels),
            })
            .collect::<Result<_>>()?;
        let d = dependence_of(options, g, n_gates)?;
        memo[g] = Some(gate_value(gate.connective(), &inputs, d)?);
    }
    Ok(memo)
}

fn repeated_guard(tree: &FaultTree, options: &PboxTreeOptions) -> Result<()> {
    if options.repeated == RepeatedEvents::IndependentCopies || !has_repeated(tree) {
        return Ok(());
    }
    let any_independent = match &options.dependence {
        GateDependence::Uniform(d) => *d == EventDependence::Independent,
        GateDependence::PerGate(v) => v.contains(&EventDependence::Independent),
    };
    if any_independent {
        return Err(bad(
            "repeated",
            0.0,
            "a basic event or gate feeds the tree more than once and some gate is combined \
             as independent, which would count it twice; use Unknown dependence, \
             evaluate_cbox_edges, or RepeatedEvents::IndependentCopies knowingly"
                .to_string(),
        ));
    }
    Ok(())
}

/// Propagates one p-box per basic event through the tree, gate by gate.
///
/// `basic` is indexed as [`Arg::BasicEvent`]; all p-boxes share a ladder.
/// Gates not reachable from the top are `None` in no result: only reachable
/// gates are evaluated, and [`PboxTreeResult::gate`] returns `None` for the
/// rest.
///
/// # Errors
///
/// [`RafflesError::DimensionMismatch`] if `basic` has the wrong length or
/// ladders differ; [`RafflesError::InvalidParameter`] for a refused
/// connective, a refused repeated event, or a p-box outside `[0, 1]`.
pub fn evaluate_pbox_tree(
    tree: &FaultTree,
    basic: &[Pbox],
    options: &PboxTreeOptions,
) -> Result<PboxTreeResult> {
    repeated_guard(tree, options)?;
    let memo = evaluate_inner(tree, basic, options)?;
    Ok(PboxTreeResult {
        gates: memo
            .into_iter()
            .map(|g| g.unwrap_or_else(|| basic.first().cloned().expect("tree has events")))
            .collect(),
        top: tree.top(),
    })
}

/// The Lye et al. (2024) edge construction for **confidence boxes**: evaluate
/// the tree with every basic event replaced by its left edge, again with its
/// right edge, and take the envelope at every gate (Case study 4:
/// `e1f = env.pbox(e1f_L, e1f_R)`).
///
/// Valid for coherent trees whose inputs are c-boxes. Repeated events are
/// allowed with [`EventDependence::Unknown`] gates.
///
/// # Errors
///
/// As [`evaluate_pbox_tree`], and [`RafflesError::InvalidParameter`] if the
/// tree is not coherent.
pub fn evaluate_cbox_edges(
    tree: &FaultTree,
    basic: &[Pbox],
    options: &PboxTreeOptions,
) -> Result<PboxTreeResult> {
    if !tree.is_coherent() {
        return Err(bad(
            "tree",
            0.0,
            "the edge construction needs a coherent (monotone) tree".to_string(),
        ));
    }
    repeated_guard(tree, options)?;
    let edge = |upper_edge: bool| -> Result<Vec<Pbox>> {
        basic
            .iter()
            .map(|p| {
                let s = if upper_edge {
                    p.upper_quantiles()
                } else {
                    p.lower_quantiles()
                };
                Pbox::from_quantile_bounds(s.to_vec(), s.to_vec())
            })
            .collect()
    };
    let left = evaluate_inner(tree, &edge(false)?, options)?;
    let right = evaluate_inner(tree, &edge(true)?, options)?;
    let mut gates = Vec::with_capacity(left.len());
    for (l, r) in left.iter().zip(right.iter()) {
        gates.push(match (l, r) {
            (Some(l), Some(r)) => envelope(l, r)?,
            _ => basic.first().cloned().expect("tree has events"),
        });
    }
    Ok(PboxTreeResult {
        gates,
        top: tree.top(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scram::fault_tree::FaultTreeBuilder;
    use crate::scram::pba::kn;

    const L: usize = 201;

    fn series_and_parallel() -> (crate::scram::fault_tree::FaultTreeModel, usize, usize) {
        let mut b = FaultTreeBuilder::new();
        b.basic_event("a", 0.5).unwrap();
        b.basic_event("b", 0.5).unwrap();
        b.gate("g_and", Connective::And, &["a", "b"]).unwrap();
        b.gate("g_or", Connective::Or, &["g_and", "b"]).unwrap();
        (b.build("g_or").unwrap(), 0, 1)
    }

    /// **Methodology.** Precise basic events through `a AND b`, `(a AND b) OR
    /// c` etc. must give the plain probability: `a = 0.1`, `b = 0.2`,
    /// `c = 0.3` independent top `1 - (1-0.02)(1-0.3) = 0.314`, each exact to
    /// `1e-12`; a house event `true` is a probability-1 input.
    ///
    /// **Result** (2026-10-20): `0.314` reproduced at all 201 levels.
    #[test]
    fn precise_tree_reduces_to_probability_formula() {
        let mut b = FaultTreeBuilder::new();
        for n in ["a", "b", "c"] {
            b.basic_event(n, 0.5).unwrap();
        }
        b.house_event("h", true).unwrap();
        b.gate("ab", Connective::And, &["a", "b", "h"]).unwrap();
        b.gate("top", Connective::Or, &["ab", "c"]).unwrap();
        let m = b.build("top").unwrap();
        let basic = [0.1, 0.2, 0.3].map(|v| point(v, L).unwrap());
        let r = evaluate_pbox_tree(
            m.tree(),
            &basic,
            &PboxTreeOptions::uniform(EventDependence::Independent),
        )
        .unwrap();
        for i in 0..L {
            assert!((r.top().lower_quantiles()[i] - 0.314).abs() < 1e-12);
            assert!((r.top().upper_quantiles()[i] - 0.314).abs() < 1e-12);
        }
        assert!(r.gate(0).is_some());
        assert!(r.gate(99).is_none());
        // A per-gate policy and the wrong-length policy.
        let per = PboxTreeOptions {
            dependence: GateDependence::PerGate(vec![
                EventDependence::Independent,
                EventDependence::Unknown,
            ]),
            repeated: RepeatedEvents::Reject,
        };
        let r2 = evaluate_pbox_tree(m.tree(), &basic, &per).unwrap();
        // OR with Fréchet: [max(0.02,0.3), min(0.32,1)] = [0.3, 0.32]
        assert!((r2.top().lower_quantiles()[0] - 0.3).abs() < 1e-12);
        assert!((r2.top().upper_quantiles()[0] - 0.32).abs() < 1e-12);
        let wrong = PboxTreeOptions {
            dependence: GateDependence::PerGate(vec![EventDependence::Unknown]),
            repeated: RepeatedEvents::Reject,
        };
        assert!(evaluate_pbox_tree(m.tree(), &basic, &wrong).is_err());
    }

    /// **Methodology.** Repeated events: independent gate-wise evaluation is
    /// refused by default; `IndependentCopies` evaluates `(a AND b) OR b` as
    /// if the two `b`s were separate (`0.25 + 0.5 - 0.125 = 0.625` at
    /// `a = b = 0.5`); Fréchet is always allowed and must enclose both the
    /// copies value and the true value `P(b) = 0.5`.
    ///
    /// **Result** (2026-10-20): refusal observed; copies value `0.625`;
    /// Fréchet box `[0.5, 1.0]` (hand-derived: AND in `[0, 0.5]`, then OR gives
    /// `[max(0, 0.5), min(1, 0.5 + 0.5)]`) encloses `0.5` and `0.625`.
    #[test]
    fn repeated_events_are_refused_or_bounded() {
        let (m, _, _) = series_and_parallel();
        let basic = [0.5, 0.5].map(|v| point(v, L).unwrap());
        let ind = PboxTreeOptions::uniform(EventDependence::Independent);
        assert!(evaluate_pbox_tree(m.tree(), &basic, &ind).is_err());
        let copies = PboxTreeOptions {
            repeated: RepeatedEvents::IndependentCopies,
            ..ind
        };
        let r = evaluate_pbox_tree(m.tree(), &basic, &copies).unwrap();
        assert!((r.top().lower_quantiles()[3] - 0.625).abs() < 1e-12);
        let u = evaluate_pbox_tree(
            m.tree(),
            &basic,
            &PboxTreeOptions::uniform(EventDependence::Unknown),
        )
        .unwrap();
        assert!(u.top().lower_quantiles()[3] <= 0.5 + 1e-12);
        assert!(u.top().upper_quantiles()[3] >= 0.625 - 1e-12);
    }

    /// **Methodology.** Not/Nand/Nor are exact complements of Null/And/Or;
    /// Xor and independent Atleast are refused; a 2-of-3 gate with Fréchet
    /// and precise `p = 0.5` each must enclose the independent 2-of-3 value
    /// `0.5` (analytic: `3p^2 - 2p^3 = 0.5`) and the edge construction on a
    /// non-coherent tree is refused.
    ///
    /// **Result** (2026-10-20): Nand of precise 0.5, 0.5 is `0.75`; 2-of-3
    /// Fréchet box contains `0.5`.
    #[test]
    fn connectives_and_refusals() {
        let basic = [0.5, 0.5, 0.5].map(|v| point(v, L).unwrap());
        let mut b = FaultTreeBuilder::new();
        for n in ["a", "b", "c"] {
            b.basic_event(n, 0.5).unwrap();
        }
        b.gate("nand", Connective::Nand, &["a", "b"]).unwrap();
        b.gate("top", Connective::Nor, &["nand", "c"]).unwrap();
        let m = b.build("top").unwrap();
        let ind = PboxTreeOptions::uniform(EventDependence::Independent);
        let r = evaluate_pbox_tree(m.tree(), &basic, &ind).unwrap();
        assert!((r.gate(0).unwrap().lower_quantiles()[0] - 0.75).abs() < 1e-12);
        assert!(evaluate_cbox_edges(m.tree(), &basic, &ind).is_err());

        let mut x = FaultTreeBuilder::new();
        for n in ["a", "b"] {
            x.basic_event(n, 0.5).unwrap();
        }
        x.gate("top", Connective::Xor, &["a", "b"]).unwrap();
        let xm = x.build("top").unwrap();
        assert!(evaluate_pbox_tree(xm.tree(), &basic[..2], &ind).is_err());

        let mut v = FaultTreeBuilder::new();
        for n in ["a", "b", "c"] {
            v.basic_event(n, 0.5).unwrap();
        }
        v.gate("top", Connective::Atleast { min: 2 }, &["a", "b", "c"])
            .unwrap();
        let vm = v.build("top").unwrap();
        assert!(evaluate_pbox_tree(vm.tree(), &basic, &ind).is_err());
        let r = evaluate_pbox_tree(
            vm.tree(),
            &basic,
            &PboxTreeOptions::uniform(EventDependence::Unknown),
        )
        .unwrap();
        assert!(r.top().lower_quantiles()[0] <= 0.5);
        assert!(r.top().upper_quantiles()[0] >= 0.5);
        assert_eq!(subsets(4, 2).len(), 6);
    }

    /// **Methodology.** Edge construction on a c-box tree without repeated
    /// events, `KN(3,10) OR KN(1,5)` under Fréchet: the envelope of the two
    /// edge evaluations (unknown dependence) must have a 95 % cut that
    /// encloses the 95 % cut of the independent gate-wise c-box.
    ///
    /// **Result** (2026-10-20): holds.
    #[test]
    fn edge_construction_encloses_independent_result() {
        let mut b = FaultTreeBuilder::new();
        b.basic_event("a", 0.5).unwrap();
        b.basic_event("b", 0.5).unwrap();
        b.gate("top", Connective::Or, &["a", "b"]).unwrap();
        let m = b.build("top").unwrap();
        let basic = vec![kn(3, 10, L).unwrap(), kn(1, 5, L).unwrap()];
        let edges = evaluate_cbox_edges(
            m.tree(),
            &basic,
            &PboxTreeOptions::uniform(EventDependence::Unknown),
        )
        .unwrap();
        let ind = evaluate_pbox_tree(
            m.tree(),
            &basic,
            &PboxTreeOptions::uniform(EventDependence::Independent),
        )
        .unwrap();
        let (e, i) = (edges.top().cut(0.05).unwrap(), ind.top().cut(0.05).unwrap());
        assert!(e.lower() <= i.lower() + 1e-12 && e.upper() + 1e-12 >= i.upper());
    }
}
