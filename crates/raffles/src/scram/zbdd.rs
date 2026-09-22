// ---------------------------------------------------------------------------
// Ported from SCRAM (a probabilistic risk analysis tool).
//
//   Upstream project: SCRAM — Olzhas Rakhimov
//   Upstream repo:    https://github.com/rakhimov/scram
//   Upstream file:    src/zbdd.cc  (`Zbdd::ConvertBdd`, `Zbdd::Minimize`,
//                                   `Zbdd::Subsume`)
//   Upstream commit:  b85b78940de38996eeffec54d946824bd4280a1c  (2019-07-03)
//   Accessed:         2026-09-21
//
//   Copyright (C) 2014-2018 Olzhas Rakhimov
//   Licensed under the GNU General Public License, version 3 or later.
//
// Same-licence port: SCRAM is GPL-3.0-or-later, RAFFLES is GPL-3.0-only.
//
// Translation notes: the three routines are ported structurally.
//
//   ConvertBdd -- upstream threads a `complement` flag through the walk
//   because its BDD uses complement edges; `super::bdd` uses explicit
//   terminals, so the flag disappears and the recursion is the bare
//   `node(x, convert(high), convert(low))`. Upstream also threads
//   `limit_order` for product-order truncation, which is kept here as an
//   `Option<usize>` rather than a sentinel.
//
//   Minimize -- ported as written: minimise both branches, `Subsume` the high
//   branch against the low, and let the zero-suppression rule drop the node
//   when the high branch empties. Upstream's `minimal()` flag on the node is a
//   memo; here that is an explicit map, because these nodes are shared
//   immutable values in a `Vec`.
//
//   Subsume -- ported as written, including its three-way split on the
//   operands' orders. Upstream compares `order` and then `index` because two
//   of its nodes can share an order; here a variable's ordering position is
//   unique, so the index tiebreak is unrepresentable rather than dropped.
//
//   ConvertBddPrimeImplicants -- ported, together with `Bdd::Consensus`
//   (which is `Apply<kAnd>(ite->high(), ite->low())`, so here a plain `And`
//   on the two branches). Upstream stores the sign in the node's `index` and
//   compares `order` then `index`; this port folds both into one monotone
//   key, `2*order` for a positive literal and `2*order + 1` for its
//   complement, which reproduces upstream's two-level comparison exactly --
//   positive sorts first there because `+i > -i`.
//
// Upstream's module handling and its `EliminateComplements` are NOT ported.
// See the module doc for what that means for a non-coherent tree.
// ---------------------------------------------------------------------------

//! Zero-suppressed decision diagrams — minimal cut sets **at scale**.
//!
//! [`super::mocus`] generates cut sets by the classical top-down expansion,
//! which is exponential and gives up on a real model:
//! `reference-data/scram`'s `Aralia/das9601` exhausts its five-million-state
//! ceiling at every order limit. This module gets the same answer from the
//! [`super::bdd::Bdd`] instead, where the work is proportional to the diagram
//! rather than to the number of intermediate sets — which is the whole reason
//! upstream reaches for a ZBDD, and the reason Rauzy's 1993 paper exists.
//!
//! A **zero-suppressed** diagram represents a *family of sets* rather than a
//! Boolean function, and its reduction rule is the one that matters here: a
//! node whose "present" branch is empty is dropped, so a variable absent from
//! every set in the family costs nothing. Cut sets are sparse — a model with
//! 108 basic events has cut sets of order 9 — which is exactly the shape that
//! rule is for.
//!
//! # Coherent and non-coherent
//!
//! The conversion keeps the variables taken on the "occurs" branch of each
//! path to `true`, and drops the rest. For a **coherent** tree that is exactly
//! the minimal cut sets. For a **non-coherent** one it discards the
//! requirement that some component be *working*, so the result is
//! conservative in the same way [`super::mocus`]'s is, and for the same
//! reason — upstream's ordinary path does the same.
//!
//! **[`prime_implicants`] is the exact alternative**, keeping the complemented
//! literals, and is upstream's `--prime-implicants`. It costs more: the
//! consensus term adds a third recursive call at every node, and upstream
//! itself does not finish it on the fixture's largest model.
//!
//! # Example
//!
//! ```
//! use raffles::scram::fault_tree::{Connective, FaultTreeBuilder};
//! use raffles::scram::zbdd::minimal_cut_sets;
//!
//! let mut b = FaultTreeBuilder::new();
//! b.basic_event("ValveOne", 0.5).unwrap();
//! b.basic_event("PumpOne", 0.7).unwrap();
//! b.basic_event("ValveTwo", 0.5).unwrap();
//! b.basic_event("PumpTwo", 0.7).unwrap();
//! b.gate("TrainOne", Connective::Or, &["ValveOne", "PumpOne"]).unwrap();
//! b.gate("TrainTwo", Connective::Or, &["ValveTwo", "PumpTwo"]).unwrap();
//! b.gate("TopEvent", Connective::And, &["TrainOne", "TrainTwo"]).unwrap();
//! let model = b.build("TopEvent").unwrap();
//!
//! let cut_sets = minimal_cut_sets(model.tree(), None).unwrap();
//! assert_eq!(cut_sets.len(), 4);
//! assert!(cut_sets.iter().all(|c| c.order() == 2));
//! ```

