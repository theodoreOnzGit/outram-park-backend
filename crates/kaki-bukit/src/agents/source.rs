// PROVENANCE
//   Upstream project: CYCAMORE <https://github.com/cyclus/cycamore>
//   Upstream file:    src/source.h, src/source.cc
//   Upstream commit:  fee8c80190e0b91dafccae6b1a3129dc544441e8
//   Upstream licence: BSD-3-Clause
//   Copyright (c) 2010-2016, University of Wisconsin Computational Nuclear
//   Engineering Research Group. All rights reserved.
//
//   Independent Rust translation, (C) 2026 Theodore Ong and the outram-park
//   contributors, GPL-3.0-only. Relicensing is ONE-WAY.

//! A material source: the upstream end of a fuel cycle.
//!
//! A mine, or anything else that supplies material without consuming any. It
//! offers a single commodity, limited by a per-time-step **throughput** and a
//! lifetime **inventory**, both of which default to unbounded.

use alloc::string::{String, ToString};
use alloc::vec::Vec;

use crate::agent::AgentId;
use crate::composition::{AtomicMasses, Composition};
use crate::context::Context;
use crate::error::{CyclusError, Result};
use crate::exchange::{BidPortfolio, CapacityConstraint};
use crate::limits::{EPS, UNLIMITED};
use crate::material::Material;
use crate::resource::Resource;

use super::{CommodRequests, Delivery, Facility, Order};

/// A facility that supplies one commodity and requests nothing.
///
/// # Composition behaviour
///
/// Upstream: if `outrecipe` is set the source supplies that composition; if it
/// is empty the source supplies **exactly the composition the requester asked
/// for**. Both are reproduced. The second is what makes a `Source` usable as a
/// generic "material appears here" stub in a test deck.
#[derive(Debug, Clone, PartialEq)]
pub struct Source {
    outcommod: String,
    outrecipe: Option<String>,
    throughput: f64,
    inventory_size: f64,
    supplied: f64,
}

impl Source {
    /// A source of `outcommod` with unbounded throughput and inventory.
    ///
    /// # Errors
    ///
    /// [`CyclusError::Value`] if `outcommod` is empty.
    pub fn new(outcommod: &str) -> Result<Self> {
        if outcommod.is_empty() {
            return Err(CyclusError::Value("a source needs an output commodity"));
        }
        Ok(Self {
            outcommod: outcommod.to_string(),
            outrecipe: None,
            throughput: UNLIMITED,
            inventory_size: UNLIMITED,
            supplied: 0.0,
        })
    }

    /// Sets the maximum mass supplied per time step, in kilograms.
    ///
    /// # Errors
    ///
    /// [`CyclusError::Value`] if negative.
    pub fn with_throughput(mut self, kg_per_step: f64) -> Result<Self> {
        if kg_per_step < 0.0 {
            return Err(CyclusError::Value("throughput cannot be negative"));
        }
        self.throughput = kg_per_step;
        Ok(self)
    }

    /// Sets the total mass this source can ever supply, in kilograms.
    ///
    /// # Errors
    ///
    /// [`CyclusError::Value`] if negative.
    pub fn with_inventory(mut self, kg: f64) -> Result<Self> {
        if kg < 0.0 {
            return Err(CyclusError::Value("inventory size cannot be negative"));
        }
        self.inventory_size = kg;
        Ok(self)
    }

    /// Supplies this named recipe instead of the requested composition.
    #[must_use]
    pub fn with_recipe(mut self, recipe: &str) -> Self {
        self.outrecipe = Some(recipe.to_string());
        self
    }

    /// The commodity supplied.
    #[must_use]
    pub fn outcommod(&self) -> &str {
        &self.outcommod
    }

    /// Total mass supplied so far, in kilograms.
    #[must_use]
    pub fn supplied(&self) -> f64 {
        self.supplied
    }

    /// Mass still available over the source's lifetime, in kilograms.
    ///
    /// [`UNLIMITED`] stays [`UNLIMITED`]: subtracting from `f64::MAX` must be
    /// a no-op, exactly as it is in the exchange solver's capacity
    /// bookkeeping.
    #[must_use]
    pub fn remaining(&self) -> f64 {
        if self.inventory_size == UNLIMITED {
            UNLIMITED
        } else {
            self.inventory_size - self.supplied
        }
    }

    /// The most this source may supply this step: the lesser of its throughput
    /// and what is left of its inventory. Upstream's `max_qty`.
    #[must_use]
    pub fn max_qty(&self) -> f64 {
        let r = self.remaining();
        if self.throughput < r {
            self.throughput
        } else {
            r
        }
    }

