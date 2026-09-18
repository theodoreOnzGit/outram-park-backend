// PROVENANCE
//   Upstream project: CYCLUS <https://github.com/cyclus/cyclus>
//   Upstream file:    src/request.h
//   Upstream commit:  d4faab7ce0566ccb8febfcf50915cdeedee59db0
//   Upstream licence: BSD-3-Clause
//   Copyright (c) 2010-2016, University of Wisconsin Computational Nuclear
//   Engineering Research Group. All rights reserved.
//
//   Independent Rust translation, (C) 2026 Theodore Ong and the outram-park
//   contributors, GPL-3.0-only. Relicensing is ONE-WAY.

//! A request: "I want this much of this commodity, and here is how much I
//! want it."
//!
//! # Not generic over the resource
//!
//! Upstream is `Request<T>`, instantiated as `Request<Material>` and
//! `Request<Product>`, with the target held as a `boost::shared_ptr<T>`. A
//! faithful generic translation would be `Request<T>` holding a `T` — which
//! works, but then every downstream container, every portfolio, every
//! translation map and the whole exchange context becomes generic too, and any
//! attempt to hold requests for both resource kinds in one collection needs a
//! trait object.
//!
//! So [`Request`] is **not** generic: it holds a
//! [`Resource`](crate::resource::Resource), the crate's owned enum with
//! `Material` and `Product` variants. One non-generic type covers both
//! upstream instantiations, a single exchange can carry both kinds at once,
//! and no lifetime or `dyn` appears anywhere. The cost is that a requester
//! wanting a material out of a trade calls
//! [`Resource::into_material`](crate::resource::Resource::into_material) and
//! handles the error case — which upstream gets for free from its template
//! parameter, and which is the one genuine ergonomic loss.

use alloc::string::{String, ToString};

use crate::agent::AgentId;
use crate::error::{CyclusError, Result};
use crate::resource::Resource;

/// The preference a request carries unless it says otherwise.
///
/// Upstream `kDefaultPref`, which was changed from `0` to `1` in Cyclus 1.4
/// because a zero preference makes the solver's objective term `qty / pref`
/// infinite. Preferences may be any strictly positive value; larger means more
/// preferred. Dimensionless.
pub const DEFAULT_PREF: f64 = 1.0;

/// Identifies one request inside an
/// [`ExchangeContext`](crate::exchange::translate::ExchangeContext).
///
/// Upstream passes bare `Request<T>*` pointers around and relies on the owning
/// portfolio to free them. Here a request is owned by its portfolio and
/// referred to by the pair (portfolio, index), so a [`Bid`](crate::exchange::bid::Bid)
/// can name the request it answers without a pointer, a lifetime or an `Rc`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RequestId {
    /// Index of the owning [`RequestPortfolio`](crate::exchange::portfolio::RequestPortfolio)
    /// within the exchange context.
    pub portfolio: usize,
    /// Index of the request within that portfolio.
    pub index: usize,
}

/// One agent's demand for one commodity.
///
/// Upstream `cyclus::Request<T>`.
#[derive(Debug, Clone, PartialEq)]
pub struct Request {
    target: Resource,
    requester: AgentId,
    commodity: String,
    preference: f64,
    exclusive: bool,
}

impl Request {
    /// Creates a request.
    ///
    /// * `target` — the resource wanted. Its
    ///   [`quantity`](crate::resource::Resource::quantity) is how much is
    ///   wanted (kg for a material); for a material its composition is the
    ///   *desired* composition, which bidders may use to decide what to offer.
    /// * `requester` — the requesting agent. Upstream holds a `Trader*`; an
    ///   [`AgentId`] is enough for everything the exchange itself does
    ///   (grouping, tie-breaking, reporting the trade back), and it keeps the
    ///   exchange free of agent pointers and of the raw `int` ids upstream
    ///   uses alongside them.
    /// * `commodity` — the commodity name bids are matched on, and the key the
    ///   [preconditioner](crate::exchange::preconditioner) weights by.
    /// * `preference` — strictly positive, dimensionless; larger is more
    ///   preferred.
    /// * `exclusive` — if `true` the request must be met in full by a single
    ///   bid, or not at all.
    ///
    /// # Errors
    ///
    /// [`CyclusError::Value`] if `preference` is not strictly positive or is
    /// not finite. Upstream does not check here — it checks during
    /// translation, where a negative preference silently drops the arc and a
    /// zero one throws. Checking at construction reports the fault where it
    /// was made.
    pub fn new(
        target: Resource,
        requester: AgentId,
        commodity: &str,
        preference: f64,
        exclusive: bool,
    ) -> Result<Self> {
        if !(preference > 0.0) || !preference.is_finite() {
            return Err(CyclusError::Value(
                "a request preference must be finite and strictly positive",
            ));
        }
        Ok(Self {
            target,
            requester,
            commodity: commodity.to_string(),
            preference,
            exclusive,
        })
    }