use std::collections::HashMap;

use super::bdd::{self, Bdd};
use super::fault_tree::{Arg, Connective, FaultTree};
use super::probability::CutSet;
use crate::{RafflesError, Result};

/// A node in the family diagram: `0` is the **empty family** (no sets at all),
/// `1` the family holding just the **empty set**, and anything above indexes
/// the builder's node table.
pub type SetId = usize;

/// The family containing no sets — a function that cannot be satisfied.
pub const EMPTY: SetId = 0;
/// The family containing exactly the empty set.
///
/// Not the same as [`EMPTY`], and the difference is the usual first
/// confusion: `EMPTY` has no members, `BASE` has one member which happens to
/// have no elements.
pub const BASE: SetId = 1;

/// Ceiling on how many distinct family nodes may be created.
///
/// Same purpose as [`super::bdd::NODE_LIMIT`]: fail with a name rather than
/// exhaust memory.
pub const NODE_LIMIT: usize = 20_000_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct SetNode {
    /// The **literal** tested, as one monotone key: `2 * order` for a
    /// variable occurring, `2 * order + 1` for it not occurring.
    ///
    /// Folding the sign into the key is what lets [`Builder::subsume`] and
    /// [`Builder::minimize`] serve both the minimal-cut-set and the
    /// prime-implicant families unchanged. Upstream keeps the sign in a
    /// separate `index` field and compares `order` then `index`; the two
    /// orderings agree, because `+i > -i` puts the positive literal first
    /// there as the even key does here.
    ///
    /// A minimal-cut-set family uses only even keys.
    key: usize,
    /// The sub-family of sets that contain this literal, with it removed.
    high: SetId,
    /// The sub-family of sets that do not contain it.
    low: SetId,
}

/// The key for "variable at `order` occurs".
fn positive(order: usize) -> usize {
    2 * order
}

/// The key for "variable at `order` does not occur".
fn negative(order: usize) -> usize {
    2 * order + 1
}

struct Builder {
    nodes: Vec<SetNode>,
    unique: HashMap<SetNode, SetId>,
    convert_memo: HashMap<bdd::NodeId, SetId>,
    minimize_memo: HashMap<SetId, SetId>,
    subsume_memo: HashMap<(SetId, SetId), SetId>,
    // --- the graph route's tables; unused by the BDD route ---
    and_memo: HashMap<(SetId, SetId), SetId>,
    or_memo: HashMap<(SetId, SetId), SetId>,
    gate_memo: HashMap<(usize, bool), SetId>,
    eliminate_memo: HashMap<SetId, SetId>,
    /// Basic-event index -> its ordering position, shared with
    /// [`super::bdd::Bdd`] so both routes order literals identically.
    event_order: HashMap<usize, usize>,
}

impl Builder {
    fn new() -> Self {
        Self {
            nodes: Vec::new(),
            unique: HashMap::new(),
            convert_memo: HashMap::new(),
            minimize_memo: HashMap::new(),
            subsume_memo: HashMap::new(),
            and_memo: HashMap::new(),
            or_memo: HashMap::new(),
            gate_memo: HashMap::new(),
            eliminate_memo: HashMap::new(),
            event_order: HashMap::new(),
        }
    }

    /// Materialises the family against an explicit order-to-event map, for
    /// the graph route, which has no [`Bdd`] to ask.
    fn collect_by_order(
        &self,
        id: SetId,
        order_to_event: &[usize],
        current: &mut Vec<usize>,
        limit_order: Option<usize>,
        out: &mut Vec<Vec<usize>>,
    ) {
        match id {
            EMPTY => return,
            BASE => {
                if !current.is_empty() {
                    let mut set = current.clone();
                    set.sort_unstable();
                    out.push(set);
                }
                return;
            }
            _ => {}
        }
        let node = self.nodes[id - 2];
        debug_assert!(
            node.key % 2 == 0,
            "complements must be eliminated before listing cut sets"
        );
        if limit_order.is_none_or(|k| current.len() < k) {
            current.push(order_to_event[node.key / 2]);
            self.collect_by_order(node.high, order_to_event, current, limit_order, out);
            current.pop();
        }
        self.collect_by_order(node.low, order_to_event, current, limit_order, out);
    }
}

