// PROVENANCE
//   Upstream project: CYCLUS <https://github.com/cyclus/cyclus>
//   Upstream file:    src/greedy_solver.h, src/greedy_solver.cc,
//                     src/exchange_solver.h, src/exchange_solver.cc
//   Upstream commit:  d4faab7ce0566ccb8febfcf50915cdeedee59db0
//   Upstream licence: BSD-3-Clause
//   Copyright (c) 2010-2016, University of Wisconsin Computational Nuclear
//   Engineering Research Group. All rights reserved.
//
//   Independent Rust translation, (C) 2026 Theodore Ong and the outram-park
//   contributors, GPL-3.0-only. Relicensing is ONE-WAY.

//! The greedy solver — Cyclus's default, and the only one here.
//!
//! # The algorithm, in full
//!
//! [Condition](crate::exchange::preconditioner) the graph, then walk every
//! request group in the order conditioning left them. Within a group, walk the
//! request nodes in order of descending average preference. For each request
//! node, sort its arcs by the requester's preference for them, descending, and
//! walk those. For each arc, offer it the smaller of (what is still needed)
//! and (what the arc's two endpoints can still supply given every constraint
//! they are under). If that is more than [`EPS`], record the match, decrement
//! both endpoints' group capacities, and move on. Stop when the group's demand
//! is met or its request nodes run out.
//!
//! It is greedy in the strict sense: no assignment is ever revisited. A
//! request that grabs a supplier's last capacity keeps it even if a later,
//! keener request would have valued it more. That is why the ordering work in
//! the preconditioner matters as much as the matching work here.
//!
//! # What is NOT here: the Coin-OR solver
//!
//! Upstream also ships `ProgSolver`, a mixed-integer-programme formulation
//! solved through `OsiCbcSolverInterface` (Coin-OR / Cbc), which finds a true
//! optimum rather than a greedy one. It is **deliberately not ported**, and it
//! is not a gap to be filled later on a whim: Cbc is a large C++ dependency, it
//! cannot be `no_std`, and this crate's single-dependency design (see the
//! crate root) exists precisely so a fuel-cycle model can run on a
//! microcontroller or in a browser. The greedy solver is what upstream selects
//! by default, and what the great majority of published Cyclus results were
//! produced with.
//!
//! One visible consequence: [`ExchangeNodeGroup::excl_node_groups`](crate::exchange::graph::ExchangeNodeGroup::excl_node_groups) is ported
//! and populated but never read, because it exists for the programme
//! formulation's "at most one of these arcs may carry flow" constraint. The
//! greedy solver enforces exclusivity per arc instead, through
//! [`Arc::excl_val`].
//!
//! # Two float comparisons that look wrong and are not
//!
//! Both are load-bearing, both are upstream's, and both are easy to "fix" into
//! a bug.
//!
//! **Exact equality against [`UNLIMITED`].** A group capacity of exactly
//! `f64::MAX` means *unbounded*. It is a sentinel, not a measurement: nothing
//! computes `f64::MAX` by accident, and the only way a capacity holds that
//! value is that a caller put it there. Comparing a sentinel by identity is
//! correct; comparing it with a tolerance would make a merely enormous
//! capacity behave as infinite. The sentinel is checked twice — when computing
//! a capacity (so the division `capacity / unit_capacity` is skipped, which
//! would otherwise turn `f64::MAX` into something finite) and when decrementing
//! one (so it stays unbounded forever).
//!
//! **ULP-distance equality for exclusive orders.** See
//! [`exclusive_value`](crate::exchange::graph::exclusive_value) for the
//! construction; the solver applies it a second time, to the quantity it is
//! about to match. Upstream's comment is worth repeating: the careful float
//! comparison "is vital for preventing false positive constraint violations
//! w.r.t. exclusivity-related capacity". An exclusive order whose available
//! capacity falls one or two ULP short of the required quantity is matched in
//! full; three ULP short and it is matched not at all. There is no middle
//! ground, by construction.

use alloc::vec;
use alloc::vec::Vec;
use core::cmp::Ordering;

use crate::agent::AgentId;
use crate::error::{CyclusError, Result};
use crate::exchange::graph::{Arc, ArcId, ExchangeGraph, GroupId, NodeId};
use crate::exchange::preconditioner::{avg_pref, GreedyPreconditioner};
use crate::limits::{float_distance, is_negative, EPS, FLOAT_ULP_EQ, UNLIMITED};

/// `std::min` for two floats, with upstream's exact tie/NaN behaviour
/// (`b < a ? b : a`). `f64::min` is not used because it is a `std` inherent
/// method and this crate is `no_std`.
#[inline]
fn min2(a: f64, b: f64) -> f64 {
    if b < a {
        b
    } else {
        a
    }
}

/// `std::max` for two floats (`a < b ? b : a`).
#[inline]
fn max2(a: f64, b: f64) -> f64 {
    if a < b {
        b
    } else {
        a
    }
}

/// Whether upstream's exclusive-order handling is applied at all.
///
/// Upstream `ExchangeSolver::kDefaultExclusive`, which is `true`.
pub const DEFAULT_EXCLUSIVE: bool = true;

/// The greedy exchange solver.
///
/// Create one, hand it a graph, read the [`Match`](crate::exchange::graph::Match)es
/// off the graph afterwards. A solver may be reused across time steps; every
/// call to [`solve`](Self::solve) resets its internal state.
///
/// Upstream `cyclus::GreedySolver`, which also inherits the pseudo-cost
/// machinery from `cyclus::ExchangeSolver`; both are here.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct GreedySolver {
    exclusive_orders: bool,
    conditioner: Option<GreedyPreconditioner>,
    n_qty: Vec<f64>,
    grp_caps: Vec<Vec<f64>>,
    obj: f64,
    unmatched: f64,
}

