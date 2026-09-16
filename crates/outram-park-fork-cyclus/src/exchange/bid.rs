// PROVENANCE
//   Upstream project: CYCLUS <https://github.com/cyclus/cyclus>
//   Upstream file:    src/bid.h
//   Upstream commit:  d4faab7ce0566ccb8febfcf50915cdeedee59db0
//   Upstream licence: BSD-3-Clause
//   Copyright (c) 2010-2016, University of Wisconsin Computational Nuclear
//   Engineering Research Group. All rights reserved.
//
//   Independent Rust translation, (C) 2026 Theodore Ong and the outram-park
//   contributors, GPL-3.0-only. Relicensing is ONE-WAY.

//! A bid: one supplier's answer to one request.
//!
//! Like [`Request`](crate::exchange::request::Request), [`Bid`] is not generic
//! over the resource — it holds a [`Resource`](crate::resource::Resource). The
//! reasoning is written out in the
//! [`request`](crate::exchange::request) module doc.

use crate::error::{CyclusError, Result};
use crate::exchange::request::RequestId;
use crate::resource::Resource;

/// Identifies one bid inside an
/// [`ExchangeContext`](crate::exchange::translate::ExchangeContext), as
/// (portfolio, index). The counterpart of
/// [`RequestId`](crate::exchange::request::RequestId).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BidId {
    /// Index of the owning [`BidPortfolio`](crate::exchange::portfolio::BidPortfolio)
    /// within the exchange context.
    pub portfolio: usize,
    /// Index of the bid within that portfolio.
    pub index: usize,
}

/// An offer of a resource in response to a specific request.
///
/// Upstream `cyclus::Bid<T>`.
#[derive(Debug, Clone, PartialEq)]
pub struct Bid {
    request: RequestId,
    offer: Resource,
    bidder: i32,
    exclusive: bool,
    preference: Option<f64>,
    shared_offer: Option<u32>,
}

impl Bid {
    /// Creates a bid answering `request` with `offer`.
    ///
    /// * `offer` — what is actually being supplied. Its quantity must be
    ///   strictly positive and need not equal the request's; the solver will
    ///   match the smaller of the two (or nothing, if either side is
    ///   exclusive and they disagree).
    /// * `bidder` — the bidding agent's id.
    /// * `exclusive` — if `true` the offer must be taken whole or not at all.
    ///
    /// The bid's preference is left unset, meaning the arc inherits the
    /// *requester's* preference. Use
    /// [`with_preference`](Self::with_preference) to override it.
    ///
    /// # Errors
    ///
    /// [`CyclusError::Value`] if the offered quantity is not strictly
    /// positive — the same check upstream's `BidPortfolio::AddBid` makes,
    /// moved to the constructor so a bad bid cannot exist at all.
    pub fn new(request: RequestId, offer: Resource, bidder: i32, exclusive: bool) -> Result<Self> {
        if !(offer.quantity() > 0.0) {
            return Err(CyclusError::Value(
                "a bid must offer a strictly positive quantity",
            ));
        }
        Ok(Self {
            request,
            offer,
            bidder,
            exclusive,
            preference: None,
            shared_offer: None,
        })
    }

    /// Overrides the arc preference for this bid.
    ///
    /// Upstream stores this as a `double` whose "unset" value is a quiet
    /// `NaN`, and tests it with `std::isnan`. An `Option<f64>` says the same
    /// thing without a sentinel, and makes the unset case impossible to
    /// propagate into arithmetic by accident.
    ///
    /// Upstream's warning applies unchanged: this should only be set by a
    /// bidder acting on the requester's own cost function. A bidder that
    /// simply raises its preference is rigging the exchange in its favour.
    ///
    /// # Errors
    ///
    /// [`CyclusError::Value`] if `preference` is not finite and strictly
    /// positive.
    pub fn with_preference(mut self, preference: f64) -> Result<Self> {
        if !(preference > 0.0) || !preference.is_finite() {
            return Err(CyclusError::Value(
                "a bid preference must be finite and strictly positive",
            ));
        }
        self.preference = Some(preference);
        Ok(self)
    }

    /// Tags this bid as offering the same physical resource as every other bid
    /// carrying the same tag, so that at most one of them can be filled.
    ///
    /// # Why this exists
    ///
    /// Upstream detects this by **pointer identity**: `TranslateBidPortfolio`
    /// keys a map on `b->offer()`, a `shared_ptr<T>`, and every exclusive bid
    /// sharing one pointer becomes one exclusive node group. A
    /// [`Resource`](crate::resource::Resource) here is an owned value, so two
    /// bids offering "the same" resource are indistinguishable from two bids
    /// offering equal but separate resources — an identity that does not exist
    /// cannot be recovered. The bidder therefore states it explicitly.
    ///
    /// The tag is scoped to the bid's own portfolio, which matches upstream:
    /// the map is local to `TranslateBidPortfolio`, so bids in different
    /// portfolios never share an exclusive group.
    #[must_use]
    pub fn with_shared_offer(mut self, tag: u32) -> Self {
        self.shared_offer = Some(tag);
        self
    }

    /// The request being answered.
    #[must_use]
    pub fn request(&self) -> RequestId {
        self.request
    }

    /// The resource offered.
    #[must_use]
    pub fn offer(&self) -> &Resource {
        &self.offer
    }

    /// How much is offered, in the resource's units. Strictly positive.
    #[must_use]
    pub fn quantity(&self) -> f64 {
        self.offer.quantity()
    }

    /// The bidding agent's id.
    #[must_use]
    pub fn bidder(&self) -> i32 {
        self.bidder
    }

    /// Whether the offer must be taken whole or not at all.
    #[must_use]
    pub fn exclusive(&self) -> bool {
        self.exclusive
    }

    /// The bid's own preference, or `None` to inherit the requester's.
    #[must_use]
    pub fn preference(&self) -> Option<f64> {
        self.preference
    }

    /// The shared-offer tag set by [`with_shared_offer`](Self::with_shared_offer).
    #[must_use]
    pub fn shared_offer(&self) -> Option<u32> {
        self.shared_offer
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::product::Product;

    fn product(qty: f64) -> Resource {
        Resource::from(Product::new(qty, "MWh").unwrap())
    }

    fn req() -> RequestId {
        RequestId {
            portfolio: 0,
            index: 0,
        }
    }

    #[test]
    fn a_bid_defaults_to_inheriting_the_requesters_preference() {
        let b = Bid::new(req(), product(5.0), 3, false).unwrap();
        assert_eq!(b.preference(), None);
        assert_eq!(b.quantity(), 5.0);
        assert_eq!(b.bidder(), 3);
        assert_eq!(b.request(), req());
        assert_eq!(b.shared_offer(), None);
    }

    #[test]
    fn a_zero_or_negative_offer_is_rejected() {
        assert_eq!(
            Bid::new(req(), product(0.0), 1, false).unwrap_err(),
            CyclusError::Value("a bid must offer a strictly positive quantity")
        );
    }

    #[test]
    fn an_overridden_preference_must_be_positive_and_finite() {
        let b = Bid::new(req(), product(1.0), 1, false).unwrap();
        assert_eq!(b.clone().with_preference(2.5).unwrap().preference(), Some(2.5));
        assert!(b.clone().with_preference(0.0).is_err());
        assert!(b.with_preference(f64::NAN).is_err());
    }

    #[test]
    fn a_shared_offer_tag_is_recorded() {
        let b = Bid::new(req(), product(1.0), 1, true)
            .unwrap()
            .with_shared_offer(42);
        assert_eq!(b.shared_offer(), Some(42));
        assert!(b.exclusive());
    }
}