impl Builder {
    /// Returns the node for this triple, applying the **zero-suppression**
    /// rule and reusing an existing node.
    ///
    /// The rule — a node whose `high` branch is the empty family is dropped in
    /// favour of its `low` branch — is what distinguishes a ZBDD from a BDD,
    /// and it is why a variable in no set costs nothing.
    fn make(&mut self, key: usize, high: SetId, low: SetId) -> Result<SetId> {
        if high == EMPTY {
            return Ok(low);
        }
        let node = SetNode { key, high, low };
        if let Some(&existing) = self.unique.get(&node) {
            return Ok(existing);
        }
        if self.nodes.len() + 2 > NODE_LIMIT {
            return Err(RafflesError::InvalidParameter {
                parameter: "tree".to_string(),
                value: self.nodes.len() as f64,
                reason: format!(
                    "the cut-set diagram exceeded {NODE_LIMIT} nodes. Lower the order limit, \
                     or ask only for the top-event probability, which `scram::bdd` computes \
                     without enumerating cut sets"
                ),
            });
        }
        let id = self.nodes.len() + 2;
        self.nodes.push(node);
        self.unique.insert(node, id);
        Ok(id)
    }

    fn key_of(&self, id: SetId) -> usize {
        match id {
            EMPTY | BASE => usize::MAX,
            _ => self.nodes[id - 2].key,
        }
    }

    /// Upstream `Zbdd::ConvertBdd`: the family of variable sets taken on the
    /// "occurs" branch along each path to `true`.
    fn convert(&mut self, bdd: &Bdd, node: bdd::NodeId) -> Result<SetId> {
        match node {
            bdd::ZERO => return Ok(EMPTY),
            bdd::ONE => return Ok(BASE),
            _ => {}
        }
        if let Some(&hit) = self.convert_memo.get(&node) {
            return Ok(hit);
        }
        let ite = bdd.ite(node).expect("not a terminal");
        let high = self.convert(bdd, ite.high)?;
        let low = self.convert(bdd, ite.low)?;
        let result = self.make(positive(ite.order), high, low)?;
        self.convert_memo.insert(node, result);
        Ok(result)
    }

    /// Upstream `Zbdd::Subsume`: every set in `high` that contains some set in
    /// `low`, removed.
    ///
    /// This is the operation minimisation is built from, and the only one here
    /// whose shape is not obvious. Read it as: walk both families in variable
    /// order, and wherever a set in `high` could still be a superset of a set
    /// in `low`, keep following; the moment it cannot, stop.
    fn subsume(&mut self, high: SetId, low: SetId) -> Result<SetId> {
        // Upstream: `if (low->terminal()) return value ? kEmpty_ : high;`
        // The empty set is contained in everything, so a `low` of {∅} removes
        // every set in `high`.
        if low == BASE {
            return Ok(EMPTY);
        }
        if low == EMPTY {
            return Ok(high);
        }
        // Upstream: `if (high->terminal()) return high;`
        if high == EMPTY || high == BASE {
            return Ok(high);
        }
        if let Some(&hit) = self.subsume_memo.get(&(high, low)) {
            return Ok(hit);
        }

        let (ho, lo) = (self.key_of(high), self.key_of(low));
        let result = if ho > lo {
            // `high` never tests `low`'s variable, so only the sets in `low`
            // that omit it can subsume anything here.
            let r = self.subsume(high, self.nodes[low - 2].low)?;
            r
        } else if ho == lo {
            let (h_node, l_node) = (self.nodes[high - 2], self.nodes[low - 2]);
            let mut subhigh = self.subsume(h_node.high, l_node.high)?;
            subhigh = self.subsume(subhigh, l_node.low)?;
            let sublow = self.subsume(h_node.low, l_node.low)?;
            self.make(ho, subhigh, sublow)?
        } else {
            let h_node = self.nodes[high - 2];
            let subhigh = self.subsume(h_node.high, low)?;
            let sublow = self.subsume(h_node.low, low)?;
            self.make(ho, subhigh, sublow)?
        };
        self.subsume_memo.insert((high, low), result);
        Ok(result)
    }

    /// Upstream `Zbdd::Minimize`: the family reduced to its minimal members.
    fn minimize(&mut self, id: SetId) -> Result<SetId> {
        if id == EMPTY || id == BASE {
            return Ok(id);
        }
        if let Some(&hit) = self.minimize_memo.get(&id) {
            return Ok(hit);
        }
        let node = self.nodes[id - 2];
        let high = self.minimize(node.high)?;
        let low = self.minimize(node.low)?;
        // A set that contains this literal and is a superset of one that does
        // not is not minimal. `make` then drops the node if nothing survives.
        let high = self.subsume(high, low)?;
        let result = self.make(node.key, high, low)?;
        self.minimize_memo.insert(id, result);
        Ok(result)
    }

    /// How many sets the family holds, without materialising them.
    fn count(&self, id: SetId, memo: &mut HashMap<SetId, u128>) -> u128 {
        match id {
            EMPTY => return 0,
            BASE => return 1,
            _ => {}
        }
        if let Some(&hit) = memo.get(&id) {
            return hit;
        }
        let node = self.nodes[id - 2];
        let n = self.count(node.high, memo) + self.count(node.low, memo);
        memo.insert(id, n);
        n
    }

