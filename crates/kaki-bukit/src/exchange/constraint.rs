// PROVENANCE
//   Upstream project: CYCLUS <https://github.com/cyclus/cyclus>
//   Upstream file:    src/capacity_constraint.h, and the Converter subclasses
//                     in src/request_portfolio.h (QtyCoeffConverter) and in
//                     CYCAMORE's src/enrichment.h and src/fuel_fab.cc
//   Upstream commit:  d4faab7ce0566ccb8febfcf50915cdeedee59db0
//   Upstream licence: BSD-3-Clause
//   Copyright (c) 2010-2016, University of Wisconsin Computational Nuclear
//   Engineering Research Group. All rights reserved.
//
//   Independent Rust translation, (C) 2026 Theodore Ong and the outram-park
//   contributors, GPL-3.0-only. Relicensing is ONE-WAY.

//! Capacity constraints: the limits an agent puts on what it can trade.
//!
//! A constraint is a **capacity** (a budget) plus a **converter** (an exchange
//! rate). The capacity says how much of some limited thing the agent has; the
//! converter says how much of that thing one unit of a given resource would
//! consume. Dividing the first by the second is how the solver turns "I have
//! 100 SWU of enrichment capacity this month" into "therefore I can supply at
//! most 43 kg of *this particular* enriched product".
//!
//! # The converter is an enum, not a functor
//!
//! Upstream's `Converter<T>` is an abstract base class with one pure virtual
//! `convert()`, held by `shared_ptr`, and subclassed once per conversion —
//! `TrivialConverter`, `QtyCoeffConverter`, and in CYCAMORE `SWUConverter`,
//! `NatUConverter`, `FissConverter`, `FillConverter`, `TopupConverter`. That
//! is a trait object, which this workspace forbids, so [`Converter`] is an
//! enum of the conversions that are actually used.
//!
//! **Adding a novel conversion means adding a variant.** That sounds worse
//! than `dyn` and is in fact better here, for three reasons:
//!
//! 1. **The set is closed and small.** Seven subclasses exist across CYCLUS
//!    and CYCAMORE combined, and five of them are `coefficient * quantity` or
//!    a constant. A closed set is what an enum is for.
//! 2. **A new variant is a compile error at every `match`**, which is exactly
//!    where a reviewer wants to be stopped. A new `dyn` subclass compiles
//!    silently and is discovered at run time, if at all.
//! 3. **Equality becomes real.** Upstream's `operator==` on a converter is a
//!    `dynamic_cast` that returns `false` by default, so two constraints that
//!    are semantically identical usually compare unequal — and constraints
//!    live in a `std::set`. Here `#[derive(PartialEq)]` compares what the
//!    converter actually is.
//!
//! The cost is real and should be stated: a downstream crate cannot add a
//! conversion without editing this enum. Given that a converter's result feeds
//! straight into a mass balance, having every conversion in the fuel cycle
//! visible in one place is the trade this crate wants.

use crate::error::{CyclusError, Result};
use crate::resource::Resource;

