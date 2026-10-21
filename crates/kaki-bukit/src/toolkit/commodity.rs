// PROVENANCE
//   Upstream project: CYCLUS <https://github.com/cyclus/cyclus>
//   Upstream file:    src/toolkit/commodity.h, src/toolkit/commodity.cc,
//                     src/toolkit/commodity_producer.h,
//                     src/toolkit/commodity_producer.cc
//   Upstream commit:  d4faab7ce0566ccb8febfcf50915cdeedee59db0
//   Upstream licence: BSD-3-Clause
//   Copyright (c) 2010-2016, University of Wisconsin Computational Nuclear
//   Engineering Research Group. All rights reserved.
//
//   Independent Rust translation, (C) 2026 Theodore Ong and the outram-park
//   contributors, GPL-3.0-only. Relicensing is ONE-WAY.

//! Named commodities, and who produces them at what capacity and cost.
//!
//! A **commodity** is the label a fuel-cycle market trades under — `"natural_u"`,
//! `"enriched_u"`, `"spent_fuel"`, `"power"`. It is a name and nothing else:
//! upstream's own comment calls the class "currently super simple" and explains
//! that it exists rather than a bare `std::string` only so the code reads
//! better and so the type can grow later. That reasoning carries over exactly,
//! and a newtype costs nothing here.
//!
//! A [`CommodityProducer`] is the registry an agent keeps of what it makes: for
//! each commodity, a **production capacity** and a **production cost**.
//!
//! # Units
//!
//! | Quantity | Units |
//! |---|---|
//! | [`CommodInfo::capacity`] | production capacity **per time step**, in the commodity's own units — kilograms for a material commodity, megawatt-hours or similar for a product one. Upstream fixes no unit and neither does this. |
//! | [`CommodInfo::cost`] | cost per unit of that commodity, dimensionless to the kernel. Defaults to [`MODIFIER_LIMIT`] (`1e10`), upstream's "effectively infinite" sentinel, so an unpriced commodity is never chosen by a cost-minimising decision. |
//!
//! # Divergences from upstream
//!
//! Upstream's `CommodityProducer` inherits `AgentManaged`, holding a back
//! pointer to the agent that owns it. There is no agent framework here yet, so
//! that link is absent; a facility owns a [`CommodityProducer`] by value.
//!
//! Two smaller behavioural differences are called out on the methods
//! themselves, because both are easy to trip over when reading the two files
//! side by side: [`CommodityProducer::add`] does not overwrite (matching
//! `std::map::insert`, which is not obvious), and the queries do **not** insert
//! a default entry the way upstream's `operator[]` silently does.
//!
//! [`MODIFIER_LIMIT`]: crate::limits::MODIFIER_LIMIT

use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use crate::limits::MODIFIER_LIMIT;

/// A named commodity traded in the fuel cycle. Upstream `Commodity`.
///
/// Ordered by name, so a [`CommodityProducer`]'s registry iterates
/// deterministically — the translation of upstream's `CommodityCompare`
/// functor, which exists for exactly that purpose ("we do not care how they
/// are compared, only that they can be").
#[derive(Debug, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Commodity {
    name: String,
}

impl Commodity {
    /// A commodity with the given name. Upstream `Commodity(std::string)`.
    ///
    /// The name is opaque to the kernel; it is matched exactly, including
    /// case and whitespace.
    #[must_use]
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
        }
    }

    /// The commodity's name. Upstream `Commodity::name()`.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }
}

impl From<&str> for Commodity {
    fn from(s: &str) -> Self {
        Self::new(s)
    }
}

impl From<String> for Commodity {
    fn from(name: String) -> Self {
        Self { name }
    }
}

/// What a producer can make of one commodity, and at what price. Upstream
/// `CommodInfo`.
///
/// See the [module docs](self) for the units, which the kernel does not fix.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CommodInfo {
    /// Production capacity per time step, in the commodity's own units.
    /// Non-negative in any sensible configuration; not checked, as upstream
    /// does not check it either.
    pub capacity: f64,
    /// Production cost per unit. Defaults to
    /// [`MODIFIER_LIMIT`](crate::limits::MODIFIER_LIMIT).
    pub cost: f64,
}