    /// Materialises the family, dropping any set above `limit_order`.
    fn collect(
        &self,
        id: SetId,
        bdd: &Bdd,
        current: &mut Vec<usize>,
        limit_order: Option<usize>,
        out: &mut Vec<Vec<usize>>,
    ) {
        match id {
            EMPTY => return,
            BASE => {
                if !current.is_empty() {
                    let mut set = current.clone();
                    set.sort_unstable();
                    out.push(set);
                }
                return;
            }
            _ => {}
        }
        let node = self.nodes[id - 2];
        debug_assert!(node.key % 2 == 0, "a cut-set family holds no complements");
        if limit_order.is_none_or(|k| current.len() < k) {
            current.push(bdd.basic_event(node.key / 2));
            self.collect(node.high, bdd, current, limit_order, out);
            current.pop();
        }
        self.collect(node.low, bdd, current, limit_order, out);
    }
}

/// The minimal cut sets of a fault tree, by way of its BDD.
///
/// Equivalent to [`super::mocus::minimal_cut_sets`] and enormously more
/// scalable: `mocus` enumerates intermediate sets, this walks a diagram. On
/// `Aralia/das9601` — 288 gates, non-coherent — `mocus` gives up and this
/// does not.
///
/// `limit_order` discards cut sets above that order, as upstream's
/// `limit_order` setting does; `None` keeps all of them. Note the truncation
/// happens when the family is materialised, so unlike `mocus` it does not
/// reduce the work — it is for trimming the answer, not for making a hard
/// model tractable.
///
/// Returned cut sets are sorted by order then by member index, matching
/// `mocus`, so the two can be compared directly.
///
/// # Errors
///
/// [`RafflesError::InvalidParameter`] if the [`Bdd`] or the family diagram
/// exceeds its node limit.
///
/// # A non-coherent tree's answer is conservative
///
/// See the module doc: the complemented literals are dropped, exactly as in
/// `mocus` and in upstream's ordinary (non-prime-implicant) path.
pub fn minimal_cut_sets(tree: &FaultTree, limit_order: Option<usize>) -> Result<Vec<CutSet>> {
    let bdd = Bdd::build(tree)?;
    from_bdd(&bdd, limit_order)
}

/// The minimal cut sets of an already-built diagram.
///
/// Use this rather than [`minimal_cut_sets`] when the [`Bdd`] is also wanted
/// for its probability — building it twice is the expensive half.
///
/// # Errors
///
/// [`RafflesError::InvalidParameter`] if the family diagram exceeds
/// [`NODE_LIMIT`].
pub fn from_bdd(bdd: &Bdd, limit_order: Option<usize>) -> Result<Vec<CutSet>> {
    let mut builder = Builder::new();
    let converted = builder.convert(bdd, bdd.root())?;
    let minimal = builder.minimize(converted)?;
    // The family holding just the empty set is upstream's "UNITY/Base": the
    // top event is unconditional. See `super::mocus::unity_error`.
    if minimal == BASE {
        return Err(super::mocus::unity_error());
    }

    let mut raw = Vec::new();
    builder.collect(minimal, bdd, &mut Vec::new(), limit_order, &mut raw);
    raw.sort_unstable();
    raw.dedup();
    raw.sort_by(|a, b| a.len().cmp(&b.len()).then_with(|| a.cmp(b)));
    raw.into_iter()
        .map(|members| CutSet::new(&members))
        .collect()
}

/// How many minimal cut sets a tree has, without materialising them.
///
/// The count comes off the diagram in time proportional to its size, so a
/// model with millions of cut sets can be counted even where listing them is
/// hopeless. Untruncated — an order limit is a property of the listing, not of
/// the family.
///
/// # Errors
///
/// [`RafflesError::InvalidParameter`] if either diagram exceeds its node
/// limit.
pub fn count_minimal_cut_sets(tree: &FaultTree) -> Result<u128> {
    let bdd = Bdd::build(tree)?;
    let mut builder = Builder::new();
    let converted = builder.convert(&bdd, bdd.root())?;
    let minimal = builder.minimize(converted)?;
    Ok(builder.count(minimal, &mut HashMap::new()))
}

/// A **prime implicant**: a minimal condition sufficient for the top event,
/// recording both what must fail and what must hold.
///
/// This is what a minimal cut set becomes once complemented literals are kept.
/// On a **coherent** tree the two coincide and [`negative`](Self::negative) is
/// always empty — no component working can ever help cause failure. On a
/// **non-coherent** tree they differ, and the difference is the whole point:
/// cut sets discard the negative literals and so describe a strictly larger
/// function, while prime implicants describe the real one.
///
/// Members are indices into the basic-event probability slice, as
/// [`CutSet`]'s are.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PrimeImplicant {
    positive: Vec<usize>,
    negative: Vec<usize>,
}

impl PrimeImplicant {
    /// Basic events that must **occur**, ascending.
    pub fn positive(&self) -> &[usize] {
        &self.positive
    }

    /// Basic events that must **not** occur, ascending.
    pub fn negative(&self) -> &[usize] {
        &self.negative
    }

