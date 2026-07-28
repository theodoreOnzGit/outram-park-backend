//! End-to-end verification of the CIET Educational Simulator v2 OPC-UA interface.
//!
//! This is the headless functional test of the interface the simulator exposes:
//! it starts the **real** OPC-UA server against a real shared [`CietState`],
//! connects with a **real** `async-opcua` client over a real TCP socket, and
//! exercises both directions of the contract. No GUI and no display are
//! involved, so it runs anywhere the crate builds — including on Termux.
//!
//! ## Why this test exists
//!
//! The OPC-UA layer is the one part of v2 that has no counterpart in v1, so it
//! has no validated predecessor to inherit confidence from. Everything else in
//! v2 is a port of physics the maintainer has already done validation work on;
//! this interface is new code, and the failure modes that matter (a client
//! silently reading a stale or wrong node, a write that is accepted but never
//! reaches the solver, an out-of-range write that corrupts the state) are all
//! invisible to a unit test of either side alone.
//!
//! ## What is deliberately NOT covered
//!
//! - **The physics.** No simulation is run here; the state is driven directly.
//!   Port equivalence between v1 and v2 physics is tracked separately.
//! - **mDNS discovery.** Announcing and browsing need a live multicast-capable
//!   network; a CI machine or container often has neither. Discovery is
//!   exercised by hand instead — see the v2 README.
//! - **Security.** There is none to test: the server is deliberately
//!   `SecurityPolicy::None` + anonymous. That is the documented design of a
//!   teaching demonstrator, not an oversight.
//!
//! ## Scope
//!
//! Per `RESPONSIBLE_USE.md`, the simulator and this interface are an **offline
//! educational demonstration** only, never to be connected to live operational
//! systems, plant systems, or safety-critical infrastructure.

use std::sync::Arc;
use std::time::Duration;

use outram_park_digital_twin_engine::ciet_opcua::node_map::{
    CietControl, CietSignal, CietSwitch, CIET_NAMESPACE_URI,
};
use outram_park_digital_twin_engine::ciet_opcua::server::{
    spawn_opcua_server_thread, OpcuaServerConfig,
};
use outram_park_digital_twin_engine::ciet_opcua::state::{new_shared_state, SharedCietState};
use outram_park_digital_twin_engine::ciet_opcua::user_controls::{
    new_shared_user_controls, SharedUserControls,
};

use opcua::client::{ClientBuilder, IdentityToken, Session};
use opcua::types::{
    AttributeId, DataValue, EndpointDescription, MessageSecurityMode, NodeId, ReadValueId,
    TimestampsToReturn, UserTokenPolicy, Variant, WriteValue,
};

/// Port used by this test.
///
/// Deliberately **not** 4840: a developer running the real simulator on the
/// same machine would otherwise make the test fail with `AddrInUse` for reasons
/// that have nothing to do with the code under test.
const TEST_PORT: u16 = 48431;

/// How long to wait for the server thread to bind and start accepting.
const SERVER_START_GRACE: Duration = Duration::from_millis(1500);

/// How long to allow the updater task to publish a state change into the
/// address space before a read is expected to observe it.
const PUBLISH_GRACE: Duration = Duration::from_millis(600);

/// Bring up the server on loopback with mDNS advertisement disabled.
fn start_test_server(
    state: SharedCietState,
    user_controls: SharedUserControls,
) -> OpcuaServerConfig {
    let config = OpcuaServerConfig {
        bind_address: "127.0.0.1".to_owned(),
        port: TEST_PORT,
        // No announcement: a test must not advertise a service on the
        // developer's network, and multicast may be unavailable anyway.
        advertise_over_mdns: false,
        application_name: "CIET Educational Simulator v2 (test)".to_owned(),
    };

    spawn_opcua_server_thread(state, user_controls, config.clone())
        .expect("OPC-UA server thread should start");

    std::thread::sleep(SERVER_START_GRACE);
    config
}

/// Stand in for the physics thread's once-per-timestep request drain.
///
/// Remote writes land in [`CietUserControls`], not in the plant state; the
/// physics thread applies them at the top of each timestep. This test runs no
/// physics, so it performs that step explicitly. Returns how many requests were
/// applied, which is itself worth asserting on — a write that produced no
/// pending request would otherwise look identical to a write that was applied.
fn pump_pending_requests(state: &SharedCietState, user_controls: &SharedUserControls) -> usize {
    let mut controls_guard = user_controls.write().expect("user controls lock");
    let mut state_guard = state.write().expect("state lock");
    controls_guard.apply_and_clear(&mut state_guard)
}

