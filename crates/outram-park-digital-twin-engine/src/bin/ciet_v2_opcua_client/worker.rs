//! The OPC-UA client session, on its own thread with its own tokio runtime.
//!
//! ## Why a dedicated thread
//!
//! `egui` repaints on the main thread and must stay responsive at display rate.
//! An OPC-UA connect can block for seconds (TCP connect, `GetEndpoints`,
//! `CreateSession`, `ActivateSession`), and a read on a lossy WiFi link can block
//! for as long as the request timeout. Doing any of that on the repaint thread
//! would freeze the window — the classic symptom of a GUI that "hangs when you
//! click Connect".
//!
//! So: [`spawn_client_worker`] starts one `std::thread`, that thread builds its
//! own multi-thread tokio runtime, and the two sides meet only through
//! [`SharedClientState`]. Nothing in this module touches `egui`, and nothing in
//! the UI modules awaits.
//!
//! ## Lock discipline (important)
//!
//! [`SharedClientState`] is a `std::sync::RwLock`, whose guard is **not** held
//! across an `.await`. Every function here takes the lock, does one short
//! read-out or write-in, and drops the guard before awaiting anything. Holding it
//! across an await would let a network stall block the GUI's repaint on the same
//! lock — the exact failure the dedicated thread exists to prevent.
//!
//! ## Values: subscription preferred, polling as the fallback
//!
//! The client first tries the proper OPC-UA mechanism — a subscription with one
//! monitored item per variable, so the *server* decides when to publish a change.
//! If `CreateSubscription` or `CreateMonitoredItems` is refused, it falls back to
//! calling the `Read` service every [`UPDATE_INTERVAL_MS`], and records which
//! mode it ended up in ([`TransportMode`]) so the UI states it plainly rather
//! than implying a push model that is not running.
//!
//! ## Security: none, deliberately
//!
//! The session uses `SecurityPolicy::None`, `MessageSecurityMode::None` and
//! [`IdentityToken::Anonymous`] — no certificate, no signing, no encryption, no
//! user token. This matches the CIET v2 server, which is an **offline teaching
//! demonstrator**. Anyone who can reach the port can read every output and write
//! every control, and the UI says so on a permanent banner. Per
//! `RESPONSIBLE_USE.md` this client must never be pointed at live operational
//! systems, plant systems, safety-critical infrastructure, real-time plant
//! monitoring, or institutional production systems.
//!
//! ## What this module never does
//!
//! It never scans, sweeps, probes or port-knocks a network. The only addresses it
//! ever contacts are ones a simulator voluntarily announced over mDNS, or ones
//! the user typed in by hand.

use std::sync::Arc;
use std::time::{Duration, Instant};

use opcua::client::{ClientBuilder, DataChangeCallback, IdentityToken, Session};
use opcua::crypto::SecurityPolicy;
use opcua::types::{
    EndpointDescription, MessageSecurityMode, NodeId, ReadValueId, StatusCode, TimestampsToReturn,
    UserTokenPolicy, VariableId, Variant, WriteValue,
};

use outram_park_digital_twin_engine::ciet_opcua::node_map::{CietControl, CietSwitch};
use outram_park_digital_twin_engine::ciet_opcua::CIET_NAMESPACE_URI;

use crate::endpoint::{diagnose_connection_failure, resolve_namespace_index};
use crate::nodes::NodeIndex;
use crate::shared_state::{
    ClientCommand, ConnectionState, SharedClientState, TransportMode, WriteOutcome, WriteTarget,
    WrittenValue,
};
use crate::values::record_value;

/// Application name this client presents to the server, visible in the
/// simulator's session list.
pub const APPLICATION_NAME: &str = "OUTRAM PARK CIET v2 demo client";

/// Application URI this client presents to the server.
pub const APPLICATION_URI: &str = "urn:outram-park:ciet-v2-opcua-demo-client";

/// Interval between value updates, milliseconds — the subscription's requested
/// publishing interval and the polling fallback's read period.
///
/// 250 ms is four updates per second: fast enough that a slider write looks
/// immediate and a temperature transient looks continuous, slow enough that 36
/// nodes over WiFi is a trivial load.
pub const UPDATE_INTERVAL_MS: u64 = 250;