impl GreedySolver {
    /// A solver with exclusive orders enforced and a default (unweighted)
    /// preconditioner. Upstream's no-argument `GreedySolver()`.
    #[must_use]
    pub fn new() -> Self {
        Self {
            exclusive_orders: DEFAULT_EXCLUSIVE,
            conditioner: Some(GreedyPreconditioner::new()),
            n_qty: Vec::new(),
            grp_caps: Vec::new(),
            obj: 0.0,
            unmatched: 0.0,
        }
    }

    /// Chooses whether exclusive orders are enforced when costing arcs.
    ///
    /// Note the asymmetry, which is upstream's: this flag is consulted only by
    /// [`arc_cost`](Self::arc_cost) and hence by the pseudo-cost. The matching
    /// loop applies the all-or-nothing rule to any arc whose
    /// [`Arc::exclusive`] is set, regardless of this flag.
    #[must_use]
    pub fn with_exclusive_orders(mut self, exclusive_orders: bool) -> Self {
        self.exclusive_orders = exclusive_orders;
        self
    }

    /// Chooses the preconditioner, or `None` to leave the graph's order
    /// untouched.
    ///
    /// Upstream warns that "the GreedySolver is responsible for deleting its
    /// conditioner" and that passing `NULL` disables conditioning. Ownership is
    /// not a question here; the `None` case is kept because it is genuinely
    /// useful — a caller who has ordered the graph itself, or a test that wants
    /// to observe the raw matching order, passes `None`.
    #[must_use]
    pub fn with_conditioner(mut self, conditioner: Option<GreedyPreconditioner>) -> Self {
        self.conditioner = conditioner;
        self
    }

    /// Solves `graph` in place, appending a
    /// [`Match`](crate::exchange::graph::Match) for every flow, and returns
    /// the objective value.
    ///
    /// Upstream `GreedySolver::SolveGraph` (via `ExchangeSolver::Solve`).
    /// The steps, in upstream's order — the order matters, because the
    /// pseudo-cost is computed from the arcs *before* conditioning touches
    /// anything:
    ///
    /// 1. Compute the pseudo-cost of unmet demand.
    /// 2. Condition the graph.
    /// 3. Reset the objective, the unmatched total and the per-node tallies.
    /// 4. Snapshot every group's capacities into the solver's working copy.
    /// 5. Greedily satisfy each request group in turn.
    /// 6. Add `unmatched * pseudo_cost` to the objective.
    ///
    /// Existing matches on the graph are **not** cleared — call
    /// [`ExchangeGraph::clear_matches`] first when re-solving, as upstream
    /// requires.
    ///
    /// # Errors
    ///
    /// [`CyclusError::State`] if a node in the graph has no group;
    /// [`CyclusError::Value`] if a node is matched beyond its own quantity, or
    /// if a node's unit capacities do not line up with its group's capacities.
    pub fn solve(&mut self, graph: &mut ExchangeGraph) -> Result<f64> {
        let pseudo_cost = self.pseudo_cost(graph);

        if let Some(c) = &self.conditioner {
            c.condition(graph);
        }

        self.obj = 0.0;
        self.unmatched = 0.0;
        self.n_qty = vec![0.0; graph.nodes().len()];
        self.init(graph);

        // Taken after conditioning, which permutes this list.
        let groups: Vec<GroupId> = graph.request_groups().to_vec();
        for g in groups {
            self.greedily_satisfy_set(graph, g)?;
        }

        self.obj += self.unmatched * pseudo_cost;
        Ok(self.obj)
    }

    /// Snapshots every group's capacities into the solver's working copy.
    ///
    /// Upstream `GreedySolver::Init`, which calls `GetCaps` over the request
    /// and supply groups. Zeroing the per-node tallies, upstream's other job
    /// in `GetCaps`, is done in [`solve`](Self::solve) by sizing `n_qty` to the
    /// node arena.
    ///
    /// The graph's own capacities are never modified: a solved graph can be
    /// re-solved, and the caller can still read what the capacities originally
    /// were.
    fn init(&mut self, graph: &ExchangeGraph) {
        self.grp_caps = graph
            .groups()
            .iter()
            .map(|g| g.capacities().to_vec())
            .collect();
    }

    /// The solver's remaining working capacities for a group, in the same
    /// order as [`ExchangeNodeGroup::capacities`](crate::exchange::graph::ExchangeNodeGroup::capacities).
    ///
    /// Empty before the first [`solve`](Self::solve). Reading this after a
    /// solve is how a caller sees how much of each constraint was consumed —
    /// an entry still equal to [`UNLIMITED`] was never decremented, by design.
    #[must_use]
    pub fn group_capacities(&self, group: GroupId) -> &[f64] {
        self.grp_caps.get(group.0).map_or(&[][..], Vec::as_slice)
    }

    /// The quantity the solver has assigned to a node so far, in the
    /// resource's units.
    #[must_use]
    pub fn node_qty(&self, node: NodeId) -> f64 {
        self.n_qty.get(node.0).copied().unwrap_or(0.0)
    }

    /// The objective value of the last solve.
    ///
    /// The greedy solver *minimises* cost, and treats `1 / preference` as the
    /// unit cost of flow, so the objective is
    /// `sum(qty / pref) + unmatched * pseudo_cost`. Lower is better, and a
    /// solution that leaves demand unmet is penalised by a term guaranteed to
    /// exceed any real arc's cost.
    #[must_use]
    pub fn obj(&self) -> f64 {
        self.obj
    }