/// Connect an anonymous, unencrypted client session to the test server.
async fn connect_client() -> Arc<Session> {
    let endpoint_url = format!("opc.tcp://127.0.0.1:{TEST_PORT}/ciet");

    let mut client = ClientBuilder::new()
        .application_name("CIET v2 round-trip test client")
        .application_uri("urn:outram-park:ciet-v2-roundtrip-test")
        .create_sample_keypair(true)
        .trust_server_certs(true)
        .session_retry_limit(2)
        .pki_dir(std::env::temp_dir().join("ciet-v2-roundtrip-test-pki"))
        .client()
        .expect("client configuration should be valid");

    let endpoint: EndpointDescription = (
        endpoint_url.as_ref(),
        "None",
        MessageSecurityMode::None,
        UserTokenPolicy::anonymous(),
    )
        .into();

    let (session, event_loop) = client
        .connect_to_matching_endpoint(endpoint, IdentityToken::Anonymous)
        .await
        .expect("should connect to the CIET v2 endpoint");

    let handle = event_loop.spawn();
    session.wait_for_connection().await;
    // The event loop must outlive this function; leaking the handle is the
    // simplest way to keep it running for the lifetime of the test process.
    std::mem::forget(handle);

    session
}

/// Resolve the runtime namespace index for [`CIET_NAMESPACE_URI`].
///
/// The index is assigned by the server at start-up and is *not* guaranteed to
/// be 2, so both this test and the bundled demo client resolve it rather than
/// hard-coding it. Reads the standard `Server_NamespaceArray` node (`ns=0;i=2255`).
async fn resolve_namespace_index(session: &Session) -> u16 {
    let namespace_array_node = NodeId::new(0, 2255u32);
    let to_read = [ReadValueId {
        node_id: namespace_array_node,
        attribute_id: AttributeId::Value as u32,
        ..Default::default()
    }];

    let results = session
        .read(&to_read, TimestampsToReturn::Neither, 0.0)
        .await
        .expect("reading the namespace array should succeed");

    let value = results
        .first()
        .and_then(|dv| dv.value.clone())
        .expect("namespace array should have a value");

    let namespaces: Vec<String> = match value {
        Variant::Array(array) => array
            .values
            .iter()
            .map(|v| match v {
                Variant::String(s) => s.as_ref().to_string(),
                other => other.to_string(),
            })
            .collect(),
        other => panic!("namespace array had unexpected variant: {other:?}"),
    };

    namespaces
        .iter()
        .position(|uri| uri == CIET_NAMESPACE_URI)
        .unwrap_or_else(|| {
            panic!("CIET namespace {CIET_NAMESPACE_URI} not found in {namespaces:?}")
        }) as u16
}

/// Read one `f64` variable by its CIET string identifier.
async fn read_double(session: &Session, ns: u16, identifier: &str) -> f64 {
    let to_read = [ReadValueId {
        node_id: NodeId::new(ns, identifier),
        attribute_id: AttributeId::Value as u32,
        ..Default::default()
    }];

    let results = session
        .read(&to_read, TimestampsToReturn::Both, 0.0)
        .await
        .unwrap_or_else(|e| panic!("read of {identifier} failed: {e}"));

    let dv = results.first().expect("one result expected");
    match dv.value.clone() {
        Some(Variant::Double(v)) => v,
        Some(Variant::Float(v)) => v as f64,
        other => panic!("{identifier}: expected a Double, got {other:?} (status {:?})", dv.status),
    }
}

/// Read one `bool` variable by its CIET string identifier.
async fn read_bool(session: &Session, ns: u16, identifier: &str) -> bool {
    let to_read = [ReadValueId {
        node_id: NodeId::new(ns, identifier),
        attribute_id: AttributeId::Value as u32,
        ..Default::default()
    }];

    let results = session
        .read(&to_read, TimestampsToReturn::Both, 0.0)
        .await
        .unwrap_or_else(|e| panic!("read of {identifier} failed: {e}"));

    match results.first().and_then(|dv| dv.value.clone()) {
        Some(Variant::Boolean(v)) => v,
        other => panic!("{identifier}: expected a Boolean, got {other:?}"),
    }
}