    /// The composition to offer against a request for `target`.
    fn offer_comp(&self, ctx: &Context, target: &Resource) -> Result<Composition> {
        match &self.outrecipe {
            Some(name) => Ok(ctx.recipe(name)?.clone()),
            None => target
                .as_material()
                .map(|m| m.comp().clone())
                .ok_or(CyclusError::Value(
                    "a source with no recipe can only answer material requests",
                )),
        }
    }
}

impl Facility for Source {
    /// Bids on every request for [`outcommod`](Source::outcommod), up to
    /// [`max_qty`](Source::max_qty).
    ///
    /// The portfolio carries one [`CapacityConstraint`] at `max_qty`, which is
    /// what stops the solver from matching more than this step's throughput
    /// across several requesters — the bids themselves are each sized to their
    /// own request, so without the constraint they could sum above it. That is
    /// upstream's arrangement exactly.
    fn material_bids(
        &mut self,
        ctx: &Context,
        me: AgentId,
        commod_requests: &CommodRequests,
    ) -> Result<Vec<BidPortfolio>> {
        let max_qty = self.max_qty();
        if max_qty < EPS {
            return Ok(Vec::new());
        }
        let Some(requests) = commod_requests.get(&self.outcommod) else {
            return Ok(Vec::new());
        };

        let mut port = BidPortfolio::new();
        for (rid, req) in requests {
            let qty = if req.quantity() < max_qty {
                req.quantity()
            } else {
                max_qty
            };
            if qty < EPS {
                continue;
            }
            let comp = self.offer_comp(ctx, req.target())?;
            let offer = Resource::from(Material::new(qty, comp)?);
            port.bid(*rid, offer, me, false)?;
        }
        if port.bids().is_empty() {
            return Ok(Vec::new());
        }
        port.add_constraint(CapacityConstraint::new(max_qty)?);
        Ok(alloc::vec![port])
    }

    /// Produces the matched material and charges it against the inventory.
    ///
    /// # Errors
    ///
    /// [`CyclusError::State`] if the orders total more than
    /// [`max_qty`](Source::max_qty), which would mean the capacity constraint
    /// above failed to bind — a solver bug rather than a configuration one, so
    /// it is worth failing loudly instead of quietly over-supplying.
    fn respond_to_trades(
        &mut self,
        ctx: &Context,
        orders: &[Order],
        _masses: &AtomicMasses,
    ) -> Result<Vec<Resource>> {
        let total: f64 = orders.iter().map(|o| o.trade.amt).sum();
        if total - self.max_qty() > EPS {
            return Err(CyclusError::State(
                "a source was matched above its declared capacity",
            ));
        }

        let mut out = Vec::with_capacity(orders.len());
        for o in orders {
            let comp = self.offer_comp(ctx, &o.target)?;
            out.push(Resource::from(Material::new(o.trade.amt, comp)?));
            self.supplied += o.trade.amt;
        }
        Ok(out)
    }

    /// A source never receives anything.
    ///
    /// # Errors
    ///
    /// [`CyclusError::State`] if handed a delivery, which would mean the
    /// driver matched it as a requester — it never posts requests, so this
    /// cannot happen through the protocol.
    fn accept_trades(
        &mut self,
        _ctx: &Context,
        deliveries: Vec<Delivery>,
        _masses: &AtomicMasses,
    ) -> Result<()> {
        if deliveries.is_empty() {
            Ok(())
        } else {
            Err(CyclusError::State("a source cannot receive material"))
        }
    }

    /// Retires once the lifetime inventory is exhausted. Upstream's
    /// `CheckDecommissionCondition` equivalent for an exhausted source.
    fn check_decommission(&self, _ctx: &Context) -> bool {
        self.inventory_size != UNLIMITED && self.remaining() < EPS
    }

