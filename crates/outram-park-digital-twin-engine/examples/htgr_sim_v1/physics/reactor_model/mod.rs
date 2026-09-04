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
//! | [`ReactorModelKind::OneNodePorousMedia`] | 1 (whole bed), two temperatures | **Real** -- implicit 2x2 backward-Euler LTNE solid/fluid balance, and the tier `htgr_sim_v1` opens on | [`one_node`] |
//! | [`ReactorModelKind::AxialSevenNode`] | 7, stacked along flow | Placeholder, falls back to [`one_node`]'s `OneNodePorousMedia` math | [`axial_seven_node`] |
//! | [`ReactorModelKind::CoarseMeshGenFoam`] | GeN-Foam coarse mesh | Placeholder, falls back to [`one_node`]'s `OneNodePorousMedia` math | [`coarse_mesh_genfoam`] |
//!
//! **History: there used to be a fourth, simpler tier, `OneNode`.** It was an
//! effectiveness-NTU closed form that treated the helium as external (zero
//! fluid capacitance) -- migrated 2026-08-16 from the former
//! `physics::pebble_bed` module, and what the maintainer called "the NTU
//! method". It was removed on 2026-08-17 once `OneNodePorousMedia` -- a more
//! physically complete two-temperature (solid + fluid) implicit balance --
//! became the default: the maintainer asked for the old one-node code to be
//! deleted rather than kept alongside its replacement. Its derivation (why an
//! effectiveness-NTU exponential form is exact and bounded for a single
//! isothermal-wall node, and why an earlier arithmetic-mean version of it
//! produced a second-law violation above `NTU = 2`) is preserved in git
//! history, in `one_node.rs`'s own "History" note, and in the workspace root
//! `CLAUDE.md`'s "Human review caught what the tests did not" section --
//! nothing here still depends on it.
//!
//! **`OneNodePorousMedia` is not a placeholder.** It is one spatial control
//! volume over the whole bed, like the removed `OneNode` was, but the helium
//! gets its own thermal node and the coupled solid/fluid balance is solved
//! implicitly each step -- see [`one_node::PebbleBedPorousMediaNode`] for the
//! full derivation. It is untested against a reference beyond its own
//! energy-conservation checks, so [`ReactorModelKind::is_implemented`]
//! reports it as implemented (real, independent physics) without any claim
//! that it is validated.
//!
//! **`AxialSevenNode` and `CoarseMeshGenFoam` are placeholders that fall back
//! to `OneNodePorousMedia`'s own math.** Selecting either changes nothing
//! about the numbers this plant produces yet; only the module each would
//! eventually own, and the design each is scaffolded for, differ. See each
//! module's own doc comment for what a real implementation would need to
//! become non-trivial.
//!
//! ## Why the geometry stays in `one_node`
//!
//! The HTR-10 core geometry, the Wakao film correlation and the pebble
//! properties are properties of the *bed*, not of any one fidelity's thermal
//! solve -- `primary_loop`, `secondary_loop` and `kinetics` all read them
//! (`design()`, `bed_heat_capacity()`, `superficial_area()`, ...) regardless
//! of which [`ReactorModelKind`] is selected. Only the *thermal solve* --
//! [`one_node::PebbleBedPorousMediaNode::step`] and its eventual
//! `AxialSevenNode` / `CoarseMeshGenFoam` counterparts -- is fidelity-specific.
//! So this module re-exports `one_node`'s geometry surface under the
//! historical `pebble_bed` name at [`super`]
//! (`pub use reactor_model::one_node as pebble_bed;`) rather than duplicating
//! it per tier.
//!
//! ## Switching fidelity at runtime
//!
//! [`ReactorModel::new`] builds a fresh model for a given [`ReactorModelKind`].
//! There is no in-place fidelity conversion (e.g. seeding a 7-node profile
//! from a 1-node average) -- switching rebuilds from each variant's own
//! `new()`, the same cold-start seed every fresh plant uses. That is the
//! right default while the two placeholder tiers carry no independent state
//! to preserve; it is a design question worth revisiting once
//! `AxialSevenNode` is real and a mid-run switch needs to interpolate a
//! profile from a single bed-average temperature.

pub mod axial_seven_node;
pub mod coarse_mesh_genfoam;
pub mod htr10_rz_geometry;
pub mod one_node;

use axial_seven_node::AxialSevenNodeCore;
use coarse_mesh_genfoam::CoarseMeshGenFoamCore;
use one_node::PebbleBedPorousMediaNode;
use std::fmt;
use uom::si::f64::{MassRate, Power, ThermodynamicTemperature, Time};
use uom::si::power::watt;

