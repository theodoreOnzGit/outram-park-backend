// ---------------------------------------------------------------------------
// The PROBABILITY CALCULATION below is ported from SCRAM (a probabilistic risk
// analysis tool).
//
//   Upstream project: SCRAM — Olzhas Rakhimov
//   Upstream repo:    https://github.com/rakhimov/scram
//   Upstream file:    src/probability_analysis.cc
//                     (`ProbabilityAnalyzer<Bdd>::CalculateProbability`)
//   Upstream commit:  b85b78940de38996eeffec54d946824bd4280a1c  (2019-07-03)
//   Accessed:         2026-09-21
//
//   Copyright (C) 2014-2018 Olzhas Rakhimov
//   Licensed under the GNU General Public License, version 3 or later.
//
// Same-licence port: SCRAM is GPL-3.0-or-later, RAFFLES is GPL-3.0-only.
//
// Translation notes: upstream's routine is
//
//     ite.p(p_var * high + (1 - p_var) * low);
//
// with `low = 1 - low` when the edge is complemented, `1` at the terminal, and
// a final `prob = 1 - prob` when the root itself is complemented. This port
// uses EXPLICIT zero and one terminals instead of complement edges, so those
// three flips disappear and the recurrence is the bare Shannon expansion. The
// complement-edge representation halves node count; it does not change a
// computed value, and the equivalence is checked against upstream's own
// numbers in the tests.
//
// Upstream's `mark` field (a flip-flop visited flag stored on the node) is a
// memo keyed by node identity; here that is an explicit map, because these
// nodes are shared immutable values in a `Vec` rather than mutable objects.
// Upstream's module indirection is not ported -- there is no preprocessor
// here to find modules.
//
// The REST of this file is not a port. Upstream's `Bdd` is built from a
// `Pdag` that a 2,411-line preprocessor has rewritten, and neither is ported;
// this is a direct Bryant-style construction from the fault tree, with a
// unique table and memoised apply. The algorithm is the standard one:
//
//   R. E. Bryant, "Graph-Based Algorithms for Boolean Function Manipulation",
//   IEEE Transactions on Computers C-35(8) (1986), 677-691,
//   doi:10.1109/TC.1986.1676819
// ---------------------------------------------------------------------------

//! Binary decision diagrams — the **exact** top-event probability, at any
//! scale.
//!
//! [`super::probability::Approximation::Exact`] computes the same quantity by
//! inclusion-exclusion over the minimal cut sets, which is `2^n` in their
//! number and refuses past
//! [`super::probability::EXACT_CUT_SET_LIMIT`]. This module has no such limit:
//! it evaluates the fault tree's Boolean function directly, so it scales to
//! models where enumerating cut sets is hopeless, and it needs no cut sets at
//! all.
//!
//! **For a non-coherent tree the two are not even the same number.** Minimal
//! cut sets are conservative — deleting the complemented literals discards the
//! requirement that a component be *working* — so quantifying them gives an
//! upper bound. The BDD evaluates the real function. On the fixture's small
//! non-coherent model that is `0.5032` here against `0.6220` from the cut
//! sets, and `0.5032` is the right answer.
//!
//! # Variable ordering
//!
//! A BDD's *size* depends heavily on the order its variables are tested in;
//! its *value* does not. This module orders variables by first appearance in a
//! depth-first walk from the top gate, which keeps related events adjacent and
//! is what makes the fixture's largest model tractable. Finding a good order
//! in general is NP-hard, and upstream spends its preprocessor on the problem;
//! nothing here does. A tree that blows up under this order would need that
//! work, and [`Bdd::node_count`] is how you would see it happening.
//!
//! # Example
//!
//! ```
//! use raffles::scram::fault_tree::{Connective, FaultTreeBuilder};
//! use raffles::scram::bdd::Bdd;
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
//! let bdd = Bdd::build(model.tree()).unwrap();
//! let p = bdd.probability(model.probabilities()).unwrap();
//! assert!((p - 0.7225).abs() < 1e-12);
//! ```

use std::collections::HashMap;

use super::fault_tree::{Arg, Connective, FaultTree};
use crate::{RafflesError, Result};

/// A node in the diagram: `0` is the constant **false**, `1` the constant
/// **true**, and anything above indexes [`Bdd::nodes`].
pub type NodeId = usize;