/// How much of a constrained quantity one resource consumes.
///
/// Upstream `cyclus::Converter<T>` and its subclasses; see the module doc for
/// why this is an enum.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Converter {
    /// The resource's own quantity, unchanged — kg for a material.
    ///
    /// Upstream `TrivialConverter<T>`, the default, and by a wide margin the
    /// most common: every plain `CapacityConstraint<Material>(throughput)` in
    /// CYCAMORE's source, sink, reactor, separations, conversion and fuel-fab
    /// agents uses it.
    Quantity,

    /// `coefficient * quantity`.
    ///
    /// Covers two upstream converters:
    ///
    /// * `QtyCoeffConverter`, which weights a request in a portfolio of
    ///   *mutual* requests (10 kg of MOX and 9 kg of UOX meeting one demand
    ///   get coefficients `9.5/10` and `9.5/9`). The coefficient is per
    ///   request, so translation resolves it — see
    ///   [`Converter::RequestMassCoeff`].
    /// * CYCAMORE's fuel-fab `FissConverter` / `FillConverter` /
    ///   `TopupConverter`, each of which is a blending fraction times the
    ///   offered quantity.
    ///
    /// The coefficient is dimensionless and normally in `[0, 1]`, though
    /// nothing requires that.
    Scaled(f64),

    /// A fixed amount, independent of the resource.
    ///
    /// Covers the two constant branches of CYCAMORE's fuel-fab converters:
    /// `0.0` ("this stream is not needed for this blend") and
    /// [`CY_LARGE_DOUBLE`](crate::limits::CY_LARGE_DOUBLE) ("this blend is
    /// impossible — do not bid at all", which drives the computed capacity to
    /// effectively zero).
    Constant(f64),

    /// `quantity * (the requesting node's mass coefficient)`.
    ///
    /// This is upstream's `QtyCoeffConverter` in its unresolved form: the
    /// coefficient depends on *which request* the arc leads to, which the
    /// converter cannot know until the arc exists. Upstream passes the arc and
    /// an `ExchangeTranslationContext` into `convert()` to look it up; here
    /// [`convert`](Self::convert) takes the coefficient as its second argument
    /// and the translator supplies it.
    ///
    /// Everywhere else the second argument is `1.0`, making this identical to
    /// [`Converter::Quantity`].
    RequestMassCoeff,
}

impl Converter {
    /// Converts `offer` into the constrained quantity it would consume.
    ///
    /// `request_mass_coeff` is the per-request weighting for
    /// [`Converter::RequestMassCoeff`]; pass `1.0` for every other variant and
    /// wherever there is no request context (all bid-side constraints).
    ///
    /// The returned value is in the *constraint's* units, which are not
    /// generally the resource's.
    #[must_use]
    pub fn convert(&self, offer: &Resource, request_mass_coeff: f64) -> f64 {
        match self {
            Self::Quantity => offer.quantity(),
            Self::Scaled(c) => c * offer.quantity(),
            Self::Constant(c) => *c,
            Self::RequestMassCoeff => offer.quantity() * request_mass_coeff,
        }
    }
}

impl Default for Converter {
    /// [`Converter::Quantity`], matching upstream's `TrivialConverter` default.
    fn default() -> Self {
        Self::Quantity
    }
}

/// A limit on what an agent can trade: a capacity plus how resources consume
/// it.
///
/// Upstream `cyclus::CapacityConstraint<T>`.
///
/// # Conversions deliberately not ported
///
/// CYCAMORE's `SWUConverter` and `NatUConverter` (separative work and natural
/// uranium feed for an enrichment plant) are **not** variants of
/// [`Converter`]. Both are built on the enrichment toolkit — `Assays`,
/// `SwuRequired`, `FeedQty`, `UraniumAssayMass` — which lives in
/// [`toolkit`](crate::toolkit) and is not part of this module's port. Writing
/// the assay arithmetic a second time here to fill the enum would create the
/// duplicate implementation this workspace exists to avoid. When the toolkit
/// lands they are two more variants and one more `match` arm each.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CapacityConstraint {
    capacity: f64,
    converter: Converter,
}

impl CapacityConstraint {
    /// A constraint of `capacity` units consumed at the resource's own
    /// quantity (the trivial converter).
    ///
    /// # Errors
    ///
    /// [`CyclusError::Value`] if `capacity` is not strictly positive.
    /// Upstream throws exactly here, with the reasoning that a non-positive
    /// capacity silently blocks every trade — CYCAMORE's fuel fab guards
    /// against it with `std::max(qty, CY_NEAR_ZERO)` at three call sites.
    pub fn new(capacity: f64) -> Result<Self> {
        Self::with_converter(capacity, Converter::Quantity)
    }

