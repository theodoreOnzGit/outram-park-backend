//! What CIET v2 supplies to the shared OPC-UA layer.
//!
//! [`opcua_core`](crate::opcua_core) serves any OUTRAM PARK digital twin; this
//! module is CIET's half of that contract, and nothing here is transport code:
//!
//! | Supplied | Item | Meaning |
//! |---|---|---|
//! | who CIET is | [`CietOpcuaSimulator`] / [`CIET_OPCUA_PROFILE`] | namespace URI, endpoint path, PKI directory name, mDNS marker, ... |
//! | what CIET publishes | [`CietNode`] | one variant per OPC-UA variable, wrapping the three [`super::node_map`] enums |
//!
//! [`CietNode`] is where the node map meets the wire: it says how each variable
//! is read out of a [`CietState`] snapshot and how a client's write is recorded
//! in a [`CietUserControls`](super::user_controls::CietUserControls) mailbox.
//! Adding a variable is still a matter of adding one variant to the node map —
//! the compiler then points at every `match` arm here that must be updated.
//!
//! ## Units
//!
//! Values cross into OPC-UA as bare `Variant`s, so the engineering unit travels
//! in the variable's *description* text (see [`CietNode::description`]), which
//! is what a client displays next to the number. The units themselves are
//! defined once, in [`super::node_map`].
//!
//! ## Scope (`RESPONSIBLE_USE.md`)
//!
//! CIET v2 is an **offline educational simulator**. This interface must never be
//! connected to live operational systems, plant systems, safety-critical
//! infrastructure, real-time plant monitoring, or institutional production
//! systems.

use opcua::nodes::AccessLevel;
use opcua::types::{DataTypeId, DataValue, StatusCode, Variant};

use crate::opcua_core::simulator::{
    variant_as_f64, OpcuaFolder, OpcuaSimulator, OpcuaSimulatorProfile, OpcuaVariable,
};

use super::discovery::{CIET_MDNS_INSTANCE_PREFIX, CIET_PRODUCT_TXT_VALUE};
use super::node_map::{
    CietControl, CietSignal, CietSwitch, CIET_NAMESPACE_URI, CONTROLS_FOLDER_NAME,
    DEFAULT_OPCUA_PORT, ENDPOINT_PATH, OUTPUTS_FOLDER_NAME,
};
use super::pki_paths::CIET_V2_PKI_DIR_NAME;
use super::state::CietState;
use super::user_controls::{CietUserControls, SharedUserControls};

/// Base OPC-UA application URI. The port is appended per instance.
///
/// **This must never equal [`CIET_NAMESPACE_URI`].** `async-opcua`'s diagnostics
/// node manager registers the application URI as *its own* namespace and claims
/// every node at that index (`owns_node` is `id.namespace == self.namespace_index`).
/// Identical strings resolve to one index, so the diagnostics manager would
/// shadow the whole CIET namespace and every CIET read would return
/// `BadNodeIdUnknown` despite the nodes being present and browsable. Keeping them
/// distinct is what makes the server's own namespace index 1 and CIET's index 2,
/// as [`super::node_map`] documents.
pub const CIET_APPLICATION_URI: &str = "urn:outram-park:ciet-educational-simulator-v2:server";

/// Default human-facing OPC-UA application name.
pub const CIET_DEFAULT_APPLICATION_NAME: &str = "CIET Educational Simulator v2";

/// Node id (string identifier) of the folder holding the read-only outputs.
pub const CIET_OUTPUTS_FOLDER_NODE_ID: &str = "CIET.Outputs";

/// Node id (string identifier) of the folder holding the writable controls.
pub const CIET_CONTROLS_FOLDER_NODE_ID: &str = "CIET.Controls";

/// Name given to the CIET node manager inside `async-opcua`.
pub const CIET_NODE_MANAGER_NAME: &str = "ciet";

/// Prefix the shared layer stamps on CIET's console lines, giving
/// `"CIET v2 OPC-UA: ..."` and `"CIET v2 mDNS: ..."`.
pub const CIET_LOG_PREFIX: &str = "CIET v2";