    /// Total number of literals — upstream reports this as the product's
    /// `order`, counting a complemented literal like any other.
    pub fn order(&self) -> usize {
        self.positive.len() + self.negative.len()
    }

    /// Probability that this implicant holds, for independent basic events.
    ///
    /// The product of `p` over the positive members and `1 - p` over the
    /// negative ones. This is what upstream reports as a product's
    /// `probability` under `--prime-implicants`.
    ///
    /// # Errors
    ///
    /// [`RafflesError::InvalidParameter`] if a member is out of range, or a
    /// probability is outside `[0, 1]` or not finite.
    pub fn probability(&self, event_probabilities: &[f64]) -> Result<f64> {
        let mut product = 1.0;
        for (members, occurring) in [(&self.positive, true), (&self.negative, false)] {
            for &member in members {
                let p = *event_probabilities.get(member).ok_or_else(|| {
                    RafflesError::InvalidParameter {
                        parameter: "prime implicant member".to_string(),
                        value: member as f64,
                        reason: format!(
                            "basic-event index {member} is out of range for {} probabilities",
                            event_probabilities.len()
                        ),
                    }
                })?;
                if !(0.0..=1.0).contains(&p) || !p.is_finite() {
                    return Err(RafflesError::InvalidParameter {
                        parameter: "basic-event probability".to_string(),
                        value: p,
                        reason: format!("probability of basic event {member} must lie in [0, 1]"),
                    });
                }
                product *= if occurring { p } else { 1.0 - p };
            }
        }
        Ok(product)
    }
}

impl Builder {
    /// Upstream `Zbdd::ConvertBddPrimeImplicants`.
    ///
    /// The classical recursion: an implicant of `f` either contains `x`, or
    /// contains its complement, or contains neither — and the last case is
    /// precisely an implicant of the **consensus** `f_x AND f_not-x`, which is
    /// upstream's `Bdd::Consensus` and here a plain `And` on the two branches.
    ///
    /// Non-prime members are removed afterwards by [`Builder::minimize`],
    /// exactly as upstream does: `Minimize(ConvertBdd(...))`.
    fn convert_prime_implicants(&mut self, bdd: &mut Bdd, node: bdd::NodeId) -> Result<SetId> {
        match node {
            bdd::ZERO => return Ok(EMPTY),
            bdd::ONE => return Ok(BASE),
            _ => {}
        }
        if let Some(&hit) = self.convert_memo.get(&node) {
            return Ok(hit);
        }
        let ite = bdd.ite(node).expect("not a terminal");
        let consensus_node = bdd.conjoin(ite.high, ite.low)?;

        let high = self.convert_prime_implicants(bdd, ite.high)?;
        let low = self.convert_prime_implicants(bdd, ite.low)?;
        let consensus = self.convert_prime_implicants(bdd, consensus_node)?;

        // Upstream:
        //   GetReducedVertex(ite, false, high,
        //     GetReducedVertex(ite, true, low, consensus))
        let without = self.make(negative(ite.order), low, consensus)?;
        let result = self.make(positive(ite.order), high, without)?;
        self.convert_memo.insert(node, result);
        Ok(result)
    }

    /// Materialises a signed family.
    fn collect_signed(
        &self,
        id: SetId,
        bdd: &Bdd,
        pos: &mut Vec<usize>,
        neg: &mut Vec<usize>,
        out: &mut Vec<PrimeImplicant>,
    ) {
        match id {
            EMPTY => return,
            BASE => {
                if !pos.is_empty() || !neg.is_empty() {
                    let mut positive = pos.clone();
                    let mut negative = neg.clone();
                    positive.sort_unstable();
                    negative.sort_unstable();
                    out.push(PrimeImplicant { positive, negative });
                }
                return;
            }
            _ => {}
        }
        let node = self.nodes[id - 2];
        let event = bdd.basic_event(node.key / 2);
        if node.key % 2 == 0 {
            pos.push(event);
            self.collect_signed(node.high, bdd, pos, neg, out);
            pos.pop();
        } else {
            neg.push(event);
            self.collect_signed(node.high, bdd, pos, neg, out);
            neg.pop();
        }
        self.collect_signed(node.low, bdd, pos, neg, out);
    }
}

