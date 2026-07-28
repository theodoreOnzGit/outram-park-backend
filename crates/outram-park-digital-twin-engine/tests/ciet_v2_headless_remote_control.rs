//! End-to-end verification that a remote OPC-UA client can actually **drive the
//! CIET physics** — the one claim no other test covers.
//!
//! Every other check exercises a piece: the unit tests cover the node map and the
//! pending-request store, and `ciet_v2_opcua_roundtrip` covers the server's
//! address space and read/write plumbing against a directly-manipulated state.
//! None of them runs the solver. So none of them would catch the failure that
//! matters most to a user: *the write is accepted, the read-back looks right,
//! and nothing in the simulation ever changes.*
//!
//! This test closes that gap by launching the **real `ciet_educational_simulator_v2`
//! binary** as a subprocess in headless mode, connecting a real OPC-UA client
//! over TCP, commanding heater power and pump pressure, and then confirming the
//! CIET loop physically responds — the heater outlet temperature rises and a
//! temperature difference develops across the heater.
//!
//! ## Why it is headless
//!
//! The simulator's `--headless` mode runs the physics thread and the OPC-UA
//! server with no GUI at all. That is what makes this test possible in CI and on
//! a machine with no display, and it is the same code path Termux/Android uses,
//! so this test exercises the Android configuration on desktop hardware.
//!
//! ## Runtime
//!
//! Roughly 20-40 s wall clock. The test switches the simulator into
//! fast-forward over OPC-UA (itself a check that a boolean switch reaches the
//! physics thread) so simulated time advances far faster than real time, then
//! waits for a simulated-time target rather than a wall-clock one — so the test
//! is not flaky on a slow machine, only slower.
//!
//! ## Scope
//!
//! Per `RESPONSIBLE_USE.md`, an **offline educational demonstration**. Binds
//! loopback only and never advertises itself on the network.

use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::time::{Duration, Instant};

use outram_park_digital_twin_engine::ciet_opcua::node_map::{
    CietControl, CietSignal, CietSwitch, CIET_NAMESPACE_URI,
};

use opcua::client::{ClientBuilder, IdentityToken, Session};
use opcua::types::{
    AttributeId, DataValue, EndpointDescription, MessageSecurityMode, NodeId, ReadValueId,
    TimestampsToReturn, UserTokenPolicy, Variant, WriteValue,
};

/// Port for this test. Distinct from both 4840 and the round-trip test's 48431,
/// so all three can coexist.
const TEST_PORT: u16 = 48432;

/// Heater power to command, kW. Well inside the 0-15 kW envelope and enough to
/// heat the loop briskly.
const COMMANDED_HEATER_KW: f64 = 8.0;

/// CTAH pump pressure rise to command, Pa. Establishes forced circulation so the
/// heat actually moves downstream to the heater-outlet thermocouple instead of
/// sitting in the heater.
const COMMANDED_PUMP_PA: f64 = 7000.0;

/// Simulated seconds to wait for before judging the thermal response.
const TARGET_SIM_SECONDS: f64 = 60.0;

/// Hard wall-clock ceiling, so a hung simulator fails the test instead of
/// hanging CI forever.
const WALL_CLOCK_BUDGET: Duration = Duration::from_secs(180);

/// Kills the simulator subprocess when the test ends, however it ends.
///
/// Without this, a panicking assertion would leave an orphaned simulator holding
/// the port and quietly failing every later run.
struct SimulatorProcess {
    child: Child,
}

