// PROVENANCE
//   Upstream project: CYCLUS <https://github.com/cyclus/cyclus>
//   Upstream file:    src/greedy_preconditioner.h, src/greedy_preconditioner.cc
//   Upstream commit:  d4faab7ce0566ccb8febfcf50915cdeedee59db0
//   Upstream licence: BSD-3-Clause
//   Copyright (c) 2010-2016, University of Wisconsin Computational Nuclear
//   Engineering Research Group. All rights reserved.
//
//   Independent Rust translation, (C) 2026 Theodore Ong and the outram-park
//   contributors, GPL-3.0-only. Relicensing is ONE-WAY.

//! Node and group ordering: deciding what the greedy solver sees first.
//!
//! A greedy solver serves whatever it meets first, so the order *is* the
//! policy. The preconditioner sets that order from two inputs: a
//! caller-supplied **commodity weight** (how important is this commodity?) and
//! the **average preference** of a node's arcs (how much does the requester
//! want these particular bids?).
//!
//! The node weight is
//!
//! ```text
//! w_node = w_commod * (1 + p / (1 + p))
//! ```
//!
//! where `p` is the node's average arc preference. The second factor is
//! bounded in `[1, 2)`, so **preference can never outrank commodity weight**:
//! a node whose commodity is twice as important always sorts ahead, however
//! keenly the other node's bids are preferred. That is the whole design, and
//! it is easy to lose by "simplifying" the formula.
//!
//! Conditioning then happens in three steps, matching upstream exactly:
//!
//! 1. Within each request group, sort nodes by node weight, descending.
//! 2. Compute each group's weight as the mean of its nodes' weights.
//! 3. Sort the request groups by group weight, descending.
//!
//! Every sort is **stable**, so nodes of equal weight keep the order they were
//! created in and the solution is reproducible. Upstream uses
//! `std::stable_sort` for the same reason; Rust's `sort_by` is stable, and
//! `sort_unstable_by` must never be substituted here.

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;

use crate::exchange::graph::{ExchangeGraph, ExchangeNode, GroupId, NodeId};

/// Whether commodity weights were given heaviest-first or lightest-first.
///
/// Upstream `GreedyPreconditioner::WgtOrder`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WgtOrder {
    /// Weights are given lightest-first and must be reversed:
    /// `w -> max + min - w`. Upstream `REVERSE`.
    Reverse,
    /// Weights are already heaviest-first and are used as given. Upstream
    /// `END`, the default.
    HeaviestFirst,
}

/// The mean preference across a node's arcs, or `0.0` if it has none.
///
/// Upstream `cyclus::AvgPref`. Only request nodes carry preferences, so a bid
/// node always scores `0.0`.
///
/// The sum is taken in [`ArcId`](crate::exchange::graph::ArcId) order, which
/// is arc creation order. Upstream sums in `std::map<Arc, double>` order,
/// which compares the arcs' *node pointers* — so the result there depends on
/// where the allocator happened to put the nodes, and two runs of the same
/// input can differ in the last bits. This ordering is deterministic.
#[must_use]
pub fn avg_pref(node: &ExchangeNode) -> f64 {
    if node.prefs.is_empty() {
        return 0.0;
    }
    let sum: f64 = node.prefs.values().sum();
    #[allow(clippy::cast_precision_loss)]
    let n = node.prefs.len() as f64;
    sum / n
}