/// Write one variable and return the server's status code.
async fn write_variant(
    session: &Session,
    ns: u16,
    identifier: &str,
    value: Variant,
) -> opcua::types::StatusCode {
    let to_write = [WriteValue {
        node_id: NodeId::new(ns, identifier),
        attribute_id: AttributeId::Value as u32,
        index_range: Default::default(),
        value: DataValue::value_only(value),
    }];

    let results = session
        .write(&to_write)
        .await
        .unwrap_or_else(|e| panic!("write of {identifier} failed: {e}"));

    *results.first().expect("one status expected")
}

/// Verification of the complete CIET v2 OPC-UA contract in both directions.
///
/// **Methodology.** Start the real OPC-UA server on `127.0.0.1:48431` against a
/// default (isothermal 21 degC, at rest) [`CietState`]; connect a real
/// `async-opcua` client anonymously with `SecurityPolicy::None`; then check five
/// properties:
///
/// 1. **Namespace resolution** — `urn:outram-park:ciet-educational-simulator-v2`
///    appears in the server's `Server_NamespaceArray`, so a client can resolve
///    the index instead of assuming `ns=2`.
/// 2. **Every node is reachable** — every `CietSignal`, `CietControl` and
///    `CietSwitch` in the node map reads back a value of the right OPC-UA type
///    with a good status. This is what catches a node that was declared in the
///    enum but never added to the address space.
/// 3. **Outputs reflect plant state** — mutating the shared state directly
///    (standing in for the physics thread) changes what a client reads. Uses
///    BT-12 = 87.5 degC and heater power = 6.25 kW, values with exact `f64`
///    representations so the comparison is not a floating-point argument.
/// 4. **Writes reach plant state** — writing a control over OPC-UA changes the
///    shared state, verified by inspecting the state directly rather than by
///    reading the node back (which could pass on a cached address-space value
///    while the solver never saw the write).
/// 5. **The safety envelope holds over the wire** — an out-of-range write
///    (1000 kW against a 15 kW ceiling) is clamped, not stored raw, and a NaN
///    write leaves the previous value intact and finite.
/// 6. **A stale full-struct overwrite cannot lose a remote write** — the
///    regression guard for the lost-update race (bead `op-wqk.13.9`), exercised
///    end to end: write a set point over OPC-UA, then store a snapshot taken
///    *before* that write back over the plant state, exactly as v1's GUI did
///    every repaint, and confirm the write still takes effect.
///
/// Note that writes are checked through the pending-request store
/// (`CietUserControls`), which is where remote intent now lives — a write is
/// queued by the server callback and applied by the physics thread at the top of
/// each timestep. This test runs no physics, so it drains the queue explicitly
/// via [`pump_pending_requests`] and asserts on the pending state *before* the
/// drain as well as the plant state after it.
///
/// Pass criterion: all six hold, with numeric agreement to 1e-9.
///
/// **Results.** Recorded on the first green run; re-run with
/// `cargo test --release -p outram-park-digital-twin-engine --test ciet_v2_opcua_roundtrip`.
/// This test is the gate for the claim "the OPC-UA interface works"; until it
/// passes on a given machine, that claim is unverified there. It says nothing
/// about the physics being right — see the port-equivalence check tracked in
/// beads for that.
#[test]
fn opcua_interface_round_trips_controls_and_outputs() {
    let state = new_shared_state();
    let user_controls = new_shared_user_controls();
    let _config = start_test_server(Arc::clone(&state), Arc::clone(&user_controls));

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .expect("tokio runtime");

    runtime.block_on(async {
        let session = connect_client().await;
        let ns = resolve_namespace_index(&session).await;

        // ---- 2. every declared node is actually served ----
        for signal in CietSignal::ALL {
            let _ = read_double(&session, ns, signal.node_identifier()).await;
        }
        for control in CietControl::ALL {
            let _ = read_double(&session, ns, control.node_identifier()).await;
        }
        for switch in CietSwitch::ALL {
            let _ = read_bool(&session, ns, switch.node_identifier()).await;
        }

        // ---- 3. outputs follow plant state ----
        {
            let mut guard = state.write().expect("state lock");
            guard.bt_12_heater_outlet_deg_c = 87.5;
            guard.heater_power_kilowatts = 6.25;
        }
        tokio::time::sleep(PUBLISH_GRACE).await;

        let bt12 = read_double(
            &session,
            ns,
            CietSignal::Bt12HeaterOutletDegC.node_identifier(),
        )
        .await;
        assert!(
            (bt12 - 87.5).abs() < 1e-9,
            "BT-12 read {bt12}, expected 87.5 degC"
        );

        let power = read_double(&session, ns, CietSignal::HeaterPowerKw.node_identifier()).await;
        assert!(
            (power - 6.25).abs() < 1e-9,
            "heater power read {power}, expected 6.25 kW"
        );

        // ---- 4. writes reach plant state (via the pending-request store) ----
        let status = write_variant(
            &session,
            ns,
            CietControl::CtahPumpPressurePascals.node_identifier(),
            Variant::Double(8000.0),
        )
        .await;
        assert!(
            status.is_good(),
            "pump pressure write returned {status:?}, expected Good"
        );

        tokio::time::sleep(PUBLISH_GRACE).await;
        // Before the drain, the request must be pending and the plant state
        // untouched -- that is the whole point of separating remote intent from
        // plant state, so assert it rather than assuming it.
        assert_eq!(
            user_controls
                .read()
                .expect("user controls lock")
                .pending_control(CietControl::CtahPumpPressurePascals),
            Some(8000.0),
            "the write should be queued as a pending request"
        );
        assert_eq!(
            pump_pending_requests(&state, &user_controls),
            1,
            "exactly one request should have been applied"
        );

        let stored = state
            .read()
            .expect("state lock")
            .get_ctah_pump_pressure_f64();
        assert!(
            (stored - 8000.0).abs() < 1e-9,
            "pump pressure in plant state is {stored} Pa, expected 8000"
        );

        // a boolean control, since those take a different code path
        let status = write_variant(
            &session,
            ns,
            CietSwitch::CtahBranchBlocked.node_identifier(),
            Variant::Boolean(true),
        )
        .await;
        assert!(status.is_good(), "branch-block write returned {status:?}");

        tokio::time::sleep(PUBLISH_GRACE).await;
        assert_eq!(pump_pending_requests(&state, &user_controls), 1);
        assert!(
            state.read().expect("state lock").is_ctah_branch_blocked,
            "CTAH branch should be blocked after the write"
        );

        // ---- 5. the safety envelope holds over the wire ----
        let status = write_variant(
            &session,
            ns,
            CietControl::HeaterPowerKw.node_identifier(),
            Variant::Double(1000.0),
        )
        .await;
        assert!(
            status.is_good(),
            "out-of-range write should be accepted and clamped, got {status:?}"
        );

        tokio::time::sleep(PUBLISH_GRACE).await;
        pump_pending_requests(&state, &user_controls);
        let clamped = state.read().expect("state lock").heater_power_kilowatts;
        assert!(
            (clamped - 15.0).abs() < 1e-9,
            "heater power should clamp to the 15 kW ceiling, got {clamped}"
        );

        // NaN must not poison the solver's inputs.
        let _ = write_variant(
            &session,
            ns,
            CietControl::HeaterPowerKw.node_identifier(),
            Variant::Double(f64::NAN),
        )
        .await;

        tokio::time::sleep(PUBLISH_GRACE).await;
        pump_pending_requests(&state, &user_controls);
        let after_nan = state.read().expect("state lock").heater_power_kilowatts;
        assert!(
            after_nan.is_finite() && (after_nan - 15.0).abs() < 1e-9,
            "a NaN write must leave the previous value intact, got {after_nan}"
        );

        // ---- 6. a stale full-struct overwrite cannot lose a remote write ----
        // This is the regression guard for op-wqk.13.9 at the socket level: the
        // unit test in `user_controls` proves the mechanism, this proves it end
        // to end through a real OPC-UA write.
        let stale_clone = state.read().expect("state lock").clone();
        let status = write_variant(
            &session,
            ns,
            CietControl::Bt66TchxOutletSetPointDegC.node_identifier(),
            Variant::Double(45.0),
        )
        .await;
        assert!(status.is_good(), "set-point write returned {status:?}");
        tokio::time::sleep(PUBLISH_GRACE).await;

        // the GUI stores a snapshot taken before the write arrived
        state
            .write()
            .expect("state lock")
            .overwrite_state(stale_clone);

        assert_eq!(pump_pending_requests(&state, &user_controls), 1);
        let set_point = state
            .read()
            .expect("state lock")
            .bt_66_tchx_outlet_set_pt_deg_c;
        assert!(
            (set_point - 45.0).abs() < 1e-9,
            "remote write was lost to a stale overwrite; set point is {set_point}"
        );
    });
}