/// How often the worker checks its command queue while a session is up,
/// milliseconds. Shorter than the update interval so a write feels instant.
const COMMAND_TICK_MS: u64 = 50;

/// How often the worker checks its command queue while idle, milliseconds.
const IDLE_TICK_MS: u64 = 100;

/// Connection attempts the OPC-UA stack makes before giving up.
///
/// Deliberately small. A demo client that silently retried forever would leave
/// the user staring at "Connecting" with no diagnosis; failing quickly lets the
/// UI show the cause and the fix (wrong address / simulator not running /
/// isolating WiFi).
const SESSION_RETRY_LIMIT: i32 = 1;

/// Per-request timeout, seconds. Also bounds how long a stalled read can hold
/// the worker before it reports a failure.
const REQUEST_TIMEOUT_SECONDS: u64 = 5;

/// Requested subscription lifetime in publishing intervals.
const SUBSCRIPTION_LIFETIME_COUNT: u32 = 400;

/// Requested keep-alive count in publishing intervals.
const SUBSCRIPTION_KEEP_ALIVE_COUNT: u32 = 40;

/// How a session ended, when it ended without an error.
#[derive(Debug, Clone)]
enum SessionExit {
    /// The user asked to disconnect.
    Disconnected,
    /// The user asked to connect somewhere else; the URL to go to next.
    Reconnect(String),
}

/// A session failure, carrying enough to diagnose it for the user.
#[derive(Debug, Clone)]
struct SessionFailure {
    /// Full text from the OPC-UA stack, shown verbatim.
    message: String,
    /// Symbolic `StatusCode` name, or empty when there was none.
    status_name: String,
}

impl SessionFailure {
    /// Build from an `opcua` error, keeping both its status code and its text.
    fn from_opcua(error: &opcua::types::Error) -> Self {
        Self {
            message: error.to_string(),
            status_name: error.status().to_string(),
        }
    }

    /// Build from a plain message with no OPC-UA status code — used for
    /// failures this client detects itself, such as a server with no CIET
    /// namespace.
    fn from_message(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            status_name: String::new(),
        }
    }
}

/// Start the OPC-UA worker thread and hand back its join handle.
///
/// The thread runs until the process exits; it has no shutdown command because
/// closing the window ends the process. The returned handle is kept by the app so
/// the thread is not detached, which keeps it visible in a debugger and in a
/// panic backtrace.
///
/// # Arguments
///
/// * `shared` — the state handle the GUI also holds. The worker writes readings
///   and connection state into it and drains commands out of it.
///
/// # Panics
///
/// Does not panic on network failure — those become
/// [`ConnectionState::Failed`]. It does panic if the tokio runtime cannot be
/// built at all, which means the process has no usable threads and could not
/// have run anyway.
pub fn spawn_client_worker(shared: SharedClientState) -> std::thread::JoinHandle<()> {
    std::thread::Builder::new()
        .name("ciet-opcua-client".to_string())
        .spawn(move || {
            let runtime = tokio::runtime::Builder::new_multi_thread()
                .worker_threads(2)
                .enable_all()
                .build()
                .expect("OPC-UA client worker could not build a tokio runtime");
            runtime.block_on(worker_main(shared));
        })
        .expect("could not spawn the OPC-UA client worker thread")
}

/// The worker's outer loop: wait for a connect request, run the session, record
/// how it ended, repeat.
async fn worker_main(shared: SharedClientState) {
    let mut queued_target: Option<String> = None;

    loop {
        let target = match queued_target.take() {
            Some(url) => url,
            None => wait_for_connect_request(&shared).await,
        };

        set_connecting(&shared, &target);

        match run_session(&shared, &target).await {
            Ok(SessionExit::Disconnected) => set_disconnected(&shared),
            Ok(SessionExit::Reconnect(next_url)) => queued_target = Some(next_url),
            Err(failure) => set_failed(&shared, &target, &failure),
        }
    }
}