    /// A request at [`DEFAULT_PREF`], not exclusive.
    ///
    /// # Errors
    ///
    /// Never, in practice — [`DEFAULT_PREF`] is valid. The signature keeps the
    /// `Result` so that tightening the constructor later is not a breaking
    /// change.
    pub fn simple(target: Resource, requester: AgentId, commodity: &str) -> Result<Self> {
        Self::new(target, requester, commodity, DEFAULT_PREF, false)
    }

    /// The resource wanted.
    #[must_use]
    pub fn target(&self) -> &Resource {
        &self.target
    }

    /// How much is wanted, in the resource's units.
    #[must_use]
    pub fn quantity(&self) -> f64 {
        self.target.quantity()
    }

    /// The requesting agent.
    ///
    /// Not optional: a request without a requester cannot be constructed,
    /// which is why this is an [`AgentId`] and not an `Option<AgentId>` the
    /// way [`ExchangeNode::agent_id`](crate::exchange::graph::ExchangeNode::agent_id)
    /// is. Upstream's unset case exists only on the translated graph node.
    #[must_use]
    pub fn requester(&self) -> AgentId {
        self.requester
    }

    /// The commodity name.
    #[must_use]
    pub fn commodity(&self) -> &str {
        &self.commodity
    }

    /// The preference, strictly positive and dimensionless.
    #[must_use]
    pub fn preference(&self) -> f64 {
        self.preference
    }

    /// Whether this request must be met in full by one bid.
    #[must_use]
    pub fn exclusive(&self) -> bool {
        self.exclusive
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::product::Product;

    fn product(qty: f64) -> Resource {
        Resource::from(Product::new(qty, "MWh").unwrap())
    }

    #[test]
    fn a_simple_request_takes_the_default_preference() {
        let r = Request::simple(product(10.0), AgentId(7), "power").unwrap();
        assert_eq!(r.preference(), DEFAULT_PREF);
        assert_eq!(r.quantity(), 10.0);
        assert_eq!(r.requester(), AgentId(7));
        assert_eq!(r.commodity(), "power");
        assert!(!r.exclusive());
    }

    #[test]
    fn a_non_positive_preference_is_rejected_at_construction() {
        for bad in [0.0, -1.0, f64::NAN, f64::INFINITY] {
            assert_eq!(
                Request::new(product(1.0), AgentId(1), "power", bad, false).unwrap_err(),
                CyclusError::Value(
                    "a request preference must be finite and strictly positive"
                )
            );
        }
    }

    #[test]
    fn a_request_carries_either_resource_kind_without_being_generic() {
        use crate::comp_math::CompMap;
        use crate::composition::{AtomicMasses, Composition};
        use crate::material::Material;
        use crate::nuclide::nuc;

        let map: CompMap = [(nuc::U235, 1.0)].into_iter().collect();
        let c = Composition::from_mass(map, &AtomicMasses::MassNumber).unwrap();
        let m = Resource::from(Material::new(2.5, c).unwrap());

        let rm = Request::simple(m, AgentId(1), "fuel").unwrap();
        let rp = Request::simple(product(3.0), AgentId(1), "power").unwrap();
        assert_eq!(rm.quantity(), 2.5);
        assert_eq!(rp.quantity(), 3.0);
    }
}