/// The prime implicants of a fault tree.
///
/// Where [`minimal_cut_sets`] discards complemented literals — making its
/// answer conservative on a non-coherent tree — this keeps them, so the result
/// describes the tree's function **exactly**. On a coherent tree the two are
/// the same sets and every implicant's [`PrimeImplicant::negative`] is empty.
///
/// Returned implicants are sorted by order, then positive members, then
/// negative, so two runs compare equal.
///
/// # Cost
///
/// Substantially more than cut sets, because the consensus term adds a third
/// recursive call at every node. **Upstream is no faster**: `scram
/// --prime-implicants` does not finish within five minutes on the fixture's
/// `Aralia/das9601`, where its minimal cut sets take under a second. Expect
/// this to be usable on small and medium trees only.
///
/// # Errors
///
/// [`RafflesError::InvalidParameter`] if either diagram exceeds its node
/// limit.
///
/// # Example
///
/// ```
/// use raffles::scram::fault_tree::{Connective, FaultTreeBuilder};
/// use raffles::scram::zbdd::prime_implicants;
///
/// // `a AND NOT b` -- failure needs `a` to fail AND `b` to be working.
/// let mut b = FaultTreeBuilder::new();
/// b.basic_event("a", 0.1).unwrap();
/// b.basic_event("b", 0.2).unwrap();
/// b.gate("NotB", Connective::Not, &["b"]).unwrap();
/// b.gate("Top", Connective::And, &["a", "NotB"]).unwrap();
/// let model = b.build("Top").unwrap();
///
/// let pis = prime_implicants(model.tree()).unwrap();
/// assert_eq!(pis.len(), 1);
/// assert_eq!(pis[0].positive(), &[0]);   // a must occur
/// assert_eq!(pis[0].negative(), &[1]);   // b must not
/// assert!((pis[0].probability(model.probabilities()).unwrap() - 0.08).abs() < 1e-12);
/// ```
pub fn prime_implicants(tree: &FaultTree) -> Result<Vec<PrimeImplicant>> {
    let mut bdd = Bdd::build(tree)?;
    let mut builder = Builder::new();
    let root = bdd.root();
    let converted = builder.convert_prime_implicants(&mut bdd, root)?;
    let minimal = builder.minimize(converted)?;

    let mut out = Vec::new();
    builder.collect_signed(minimal, &bdd, &mut Vec::new(), &mut Vec::new(), &mut out);
    out.sort_unstable();
    out.dedup();
    out.sort_by(|a, b| {
        a.order()
            .cmp(&b.order())
            .then_with(|| a.positive.cmp(&b.positive))
            .then_with(|| a.negative.cmp(&b.negative))
    });
    Ok(out)
}

// ===========================================================================
// The non-BDD route: a ZBDD built straight from the gate graph.
//
// Ported from SCRAM's `Zbdd::ConvertGraph`, `Zbdd::Apply<kAnd>`,
// `Zbdd::Apply<kOr>` and `Zbdd::EliminateComplements` (src/zbdd.cc, same
// commit and licence as the header at the top of this file).
//
// Translation notes: upstream threads `limit_order` through every `Apply` as
// a pruning optimisation and tracks `MayBeUnity` to decide when to decrement
// it. Neither is ported -- this route applies the order limit when the family
// is listed, as `from_bdd` does, so the answers match and only the work
// differs. Upstream's module handling is likewise absent, there being no
// preprocessor here to find modules.
//
// Upstream requires its input already in negation normal form with only AND
// and OR gates ("Complements must be pushed down to variables", and
// `Apply` asserts `kOr` for anything else) -- its preprocessor guarantees
// that. There is no preprocessor here, so [`Builder::convert_gate`] does the
// same job on the fly: it carries a sign through the recursion and expands
// each connective into its AND/OR form under that sign, which is the same De
// Morgan pushdown `super::mocus::branches` performs.
// ===========================================================================

impl Builder {
    /// Upstream `Zbdd::Apply<kAnd>` — the **product** of two families.
    ///
    /// Not intersection: the cut sets of `AND(f, g)` are every union of one
    /// set from `f` with one from `g`, which is what makes this the join and
    /// why it can grow.
    fn apply_and(&mut self, a: SetId, b: SetId) -> Result<SetId> {
        if a == EMPTY || b == EMPTY {
            return Ok(EMPTY);
        }
        if a == BASE {
            return Ok(b);
        }
        if b == BASE || a == b {
            return Ok(a);
        }
        if let Some(&hit) = self.and_memo.get(&(a.min(b), a.max(b))) {
            return Ok(hit);
        }
        // Upstream orders the operands so the first tests the earlier
        // literal; its two-level (order, index) comparison is this one key.
        let (one, two) = if self.key_of(a) <= self.key_of(b) {
            (a, b)
        } else {
            (b, a)
        };
        let (k1, k2) = (self.key_of(one), self.key_of(two));
        let (n1, n2) = (self.nodes[one - 2], self.nodes[two - 2]);

        let (high, low) = if k1 == k2 {
            // Same literal:
            //   (x*f1 + f0) * (x*g1 + g0) = x*(f1*(g1 + g0) + f0*g1) + f0*g0
            let g = self.apply_or(n2.high, n2.low)?;
            let t1 = self.apply_and(n1.high, g)?;
            let t2 = self.apply_and(n1.low, n2.high)?;
            (self.apply_or(t1, t2)?, self.apply_and(n1.low, n2.low)?)
        } else if k1 / 2 == k2 / 2 {
            // Same variable, opposite signs:
            //   (x*f1 + f0) * (~x*g1 + g0) = x*f1*g0 + f0*(~x*g1 + g0)
            (
                self.apply_and(n1.high, n2.low)?,
                self.apply_and(n1.low, two)?,
            )
        } else {
            (self.apply_and(n1.high, two)?, self.apply_and(n1.low, two)?)
        };

        let result = self.reduce_after_apply(k1, high, low)?;
        self.and_memo.insert((a.min(b), a.max(b)), result);
        Ok(result)
    }