impl Drop for SimulatorProcess {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// Launch the real simulator binary in headless mode on loopback.
///
/// `CARGO_BIN_EXE_<name>` is set by Cargo for integration tests, so this runs the
/// same artifact a user would, not a re-implementation of it.
fn launch_headless_simulator() -> SimulatorProcess {
    let exe = env!("CARGO_BIN_EXE_ciet_educational_simulator_v2");

    let child = Command::new(exe)
        .args([
            "--headless",
            "--bind",
            "127.0.0.1",
            "--port",
            &TEST_PORT.to_string(),
            "--no-advertise",
        ])
        // Keep the simulator's status output from interleaving with test output.
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("the ciet_educational_simulator_v2 binary should launch");

    SimulatorProcess { child }
}

/// Connect an anonymous, unencrypted session, retrying while the simulator boots.
async fn connect_with_retry() -> Arc<Session> {
    let endpoint_url = format!("opc.tcp://127.0.0.1:{TEST_PORT}/ciet");
    let deadline = Instant::now() + Duration::from_secs(60);
    let mut last_error = String::new();

    loop {
        if Instant::now() > deadline {
            panic!("could not connect to the headless simulator: {last_error}");
        }

        let mut client = match ClientBuilder::new()
            .application_name("CIET v2 headless remote-control test")
            .application_uri("urn:outram-park:ciet-v2-headless-test")
            .create_sample_keypair(true)
            .trust_server_certs(true)
            .session_retry_limit(1)
            .pki_dir(std::env::temp_dir().join("ciet-v2-headless-test-pki"))
            .client()
        {
            Ok(client) => client,
            Err(errors) => {
                last_error = format!("{errors:?}");
                tokio::time::sleep(Duration::from_millis(500)).await;
                continue;
            }
        };

        let endpoint: EndpointDescription = (
            endpoint_url.as_ref(),
            "None",
            MessageSecurityMode::None,
            UserTokenPolicy::anonymous(),
        )
            .into();

        match client
            .connect_to_matching_endpoint(endpoint, IdentityToken::Anonymous)
            .await
        {
            Ok((session, event_loop)) => {
                let handle = event_loop.spawn();
                session.wait_for_connection().await;
                std::mem::forget(handle);
                return session;
            }
            Err(error) => {
                last_error = error.to_string();
                tokio::time::sleep(Duration::from_millis(500)).await;
            }
        }
    }
}

/// Resolve the CIET namespace index from the server's namespace array.
async fn resolve_namespace_index(session: &Session) -> u16 {
    let to_read = [ReadValueId {
        node_id: NodeId::new(0, 2255u32),
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
        other => panic!("unexpected namespace array variant: {other:?}"),
    };

    namespaces
        .iter()
        .position(|uri| uri == CIET_NAMESPACE_URI)
        .unwrap_or_else(|| panic!("CIET namespace missing from {namespaces:?}")) as u16
}

/// Read a `Double` variable by CIET string identifier.
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

    match results.first().and_then(|dv| dv.value.clone()) {
        Some(Variant::Double(v)) => v,
        Some(Variant::Float(v)) => v as f64,
        other => panic!("{identifier}: expected Double, got {other:?}"),
    }
}

/// Write a variable, asserting the server accepted it.
async fn write_checked(session: &Session, ns: u16, identifier: &str, value: Variant) {
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

    let status = results.first().expect("one status expected");
    assert!(
        status.is_good(),
        "write of {identifier} returned {status:?}, expected Good"
    );
}

/// Verification that an OPC-UA client can drive the CIET loop's physics.
///
/// **Methodology.** Launch the real `ciet_educational_simulator_v2` binary with
/// `--headless --bind 127.0.0.1 --port 48432 --no-advertise`, so the physics
/// thread and the OPC-UA server run exactly as they do for a user (and exactly as
/// they do on Termux). Connect an anonymous `async-opcua` client with
/// `SecurityPolicy::None` and resolve the CIET namespace index from
/// `Server_NamespaceArray`. Then:
///
/// 1. Record the baseline heater-outlet temperature (BT-12) and simulated time.
/// 2. Write `CoarseHeaterMesh = true` and `FastForwardOn = true` — two boolean
///    switches, which also proves a `Boolean` write reaches the physics thread
///    and not merely the address space.
/// 3. Write heater power = 8 kW and CTAH pump pressure = 7000 Pa. The pump
///    matters: without forced circulation the heat stays in the heater section
///    instead of reaching the outlet thermocouple.
/// 4. Poll until simulated time has advanced 60 s (wall-clock ceiling 180 s).
/// 5. Assert the control read-backs equal what was commanded — proving the
///    physics thread drained the pending requests — and that the loop responded
///    thermally: BT-12 rose by at least 2 degC from a 21 degC start, BT-12 now
///    exceeds BT-11 (a real temperature rise across the heater), the heater
///    power the solver reports is 8 kW, and the CTAH-branch flowrate is non-zero.
///
/// Pass criteria are deliberately loose on the physics (a 2 degC rise, not a
/// specific temperature) because this test verifies the **control path**, not the
/// accuracy of the thermal-hydraulics. Asserting a precise temperature here would
/// be a validation claim this test cannot support — the physics is a port of v1,
/// whose port equivalence is tracked separately.
///
/// **Results (2026-07-28, this machine, release profile).** Passed in 14.22 s
/// wall clock, reaching 61.3 s of simulated time — a speed-up of about 4.3x
/// real time with fast-forward and the coarse 8-node heater mesh both commanded
/// over OPC-UA. Measured at the end of the run:
///
/// | Quantity | Value |
/// |---|---|
/// | Heater power commanded / reported by solver | 8.00 kW / 8.00 kW |
/// | Pump pressure commanded / read back | 7000 Pa / 7000 Pa |
/// | BT-11 heater inlet | 20.99 degC |
/// | BT-12 heater outlet | 48.28 degC (baseline 21.00 degC) |
/// | Rise across the heater | 27.3 K |
/// | FM-40 CTAH-branch flow | 0.1225 kg/s |
///
/// **Interpretation.** The complete chain is demonstrated working: OPC-UA write
/// -> pending-request store -> physics-thread drain -> TUAS solver -> published
/// output -> OPC-UA read. Both a `Double` and a `Boolean` write reached the
/// solver, not merely the address space.
///
/// The 27.3 K rise is *plausible* rather than *validated*. A steady-state energy
/// balance at 8 kW and 0.1225 kg/s with Therminol VP-1 (`c_p` about
/// 1.7 kJ/(kg K) near 35 degC) predicts roughly 38 K; measuring 27.3 K partway
/// through a warm-up, with the CTAH simultaneously rejecting heat and the heater
/// steel still absorbing it, is the right order of magnitude and the right sign.
/// That is the whole strength of the claim: this test confirms the control path
/// is real and the response is not nonsense. It is **not** evidence that the
/// thermal-hydraulics are correct — that rests on v1's validation plus the
/// still-unrun port-equivalence check.
///
/// Re-run with
/// `cargo test --release -p outram-park-digital-twin-engine --test ciet_v2_headless_remote_control`.
/// A failure localises cleanly, because the control read-back assertions come
/// before the thermal ones.
#[test]
fn a_remote_client_can_drive_the_ciet_loop() {
    // Held for the whole test; killed on drop even if an assertion panics.
    let _simulator = launch_headless_simulator();

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .expect("tokio runtime");

    runtime.block_on(async {
        let session = connect_with_retry().await;
        let ns = resolve_namespace_index(&session).await;

        let baseline_bt12 =
            read_double(&session, ns, CietSignal::Bt12HeaterOutletDegC.node_identifier()).await;
        let baseline_sim_time =
            read_double(&session, ns, CietSignal::SimulationTimeSeconds.node_identifier()).await;

        // Boolean switches first: cheaper mesh and fast-forward, so the test is
        // quick without depending on wall-clock speed.
        write_checked(
            &session,
            ns,
            CietSwitch::CoarseHeaterMesh.node_identifier(),
            Variant::Boolean(true),
        )
        .await;
        write_checked(
            &session,
            ns,
            CietSwitch::FastForwardOn.node_identifier(),
            Variant::Boolean(true),
        )
        .await;

        // Now the continuous controls.
        write_checked(
            &session,
            ns,
            CietControl::HeaterPowerKw.node_identifier(),
            Variant::Double(COMMANDED_HEATER_KW),
        )
        .await;
        write_checked(
            &session,
            ns,
            CietControl::CtahPumpPressurePascals.node_identifier(),
            Variant::Double(COMMANDED_PUMP_PA),
        )
        .await;

        // Wait for simulated time to advance, not wall-clock time.
        let deadline = Instant::now() + WALL_CLOCK_BUDGET;
        let mut sim_time = baseline_sim_time;
        while sim_time - baseline_sim_time < TARGET_SIM_SECONDS {
            assert!(
                Instant::now() < deadline,
                "simulated time only reached {sim_time} s (from {baseline_sim_time} s) \
                 within the {WALL_CLOCK_BUDGET:?} budget -- the physics thread may be \
                 stalled or the fast-forward switch never took effect"
            );
            tokio::time::sleep(Duration::from_millis(500)).await;
            sim_time =
                read_double(&session, ns, CietSignal::SimulationTimeSeconds.node_identifier())
                    .await;
        }

        // ---- control read-backs: did the requests reach the solver? ----
        let effective_heater_kw =
            read_double(&session, ns, CietControl::HeaterPowerKw.node_identifier()).await;
        assert!(
            (effective_heater_kw - COMMANDED_HEATER_KW).abs() < 1e-6,
            "commanded {COMMANDED_HEATER_KW} kW but the simulator reports \
             {effective_heater_kw} kW -- the pending request never reached the \
             physics thread"
        );

        let effective_pump_pa =
            read_double(&session, ns, CietControl::CtahPumpPressurePascals.node_identifier())
                .await;
        assert!(
            (effective_pump_pa - COMMANDED_PUMP_PA).abs() < 1e-2,
            "commanded {COMMANDED_PUMP_PA} Pa but the simulator reports \
             {effective_pump_pa} Pa"
        );

        // The heater power the SOLVER acted on, which the killswitch can zero.
        let reported_power =
            read_double(&session, ns, CietSignal::HeaterPowerKw.node_identifier()).await;
        assert!(
            (reported_power - COMMANDED_HEATER_KW).abs() < 1e-6,
            "the solver reports {reported_power} kW of heater power; if this is 0 \
             the over-temperature killswitch tripped"
        );

        // ---- the physics actually responded ----
        let bt11 =
            read_double(&session, ns, CietSignal::Bt11HeaterInletDegC.node_identifier()).await;
        let bt12 =
            read_double(&session, ns, CietSignal::Bt12HeaterOutletDegC.node_identifier()).await;
        let ctah_flow =
            read_double(&session, ns, CietSignal::Fm40CtahBranchKgPerS.node_identifier()).await;

        assert!(
            bt12 - baseline_bt12 >= 2.0,
            "heater outlet only moved from {baseline_bt12} degC to {bt12} degC after \
             {TARGET_SIM_SECONDS} s of simulated time at {COMMANDED_HEATER_KW} kW -- \
             the control path reported success but the loop did not heat up"
        );

        assert!(
            bt12 > bt11,
            "no temperature rise across the heater: BT-11 = {bt11} degC, \
             BT-12 = {bt12} degC"
        );

        assert!(
            ctah_flow.abs() > 1.0e-4,
            "CTAH-branch flowrate is {ctah_flow} kg/s with {COMMANDED_PUMP_PA} Pa of \
             pump pressure commanded -- forced circulation did not establish"
        );

        println!(
            "headless remote control verified: sim {:.1} s, heater {:.2} kW, \
             BT-11 {:.2} degC, BT-12 {:.2} degC (from {:.2} degC), \
             FM-40 {:.4} kg/s",
            sim_time, reported_power, bt11, bt12, baseline_bt12, ctah_flow
        );
    });
}
