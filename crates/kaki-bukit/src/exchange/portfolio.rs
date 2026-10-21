// PROVENANCE
//   Upstream project: CYCLUS <https://github.com/cyclus/cyclus>
//   Upstream file:    src/request_portfolio.h, src/bid_portfolio.h
//   Upstream commit:  d4faab7ce0566ccb8febfcf50915cdeedee59db0
//   Upstream licence: BSD-3-Clause
//   Copyright (c) 2010-2016, University of Wisconsin Computational Nuclear
//   Engineering Research Group. All rights reserved.
//
//   Independent Rust translation, (C) 2026 Theodore Ong and the outram-park
//   contributors, GPL-3.0-only. Relicensing is ONE-WAY.

//! Portfolios: one agent's requests, or one agent's bids, with the
//! constraints that act across all of them.
//!
//! A portfolio is the unit of *mutual* satisfaction. Putting two requests in
//! one portfolio and declaring them mutual says "either of these will do";
//! putting two bids in one portfolio says "these share my throughput". The
//! constraints hang off the portfolio, not the individual request or bid,
//! which is why one portfolio becomes one
//! [`ExchangeNodeGroup`](crate::exchange::graph::ExchangeNodeGroup) in the
//! graph and its constraints become that group's capacities.
//!
//! # Ownership without `shared_ptr`
//!
//! Upstream portfolios hold raw `Request<T>*`/`Bid<T>*` and delete them in the
//! destructor, while the portfolio itself is a `shared_ptr` with
//! `enable_shared_from_this` so each request can hold a `weak_ptr` back to it.
//! Here the portfolio owns its requests by value in a `Vec`, and the back
//! reference is the index — a [`RequestId`] or [`BidId`](crate::exchange::bid::BidId) naming (portfolio,
//! index). Nothing is shared, nothing is freed twice, and the cycle that
//! forced the `weak_ptr` never forms.

use alloc::vec::Vec;

use crate::agent::AgentId;
use crate::error::{CyclusError, Result};
use crate::exchange::bid::Bid;
use crate::exchange::constraint::{CapacityConstraint, Converter};
use crate::exchange::request::{Request, RequestId};
use crate::resource::Resource;

/// One requester's demands, and the constraints across them.
///
/// Upstream `cyclus::RequestPortfolio<T>`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct RequestPortfolio {
    requests: Vec<Request>,
    mass_coeffs: Vec<f64>,
    constraints: Vec<CapacityConstraint>,
    qty: f64,
    requester: Option<AgentId>,
}

impl RequestPortfolio {
    /// An empty portfolio, with no requester yet.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a request and returns its index within this portfolio.
    ///
    /// The portfolio's total [`qty`](Self::qty) grows by the request's
    /// quantity, and the request's mass coefficient starts at `1.0`.
    ///
    /// # Errors
    ///
    /// [`CyclusError::Key`] if the request's requester differs from the one
    /// that opened the portfolio. Upstream's `VerifyRequester_`, with the same
    /// message: a portfolio speaks for exactly one agent.
    pub fn add_request(&mut self, request: Request) -> Result<usize> {
        match self.requester {
            None => self.requester = Some(request.requester()),
            Some(id) if id != request.requester() => {
                return Err(CyclusError::Key("insertion error: requesters do not match"));
            }
            Some(_) => {}
        }
        self.qty += request.quantity();
        self.requests.push(request);
        self.mass_coeffs.push(1.0);
        Ok(self.requests.len() - 1)
    }

    /// Convenience: builds a request from its parts and adds it.
    ///
    /// # Errors
    ///
    /// Whatever [`Request::new`] rejects, plus
    /// [`add_request`](Self::add_request)'s requester check.
    pub fn request(
        &mut self,
        target: Resource,
        requester: AgentId,
        commodity: &str,
        preference: f64,
        exclusive: bool,
    ) -> Result<usize> {
        let r = Request::new(target, requester, commodity, preference, exclusive)?;
        self.add_request(r)
    }

