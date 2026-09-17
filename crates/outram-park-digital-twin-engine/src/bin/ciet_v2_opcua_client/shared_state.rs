//! The single piece of state shared between the GUI thread and the OPC-UA
//! client thread.
//!
//! ## Why one `Arc<RwLock<T>>` and no channels
//!
//! The GUI repaints at display rate and must never block on network I/O; the
//! OPC-UA session lives on its own thread with its own tokio runtime. They meet
//! at exactly one place: [`SharedClientState`], an `Arc<RwLock<`
//! [`ClientSharedState`] `>>`.
//!
//! `RwLock` rather than `Mutex` per the workspace Rust design rules — the GUI is
//! a reader many times a second and the worker a writer four times a second, so
//! serialising reads would be pure waste.
//!
//! Commands flow the *other* way (GUI to worker) through
//! [`ClientSharedState::pending_commands`] rather than an `mpsc` channel. That
//! keeps the whole GUI/worker contract in one struct a reader can hold in their
//! head, and it avoids a blocking `recv` inside an async runtime. The worker
//! drains the queue with [`take_commands`](ClientSharedState::take_commands) on
//! its poll tick.
//!
//! ## Never fabricate a reading
//!
//! Every measured quantity is an `Option<`[`NumericSample`]`>` /
//! `Option<`[`BooleanSample`]`>`, and `None` means *this node has not been read
//! yet*. The UI renders `None` as `--`. There is no default of `0.0`, because a
//! zero displayed in a temperature column is indistinguishable from a real
//! reading of 0 degC and would be a fabricated value — forbidden by
//! `RESPONSIBLE_USE.md` and by this crate's `CLAUDE.md`. For the same reason
//! [`clear_readings`](ClientSharedState::clear_readings) wipes every sample when
//! a new connection is started, so numbers from a *previous* session can never
//! be mistaken for live ones.

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

use opcua::types::StatusCode;

use outram_park_digital_twin_engine::ciet_opcua::node_map::{CietControl, CietSignal, CietSwitch};

/// Signals given a time trend, in the order the trend panel stacks them.
///
/// Chosen to tell the story of one loop pass at a glance: the heater power going
/// in (kW), then the fluid temperature at the heater outlet (BT-12), the CTAH
/// outlet (BT-41) and the TCHX outlet (BT-66), plus the heater inlet (BT-11) so
/// the rise across the heater is visible as the gap between two lines.
pub const TRENDED_SIGNALS: &[CietSignal] = &[
    CietSignal::HeaterPowerKw,
    CietSignal::Bt11HeaterInletDegC,
    CietSignal::Bt12HeaterOutletDegC,
    CietSignal::Bt41CtahOutletDegC,
    CietSignal::Bt66TchxOutletDegC,
];

/// Points retained per trend before the oldest are dropped.
///
/// At the 250 ms update rate this is 4000 * 0.25 s = **1000 s ≈ 17 minutes** of
/// simulated history per signal, which spans a CIET natural-circulation
/// transient comfortably. Five trends at 4000 points of `[f64; 2]` is 320 kB —
/// negligible, and bounded, so a client left running overnight cannot grow
/// without limit.
pub const TREND_CAPACITY: usize = 4000;

/// Write outcomes kept for the UI's activity log.
pub const WRITE_LOG_CAPACITY: usize = 64;

/// How the client is currently getting values from the server.
///
/// OPC-UA subscriptions are the right mechanism — the server pushes a value only
/// when it changes — but they are also the part of a stack most likely to be
/// unimplemented or subtly broken. So the client prefers a subscription and
/// falls back to straight `Read` polling, and records which one it ended up
/// using so the UI can say so honestly rather than implying a push model that
/// is not running.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransportMode {
    /// Monitored items on a subscription; the server pushes changes.
    Subscription {
        /// Requested publishing interval, milliseconds.
        publishing_interval_ms: u64,
    },
    /// Repeated `Read` service calls from the client.
    Polling {
        /// Interval between reads, milliseconds.
        interval_ms: u64,
    },
}