    /// The total demand left unfilled by the last solve, in the resource's
    /// units. Summed over every request group.
    #[must_use]
    pub fn unmatched(&self) -> f64 {
        self.unmatched
    }

    /// The flow an arc can carry, given how much its endpoints already carry.
    ///
    /// Upstream `GreedySolver::Capacity(const Arc&, double, double)`: the
    /// minimum of the two endpoints' capacities, where — and this is the line
    /// that is silent when wrong — the **request** side takes the *maximum*
    /// over its constraints and the **bid** side takes the *minimum*.
    ///
    /// The asymmetry is not a mistake. A bid's constraints are limits: every
    /// one must hold, so the binding one is the smallest. A request's
    /// constraints are demands: the request is satisfied only when all of them
    /// are met, so the governing one is the largest. Swapping them produces a
    /// solver that quietly under-supplies multi-constraint requests and
    /// over-commits multi-constraint bids, with no error anywhere.
    ///
    /// `u_curr_qty` and `v_curr_qty` are what has already been assigned to the
    /// request and bid nodes respectively.
    ///
    /// # Errors
    ///
    /// [`CyclusError::Key`] for an unknown arc or node,
    /// [`CyclusError::State`] for a node with no group.
    pub fn capacity_arc(
        &self,
        graph: &ExchangeGraph,
        arc: ArcId,
        u_curr_qty: f64,
        v_curr_qty: f64,
    ) -> Result<f64> {
        let a = *graph.arc(arc)?;
        let ucap = self.capacity_node(graph, a.unode(), arc, false, u_curr_qty)?;
        let vcap = self.capacity_node(graph, a.vnode(), arc, true, v_curr_qty)?;
        Ok(min2(ucap, vcap))
    }

    /// The flow a single node can still take along an arc.
    ///
    /// Upstream `GreedySolver::Capacity(ExchangeNode::Ptr, const Arc&, bool,
    /// double)`.
    ///
    /// For each constraint `i` the permitted flow is
    /// `group_capacity[i] / unit_capacity[i]`, except that a group capacity of
    /// exactly [`UNLIMITED`] yields `UNLIMITED` without dividing. `min_cap`
    /// selects the minimum of those (use `true` for a bid node) or the maximum
    /// (`false` for a request node); see [`capacity_arc`](Self::capacity_arc)
    /// for why. The result is then capped by the node's own remaining
    /// quantity, `node.qty - curr_qty`.
    ///
    /// A node with no unit capacities recorded for this arc is unconstrained,
    /// and its remaining quantity is returned directly.
    ///
    /// # Errors
    ///
    /// [`CyclusError::Key`] for an unknown node, [`CyclusError::State`] if the
    /// node has no group, [`CyclusError::Value`] if the node has more unit
    /// capacities on this arc than its group has capacities.
    pub fn capacity_node(
        &self,
        graph: &ExchangeGraph,
        node: NodeId,
        arc: ArcId,
        min_cap: bool,
        curr_qty: f64,
    ) -> Result<f64> {
        let n = graph.node(node)?;
        let group = n.group.ok_or(CyclusError::State(
            "a notion of node capacity requires a nodegroup",
        ))?;

        let unit_caps = match n.unit_capacities.get(&arc) {
            Some(v) if !v.is_empty() => v,
            _ => return Ok(n.qty - curr_qty),
        };

        let group_caps = self.group_capacities(group);
        if group_caps.len() < unit_caps.len() {
            return Err(CyclusError::Value(
                "a node has more unit capacities on an arc than its group has capacities",
            ));
        }

        let mut cap = if min_cap {
            f64::INFINITY
        } else {
            f64::NEG_INFINITY
        };
        for (i, &u_cap) in unit_caps.iter().enumerate() {
            let grp_cap = group_caps[i];
            // Sentinel check by exact equality — see the module doc.
            let c = if grp_cap == UNLIMITED {
                UNLIMITED
            } else {
                grp_cap / u_cap
            };
            cap = if min_cap { min2(cap, c) } else { max2(cap, c) };
        }

        Ok(min2(cap, n.qty - curr_qty))
    }

    /// Fills one request group as far as it can. Upstream
    /// `GreedySolver::GreedilySatisfySet`.
    fn greedily_satisfy_set(&mut self, graph: &mut ExchangeGraph, group: GroupId) -> Result<()> {
        // Stable sort by descending average preference, ties broken by
        // descending agent id. Stability is what makes the solution
        // reproducible when weights tie, which they routinely do.
        let mut nodes: Vec<NodeId> = graph.group(group)?.nodes().to_vec();
        nodes.sort_by(|&l, &r| avg_pref_cmp(graph, l, r));
        *graph.group_mut(group)?.nodes_mut() = nodes.clone();

        let target = graph.group(group)?.qty();
        let mut matched = 0.0;

        let mut i = 0;
        // Upstream's `match <= target`: the loop continues at exact equality,
        // which lets a zero-quantity group still visit its nodes.
        while matched <= target && i < nodes.len() {
            let req = nodes[i];
            let mut sorted: Vec<ArcId> = graph.arcs_for_node(req).to_vec();
            sorted.sort_by(|&l, &r| req_pref_cmp(graph, l, r));

            let mut j = 0;
            while matched <= target && j < sorted.len() {
                let a = sorted[j];
                let arc = *graph.arc(a)?;
                let (u, v) = (arc.unode(), arc.vnode());
                let remain = target - matched;

                let mut tomatch = min2(
                    remain,
                    self.capacity_arc(graph, a, self.node_qty(u), self.node_qty(v))?,
                );

                // Exclusivity adjustment: all, or nothing.
                if arc.exclusive() {
                    let excl_val = arc.excl_val();
                    let dist = float_distance(tomatch, excl_val);
                    tomatch = if dist >= FLOAT_ULP_EQ { 0.0 } else { excl_val };
                }

                if tomatch > EPS {
                    self.update_capacity(graph, u, a, tomatch)?;
                    self.update_capacity(graph, v, a, tomatch)?;
                    self.n_qty[u.0] += tomatch;
                    self.n_qty[v.0] += tomatch;
                    graph.add_match(a, tomatch);
                    matched += tomatch;
                    let pref = graph.node(u)?.pref(a);
                    self.update_obj(tomatch, pref);
                }
                j += 1;
            }
            i += 1;
        }

        self.unmatched += target - matched;
        Ok(())
    }