/// Every naming and identity string CIET v2's OPC-UA interface is built from.
///
/// This is the one place those strings are collected; the shared layer reads
/// them and never hard-codes anything CIET-shaped. None of them is a physical
/// quantity, so none carries a unit.
pub const CIET_OPCUA_PROFILE: OpcuaSimulatorProfile = OpcuaSimulatorProfile {
    namespace_uri: CIET_NAMESPACE_URI,
    application_uri: CIET_APPLICATION_URI,
    endpoint_path: ENDPOINT_PATH,
    default_application_name: CIET_DEFAULT_APPLICATION_NAME,
    default_port: DEFAULT_OPCUA_PORT,
    node_manager_name: CIET_NODE_MANAGER_NAME,
    outputs_folder_name: OUTPUTS_FOLDER_NAME,
    outputs_folder_node_id: CIET_OUTPUTS_FOLDER_NODE_ID,
    controls_folder_name: CONTROLS_FOLDER_NAME,
    controls_folder_node_id: CIET_CONTROLS_FOLDER_NODE_ID,
    pki_dir_name: CIET_V2_PKI_DIR_NAME,
    mdns_instance_prefix: CIET_MDNS_INSTANCE_PREFIX,
    mdns_product_marker: CIET_PRODUCT_TXT_VALUE,
    log_prefix: CIET_LOG_PREFIX,
};

/// The CIET Educational Simulator v2, as the shared OPC-UA layer sees it.
///
/// A zero-sized marker: it carries no data and costs nothing at runtime. It
/// exists so shared types can be bound to CIET at compile time — that is what
/// makes [`super::discovery::SimulatorBrowser`] find CIET simulators and nothing
/// else.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CietOpcuaSimulator;

impl OpcuaSimulator for CietOpcuaSimulator {
    const PROFILE: OpcuaSimulatorProfile = CIET_OPCUA_PROFILE;
}

/// One CIET variable, tagged by which node-map enum it came from.
///
/// Enum dispatch rather than a trait object, per the workspace Rust design
/// rules: a fourth kind of variable becomes a compile error at every `match`
/// instead of a runtime surprise.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CietNode {
    /// A read-only output, served as an OPC-UA `Double`.
    Signal(CietSignal),
    /// A writable continuous control, served as an OPC-UA `Double`.
    Control(CietControl),
    /// A writable on/off control, served as an OPC-UA `Boolean`.
    Switch(CietSwitch),
}

impl OpcuaVariable for CietNode {
    type Simulator = CietOpcuaSimulator;
    type Snapshot = CietState;
    type Requests = CietUserControls;

    /// Every CIET variable, outputs first, in address-space order.
    fn all() -> Vec<Self> {
        CietSignal::ALL
            .iter()
            .map(|s| Self::Signal(*s))
            .chain(CietControl::ALL.iter().map(|c| Self::Control(*c)))
            .chain(CietSwitch::ALL.iter().map(|s| Self::Switch(*s)))
            .collect()
    }