/// Poll the command queue until a [`ClientCommand::Connect`] arrives, returning
/// its URL.
///
/// Commands that need a session are answered honestly while idle: a write is
/// logged as failed with `BadNotConnected` rather than being silently dropped, so
/// a user who moves a slider before connecting sees why nothing happened.
async fn wait_for_connect_request(shared: &SharedClientState) -> String {
    loop {
        let commands = drain_commands(shared);
        for command in commands {
            match command {
                ClientCommand::Connect { endpoint_url } => return endpoint_url,
                ClientCommand::Disconnect => {
                    // Already disconnected; nothing to do.
                }
                ClientCommand::WriteControl { control, value } => {
                    record_not_connected_write(
                        shared,
                        WriteTarget::Control(control),
                        WrittenValue::Numeric(value),
                    );
                }
                ClientCommand::WriteSwitch { switch, value } => {
                    record_not_connected_write(
                        shared,
                        WriteTarget::Switch(switch),
                        WrittenValue::Boolean(value),
                    );
                }
            }
        }
        tokio::time::sleep(Duration::from_millis(IDLE_TICK_MS)).await;
    }
}

/// Open a session to `endpoint_url`, stream values, service writes, and run
/// until the user disconnects or the session dies.
async fn run_session(
    shared: &SharedClientState,
    endpoint_url: &str,
) -> Result<SessionExit, SessionFailure> {
    // ---- 1. Build the client. Anonymous, unencrypted, no certificate. ----
    let mut client = ClientBuilder::new()
        .application_name(APPLICATION_NAME)
        .application_uri(APPLICATION_URI)
        .product_uri(APPLICATION_URI)
        .create_sample_keypair(false)
        .trust_server_certs(true)
        .verify_server_certs(false)
        .session_retry_limit(SESSION_RETRY_LIMIT)
        .request_timeout(Duration::from_secs(REQUEST_TIMEOUT_SECONDS))
        .session_name(APPLICATION_NAME)
        .client()
        .map_err(|errors| {
            SessionFailure::from_message(format!(
                "client configuration rejected: {}",
                errors.join("; ")
            ))
        })?;

    let endpoint: EndpointDescription = (
        endpoint_url,
        SecurityPolicy::None.to_str(),
        MessageSecurityMode::None,
        UserTokenPolicy::anonymous(),
    )
        .into();

    // ---- 2. Connect and activate. ----
    let (session, event_loop) = client
        .connect_to_matching_endpoint(endpoint, IdentityToken::Anonymous)
        .await
        .map_err(|e| SessionFailure::from_opcua(&e))?;
    let event_loop_handle = event_loop.spawn();

    if !session.wait_for_connection().await {
        event_loop_handle.abort();
        return Err(SessionFailure::from_message(
            "the session never activated -- the server accepted the socket but did not \
             complete an OPC-UA session",
        ));
    }

    // ---- 3. Resolve the namespace index from the server, never assume it. ----
    let namespace_array = read_namespace_array(&session).await.map_err(|failure| {
        event_loop_handle.abort();
        failure
    })?;
    let namespace_index = match resolve_namespace_index(&namespace_array, CIET_NAMESPACE_URI) {
        Ok(index) => index,
        Err(error) => {
            let _ = session.disconnect().await;
            event_loop_handle.abort();
            return Err(SessionFailure::from_message(error.to_string()));
        }
    };
    let node_index = Arc::new(NodeIndex::new(namespace_index));

    // ---- 4. One immediate read, so the grid fills without waiting a tick. ----
    if let Err(failure) = poll_all_values(&session, &node_index, shared).await {
        let _ = session.disconnect().await;
        event_loop_handle.abort();
        return Err(failure);
    }

    // ---- 5. Prefer a subscription; fall back to polling. ----
    let transport = match try_subscribe(&session, &node_index, shared).await {
        Ok(()) => TransportMode::Subscription {
            publishing_interval_ms: UPDATE_INTERVAL_MS,
        },
        Err(reason) => {
            set_worker_note(
                shared,
                format!(
                    "subscription unavailable ({reason}); falling back to polling every \
                     {UPDATE_INTERVAL_MS} ms"
                ),
            );
            TransportMode::Polling {
                interval_ms: UPDATE_INTERVAL_MS,
            }
        }
    };

    set_connected(shared, endpoint_url, namespace_index, transport);

    // ---- 6. Serve commands and, if polling, read values, until told to stop. ----
    let mut last_poll = Instant::now();
    let exit = loop {
        if event_loop_handle.is_finished() {
            return Err(SessionFailure::from_message(
                "the session dropped -- the server closed the connection or the link went \
                 away",
            ));
        }

        let commands = drain_commands(shared);
        let mut exit_reason: Option<SessionExit> = None;
        for command in commands {
            match command {
                ClientCommand::Disconnect => exit_reason = Some(SessionExit::Disconnected),
                ClientCommand::Connect { endpoint_url } => {
                    exit_reason = Some(SessionExit::Reconnect(endpoint_url));
                }
                ClientCommand::WriteControl { control, value } => {
                    write_control(&session, &node_index, shared, control, value).await;
                }
                ClientCommand::WriteSwitch { switch, value } => {
                    write_switch(&session, &node_index, shared, switch, value).await;
                }
            }
        }
        if let Some(reason) = exit_reason {
            break reason;
        }

        let is_polling = matches!(transport, TransportMode::Polling { .. });
        if is_polling && last_poll.elapsed() >= Duration::from_millis(UPDATE_INTERVAL_MS) {
            last_poll = Instant::now();
            if let Err(failure) = poll_all_values(&session, &node_index, shared).await {
                let _ = session.disconnect().await;
                event_loop_handle.abort();
                return Err(failure);
            }
        }

        append_trend_points(shared);
        tokio::time::sleep(Duration::from_millis(COMMAND_TICK_MS)).await;
    };

    let _ = session.disconnect().await;
    event_loop_handle.abort();
    Ok(exit)
}