    /// A constraint of `capacity` units in the converter's own units.
    ///
    /// # Errors
    ///
    /// [`CyclusError::Value`] if `capacity` is not strictly positive.
    pub fn with_converter(capacity: f64, converter: Converter) -> Result<Self> {
        if !(capacity > 0.0) {
            return Err(CyclusError::Value(
                "capacity is not positive, no trades will be executed",
            ));
        }
        Ok(Self {
            capacity,
            converter,
        })
    }

    /// The capacity, in the converter's units.
    #[must_use]
    pub fn capacity(&self) -> f64 {
        self.capacity
    }

    /// The converter.
    #[must_use]
    pub fn converter(&self) -> Converter {
        self.converter
    }

    /// Shorthand for `self.converter().convert(offer, request_mass_coeff)`.
    #[must_use]
    pub fn convert(&self, offer: &Resource, request_mass_coeff: f64) -> f64 {
        self.converter.convert(offer, request_mass_coeff)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::limits::CY_LARGE_DOUBLE;
    use crate::product::Product;

    fn product(qty: f64) -> Resource {
        Resource::from(Product::new(qty, "MWh").unwrap())
    }

    #[test]
    fn the_trivial_converter_returns_the_resources_own_quantity() {
        assert_eq!(Converter::Quantity.convert(&product(12.5), 1.0), 12.5);
        assert_eq!(Converter::default(), Converter::Quantity);
    }

    #[test]
    fn a_scaled_converter_is_a_blend_fraction_times_the_quantity() {
        // CYCAMORE fuel-fab: AtomToMassFrac(frac, ..) * m->quantity().
        assert_eq!(Converter::Scaled(0.25).convert(&product(8.0), 1.0), 2.0);
    }

    #[test]
    fn a_constant_converter_ignores_the_resource() {
        // The fuel-fab "this stream is not needed" and "do not bid" branches.
        assert_eq!(Converter::Constant(0.0).convert(&product(8.0), 1.0), 0.0);
        assert_eq!(
            Converter::Constant(CY_LARGE_DOUBLE).convert(&product(8.0), 1.0),
            CY_LARGE_DOUBLE
        );
    }

    #[test]
    fn the_request_mass_coefficient_weights_mutual_requests() {
        // 10 kg of MOX and 9 kg of UOX meeting one demand of 9.5 kg.
        let c = Converter::RequestMassCoeff;
        assert_eq!(c.convert(&product(10.0), 9.5 / 10.0), 9.5);
        assert_eq!(c.convert(&product(9.0), 9.5 / 9.0), 9.5);
        // With no request context it degenerates to the trivial converter.
        assert_eq!(c.convert(&product(10.0), 1.0), 10.0);
    }

    #[test]
    fn a_non_positive_capacity_is_rejected_as_upstream_does() {
        for bad in [0.0, -1.0] {
            assert_eq!(
                CapacityConstraint::new(bad).unwrap_err(),
                CyclusError::Value("capacity is not positive, no trades will be executed")
            );
        }
        assert!(CapacityConstraint::new(f64::NAN).is_err());
    }

    #[test]
    fn a_constraint_reports_its_capacity_and_converts_through_it() {
        let c = CapacityConstraint::with_converter(100.0, Converter::Scaled(0.5)).unwrap();
        assert_eq!(c.capacity(), 100.0);
        assert_eq!(c.converter(), Converter::Scaled(0.5));
        assert_eq!(c.convert(&product(4.0), 1.0), 2.0);
    }

    #[test]
    fn two_semantically_identical_constraints_compare_equal() {
        // Upstream's dynamic_cast-based operator== returns false by default,
        // so this is a behavioural improvement, not a translation of one.
        let a = CapacityConstraint::with_converter(3.0, Converter::Scaled(0.5)).unwrap();
        let b = CapacityConstraint::with_converter(3.0, Converter::Scaled(0.5)).unwrap();
        assert_eq!(a, b);
    }
}