/// Which pebble-bed fidelity tier is selected -- the `HeaterType`-shaped
/// marker for [`ReactorModel`].
///
/// `Copy` and carries no data of its own, exactly like
/// `ciet_opcua::state::HeaterType`: it exists to be compared, displayed, and
/// matched, and to be the thing a future `ComboBox` reads and writes. The
/// data actually lives in the [`ReactorModel`] variant it selects.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ReactorModelKind {
    /// One control volume over the whole bed, with the helium given its own
    /// thermal node (a two-temperature LTNE porous-media balance) rather
    /// than being treated as external. **Real** physics -- not a
    /// placeholder. See [`one_node::PebbleBedPorousMediaNode`]. **Default**:
    /// it is the more physically complete of this simulator's real tiers to
    /// date (solid AND fluid thermal inertia, rather than the fluid side
    /// treated as massless), so `htgr_sim_v1` opens on this tier.
    #[default]
    OneNodePorousMedia,
    /// Seven control volumes stacked along the flow direction, so the
    /// inlet-to-outlet gradient becomes a computed result instead of a
    /// whole-bed lump. **Placeholder** -- falls back to `OneNodePorousMedia`'s
    /// math. See [`axial_seven_node`] for the scaffolded design.
    AxialSevenNode,
    /// A GeN-Foam coarse mesh (`outram_foam_appbuilder_lib::genfoam`) coupled
    /// multi-region neutronics + thermal-hydraulics solve, stepped with the
    /// [`teh_o_prke::nordheim_fuchs::NordheimFuchsExactTimestepper`] rather
    /// than GeN-Foam's own point-kinetics coupling. **Placeholder** -- falls
    /// back to `OneNodePorousMedia`'s math. See [`coarse_mesh_genfoam`] for
    /// the scaffolded design and why it is not wired in yet.
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
            Self::OneNodePorousMedia => "One Node (implicit porous-media, 2 temperatures)",
            Self::AxialSevenNode => "Axial, 7 Nodes (placeholder -> one node)",
            Self::CoarseMeshGenFoam => "Coarse Mesh, GeN-Foam (placeholder -> one node)",
        }
    }

    /// Whether this tier has a real, independent thermal solve as of this
    /// writing. The two placeholder tiers currently reproduce
    /// `OneNodePorousMedia`'s numbers exactly, so a caller that wants to
    /// know "is this a different model" rather than "which enum variant is
    /// selected" should check this rather than matching on the kind
    /// directly.
    #[allow(dead_code)] // read by a future control-panel dropdown; see the module doc comment
    pub fn is_implemented(&self) -> bool {
        matches!(self, Self::OneNodePorousMedia)
    }
}

