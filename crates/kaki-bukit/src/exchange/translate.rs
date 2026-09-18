// PROVENANCE
//   Upstream project: CYCLUS <https://github.com/cyclus/cyclus>
//   Upstream file:    src/exchange_translator.h, src/exchange_context.h,
//                     src/exchange_translation_context.h
//   Upstream commit:  d4faab7ce0566ccb8febfcf50915cdeedee59db0
//   Upstream licence: BSD-3-Clause
//   Copyright (c) 2010-2016, University of Wisconsin Computational Nuclear
//   Engineering Research Group. All rights reserved.
//
//   Independent Rust translation, (C) 2026 Theodore Ong and the outram-park
//   contributors, GPL-3.0-only. Relicensing is ONE-WAY.

//! Turning portfolios into a graph, and matches back into trades.
//!
//! This is the bridge between the two halves of the module: the
//! resource-aware half ([`Request`], [`Bid`], portfolios, constraints) and the
//! resource-blind half ([`ExchangeGraph`]). Collect portfolios into an
//! [`ExchangeContext`], [`translate`](ExchangeContext::translate) it, solve the
//! resulting graph, then
//! [`back_translate`](Translation::back_translate) the matches into
//! [`Trade`]s.
//!
//! Upstream splits this across `ExchangeContext` (collecting portfolios and
//! resolving preferences), `ExchangeTranslator` (building the graph) and
//! `ExchangeTranslationContext` (the four node/request/bid maps). All three
//! are small, none is useful without the others, and two of them are templates
//! over the resource type — so they are one non-generic type here plus its
//! output.

use alloc::collections::BTreeMap;
use alloc::vec::Vec;

use crate::error::{CyclusError, Result};
use crate::exchange::bid::{Bid, BidId};
use crate::exchange::constraint::{CapacityConstraint, Converter};
use crate::exchange::graph::{ExchangeGraph, ExchangeNode, NodeId};
use crate::exchange::portfolio::{BidPortfolio, RequestPortfolio};
use crate::exchange::request::{Request, RequestId};
use crate::exchange::trade::Trade;

/// Every portfolio offered in one time step's exchange.
///
/// Upstream `cyclus::ExchangeContext<T>`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ExchangeContext {
    requests: Vec<RequestPortfolio>,
    bids: Vec<BidPortfolio>,
}

impl ExchangeContext {
    /// An empty context.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a request portfolio and returns its index, which is the
    /// `portfolio` field of every [`RequestId`] naming one of its requests.
    pub fn add_request_portfolio(&mut self, portfolio: RequestPortfolio) -> usize {
        self.requests.push(portfolio);
        self.requests.len() - 1
    }

    /// Adds a bid portfolio and returns its index.
    pub fn add_bid_portfolio(&mut self, portfolio: BidPortfolio) -> usize {
        self.bids.push(portfolio);
        self.bids.len() - 1
    }

    /// The request portfolios, in insertion order.
    #[must_use]
    pub fn request_portfolios(&self) -> &[RequestPortfolio] {
        &self.requests
    }

    /// The bid portfolios, in insertion order.
    #[must_use]
    pub fn bid_portfolios(&self) -> &[BidPortfolio] {
        &self.bids
    }

    /// Borrows a request by id.
    ///
    /// # Errors
    ///
    /// [`CyclusError::Key`] if the id names no request in this context.
    pub fn request(&self, id: RequestId) -> Result<&Request> {
        self.requests
            .get(id.portfolio)
            .and_then(|p| p.requests().get(id.index))
            .ok_or(CyclusError::Key("no such request in this exchange"))
    }

    /// Borrows a bid by id.
    ///
    /// # Errors
    ///
    /// [`CyclusError::Key`] if the id names no bid in this context.
    pub fn bid(&self, id: BidId) -> Result<&Bid> {
        self.bids
            .get(id.portfolio)
            .and_then(|p| p.bids().get(id.index))
            .ok_or(CyclusError::Key("no such bid in this exchange"))
    }