impl Default for CommodInfo {
    /// Upstream's defaults: zero capacity, cost
    /// [`MODIFIER_LIMIT`](crate::limits::MODIFIER_LIMIT).
    ///
    /// The cost default is the interesting half. `1e10` is upstream's
    /// stand-in for "prohibitively expensive", so a commodity registered
    /// without a price is never picked by anything minimising cost — it fails
    /// loudly in the result rather than quietly looking free.
    fn default() -> Self {
        Self {
            capacity: 0.0,
            cost: MODIFIER_LIMIT,
        }
    }
}

impl CommodInfo {
    /// A capacity/cost pair. Upstream `CommodInfo(capacity, cost)`.
    #[must_use]
    pub fn new(capacity: f64, cost: f64) -> Self {
        Self { capacity, cost }
    }
}

/// A registry of the commodities an agent produces. Upstream
/// `CommodityProducer`.
///
/// Held by value on the producing facility. The registry is ordered by
/// commodity name, so [`produced_commodities`](CommodityProducer::produced_commodities)
/// and any iteration over it reproduce run to run.
///
/// # Example
///
/// ```
/// use kaki_bukit::toolkit::commodity::{Commodity, CommodityProducer};
///
/// let mut mine = CommodityProducer::new();
/// let natu = Commodity::new("natural_u");
/// mine.add(&natu);
/// mine.set_capacity(&natu, 2.5e5);   // kg per time step
/// mine.set_cost(&natu, 40.0);        // per kg
///
/// assert!(mine.produces(&natu));
/// assert_eq!(mine.capacity(&natu), 2.5e5);
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct CommodityProducer {
    commodities: BTreeMap<Commodity, CommodInfo>,
    default_capacity: f64,
    default_cost: f64,
}

impl Default for CommodityProducer {
    fn default() -> Self {
        Self::new()
    }
}

impl CommodityProducer {
    /// A producer registering nothing, with upstream's defaults for any
    /// commodity later added: zero capacity, cost
    /// [`MODIFIER_LIMIT`](crate::limits::MODIFIER_LIMIT).
    #[must_use]
    pub fn new() -> Self {
        Self::with_defaults(0.0, MODIFIER_LIMIT)
    }

    /// A producer whose [`add`](CommodityProducer::add) uses the given
    /// defaults. Upstream `CommodityProducer(default_capacity, default_cost)`.
    ///
    /// # Parameters
    ///
    /// - `default_capacity` — production capacity per time step, in the
    ///   commodity's own units.
    /// - `default_cost` — cost per unit.
    #[must_use]
    pub fn with_defaults(default_capacity: f64, default_cost: f64) -> Self {
        Self {
            commodities: BTreeMap::new(),
            default_capacity,
            default_cost,
        }
    }

    /// `true` if `commodity` is registered. Upstream `Produces()`.
    #[must_use]
    pub fn produces(&self, commodity: &Commodity) -> bool {
        self.commodities.contains_key(commodity)
    }

    /// The registered production capacity per time step, in the commodity's
    /// own units. Upstream `Capacity()`.
    ///
    /// # Divergence
    ///
    /// Upstream indexes with `std::map::operator[]`, which **inserts a
    /// default-constructed entry** for an unregistered commodity — so asking
    /// an upstream producer what its capacity for `"plutonium"` is makes it
    /// start producing `"plutonium"` at zero capacity, and `Produces()` then
    /// returns `true`. That is an upstream accident, not a design. Here a
    /// query is a query: an unregistered commodity reports this producer's
    /// default capacity and the registry is unchanged.
    #[must_use]
    pub fn capacity(&self, commodity: &Commodity) -> f64 {
        self.commodities
            .get(commodity)
            .map_or(self.default_capacity, |i| i.capacity)
    }

    /// The registered production cost per unit. Upstream `Cost()`.
    ///
    /// Same non-inserting divergence as [`capacity`](CommodityProducer::capacity).
    #[must_use]
    pub fn cost(&self, commodity: &Commodity) -> f64 {
        self.commodities
            .get(commodity)
            .map_or(self.default_cost, |i| i.cost)
    }

