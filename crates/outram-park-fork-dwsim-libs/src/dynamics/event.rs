//! Scheduled events — a timed change to one flowsheet property, optionally
//! ramped rather than stepped.
//!
//! # Attribution
//!
//! Pure-Rust port of **DWSIM** `DWSIM.DynamicsManager/Event.vb` (whole file,
//! lines 22-61) and the three dynamics enums in
//! `DWSIM.Interfaces/Enums.vb:27-51`, upstream commit
//! `1abf72d1b6b41d3e9a8cc770d3cc4e8fc76e5766` (branch `windows`), GPL-3.0.
//! Upstream copyright: 2020 Daniel Wagner O. de Medeiros and the DWSIM
//! contributors. This port is GPL-3.0-only. Independent OUTRAM PARK fork, **not**
//! the official DWSIM software.
//!
//! # What an event is
//!
//! An event says: *at simulated time `t`, set property `P` of object `O` to
//! value `v`*. Upstream's run loop fires an event when the simulated clock first
//! crosses its timestamp (FormDynamicsIntegratorControl.vb:164), and — when the
//! transition is not a step change — the manager additionally *ramps* the
//! property towards `v` on every step leading up to `t`
//! (Manager.vb:211-343, ported as
//! [`crate::dynamics::manager::DynamicsManager::property_values_from_events`]).
//!
//! # Excluded DWSIM behavior
//!
//! - **XML serialization** — `SaveData` / `LoadData` (Event.vb:52-59), which
//!   round-trip the object through `XMLSerializer`. This port keeps the data
//!   model; persistence is out of scope for the crate (see the
//!   [`crate::flowsheet`] module header, which excludes the same layer).
//! - **Script events.** [`DynamicsEventType::RunScript`] is ported as a *value*
//!   because upstream stores it and the run loop matches on it, but its effect
//!   is a no-op: upstream's `Case Dynamics.DynamicsEventType.RunScript` arm at
//!   FormDynamicsIntegratorControl.vb:81 is **empty** — DWSIM never actually
//!   runs the script from the integrator loop, it only stores the `ScriptID`.
//!   The IronPython scripting host is excluded crate-wide.

use crate::dynamics::property::{DynamicProperty, PropertyRef};
use crate::dynamics::sim_time::SimInstant;
use crate::flowsheet::objects::ObjectId;

/// What kind of action an event performs — DWSIM's
/// `Enums.Dynamics.DynamicsEventType` (Enums.vb:28-31).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum DynamicsEventType {
    /// Set a flowsheet property to a value (upstream value `0`, the default;
    /// Event.vb:32).
    #[default]
    ChangeProperty,
    /// Run a named script (upstream value `1`). **Stored but never executed** —
    /// see the module header.
    RunScript,
}

/// How the property gets from its current value to the event's target —
/// DWSIM's `Enums.Dynamics.DynamicsEventTransitionType` (Enums.vb:40-46).
///
/// Note the upstream discriminants skip `2`: `StepChange = 0`,
/// `LinearChange = 1`, `LogChange = 3`, `InverseLogChange = 4`,
/// `RandomChange = 5`. The gap is preserved in
/// [`DynamicsEventTransitionType::upstream_value`] in case a reader is comparing
/// against a DWSIM XML file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum DynamicsEventTransitionType {
    /// The value jumps to the target when the event fires (upstream `0`, the
    /// default; Event.vb:46). A step change is **excluded** from the ramping
    /// path — `Manager.vb:225` skips it entirely.
    #[default]
    StepChange,
    /// Linear ramp from the reference state's value to the target (upstream
    /// `1`; Manager.vb:312-314).
    LinearChange,
    /// Ramp that is linear in `ln(value)` — geometric (upstream `3`;
    /// Manager.vb:316-318).
    LogChange,
    /// A log ramp mirrored about the target (upstream `4`;
    /// Manager.vb:320-323).
    InverseLogChange,
    /// A uniformly random value between the reference and the target (upstream
    /// `5`; Manager.vb:325-327).
    RandomChange,
}