impl TransportMode {
    /// Short label naming the mechanism and its rate, for the status bar.
    pub fn label(&self) -> String {
        match self {
            Self::Subscription {
                publishing_interval_ms,
            } => format!("subscription, {publishing_interval_ms} ms publishing interval"),
            Self::Polling { interval_ms } => format!("polling, every {interval_ms} ms"),
        }
    }
}

/// Where the client's OPC-UA session currently stands.
///
/// A four-variant enum rather than a pair of booleans, so that "connecting" and
/// "failed" cannot be confused with each other or with "connected", and so the
/// compiler forces every UI branch to handle all four (workspace Rust design
/// rules: enums for dispatch, never `Box<dyn Trait>` and never a bare bool).
#[derive(Debug, Clone)]
pub enum ConnectionState {
    /// No session and none being attempted. The starting state, and the state
    /// after a clean user-requested disconnect.
    Disconnected,

    /// A connection attempt is in flight: TCP connect, `GetEndpoints`,
    /// `CreateSession`, `ActivateSession`, then namespace resolution.
    Connecting {
        /// Canonical URL being attempted.
        endpoint_url: String,
        /// When the attempt started, for an "attempting for N s" line.
        started_at: Instant,
    },

    /// An activated session with the CIET namespace resolved.
    Connected {
        /// Canonical URL of the connected endpoint.
        endpoint_url: String,
        /// When the session became usable, for an uptime readout.
        session_start: Instant,
        /// The `ns=` index this server assigned the CIET namespace. Read from
        /// the server, never assumed — see
        /// [`resolve_namespace_index`](crate::endpoint::resolve_namespace_index).
        namespace_index: u16,
        /// Whether values are arriving by subscription or by polling.
        transport: TransportMode,
    },

    /// The last attempt or session failed. Holds the diagnosis so the UI can
    /// show a cause and a fix instead of a bare status code.
    Failed {
        /// URL that was being used when it failed.
        endpoint_url: String,
        /// Raw text from the OPC-UA stack, shown verbatim and never reworded.
        message: String,
        /// Symbolic `StatusCode` name, e.g. `BadTimeout`. Empty when the failure
        /// had no status code (a URL that never parsed, for instance).
        status_name: String,
        /// The likely cause, for the "what to try next" block.
        cause: crate::endpoint::ConnectionFailureCause,
        /// When it failed.
        failed_at: Instant,
    },
}

impl ConnectionState {
    /// One-word label for the status strip.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Disconnected => "Disconnected",
            Self::Connecting { .. } => "Connecting",
            Self::Connected { .. } => "Connected",
            Self::Failed { .. } => "Failed",
        }
    }

    /// `true` only in [`Self::Connected`] — the one state in which reads are
    /// live and writes will be attempted.
    pub fn is_connected(&self) -> bool {
        matches!(self, Self::Connected { .. })
    }

    /// `true` while an attempt is in flight, so the UI can disable the Connect
    /// buttons instead of queueing a second attempt.
    pub fn is_busy(&self) -> bool {
        matches!(self, Self::Connecting { .. })
    }

    /// The endpoint URL this state refers to, if any.
    pub fn endpoint_url(&self) -> Option<&str> {
        match self {
            Self::Disconnected => None,
            Self::Connecting { endpoint_url, .. }
            | Self::Connected { endpoint_url, .. }
            | Self::Failed { endpoint_url, .. } => Some(endpoint_url),
        }
    }

    /// How long the session has been up, or the attempt has been running, or
    /// the failure has been showing.
    pub fn age(&self) -> Option<Duration> {
        match self {
            Self::Disconnected => None,
            Self::Connecting { started_at, .. } => Some(started_at.elapsed()),
            Self::Connected { session_start, .. } => Some(session_start.elapsed()),
            Self::Failed { failed_at, .. } => Some(failed_at.elapsed()),
        }
    }
}

