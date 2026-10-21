// PROVENANCE
//   Upstream project: CYCLUS <https://github.com/cyclus/cyclus>
//   Upstream file:    src/trade.h
//   Upstream commit:  d4faab7ce0566ccb8febfcf50915cdeedee59db0
//   Upstream licence: BSD-3-Clause
//   Copyright (c) 2010-2016, University of Wisconsin Computational Nuclear
//   Engineering Research Group. All rights reserved.
//
//   Independent Rust translation, (C) 2026 Theodore Ong and the outram-park
//   contributors, GPL-3.0-only. Relicensing is ONE-WAY.

//! The output of the exchange: who gives how much of what, to whom.

use crate::exchange::bid::BidId;
use crate::exchange::request::RequestId;

/// A decided trade — a request, the bid that fills it, and how much moves.
///
/// Upstream `cyclus::Trade<T>`. The amount may be less than either the request
/// or the bid quantity, because the solver splits partially-filled orders
/// (unless either side is exclusive, in which case it is all or nothing).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Trade {
    /// The request being filled.
    pub request: RequestId,
    /// The bid filling it.
    pub bid: BidId,
    /// How much actually moves, in the resource's units (kg for a material).
    /// Strictly positive.
    pub amt: f64,
    /// Price per unit.
    ///
    /// Upstream carries this field and its own comment says it "is not
    /// currently used"; it is defaulted to `0.0` and kept for fidelity, so
    /// that an economics layer added later does not have to change this type.
    pub price: f64,
}

impl Trade {
    /// A trade of `amt` units at the default price of zero.
    #[must_use]
    pub fn new(request: RequestId, bid: BidId, amt: f64) -> Self {
        Self {
            request,
            bid,
            amt,
            price: 0.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_new_trade_has_a_zero_price() {
        let t = Trade::new(
            RequestId {
                portfolio: 1,
                index: 2,
            },
            BidId {
                portfolio: 3,
                index: 4,
            },
            7.5,
        );
        assert_eq!(t.amt, 7.5);
        assert_eq!(t.price, 0.0);
        assert_eq!(t.request.index, 2);
        assert_eq!(t.bid.portfolio, 3);
    }
}