impl DynamicsEventTransitionType {
    /// The integer this variant is stored as in a DWSIM file (Enums.vb:40-46).
    #[must_use]
    pub fn upstream_value(self) -> i32 {
        match self {
            DynamicsEventTransitionType::StepChange => 0,
            DynamicsEventTransitionType::LinearChange => 1,
            DynamicsEventTransitionType::LogChange => 3,
            DynamicsEventTransitionType::InverseLogChange => 4,
            DynamicsEventTransitionType::RandomChange => 5,
        }
    }
}

/// Which historian state a ramp measures its *starting* value from — DWSIM's
/// `Enums.Dynamics.DynamicsEventTransitionReferenceType` (Enums.vb:48-51),
/// consumed at Manager.vb:239-279.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum DynamicsEventTransitionReferenceType {
    /// The oldest state in the historian — the run's initial condition
    /// (upstream `0`; Manager.vb:241-245).
    InitialState,
    /// The event immediately before this one in timestamp order, or the initial
    /// state if this is the first event (upstream `1`, the default;
    /// Event.vb:48, Manager.vb:247-263).
    #[default]
    PreviousEvent,
    /// A specific other event, named by
    /// [`DynamicEvent::transition_reference_event_id`] (upstream `2`;
    /// Manager.vb:265-277).
    SpecificEvent,
}

/// One scheduled change to the flowsheet — the port of upstream's
/// `DynamicEvent` class (Event.vb:22-61).
///
/// Units: [`DynamicEvent::property_value`] is expressed in
/// [`DynamicEvent::property_units`], **not** in SI, exactly as upstream stores
/// it (Event.vb:38-40 keeps `SimulationObjectPropertyValue` as a string
/// alongside `SimulationObjectPropertyUnits`). Use
/// [`DynamicEvent::value_in_internal_units`] to get the value DWSIM would write
/// onto the flowsheet.
#[derive(Debug, Clone, PartialEq)]
pub struct DynamicEvent {
    /// Unique identifier within the event set (Event.vb:26). Also the key the
    /// set stores it under and the target of
    /// [`DynamicEvent::transition_reference_event_id`].
    pub id: String,
    /// Human-readable label (Event.vb:28).
    pub description: String,
    /// The simulated time the event fires at (Event.vb:30, `DateTime.MinValue`
    /// by default — i.e. [`SimInstant::ZERO`]).
    pub timestamp: SimInstant,
    /// Change a property, or run a script (Event.vb:32).
    pub event_type: DynamicsEventType,
    /// Which object and property to change (Event.vb:34-36, plus the compound
    /// index this port breaks out of the property string).
    pub target: PropertyRef,
    /// The target value, **in [`DynamicEvent::property_units`]**
    /// (Event.vb:38 — upstream stores this as an invariant-culture string).
    pub property_value: f64,
    /// The unit string `property_value` is written in (Event.vb:40). An empty
    /// string, or any unit this port does not recognise, means the value is
    /// already in DWSIM-internal units — see
    /// [`crate::dynamics::property::convert_to_internal`].
    pub property_units: String,
    /// The script to run for a [`DynamicsEventType::RunScript`] event
    /// (Event.vb:42). Stored only; never executed (see the module header).
    pub script_id: String,
    /// Whether the event participates in the run (Event.vb:44, default `True`;
    /// tested at FormDynamicsIntegratorControl.vb:75).
    pub enabled: bool,
    /// Step or ramp (Event.vb:46).
    pub transition_type: DynamicsEventTransitionType,
    /// Where a ramp starts from (Event.vb:48).
    pub transition_reference: DynamicsEventTransitionReferenceType,
    /// The event a [`DynamicsEventTransitionReferenceType::SpecificEvent`] ramp
    /// refers to (Event.vb:50).
    pub transition_reference_event_id: String,
}

impl DynamicEvent {
    /// A step-change event that sets `property` of `object` to `value` (in
    /// `units`) at simulated time `timestamp`.
    ///
    /// The defaults match upstream's field initialisers (Event.vb:26-50):
    /// enabled, [`DynamicsEventType::ChangeProperty`],
    /// [`DynamicsEventTransitionType::StepChange`],
    /// [`DynamicsEventTransitionReferenceType::PreviousEvent`].
    #[must_use]
    pub fn change_property(
        id: impl Into<String>,
        timestamp: SimInstant,
        object: ObjectId,
        property: DynamicProperty,
        value: f64,
        units: impl Into<String>,
    ) -> Self {
        DynamicEvent {
            id: id.into(),
            description: String::new(),
            timestamp,
            event_type: DynamicsEventType::ChangeProperty,
            target: PropertyRef::new(object, property),
            property_value: value,
            property_units: units.into(),
            script_id: String::new(),
            enabled: true,
            transition_type: DynamicsEventTransitionType::StepChange,
            transition_reference: DynamicsEventTransitionReferenceType::PreviousEvent,
            transition_reference_event_id: String::new(),
        }
    }

