//! Selectable pebble-bed core fidelity -- one enum, three tiers, matched
//! exhaustively (no trait objects, per the workspace `CLAUDE.md` "Rust design
//! rules": the set of reactor-model fidelities is closed and known at compile
//! time, so an enum is the right tool, not `Box<dyn Trait>`).
//!
//! ## The precedent this mirrors
//!
//! This is the same shape as `outram_park_digital_twin_engine::ciet_opcua::state::HeaterType`
//! in the CIET educational simulator: a small `Copy` marker enum with a
//! `Display` impl, read by both the physics thread and (eventually) a GUI
//! `ComboBox`, so a fidelity choice is one piece of shared state rather than a
//! GUI-local field the repaint loop has to keep synchronised by hand. See
//! `src/bin/ciet_educational_simulator_v2/.../online_calibration/mod.rs` for
//! the write-on-change-only `ComboBox` pattern to reuse when this gets wired
//! into `htgr_sim_v1`'s own control panel -- **not done in this change**, only
//! the enum and the module layout are. [`ReactorModelKind`] is the
//! `HeaterType`-shaped marker; [`ReactorModel`] is the data-carrying enum the
//! marker selects between.
//!
//! ## The three tiers
//!
//! | Variant | Nodes | Status | Lives in |
//! |---|---|---|---|
//! | [`ReactorModelKind::OneNode`] | 1 (whole bed) | **Real** -- the model this simulator has run since 2026-08-12 | [`one_node`] |
//! | [`ReactorModelKind::AxialSevenNode`] | 7, stacked along flow | Placeholder, falls back to [`one_node`] | [`axial_seven_node`] |
//! | [`ReactorModelKind::CoarseMeshGenFoam`] | GeN-Foam coarse mesh | Placeholder, falls back to [`one_node`] | [`coarse_mesh_genfoam`] |
//!
//! **`OneNode` is not a placeholder.** It is the effectiveness-NTU lumped bed
//! this plant has always modelled -- what the maintainer also calls "the NTU
//! method" -- migrated here unmodified from the former `physics::pebble_bed`
//! module. See [`one_node`] for its full nodalisation discussion, what is real
//! in it, and what is still illustrative.
//!
//! **`AxialSevenNode` and `CoarseMeshGenFoam` are placeholders that fall back
//! to `OneNode`'s own math.** Selecting either changes nothing about the
//! numbers this plant produces yet; only the module each would eventually own,
//! and the design each is scaffolded for, differ. See each module's own doc
//! comment for what a real implementation would need to become non-trivial.
//!
//! ## Why the geometry stays in `one_node`
//!
//! The HTR-10 core geometry, the Wakao film correlation and the pebble
//! properties are properties of the *bed*, not of any one fidelity's thermal
//! solve -- `primary_loop`, `secondary_loop` and `kinetics` all read them
//! (`design()`, `bed_heat_capacity()`, `superficial_area()`, ...) regardless
//! of which [`ReactorModelKind`] is selected. Only the *thermal solve* --
//! [`one_node::PebbleBedCore::step`] and its eventual `AxialSevenNode` /
//! `CoarseMeshGenFoam` counterparts -- is fidelity-specific. So this module
//! re-exports `one_node`'s geometry surface under the historical `pebble_bed`
//! name at [`super`] (`pub use reactor_model::one_node as pebble_bed;`) rather
//! than duplicating it per tier.
//!
//! ## Switching fidelity at runtime
//!
//! [`ReactorModel::new`] builds a fresh model for a given [`ReactorModelKind`].
//! There is no in-place fidelity conversion (e.g. seeding a 7-node profile
//! from a 1-node average) -- switching rebuilds from each variant's own
//! `new()`, the same cold-start seed every fresh plant uses. That is the
//! right default while two of the three variants carry no independent state
//! to preserve; it is a design question worth revisiting once
//! `AxialSevenNode` is real and a mid-run switch needs to interpolate a
//! profile from a single bed-average temperature.

pub mod axial_seven_node;
pub mod coarse_mesh_genfoam;
pub mod one_node;

use axial_seven_node::AxialSevenNodeCore;
use coarse_mesh_genfoam::CoarseMeshGenFoamCore;
use one_node::PebbleBedCore;
use std::fmt;
use uom::si::f64::{MassRate, Power, ThermodynamicTemperature, Time};