    /// The preference of the arc a bid would create.
    ///
    /// Upstream `ExchangeContext::AddBid`: the bid's own preference if it set
    /// one, otherwise the request's. Upstream distinguishes the two with
    /// `std::isnan` on a `double`; [`Bid::preference`] is an `Option<f64>`, so
    /// the sentinel is gone.
    ///
    /// # Errors
    ///
    /// [`CyclusError::Key`] if the bid names no request in this context.
    pub fn arc_preference(&self, bid: &Bid) -> Result<f64> {
        match bid.preference() {
            Some(p) => Ok(p),
            None => Ok(self.request(bid.request())?.preference()),
        }
    }

    /// Builds the exchange graph.
    ///
    /// Upstream `ExchangeTranslator::Translate`, in the same order: every
    /// request portfolio becomes a request group, then every bid portfolio
    /// becomes a supply group and each of its bids becomes an arc.
    ///
    /// Each portfolio's constraints become its group's capacities in insertion
    /// order, and each arc pushes one unit capacity per constraint onto both
    /// of its endpoints, computed as `converter(offer) / offer.quantity()`.
    /// Note that the *request*-side constraints are converted against the
    /// **bid's** offered resource, not the requested one — that is upstream's
    /// behaviour, and it is what lets a requester constrain on a property of
    /// what it is actually being sent.
    ///
    /// Every request portfolio additionally gets the automatic mass
    /// constraint, appended last; see
    /// [`RequestPortfolio::qty_constraint`].
    ///
    /// # Errors
    ///
    /// [`CyclusError::Key`] if a bid names a request that is not in this
    /// context, or [`CyclusError::Value`] if a request portfolio demands a
    /// non-positive total quantity. A **negative** arc preference drops that
    /// arc silently, and a **zero** one is an error — both are upstream's
    /// behaviour, though by construction neither can arise from a
    /// [`Request`] or [`Bid`] built through this crate's constructors, which
    /// reject non-positive preferences outright.
    pub fn translate(&self) -> Result<Translation> {
        let mut graph = ExchangeGraph::new();
        let mut node_to_request: BTreeMap<NodeId, RequestId> = BTreeMap::new();
        let mut node_to_bid: BTreeMap<NodeId, BidId> = BTreeMap::new();
        let mut request_to_node: BTreeMap<RequestId, NodeId> = BTreeMap::new();

        // --- request groups ---
        let mut req_constraints: Vec<Vec<CapacityConstraint>> = Vec::new();
        for (p, rp) in self.requests.iter().enumerate() {
            let g = graph.add_request_group(rp.qty());
            for (i, r) in rp.requests().iter().enumerate() {
                let mut node = ExchangeNode::new(r.quantity())
                    .exclusive(r.exclusive())
                    .commodity(r.commodity());
                node = node.agent(r.requester());
                let n = graph.add_node(g, node)?;
                let id = RequestId {
                    portfolio: p,
                    index: i,
                };
                node_to_request.insert(n, id);
                request_to_node.insert(id, n);
            }

            // Upstream mutates the portfolio to append this; we keep it in a
            // local working list so translating twice gives the same graph.
            let mut cs: Vec<CapacityConstraint> = rp.constraints().to_vec();
            cs.push(rp.qty_constraint()?);
            for c in &cs {
                graph.add_capacity(g, c.capacity())?;
            }
            req_constraints.push(cs);
        }

        // --- supply groups and arcs ---
        for (q, bp) in self.bids.iter().enumerate() {
            let g = graph.add_supply_group();
            let mut bid_nodes: Vec<NodeId> = Vec::with_capacity(bp.bids().len());
            let mut shared: BTreeMap<u32, Vec<NodeId>> = BTreeMap::new();

            for (j, b) in bp.bids().iter().enumerate() {
                let req = self.request(b.request())?;
                let mut node = ExchangeNode::new(b.quantity())
                    .exclusive(b.exclusive())
                    .commodity(req.commodity());
                node = node.agent(b.bidder());
                let n = graph.add_node(g, node)?;
                bid_nodes.push(n);
                node_to_bid.insert(
                    n,
                    BidId {
                        portfolio: q,
                        index: j,
                    },
                );
                if b.exclusive() {
                    if let Some(tag) = b.shared_offer() {
                        shared.entry(tag).or_default().push(n);
                    }
                }
            }
            for (_, nodes) in shared {
                graph.add_excl_group(g, nodes)?;
            }
            for c in bp.constraints() {
                graph.add_capacity(g, c.capacity())?;
            }

            for (j, b) in bp.bids().iter().enumerate() {
                let pref = self.arc_preference(b)?;
                if pref < 0.0 {
                    continue; // upstream drops the arc entirely
                }
                if pref == 0.0 {
                    return Err(CyclusError::Value(
                        "0-valued preferences are not allowed; use a positive preference",
                    ));
                }
                let req_id = b.request();
                let unode = *request_to_node
                    .get(&req_id)
                    .ok_or(CyclusError::Key("no such request in this exchange"))?;
                let vnode = bid_nodes[j];

                let a = graph.add_arc(unode, vnode)?;
                graph.set_arc_pref(a, pref)?;
                graph.node_mut(unode)?.prefs.insert(a, pref);

                let offer_qty = b.quantity();

                // Bid-side constraints, converted against the offer.
                let mut v_caps = Vec::with_capacity(bp.constraints().len());
                for c in bp.constraints() {
                    v_caps.push(c.convert(b.offer(), 1.0) / offer_qty);
                }
                if !v_caps.is_empty() {
                    graph.node_mut(vnode)?.unit_capacities.insert(a, v_caps);
                }

                // Request-side constraints, also converted against the offer.
                let coeff = self.requests[req_id.portfolio].mass_coeff(req_id.index);
                let cs = &req_constraints[req_id.portfolio];
                let mut u_caps = Vec::with_capacity(cs.len());
                for c in cs {
                    let m = if c.converter() == Converter::RequestMassCoeff {
                        coeff
                    } else {
                        1.0
                    };
                    u_caps.push(c.convert(b.offer(), m) / offer_qty);
                }
                if !u_caps.is_empty() {
                    graph.node_mut(unode)?.unit_capacities.insert(a, u_caps);
                }
            }
        }

        Ok(Translation {
            graph,
            node_to_request,
            node_to_bid,
        })
    }
}

