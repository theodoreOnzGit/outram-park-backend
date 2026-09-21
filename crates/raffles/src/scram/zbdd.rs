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
// Upstream's module handling, its prime-implicant path
// (`ConvertBddPrimeImplicants`, reached under `--prime-implicants`) and its
// `EliminateComplements` are NOT ported. See the module doc for what that
// means for a non-coherent tree.
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
//! reason — upstream's ordinary path does the same, and only
//! `--prime-implicants` (not ported) keeps the complemented literals.
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
use super::fault_tree::FaultTree;
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
    /// Ordering position of the variable, shared with the source
    /// [`Bdd`] so the two walk in step.
    order: usize,
    /// The sub-family of sets that contain this variable, with it removed.
    high: SetId,
    /// The sub-family of sets that do not contain it.
    low: SetId,
}

struct Builder {
    nodes: Vec<SetNode>,
    unique: HashMap<SetNode, SetId>,
    convert_memo: HashMap<bdd::NodeId, SetId>,
    minimize_memo: HashMap<SetId, SetId>,
    subsume_memo: HashMap<(SetId, SetId), SetId>,
}

impl Builder {
    /// Returns the node for this triple, applying the **zero-suppression**
    /// rule and reusing an existing node.
    ///
    /// The rule — a node whose `high` branch is the empty family is dropped in
    /// favour of its `low` branch — is what distinguishes a ZBDD from a BDD,
    /// and it is why a variable in no set costs nothing.
    fn make(&mut self, order: usize, high: SetId, low: SetId) -> Result<SetId> {
        if high == EMPTY {
            return Ok(low);
        }
        let node = SetNode { order, high, low };
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

    fn order_of(&self, id: SetId) -> usize {
        match id {
            EMPTY | BASE => usize::MAX,
            _ => self.nodes[id - 2].order,
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
        let result = self.make(ite.order, high, low)?;
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

        let (ho, lo) = (self.order_of(high), self.order_of(low));
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
        // A set that contains this variable and is a superset of one that does
        // not is not minimal. `make` then drops the node if nothing survives.
        let high = self.subsume(high, low)?;
        let result = self.make(node.order, high, low)?;
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
        if limit_order.is_none_or(|k| current.len() < k) {
            current.push(bdd.basic_event(node.order));
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
    let mut builder = Builder {
        nodes: Vec::new(),
        unique: HashMap::new(),
        convert_memo: HashMap::new(),
        minimize_memo: HashMap::new(),
        subsume_memo: HashMap::new(),
    };
    let converted = builder.convert(bdd, bdd.root())?;
    let minimal = builder.minimize(converted)?;

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
    let mut builder = Builder {
        nodes: Vec::new(),
        unique: HashMap::new(),
        convert_memo: HashMap::new(),
        minimize_memo: HashMap::new(),
        subsume_memo: HashMap::new(),
    };
    let converted = builder.convert(&bdd, bdd.root())?;
    let minimal = builder.minimize(converted)?;
    Ok(builder.count(minimal, &mut HashMap::new()))
}