/// A `Double` value read from the server, with the status that came with it.
///
/// The status is retained rather than discarded because OPC-UA can return a
/// value *and* a non-`Good` status (`UncertainLastUsableValue`, for instance),
/// and a client that showed the number while dropping the caveat would be
/// presenting a stale reading as a fresh one.
#[derive(Debug, Clone, Copy)]
pub struct NumericSample {
    /// The value, in the unit given by the node's `unit()`.
    pub value: f64,
    /// Status the server attached to it.
    pub status: StatusCode,
    /// When this client received it, for an age readout.
    pub received_at: Instant,
}

impl NumericSample {
    /// Whether the server called this value good. `false` means the number is
    /// shown with a warning, not silently.
    pub fn is_good(&self) -> bool {
        self.status.is_good()
    }
}

/// A `Boolean` value read from the server, with its status.
#[derive(Debug, Clone, Copy)]
pub struct BooleanSample {
    /// The value.
    pub value: bool,
    /// Status the server attached to it.
    pub status: StatusCode,
    /// When this client received it.
    pub received_at: Instant,
}

impl BooleanSample {
    /// Whether the server called this value good.
    pub fn is_good(&self) -> bool {
        self.status.is_good()
    }
}

/// A bounded time trend of one signal against **simulated** time.
///
/// The x axis is the simulator's own `SimulationTimeSeconds`, not wall-clock
/// time, so a trend still reads correctly while the simulator is in fast-forward
/// or slow-motion. Points are only appended when simulated time has actually
/// advanced, which keeps a paused simulator from stacking thousands of points on
/// one abscissa.
#[derive(Debug, Clone, Default)]
pub struct TrendBuffer {
    points: VecDeque<[f64; 2]>,
}

impl TrendBuffer {
    /// An empty trend.
    pub fn new() -> Self {
        Self {
            points: VecDeque::new(),
        }
    }

    /// Append `[simulation_time_seconds, value]`, dropping the oldest point once
    /// [`TREND_CAPACITY`] is reached.
    ///
    /// Non-finite values and non-advancing time are ignored: a `NaN` would blank
    /// the whole plot, and a repeated abscissa carries no information.
    pub fn push(&mut self, simulation_time_seconds: f64, value: f64) {
        if !simulation_time_seconds.is_finite() || !value.is_finite() {
            return;
        }
        if let Some(last) = self.points.back() {
            if simulation_time_seconds <= last[0] {
                return;
            }
        }
        if self.points.len() >= TREND_CAPACITY {
            self.points.pop_front();
        }
        self.points.push_back([simulation_time_seconds, value]);
    }

    /// The retained points, oldest first, as `[simulation_time_seconds, value]`.
    pub fn points(&self) -> Vec<[f64; 2]> {
        self.points.iter().copied().collect()
    }

    /// Number of retained points.
    pub fn len(&self) -> usize {
        self.points.len()
    }

    /// Whether the trend has no points yet — the UI shows "waiting for data"
    /// rather than an empty set of axes.
    pub fn is_empty(&self) -> bool {
        self.points.is_empty()
    }

    /// Drop every point. Called when a new session starts, so one session's
    /// history is never drawn continuing into another's.
    pub fn clear(&mut self) {
        self.points.clear();
    }
}

/// Which writable node a write was aimed at.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WriteTarget {
    /// A continuous control.
    Control(CietControl),
    /// An on/off switch.
    Switch(CietSwitch),
}

impl WriteTarget {
    /// Human-facing label of the target node.
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Control(control) => control.display_name(),
            Self::Switch(switch) => switch.display_name(),
        }
    }
}

/// The value a write carried.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum WrittenValue {
    /// A `Double` written to a [`CietControl`].
    Numeric(f64),
    /// A `Boolean` written to a [`CietSwitch`].
    Boolean(bool),
}

impl WrittenValue {
    /// The value formatted for the activity log.
    pub fn display(&self) -> String {
        match self {
            Self::Numeric(v) => format!("{v:.4}"),
            Self::Boolean(v) => v.to_string(),
        }
    }
}

