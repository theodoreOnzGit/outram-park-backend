// PROVENANCE
//   Upstream project: CYCAMORE <https://github.com/cyclus/cycamore>
//   Upstream file:    src/sink.h, src/sink.cc
//   Upstream commit:  fee8c80190e0b91dafccae6b1a3129dc544441e8
//   Upstream licence: BSD-3-Clause
//   Copyright (c) 2010-2016, University of Wisconsin Computational Nuclear
//   Engineering Research Group. All rights reserved.
//
//   Independent Rust translation, (C) 2026 Theodore Ong and the outram-park
//   contributors, GPL-3.0-only. Relicensing is ONE-WAY.

//! A material sink: the downstream end of a fuel cycle.
//!
//! A repository, or any other terminus that accepts material and never
//! releases it. It requests one or more commodities, limited by a
//! per-time-step **capacity** and a total **inventory size**.

use alloc::string::{String, ToString};
use alloc::vec::Vec;

use crate::agent::AgentId;
use crate::composition::{AtomicMasses, Composition};
use crate::context::Context;
use crate::error::{CyclusError, Result};
use crate::exchange::{CapacityConstraint, RequestPortfolio};
use crate::limits::{EPS, UNLIMITED};
use crate::material::Material;
use crate::nuclide::nuc;
use crate::resource::Resource;
use crate::toolkit::res_buf::ResBuf;

use super::{Delivery, Facility, Order};

/// A facility that accepts one or more commodities and supplies nothing.
///
/// # What it asks for
///
/// If `inrecipe` is set the sink requests that composition; otherwise it
/// requests a placeholder composition, since the exchange needs a target
/// resource to size the request even when the requester does not care what it
/// receives. Upstream does the same, using a single unit of a nuclide as the
/// placeholder.
#[derive(Debug, Clone, PartialEq)]
pub struct Sink {
    in_commods: Vec<String>,
    inrecipe: Option<String>,
    capacity: f64,
    inventory: ResBuf,
}

impl Sink {
    /// A sink accepting `in_commods`, with unbounded capacity and inventory.
    ///
    /// # Errors
    ///
    /// [`CyclusError::Value`] if `in_commods` is empty or contains an empty
    /// name.
    pub fn new(in_commods: &[&str]) -> Result<Self> {
        if in_commods.is_empty() {
            return Err(CyclusError::Value("a sink needs at least one in-commodity"));
        }
        if in_commods.iter().any(|c| c.is_empty()) {
            return Err(CyclusError::Value("an in-commodity name cannot be empty"));
        }
        Ok(Self {
            in_commods: in_commods.iter().map(|s| (*s).to_string()).collect(),
            inrecipe: None,
            capacity: UNLIMITED,
            inventory: ResBuf::new(),
        })
    }

    /// Sets the maximum mass accepted per time step, in kilograms.
    ///
    /// # Errors
    ///
    /// [`CyclusError::Value`] if negative.
    pub fn with_capacity(mut self, kg_per_step: f64) -> Result<Self> {
        if kg_per_step < 0.0 {
            return Err(CyclusError::Value("capacity cannot be negative"));
        }
        self.capacity = kg_per_step;
        Ok(self)
    }

    /// Sets the total mass this sink can ever hold, in kilograms.
    ///
    /// # Errors
    ///
    /// [`CyclusError::Value`] if negative.
    pub fn with_max_inventory(mut self, kg: f64) -> Result<Self> {
        self.inventory = ResBuf::with_capacity(kg)?;
        Ok(self)
    }

    /// Requests this named recipe rather than a placeholder composition.
    #[must_use]
    pub fn with_recipe(mut self, recipe: &str) -> Self {
        self.inrecipe = Some(recipe.to_string());
        self
    }

    /// The commodities accepted.
    #[must_use]
    pub fn in_commods(&self) -> &[String] {
        &self.in_commods
    }

    /// The mass currently held, in kilograms.
    #[must_use]
    pub fn quantity(&self) -> f64 {
        self.inventory.quantity()
    }

    /// Remaining room, in kilograms.
    #[must_use]
    pub fn space(&self) -> f64 {
        self.inventory.space()
    }