    /// The string part of this variable's `NodeId`.
    fn node_identifier(&self) -> &'static str {
        match self {
            Self::Signal(s) => s.node_identifier(),
            Self::Control(c) => c.node_identifier(),
            Self::Switch(s) => s.node_identifier(),
        }
    }

    /// Short OPC-UA browse name (one path segment).
    fn browse_name(&self) -> &'static str {
        match self {
            Self::Signal(s) => s.browse_name(),
            Self::Control(c) => c.browse_name(),
            Self::Switch(s) => s.browse_name(),
        }
    }

    /// Human-facing label.
    fn display_name(&self) -> &'static str {
        match self {
            Self::Signal(s) => s.display_name(),
            Self::Control(c) => c.display_name(),
            Self::Switch(s) => s.display_name(),
        }
    }

    /// Description shown by a client, naming the engineering unit and, for a
    /// control, the envelope writes are clamped to.
    fn description(&self) -> String {
        match self {
            Self::Signal(s) => format!(
                "{} [{}]. Read-only CIET output.",
                s.display_name(),
                s.unit()
            ),
            Self::Control(c) => {
                let (min, max) = c.valid_range();
                let unit = c.unit();
                format!(
                    "{} [{unit}]. Writable; requests are clamped to [{min}, {max}] {unit} \
                     and NaN is ignored when the simulator applies them.",
                    c.display_name()
                )
            }
            Self::Switch(s) => {
                format!(
                    "{} [on/off]. Writable boolean CIET control.",
                    s.display_name()
                )
            }
        }
    }

    /// OPC-UA data type: `Double` for the continuous variables, `Boolean` for
    /// the switches.
    fn data_type(&self) -> DataTypeId {
        match self {
            Self::Signal(_) | Self::Control(_) => DataTypeId::Double,
            Self::Switch(_) => DataTypeId::Boolean,
        }
    }

    /// Access level: outputs are read-only, controls and switches are writable.
    fn access_level(&self) -> AccessLevel {
        match self {
            Self::Signal(_) => AccessLevel::CURRENT_READ,
            Self::Control(_) | Self::Switch(_) => {
                AccessLevel::CURRENT_READ | AccessLevel::CURRENT_WRITE
            }
        }
    }

    /// Outputs are filed under the outputs folder, controls and switches under
    /// the controls folder.
    fn folder(&self) -> OpcuaFolder {
        match self {
            Self::Signal(_) => OpcuaFolder::Outputs,
            Self::Control(_) | Self::Switch(_) => OpcuaFolder::Controls,
        }
    }

    /// Read this variable out of a plant-state snapshot.
    ///
    /// Signals and controls are `f64` in the unit named by the node map;
    /// switches are booleans.
    fn read(&self, state: &CietState) -> Variant {
        match self {
            Self::Signal(signal) => Variant::from(signal.read(state)),
            Self::Control(control) => Variant::from(control.read(state)),
            Self::Switch(switch) => Variant::from(switch.read(state)),
        }
    }

    /// Record a client's write as a pending request, never applying it here.
    ///
    /// See [`record_control_request`] and [`record_switch_request`] for the
    /// deferred-apply contract. A signal is read-only, so the shared layer
    /// registers no write callback for one and this arm is unreachable in
    /// practice; it returns `BadNotWritable` rather than silently succeeding.
    fn record_write(&self, requests: &mut CietUserControls, value: DataValue) -> StatusCode {
        match self {
            Self::Signal(_) => StatusCode::BadNotWritable,
            Self::Control(control) => record_control_request(requests, *control, value),
            Self::Switch(switch) => record_switch_request(requests, *switch, value),
        }
    }
}

/// Record a client's write to a continuous control as a *pending request*.
///
/// The write is **not** applied to [`CietState`] here; it is parked in
/// [`CietUserControls`] and applied by the physics thread on its next timestep,
/// which is where clamping and NaN rejection happen (see
/// [`super::user_controls`] for why). The client therefore reads back the
/// *effective* value: write 1000 kW, read back the 15 kW ceiling.
///
/// `value` carries the control's own unit (see [`CietControl::unit`]).
///
/// Returns `BadTypeMismatch` for a non-numeric payload and `BadNothingToDo` for
/// no payload.
pub fn record_control_request(
    requests: &mut CietUserControls,
    control: CietControl,
    value: DataValue,
) -> StatusCode {
    let Some(variant) = value.value else {
        return StatusCode::BadNothingToDo;
    };
    let Some(number) = variant_as_f64(&variant) else {
        return StatusCode::BadTypeMismatch;
    };
    requests.request_control(control, number);
    StatusCode::Good
}

/// Record a client's write to an on/off control as a *pending request*.
///
/// Same deferred-apply contract as [`record_control_request`]. Strictly typed:
/// only an OPC-UA `Boolean` is accepted, because a switch has no meaningful
/// numeric interpretation and silently treating `0.0` as `false` would hide a
/// client bug.
pub fn record_switch_request(
    requests: &mut CietUserControls,
    switch: CietSwitch,
    value: DataValue,
) -> StatusCode {
    let Some(variant) = value.value else {
        return StatusCode::BadNothingToDo;
    };
    let Variant::Boolean(flag) = variant else {
        return StatusCode::BadTypeMismatch;
    };
    requests.request_switch(switch, flag);
    StatusCode::Good
}