/// Selectable pebble-bed core, dispatched by [`ReactorModelKind`].
///
/// Enum dispatch, not a trait object, per the workspace design rules: the
/// three variants are matched exhaustively in every method below, so adding a
/// fourth fidelity tier is a compile error at every call site until it is
/// handled, not a silent runtime gap.
///
/// `Clone + Copy`, like the [`one_node::PebbleBedPorousMediaNode`] it
/// currently always wraps or falls back to -- `HtgrPlant::step_with_correctors`'s
/// outer corrector loop snapshots and restores the whole plant by value each
/// corrector (`let core_at_step_start = self.core;` / `self.core =
/// core_at_step_start;`), so every variant's state must stay cheap,
/// `Copy` data.
#[derive(Clone, Copy, Debug)]
pub enum ReactorModel {
    /// The real, two-temperature implicit porous-media bed. See
    /// [`one_node::PebbleBedPorousMediaNode`].
    OneNodePorousMedia(PebbleBedPorousMediaNode),
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
            ReactorModelKind::OneNodePorousMedia => {
                Self::OneNodePorousMedia(PebbleBedPorousMediaNode::new())
            }
            ReactorModelKind::AxialSevenNode => Self::AxialSevenNode(AxialSevenNodeCore::new()),
            ReactorModelKind::CoarseMeshGenFoam => {
                Self::CoarseMeshGenFoam(CoarseMeshGenFoamCore::new())
            }
        }
    }

    /// The fidelity tier currently selected.
    pub fn kind(&self) -> ReactorModelKind {
        match self {
            Self::OneNodePorousMedia(_) => ReactorModelKind::OneNodePorousMedia,
            Self::AxialSevenNode(_) => ReactorModelKind::AxialSevenNode,
            Self::CoarseMeshGenFoam(_) => ReactorModelKind::CoarseMeshGenFoam,
        }
    }

    /// Advance the selected model by `dt` and return the heat rate handed to
    /// the helium. Every tier -- real or placeholder -- answers the same
    /// question the primary loop asks: how much heat crossed the pebble
    /// surface this step, given the fission power and the core inlet state.
    ///
    /// `fission_power` here is, for every tier, already the CALLER-SUMMED
    /// reactor thermal power (fission plus fission-product decay heat) --
    /// see `mod.rs`'s "Pebble bed absorbs the core's THERMAL power" comment
    /// at the `HtgrPlant::step_with_correctors` call site, which passes
    /// `kinetics::Kinetics::core_thermal_power()`. Every variant wraps
    /// [`one_node::PebbleBedPorousMediaNode::step`] (directly, or via a
    /// placeholder's fallback), whose signature takes fission power and
    /// decay heat as SEPARATE arguments (see its doc comment); this method
    /// folds the already-summed value in as `fission_power` with a zero
    /// `decay_heat_power`, since there is no separate decay-heat quantity
    /// available at this layer to pass instead.
    /// [`one_node::tests::fission_power_and_decay_heat_power_sum_into_the_same_source_term`]
    /// establishes that the split does not change the result -- only the
    /// sum enters `PebbleBedPorousMediaNode`'s balance -- so this is exact,
    /// not an approximation.
    pub fn step(
        &mut self,
        dt: Time,
        fission_power: Power,
        helium_inlet_temperature: ThermodynamicTemperature,
        helium_mass_flow: MassRate,
    ) -> Power {
        match self {
            Self::OneNodePorousMedia(core) => core.step(
                dt,
                fission_power,
                Power::new::<watt>(0.0),
                helium_inlet_temperature,
                helium_mass_flow,
            ),
            Self::AxialSevenNode(core) => core.step(
                dt,
                fission_power,
                helium_inlet_temperature,
                helium_mass_flow,
            ),
            Self::CoarseMeshGenFoam(core) => core.step(
                dt,
                fission_power,
                helium_inlet_temperature,
                helium_mass_flow,
            ),
        }
    }

    /// Lumped/bed-average pebble temperature -- a bed average, never a peak
    /// fuel temperature, in every tier implemented so far.
    pub fn temperature(&self) -> ThermodynamicTemperature {
        match self {
            Self::OneNodePorousMedia(core) => core.pebble_temperature(),
            Self::AxialSevenNode(core) => core.temperature(),
            Self::CoarseMeshGenFoam(core) => core.temperature(),
        }
    }

    /// Helium temperature leaving the bed.
    ///
    /// For `OneNodePorousMedia` this is
    /// [`one_node::PebbleBedPorousMediaNode::helium_temperature`] -- the
    /// node's own (well-mixed) fluid temperature, which stands in for an
    /// "outlet" the same way [`one_node::PebbleBedPorousMediaNode`]'s CSTR
    /// assumption already treats it (see that struct's doc comment): this
    /// tier has no separate outlet state to report.
    pub fn helium_outlet_temperature(&self) -> ThermodynamicTemperature {
        match self {
            Self::OneNodePorousMedia(core) => core.helium_temperature(),
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
    /// `OneNodePorousMedia`'s numbers exactly (they fall back to the same
    /// [`one_node::PebbleBedPorousMediaNode`] math) -- so a fidelity switch
    /// today changes nothing about the plant's behaviour, only which module
    /// owns the eventual real implementation.
    ///
    /// Methodology: step a fresh instance of each kind once, with identical
    /// inputs, and compare the returned heat rate and both temperatures
    /// bit-for-bit (`f64` equality is fine here -- the placeholders wrap the
    /// exact same `PebbleBedPorousMediaNode` value, not a re-derivation).
    #[test]
    fn placeholder_tiers_reproduce_one_node_porous_media_exactly() {
        let dt = Time::new::<second>(0.1);
        let fission_power = Power::new::<watt>(1.0e7);
        let inlet = ThermodynamicTemperature::new::<kelvin>(523.15);
        let flow = MassRate::new::<kilogram_per_second>(4.3);

        let mut one_node = ReactorModel::new(ReactorModelKind::OneNodePorousMedia);
        let mut axial = ReactorModel::new(ReactorModelKind::AxialSevenNode);
        let mut genfoam = ReactorModel::new(ReactorModelKind::CoarseMeshGenFoam);

        let q_one = one_node.step(dt, fission_power, inlet, flow).get::<watt>();
        let q_axial = axial.step(dt, fission_power, inlet, flow).get::<watt>();
        let q_genfoam = genfoam.step(dt, fission_power, inlet, flow).get::<watt>();

        assert_eq!(
            q_one, q_axial,
            "AxialSevenNode placeholder diverged from OneNodePorousMedia"
        );
        assert_eq!(
            q_one, q_genfoam,
            "CoarseMeshGenFoam placeholder diverged from OneNodePorousMedia"
        );
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
            ReactorModelKind::OneNodePorousMedia,
            ReactorModelKind::AxialSevenNode,
            ReactorModelKind::CoarseMeshGenFoam,
        ] {
            assert_eq!(ReactorModel::new(kind).kind(), kind);
        }
    }
}