    /// Upstream `Zbdd::Apply<kOr>` — the **union** of two families.
    fn apply_or(&mut self, a: SetId, b: SetId) -> Result<SetId> {
        if a == BASE || b == BASE {
            return Ok(BASE);
        }
        if a == EMPTY {
            return Ok(b);
        }
        if b == EMPTY || a == b {
            return Ok(a);
        }
        if let Some(&hit) = self.or_memo.get(&(a.min(b), a.max(b))) {
            return Ok(hit);
        }
        let (one, two) = if self.key_of(a) <= self.key_of(b) {
            (a, b)
        } else {
            (b, a)
        };
        let (k1, k2) = (self.key_of(one), self.key_of(two));
        let (n1, n2) = (self.nodes[one - 2], self.nodes[two - 2]);

        let (high, low) = if k1 == k2 {
            (
                self.apply_or(n1.high, n2.high)?,
                self.apply_or(n1.low, n2.low)?,
            )
        } else {
            // `x` and `~x` both leading straight to {∅} covers everything:
            // upstream's `x + ~x = 1`.
            if k1 / 2 == k2 / 2 && n1.high == BASE && n2.high == BASE {
                let result = BASE;
                self.or_memo.insert((a.min(b), a.max(b)), result);
                return Ok(result);
            }
            (n1.high, self.apply_or(n1.low, two)?)
        };

        let result = self.reduce_after_apply(k1, high, low)?;
        self.or_memo.insert((a.min(b), a.max(b)), result);
        Ok(result)
    }

    /// The tail both `Apply` specialisations share.
    ///
    /// Upstream: a `high` branch that still tests the *same variable* is
    /// absorbed into its own `low`, then `Minimize(GetReducedVertex(...))`.
    /// The absorption is what keeps a literal from appearing twice on one
    /// path.
    fn reduce_after_apply(&mut self, key: usize, high: SetId, low: SetId) -> Result<SetId> {
        let high = if high > BASE && self.key_of(high) / 2 == key / 2 {
            self.nodes[high - 2].low
        } else {
            high
        };
        let reduced = self.make(key, high, low)?;
        self.minimize(reduced)
    }

    /// Upstream `Zbdd::EliminateComplements`.
    ///
    /// A node testing a **complemented** literal is replaced by the union of
    /// its two branches — which deletes the literal, since the sets that had
    /// it and the sets that did not become one family. Positive nodes are
    /// kept. This is the step that turns prime implicants into (conservative)
    /// minimal cut sets, and it is why the BDD route's `convert` can simply
    /// never create a negative node in the first place.
    fn eliminate_complements(&mut self, id: SetId) -> Result<SetId> {
        if id == EMPTY || id == BASE {
            return Ok(id);
        }
        if let Some(&hit) = self.eliminate_memo.get(&id) {
            return Ok(hit);
        }
        let node = self.nodes[id - 2];
        let high = self.eliminate_complements(node.high)?;
        let low = self.eliminate_complements(node.low)?;
        let result = if node.key % 2 == 1 {
            // Upstream: `return Apply<kOr>(high, low, limit_order);`
            self.apply_or(high, low)?
        } else {
            let reduced = self.make(node.key, high, low)?;
            self.minimize(reduced)?
        };
        self.eliminate_memo.insert(id, result);
        Ok(result)
    }