/// The recorded result of one OPC-UA `Write`.
///
/// Kept whether it succeeded or failed. A failed write that vanished silently is
/// the worst outcome for a demo — the user moves a slider, nothing happens, and
/// there is nothing on screen to explain it — so every outcome is logged and the
/// failures are drawn in the UI's error colour.
#[derive(Debug, Clone)]
pub struct WriteOutcome {
    /// The node written.
    pub target: WriteTarget,
    /// The value sent.
    pub value: WrittenValue,
    /// The `StatusCode` the server returned, or the transport failure's code.
    pub status: StatusCode,
    /// Extra text when the write never reached the server (session dropped,
    /// not connected). Empty when the server itself answered.
    pub message: String,
    /// When the outcome was recorded.
    pub at: Instant,
}

impl WriteOutcome {
    /// Whether the server accepted the write.
    pub fn is_good(&self) -> bool {
        self.status.is_good()
    }
}

/// A request from the GUI thread to the OPC-UA worker thread.
///
/// An enum, matched exhaustively by the worker, so a new command cannot be added
/// without the compiler pointing at the place that must handle it.
#[derive(Debug, Clone)]
pub enum ClientCommand {
    /// Open a session to this already-normalised URL, dropping any current one.
    Connect {
        /// Canonical `opc.tcp://host:port/path` URL.
        endpoint_url: String,
    },
    /// Close the current session and return to [`ConnectionState::Disconnected`].
    Disconnect,
    /// Write a `Double` to a control.
    WriteControl {
        /// Which control.
        control: CietControl,
        /// The value, in the control's own unit. The **server** clamps it to
        /// the control's `valid_range()`; this client sends what the user asked
        /// for so the clamping is visible in the read-back.
        value: f64,
    },
    /// Write a `Boolean` to a switch.
    WriteSwitch {
        /// Which switch.
        switch: CietSwitch,
        /// The value.
        value: bool,
    },
}

/// Everything the GUI and the OPC-UA worker thread share.
///
/// Read by the GUI on every repaint; written by the worker on every update tick.
/// Hold the lock for as short a time as possible on both sides — in particular
/// the worker must copy values out and release the guard *before* any `.await`,
/// since this is a `std::sync::RwLock` and its guard must not cross an await
/// point.
#[derive(Debug)]
pub struct ClientSharedState {
    /// Where the session stands. The UI matches this exhaustively.
    pub connection: ConnectionState,

    /// Commands the GUI has queued and the worker has not yet drained.
    pub pending_commands: Vec<ClientCommand>,

    /// Latest reading per read-only signal. Absent means *never read* and is
    /// displayed as `--`.
    pub signals: HashMap<CietSignal, NumericSample>,

    /// Latest read-back per control, so the user can see the value the server
    /// actually holds after clamping — which may differ from what they sent.
    pub controls: HashMap<CietControl, NumericSample>,

    /// Latest read-back per switch.
    pub switches: HashMap<CietSwitch, BooleanSample>,

    /// Time trends for [`TRENDED_SIGNALS`].
    pub trends: HashMap<CietSignal, TrendBuffer>,

    /// Most recent write outcomes, newest first, capped at
    /// [`WRITE_LOG_CAPACITY`].
    pub write_log: VecDeque<WriteOutcome>,

    /// Count of data values received since this session started, for a
    /// "the link is alive" indicator that does not depend on any value changing.
    pub values_received: u64,

    /// When the last data value arrived.
    pub last_value_at: Option<Instant>,

    /// Non-fatal worker note, e.g. "subscription refused, fell back to polling".
    /// Distinct from [`ConnectionState::Failed`], which ends the session.
    pub worker_note: Option<String>,
}

impl Default for ClientSharedState {
    fn default() -> Self {
        Self::new()
    }
}

impl ClientSharedState {
    /// A disconnected client with no readings and no queued commands.
    pub fn new() -> Self {
        let mut trends = HashMap::new();
        for signal in TRENDED_SIGNALS {
            trends.insert(*signal, TrendBuffer::new());
        }
        Self {
            connection: ConnectionState::Disconnected,
            pending_commands: Vec::new(),
            signals: HashMap::new(),
            controls: HashMap::new(),
            switches: HashMap::new(),
            trends,
            write_log: VecDeque::new(),
            values_received: 0,
            last_value_at: None,
            worker_note: None,
        }
    }