/// Which pebble-bed fidelity tier is selected -- the `HeaterType`-shaped
/// marker for [`ReactorModel`].
///
/// `Copy` and carries no data of its own, exactly like
/// `ciet_opcua::state::HeaterType`: it exists to be compared, displayed, and
/// matched, and to be the thing a future `ComboBox` reads and writes. The
/// data actually lives in the [`ReactorModel`] variant it selects.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ReactorModelKind {
    /// The lumped, effectiveness-NTU whole-bed model -- one control volume,
    /// no axial or radial profile. See [`one_node`]. Default: this is the
    /// only tier with a real implementation.
    #[default]
    OneNode,
    /// Seven control volumes stacked along the flow direction, so the
    /// inlet-to-outlet gradient becomes a computed result instead of a
    /// whole-bed lump. **Placeholder** -- falls back to `OneNode`'s math. See
    /// [`axial_seven_node`] for the scaffolded design.
    AxialSevenNode,
    /// A GeN-Foam coarse mesh (`outram_foam_appbuilder_lib::genfoam`) coupled
    /// multi-region neutronics + thermal-hydraulics solve, stepped with the
    /// [`teh_o_prke::nordheim_fuchs::NordheimFuchsExactTimestepper`] rather
    /// than GeN-Foam's own point-kinetics coupling. **Placeholder** -- falls
    /// back to `OneNode`'s math. See [`coarse_mesh_genfoam`] for the
    /// scaffolded design and why it is not wired in yet.
    CoarseMeshGenFoam,
}

impl fmt::Display for ReactorModelKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self, f)
    }
}

impl ReactorModelKind {
    /// Human-readable label for a future `ComboBox` entry, distinct from the
    /// bare `Debug`/`Display` form so the dropdown can say what the tier
    /// costs and whether it is real.
    #[allow(dead_code)] // read by a future control-panel dropdown; see the module doc comment
    pub fn menu_label(&self) -> &'static str {
        match self {
            Self::OneNode => "One Node (NTU method)",
            Self::AxialSevenNode => "Axial, 7 Nodes (placeholder -> one node)",
            Self::CoarseMeshGenFoam => "Coarse Mesh, GeN-Foam (placeholder -> one node)",
        }
    }

    /// Whether this tier has a real, independent thermal solve as of this
    /// writing. Both placeholder tiers currently reproduce [`OneNode`]'s
    /// numbers exactly, so a caller that wants to know "is this a different
    /// model" rather than "which enum variant is selected" should check this
    /// rather than matching on the kind directly.
    ///
    /// [`OneNode`]: Self::OneNode
    #[allow(dead_code)] // read by a future control-panel dropdown; see the module doc comment
    pub fn is_implemented(&self) -> bool {
        matches!(self, Self::OneNode)
    }
}

/// Selectable pebble-bed core, dispatched by [`ReactorModelKind`].
///
/// Enum dispatch, not a trait object, per the workspace design rules: the
/// three variants are matched exhaustively in every method below, so adding a
/// fourth fidelity tier is a compile error at every call site until it is
/// handled, not a silent runtime gap.
///
/// `Clone + Copy`, like the [`one_node::PebbleBedCore`] it currently always
/// wraps or falls back to -- `HtgrPlant::step_with_correctors`'s outer
/// corrector loop snapshots and restores the whole plant by value each
/// corrector (`let core_at_step_start = self.core;` / `self.core =
/// core_at_step_start;`), so every variant's state must stay cheap,
/// `Copy` data.
#[derive(Clone, Copy, Debug)]
pub enum ReactorModel {
    /// The real, lumped effectiveness-NTU bed. See [`one_node::PebbleBedCore`].
    OneNode(PebbleBedCore),
    /// Placeholder -- see [`axial_seven_node`].
    AxialSevenNode(AxialSevenNodeCore),
    /// Placeholder -- see [`coarse_mesh_genfoam`].
    CoarseMeshGenFoam(CoarseMeshGenFoamCore),
}

impl ReactorModel {
    /// Construct the requested fidelity tier at its own cold-start seed. See
    /// the module doc comment for why switching kind rebuilds rather than
    /// converts in place.
    pub fn new(kind: ReactorModelKind) -> Self {
        match kind {
            ReactorModelKind::OneNode => Self::OneNode(PebbleBedCore::new()),
            ReactorModelKind::AxialSevenNode => Self::AxialSevenNode(AxialSevenNodeCore::new()),
            ReactorModelKind::CoarseMeshGenFoam => {
                Self::CoarseMeshGenFoam(CoarseMeshGenFoamCore::new())
            }
        }
    }

    /// The fidelity tier currently selected.
    pub fn kind(&self) -> ReactorModelKind {
        match self {
            Self::OneNode(_) => ReactorModelKind::OneNode,
            Self::AxialSevenNode(_) => ReactorModelKind::AxialSevenNode,
            Self::CoarseMeshGenFoam(_) => ReactorModelKind::CoarseMeshGenFoam,
        }
    }