/// Read the server's `Server_NamespaceArray` (`ns=0;i=2255`) as strings.
///
/// This is the standard, mandatory variable every OPC-UA server publishes, and it
/// is what makes hard-coding `ns=2` unnecessary.
async fn read_namespace_array(session: &Session) -> Result<Vec<String>, SessionFailure> {
    let node_id = NodeId::new(0u16, VariableId::Server_NamespaceArray as u32);
    let request = [ReadValueId::new_value(node_id)];

    let values = session
        .read(&request, TimestampsToReturn::Neither, 0.0)
        .await
        .map_err(|e| SessionFailure::from_opcua(&e))?;

    let Some(data_value) = values.into_iter().next() else {
        return Err(SessionFailure::from_message(
            "server returned no value for its namespace array",
        ));
    };

    match data_value.value {
        Some(Variant::Array(array)) => Ok(array
            .values
            .iter()
            .map(|variant| match variant {
                Variant::String(text) => text.as_ref().to_string(),
                other => format!("{other:?}"),
            })
            .collect()),
        Some(Variant::String(single)) => Ok(vec![single.as_ref().to_string()]),
        Some(other) => Err(SessionFailure::from_message(format!(
            "server's namespace array was not an array of strings but {other:?}"
        ))),
        None => Err(SessionFailure::from_message(
            "server's namespace array read back empty",
        )),
    }
}