    /// Queue a command for the worker thread.
    pub fn push_command(&mut self, command: ClientCommand) {
        self.pending_commands.push(command);
    }

    /// Take every queued command, leaving the queue empty.
    ///
    /// Called by the worker once per tick. Draining rather than peeking means a
    /// command is delivered exactly once even if the worker is slow.
    pub fn take_commands(&mut self) -> Vec<ClientCommand> {
        std::mem::take(&mut self.pending_commands)
    }

    /// Forget every reading, trend point and write outcome.
    ///
    /// Called when a connection attempt starts, so that nothing from a previous
    /// session can be read as live data. The `--` placeholders that come back
    /// are the honest display for "not read yet".
    pub fn clear_readings(&mut self) {
        self.signals.clear();
        self.controls.clear();
        self.switches.clear();
        for trend in self.trends.values_mut() {
            trend.clear();
        }
        self.write_log.clear();
        self.values_received = 0;
        self.last_value_at = None;
        self.worker_note = None;
    }

    /// Record a signal reading and count it.
    pub fn record_signal(&mut self, signal: CietSignal, sample: NumericSample) {
        self.signals.insert(signal, sample);
        self.values_received = self.values_received.saturating_add(1);
        self.last_value_at = Some(sample.received_at);
    }

    /// Record a control read-back and count it.
    pub fn record_control(&mut self, control: CietControl, sample: NumericSample) {
        self.controls.insert(control, sample);
        self.values_received = self.values_received.saturating_add(1);
        self.last_value_at = Some(sample.received_at);
    }

    /// Record a switch read-back and count it.
    pub fn record_switch(&mut self, switch: CietSwitch, sample: BooleanSample) {
        self.switches.insert(switch, sample);
        self.values_received = self.values_received.saturating_add(1);
        self.last_value_at = Some(sample.received_at);
    }

    /// Append the current value of every trended signal against the current
    /// simulated time.
    ///
    /// Does nothing until `SimulationTimeSeconds` has been read, because without
    /// an abscissa there is nothing honest to plot against — the alternative,
    /// substituting wall-clock time, would silently mislabel a fast-forwarded
    /// run.
    pub fn append_trend_points(&mut self) {
        let Some(simulation_time) = self
            .signals
            .get(&CietSignal::SimulationTimeSeconds)
            .map(|sample| sample.value)
        else {
            return;
        };

        for signal in TRENDED_SIGNALS {
            let Some(sample) = self.signals.get(signal).copied() else {
                continue;
            };
            if let Some(trend) = self.trends.get_mut(signal) {
                trend.push(simulation_time, sample.value);
            }
        }
    }

    /// Record a write outcome at the front of the log, dropping the oldest past
    /// [`WRITE_LOG_CAPACITY`].
    pub fn record_write(&mut self, outcome: WriteOutcome) {
        if self.write_log.len() >= WRITE_LOG_CAPACITY {
            self.write_log.pop_back();
        }
        self.write_log.push_front(outcome);
    }

    /// Whether any write in the log failed — drives a warning in the status
    /// strip so a failure is noticed without opening the log.
    pub fn has_failed_write(&self) -> bool {
        self.write_log.iter().any(|outcome| !outcome.is_good())
    }
}

/// The shared handle both threads hold.
pub type SharedClientState = Arc<RwLock<ClientSharedState>>;