/// A translated exchange: the graph, plus the maps needed to read its solution
/// back as trades.
///
/// Upstream `ExchangeTranslationContext<T>` together with the graph the
/// translator produced.
#[derive(Debug, Clone, PartialEq)]
pub struct Translation {
    /// The graph to solve. Public because solving mutates it in place, and
    /// the caller chooses the solver.
    pub graph: ExchangeGraph,
    node_to_request: BTreeMap<NodeId, RequestId>,
    node_to_bid: BTreeMap<NodeId, BidId>,
}

impl Translation {
    /// The request a node was translated from, if it is a request node.
    #[must_use]
    pub fn request_of(&self, node: NodeId) -> Option<RequestId> {
        self.node_to_request.get(&node).copied()
    }

    /// The bid a node was translated from, if it is a bid node.
    #[must_use]
    pub fn bid_of(&self, node: NodeId) -> Option<BidId> {
        self.node_to_bid.get(&node).copied()
    }

    /// Converts the solved graph's matches into trades.
    ///
    /// Upstream `ExchangeTranslator::BackTranslateSolution` over
    /// `BackTranslateMatch`.
    ///
    /// # Errors
    ///
    /// [`CyclusError::Key`] if a match names an arc or node this translation
    /// does not know — which can only happen if the graph was edited after
    /// translation.
    pub fn back_translate(&self) -> Result<Vec<Trade>> {
        let mut out = Vec::with_capacity(self.graph.matches().len());
        for m in self.graph.matches() {
            let arc = self.graph.arc(m.arc)?;
            let request = self
                .request_of(arc.unode())
                .ok_or(CyclusError::Key("matched arc has no translated request"))?;
            let bid = self
                .bid_of(arc.vnode())
                .ok_or(CyclusError::Key("matched arc has no translated bid"))?;
            out.push(Trade::new(request, bid, m.qty));
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::AgentId;
    use crate::exchange::greedy::GreedySolver;
    use crate::exchange::request::DEFAULT_PREF;
    use crate::limits::almost_eq;
    use crate::product::Product;
    use crate::resource::Resource;

    fn product(qty: f64) -> Resource {
        Resource::from(Product::new(qty, "MWh").unwrap())
    }

    /// One requester wanting `demand`, one bidder offering `offer`, with an
    /// optional throughput constraint on the bidder.
    fn simple_context(demand: f64, offer: f64, throughput: Option<f64>) -> ExchangeContext {
        let mut ctx = ExchangeContext::new();
        let mut rp = RequestPortfolio::new();
        rp.request(product(demand), AgentId(1), "power", DEFAULT_PREF, false)
            .unwrap();
        let p = ctx.add_request_portfolio(rp);
        assert_eq!(p, 0);

        let mut bp = BidPortfolio::new();
        bp.bid(
            RequestId {
                portfolio: 0,
                index: 0,
            },
            product(offer),
            AgentId(2),
            false,
        )
        .unwrap();
        if let Some(t) = throughput {
            bp.add_constraint(CapacityConstraint::new(t).unwrap());
        }
        ctx.add_bid_portfolio(bp);
        ctx
    }

    #[test]
    fn a_translated_exchange_solves_and_back_translates_to_one_trade() {
        let ctx = simple_context(10.0, 10.0, None);
        let mut t = ctx.translate().unwrap();

        // One request group, one supply group, one arc.
        assert_eq!(t.graph.request_groups().len(), 1);
        assert_eq!(t.graph.supply_groups().len(), 1);
        assert_eq!(t.graph.arcs().len(), 1);

        GreedySolver::new().solve(&mut t.graph).unwrap();
        let trades = t.back_translate().unwrap();
        assert_eq!(trades.len(), 1);
        assert_eq!(trades[0].amt, 10.0);
        assert_eq!(trades[0].request.index, 0);
        assert_eq!(trades[0].bid.index, 0);
        assert_eq!(ctx.request(trades[0].request).unwrap().requester(), AgentId(1));
        assert_eq!(ctx.bid(trades[0].bid).unwrap().bidder(), AgentId(2));
    }

    #[test]
    fn the_automatic_mass_constraint_caps_a_request_group_at_its_own_demand() {
        // The bidder offers far more than is wanted; the request portfolio's
        // own qty constraint is what stops the trade at 10.
        let ctx = simple_context(10.0, 40.0, None);
        let mut t = ctx.translate().unwrap();
        // One capacity on the request group: the automatic mass constraint.
        let rg = t.graph.request_groups()[0];
        assert_eq!(t.graph.group(rg).unwrap().capacities(), &[10.0]);

        GreedySolver::new().solve(&mut t.graph).unwrap();
        let trades = t.back_translate().unwrap();
        assert_eq!(trades.len(), 1);
        assert_eq!(trades[0].amt, 10.0);
    }

    #[test]
    fn a_bidders_throughput_constraint_binds_through_the_translated_unit_capacity() {
        let ctx = simple_context(10.0, 10.0, Some(4.0));
        let mut t = ctx.translate().unwrap();

        let sg = t.graph.supply_groups()[0];
        assert_eq!(t.graph.group(sg).unwrap().capacities(), &[4.0]);
        // converter(offer)/offer.quantity() == 1 for the trivial converter.
        let vnode = t.graph.group(sg).unwrap().nodes()[0];
        let a = t.graph.arcs_for_node(vnode)[0];
        assert_eq!(t.graph.node(vnode).unwrap().unit_capacities[&a], alloc::vec![1.0]);

        let mut s = GreedySolver::new();
        s.solve(&mut t.graph).unwrap();
        assert_eq!(t.back_translate().unwrap()[0].amt, 4.0);
        assert_eq!(s.unmatched(), 6.0);
    }

    #[test]
    fn a_bid_preference_overrides_the_requesters_and_reorders_the_match() {
        let mut ctx = ExchangeContext::new();
        let mut rp = RequestPortfolio::new();
        rp.request(product(5.0), AgentId(1), "power", 1.0, false).unwrap();
        ctx.add_request_portfolio(rp);
        let rid = RequestId {
            portfolio: 0,
            index: 0,
        };

        // Bidder 2 is generous but ordinary; bidder 3 carries a keener
        // preference the requester's cost function granted it.
        let mut dull = BidPortfolio::new();
        dull.add_bid(Bid::new(rid, product(5.0), AgentId(2), false).unwrap())
            .unwrap();
        ctx.add_bid_portfolio(dull);
        let mut keen = BidPortfolio::new();
        keen.add_bid(
            Bid::new(rid, product(5.0), AgentId(3), false)
                .unwrap()
                .with_preference(9.0)
                .unwrap(),
        )
        .unwrap();
        let keen_idx = ctx.add_bid_portfolio(keen);

        let mut t = ctx.translate().unwrap();
        GreedySolver::new().solve(&mut t.graph).unwrap();
        let trades = t.back_translate().unwrap();
        assert_eq!(trades.len(), 1);
        assert_eq!(trades[0].bid.portfolio, keen_idx);
        assert_eq!(trades[0].amt, 5.0);
    }

    #[test]
    fn mutual_requests_are_satisfied_by_a_full_order_of_either() {
        // 10 kg of MOX or 9 kg of UOX meet one demand of 9.5.
        let mut ctx = ExchangeContext::new();
        let mut rp = RequestPortfolio::new();
        let mox = rp
            .request(product(10.0), AgentId(1), "mox", DEFAULT_PREF, false)
            .unwrap();
        let uox = rp
            .request(product(9.0), AgentId(1), "uox", DEFAULT_PREF, false)
            .unwrap();
        rp.add_mutual_reqs(&[mox, uox]).unwrap();
        assert_eq!(rp.qty(), 9.5);
        ctx.add_request_portfolio(rp);

        // Only the UOX supplier bids, offering the full 9 kg.
        let mut bp = BidPortfolio::new();
        bp.bid(
            RequestId {
                portfolio: 0,
                index: uox,
            },
            product(9.0),
            AgentId(2),
            false,
        )
        .unwrap();
        ctx.add_bid_portfolio(bp);

        let mut t = ctx.translate().unwrap();
        let mut s = GreedySolver::new();
        s.solve(&mut t.graph).unwrap();
        let trades = t.back_translate().unwrap();
        assert_eq!(trades.len(), 1);
        // 9 kg of UOX is the whole 9.5-unit demand, because its mass
        // coefficient is 9.5/9.
        assert_eq!(trades[0].amt, 9.0);
        assert!(almost_eq(s.unmatched(), 0.5));
    }

    #[test]
    fn exclusive_bids_sharing_one_offer_form_one_exclusive_group() {
        let mut ctx = ExchangeContext::new();
        let mut rp = RequestPortfolio::new();
        rp.request(product(4.0), AgentId(1), "fuel", DEFAULT_PREF, false)
            .unwrap();
        rp.request(product(4.0), AgentId(1), "fuel", DEFAULT_PREF, false)
            .unwrap();
        ctx.add_request_portfolio(rp);

        let mut bp = BidPortfolio::new();
        for i in 0..2 {
            bp.add_bid(
                Bid::new(
                    RequestId {
                        portfolio: 0,
                        index: i,
                    },
                    product(4.0),
                    AgentId(2),
                    true,
                )
                .unwrap()
                .with_shared_offer(7),
            )
            .unwrap();
        }
        ctx.add_bid_portfolio(bp);

        let t = ctx.translate().unwrap();
        let sg = t.graph.supply_groups()[0];
        assert_eq!(t.graph.group(sg).unwrap().excl_node_groups().len(), 1);
        assert_eq!(t.graph.group(sg).unwrap().excl_node_groups()[0].len(), 2);
    }

    #[test]
    fn a_bid_naming_an_unknown_request_is_a_key_error() {
        let mut ctx = ExchangeContext::new();
        let mut bp = BidPortfolio::new();
        bp.bid(
            RequestId {
                portfolio: 3,
                index: 0,
            },
            product(1.0),
            AgentId(2),
            false,
        )
        .unwrap();
        ctx.add_bid_portfolio(bp);
        assert_eq!(
            ctx.translate().unwrap_err(),
            CyclusError::Key("no such request in this exchange")
        );
    }

    #[test]
    fn translating_twice_gives_the_same_graph() {
        // Upstream mutates the portfolio with a fresh mass constraint on every
        // Translate(), so its second graph has one capacity more than its
        // first. This one is idempotent.
        let ctx = simple_context(10.0, 10.0, Some(4.0));
        let a = ctx.translate().unwrap();
        let b = ctx.translate().unwrap();
        assert_eq!(a.graph, b.graph);
    }
}