/// The constant-false terminal.
pub const ZERO: NodeId = 0;
/// The constant-true terminal.
pub const ONE: NodeId = 1;

/// Ceiling on how many distinct nodes may be created.
///
/// A BDD is exponential in the worst case, and on a bad variable order an
/// ordinary-looking model reaches that worst case. Hitting this returns an
/// error naming the limit rather than exhausting memory, for the same reason
/// [`super::mocus::EXPANSION_LIMIT`] exists: a diagnosable failure beats a
/// disappearing process.
pub const NODE_LIMIT: usize = 20_000_000;

/// One if-then-else node: test `variable`, follow `high` when it occurs and
/// `low` when it does not.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Ite {
    /// Position of the tested variable in the diagram's own ordering, **not**
    /// a basic-event index. [`Bdd::basic_event`] converts.
    pub order: usize,
    /// Followed when the variable occurs.
    pub high: NodeId,
    /// Followed when it does not.
    pub low: NodeId,
}

/// A reduced, ordered binary decision diagram of a fault tree's Boolean
/// function.
#[derive(Debug, Clone)]
pub struct Bdd {
    nodes: Vec<Ite>,
    /// `order_to_event[i]` is the basic-event index tested at position `i`.
    order_to_event: Vec<usize>,
    root: NodeId,
    /// Construction tables, kept so [`Bdd::conjoin`] can extend the diagram
    /// after the fact. A caller that only wants a probability never touches
    /// them; the prime-implicant recursion does, at every node.
    unique: HashMap<Ite, NodeId>,
    apply_memo: HashMap<(Op, NodeId, NodeId), NodeId>,
    not_memo: HashMap<NodeId, NodeId>,
}

/// The binary operations [`Builder::apply`] memoises.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Op {
    And,
    Or,
    Xor,
}

/// Construction state: the unique table that keeps the diagram reduced, and
/// the apply memo that keeps it from being exponential in time as well as
/// space.
struct Builder {
    nodes: Vec<Ite>,
    unique: HashMap<Ite, NodeId>,
    apply_memo: HashMap<(Op, NodeId, NodeId), NodeId>,
    not_memo: HashMap<NodeId, NodeId>,
    event_to_order: HashMap<usize, usize>,
}

impl Builder {
    fn node_limit_error(&self) -> RafflesError {
        RafflesError::InvalidParameter {
            parameter: "tree".to_string(),
            value: self.nodes.len() as f64,
            reason: format!(
                "the diagram exceeded {NODE_LIMIT} nodes. A BDD is exponential in the worst \
                 case and its size depends on the variable order, which this port does not \
                 optimise; upstream SCRAM spends a preprocessor on exactly that"
            ),
        }
    }

    /// Returns the node for this triple, reusing an existing one and
    /// collapsing a redundant test. These two reductions are what make the
    /// diagram canonical, so they belong here rather than at the call sites.
    fn make(&mut self, order: usize, high: NodeId, low: NodeId) -> Result<NodeId> {
        if high == low {
            // Testing the variable changes nothing.
            return Ok(high);
        }
        let ite = Ite { order, high, low };
        if let Some(&existing) = self.unique.get(&ite) {
            return Ok(existing);
        }
        if self.nodes.len() + 2 > NODE_LIMIT {
            return Err(self.node_limit_error());
        }
        // +2 because ids 0 and 1 are the terminals and are not stored.
        let id = self.nodes.len() + 2;
        self.nodes.push(ite);
        self.unique.insert(ite, id);
        Ok(id)
    }

    fn order_of(&self, node: NodeId) -> usize {
        match node {
            ZERO | ONE => usize::MAX,
            _ => self.nodes[node - 2].order,
        }
    }

    /// The two branches of `node` as seen at ordering position `order`.
    ///
    /// A node tested later than `order` does not depend on that variable, so
    /// both branches are the node itself — which is how a single `apply`
    /// handles operands at different levels.
    fn cofactors(&self, node: NodeId, order: usize) -> (NodeId, NodeId) {
        if node > ONE && self.nodes[node - 2].order == order {
            let ite = self.nodes[node - 2];
            (ite.high, ite.low)
        } else {
            (node, node)
        }
    }