    /// Declares the named requests mutually satisfying: any one of them fills
    /// the same demand.
    ///
    /// Upstream `AddMutualReqs`. The portfolio's total quantity becomes the
    /// *average* of their quantities rather than the sum, and each request
    /// gets a mass coefficient of `its quantity / that average`, which the
    /// automatic mass constraint then uses as its unit capacity.
    ///
    /// # Upstream's code and upstream's comment disagree, and the code wins
    ///
    /// `request_portfolio.h`'s class comment says, of 10 kg of MOX and 9 kg of
    /// UOX meeting one demand: "the total demand is 9.5, the MOX order is
    /// given a coefficient of 9.5 / 10, and the UOX order is given a
    /// coefficient of 9.5 / 9". The implementation two screens below computes
    /// the **reciprocal** — `mass_coeffs_[r] = r->target()->quantity() /
    /// avg_qty`, i.e. `10/9.5` and `9/9.5`.
    ///
    /// The difference is observable. The constraint's capacity is the average,
    /// 9.5, and the flow it permits is `capacity / coefficient`; with the
    /// comment's coefficient that is exactly 10 kg of MOX (a full order, as
    /// the comment promises), and with the code's it is 9.025 kg (a full order
    /// minus 10 %). No upstream test covers either — `request_portfolio_tests.cc`
    /// does not exercise `AddMutualReqs` at all.
    ///
    /// **The code is translated, not the comment**, because it is what every
    /// published Cyclus result was produced with, and quietly "correcting" a
    /// mass balance to match a doc comment is not a translation. Anyone
    /// relying on mutual requests should know the mechanism under-fills
    /// relative to its stated design.
    ///
    /// # Errors
    ///
    /// [`CyclusError::Key`] if any index is not a request of this portfolio,
    /// or [`CyclusError::Value`] if the list is empty (upstream divides by the
    /// list's size without checking, producing a NaN quantity).
    pub fn add_mutual_reqs(&mut self, indices: &[usize]) -> Result<()> {
        if indices.is_empty() {
            return Err(CyclusError::Value(
                "a set of mutual requests must not be empty",
            ));
        }
        let mut sum = 0.0;
        for &i in indices {
            let r = self
                .requests
                .get(i)
                .ok_or(CyclusError::Key("no such request in this portfolio"))?;
            sum += r.quantity();
        }
        #[allow(clippy::cast_precision_loss)]
        let avg_qty = sum / indices.len() as f64;
        for &i in indices {
            let qty = self.requests[i].quantity();
            self.mass_coeffs[i] = qty / avg_qty;
            self.qty -= qty;
        }
        self.qty += avg_qty;
        Ok(())
    }

    /// Adds a constraint acting across every request in the portfolio.
    ///
    /// Constraints are positional: the order they are added here is the order
    /// the capacities appear on the translated group, and therefore the order
    /// of each node's unit capacities.
    ///
    /// Upstream keeps them in a `std::set` ordered by a global monotonic id,
    /// which is insertion order in all but pathological cases; a `Vec` says
    /// the same thing without the id counter. The one behavioural difference
    /// is that a duplicate constraint is stored twice here and once upstream.
    pub fn add_constraint(&mut self, constraint: CapacityConstraint) {
        self.constraints.push(constraint);
    }

    /// The requesting agent, or `None` if no request has been added yet.
    ///
    /// The `Option` here is the portfolio's own "not yet determined" state —
    /// upstream's `Trader* requester_ == NULL` — and not the unset-agent
    /// sentinel that [`ExchangeNode::agent_id`](crate::exchange::graph::ExchangeNode::agent_id)
    /// carries. A [`Request`] itself always names its requester.
    #[must_use]
    pub fn requester(&self) -> Option<AgentId> {
        self.requester
    }

    /// The total quantity this portfolio demands, in the resource's units.
    ///
    /// The sum of the member requests' quantities, except that a set declared
    /// mutual by [`add_mutual_reqs`](Self::add_mutual_reqs) contributes its
    /// average instead of its sum.
    #[must_use]
    pub fn qty(&self) -> f64 {
        self.qty
    }

    /// The requests, in insertion order.
    #[must_use]
    pub fn requests(&self) -> &[Request] {
        &self.requests
    }

    /// The constraints, in insertion order.
    #[must_use]
    pub fn constraints(&self) -> &[CapacityConstraint] {
        &self.constraints
    }