/// Create a subscription with one monitored item per CIET variable.
///
/// # Errors
///
/// Returns a short reason string on failure. That is not fatal to the session —
/// the caller falls back to polling and tells the user which mode is running.
async fn try_subscribe(
    session: &Session,
    node_index: &Arc<NodeIndex>,
    shared: &SharedClientState,
) -> Result<(), String> {
    let callback_index = Arc::clone(node_index);
    let callback_state = Arc::clone(shared);

    let subscription_id = session
        .create_subscription(
            Duration::from_millis(UPDATE_INTERVAL_MS),
            SUBSCRIPTION_LIFETIME_COUNT,
            SUBSCRIPTION_KEEP_ALIVE_COUNT,
            0,
            0,
            true,
            DataChangeCallback::new(move |data_value, monitored_item| {
                let node_id = &monitored_item.item_to_monitor().node_id;
                let Some(node) = callback_index.lookup(node_id) else {
                    // Not a CIET node — ignore rather than guess.
                    return;
                };
                let status = data_value.status.unwrap_or(StatusCode::Good);
                if let Ok(mut state) = callback_state.write() {
                    record_value(&mut state, node, data_value.value.as_ref(), status);
                }
            }),
        )
        .await
        .map_err(|e| e.to_string())?;

    let created = session
        .create_monitored_items(
            subscription_id,
            TimestampsToReturn::Both,
            node_index.all_monitored_item_requests(),
        )
        .await
        .map_err(|e| e.to_string())?;

    let refused = created
        .iter()
        .filter(|item| !item.result.status_code.is_good())
        .count();
    if refused > 0 {
        return Err(format!(
            "{refused} of {} monitored items were refused",
            node_index.len()
        ));
    }

    Ok(())
}

/// Read every CIET variable once and record the results.
///
/// Used for the initial fill in both transport modes, and on every tick in the
/// polling fallback. Request and response are paired positionally, in
/// [`NodeIndex::all_nodes`] order.
async fn poll_all_values(
    session: &Session,
    node_index: &NodeIndex,
    shared: &SharedClientState,
) -> Result<(), SessionFailure> {
    let nodes = node_index.all_nodes();
    let request = node_index.all_read_value_ids();

    let values = session
        .read(&request, TimestampsToReturn::Both, 0.0)
        .await
        .map_err(|e| SessionFailure::from_opcua(&e))?;

    if values.len() != nodes.len() {
        return Err(SessionFailure::from_message(format!(
            "server answered a {}-node read with {} values",
            nodes.len(),
            values.len()
        )));
    }

    let Ok(mut state) = shared.write() else {
        return Ok(());
    };
    for (node, data_value) in nodes.into_iter().zip(values.into_iter()) {
        let status = data_value.status.unwrap_or(StatusCode::Good);
        record_value(&mut state, node, data_value.value.as_ref(), status);
    }
    Ok(())
}

/// Write a `Double` to a control and log the outcome.
///
/// The **requested** value is sent unclamped. The server clamps it to the
/// control's `valid_range()`, and the subscription/poll brings back what it
/// actually stored — which is how the UI can show a user that their 1000 kW
/// request became the 15 kW ceiling.
async fn write_control(
    session: &Session,
    node_index: &NodeIndex,
    shared: &SharedClientState,
    control: CietControl,
    value: f64,
) {
    let write_value =
        WriteValue::value_attr(node_index.control_node_id(control), Variant::Double(value));
    let (status, message) = send_write(session, write_value).await;
    record_write(
        shared,
        WriteTarget::Control(control),
        WrittenValue::Numeric(value),
        status,
        message,
    );
}

/// Write a `Boolean` to a switch and log the outcome.
async fn write_switch(
    session: &Session,
    node_index: &NodeIndex,
    shared: &SharedClientState,
    switch: CietSwitch,
    value: bool,
) {
    let write_value =
        WriteValue::value_attr(node_index.switch_node_id(switch), Variant::Boolean(value));
    let (status, message) = send_write(session, write_value).await;
    record_write(
        shared,
        WriteTarget::Switch(switch),
        WrittenValue::Boolean(value),
        status,
        message,
    );
}

/// Issue one OPC-UA `Write` and reduce the answer to a status and a message.
///
/// A transport failure and a server-side rejection are reported differently, so
/// the UI can distinguish "the write never left" from "the server said no".
async fn send_write(session: &Session, write_value: WriteValue) -> (StatusCode, String) {
    match session.write(&[write_value]).await {
        Ok(statuses) => match statuses.first() {
            Some(status) => (*status, String::new()),
            None => (
                StatusCode::BadUnexpectedError,
                "server accepted the write request but returned no result".to_string(),
            ),
        },
        Err(error) => (error.status(), error.to_string()),
    }
}