/// The conditioning weight of a node: `w_commod * (1 + p / (1 + p))`.
///
/// Upstream `cyclus::NodeWeight`.
///
/// `weights` maps commodity name to weight; pass `None` (or an empty map) to
/// weight every commodity equally at `1.0`. `avg_pref` is the node's average
/// arc preference, from [`avg_pref`].
///
/// # A commodity missing from a non-empty map scores zero
///
/// Upstream's header says `Condition` "throws KeyError if a commodity is in
/// the graph but not in the weight mapping". The implementation does no such
/// thing: it indexes a `std::map` with `operator[]`, which default-constructs
/// a `0.0` and inserts it. The *code* is translated, not the comment, because
/// the code is what every existing simulation has been running against — but
/// the consequence is worth knowing: one misspelled commodity name silently
/// sends those requests to the back of the queue instead of failing loudly.
#[must_use]
pub fn node_weight(
    node: &ExchangeNode,
    weights: Option<&BTreeMap<String, f64>>,
    avg_pref: f64,
) -> f64 {
    let commod_weight = match weights {
        Some(w) if !w.is_empty() => w.get(&node.commod).copied().unwrap_or(0.0),
        _ => 1.0,
    };
    commod_weight * (1.0 + avg_pref / (1.0 + avg_pref))
}

/// The mean node weight across a group, or `0.0` for an empty group.
///
/// Upstream `cyclus::GroupWeight`. `avg_prefs` supplies each node's average
/// preference, so that it is computed once per node rather than once per
/// comparison.
#[must_use]
pub fn group_weight(
    graph: &ExchangeGraph,
    group: GroupId,
    weights: Option<&BTreeMap<String, f64>>,
    avg_prefs: &BTreeMap<NodeId, f64>,
) -> f64 {
    let Ok(g) = graph.group(group) else {
        return 0.0;
    };
    if g.nodes().is_empty() {
        return 0.0;
    }
    let mut sum = 0.0;
    for &n in g.nodes() {
        let Ok(node) = graph.node(n) else { continue };
        sum += node_weight(node, weights, avg_prefs.get(&n).copied().unwrap_or(0.0));
    }
    #[allow(clippy::cast_precision_loss)]
    let count = g.nodes().len() as f64;
    sum / count
}

/// Orders an [`ExchangeGraph`] for the greedy solver.
///
/// Upstream `cyclus::GreedyPreconditioner`. Conditioning is in place and
/// idempotent for a given graph: it only permutes node and group order.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct GreedyPreconditioner {
    commod_weights: BTreeMap<String, f64>,
}

impl GreedyPreconditioner {
    /// A preconditioner with no commodity weights: every commodity counts
    /// `1.0`, so ordering is by average preference alone.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// A preconditioner with commodity weights given **heaviest first** —
    /// a larger number means a more important commodity. Weights are assumed
    /// non-negative, as upstream assumes.
    #[must_use]
    pub fn with_weights(commod_weights: BTreeMap<String, f64>) -> Self {
        Self::with_weights_ordered(commod_weights, WgtOrder::HeaviestFirst)
    }

    /// A preconditioner with commodity weights in either direction.
    ///
    /// [`WgtOrder::Reverse`] maps each weight `w` to `max + min - w`, which
    /// reflects the set about its own midpoint and so turns a lightest-first
    /// ranking into a heaviest-first one while keeping the spacing.
    #[must_use]
    pub fn with_weights_ordered(commod_weights: BTreeMap<String, f64>, order: WgtOrder) -> Self {
        let mut me = Self { commod_weights };
        if !me.commod_weights.is_empty() {
            me.process_weights(order);
        }
        me
    }

    /// The commodity weights, after any reversal.
    #[must_use]
    pub fn commod_weights(&self) -> &BTreeMap<String, f64> {
        &self.commod_weights
    }

    fn process_weights(&mut self, order: WgtOrder) {
        if order != WgtOrder::Reverse {
            return;
        }
        let mut min = f64::INFINITY;
        let mut max = f64::NEG_INFINITY;
        for &w in self.commod_weights.values() {
            if w < min {
                min = w;
            }
            if w > max {
                max = w;
            }
        }
        for w in self.commod_weights.values_mut() {
            *w = max + min - *w;
        }
    }