    fn apply(&mut self, op: Op, a: NodeId, b: NodeId) -> Result<NodeId> {
        // Terminal cases, which are also what stops the recursion.
        match op {
            Op::And => {
                if a == ZERO || b == ZERO {
                    return Ok(ZERO);
                }
                if a == ONE {
                    return Ok(b);
                }
                if b == ONE || a == b {
                    return Ok(a);
                }
            }
            Op::Or => {
                if a == ONE || b == ONE {
                    return Ok(ONE);
                }
                if a == ZERO {
                    return Ok(b);
                }
                if b == ZERO || a == b {
                    return Ok(a);
                }
            }
            Op::Xor => {
                if a == b {
                    return Ok(ZERO);
                }
                if a == ZERO {
                    return Ok(b);
                }
                if b == ZERO {
                    return Ok(a);
                }
            }
        }
        // And/Or are commutative, so normalising the key halves the memo.
        let key = if op == Op::Xor || a <= b {
            (op, a, b)
        } else {
            (op, b, a)
        };
        if let Some(&hit) = self.apply_memo.get(&key) {
            return Ok(hit);
        }

        let order = self.order_of(a).min(self.order_of(b));
        let (a_high, a_low) = self.cofactors(a, order);
        let (b_high, b_low) = self.cofactors(b, order);
        let high = self.apply(op, a_high, b_high)?;
        let low = self.apply(op, a_low, b_low)?;
        let result = self.make(order, high, low)?;
        self.apply_memo.insert(key, result);
        Ok(result)
    }

    fn not(&mut self, node: NodeId) -> Result<NodeId> {
        match node {
            ZERO => return Ok(ONE),
            ONE => return Ok(ZERO),
            _ => {}
        }
        if let Some(&hit) = self.not_memo.get(&node) {
            return Ok(hit);
        }
        let ite = self.nodes[node - 2];
        let high = self.not(ite.high)?;
        let low = self.not(ite.low)?;
        let result = self.make(ite.order, high, low)?;
        self.not_memo.insert(node, result);
        Ok(result)
    }

    /// `if f then g else h`, by definition.
    fn ite(&mut self, f: NodeId, g: NodeId, h: NodeId) -> Result<NodeId> {
        let not_f = self.not(f)?;
        let then_branch = self.apply(Op::And, f, g)?;
        let else_branch = self.apply(Op::And, not_f, h)?;
        self.apply(Op::Or, then_branch, else_branch)
    }

    /// At least `k` of `args` hold.
    ///
    /// Built by the obvious recurrence rather than by OR-ing `C(n, k)`
    /// combinations: `at_least(i, j)` is
    /// `ite(args[i], at_least(i+1, j-1), at_least(i+1, j))`, which is `O(n*k)`
    /// applies instead of `O(C(n,k))`. `mocus` can afford the combinations
    /// because it needs each one as a separate cut set; here they would only
    /// be merged again.
    fn atleast(&mut self, args: &[NodeId], k: usize) -> Result<NodeId> {
        // `row[j]` is "at least j of the arguments not yet consumed".
        let mut row: Vec<NodeId> = (0..=k).map(|j| if j == 0 { ONE } else { ZERO }).collect();
        for &arg in args.iter().rev() {
            let mut next = row.clone();
            for j in 1..=k {
                next[j] = self.ite(arg, row[j - 1], row[j])?;
            }
            row = next;
        }
        Ok(row[k])
    }
}

impl Bdd {
    /// Builds the diagram of a fault tree's Boolean function.
    ///
    /// # Errors
    ///
    /// [`RafflesError::InvalidParameter`] if the diagram exceeds
    /// [`NODE_LIMIT`] nodes.
    /// The variable ordering this module uses: basic events by first
    /// appearance in a depth-first walk from the top gate.
    ///
    /// `result[i]` is the basic event tested at ordering position `i`.
    /// Exposed so [`super::zbdd`]'s graph route can order its literals the
    /// same way — a disagreement between the two routes is then a real
    /// disagreement and not an ordering artefact.
    pub fn variable_order(tree: &FaultTree) -> Vec<usize> {
        let mut order_to_event = Vec::new();
        let mut seen_events = std::collections::HashSet::new();
        let mut stack = vec![tree.top()];
        let mut seen_gates = vec![false; tree.gates().len()];
        while let Some(gate) = stack.pop() {
            if std::mem::replace(&mut seen_gates[gate], true) {
                continue;
            }
            // Reversed so that a left-to-right reading of the tree gives a
            // left-to-right variable order, which is what keeps the order
            // stable against an irrelevant change in traversal.
            for arg in tree.gates()[gate].args().iter().rev() {
                match *arg {
                    Arg::Gate(g) => stack.push(g),
                    // A house event is a constant, so it orders no variable.
                    Arg::Constant(_) => {}
                    Arg::BasicEvent(e) => {
                        if seen_events.insert(e) {
                            order_to_event.push(e);
                        }
                    }
                }
            }
        }
        order_to_event
    }