    /// Adds a matched flow's contribution to the objective.
    ///
    /// Upstream `GreedySolver::UpdateObj`: `obj += qty / pref`. The objective
    /// is a *minimand*, so the reciprocal of preference plays the role of a
    /// unit cost — a keenly preferred trade is a cheap one.
    fn update_obj(&mut self, qty: f64, pref: f64) {
        self.obj += qty / pref;
    }

    /// Decrements a node's group capacities by a matched flow.
    ///
    /// Upstream `GreedySolver::UpdateCapacity`. A capacity of exactly
    /// [`UNLIMITED`] is left alone rather than decremented — the second half
    /// of the sentinel protocol described in the module doc.
    ///
    /// # Deviation: mismatched capacity vectors
    ///
    /// Upstream asserts `unit_caps.size() == caps.size()` and, with asserts
    /// compiled out, reads past the end of `unit_caps` when they differ. Here:
    /// an *empty* unit-capacity vector means "unconstrained along this arc",
    /// so there is nothing to decrement and the group capacities are left
    /// untouched; any other mismatch is a [`CyclusError::Value`].
    fn update_capacity(
        &mut self,
        graph: &ExchangeGraph,
        node: NodeId,
        arc: ArcId,
        qty: f64,
    ) -> Result<()> {
        let n = graph.node(node)?;
        let group = n.group.ok_or(CyclusError::State(
            "a notion of node capacity requires a nodegroup",
        ))?;
        let unit_caps: &[f64] = n.unit_capacities.get(&arc).map_or(&[][..], Vec::as_slice);

        if !unit_caps.is_empty() {
            let caps = self
                .grp_caps
                .get_mut(group.0)
                .ok_or(CyclusError::Key("no such exchange node group"))?;
            if unit_caps.len() != caps.len() {
                return Err(CyclusError::Value(
                    "a node's unit capacities do not match its group's capacities",
                ));
            }
            for (i, cap) in caps.iter_mut().enumerate() {
                let prev = *cap;
                *cap = if prev == UNLIMITED {
                    UNLIMITED
                } else {
                    prev - qty * unit_caps[i]
                };
            }
        }

        if is_negative(n.qty - qty) {
            return Err(CyclusError::Value(
                "a bid or request was matched to a higher value than it was set at; \
                 check the portfolio's capacity constraints",
            ));
        }
        Ok(())
    }

    /// The cost of an arc. Upstream `ExchangeSolver::Cost`.
    ///
    /// `excl_val / pref` for an exclusive arc when exclusive orders are
    /// enforced, and `1 / pref` otherwise — the cost of one unit of flow,
    /// scaled by the whole order for an order that cannot be split.
    #[must_use]
    pub fn cost(arc: &Arc, exclusive_orders: bool) -> f64 {
        if exclusive_orders && arc.exclusive() {
            arc.excl_val() / arc.pref()
        } else {
            1.0 / arc.pref()
        }
    }

    /// [`cost`](Self::cost) with this solver's exclusive-orders setting.
    /// Upstream `ExchangeSolver::ArcCost`.
    #[must_use]
    pub fn arc_cost(&self, arc: &Arc) -> f64 {
        Self::cost(arc, self.exclusive_orders)
    }

    /// The per-unit cost charged to unmet demand, at upstream's default cost
    /// factor of `0.1`. Upstream `ExchangeSolver::PseudoCost()`.
    #[must_use]
    pub fn pseudo_cost(&self, graph: &ExchangeGraph) -> f64 {
        self.pseudo_cost_by_pref(graph, 1e-1)
    }

    /// The per-unit cost charged to unmet demand.
    ///
    /// Upstream `ExchangeSolver::PseudoCostByPref`: the largest arc cost in
    /// the graph, inflated by `(1 + cost_factor)`, so that leaving demand
    /// unmet is always more expensive than filling it on the worst available
    /// arc. That is what makes the objective comparable between solutions that
    /// satisfy different amounts of demand.
    ///
    /// The exclusive-value factor is divided back out for arcs whose
    /// `excl_val` is below unity, because such an arc's cost is *smaller* than
    /// its unit cost and would otherwise deflate the maximum.
    ///
    /// Returns `0.0` for a graph with no arcs — nothing can be filled, so
    /// nothing is charged, which is upstream's behaviour too.
    #[must_use]
    pub fn pseudo_cost_by_pref(&self, graph: &ExchangeGraph, cost_factor: f64) -> f64 {
        let mut max_cost = 0.0;
        for a in graph.arcs() {
            let factor = if a.exclusive() && a.excl_val() < 1.0 {
                1.0 / a.excl_val()
            } else {
                1.0
            };
            max_cost = max2(max_cost, self.arc_cost(a) * factor);
        }
        max_cost * (1.0 + cost_factor)
    }
}