    /// Advance the selected model by `dt` and return the heat rate handed to
    /// the helium. Signature matches [`one_node::PebbleBedCore::step`]
    /// exactly, since every tier -- real or placeholder -- answers the same
    /// question the primary loop asks: how much heat crossed the pebble
    /// surface this step, given the fission power and the core inlet state.
    pub fn step(
        &mut self,
        dt: Time,
        fission_power: Power,
        helium_inlet_temperature: ThermodynamicTemperature,
        helium_mass_flow: MassRate,
    ) -> Power {
        match self {
            Self::OneNode(core) => {
                core.step(dt, fission_power, helium_inlet_temperature, helium_mass_flow)
            }
            Self::AxialSevenNode(core) => {
                core.step(dt, fission_power, helium_inlet_temperature, helium_mass_flow)
            }
            Self::CoarseMeshGenFoam(core) => {
                core.step(dt, fission_power, helium_inlet_temperature, helium_mass_flow)
            }
        }
    }

    /// Lumped/bed-average pebble temperature. See
    /// [`one_node::PebbleBedCore::temperature`] for what this does and does
    /// not represent -- a bed average, never a peak fuel temperature, in every
    /// tier implemented so far.
    pub fn temperature(&self) -> ThermodynamicTemperature {
        match self {
            Self::OneNode(core) => core.temperature(),
            Self::AxialSevenNode(core) => core.temperature(),
            Self::CoarseMeshGenFoam(core) => core.temperature(),
        }
    }

    /// Helium temperature leaving the bed. See
    /// [`one_node::PebbleBedCore::helium_outlet_temperature`].
    pub fn helium_outlet_temperature(&self) -> ThermodynamicTemperature {
        match self {
            Self::OneNode(core) => core.helium_outlet_temperature(),
            Self::AxialSevenNode(core) => core.helium_outlet_temperature(),
            Self::CoarseMeshGenFoam(core) => core.helium_outlet_temperature(),
        }
    }
}

impl Default for ReactorModel {
    fn default() -> Self {
        Self::new(ReactorModelKind::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uom::si::mass_rate::kilogram_per_second;
    use uom::si::power::watt;
    use uom::si::thermodynamic_temperature::kelvin;
    use uom::si::time::second;

    /// Every [`ReactorModelKind`] must be constructible and steppable through
    /// the enum, and the two placeholder tiers must currently reproduce
    /// `OneNode`'s numbers exactly (they fall back to the same
    /// [`one_node::PebbleBedCore`] math) -- so a fidelity switch today changes
    /// nothing about the plant's behaviour, only which module owns the
    /// eventual real implementation.
    ///
    /// Methodology: step a fresh instance of each kind once, with identical
    /// inputs, and compare the returned heat rate and both temperatures
    /// bit-for-bit (`f64` equality is fine here -- the placeholders wrap the
    /// exact same `PebbleBedCore` value, not a re-derivation).
    #[test]
    fn placeholder_tiers_reproduce_one_node_exactly() {
        let dt = Time::new::<second>(0.1);
        let fission_power = Power::new::<watt>(1.0e7);
        let inlet = ThermodynamicTemperature::new::<kelvin>(523.15);
        let flow = MassRate::new::<kilogram_per_second>(4.3);

        let mut one_node = ReactorModel::new(ReactorModelKind::OneNode);
        let mut axial = ReactorModel::new(ReactorModelKind::AxialSevenNode);
        let mut genfoam = ReactorModel::new(ReactorModelKind::CoarseMeshGenFoam);

        let q_one = one_node.step(dt, fission_power, inlet, flow).get::<watt>();
        let q_axial = axial.step(dt, fission_power, inlet, flow).get::<watt>();
        let q_genfoam = genfoam.step(dt, fission_power, inlet, flow).get::<watt>();

        assert_eq!(q_one, q_axial, "AxialSevenNode placeholder diverged from OneNode");
        assert_eq!(q_one, q_genfoam, "CoarseMeshGenFoam placeholder diverged from OneNode");
        assert_eq!(one_node.temperature(), axial.temperature());
        assert_eq!(one_node.temperature(), genfoam.temperature());
        assert_eq!(
            one_node.helium_outlet_temperature(),
            axial.helium_outlet_temperature()
        );
        assert_eq!(
            one_node.helium_outlet_temperature(),
            genfoam.helium_outlet_temperature()
        );
    }

    #[test]
    fn kind_round_trips_through_new_and_kind() {
        for kind in [
            ReactorModelKind::OneNode,
            ReactorModelKind::AxialSevenNode,
            ReactorModelKind::CoarseMeshGenFoam,
        ] {
            assert_eq!(ReactorModel::new(kind).kind(), kind);
        }
    }
}