    /// The inventory buffer, for inspection.
    #[must_use]
    pub fn inventory(&self) -> &ResBuf {
        &self.inventory
    }

    /// The most this sink will take this step: the lesser of its per-step
    /// capacity and its remaining room. Upstream's `RequestAmt`.
    #[must_use]
    pub fn request_amt(&self) -> f64 {
        let space = self.inventory.space();
        if self.capacity < space {
            self.capacity
        } else {
            space
        }
    }

    /// The composition to request.
    fn request_comp(&self, ctx: &Context, masses: &AtomicMasses) -> Result<Composition> {
        match &self.inrecipe {
            Some(name) => Ok(ctx.recipe(name)?.clone()),
            // Upstream requests a single unit of one nuclide when it has no
            // recipe: the exchange needs a target to size the request, but a
            // sink that takes anything does not care what it is.
            None => Composition::from_nuclide(nuc::U238, masses),
        }
    }
}

impl Facility for Sink {
    /// Requests [`request_amt`](Sink::request_amt) of each accepted commodity.
    ///
    /// All the requests share one portfolio carrying a single
    /// [`CapacityConstraint`] at `request_amt`. That is what makes them
    /// **alternatives rather than a sum**: a sink accepting three commodities
    /// asks each for the full amount, but can only receive that amount in
    /// total. Upstream arranges it the same way, and dropping the constraint
    /// would let the sink take three times its capacity.
    fn material_requests(&mut self, ctx: &Context, me: AgentId) -> Result<Vec<RequestPortfolio>> {
        let amt = self.request_amt();
        if amt < EPS {
            return Ok(Vec::new());
        }
        // Requests are sized in kg, and the placeholder composition only
        // has to be *a* composition; MassNumber is adequate and needs no data.
        let comp = self.request_comp(ctx, &AtomicMasses::MassNumber)?;
        let mut port = RequestPortfolio::new();
        for commod in &self.in_commods {
            let target = Resource::from(Material::new(amt, comp.clone())?);
            port.request(target, me, commod, crate::exchange::DEFAULT_PREF, false)?;
        }
        port.add_constraint(CapacityConstraint::new(amt)?);
        Ok(alloc::vec![port])
    }

    /// A sink never supplies anything.
    ///
    /// # Errors
    ///
    /// [`CyclusError::State`] if given orders, which cannot happen through the
    /// protocol since it never bids.
    fn respond_to_trades(
        &mut self,
        _ctx: &Context,
        orders: &[Order],
        _masses: &AtomicMasses,
    ) -> Result<Vec<Resource>> {
        if orders.is_empty() {
            Ok(Vec::new())
        } else {
            Err(CyclusError::State("a sink cannot supply material"))
        }
    }

    /// Stores everything received.
    ///
    /// # Errors
    ///
    /// [`CyclusError::Value`] if a delivery overruns the inventory capacity,
    /// which means the constraint declared in
    /// [`material_requests`](Sink::material_requests) failed to bind.
    ///
    /// Note the resource is **moved** into the buffer, so a rejected push
    /// would destroy it; [`ResBuf::push`] hands it back in the error instead,
    /// and the mass is lost only when that `Rejected` is converted into a
    /// plain [`CyclusError`]. That conversion happens here, at the end of the
    /// line for this material, which is the one place it is harmless.
    fn accept_trades(
        &mut self,
        _ctx: &Context,
        deliveries: Vec<Delivery>,
        _masses: &AtomicMasses,
    ) -> Result<()> {
        for d in deliveries {
            self.inventory.push(d.resource)?;
        }
        Ok(())
    }

