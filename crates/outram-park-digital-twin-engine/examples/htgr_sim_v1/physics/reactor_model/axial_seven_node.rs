//! Seven-node axial pebble-bed core -- **placeholder**, falls back to
//! [`super::one_node`]'s [`PebbleBedPorousMediaNode`].
//!
//! ## What this tier is for
//!
//! [`super::one_node`]'s real tiers lump the whole 1.97 m bed height into one
//! spatial node, so the 250 degC-in / 700 degC-out published operating swing
//! does not exist anywhere in either -- only a single bed-average temperature
//! does. This tier is the first rung of the refinement path
//! [`super::one_node`]'s own module doc comment names: split the bed
//! **axially** first, before splitting radially inside a pebble.
//!
//! ## Scaffolded design (not implemented)
//!
//! Seven stacked control volumes along the flow direction (helium flows
//! **downward** through this bed, so node 0 is the top/inlet, node 6 the
//! bottom/outlet -- see [`super::one_node`] for the flow direction and why).
//! Each node would need:
//!
//! - its own two-temperature (solid + fluid) state, seven
//!   [`PebbleBedPorousMediaNode`]-shaped balances rather than one, sized by
//!   `1/7` of the bed's total graphite mass, heat-transfer area and void
//!   volume -- all already derived quantities in [`super::one_node`]
//!   ([`super::one_node::graphite_mass`], [`super::one_node::heat_transfer_area`]),
//!   so this tier does not need new geometry, only geometry sliced seven
//!   ways;
//! - the same evaluated Wakao film + intra-pebble conduction coefficient
//!   ([`super::one_node::overall_htc_at_flow`]) applied per node at that
//!   node's own local helium temperature, not the bulk mean -- this is
//!   exactly the resolution [`super::one_node`]'s doc comment says a
//!   whole-bed evaluation cannot buy;
//! - the helium marching node-to-node, each node's fluid temperature becoming
//!   the next node's inlet, with an implicit two-temperature backward-Euler
//!   solve per node (the same coupled solid/fluid system
//!   [`PebbleBedPorousMediaNode::step`] already assembles for one node, just
//!   evaluated seven times in series instead of once over the whole bed --
//!   or, more efficiently, as one banded 14x14 system solved simultaneously);
//! - optionally, axial conduction between adjacent graphite nodes through
//!   the Zehner-Bauer-Schlunder effective bed conductivity
//!   ([`outram_park_digital_twin_engine::htr10::zbs`]), which
//!   [`super::one_node`] already computes but leaves out of its heat path
//!   for exactly the reason this tier would put it back in: a nodalised bed
//!   has an internal gradient for a conductivity to act on, a single control
//!   volume does not.
//!
//! **A real precedent for this marching pattern already exists in this
//! example**, not invented for this scaffold: `steam_generator`'s counter-flow
//! exchanger resolves an 8-node array with the same kind of per-node
//! coefficient-then-march structure this bed would need, just co-current
//! instead of counter-flow and without the tube-metal third array. A real
//! implementation of this tier should read that module before designing its
//! own march.
//!
//! ## Why this is a placeholder today
//!
//! Building the seven-node balance, its own V&V (an axial temperature
//! profile to check against, at minimum the published 250/700 degC
//! inlet/outlet split), and its own tests is a distinct, sizeable piece of
//! work from wiring the *selection* of it into the plant -- which is what this
//! change delivers. Falling back to [`super::one_node`]'s
//! [`PebbleBedPorousMediaNode`] (the more physically complete of the two real
//! tiers, and the plant's own default since 2026-08-17) keeps
//! [`ReactorModelKind::AxialSevenNode`](super::ReactorModelKind::AxialSevenNode)
//! selectable and exercised by the enum's own tests now, without shipping an
//! unvalidated seven-node balance ahead of it being real.

use super::one_node::PebbleBedPorousMediaNode;
use uom::si::f64::{MassRate, Power, ThermodynamicTemperature, Time};
use uom::si::power::watt;

/// Placeholder seven-node axial core. Currently a thin wrapper around
/// [`PebbleBedPorousMediaNode`] -- every method below delegates to it
/// unmodified, so selecting this tier changes nothing about the plant's
/// numbers yet. See the module doc comment for the design a real
/// implementation would replace this with.
#[derive(Clone, Copy, Debug)]
pub struct AxialSevenNodeCore {
    /// The one-node fallback this placeholder currently *is*. Not a `[7]`
    /// array of nodes -- that is exactly what implementing this tier for real
    /// would replace this field with.
    fallback: PebbleBedPorousMediaNode,
}

impl AxialSevenNodeCore {
    /// Construct at the same cold-start seed [`PebbleBedPorousMediaNode::new`] uses.
    pub fn new() -> Self {
        Self {
            fallback: PebbleBedPorousMediaNode::new(),
        }
    }

    /// Delegates to [`PebbleBedPorousMediaNode::step`] -- see the module doc
    /// comment for what a real seven-node march would do instead.
    ///
    /// `fission_power` here is the caller-summed reactor thermal power (see
    /// [`super::ReactorModel::step`]'s doc comment); it is passed through as
    /// [`PebbleBedPorousMediaNode::step`]'s `fission_power` argument with a
    /// zero `decay_heat_power`, the same fold-in [`super::ReactorModel::step`]
    /// performs for its own `OneNodePorousMedia` arm.
    pub fn step(
        &mut self,
        dt: Time,
        fission_power: Power,
        helium_inlet_temperature: ThermodynamicTemperature,
        helium_mass_flow: MassRate,
    ) -> Power {
        self.fallback.step(
            dt,
            fission_power,
            Power::new::<watt>(0.0),
            helium_inlet_temperature,
            helium_mass_flow,
        )
    }

    /// Delegates to [`PebbleBedPorousMediaNode::pebble_temperature`]. A real
    /// implementation would need to decide what this reports -- a bed
    /// average across all seven nodes, or the outlet-end (hottest) node --
    /// since callers currently treat it as the bed average
    /// [`super::one_node`] provides.
    pub fn temperature(&self) -> ThermodynamicTemperature {
        self.fallback.pebble_temperature()
    }

    /// Delegates to [`PebbleBedPorousMediaNode::helium_temperature`] -- in a
    /// real implementation this becomes the last (outlet) node's helium
    /// temperature.
    pub fn helium_outlet_temperature(&self) -> ThermodynamicTemperature {
        self.fallback.helium_temperature()
    }
}

impl Default for AxialSevenNodeCore {
    fn default() -> Self {
        Self::new()
    }
}