    /// The mass coefficient of request `index` — `1.0` unless it was named in
    /// a mutual-request set. Returns `1.0` for an unknown index, which is the
    /// neutral value.
    #[must_use]
    pub fn mass_coeff(&self, index: usize) -> f64 {
        self.mass_coeffs.get(index).copied().unwrap_or(1.0)
    }

    /// The automatic mass constraint every request portfolio gets during
    /// translation: a capacity equal to [`qty`](Self::qty), consumed through
    /// [`Converter::RequestMassCoeff`].
    ///
    /// Upstream builds this in `ExchangeTranslator::Translate` from
    /// `qty_converter()` and *mutates the portfolio* to add it. Translation
    /// here takes the portfolio by shared reference and appends this
    /// constraint to its own working list instead, which leaves the
    /// portfolio unchanged and therefore makes translating the same context
    /// twice give the same graph — upstream's version accumulates a duplicate
    /// mass constraint on every re-translation.
    ///
    /// # Errors
    ///
    /// [`CyclusError::Value`] if the portfolio's total quantity is not
    /// strictly positive, i.e. it demands nothing.
    pub fn qty_constraint(&self) -> Result<CapacityConstraint> {
        CapacityConstraint::with_converter(self.qty, Converter::RequestMassCoeff)
    }
}

/// One bidder's offers, and the constraints across them.
///
/// Upstream `cyclus::BidPortfolio<T>`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct BidPortfolio {
    bids: Vec<Bid>,
    constraints: Vec<CapacityConstraint>,
    bidder: Option<AgentId>,
}

impl BidPortfolio {
    /// An empty portfolio, with no bidder yet.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a bid and returns its index within this portfolio.
    ///
    /// # Errors
    ///
    /// [`CyclusError::Key`] if the bid's bidder differs from the one that
    /// opened the portfolio. Upstream's `VerifyResponder_`. The
    /// positive-quantity check upstream also makes here has moved to
    /// [`Bid::new`].
    pub fn add_bid(&mut self, bid: Bid) -> Result<usize> {
        match self.bidder {
            None => self.bidder = Some(bid.bidder()),
            Some(id) if id != bid.bidder() => {
                return Err(CyclusError::Key("insertion error: bidders do not match"));
            }
            Some(_) => {}
        }
        self.bids.push(bid);
        Ok(self.bids.len() - 1)
    }

    /// Convenience: builds a bid from its parts and adds it.
    ///
    /// # Errors
    ///
    /// Whatever [`Bid::new`] rejects, plus [`add_bid`](Self::add_bid)'s bidder
    /// check.
    pub fn bid(
        &mut self,
        request: RequestId,
        offer: Resource,
        bidder: AgentId,
        exclusive: bool,
    ) -> Result<usize> {
        let b = Bid::new(request, offer, bidder, exclusive)?;
        self.add_bid(b)
    }

    /// Adds a constraint acting across every bid in the portfolio — a
    /// throughput limit, an inventory, an enrichment plant's monthly SWU.
    ///
    /// Positional, exactly as for
    /// [`RequestPortfolio::add_constraint`].
    pub fn add_constraint(&mut self, constraint: CapacityConstraint) {
        self.constraints.push(constraint);
    }

    /// The bidding agent, or `None` if no bid has been added yet. Same
    /// "not yet determined" sense as [`RequestPortfolio::requester`].
    #[must_use]
    pub fn bidder(&self) -> Option<AgentId> {
        self.bidder
    }

    /// The bids, in insertion order.
    ///
    /// Upstream holds these in a `std::set<Bid<T>*>`, which orders by *pointer
    /// value* — so the order in which bids are translated into nodes, and
    /// hence node ids, is allocator-dependent upstream and deterministic here.
    #[must_use]
    pub fn bids(&self) -> &[Bid] {
        &self.bids
    }