/// Upstream `AvgPrefComp`: descending average preference, ties broken by
/// descending agent id so the order is total.
fn avg_pref_cmp(graph: &ExchangeGraph, l: NodeId, r: NodeId) -> Ordering {
    let (lpref, lid) = graph
        .node(l)
        .map_or((0.0, None), |n| (avg_pref(n), n.agent_id));
    let (rpref, rid) = graph
        .node(r)
        .map_or((0.0, None), |n| (avg_pref(n), n.agent_id));
    match rpref.total_cmp(&lpref) {
        Ordering::Equal => rid.cmp(&lid),
        other => other,
    }
}

/// Upstream `ReqPrefComp`: descending arc preference as the requester sees it,
/// ties broken by the two agent ids, descending and lexicographic.
fn req_pref_cmp(graph: &ExchangeGraph, l: ArcId, r: ArcId) -> Ordering {
    let key = |a: ArcId| -> (f64, Option<AgentId>, Option<AgentId>) {
        let Ok(arc) = graph.arc(a) else {
            return (0.0, None, None);
        };
        let pref = graph.node(arc.unode()).map_or(0.0, |n| n.pref(a));
        let u = graph.node(arc.unode()).map_or(None, |n| n.agent_id);
        let v = graph.node(arc.vnode()).map_or(None, |n| n.agent_id);
        (pref, u, v)
    };
    let (lp, lu, lv) = key(l);
    let (rp, ru, rv) = key(r);
    match rp.total_cmp(&lp) {
        Ordering::Equal => (ru, rv).cmp(&(lu, lv)),
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::exchange::graph::ExchangeNode;
    use crate::limits::almost_eq;
    use alloc::vec;

    /// `x` moved `n` representable values down (positive `x` only).
    fn down(x: f64, n: u64) -> f64 {
        f64::from_bits(x.to_bits() - n)
    }

    /// A graph builder for the tests: one request group, one supply group.
    struct Fixture {
        graph: ExchangeGraph,
        rg: GroupId,
        sg: GroupId,
    }

    impl Fixture {
        fn new(demand: f64) -> Self {
            let mut graph = ExchangeGraph::new();
            let rg = graph.add_request_group(demand);
            let sg = graph.add_supply_group();
            Self { graph, rg, sg }
        }

        fn request(&mut self, qty: f64) -> NodeId {
            self.graph
                .add_node(self.rg, ExchangeNode::new(qty).commodity("fuel").agent(AgentId(1)))
                .unwrap()
        }

        fn excl_request(&mut self, qty: f64) -> NodeId {
            self.graph
                .add_node(
                    self.rg,
                    ExchangeNode::new(qty)
                        .exclusive(true)
                        .commodity("fuel")
                        .agent(AgentId(1)),
                )
                .unwrap()
        }

        fn bid(&mut self, qty: f64) -> NodeId {
            self.graph
                .add_node(self.sg, ExchangeNode::new(qty).commodity("fuel").agent(AgentId(2)))
                .unwrap()
        }

        fn excl_bid(&mut self, qty: f64) -> NodeId {
            self.graph
                .add_node(
                    self.sg,
                    ExchangeNode::new(qty)
                        .exclusive(true)
                        .commodity("fuel")
                        .agent(AgentId(2)),
                )
                .unwrap()
        }

        /// Connects with a preference, and records it on both the arc and the
        /// request node exactly as the translator does.
        fn arc(&mut self, u: NodeId, v: NodeId, pref: f64) -> ArcId {
            let a = self.graph.add_arc(u, v).unwrap();
            self.graph.set_arc_pref(a, pref).unwrap();
            self.graph.node_mut(u).unwrap().prefs.insert(a, pref);
            a
        }

        /// Puts `unit_cap` of unit capacity on the bid side of `arc` for the
        /// group's single capacity.
        fn bid_unit_cap(&mut self, v: NodeId, a: ArcId, unit_cap: f64) {
            self.graph
                .node_mut(v)
                .unwrap()
                .unit_capacities
                .insert(a, vec![unit_cap]);
        }

        fn solve(&mut self) -> (GreedySolver, f64) {
            let mut s = GreedySolver::new();
            let obj = s.solve(&mut self.graph).unwrap();
            (s, obj)
        }
    }

    #[test]
    fn one_request_met_exactly_by_one_bid() {
        let mut f = Fixture::new(10.0);
        let u = f.request(10.0);
        let v = f.bid(10.0);
        let a = f.arc(u, v, 1.0);

        let (s, obj) = f.solve();
        assert_eq!(f.graph.matches().len(), 1);
        assert_eq!(f.graph.matches()[0].arc, a);
        assert_eq!(f.graph.matches()[0].qty, 10.0);
        assert_eq!(s.unmatched(), 0.0);
        // obj = qty/pref = 10/1, plus nothing for unmet demand.
        assert_eq!(obj, 10.0);
    }

    #[test]
    fn a_request_larger_than_one_bid_is_split_across_two() {
        let mut f = Fixture::new(10.0);
        let u = f.request(10.0);
        let v1 = f.bid(6.0);
        let v2 = f.bid(5.0);
        let a1 = f.arc(u, v1, 2.0);
        let a2 = f.arc(u, v2, 1.0);

        let (s, _) = f.solve();
        // The preferred bid is drained first, then the remainder comes from
        // the second bid -- 4 of its 5, not all 5.
        assert_eq!(f.graph.matches().len(), 2);
        assert_eq!(f.graph.matches()[0], crate::exchange::graph::Match { arc: a1, qty: 6.0 });
        assert_eq!(f.graph.matches()[1], crate::exchange::graph::Match { arc: a2, qty: 4.0 });
        let total: f64 = f.graph.matches().iter().map(|m| m.qty).sum();
        assert_eq!(total, 10.0);
        assert_eq!(s.unmatched(), 0.0);
        assert_eq!(s.node_qty(v2), 4.0);
    }

    #[test]
    fn demand_beyond_the_available_bids_is_reported_unmatched() {
        let mut f = Fixture::new(10.0);
        let u = f.request(10.0);
        let v1 = f.bid(4.0);
        let v2 = f.bid(3.0);
        f.arc(u, v1, 1.0);
        f.arc(u, v2, 1.0);

        let (s, _) = f.solve();
        let total: f64 = f.graph.matches().iter().map(|m| m.qty).sum();
        assert_eq!(total, 7.0);
        assert_eq!(s.unmatched(), 3.0);
    }

    #[test]
    fn the_higher_preference_bid_is_matched_first() {
        // The core promise of the solver: order by preference, not by
        // insertion. The keener arc is added second here on purpose.
        let mut f = Fixture::new(5.0);
        let u = f.request(5.0);
        let dull = f.bid(5.0);
        let keen = f.bid(5.0);
        let a_dull = f.arc(u, dull, 0.5);
        let a_keen = f.arc(u, keen, 2.0);

        let (s, _) = f.solve();
        assert_eq!(f.graph.matches().len(), 1);
        assert_eq!(f.graph.matches()[0].arc, a_keen);
        assert_eq!(f.graph.matches()[0].qty, 5.0);
        assert_eq!(s.node_qty(dull), 0.0);
        // And the dull arc exists, it simply never carried anything.
        assert_eq!(f.graph.arc(a_dull).unwrap().pref(), 0.5);
    }

    #[test]
    fn a_supply_constraint_binds_below_the_bid_quantity() {
        // The bidder offers 10 kg but has only 4 units of throughput, each kg
        // consuming one unit.
        let mut f = Fixture::new(10.0);
        let u = f.request(10.0);
        let v = f.bid(10.0);
        let a = f.arc(u, v, 1.0);
        f.bid_unit_cap(v, a, 1.0);
        f.graph.add_capacity(f.sg, 4.0).unwrap();

        let (s, _) = f.solve();
        assert_eq!(f.graph.matches().len(), 1);
        assert_eq!(f.graph.matches()[0].qty, 4.0);
        assert_eq!(s.unmatched(), 6.0);
        // The constraint is exhausted.
        assert_eq!(s.group_capacities(f.sg), &[0.0]);
    }

    #[test]
    fn a_unit_capacity_above_unity_converts_the_constraint() {
        // 100 units of capacity, but each kg consumes 2.5 of them: 40 kg.
        let mut f = Fixture::new(100.0);
        let u = f.request(100.0);
        let v = f.bid(100.0);
        let a = f.arc(u, v, 1.0);
        f.bid_unit_cap(v, a, 2.5);
        f.graph.add_capacity(f.sg, 100.0).unwrap();

        let (s, _) = f.solve();
        assert_eq!(f.graph.matches()[0].qty, 40.0);
        assert_eq!(s.group_capacities(f.sg), &[0.0]);
    }

    #[test]
    fn an_exclusive_bid_is_matched_in_full() {
        let mut f = Fixture::new(10.0);
        let u = f.request(10.0);
        let v = f.excl_bid(4.0);
        let a = f.arc(u, v, 1.0);
        assert_eq!(f.graph.arc(a).unwrap().excl_val(), 4.0);

        let (s, _) = f.solve();
        assert_eq!(f.graph.matches().len(), 1);
        assert_eq!(f.graph.matches()[0].qty, 4.0);
        assert_eq!(s.unmatched(), 6.0);
    }

    #[test]
    fn an_exclusive_bid_larger_than_the_demand_is_not_matched_at_all() {
        let mut f = Fixture::new(3.0);
        let u = f.request(3.0);
        let v = f.excl_bid(4.0);
        let a = f.arc(u, v, 1.0);
        // The node quantities already disagree, so the arc can never carry
        // flow: excl_val is zero.
        assert_eq!(f.graph.arc(a).unwrap().excl_val(), 0.0);

        let (s, _) = f.solve();
        assert!(f.graph.matches().is_empty());
        assert_eq!(s.unmatched(), 3.0);
    }

    #[test]
    fn an_exclusive_order_one_ulp_short_of_capacity_is_still_matched_in_full() {
        // Remaining capacity is 1 ULP below the exclusive quantity. Upstream's
        // ULP comparison exists precisely so this rounds up rather than
        // becoming a false constraint violation.
        let mut f = Fixture::new(10.0);
        let u = f.request(10.0);
        let v = f.excl_bid(4.0);
        let a = f.arc(u, v, 1.0);
        f.bid_unit_cap(v, a, 1.0);
        f.graph.add_capacity(f.sg, down(4.0, 1)).unwrap();

        let (_, _) = f.solve();
        assert_eq!(f.graph.matches().len(), 1);
        assert_eq!(f.graph.matches()[0].qty, 4.0);
    }

    #[test]
    fn an_exclusive_order_three_ulps_short_of_capacity_is_matched_not_at_all() {
        // Three ULP short is outside float_ulp_eq = 2, so the order is
        // refused outright. Zero, never a partial match: that is what
        // "exclusive" means.
        let mut f = Fixture::new(10.0);
        let u = f.request(10.0);
        let v = f.excl_bid(4.0);
        let a = f.arc(u, v, 1.0);
        f.bid_unit_cap(v, a, 1.0);
        f.graph.add_capacity(f.sg, down(4.0, 3)).unwrap();

        let (s, _) = f.solve();
        assert!(f.graph.matches().is_empty());
        assert_eq!(s.unmatched(), 10.0);
    }

    #[test]
    fn an_exclusive_order_at_exactly_two_ulps_short_is_matched_in_full() {
        // The threshold is `dist >= 2 -> reject`, so 2 ULP short is rejected
        // and 1 is accepted; check the boundary from both sides in one test.
        for (ulps, expect_match) in [(1_u64, true), (2, false)] {
            let mut f = Fixture::new(10.0);
            let u = f.request(10.0);
            let v = f.excl_bid(4.0);
            let a = f.arc(u, v, 1.0);
            f.bid_unit_cap(v, a, 1.0);
            f.graph.add_capacity(f.sg, down(4.0, ulps)).unwrap();
            let _ = f.solve();
            assert_eq!(f.graph.matches().is_empty(), !expect_match, "{ulps} ULP");
        }
    }

    #[test]
    fn an_exclusive_request_is_met_whole_or_not_at_all() {
        let mut f = Fixture::new(4.0);
        let u = f.excl_request(4.0);
        let v = f.bid(10.0);
        f.arc(u, v, 1.0);
        let (s, _) = f.solve();
        assert_eq!(f.graph.matches()[0].qty, 4.0);
        assert_eq!(s.unmatched(), 0.0);

        // Now the same request against a bid that cannot cover it.
        let mut f = Fixture::new(4.0);
        let u = f.excl_request(4.0);
        let v = f.bid(3.0);
        f.arc(u, v, 1.0);
        let (s, _) = f.solve();
        assert!(f.graph.matches().is_empty());
        assert_eq!(s.unmatched(), 4.0);
    }

    #[test]
    fn the_unlimited_sentinel_is_never_decremented() {
        let mut f = Fixture::new(10.0);
        let u = f.request(10.0);
        let v = f.bid(10.0);
        let a = f.arc(u, v, 1.0);
        f.bid_unit_cap(v, a, 1.0);
        f.graph.add_capacity(f.sg, UNLIMITED).unwrap();

        let (s, _) = f.solve();
        // The bid's own quantity still binds, so 10 flows...
        assert_eq!(f.graph.matches()[0].qty, 10.0);
        // ...but the capacity is exactly where it started, not
        // `f64::MAX - 10.0` (which would in fact also be f64::MAX, and that is
        // the trap: the protocol must hold for a capacity that is decremented
        // by something large, too).
        assert_eq!(s.group_capacities(f.sg), &[UNLIMITED]);
    }

    #[test]
    fn an_unlimited_capacity_does_not_divide_by_the_unit_capacity() {
        // With a unit capacity of 0.5, a finite capacity would double. The
        // sentinel must survive the division untouched, or it stops being a
        // sentinel after one call.
        let mut f = Fixture::new(10.0);
        let u = f.request(10.0);
        let v = f.bid(10.0);
        let a = f.arc(u, v, 0.5);
        f.bid_unit_cap(v, a, 0.5);
        f.graph.add_capacity(f.sg, UNLIMITED).unwrap();

        let mut s = GreedySolver::new();
        s.solve(&mut f.graph).unwrap();
        assert_eq!(s.group_capacities(f.sg), &[UNLIMITED]);
        // And 10 units still flowed, capped only by the bid's own quantity.
        assert_eq!(f.graph.matches()[0].qty, 10.0);
    }

    #[test]
    fn a_request_node_takes_the_maximum_over_its_constraints_and_a_bid_the_minimum() {
        // Two constraints on each side, deliberately disagreeing. The bid side
        // must bind at 2 (the smaller), the request side must demand 8 (the
        // larger) -- so the arc carries min(8, 2) = 2.
        let mut f = Fixture::new(10.0);
        let u = f.request(10.0);
        let v = f.bid(10.0);
        let a = f.arc(u, v, 1.0);
        f.graph.node_mut(v).unwrap().unit_capacities.insert(a, vec![1.0, 1.0]);
        f.graph.node_mut(u).unwrap().unit_capacities.insert(a, vec![1.0, 1.0]);
        f.graph.add_capacity(f.sg, 2.0).unwrap();
        f.graph.add_capacity(f.sg, 7.0).unwrap();
        f.graph.add_capacity(f.rg, 3.0).unwrap();
        f.graph.add_capacity(f.rg, 8.0).unwrap();

        let mut s = GreedySolver::new();
        s.init(&f.graph);
        assert_eq!(s.capacity_node(&f.graph, v, a, true, 0.0).unwrap(), 2.0);
        assert_eq!(s.capacity_node(&f.graph, u, a, false, 0.0).unwrap(), 8.0);
        assert_eq!(s.capacity_arc(&f.graph, a, 0.0, 0.0).unwrap(), 2.0);
    }

    #[test]
    fn the_objective_accumulates_flow_over_preference_plus_a_pseudo_cost_for_unmet_demand() {
        let mut f = Fixture::new(10.0);
        let u = f.request(10.0);
        let v = f.bid(4.0);
        f.arc(u, v, 2.0);

        let (s, obj) = f.solve();
        // Matched 4 at preference 2 -> 4/2 = 2. Unmatched 6.
        assert_eq!(s.unmatched(), 6.0);
        // Pseudo-cost: the worst arc cost (1/2) inflated by 1 + 0.1.
        let pc = s.pseudo_cost(&f.graph);
        assert!(almost_eq(pc, 0.55));
        assert!(almost_eq(obj, 2.0 + 6.0 * 0.55));
        assert!(almost_eq(s.obj(), obj));
    }

    #[test]
    fn the_pseudo_cost_exceeds_every_real_arc_cost() {
        // That is the property the whole construction exists for: unmet demand
        // must never look cheaper than filling it badly.
        let mut f = Fixture::new(10.0);
        let u = f.request(10.0);
        let v1 = f.bid(4.0);
        let v2 = f.bid(4.0);
        f.arc(u, v1, 4.0);
        f.arc(u, v2, 0.25);

        let s = GreedySolver::new();
        let pc = s.pseudo_cost(&f.graph);
        for a in f.graph.arcs() {
            assert!(pc > s.arc_cost(a), "pseudo cost {pc} vs arc cost");
        }
    }

    #[test]
    fn an_exclusive_arc_is_costed_by_its_whole_order() {
        let mut f = Fixture::new(10.0);
        let u = f.request(10.0);
        let v = f.excl_bid(4.0);
        let a = f.arc(u, v, 2.0);
        let arc = *f.graph.arc(a).unwrap();
        // With exclusive orders on: excl_val / pref = 4 / 2.
        assert_eq!(GreedySolver::cost(&arc, true), 2.0);
        // With them off: the plain unit cost 1 / pref.
        assert_eq!(GreedySolver::cost(&arc, false), 0.5);
        let s = GreedySolver::new().with_exclusive_orders(false);
        assert_eq!(s.arc_cost(&arc), 0.5);
    }

    #[test]
    fn a_request_with_no_bids_is_entirely_unmatched_and_does_not_panic() {
        // Upstream needs an explicit `count() > 0` guard here because
        // `std::map::at` would throw; the arena returns an empty slice.
        let mut f = Fixture::new(10.0);
        f.request(10.0);
        let (s, _) = f.solve();
        assert!(f.graph.matches().is_empty());
        assert_eq!(s.unmatched(), 10.0);
    }

    #[test]
    fn a_node_with_no_group_is_a_state_error() {
        let mut graph = ExchangeGraph::new();
        let rg = graph.add_request_group(1.0);
        let sg = graph.add_supply_group();
        let u = graph.add_node(rg, ExchangeNode::new(1.0)).unwrap();
        let v = graph.add_node(sg, ExchangeNode::new(1.0)).unwrap();
        let a = graph.add_arc(u, v).unwrap();
        // Detach the node from its group, as an un-added node would be.
        graph.node_mut(v).unwrap().group = None;
        graph.node_mut(v).unwrap().unit_capacities.insert(a, vec![1.0]);

        let mut s = GreedySolver::new();
        s.init(&graph);
        assert_eq!(
            s.capacity_node(&graph, v, a, true, 0.0).unwrap_err(),
            CyclusError::State("a notion of node capacity requires a nodegroup")
        );
    }

    #[test]
    fn two_request_groups_are_served_in_conditioned_order() {
        let mut graph = ExchangeGraph::new();
        let g_light = graph.add_request_group(5.0);
        let g_heavy = graph.add_request_group(5.0);
        let sg = graph.add_supply_group();
        let u_light = graph
            .add_node(g_light, ExchangeNode::new(5.0).commodity("eggs").agent(AgentId(1)))
            .unwrap();
        let u_heavy = graph
            .add_node(g_heavy, ExchangeNode::new(5.0).commodity("spam").agent(AgentId(2)))
            .unwrap();
        // One bid node, of limited quantity, reachable from both.
        let v = graph
            .add_node(sg, ExchangeNode::new(5.0).commodity("fuel").agent(AgentId(3)))
            .unwrap();
        let a_light = graph.add_arc(u_light, v).unwrap();
        let a_heavy = graph.add_arc(u_heavy, v).unwrap();
        graph.set_arc_pref(a_light, 1.0).unwrap();
        graph.set_arc_pref(a_heavy, 1.0).unwrap();
        graph.node_mut(u_light).unwrap().prefs.insert(a_light, 1.0);
        graph.node_mut(u_heavy).unwrap().prefs.insert(a_heavy, 1.0);

        let mut weights = alloc::collections::BTreeMap::new();
        weights.insert(alloc::string::String::from("spam"), 5.0);
        weights.insert(alloc::string::String::from("eggs"), 1.0);
        let mut s = GreedySolver::new()
            .with_conditioner(Some(GreedyPreconditioner::with_weights(weights)));
        s.solve(&mut graph).unwrap();

        // "spam" outweighs "eggs", so the heavy group takes the whole bid and
        // the light group goes unfilled -- the greedy solver never revisits.
        assert_eq!(graph.matches().len(), 1);
        assert_eq!(graph.matches()[0].arc, a_heavy);
        assert_eq!(s.unmatched(), 5.0);
    }

    #[test]
    fn a_match_below_the_epsilon_is_not_recorded() {
        let mut f = Fixture::new(10.0);
        let u = f.request(10.0);
        let v = f.bid(10.0);
        let a = f.arc(u, v, 1.0);
        f.bid_unit_cap(v, a, 1.0);
        // Capacity below EPS: the flow is numerical noise, not a trade.
        f.graph.add_capacity(f.sg, EPS / 2.0).unwrap();

        let (s, _) = f.solve();
        assert!(f.graph.matches().is_empty());
        assert_eq!(s.unmatched(), 10.0);
    }
}