// ---------------------------------------------------------------------------
// Short, guard-scoped helpers. Each takes the lock and drops it immediately --
// never across an `.await`.
// ---------------------------------------------------------------------------

/// Take every queued command, or an empty list if the lock is poisoned.
fn drain_commands(shared: &SharedClientState) -> Vec<ClientCommand> {
    match shared.write() {
        Ok(mut state) => state.take_commands(),
        Err(_) => Vec::new(),
    }
}

/// Move to [`ConnectionState::Connecting`] and clear the previous session's
/// readings, so no stale number can be read as live.
fn set_connecting(shared: &SharedClientState, endpoint_url: &str) {
    if let Ok(mut state) = shared.write() {
        state.clear_readings();
        state.connection = ConnectionState::Connecting {
            endpoint_url: endpoint_url.to_string(),
            started_at: Instant::now(),
        };
    }
}

/// Move to [`ConnectionState::Connected`].
fn set_connected(
    shared: &SharedClientState,
    endpoint_url: &str,
    namespace_index: u16,
    transport: TransportMode,
) {
    if let Ok(mut state) = shared.write() {
        state.connection = ConnectionState::Connected {
            endpoint_url: endpoint_url.to_string(),
            session_start: Instant::now(),
            namespace_index,
            transport,
        };
    }
}

/// Move to [`ConnectionState::Disconnected`] and drop the readings.
fn set_disconnected(shared: &SharedClientState) {
    if let Ok(mut state) = shared.write() {
        state.clear_readings();
        state.connection = ConnectionState::Disconnected;
    }
}

/// Move to [`ConnectionState::Failed`], attaching the diagnosis.
fn set_failed(shared: &SharedClientState, endpoint_url: &str, failure: &SessionFailure) {
    let cause = diagnose_connection_failure(&failure.status_name, &failure.message);
    if let Ok(mut state) = shared.write() {
        state.clear_readings();
        state.connection = ConnectionState::Failed {
            endpoint_url: endpoint_url.to_string(),
            message: failure.message.clone(),
            status_name: failure.status_name.clone(),
            cause,
            failed_at: Instant::now(),
        };
    }
}

/// Record a non-fatal note for the UI, e.g. the polling fallback.
fn set_worker_note(shared: &SharedClientState, note: String) {
    if let Ok(mut state) = shared.write() {
        state.worker_note = Some(note);
    }
}

/// Append the current values of the trended signals against simulated time.
fn append_trend_points(shared: &SharedClientState) {
    if let Ok(mut state) = shared.write() {
        state.append_trend_points();
    }
}

/// Log a write outcome.
fn record_write(
    shared: &SharedClientState,
    target: WriteTarget,
    value: WrittenValue,
    status: StatusCode,
    message: String,
) {
    if let Ok(mut state) = shared.write() {
        state.record_write(WriteOutcome {
            target,
            value,
            status,
            message,
            at: Instant::now(),
        });
    }
}