    /// The constraints, in insertion order.
    #[must_use]
    pub fn constraints(&self) -> &[CapacityConstraint] {
        &self.constraints
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::exchange::request::DEFAULT_PREF;
    use crate::limits::almost_eq;
    use crate::product::Product;

    fn product(qty: f64) -> Resource {
        Resource::from(Product::new(qty, "MWh").unwrap())
    }

    fn req_id() -> RequestId {
        RequestId {
            portfolio: 0,
            index: 0,
        }
    }

    #[test]
    fn a_request_portfolio_sums_quantities_and_adopts_one_requester() {
        let mut p = RequestPortfolio::new();
        p.request(product(4.0), AgentId(1), "power", DEFAULT_PREF, false)
            .unwrap();
        p.request(product(6.0), AgentId(1), "power", DEFAULT_PREF, false)
            .unwrap();
        assert_eq!(p.qty(), 10.0);
        assert_eq!(p.requester(), Some(AgentId(1)));
        assert_eq!(p.requests().len(), 2);
    }

    #[test]
    fn a_second_requester_is_rejected() {
        let mut p = RequestPortfolio::new();
        p.request(product(4.0), AgentId(1), "power", DEFAULT_PREF, false)
            .unwrap();
        assert_eq!(
            p.request(product(4.0), AgentId(2), "power", DEFAULT_PREF, false)
                .unwrap_err(),
            CyclusError::Key("insertion error: requesters do not match")
        );
    }

    #[test]
    fn mutual_requests_average_the_demand_and_weight_the_coefficients() {
        // Upstream's own worked example: 10 kg of MOX or 9 kg of UOX.
        let mut p = RequestPortfolio::new();
        let mox = p
            .request(product(10.0), AgentId(1), "mox", DEFAULT_PREF, false)
            .unwrap();
        let uox = p
            .request(product(9.0), AgentId(1), "uox", DEFAULT_PREF, false)
            .unwrap();
        assert_eq!(p.qty(), 19.0);

        p.add_mutual_reqs(&[mox, uox]).unwrap();
        // The demand becomes the average, not the sum.
        assert_eq!(p.qty(), 9.5);
        // The coefficients are `qty / avg`, as upstream's CODE computes them.
        // Its class comment states the reciprocals; see `add_mutual_reqs`.
        assert_eq!(p.mass_coeff(mox), 10.0 / 9.5);
        assert_eq!(p.mass_coeff(uox), 9.0 / 9.5);
        assert!(p.mass_coeff(mox) > p.mass_coeff(uox));

        let c = p.qty_constraint().unwrap();
        assert_eq!(c.capacity(), 9.5);
        // A full order consumes more than the whole capacity -- which is the
        // observable consequence of the code/comment discrepancy, pinned here
        // so that a future change to either is deliberate.
        assert!(c.convert(&product(10.0), p.mass_coeff(mox)) > 9.5);
        let permitted = c.capacity() / p.mass_coeff(mox);
        assert!(almost_eq(permitted, 9.025));
    }

    #[test]
    fn an_empty_or_unknown_mutual_set_is_an_error_rather_than_a_nan() {
        let mut p = RequestPortfolio::new();
        p.request(product(1.0), AgentId(1), "power", DEFAULT_PREF, false)
            .unwrap();
        assert!(p.add_mutual_reqs(&[]).is_err());
        assert!(p.add_mutual_reqs(&[7]).is_err());
    }

    #[test]
    fn a_bid_portfolio_adopts_one_bidder_and_keeps_insertion_order() {
        let mut p = BidPortfolio::new();
        let i = p.bid(req_id(), product(3.0), AgentId(9), false).unwrap();
        let j = p.bid(req_id(), product(4.0), AgentId(9), true).unwrap();
        assert_eq!((i, j), (0, 1));
        assert_eq!(p.bidder(), Some(AgentId(9)));
        assert_eq!(p.bids()[0].quantity(), 3.0);
        assert_eq!(p.bids()[1].quantity(), 4.0);

        assert_eq!(
            p.bid(req_id(), product(1.0), AgentId(10), false).unwrap_err(),
            CyclusError::Key("insertion error: bidders do not match")
        );
    }

    #[test]
    fn constraints_keep_the_order_they_were_added_in() {
        let mut p = BidPortfolio::new();
        p.add_constraint(CapacityConstraint::new(5.0).unwrap());
        p.add_constraint(
            CapacityConstraint::with_converter(2.0, Converter::Scaled(0.5)).unwrap(),
        );
        assert_eq!(p.constraints()[0].capacity(), 5.0);
        assert_eq!(p.constraints()[1].converter(), Converter::Scaled(0.5));
    }
}