    /// The full record for a commodity, or `None` if it is not registered.
    ///
    /// **Not an upstream member** — upstream has no non-inserting lookup at
    /// all. This is the honest form of [`capacity`](CommodityProducer::capacity)
    /// and [`cost`](CommodityProducer::cost) for a caller that needs to
    /// distinguish "registered at the default" from "not registered".
    #[must_use]
    pub fn info(&self, commodity: &Commodity) -> Option<&CommodInfo> {
        self.commodities.get(commodity)
    }

    /// Sets the production capacity per time step. Upstream `SetCapacity()`.
    ///
    /// Registers `commodity` if it was not already, at this producer's default
    /// cost — matching upstream, whose `operator[]` does the same.
    pub fn set_capacity(&mut self, commodity: &Commodity, capacity: f64) {
        self.entry(commodity).capacity = capacity;
    }

    /// Sets the production cost per unit. Upstream `SetCost()`.
    ///
    /// Registers `commodity` if it was not already, at this producer's default
    /// capacity.
    pub fn set_cost(&mut self, commodity: &Commodity, cost: f64) {
        self.entry(commodity).cost = cost;
    }

    /// Registers `commodity` at this producer's defaults. Upstream
    /// `Add(const Commodity&)`.
    ///
    /// **Does nothing if the commodity is already registered.** That is
    /// upstream's behaviour and it is not obvious from reading the C++:
    /// `commodities_.insert(std::make_pair(...))` on a `std::map` is a no-op
    /// when the key exists, so `Add` never overwrites an already-set capacity
    /// or cost. Reproduced here deliberately — a facility that re-declares a
    /// commodity must not silently lose the numbers it configured.
    pub fn add(&mut self, commodity: &Commodity) {
        self.add_with(commodity, CommodInfo::new(self.default_capacity, self.default_cost));
    }

    /// Registers `commodity` with an explicit record. Upstream
    /// `Add(const Commodity&, const CommodInfo&)`.
    ///
    /// Does nothing if already registered — see [`add`](CommodityProducer::add).
    pub fn add_with(&mut self, commodity: &Commodity, info: CommodInfo) {
        if !self.commodities.contains_key(commodity) {
            self.commodities.insert(commodity.clone(), info);
        }
    }

    /// Unregisters `commodity`. Upstream `Rm()`. A no-op if it was not
    /// registered.
    pub fn rm(&mut self, commodity: &Commodity) {
        self.commodities.remove(commodity);
    }

    /// Every registered commodity, in ascending name order. Upstream
    /// `ProducedCommodities()`.
    #[must_use]
    pub fn produced_commodities(&self) -> Vec<Commodity> {
        self.commodities.keys().cloned().collect()
    }

    /// The number of registered commodities.
    ///
    /// **Not an upstream member.** Convenience over
    /// [`produced_commodities`](CommodityProducer::produced_commodities),
    /// which allocates.
    #[must_use]
    pub fn len(&self) -> usize {
        self.commodities.len()
    }