/// Log a write that was attempted with no session, so the UI can explain the
/// no-op rather than leaving the user to wonder.
fn record_not_connected_write(
    shared: &SharedClientState,
    target: WriteTarget,
    value: WrittenValue,
) {
    record_write(
        shared,
        target,
        value,
        StatusCode::BadNotConnected,
        "not connected -- connect to a simulator first".to_string(),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shared_state::NumericSample;
    use outram_park_digital_twin_engine::ciet_opcua::node_map::CietSignal;

    /// Verifies a write attempted with no session is logged as a failure rather
    /// than silently discarded.
    ///
    /// **Methodology.** Call [`record_not_connected_write`] for a control and a
    /// switch on a disconnected shared state, then assert both appear in the
    /// write log with `BadNotConnected` and that `has_failed_write()` is true.
    /// Pass criterion: 2 entries, both bad, both carrying an explanatory
    /// message.
    ///
    /// **Results (2026-07-28).** 2 / 2 entries logged with status
    /// `BadNotConnected` and the message "not connected -- connect to a
    /// simulator first"; `has_failed_write()` true. Interpretation: a user who
    /// moves a slider before connecting is told why nothing moved, instead of
    /// watching an unresponsive UI.
    #[test]
    fn writes_without_a_session_are_logged_as_failures() {
        let shared = crate::shared_state::new_shared_client_state();
        record_not_connected_write(
            &shared,
            WriteTarget::Control(CietControl::HeaterPowerKw),
            WrittenValue::Numeric(8.0),
        );
        record_not_connected_write(
            &shared,
            WriteTarget::Switch(CietSwitch::FastForwardOn),
            WrittenValue::Boolean(true),
        );

        let state = shared.read().unwrap();
        assert_eq!(state.write_log.len(), 2);
        assert!(state.has_failed_write());
        for outcome in &state.write_log {
            assert_eq!(outcome.status, StatusCode::BadNotConnected);
            assert!(outcome.message.contains("not connected"));
        }
    }

    /// Verifies the connection-state helpers move the shared state through the
    /// intended sequence and wipe readings at every transition that ends a
    /// session's validity.
    ///
    /// **Methodology.** Record a reading, then drive
    /// `set_connecting` → `set_connected` → `set_disconnected` and, separately,
    /// `set_failed`, asserting the [`ConnectionState`] variant after each and
    /// that readings are empty after `set_connecting`, `set_disconnected` and
    /// `set_failed`. Pass criterion: correct variant at each step; readings
    /// cleared at all three clearing transitions; the resolved namespace index
    /// preserved on `Connected`.
    ///
    /// **Results (2026-07-28).** Sequence measured as
    /// `Disconnected → Connecting → Connected{ns=3} → Disconnected`, with
    /// `signals` empty after each of the three clearing transitions and the
    /// namespace index read back as 3 (not 2, confirming it is carried through
    /// rather than defaulted). `set_failed` produced `Failed` with cause
    /// `Unreachable` for a `BadTimeout`. Interpretation: numbers from one
    /// session can never be displayed as belonging to the next.
    #[test]
    fn connection_transitions_clear_stale_readings() {
        let shared = crate::shared_state::new_shared_client_state();
        let url = "opc.tcp://192.168.1.42:4840/ciet";

        {
            let mut state = shared.write().unwrap();
            state.record_signal(
                CietSignal::Bt12HeaterOutletDegC,
                NumericSample {
                    value: 86.5,
                    status: StatusCode::Good,
                    received_at: Instant::now(),
                },
            );
            assert!(matches!(state.connection, ConnectionState::Disconnected));
        }

        set_connecting(&shared, url);
        {
            let state = shared.read().unwrap();
            assert!(matches!(
                state.connection,
                ConnectionState::Connecting { .. }
            ));
            assert!(state.signals.is_empty(), "stale reading survived connect");
        }

        set_connected(
            &shared,
            url,
            3,
            TransportMode::Subscription {
                publishing_interval_ms: UPDATE_INTERVAL_MS,
            },
        );
        {
            let state = shared.read().unwrap();
            match &state.connection {
                ConnectionState::Connected {
                    namespace_index,
                    transport,
                    ..
                } => {
                    assert_eq!(*namespace_index, 3);
                    assert!(matches!(transport, TransportMode::Subscription { .. }));
                }
                other => panic!("expected Connected, got {other:?}"),
            }
        }

        set_disconnected(&shared);
        {
            let state = shared.read().unwrap();
            assert!(matches!(state.connection, ConnectionState::Disconnected));
            assert!(state.signals.is_empty());
        }

        set_failed(
            &shared,
            url,
            &SessionFailure {
                message: "BadTimeout: request timed out".to_string(),
                status_name: "BadTimeout".to_string(),
            },
        );
        {
            let state = shared.read().unwrap();
            match &state.connection {
                ConnectionState::Failed { cause, .. } => {
                    assert_eq!(*cause, crate::endpoint::ConnectionFailureCause::Unreachable);
                }
                other => panic!("expected Failed, got {other:?}"),
            }
            assert!(state.signals.is_empty());
        }
    }
}