    /// The target value converted into DWSIM-internal units — upstream's
    /// `Converter.ConvertToSI(ev.SimulationObjectPropertyUnits, ev.SimulationObjectPropertyValue)`
    /// (FormDynamicsIntegratorControl.vb:79, Manager.vb:231).
    #[must_use]
    pub fn value_in_internal_units(&self) -> f64 {
        crate::dynamics::property::convert_to_internal(&self.property_units, self.property_value)
    }

    /// Builder-style setter for the ramp type and its reference, mirroring the
    /// three transition fields upstream exposes in its event editor.
    #[must_use]
    pub fn with_transition(
        mut self,
        transition_type: DynamicsEventTransitionType,
        reference: DynamicsEventTransitionReferenceType,
    ) -> Self {
        self.transition_type = transition_type;
        self.transition_reference = reference;
        self
    }
}

impl Default for DynamicEvent {
    /// The upstream field initialisers verbatim (Event.vb:26-50): empty strings,
    /// timestamp at [`SimInstant::ZERO`] (`DateTime.MinValue`), enabled,
    /// change-property, step change, previous-event reference.
    fn default() -> Self {
        DynamicEvent {
            id: String::new(),
            description: String::new(),
            timestamp: SimInstant::ZERO,
            event_type: DynamicsEventType::ChangeProperty,
            target: PropertyRef::new(ObjectId(String::new()), DynamicProperty::Temperature),
            property_value: 0.0,
            property_units: String::new(),
            script_id: String::new(),
            enabled: true,
            transition_type: DynamicsEventTransitionType::StepChange,
            transition_reference: DynamicsEventTransitionReferenceType::PreviousEvent,
            transition_reference_event_id: String::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_the_upstream_field_initialisers() {
        let e = DynamicEvent::default();
        assert_eq!(e.timestamp, SimInstant::ZERO);
        assert!(e.enabled);
        assert_eq!(e.event_type, DynamicsEventType::ChangeProperty);
        assert_eq!(e.transition_type, DynamicsEventTransitionType::StepChange);
        assert_eq!(
            e.transition_reference,
            DynamicsEventTransitionReferenceType::PreviousEvent
        );
    }

    #[test]
    fn transition_discriminants_preserve_the_upstream_gap_at_two() {
        assert_eq!(DynamicsEventTransitionType::StepChange.upstream_value(), 0);
        assert_eq!(
            DynamicsEventTransitionType::LinearChange.upstream_value(),
            1
        );
        assert_eq!(DynamicsEventTransitionType::LogChange.upstream_value(), 3);
        assert_eq!(
            DynamicsEventTransitionType::InverseLogChange.upstream_value(),
            4
        );
        assert_eq!(
            DynamicsEventTransitionType::RandomChange.upstream_value(),
            5
        );
    }

    #[test]
    fn value_is_converted_out_of_its_display_units() {
        let e = DynamicEvent::change_property(
            "ev-1",
            SimInstant::from_seconds(10.0),
            ObjectId::from("S-1"),
            DynamicProperty::Temperature,
            25.0,
            "C",
        );
        assert!((e.value_in_internal_units() - 298.15).abs() < 1e-9);
        // The stored value keeps the user's units, as upstream does.
        assert!((e.property_value - 25.0).abs() < 1e-12);
        assert_eq!(e.property_units, "C");
    }

    #[test]
    fn with_transition_sets_both_fields() {
        let e = DynamicEvent::default().with_transition(
            DynamicsEventTransitionType::LinearChange,
            DynamicsEventTransitionReferenceType::InitialState,
        );
        assert_eq!(e.transition_type, DynamicsEventTransitionType::LinearChange);
        assert_eq!(
            e.transition_reference,
            DynamicsEventTransitionReferenceType::InitialState
        );
    }
}