    pub fn build(tree: &FaultTree) -> Result<Self> {
        let order_to_event = Self::variable_order(tree);
        let event_to_order: HashMap<usize, usize> = order_to_event
            .iter()
            .enumerate()
            .map(|(order, &event)| (event, order))
            .collect();

        let mut builder = Builder {
            nodes: Vec::new(),
            unique: HashMap::new(),
            apply_memo: HashMap::new(),
            not_memo: HashMap::new(),
            event_to_order,
        };
        let mut gate_memo: HashMap<usize, NodeId> = HashMap::new();
        let root = build_gate(&mut builder, tree, tree.top(), &mut gate_memo)?;

        Ok(Self {
            nodes: builder.nodes,
            order_to_event,
            root,
            unique: builder.unique,
            apply_memo: builder.apply_memo,
            not_memo: builder.not_memo,
        })
    }

    /// How many if-then-else nodes the diagram holds.
    ///
    /// The honest measure of whether the variable order is working. Two
    /// diagrams of the same function can differ in this by orders of
    /// magnitude.
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    /// The root node.
    pub fn root(&self) -> NodeId {
        self.root
    }

    /// Whether the function is the constant false — a tree that cannot fail.
    pub fn is_never(&self) -> bool {
        self.root == ZERO
    }

    /// Whether the function is the constant true — a tree that always fails.
    pub fn is_always(&self) -> bool {
        self.root == ONE
    }

    /// The basic event tested at ordering position `order`.
    pub fn basic_event(&self, order: usize) -> usize {
        self.order_to_event[order]
    }

    /// How many variables the diagram orders.
    pub fn variable_count(&self) -> usize {
        self.order_to_event.len()
    }

    /// The conjunction of two nodes, extending the diagram as needed.
    ///
    /// Exposed for [`super::zbdd`]'s prime-implicant recursion, whose
    /// consensus term is upstream's `Bdd::Consensus` — that is,
    /// `Apply<kAnd>(ite->high(), ite->low())`. Nothing else should need to
    /// grow the diagram after construction.
    ///
    /// # Errors
    ///
    /// [`RafflesError::InvalidParameter`] if the diagram exceeds
    /// [`NODE_LIMIT`].
    pub fn conjoin(&mut self, a: NodeId, b: NodeId) -> Result<NodeId> {
        let mut builder = Builder {
            nodes: std::mem::take(&mut self.nodes),
            unique: std::mem::take(&mut self.unique),
            apply_memo: std::mem::take(&mut self.apply_memo),
            not_memo: std::mem::take(&mut self.not_memo),
            event_to_order: HashMap::new(),
        };
        let result = builder.apply(Op::And, a, b);
        self.nodes = std::mem::take(&mut builder.nodes);
        self.unique = std::mem::take(&mut builder.unique);
        self.apply_memo = std::mem::take(&mut builder.apply_memo);
        self.not_memo = std::mem::take(&mut builder.not_memo);
        result
    }

    /// The if-then-else at `node`, or `None` at a terminal.
    ///
    /// Exposed so [`super::zbdd`] can walk the diagram; a caller wanting a
    /// probability or a decision should not need it.
    pub fn ite(&self, node: NodeId) -> Option<Ite> {
        (node > ONE).then(|| self.nodes[node - 2])
    }