    /// `true` if nothing is registered.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.commodities.is_empty()
    }

    /// Copies every commodity produced by `source`, with its capacity and
    /// cost. Upstream `Copy()`.
    ///
    /// This is how a facility prototype hands its production profile to the
    /// instances cloned from it. Commodities already registered here keep
    /// their own capacity and cost **overwritten** by the source's — upstream's
    /// `Copy` calls `Add` (a no-op on a duplicate) and then `SetCapacity` /
    /// `SetCost` unconditionally, so the source wins. Reproduced exactly.
    pub fn copy(&mut self, source: &Self) {
        for c in source.produced_commodities() {
            self.add(&c);
            self.set_capacity(&c, source.capacity(&c));
            self.set_cost(&c, source.cost(&c));
        }
    }

    /// Upstream's `operator[]`, confined to the mutating paths where it is
    /// what is wanted.
    fn entry(&mut self, commodity: &Commodity) -> &mut CommodInfo {
        let default = CommodInfo::new(self.default_capacity, self.default_cost);
        self.commodities
            .entry(commodity.clone())
            .or_insert(default)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn commodities_compare_and_order_by_name() {
        assert_eq!(Commodity::new("natural_u"), Commodity::from("natural_u"));
        assert_ne!(Commodity::new("natural_u"), Commodity::new("enriched_u"));
        assert!(Commodity::new("enriched_u") < Commodity::new("natural_u"));
        assert_eq!(Commodity::new("power").name(), "power");
        // Names are matched exactly.
        assert_ne!(Commodity::new("power"), Commodity::new("Power"));
    }

    #[test]
    fn an_unregistered_commodity_reports_the_defaults_without_registering() {
        let p = CommodityProducer::new();
        let c = Commodity::new("spent_fuel");
        assert!(!p.produces(&c));
        assert_eq!(p.capacity(&c), 0.0);
        assert_eq!(p.cost(&c), MODIFIER_LIMIT);
        assert!(p.info(&c).is_none());
        // The query did NOT register it — upstream's operator[] would have.
        assert!(!p.produces(&c));
        assert!(p.is_empty());
    }

    #[test]
    fn capacity_and_cost_round_trip() {
        let mut p = CommodityProducer::new();
        let c = Commodity::new("enriched_u");
        p.add(&c);
        assert!(p.produces(&c));
        assert_eq!(p.capacity(&c), 0.0);
        assert_eq!(p.cost(&c), MODIFIER_LIMIT);

        p.set_capacity(&c, 1500.0);
        p.set_cost(&c, 2400.0);
        assert_eq!(p.capacity(&c), 1500.0);
        assert_eq!(p.cost(&c), 2400.0);
        assert_eq!(*p.info(&c).unwrap(), CommodInfo::new(1500.0, 2400.0));
    }

    #[test]
    fn add_does_not_overwrite_an_existing_registration() {
        let mut p = CommodityProducer::new();
        let c = Commodity::new("natural_u");
        p.add_with(&c, CommodInfo::new(999.0, 3.0));
        // Upstream's std::map::insert is a no-op on an existing key.
        p.add(&c);
        p.add_with(&c, CommodInfo::new(1.0, 1.0));
        assert_eq!(p.capacity(&c), 999.0);
        assert_eq!(p.cost(&c), 3.0);
    }

    #[test]
    fn setting_a_value_registers_an_unknown_commodity() {
        let mut p = CommodityProducer::with_defaults(10.0, 5.0);
        let c = Commodity::new("power");
        p.set_capacity(&c, 1000.0);
        assert!(p.produces(&c));
        assert_eq!(p.capacity(&c), 1000.0);
        // The other field took the producer's default, not the global one.
        assert_eq!(p.cost(&c), 5.0);
    }

    #[test]
    fn rm_unregisters() {
        let mut p = CommodityProducer::new();
        let c = Commodity::new("power");
        p.add(&c);
        assert_eq!(p.len(), 1);
        p.rm(&c);
        assert!(!p.produces(&c));
        assert!(p.is_empty());
        p.rm(&c); // idempotent
    }

    #[test]
    fn produced_commodities_are_sorted_by_name() {
        let mut p = CommodityProducer::new();
        for n in ["spent_fuel", "natural_u", "enriched_u"] {
            p.add(&Commodity::new(n));
        }
        let got = p.produced_commodities();
        let names: Vec<&str> = got.iter().map(Commodity::name).collect();
        assert_eq!(names, ["enriched_u", "natural_u", "spent_fuel"]);
    }

    #[test]
    fn copy_transfers_the_whole_production_profile() {
        let mut proto = CommodityProducer::new();
        let a = Commodity::new("natural_u");
        let b = Commodity::new("power");
        proto.add_with(&a, CommodInfo::new(2.5e5, 40.0));
        proto.add_with(&b, CommodInfo::new(1000.0, 60.0));

        let mut inst = CommodityProducer::new();
        // Pre-existing entry: the source's numbers win.
        inst.add_with(&a, CommodInfo::new(1.0, 1.0));
        inst.copy(&proto);

        assert_eq!(inst.len(), 2);
        assert_eq!(inst.capacity(&a), 2.5e5);
        assert_eq!(inst.cost(&a), 40.0);
        assert_eq!(inst.capacity(&b), 1000.0);
        assert_eq!(inst.cost(&b), 60.0);
    }
}