/// Build a fresh shared state handle.
pub fn new_shared_client_state() -> SharedClientState {
    Arc::new(RwLock::new(ClientSharedState::new()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::endpoint::ConnectionFailureCause;

    fn sample(value: f64) -> NumericSample {
        NumericSample {
            value,
            status: StatusCode::Good,
            received_at: Instant::now(),
        }
    }

    /// Verifies that the connection-state machine reports each of its four
    /// states distinctly, and that only `Connected` claims to be connected.
    ///
    /// **Methodology.** Construct all four [`ConnectionState`] variants and
    /// assert `label()`, `is_connected()`, `is_busy()` and `endpoint_url()` for
    /// each. The reference is the state machine's contract: exactly one variant
    /// may be `is_connected()`, exactly one may be `is_busy()`, and every
    /// variant except `Disconnected` must carry the URL it refers to (so a
    /// failure message can name the address that failed). Pass criterion: all
    /// 16 assertions hold.
    ///
    /// **Results (2026-07-28).** Labels measured as `Disconnected`,
    /// `Connecting`, `Connected`, `Failed`. `is_connected()` was true for
    /// exactly 1 of 4 variants; `is_busy()` true for exactly 1 of 4;
    /// `endpoint_url()` returned `Some` for 3 of 4 and `None` for
    /// `Disconnected`. Interpretation: the UI cannot mistake a failed or
    /// in-flight attempt for a live session, which a pair of booleans would have
    /// permitted (`connecting && connected` has no meaning but is
    /// representable).
    #[test]
    fn the_four_connection_states_are_reported_distinctly() {
        let url = "opc.tcp://192.168.1.42:4840/ciet".to_string();

        let disconnected = ConnectionState::Disconnected;
        assert_eq!(disconnected.label(), "Disconnected");
        assert!(!disconnected.is_connected());
        assert!(!disconnected.is_busy());
        assert_eq!(disconnected.endpoint_url(), None);

        let connecting = ConnectionState::Connecting {
            endpoint_url: url.clone(),
            started_at: Instant::now(),
        };
        assert_eq!(connecting.label(), "Connecting");
        assert!(!connecting.is_connected());
        assert!(connecting.is_busy());
        assert_eq!(connecting.endpoint_url(), Some(url.as_str()));

        let connected = ConnectionState::Connected {
            endpoint_url: url.clone(),
            session_start: Instant::now(),
            namespace_index: 2,
            transport: TransportMode::Subscription {
                publishing_interval_ms: 250,
            },
        };
        assert_eq!(connected.label(), "Connected");
        assert!(connected.is_connected());
        assert!(!connected.is_busy());
        assert_eq!(connected.endpoint_url(), Some(url.as_str()));

        let failed = ConnectionState::Failed {
            endpoint_url: url.clone(),
            message: "BadTimeout".to_string(),
            status_name: "BadTimeout".to_string(),
            cause: ConnectionFailureCause::Unreachable,
            failed_at: Instant::now(),
        };
        assert_eq!(failed.label(), "Failed");
        assert!(!failed.is_connected());
        assert!(!failed.is_busy());
        assert_eq!(failed.endpoint_url(), Some(url.as_str()));
    }

    /// Verifies that a node never read is absent from the state — the condition
    /// the UI renders as `--` — and that starting a new connection wipes the
    /// previous session's readings.
    ///
    /// **Methodology.** On a fresh state, assert every one of the 21 signals, 8
    /// controls and 7 switches is absent. Then record a reading for each of the
    /// three kinds, confirm presence, call `clear_readings()`, and confirm
    /// absence again. This is the anti-fabrication guarantee from
    /// `RESPONSIBLE_USE.md`: there is no `0.0` default that could be displayed
    /// as a measurement, and stale numbers cannot survive into a new session.
    /// Pass criterion: 36 absent before, 3 present after recording, 36 absent
    /// after clearing.
    ///
    /// **Results (2026-07-28).** Measured 36 / 36 nodes absent on a fresh state
    /// (21 signals + 8 controls + 7 switches, matching
    /// `node_map::total_node_count()`); 3 / 3 present after recording;
    /// 36 / 36 absent again after `clear_readings()`, with `values_received`
    /// back to 0 from 3. Interpretation: the `--` placeholder is structural, not
    /// a formatting convention that a later edit could bypass.
    #[test]
    fn unread_nodes_are_absent_rather_than_zero() {
        let mut state = ClientSharedState::new();

        for signal in CietSignal::ALL {
            assert!(
                !state.signals.contains_key(signal),
                "{signal:?} pre-populated"
            );
        }
        for control in CietControl::ALL {
            assert!(!state.controls.contains_key(control));
        }
        for switch in CietSwitch::ALL {
            assert!(!state.switches.contains_key(switch));
        }
        assert_eq!(state.values_received, 0);

        state.record_signal(CietSignal::Bt12HeaterOutletDegC, sample(86.5));
        state.record_control(CietControl::HeaterPowerKw, sample(8.0));
        state.record_switch(
            CietSwitch::FastForwardOn,
            BooleanSample {
                value: true,
                status: StatusCode::Good,
                received_at: Instant::now(),
            },
        );
        assert!(state
            .signals
            .contains_key(&CietSignal::Bt12HeaterOutletDegC));
        assert!(state.controls.contains_key(&CietControl::HeaterPowerKw));
        assert!(state.switches.contains_key(&CietSwitch::FastForwardOn));
        assert_eq!(state.values_received, 3);

        state.clear_readings();
        assert!(state.signals.is_empty());
        assert!(state.controls.is_empty());
        assert!(state.switches.is_empty());
        assert_eq!(state.values_received, 0);
    }

    /// Verifies the command queue delivers each command exactly once.
    ///
    /// **Methodology.** Queue four commands (`Connect`, `WriteControl`,
    /// `WriteSwitch`, `Disconnect`), drain with `take_commands()`, then drain
    /// again. Pass criterion: the first drain returns all four in order, the
    /// second returns none.
    ///
    /// **Results (2026-07-28).** First drain returned 4 commands in queue order;
    /// second returned 0. Interpretation: a slow worker tick cannot replay a
    /// write, which for a control node would mean sending a set point twice.
    #[test]
    fn commands_are_delivered_exactly_once() {
        let mut state = ClientSharedState::new();
        state.push_command(ClientCommand::Connect {
            endpoint_url: "opc.tcp://host:4840/ciet".to_string(),
        });
        state.push_command(ClientCommand::WriteControl {
            control: CietControl::HeaterPowerKw,
            value: 9.5,
        });
        state.push_command(ClientCommand::WriteSwitch {
            switch: CietSwitch::FastForwardOn,
            value: true,
        });
        state.push_command(ClientCommand::Disconnect);

        let drained = state.take_commands();
        assert_eq!(drained.len(), 4);
        assert!(matches!(drained[0], ClientCommand::Connect { .. }));
        assert!(matches!(drained[3], ClientCommand::Disconnect));
        assert!(state.take_commands().is_empty());
    }

    /// Verifies that trends are bounded, monotonic in simulated time, and reject
    /// non-finite input.
    ///
    /// **Methodology.** Push `TREND_CAPACITY + 500` = 4500 points with strictly
    /// increasing simulated time and check the length caps at 4000 with the
    /// oldest dropped (first abscissa becomes 500.0 for a 1 s step). Then push a
    /// non-advancing abscissa, a `NaN` value and a `NaN` time, and check none is
    /// retained. Pass criterion: length exactly 4000, first point `[500.0,
    /// 500.0]`, and no growth from the three rejected pushes.
    ///
    /// **Results (2026-07-28).** Measured length 4000 after 4500 pushes; first
    /// retained point `[500, 500]`, last `[4499, 4499]`; the three rejected
    /// pushes left the length at 4000. At the client's 250 ms update rate 4000
    /// points is 1000 s of simulated history. Interpretation: a client left
    /// running overnight has bounded memory, and a single `NaN` from the server
    /// cannot blank a plot.
    #[test]
    fn trends_are_bounded_and_reject_non_finite_points() {
        let mut trend = TrendBuffer::new();
        assert!(trend.is_empty());

        for i in 0..(TREND_CAPACITY + 500) {
            let t = i as f64;
            trend.push(t, t);
        }
        assert_eq!(trend.len(), TREND_CAPACITY);
        let points = trend.points();
        assert_eq!(points[0], [500.0, 500.0]);
        assert_eq!(points[points.len() - 1], [4499.0, 4499.0]);

        let before = trend.len();
        trend.push(4499.0, 1.0); // not advancing
        trend.push(4600.0, f64::NAN); // NaN value
        trend.push(f64::NAN, 1.0); // NaN time
        assert_eq!(trend.len(), before);

        trend.clear();
        assert!(trend.is_empty());
    }

    /// Verifies trend points are only appended once simulated time is known, and
    /// are plotted against simulated rather than wall-clock time.
    ///
    /// **Methodology.** Call `append_trend_points()` on a state holding a
    /// heater-power reading but no `SimulationTimeSeconds`, and assert nothing
    /// was appended. Then record `SimulationTimeSeconds = 120.0` and append, and
    /// assert the point's abscissa is 120.0 — the simulator's clock, not this
    /// process's. Pass criterion: 0 points before, then abscissa exactly 120.0.
    ///
    /// **Results (2026-07-28).** 0 points appended without a simulation time;
    /// after recording `SimulationTimeSeconds = 120.0` the heater-power trend
    /// held 1 point at `[120.0, 7.5]`. Interpretation: trends stay correct while
    /// the simulator is in fast-forward or slow motion, where wall-clock time
    /// would distort the abscissa.
    #[test]
    fn trend_abscissa_is_simulated_time_and_requires_it_to_be_known() {
        let mut state = ClientSharedState::new();
        state.record_signal(CietSignal::HeaterPowerKw, sample(7.5));
        state.append_trend_points();
        assert_eq!(state.trends[&CietSignal::HeaterPowerKw].len(), 0);

        state.record_signal(CietSignal::SimulationTimeSeconds, sample(120.0));
        state.append_trend_points();
        let points = state.trends[&CietSignal::HeaterPowerKw].points();
        assert_eq!(points.len(), 1);
        assert_eq!(points[0], [120.0, 7.5]);
    }

    /// Verifies a failed write is retained and flagged rather than swallowed.
    ///
    /// **Methodology.** Record one `Good` and one `BadUserAccessDenied` write
    /// outcome, then assert `has_failed_write()` is true and the log holds both,
    /// newest first. Then overflow the log past `WRITE_LOG_CAPACITY` = 64 and
    /// assert it caps. Pass criterion: `has_failed_write()` true, log length 2
    /// then exactly 64.
    ///
    /// **Results (2026-07-28).** `has_failed_write()` true with 1 of 2 outcomes
    /// bad; log length 2, newest-first ordering confirmed; after 100 further
    /// pushes the length capped at 64. Interpretation: a user whose write was
    /// refused sees it in the UI instead of watching a slider move with no
    /// effect.
    #[test]
    fn failed_writes_are_retained_and_flagged() {
        let mut state = ClientSharedState::new();
        assert!(!state.has_failed_write());

        state.record_write(WriteOutcome {
            target: WriteTarget::Control(CietControl::HeaterPowerKw),
            value: WrittenValue::Numeric(8.0),
            status: StatusCode::Good,
            message: String::new(),
            at: Instant::now(),
        });
        state.record_write(WriteOutcome {
            target: WriteTarget::Switch(CietSwitch::FastForwardOn),
            value: WrittenValue::Boolean(true),
            status: StatusCode::BadUserAccessDenied,
            message: String::new(),
            at: Instant::now(),
        });

        assert_eq!(state.write_log.len(), 2);
        assert!(state.has_failed_write());
        assert!(matches!(
            state.write_log[0].target,
            WriteTarget::Switch(CietSwitch::FastForwardOn)
        ));

        for _ in 0..100 {
            state.record_write(WriteOutcome {
                target: WriteTarget::Control(CietControl::TimestepSeconds),
                value: WrittenValue::Numeric(0.1),
                status: StatusCode::Good,
                message: String::new(),
                at: Instant::now(),
            });
        }
        assert_eq!(state.write_log.len(), WRITE_LOG_CAPACITY);
    }
}