    /// Exact probability that the top event occurs.
    ///
    /// `event_probabilities[i]` is the probability of basic event `i`. Basic
    /// events are assumed **independent**, as they are throughout this module.
    ///
    /// This is upstream's `ProbabilityAnalyzer<Bdd>::CalculateProbability`:
    /// each node contributes `p * P(high) + (1 - p) * P(low)`, memoised on the
    /// node. Because the diagram is reduced, a shared subfunction is evaluated
    /// once however many paths reach it — which is what makes this linear in
    /// the diagram rather than in the number of cut sets.
    ///
    /// # Errors
    ///
    /// [`RafflesError::InvalidParameter`] if a tested variable is out of range
    /// for `event_probabilities`, or a probability is outside `[0, 1]` or not
    /// finite.
    pub fn probability(&self, event_probabilities: &[f64]) -> Result<f64> {
        for (order, &event) in self.order_to_event.iter().enumerate() {
            let p =
                *event_probabilities
                    .get(event)
                    .ok_or_else(|| RafflesError::InvalidParameter {
                        parameter: "event_probabilities".to_string(),
                        value: event as f64,
                        reason: format!(
                            "the diagram tests basic event {event} (ordering position {order}), \
                         which is out of range for {} probabilities",
                            event_probabilities.len()
                        ),
                    })?;
            if !(0.0..=1.0).contains(&p) || !p.is_finite() {
                return Err(RafflesError::InvalidParameter {
                    parameter: "basic-event probability".to_string(),
                    value: p,
                    reason: format!("probability of basic event {event} must lie in [0, 1]"),
                });
            }
        }

        let mut memo: HashMap<NodeId, f64> = HashMap::new();
        Ok(self.probability_of(self.root, event_probabilities, &mut memo))
    }

    fn probability_of(&self, node: NodeId, p: &[f64], memo: &mut HashMap<NodeId, f64>) -> f64 {
        match node {
            ZERO => return 0.0,
            ONE => return 1.0,
            _ => {}
        }
        if let Some(&hit) = memo.get(&node) {
            return hit;
        }
        let ite = self.nodes[node - 2];
        let p_var = p[self.order_to_event[ite.order]];
        let high = self.probability_of(ite.high, p, memo);
        let low = self.probability_of(ite.low, p, memo);
        // Upstream: `ite.p(p_var * high + (1 - p_var) * low);`
        let value = p_var * high + (1.0 - p_var) * low;
        memo.insert(node, value);
        value
    }
}

/// Builds the diagram of one gate, memoised so a shared subtree is built once.
fn build_gate(
    b: &mut Builder,
    tree: &FaultTree,
    gate: usize,
    memo: &mut HashMap<usize, NodeId>,
) -> Result<NodeId> {
    if let Some(&hit) = memo.get(&gate) {
        return Ok(hit);
    }
    let mut args = Vec::with_capacity(tree.gates()[gate].args().len());
    for arg in tree.gates()[gate].args() {
        args.push(match *arg {
            Arg::Gate(g) => build_gate(b, tree, g, memo)?,
            // A house event is exactly a terminal, which is why the diagram
            // needs no special case for one anywhere else: `apply` folds it
            // away and the reduction rules delete whatever it made redundant.
            Arg::Constant(true) => ONE,
            Arg::Constant(false) => ZERO,
            Arg::BasicEvent(e) => {
                let order = b.event_to_order[&e];
                b.make(order, ONE, ZERO)?
            }
        });
    }

    let result = match tree.gates()[gate].connective() {
        Connective::And | Connective::Null => {
            let mut acc = ONE;
            for a in args {
                acc = b.apply(Op::And, acc, a)?;
            }
            acc
        }
        Connective::Or => {
            let mut acc = ZERO;
            for a in args {
                acc = b.apply(Op::Or, acc, a)?;
            }
            acc
        }
        Connective::Nand => {
            let mut acc = ONE;
            for a in args {
                acc = b.apply(Op::And, acc, a)?;
            }
            b.not(acc)?
        }
        Connective::Nor => {
            let mut acc = ZERO;
            for a in args {
                acc = b.apply(Op::Or, acc, a)?;
            }
            b.not(acc)?
        }
        Connective::Not => b.not(args[0])?,
        Connective::Xor => b.apply(Op::Xor, args[0], args[1])?,
        Connective::Atleast { min } => b.atleast(&args, min)?,
    };
    memo.insert(gate, result);
    Ok(result)
}