    fn summary(&self) -> String {
        let mut s = String::from("Sink holding ");
        s.push_str(&self.in_commods.join(", "));
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::comp_math::CompMap;
    use crate::exchange::{BidId, RequestId, Trade};
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

    fn delivery(qty: f64) -> Delivery {
        Delivery {
            trade: Trade::new(
                RequestId {
                    portfolio: 0,
                    index: 0,
                },
                BidId {
                    portfolio: 0,
                    index: 0,
                },
                qty,
            ),
            commodity: "natu".to_string(),
            resource: Resource::from(Material::new(qty, natural_u()).unwrap()),
        }
    }

    #[test]
    fn requests_its_capacity_for_each_commodity_under_one_constraint() {
        let mut s = Sink::new(&["natu", "spent_uox"])
            .unwrap()
            .with_capacity(50.0)
            .unwrap();
        let ports = s.material_requests(&ctx(), AgentId(0)).unwrap();
        assert_eq!(ports.len(), 1);
        assert_eq!(ports[0].requests().len(), 2);
        for r in ports[0].requests() {
            assert_eq!(r.quantity(), 50.0);
        }
        // The single constraint is what stops it taking 2 x 50.
        assert_eq!(ports[0].constraints().len(), 1);
    }

    #[test]
    fn capacity_and_remaining_space_both_cap_the_request() {
        let c = ctx();
        let mut s = Sink::new(&["natu"])
            .unwrap()
            .with_capacity(50.0)
            .unwrap()
            .with_max_inventory(70.0)
            .unwrap();
        assert_eq!(s.request_amt(), 50.0, "per-step capacity binds first");

        s.accept_trades(&c, vec![delivery(40.0)], &masses()).unwrap();
        assert_eq!(s.quantity(), 40.0);
        assert_eq!(s.space(), 30.0);
        assert_eq!(s.request_amt(), 30.0, "remaining space now binds");
    }

    #[test]
    fn a_full_sink_stops_requesting() {
        let c = ctx();
        let mut s = Sink::new(&["natu"]).unwrap().with_max_inventory(10.0).unwrap();
        s.accept_trades(&c, vec![delivery(10.0)], &masses()).unwrap();
        assert_eq!(s.space(), 0.0);
        assert!(s.material_requests(&c, AgentId(0)).unwrap().is_empty());
    }

    #[test]
    fn accepted_material_accumulates_and_conserves_mass() {
        let c = ctx();
        let mut s = Sink::new(&["natu"]).unwrap();
        for q in [10.0, 20.5, 3.25] {
            s.accept_trades(&c, vec![delivery(q)], &masses()).unwrap();
        }
        assert!((s.quantity() - 33.75).abs() < 1e-12);
        assert_eq!(s.inventory().count(), 3);
    }

    #[test]
    fn an_overrun_delivery_is_refused() {
        let c = ctx();
        let mut s = Sink::new(&["natu"]).unwrap().with_max_inventory(5.0).unwrap();
        assert!(s.accept_trades(&c, vec![delivery(10.0)], &masses()).is_err());
        assert_eq!(s.quantity(), 0.0);
    }

    #[test]
    fn a_sink_refuses_to_supply() {
        let c = ctx();
        let mut s = Sink::new(&["natu"]).unwrap();
        assert!(s.respond_to_trades(&c, &[], &masses()).unwrap().is_empty());
        let o = Order {
            trade: Trade::new(
                RequestId {
                    portfolio: 0,
                    index: 0,
                },
                BidId {
                    portfolio: 0,
                    index: 0,
                },
                1.0,
            ),
            commodity: "natu".to_string(),
            target: Resource::from(Material::new(1.0, natural_u()).unwrap()),
        };
        assert!(s.respond_to_trades(&c, &[o], &masses()).is_err());
    }

    #[test]
    fn a_recipe_sink_requests_that_composition() {
        let mut s = Sink::new(&["natu"])
            .unwrap()
            .with_capacity(10.0)
            .unwrap()
            .with_recipe("natural_u");
        let ports = s.material_requests(&ctx(), AgentId(0)).unwrap();
        let target = ports[0].requests()[0].target().as_material().unwrap();
        assert!((target.comp().mass_frac(nuc::U235) - 0.00711).abs() < 1e-12);
    }

    #[test]
    fn rejects_an_empty_commodity_list_or_name() {
        assert!(Sink::new(&[]).is_err());
        assert!(Sink::new(&["natu", ""]).is_err());
        assert!(Sink::new(&["natu"]).unwrap().with_capacity(-1.0).is_err());
    }
}