    /// Reorders `graph`'s request groups, and the nodes within each, as
    /// described in the module doc.
    ///
    /// Upstream `GreedyPreconditioner::Condition`. Supply groups are not
    /// touched: the greedy solver walks requests, and reaches bids only
    /// through each request's own arc list.
    pub fn condition(&self, graph: &mut ExchangeGraph) {
        let weights = if self.commod_weights.is_empty() {
            None
        } else {
            Some(&self.commod_weights)
        };

        let mut avg_prefs: BTreeMap<NodeId, f64> = BTreeMap::new();
        let groups: Vec<GroupId> = graph.request_groups().to_vec();
        let mut group_weights: BTreeMap<GroupId, f64> = BTreeMap::new();

        for &g in &groups {
            let Ok(group) = graph.group(g) else { continue };
            let nodes: Vec<NodeId> = group.nodes().to_vec();

            for &n in &nodes {
                if let Ok(node) = graph.node(n) {
                    avg_prefs.insert(n, avg_pref(node));
                }
            }

            // Stable: nodes of equal weight keep creation order.
            let mut sorted = nodes;
            sorted.sort_by(|&l, &r| {
                let wl = graph.node(l).map_or(0.0, |n| {
                    node_weight(n, weights, avg_prefs.get(&l).copied().unwrap_or(0.0))
                });
                let wr = graph.node(r).map_or(0.0, |n| {
                    node_weight(n, weights, avg_prefs.get(&r).copied().unwrap_or(0.0))
                });
                wr.total_cmp(&wl)
            });
            if let Ok(group) = graph.group_mut(g) {
                *group.nodes_mut() = sorted;
            }

            group_weights.insert(g, group_weight(graph, g, weights, &avg_prefs));
        }

        let mut ordered = groups;
        ordered.sort_by(|l, r| {
            let wl = group_weights.get(l).copied().unwrap_or(0.0);
            let wr = group_weights.get(r).copied().unwrap_or(0.0);
            wr.total_cmp(&wl)
        });
        *graph.request_groups_mut() = ordered;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::exchange::graph::ExchangeNode;
    use crate::limits::almost_eq;

    fn weights(pairs: &[(&str, f64)]) -> BTreeMap<String, f64> {
        pairs
            .iter()
            .map(|(k, v)| (String::from(*k), *v))
            .collect()
    }

    #[test]
    fn avg_pref_is_zero_without_arcs_and_the_mean_with_them() {
        let mut g = ExchangeGraph::new();
        let rg = g.add_request_group(3.0);
        let sg = g.add_supply_group();
        let u = g.add_node(rg, ExchangeNode::new(3.0)).unwrap();
        assert_eq!(avg_pref(g.node(u).unwrap()), 0.0);

        let v1 = g.add_node(sg, ExchangeNode::new(3.0)).unwrap();
        let v2 = g.add_node(sg, ExchangeNode::new(3.0)).unwrap();
        let a1 = g.add_arc(u, v1).unwrap();
        let a2 = g.add_arc(u, v2).unwrap();
        g.node_mut(u).unwrap().prefs.insert(a1, 1.0);
        g.node_mut(u).unwrap().prefs.insert(a2, 3.0);
        assert_eq!(avg_pref(g.node(u).unwrap()), 2.0);
    }

    #[test]
    fn the_preference_factor_is_bounded_so_commodity_weight_always_dominates() {
        let n = ExchangeNode::new(1.0).commodity("spam");
        let w = weights(&[("spam", 1.0)]);
        // 1 + p/(1+p) is 1 at p = 0 and tends to 2 as p grows without bound.
        assert_eq!(node_weight(&n, Some(&w), 0.0), 1.0);
        assert!(almost_eq(node_weight(&n, Some(&w), 1.0), 1.5));
        assert!(node_weight(&n, Some(&w), 1.0e12) < 2.0);
        // So a commodity weighted 2 always beats one weighted 1, whatever the
        // preferences are.
        let heavy = ExchangeNode::new(1.0).commodity("eggs");
        let w2 = weights(&[("spam", 1.0), ("eggs", 2.0)]);
        assert!(node_weight(&heavy, Some(&w2), 0.0) > node_weight(&n, Some(&w2), 1.0e12));
    }

    #[test]
    fn no_weight_map_means_every_commodity_counts_one() {
        let n = ExchangeNode::new(1.0).commodity("anything");
        assert_eq!(node_weight(&n, None, 0.0), 1.0);
        assert_eq!(node_weight(&n, Some(&BTreeMap::new()), 0.0), 1.0);
    }

    #[test]
    fn a_commodity_missing_from_a_non_empty_map_scores_zero_as_the_code_does() {
        // Upstream's doc comment promises a KeyError; its std::map operator[]
        // delivers a silent 0.0. The code is what is translated.
        let n = ExchangeNode::new(1.0).commodity("ham");
        let w = weights(&[("spam", 5.0)]);
        assert_eq!(node_weight(&n, Some(&w), 1.0), 0.0);
    }

    #[test]
    fn nodes_and_groups_are_ordered_by_commodity_weight_as_upstream_documents() {
        // Upstream's worked example, verbatim:
        //   weights = {spam: 5, eggs: 2}
        //   g1 = {eggs, spam, eggs},  g2 = {eggs, spam}
        // expected: g1 -> {spam, eggs, eggs}, g2 -> {spam, eggs}, and the
        // groups ordered {g2, g1} because g2's mean weight is higher.
        let mut g = ExchangeGraph::new();
        let g1 = g.add_request_group(3.0);
        let e1 = g.add_node(g1, ExchangeNode::new(1.0).commodity("eggs")).unwrap();
        let s1 = g.add_node(g1, ExchangeNode::new(1.0).commodity("spam")).unwrap();
        let e2 = g.add_node(g1, ExchangeNode::new(1.0).commodity("eggs")).unwrap();
        let g2 = g.add_request_group(2.0);
        let e3 = g.add_node(g2, ExchangeNode::new(1.0).commodity("eggs")).unwrap();
        let s2 = g.add_node(g2, ExchangeNode::new(1.0).commodity("spam")).unwrap();

        let pc = GreedyPreconditioner::with_weights(weights(&[("spam", 5.0), ("eggs", 2.0)]));
        pc.condition(&mut g);

        // Heaviest commodity first within each group; the two "eggs" nodes of
        // g1 keep their creation order, which is what stability buys.
        assert_eq!(g.group(g1).unwrap().nodes(), &[s1, e1, e2]);
        assert_eq!(g.group(g2).unwrap().nodes(), &[s2, e3]);
        // g2 mean = (5+2)/2 = 3.5; g1 mean = (5+2+2)/3 = 3.
        assert_eq!(g.request_groups(), &[g2, g1]);
    }

    #[test]
    fn reversed_weights_are_reflected_about_their_own_range() {
        // {spam: 5, eggs: 2} given lightest-first becomes {spam: 2, eggs: 5}.
        let pc = GreedyPreconditioner::with_weights_ordered(
            weights(&[("spam", 5.0), ("eggs", 2.0)]),
            WgtOrder::Reverse,
        );
        assert_eq!(pc.commod_weights()["spam"], 2.0);
        assert_eq!(pc.commod_weights()["eggs"], 5.0);
    }

    #[test]
    fn with_no_weights_ordering_falls_back_to_average_preference() {
        let mut g = ExchangeGraph::new();
        let rg = g.add_request_group(2.0);
        let sg = g.add_supply_group();
        let dull = g.add_node(rg, ExchangeNode::new(1.0)).unwrap();
        let keen = g.add_node(rg, ExchangeNode::new(1.0)).unwrap();
        let v = g.add_node(sg, ExchangeNode::new(2.0)).unwrap();
        let a1 = g.add_arc(dull, v).unwrap();
        let a2 = g.add_arc(keen, v).unwrap();
        g.node_mut(dull).unwrap().prefs.insert(a1, 0.5);
        g.node_mut(keen).unwrap().prefs.insert(a2, 4.0);

        GreedyPreconditioner::new().condition(&mut g);
        assert_eq!(g.group(rg).unwrap().nodes(), &[keen, dull]);
    }
}