    fn summary(&self) -> String {
        let mut s = String::from("Source supplying ");
        s.push_str(&self.outcommod);
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::comp_math::CompMap;
    use crate::exchange::{Request, RequestId};
    use crate::nuclide::nuc;
    use crate::sim::SimInfo;
    use alloc::vec;

    fn masses() -> AtomicMasses {
        AtomicMasses::MassNumber
    }

    fn natural_u() -> Composition {
        let m: CompMap = [(nuc::U235, 0.00711), (nuc::U238, 0.99289)]
            .into_iter()
            .collect();
        Composition::from_mass(m, &masses()).unwrap()
    }

    fn ctx() -> Context {
        let mut c = Context::new(SimInfo::new(12)).unwrap();
        c.add_recipe("natural_u", natural_u()).unwrap();
        c
    }

    fn requests_for(commod: &str, qty: f64) -> CommodRequests {
        let target = Resource::from(Material::new(qty, natural_u()).unwrap());
        let req = Request::simple(target, AgentId(1), commod).unwrap();
        let mut m = CommodRequests::new();
        m.insert(
            commod.to_string(),
            vec![(
                RequestId {
                    portfolio: 0,
                    index: 0,
                },
                req,
            )],
        );
        m
    }

    #[test]
    fn bids_the_requested_quantity_when_unconstrained() {
        let mut s = Source::new("natu").unwrap().with_recipe("natural_u");
        let ports = s
            .material_bids(&ctx(), AgentId(0), &requests_for("natu", 100.0))
            .unwrap();
        assert_eq!(ports.len(), 1);
        assert_eq!(ports[0].bids().len(), 1);
        assert_eq!(ports[0].bids()[0].quantity(), 100.0);
    }

    #[test]
    fn throughput_caps_the_bid() {
        let mut s = Source::new("natu")
            .unwrap()
            .with_recipe("natural_u")
            .with_throughput(30.0)
            .unwrap();
        let ports = s
            .material_bids(&ctx(), AgentId(0), &requests_for("natu", 100.0))
            .unwrap();
        assert_eq!(ports[0].bids()[0].quantity(), 30.0);
        // And the portfolio constraint binds at the same value.
        assert_eq!(ports[0].constraints().len(), 1);
    }

    #[test]
    fn does_not_bid_on_a_commodity_it_does_not_supply() {
        let mut s = Source::new("natu").unwrap().with_recipe("natural_u");
        let ports = s
            .material_bids(&ctx(), AgentId(0), &requests_for("spent_uox", 100.0))
            .unwrap();
        assert!(ports.is_empty());
    }

    #[test]
    fn with_no_recipe_it_supplies_the_requested_composition() {
        let mut s = Source::new("natu").unwrap();
        let c = ctx();
        let reqs = requests_for("natu", 10.0);
        let ports = s.material_bids(&c, AgentId(0), &reqs).unwrap();
        let offered = ports[0].bids()[0].offer().as_material().unwrap();
        assert!((offered.comp().mass_frac(nuc::U235) - 0.00711).abs() < 1e-12);
    }

    #[test]
    fn a_finite_inventory_depletes_and_then_retires_the_source() {
        let c = ctx();
        let mut s = Source::new("natu")
            .unwrap()
            .with_recipe("natural_u")
            .with_inventory(50.0)
            .unwrap();
        assert_eq!(s.max_qty(), 50.0);

        let order = Order {
            trade: crate::exchange::Trade::new(
                RequestId {
                    portfolio: 0,
                    index: 0,
                },
                crate::exchange::BidId {
                    portfolio: 0,
                    index: 0,
                },
                50.0,
            ),
            commodity: "natu".to_string(),
            target: Resource::from(Material::new(50.0, natural_u()).unwrap()),
        };
        let got = s.respond_to_trades(&c, &[order], &masses()).unwrap();
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].quantity(), 50.0);

        assert_eq!(s.supplied(), 50.0);
        assert_eq!(s.remaining(), 0.0);
        assert!(s.check_decommission(&c));

        // And it bids nothing further.
        let ports = s
            .material_bids(&c, AgentId(0), &requests_for("natu", 10.0))
            .unwrap();
        assert!(ports.is_empty());
    }

    #[test]
    fn an_unbounded_inventory_never_retires() {
        let c = ctx();
        let s = Source::new("natu").unwrap();
        assert_eq!(s.remaining(), UNLIMITED);
        assert!(!s.check_decommission(&c));
    }

    #[test]
    fn a_source_refuses_a_delivery() {
        let c = ctx();
        let mut s = Source::new("natu").unwrap();
        assert!(s.accept_trades(&c, vec![], &masses()).is_ok());
        let d = Delivery {
            trade: crate::exchange::Trade::new(
                RequestId {
                    portfolio: 0,
                    index: 0,
                },
                crate::exchange::BidId {
                    portfolio: 0,
                    index: 0,
                },
                1.0,
            ),
            commodity: "natu".to_string(),
            resource: Resource::from(Material::new(1.0, natural_u()).unwrap()),
        };
        assert!(s.accept_trades(&c, vec![d], &masses()).is_err());
    }

    #[test]
    fn rejects_an_empty_commodity_and_negative_limits() {
        assert!(Source::new("").is_err());
        assert!(Source::new("natu").unwrap().with_throughput(-1.0).is_err());
        assert!(Source::new("natu").unwrap().with_inventory(-1.0).is_err());
    }
}