    /// Upstream `Zbdd::ConvertGraph`, with the negation pushdown its
    /// preprocessor would have done already folded in.
    ///
    /// `negated` is the sign the gate was reached under. Each connective
    /// expands to an AND-fold or an OR-fold of its arguments under that sign,
    /// which is the De Morgan dual when negated.
    fn convert_gate(&mut self, tree: &FaultTree, gate: usize, negated: bool) -> Result<SetId> {
        if let Some(&hit) = self.gate_memo.get(&(gate, negated)) {
            return Ok(hit);
        }
        // A single literal is the family {{x}}.
        let literal = |b: &mut Self, arg: Arg, sign: bool| -> Result<SetId> {
            match arg {
                Arg::Constant(v) => Ok(if v != sign { BASE } else { EMPTY }),
                Arg::BasicEvent(e) => {
                    let key = if sign {
                        negative(b.event_order[&e])
                    } else {
                        positive(b.event_order[&e])
                    };
                    b.make(key, BASE, EMPTY)
                }
                Arg::Gate(_) => unreachable!("gates are converted, not made into literals"),
            }
        };
        let resolve = |b: &mut Self, arg: Arg, sign: bool| -> Result<SetId> {
            match arg {
                Arg::Gate(g) => b.convert_gate(tree, g, sign),
                other => literal(b, other, sign),
            }
        };

        let args = tree.gates()[gate].args().to_vec();
        let fold_and = |b: &mut Self, sign: bool| -> Result<SetId> {
            let mut acc = BASE;
            for arg in &args {
                let v = resolve(b, *arg, sign)?;
                acc = b.apply_and(acc, v)?;
            }
            Ok(acc)
        };
        let fold_or = |b: &mut Self, sign: bool| -> Result<SetId> {
            let mut acc = EMPTY;
            for arg in &args {
                let v = resolve(b, *arg, sign)?;
                acc = b.apply_or(acc, v)?;
            }
            Ok(acc)
        };
        // At least `k` of the arguments, by the same recurrence the BDD uses:
        // row[j] is "at least j of those not yet consumed".
        let choose = |b: &mut Self, k: usize, sign: bool| -> Result<SetId> {
            let mut row: Vec<SetId> = (0..=k).map(|j| if j == 0 { BASE } else { EMPTY }).collect();
            for arg in args.iter().rev() {
                let v = resolve(b, *arg, sign)?;
                let mut next = row.clone();
                for j in 1..=k {
                    let taken = b.apply_and(v, row[j - 1])?;
                    next[j] = b.apply_or(taken, row[j])?;
                }
                row = next;
            }
            Ok(row[k])
        };

        let result = match (tree.gates()[gate].connective(), negated) {
            (Connective::And, false) | (Connective::Nand, true) => fold_and(self, false)?,
            (Connective::And, true) | (Connective::Nand, false) => fold_or(self, true)?,
            (Connective::Or, false) | (Connective::Nor, true) => fold_or(self, false)?,
            (Connective::Or, true) | (Connective::Nor, false) => fold_and(self, true)?,
            (Connective::Null, sign) => fold_and(self, sign)?,
            (Connective::Not, sign) => fold_and(self, !sign)?,
            (Connective::Atleast { min }, false) => choose(self, min, false)?,
            (Connective::Atleast { min }, true) => choose(self, args.len() - min + 1, true)?,
            (Connective::Xor, sign) => {
                // XOR is `a and not b` or `not a and b`; its negation, IFF,
                // is `both` or `neither`.
                let (s0, s1) = if sign { (false, true) } else { (false, false) };
                let a0 = resolve(self, args[0], s0)?;
                let b0 = resolve(self, args[1], !s1)?;
                let left = self.apply_and(a0, b0)?;
                let a1 = resolve(self, args[0], !s0)?;
                let b1 = resolve(self, args[1], s1)?;
                let right = self.apply_and(a1, b1)?;
                self.apply_or(left, right)?
            }
        };
        self.gate_memo.insert((gate, negated), result);
        Ok(result)
    }
}

/// The minimal cut sets of a fault tree, **without building a BDD**.
///
/// This is upstream's other route: `Zbdd::ConvertGraph` folds the gate graph
/// bottom-up with ZBDD `Apply`, then `EliminateComplements` deletes the
/// complemented literals and `Minimize` absorbs. It is the path
/// `Zbdd(const Gate&, const Settings&)` takes, and the one upstream's MOCUS
/// drives.
///
/// [`minimal_cut_sets`] answers the same question through a BDD. Keeping both
/// is the point: they share the `Minimize`/`Subsume` tail and nothing else,
/// so agreeing is evidence. `tests/scram_graph_zbdd.rs` checks them against
/// each other, against [`super::mocus`], and against SCRAM's own products.
///
/// `limit_order` discards cut sets above that order, applied when the family
/// is listed; `None` keeps all of them.
///
/// # Errors
///
/// [`RafflesError::InvalidParameter`] if the diagram exceeds [`NODE_LIMIT`],
/// or if the tree's minimal cut sets are the empty set (see
/// [`super::mocus`]'s unity error).
pub fn minimal_cut_sets_from_graph(
    tree: &FaultTree,
    limit_order: Option<usize>,
) -> Result<Vec<CutSet>> {
    // Variable order: first appearance in a depth-first walk from the top,
    // matching `super::bdd::Bdd::build` so the two routes order their
    // literals identically and a disagreement cannot be an ordering artefact.
    let bdd_order = Bdd::variable_order(tree);
    let mut builder = Builder::new();
    builder.event_order = bdd_order
        .iter()
        .enumerate()
        .map(|(order, &event)| (event, order))
        .collect();

    let converted = builder.convert_gate(tree, tree.top(), false)?;
    let eliminated = builder.eliminate_complements(converted)?;
    let minimal = builder.minimize(eliminated)?;
    if minimal == BASE {
        return Err(super::mocus::unity_error());
    }

    let mut raw = Vec::new();
    builder.collect_by_order(minimal, &bdd_order, &mut Vec::new(), limit_order, &mut raw);
    raw.sort_unstable();
    raw.dedup();
    raw.sort_by(|a, b| a.len().cmp(&b.len()).then_with(|| a.cmp(b)));
    raw.into_iter().map(|m| CutSet::new(&m)).collect()
}