/// Take the request mailbox's write lock and record a control request through
/// it, reporting `BadInternalError` if the lock is poisoned.
///
/// A convenience for callers holding a [`SharedUserControls`] rather than the
/// mailbox itself; the shared layer's write callback does the same thing.
pub fn record_control_request_shared(
    user_controls: &SharedUserControls,
    control: CietControl,
    value: DataValue,
) -> StatusCode {
    let Ok(mut requests) = user_controls.write() else {
        return StatusCode::BadInternalError;
    };
    record_control_request(&mut requests, control, value)
}

/// Take the request mailbox's write lock and record a switch request through
/// it, reporting `BadInternalError` if the lock is poisoned.
pub fn record_switch_request_shared(
    user_controls: &SharedUserControls,
    switch: CietSwitch,
    value: DataValue,
) -> StatusCode {
    let Ok(mut requests) = user_controls.write() else {
        return StatusCode::BadInternalError;
    };
    record_switch_request(&mut requests, switch, value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::opcua_core::simulator::OpcuaSimulator;

    /// Verifies that the write path accepts the numeric `Variant`s real clients
    /// send, refuses everything else, and parks accepted writes as *pending
    /// requests* rather than touching plant state — the property the whole
    /// deferred-apply design rests on.
    ///
    /// **Methodology.** [`variant_as_f64`] over ten numeric and three
    /// non-numeric variants (exact value vs `None`). Then
    /// [`record_control_request_shared`] / [`record_switch_request_shared`]
    /// against a fresh request mailbox and a default [`CietState`], checking
    /// that an accepted write appears in `pending_control` while plant state
    /// stays **unchanged** until `apply_and_clear`, that a wrong-typed write
    /// returns `BadTypeMismatch` and records nothing, and that an out-of-range
    /// 1000 kW request applies as the 15 kW ceiling. No socket, no server, no
    /// network.
    ///
    /// **Results (2026-07-28, unchanged 2026-08-12 after the shared-layer
    /// extraction).** All ten numeric variants converted exactly
    /// (`Double(7.5)` → 7.5, `Float(2.5)` → 2.5, `Int32(-3)` → -3.0, …);
    /// `Boolean`, `String`, `Empty` → `None`. `Double(9000.0)` to
    /// `CtahPumpPressurePascals` → `Good`, pending 9000, plant state still 0 Pa
    /// until apply, then 9000 Pa. `Boolean(true)` to that control →
    /// `BadTypeMismatch`, nothing recorded. `Boolean(true)` to
    /// `CtahBranchBlocked` → `Good`, applied `true`; `Double(1.0)` →
    /// `BadTypeMismatch`. A 1000 kW heater request applied as exactly 15.0 kW.
    /// Interpretation: remote writes never touch the plant-state lock, type
    /// errors surface as a status code rather than a control that silently does
    /// nothing, and the safety envelope is enforced on apply.
    #[test]
    fn writes_are_recorded_as_requests_and_clamped_on_apply() {
        assert_eq!(variant_as_f64(&Variant::Double(7.5)), Some(7.5));
        assert_eq!(variant_as_f64(&Variant::Float(2.5)), Some(2.5));
        assert_eq!(variant_as_f64(&Variant::SByte(-3)), Some(-3.0));
        assert_eq!(variant_as_f64(&Variant::Byte(3)), Some(3.0));
        assert_eq!(variant_as_f64(&Variant::Int16(-300)), Some(-300.0));
        assert_eq!(variant_as_f64(&Variant::UInt16(300)), Some(300.0));
        assert_eq!(variant_as_f64(&Variant::Int32(-3)), Some(-3.0));
        assert_eq!(variant_as_f64(&Variant::UInt32(3)), Some(3.0));
        assert_eq!(variant_as_f64(&Variant::Int64(-3)), Some(-3.0));
        assert_eq!(variant_as_f64(&Variant::UInt64(3)), Some(3.0));

        assert_eq!(variant_as_f64(&Variant::Boolean(true)), None);
        assert_eq!(variant_as_f64(&Variant::from("8000")), None);
        assert_eq!(variant_as_f64(&Variant::Empty), None);

        let mut state = CietState::default();
        let requests = super::super::user_controls::new_shared_user_controls();

        // A well-typed write is accepted and parked, NOT applied.
        let status = record_control_request_shared(
            &requests,
            CietControl::CtahPumpPressurePascals,
            DataValue::new_now(9000.0f64),
        );
        assert_eq!(status, StatusCode::Good);
        assert_eq!(
            requests
                .read()
                .unwrap()
                .pending_control(CietControl::CtahPumpPressurePascals),
            Some(9000.0),
            "the write should be pending"
        );
        assert!(
            state.get_ctah_pump_pressure_f64().abs() < 1e-6,
            "plant state must be untouched before apply_and_clear, got {}",
            state.get_ctah_pump_pressure_f64()
        );

        // A wrong-typed write is refused and records nothing.
        let status = record_control_request_shared(
            &requests,
            CietControl::Bt41CtahOutletSetPointDegC,
            DataValue::new_now(true),
        );
        assert_eq!(status, StatusCode::BadTypeMismatch);
        assert_eq!(
            requests
                .read()
                .unwrap()
                .pending_control(CietControl::Bt41CtahOutletSetPointDegC),
            None,
            "a rejected write must not be recorded"
        );

        // Switches: Boolean accepted, Double refused.
        assert_eq!(
            record_switch_request_shared(
                &requests,
                CietSwitch::CtahBranchBlocked,
                DataValue::new_now(true)
            ),
            StatusCode::Good
        );
        assert_eq!(
            record_switch_request_shared(
                &requests,
                CietSwitch::CtahBranchBlocked,
                DataValue::new_now(1.0f64)
            ),
            StatusCode::BadTypeMismatch
        );

        // An out-of-range request is clamped when the physics thread applies it.
        assert_eq!(
            record_control_request_shared(
                &requests,
                CietControl::HeaterPowerKw,
                DataValue::new_now(1000.0f64)
            ),
            StatusCode::Good
        );

        requests.write().unwrap().apply_and_clear(&mut state);

        assert!(
            (state.get_ctah_pump_pressure_f64() - 9000.0).abs() < 1e-3,
            "pump pressure should apply as 9000 Pa, got {}",
            state.get_ctah_pump_pressure_f64()
        );
        assert!(state.is_ctah_branch_blocked, "switch should have applied");
        assert!(
            (state.heater_power_kilowatts - 15.0).abs() < 1e-9,
            "1000 kW should clamp to the 15 kW ceiling, got {}",
            state.heater_power_kilowatts
        );
        assert!(
            !requests.read().unwrap().has_pending_requests(),
            "apply_and_clear should drain the requests"
        );
    }

    /// Verifies that [`CietNode`] enumerates exactly the node map, files each
    /// variable in the right folder, and agrees with the node map on every
    /// name — the shared layer builds the whole address space from these
    /// answers, so a disagreement here is a wrong or missing OPC-UA node.
    ///
    /// **Methodology.** Compare `CietNode::all().len()` with
    /// `node_map::total_node_count()`; check each variant's folder
    /// ([`OpcuaFolder::Outputs`] for signals, [`OpcuaFolder::Controls`] for
    /// controls and switches), its access level (`CURRENT_WRITE` set for
    /// exactly the writable kinds, since that flag alone decides whether a write
    /// callback is registered), its data type (`Double`/`Boolean`), and that its
    /// node identifier and browse name match the node map's. No socket, no
    /// server.
    ///
    /// **Results (2026-08-12).** 36 variables enumerated, matching
    /// `total_node_count()` = 36 (21 signals, 8 controls, 7 switches); 21 filed
    /// under Outputs and 15 under Controls; `CURRENT_WRITE` set on exactly the
    /// 15 controls and switches; `Double` on the 29 continuous variables and
    /// `Boolean` on the 7 switches; all 36 identifiers and browse names matched
    /// the node map. Interpretation: the seam reproduces the address space the
    /// node map describes, with no variable lost in the wrapping.
    #[test]
    fn ciet_nodes_reproduce_the_node_map() {
        use super::super::node_map::total_node_count;

        let nodes = CietNode::all();
        assert_eq!(nodes.len(), total_node_count());

        let outputs = nodes
            .iter()
            .filter(|n| n.folder() == OpcuaFolder::Outputs)
            .count();
        assert_eq!(outputs, CietSignal::ALL.len(), "signals go under Outputs");
        assert_eq!(
            nodes.len() - outputs,
            CietControl::ALL.len() + CietSwitch::ALL.len(),
            "controls and switches go under Controls"
        );

        for node in &nodes {
            let writable = node.access_level().contains(AccessLevel::CURRENT_WRITE);
            let (expected_writable, expected_type, identifier, browse_name) = match node {
                CietNode::Signal(s) => (
                    false,
                    DataTypeId::Double,
                    s.node_identifier(),
                    s.browse_name(),
                ),
                CietNode::Control(c) => (
                    true,
                    DataTypeId::Double,
                    c.node_identifier(),
                    c.browse_name(),
                ),
                CietNode::Switch(s) => (
                    true,
                    DataTypeId::Boolean,
                    s.node_identifier(),
                    s.browse_name(),
                ),
            };

            assert_eq!(writable, expected_writable, "{node:?} writability");
            assert_eq!(node.data_type(), expected_type, "{node:?} data type");
            assert_eq!(node.node_identifier(), identifier, "{node:?} identifier");
            assert_eq!(node.browse_name(), browse_name, "{node:?} browse name");
        }
    }

    /// Verifies CIET's profile is internally consistent, in particular that the
    /// application URI differs from the namespace URI.
    ///
    /// **Methodology.** Check `CietOpcuaSimulator::PROFILE` for: application URI
    /// != namespace URI (identical strings make `async-opcua`'s diagnostics node
    /// manager shadow the whole CIET namespace, so every read returns
    /// `BadNodeIdUnknown`); an endpoint path starting with `/`; the two folder
    /// node identifiers being distinct and colliding with no variable's node
    /// identifier; and non-empty naming strings. No socket, no server.
    ///
    /// **Results (2026-08-12).** application URI
    /// `urn:outram-park:ciet-educational-simulator-v2:server` != namespace URI
    /// `urn:outram-park:ciet-educational-simulator-v2`; endpoint path `/ciet`;
    /// folder ids `CIET.Outputs` / `CIET.Controls`, neither used by any of the
    /// 36 variables; all naming strings non-empty. Interpretation: the profile
    /// avoids the namespace-shadowing trap that once made every CIET read fail,
    /// and its folder ids cannot collide with a variable during address-space
    /// insertion.
    #[test]
    fn the_ciet_profile_is_internally_consistent() {
        let profile = CietOpcuaSimulator::PROFILE;

        assert_ne!(
            profile.application_uri, profile.namespace_uri,
            "the application URI must differ from the namespace URI, or the \
             diagnostics node manager shadows the whole CIET namespace"
        );
        assert!(
            profile.endpoint_path.starts_with('/'),
            "endpoint path {} must start with /",
            profile.endpoint_path
        );
        assert_ne!(
            profile.outputs_folder_node_id, profile.controls_folder_node_id,
            "the two folders need distinct node ids"
        );

        for node in CietNode::all() {
            assert_ne!(node.node_identifier(), profile.outputs_folder_node_id);
            assert_ne!(node.node_identifier(), profile.controls_folder_node_id);
        }

        for name in [
            profile.namespace_uri,
            profile.application_uri,
            profile.default_application_name,
            profile.node_manager_name,
            profile.outputs_folder_name,
            profile.controls_folder_name,
            profile.pki_dir_name,
            profile.mdns_instance_prefix,
            profile.mdns_product_marker,
            profile.log_prefix,
        ] {
            assert!(!name.is_empty(), "a profile naming string is empty");
        }
    }
}
