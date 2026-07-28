//! The CIET v2 OPC-UA server: address space, callbacks, and its own thread.
//!
//! [`spawn_opcua_server_thread`] is the whole entry point. Give it the shared
//! plant state, the shared remote-write mailbox and a [`OpcuaServerConfig`], and
//! it returns an [`OpcuaServerHandle`] describing where clients should connect.
//! Everything else here supports that one call.
//!
//! ```text
//! physics thread ──write outputs, apply_and_clear()──┐
//! GUI thread ─────write controls────────────────────►├─► Arc<RwLock<CietState>>
//!                                                    │        ▲        ▲
//! OPC-UA server thread   read callbacks ─────────────────read──┘        │
//! (own tokio runtime)    200 ms updater ─────────────────read──────────-┘
//!                        write callbacks ──► Arc<RwLock<CietUserControls>>
//! ```
//!
//! ## Threading
//!
//! The server runs on its **own `std::thread` with its own multi-threaded
//! `tokio` runtime**, created inside that thread. It shares nothing with the
//! GUI's event loop, so a stalled repaint cannot stall an OPC-UA client and a
//! headless build (Termux, CI) can serve OPC-UA with no GUI at all. Nothing here
//! panics on failure: a server that cannot start reports why and the simulator
//! carries on without it.
//!
//! ## Reads are live; writes are deferred
//!
//! **Reads** are served straight from [`CietState`] under a read lock, so a
//! client always sees the *effective* value the solver is using — write 1000 kW
//! and read back the 15 kW ceiling.
//!
//! **Writes** do not touch plant state. They are parked in
//! [`super::user_controls::CietUserControls`] and applied by the physics thread
//! at the top of its next timestep, which is where clamping and NaN rejection
//! happen. That removes the lost-update race against the GUI's wholesale
//! `overwrite_state`, and keeps a room full of clients off the plant-state lock.
//!
//! ## Why a periodic push as well as read callbacks
//!
//! A `Read` service call is served by a read callback, so polling is never
//! stale. A **subscription / monitored item**, though, reports when the value
//! stored *in the address space* changes — so a task pushes current values in
//! every [`SUBSCRIPTION_PUSH_INTERVAL`] (200 ms) via `set_values`, which is what
//! makes trending work in a standard OPC-UA client.
//!
//! ## Security: there is none, deliberately
//!
//! `ServerBuilder::new_anonymous` gives one endpoint with **`SecurityPolicy::None`,
//! `MessageSecurityMode::None` and anonymous user tokens**: traffic is
//! unencrypted and unsigned, no client certificate is checked, no credential is
//! required, and therefore **anyone who can reach the TCP port can write every
//! control** — heater power, pump pressure, both branch valves, the timestep.
//!
//! That is a deliberate choice for a throwaway teaching demonstrator, so that
//! "point UaExpert at it and poke the loop" is a ten-second exercise. Hardening
//! (certificates, trust lists, user tokens, audit trails) is explicitly left to
//! security researchers rather than half-done here.
//!
//! The only mitigations you should rely on: every request is **clamped** to its
//! documented envelope on apply and NaN is ignored, so a hostile client can
//! annoy the simulation but not destabilise it; the bind address can be set to
//! loopback ([`OpcuaServerConfig::is_loopback_only`]); and a warning is printed
//! whenever it binds wider. Do not describe this interface as secured, and do not
//! run it on a network you do not control.
//!
//! ## Scope (`RESPONSIBLE_USE.md`)
//!
//! This serves an **offline educational simulator**. It must never be connected
//! to live operational systems, plant systems, safety-critical infrastructure,
//! real-time plant monitoring, or institutional production systems, and its
//! values are not authoritative for any operational, licensing or safety purpose.

use std::net::IpAddr;
use std::sync::Arc;
use std::time::Duration;

use opcua::nodes::{AccessLevel, VariableBuilder};
use opcua::server::diagnostics::NamespaceMetadata;
use opcua::server::node_manager::memory::{simple_node_manager, SimpleNodeManager};
use opcua::server::{
    Server, ServerBuilder, ServerEndpoint, ServerHandle, SubscriptionCache, ANONYMOUS_USER_TOKEN_ID,
};
use opcua::types::{
    DataTypeId, DataValue, DateTime, NodeId, NumericRange, ObjectId, QualifiedName, StatusCode,
    Variant,
};

use super::discovery::{self, MdnsAdvertisement, CIET_MDNS_INSTANCE_PREFIX};
use super::node_map::{
    self, CietControl, CietSignal, CietSwitch, CIET_NAMESPACE_URI, CONTROLS_FOLDER_NAME,
    DEFAULT_OPCUA_PORT, ENDPOINT_PATH, OUTPUTS_FOLDER_NAME,
};
use super::pki_paths::ciet_v2_instance_pki_dir;
use super::state::{CietState, SharedCietState};
use super::user_controls::SharedUserControls;

/// How often current plant values are pushed into the address space so OPC-UA
/// **subscriptions and monitored items** report changes.
///
/// 200 ms of wall-clock time. This is a notification cadence, not a solver
/// timestep — it has no effect whatsoever on the physics, and a polling `Read`
/// is always served live regardless of this interval.
pub const SUBSCRIPTION_PUSH_INTERVAL: Duration = Duration::from_millis(200);

/// Default human-facing OPC-UA application name.
const DEFAULT_APPLICATION_NAME: &str = "CIET Educational Simulator v2";

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
const APPLICATION_URI: &str = "urn:outram-park:ciet-educational-simulator-v2:server";

/// Node id (string identifier) of the folder holding the writable controls.
const CONTROLS_FOLDER_NODE_ID: &str = "CIET.Controls";

/// Node id (string identifier) of the folder holding the read-only outputs.
const OUTPUTS_FOLDER_NODE_ID: &str = "CIET.Outputs";

/// Name given to the CIET node manager inside `async-opcua`.
const NODE_MANAGER_NAME: &str = "ciet";

/// How the CIET v2 OPC-UA server should be brought up.
///
/// All four fields are transport / naming configuration; none is a physical
/// quantity, so none carries units. Use [`Default`] and override what you need.
///
/// ```ignore
/// // loopback only, no announcement -- the safe default for a shared network
/// let config = OpcuaServerConfig {
///     bind_address: "127.0.0.1".to_owned(),
///     advertise_over_mdns: false,
///     ..Default::default()
/// };
/// ```
#[derive(Debug, Clone)]
pub struct OpcuaServerConfig {
    /// Local address to bind the listening socket to.
    ///
    /// `"0.0.0.0"` (the default) accepts connections on every IPv4 interface, so
    /// other machines on the same network can connect — which is the point of
    /// the demo, and also the case in which **anyone who can reach the port can
    /// write every control** (see the module docs). `"127.0.0.1"` keeps the
    /// interface on this machine; [`is_loopback_only`](Self::is_loopback_only)
    /// reports which of the two you have.
    pub bind_address: String,

    /// TCP port to listen on.
    ///
    /// Defaults to [`node_map::DEFAULT_OPCUA_PORT`] (4840), the IANA-registered
    /// `opcua-tcp` port, which is where OPC-UA tooling looks first. Use
    /// something above 1024 and unregistered if 4840 is taken.
    pub port: u16,

    /// Whether to announce the running server on the local link over mDNS /
    /// DNS-SD, so the bundled demo client can find it without a typed URL.
    ///
    /// Defaults to `true`. Announcement is cooperative and one-way — see
    /// [`super::discovery`], which explains both that this never scans anything
    /// and that many campus/enterprise networks block it outright. A failure to
    /// announce is logged and otherwise ignored; the server still runs.
    pub advertise_over_mdns: bool,

    /// OPC-UA `ApplicationName`, shown by clients in their server list and in
    /// the endpoint description.
    ///
    /// Defaults to `"CIET Educational Simulator v2"`. Also used, sanitised, as
    /// the mDNS instance name.
    pub application_name: String,
}

impl Default for OpcuaServerConfig {
    /// Bind every interface on the standard OPC-UA port, announce over mDNS,
    /// and call ourselves `"CIET Educational Simulator v2"`.
    ///
    /// Binding all interfaces is the default because the demonstration is
    /// "connect from the phone in your hand". It is also the configuration that
    /// exposes every control to the network, which is why the simulator prints a
    /// warning in that case.
    fn default() -> Self {
        Self {
            bind_address: "0.0.0.0".to_owned(),
            port: DEFAULT_OPCUA_PORT,
            advertise_over_mdns: true,
            application_name: DEFAULT_APPLICATION_NAME.to_owned(),
        }
    }
}

impl OpcuaServerConfig {
    /// `true` if [`bind_address`](Self::bind_address) reaches this machine only.
    ///
    /// Recognises the loopback literals (`127.0.0.1`, anything in `127/8`,
    /// `::1`) and the host name `localhost`. Anything else — including the
    /// all-interfaces wildcards `0.0.0.0` and `::` — is *not* loopback-only, and
    /// means remote clients can write every control.
    ///
    /// An address that does not parse as an IP and is not `localhost` is treated
    /// as **not** loopback-only, which is the conservative answer: it makes the
    /// simulator print its warning rather than stay quiet about an exposure it
    /// could not rule out.
    pub fn is_loopback_only(&self) -> bool {
        let address = self.bind_address.trim();
        if address.eq_ignore_ascii_case("localhost") {
            return true;
        }
        // Accept a bracketed IPv6 literal, as a URL would write it.
        let unbracketed = address
            .strip_prefix('[')
            .and_then(|rest| rest.strip_suffix(']'))
            .unwrap_or(address);
        match unbracketed.parse::<IpAddr>() {
            Ok(ip) => ip.is_loopback(),
            Err(_) => false,
        }
    }

    /// `true` if [`bind_address`](Self::bind_address) is an all-interfaces
    /// wildcard, so the endpoint is reachable from other machines.
    fn binds_all_interfaces(&self) -> bool {
        let address = self.bind_address.trim();
        let unbracketed = address
            .strip_prefix('[')
            .and_then(|rest| rest.strip_suffix(']'))
            .unwrap_or(address);
        match unbracketed.parse::<IpAddr>() {
            Ok(ip) => ip.is_unspecified(),
            // An empty host is how some configs spell "everything".
            Err(_) => unbracketed.is_empty() || unbracketed == "*",
        }
    }
}

/// Where a client should point, and what it will find when it gets there.
///
/// Produced by [`spawn_opcua_server_thread`] and displayed by the simulator's
/// "how to connect" panel. Pure connection metadata — no physical quantities, no
/// units.
#[derive(Debug, Clone)]
pub struct OpcuaEndpointInfo {
    /// Endpoint URL that always works from this machine, e.g.
    /// `"opc.tcp://127.0.0.1:4840/ciet"`.
    pub loopback_url: String,

    /// Endpoint URL other machines on the same network should use, e.g.
    /// `"opc.tcp://192.168.1.42:4840/ciet"`.
    ///
    /// `None` when this machine's LAN address could not be determined (no
    /// non-loopback interface, or the query failed). It is also **not** a promise
    /// of reachability: a network with client isolation will refuse the
    /// connection anyway — see [`super::discovery`].
    pub lan_url: Option<String>,

    /// Whether the listening socket was bound to every interface rather than to
    /// one specific address.
    ///
    /// When `false`, [`primary_url`](Self::primary_url) reports the loopback URL,
    /// because a server bound to `127.0.0.1` is genuinely not reachable from
    /// elsewhere no matter what LAN address the machine holds.
    pub bound_to_all_interfaces: bool,

    /// Namespace URI every CIET variable lives in, i.e.
    /// [`node_map::CIET_NAMESPACE_URI`].
    ///
    /// A client resolves this to a running namespace *index* (usually 2) from the
    /// server's namespace array; the index is not stable across versions and must
    /// not be hard-coded.
    pub namespace_uri: &'static str,

    /// Number of CIET variables served, i.e. [`node_map::total_node_count`].
    pub node_count: usize,

    /// The PKI directory, ready to print. See [`super::pki_paths`] — it holds a
    /// self-signed keypair and nothing sensitive.
    pub pki_dir_display: String,
}

impl OpcuaEndpointInfo {
    /// The URL to show a user first.
    ///
    /// The LAN URL when the server is bound to all interfaces *and* a LAN
    /// address is known — that is the address another device needs. Otherwise
    /// the loopback URL, which is then the only one that can work.
    pub fn primary_url(&self) -> &str {
        match (&self.lan_url, self.bound_to_all_interfaces) {
            (Some(lan_url), true) => lan_url.as_str(),
            _ => self.loopback_url.as_str(),
        }
    }
}

/// A running CIET v2 OPC-UA server.
///
/// Holding this value keeps the server up: it owns the `async-opcua` server
/// handle and, when mDNS announcement is enabled, the announcement guard. No
/// lifetime parameters — everything is owned or shared through `Arc`.
///
/// Dropping the handle withdraws the mDNS announcement (its guard's `Drop`) but
/// does **not** stop the server; call [`shutdown`](Self::shutdown) for that.
/// That split is deliberate: the simulator's GUI keeps the handle for the whole
/// process lifetime and never wants an accidental move to kill the interface.
pub struct OpcuaServerHandle {
    /// Where clients connect, cached at start-up.
    endpoint_info: OpcuaEndpointInfo,
    /// `async-opcua`'s own handle, used to cancel the server.
    server_handle: ServerHandle,
    /// Live mDNS announcement, if one was requested and succeeded. Withdrawn on
    /// drop.
    mdns_advertisement: Option<MdnsAdvertisement>,
}

impl OpcuaServerHandle {
    /// Where clients should connect, and what they will find.
    pub fn endpoint_info(&self) -> &OpcuaEndpointInfo {
        &self.endpoint_info
    }

    /// Ask the server to stop.
    ///
    /// Signals cancellation and returns immediately; the server thread finishes
    /// its current work, closes sessions and exits shortly afterwards, and its
    /// tokio runtime (and the 200 ms updater task with it) is dropped. Calling
    /// this more than once is harmless.
    ///
    /// The mDNS announcement is withdrawn when the handle is *dropped*, not
    /// here, so a caller that shuts the server down and keeps the handle will
    /// still be announcing a dead endpoint. Drop the handle too.
    pub fn shutdown(&self) {
        self.server_handle.cancel();
    }
}

impl std::fmt::Debug for OpcuaServerHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OpcuaServerHandle")
            .field("endpoint_info", &self.endpoint_info)
            .field("mdns_announced", &self.mdns_advertisement.is_some())
            .finish_non_exhaustive()
    }
}

/// Things that can stop the CIET v2 OPC-UA server from starting.
///
/// Every one of these is a start-up failure. A failure to *announce* over mDNS
/// is not in this list on purpose: it is logged and the server runs anyway,
/// because discovery is a convenience and the endpoint still works with a typed
/// URL.
#[derive(Debug, thiserror::Error)]
pub enum OpcuaServerError {
    /// `async-opcua` rejected the configuration, or could not read/write its
    /// certificate store. The string is its own message.
    #[error("the OPC-UA server configuration was rejected: {0}")]
    Build(String),

    /// The CIET node manager was not present on the built server. This would
    /// mean `with_node_manager` did not take effect, i.e. an `async-opcua`
    /// version change rather than a user error.
    #[error("the CIET node manager is missing from the built server (async-opcua API change?)")]
    NodeManagerUnavailable,

    /// [`node_map::CIET_NAMESPACE_URI`] was not registered, so no CIET node id
    /// could be formed. Same class of cause as
    /// [`NodeManagerUnavailable`](Self::NodeManagerUnavailable).
    #[error("the CIET namespace {CIET_NAMESPACE_URI} was not registered by the server")]
    NamespaceUnavailable,

    /// A folder or variable could not be inserted into the address space —
    /// almost always a duplicate node id, which
    /// `node_map::tests::node_identifiers_are_unique` is there to prevent.
    #[error("could not add {0} to the OPC-UA address space (duplicate node id?)")]
    AddressSpace(String),

    /// The operating system refused to start the server thread.
    #[error("could not start the OPC-UA server thread: {0}")]
    ThreadSpawn(#[from] std::io::Error),

    /// The server thread could not create its own `tokio` runtime — normally a
    /// thread or file-descriptor limit.
    #[error("could not create the OPC-UA server runtime: {0}")]
    Runtime(String),

    /// The server thread ended before reporting whether construction succeeded,
    /// which means it panicked. The panic message itself is on stderr.
    #[error("the OPC-UA server thread ended during start-up without reporting (see stderr)")]
    StartupAborted,
}

/// One CIET variable, tagged by which node-map enum it came from.
///
/// Enum dispatch rather than a trait object, per the workspace Rust design
/// rules: a fourth kind of variable becomes a compile error at every `match`
/// instead of a runtime surprise.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CietNode {
    /// A read-only output, served as an OPC-UA `Double`.
    Signal(CietSignal),
    /// A writable continuous control, served as an OPC-UA `Double`.
    Control(CietControl),
    /// A writable on/off control, served as an OPC-UA `Boolean`.
    Switch(CietSwitch),
}

impl CietNode {
    /// Every CIET variable, outputs first, in address-space order.
    fn all() -> impl Iterator<Item = Self> {
        CietSignal::ALL
            .iter()
            .map(|s| Self::Signal(*s))
            .chain(CietControl::ALL.iter().map(|c| Self::Control(*c)))
            .chain(CietSwitch::ALL.iter().map(|s| Self::Switch(*s)))
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

    /// Read this variable out of a plant-state snapshot as an OPC-UA
    /// `DataValue`, stamped with `at` as both source and server timestamp.
    fn data_value(&self, state: &CietState, at: DateTime) -> DataValue {
        match self {
            Self::Signal(signal) => DataValue::new_at(signal.read(state), at),
            Self::Control(control) => DataValue::new_at(control.read(state), at),
            Self::Switch(switch) => DataValue::new_at(switch.read(state), at),
        }
    }
}

/// Start the CIET v2 OPC-UA server on its own thread.
///
/// Spawns a dedicated `std::thread` with its own multi-threaded `tokio` runtime;
/// that thread builds the server, populates its address space from
/// [`node_map`], wires the callbacks, and serves connections. This function
/// blocks only until the thread reports whether construction succeeded (a
/// certificate load and 36 node insertions), then optionally announces over
/// mDNS and returns.
///
/// **The build must happen on the server thread**, not here: `ServerBuilder::build`
/// spawns a tokio task internally (`ServerStatusWrapper::new`) and panics with
/// "there is no reactor running" outside a runtime. The result therefore comes
/// back over a `std::sync::mpsc` channel, which is what keeps this signature
/// synchronous so a GUI can call it with no `async` in sight.
///
/// `state` is read to serve reads; `user_controls` receives writes, which the
/// physics thread applies on its next timestep (see the module docs). Neither
/// lock is held across an `await`.
///
/// # What lands in the address space
///
/// Under `Objects`, two folders: `Outputs` holds one read-only `Double` per
/// [`CietSignal`]; `Controls` holds one writable `Double` per [`CietControl`]
/// and one writable `Boolean` per [`CietSwitch`]. Node ids are
/// `ns=<index>;s=<node_identifier()>`, with display names and unit-bearing
/// descriptions from the node map. The namespace index is assigned at start-up
/// (2 in this configuration) — clients must resolve it, never hard-code it.
///
/// # Security
///
/// There is none, deliberately: `SecurityPolicy::None` and anonymous access, so
/// **anyone who can reach the port can write every control**. Requests are still
/// clamped on apply. Read the module documentation before running this anywhere
/// but a bench.
///
/// # Errors
///
/// See [`OpcuaServerError`]. A failure to announce over mDNS is *not* an error —
/// it is reported and the server runs regardless.
pub fn spawn_opcua_server_thread(
    state: SharedCietState,
    user_controls: SharedUserControls,
    config: OpcuaServerConfig,
) -> Result<OpcuaServerHandle, OpcuaServerError> {
    // Isolate the certificate store per port. Two servers that can coexist on
    // one machine necessarily differ in port, so this makes concurrent instances
    // -- notably the headless CIET tests -- stop racing on one shared keypair.
    let pki_dir = ciet_v2_instance_pki_dir(&format!("port-{}", config.port));

    // The thread owns the runtime, the server and the address space, so it is
    // independent of any GUI event loop. The `JoinHandle` is deliberately
    // dropped (detached): the simulator's lifetime is the process lifetime, and
    // `shutdown()` on the returned handle is how the thread is asked to stop.
    let (report_sender, report_receiver) = std::sync::mpsc::channel();
    let thread_state = Arc::clone(&state);
    let thread_user_controls = Arc::clone(&user_controls);
    let thread_config = config.clone();
    let thread_pki_dir = pki_dir.clone();
    std::thread::Builder::new()
        .name("ciet-v2-opcua".to_owned())
        .spawn(move || {
            run_server_thread(
                thread_state,
                thread_user_controls,
                thread_config,
                thread_pki_dir,
                report_sender,
            );
        })?;

    // Wait for the thread to say whether the server was constructed. A `RecvError`
    // means the thread ended without reporting, which can only happen if it
    // panicked -- there is no code path that returns without sending.
    let server_handle = report_receiver
        .recv()
        .map_err(|_| OpcuaServerError::StartupAborted)??;

    let endpoint_info = build_endpoint_info(&config, &pki_dir);
    print_startup_banner(&config, &endpoint_info);

    let mdns_advertisement = if config.advertise_over_mdns {
        let instance_name = if config.application_name.trim().is_empty() {
            CIET_MDNS_INSTANCE_PREFIX
        } else {
            config.application_name.as_str()
        };
        match discovery::advertise_simulator(config.port, instance_name) {
            Ok(advertisement) => {
                println!(
                    "CIET v2 OPC-UA: announced over mDNS as {}",
                    advertisement.instance_name()
                );
                Some(advertisement)
            }
            Err(error) => {
                // Not fatal: the endpoint still works with a typed URL.
                eprintln!("CIET v2 OPC-UA: mDNS announcement unavailable -- {error}");
                None
            }
        }
    } else {
        None
    };

    Ok(OpcuaServerHandle {
        endpoint_info,
        server_handle,
        mdns_advertisement,
    })
}

/// Populate the node manager's address space with the two folders and every
/// CIET variable, returning the node ids paired with what they read.
///
/// The returned vector is the single list the read callbacks, the write
/// callbacks and the periodic push all iterate, so a variable cannot be present
/// in one and missing from another.
fn build_address_space(
    node_manager: &SimpleNodeManager,
    namespace_index: u16,
    state: &SharedCietState,
) -> Result<Vec<(NodeId, CietNode)>, OpcuaServerError> {
    let objects_folder: NodeId = ObjectId::ObjectsFolder.into();
    let outputs_folder = NodeId::new(namespace_index, OUTPUTS_FOLDER_NODE_ID);
    let controls_folder = NodeId::new(namespace_index, CONTROLS_FOLDER_NODE_ID);

    // Take one snapshot so every variable's initial value is consistent, and so
    // the lock is held once rather than 36 times. A poisoned lock means the
    // physics thread panicked; fall back to defaults rather than propagating a
    // panic into the OPC-UA layer.
    let snapshot = match state.read() {
        Ok(guard) => guard.clone(),
        Err(poisoned) => {
            eprintln!(
                "CIET v2 OPC-UA: plant state lock was poisoned at start-up; \
                 initialising the address space from the poisoned snapshot"
            );
            poisoned.into_inner().clone()
        }
    };
    let now = DateTime::now();

    let mut address_space = node_manager.address_space().write();
    let mut nodes: Vec<(NodeId, CietNode)> = Vec::with_capacity(node_map::total_node_count());

    if !address_space.add_folder(
        &outputs_folder,
        QualifiedName::new(namespace_index, OUTPUTS_FOLDER_NAME),
        OUTPUTS_FOLDER_NAME,
        &objects_folder,
    ) {
        return Err(OpcuaServerError::AddressSpace(format!(
            "the {OUTPUTS_FOLDER_NAME} folder"
        )));
    }
    if !address_space.add_folder(
        &controls_folder,
        QualifiedName::new(namespace_index, CONTROLS_FOLDER_NAME),
        CONTROLS_FOLDER_NAME,
        &objects_folder,
    ) {
        return Err(OpcuaServerError::AddressSpace(format!(
            "the {CONTROLS_FOLDER_NAME} folder"
        )));
    }

    for node in CietNode::all() {
        let node_id = NodeId::new(namespace_index, node.node_identifier());
        let parent = match node {
            CietNode::Signal(_) => &outputs_folder,
            CietNode::Control(_) | CietNode::Switch(_) => &controls_folder,
        };

        let inserted = VariableBuilder::new(
            &node_id,
            QualifiedName::new(namespace_index, node.browse_name()),
            node.display_name(),
        )
        .description(node.description())
        .data_type(node.data_type())
        // Both the access level and the *user* access level must be set:
        // `async-opcua` checks the user access level on write, so setting only
        // the former gives a silent `BadUserAccessDenied` on every control.
        .access_level(node.access_level())
        .user_access_level(node.access_level())
        .value(
            node.data_value(&snapshot, now)
                .value
                .unwrap_or(Variant::Empty),
        )
        .organized_by(parent.clone())
        .insert(&mut *address_space);

        if !inserted {
            return Err(OpcuaServerError::AddressSpace(format!(
                "variable {node_id}"
            )));
        }
        nodes.push((node_id, node));
    }

    Ok(nodes)
}

/// Wire every node's read callback, and a write callback for every writable one.
///
/// Read callbacks take a **read** lock on plant state so several clients and the
/// GUI read concurrently; write callbacks take a **write** lock on the request
/// mailbox for one assignment. Neither holds a lock across an `await`.
fn register_callbacks(
    node_manager: &SimpleNodeManager,
    state: &SharedCietState,
    user_controls: &SharedUserControls,
    nodes: &[(NodeId, CietNode)],
) {
    let inner = node_manager.inner();

    for (node_id, node) in nodes {
        let node = *node;

        let read_state = Arc::clone(state);
        inner.add_read_callback(
            node_id.clone(),
            move |_index_range, _timestamps, _max_age| {
                let guard = read_state
                    .read()
                    .map_err(|_| StatusCode::BadInternalError)?;
                Ok(node.data_value(&guard, DateTime::now()))
            },
        );

        match node {
            // Outputs are read-only: no write callback, and the address space
            // already refuses the write via the access level.
            CietNode::Signal(_) => {}
            CietNode::Control(control) => {
                let requests = Arc::clone(user_controls);
                inner.add_write_callback(node_id.clone(), move |value, _index_range| {
                    record_control_request(&requests, control, value)
                });
            }
            CietNode::Switch(switch) => {
                let requests = Arc::clone(user_controls);
                inner.add_write_callback(node_id.clone(), move |value, _index_range| {
                    record_switch_request(&requests, switch, value)
                });
            }
        }
    }
}

/// Record a client's write to a continuous control as a *pending request*.
///
/// The write is **not** applied to [`CietState`] here; it is parked in
/// [`super::user_controls::CietUserControls`] and applied by the physics thread
/// on its next timestep, which is where clamping and NaN rejection happen (see
/// the module docs for why). The client therefore reads back the *effective*
/// value: write 1000 kW, read back the 15 kW ceiling.
///
/// Returns `BadTypeMismatch` for a non-numeric payload, `BadNothingToDo` for no
/// payload, and `BadInternalError` if the request lock is poisoned.
fn record_control_request(
    user_controls: &SharedUserControls,
    control: CietControl,
    value: DataValue,
) -> StatusCode {
    let Some(variant) = value.value else {
        return StatusCode::BadNothingToDo;
    };
    let Some(number) = variant_as_f64(&variant) else {
        return StatusCode::BadTypeMismatch;
    };
    let Ok(mut requests) = user_controls.write() else {
        return StatusCode::BadInternalError;
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
fn record_switch_request(
    user_controls: &SharedUserControls,
    switch: CietSwitch,
    value: DataValue,
) -> StatusCode {
    let Some(variant) = value.value else {
        return StatusCode::BadNothingToDo;
    };
    let Variant::Boolean(flag) = variant else {
        return StatusCode::BadTypeMismatch;
    };
    let Ok(mut requests) = user_controls.write() else {
        return StatusCode::BadInternalError;
    };
    requests.request_switch(switch, flag);
    StatusCode::Good
}

/// Interpret an OPC-UA `Variant` as an `f64`, or `None` if it is not a number.
///
/// `Double` is every control's declared type, but real clients also send `Float`
/// and the integer types (a spin box bound to an `i32`, a script writing `5`
/// rather than `5.0`), so those are accepted and widened — exactly, at any
/// magnitude a CIET control accepts. Strings, booleans and structures are
/// refused.
fn variant_as_f64(variant: &Variant) -> Option<f64> {
    match variant {
        Variant::Double(value) => Some(*value),
        Variant::Float(value) => Some(*value as f64),
        Variant::SByte(value) => Some(*value as f64),
        Variant::Byte(value) => Some(*value as f64),
        Variant::Int16(value) => Some(*value as f64),
        Variant::UInt16(value) => Some(*value as f64),
        Variant::Int32(value) => Some(*value as f64),
        Variant::UInt32(value) => Some(*value as f64),
        Variant::Int64(value) => Some(*value as f64),
        Variant::UInt64(value) => Some(*value as f64),
        _ => None,
    }
}

/// Build and run the server to completion on this thread, in a runtime it owns.
///
/// Reports the outcome of construction back over `report` — exactly once, on
/// every code path — then serves connections until cancelled. Never panics: a
/// runtime that will not build, a rejected configuration, or a server that exits
/// with an error is reported and the thread returns, leaving the simulator
/// running without its OPC-UA interface.
fn run_server_thread(
    state: SharedCietState,
    user_controls: SharedUserControls,
    config: OpcuaServerConfig,
    pki_dir: std::path::PathBuf,
    report: std::sync::mpsc::Sender<Result<ServerHandle, OpcuaServerError>>,
) {
    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .thread_name("ciet-v2-opcua-rt")
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(error) => {
            let _ = report.send(Err(OpcuaServerError::Runtime(error.to_string())));
            return;
        }
    };

    runtime.block_on(async move {
        // Construction must happen inside the runtime: `ServerBuilder::build`
        // spawns a tokio task for the server-status wrapper.
        let prepared = match prepare_server(&state, &user_controls, &config, &pki_dir) {
            Ok(prepared) => prepared,
            Err(error) => {
                let _ = report.send(Err(error));
                return;
            }
        };
        let PreparedServer {
            server,
            server_handle,
            node_manager,
            nodes,
        } = prepared;

        let subscriptions = Arc::clone(server_handle.subscriptions());
        if report.send(Ok(server_handle)).is_err() {
            // The caller gave up on us before we finished building; nothing is
            // holding a handle, so there is no point serving.
            return;
        }

        // Pushes live values into the address space so subscriptions/monitored
        // items report changes. Aborted when the server stops.
        let updater_state = Arc::clone(&state);
        let updater = tokio::spawn(async move {
            let mut ticker = tokio::time::interval(SUBSCRIPTION_PUSH_INTERVAL);
            loop {
                ticker.tick().await;
                push_values_into_address_space(
                    &node_manager,
                    &subscriptions,
                    &updater_state,
                    &nodes,
                );
            }
        });

        match server.run().await {
            Ok(()) => println!("CIET v2 OPC-UA: server stopped"),
            Err(error) => eprintln!("CIET v2 OPC-UA: server stopped with an error ({error})"),
        }

        updater.abort();
    });
}

/// A built-but-not-yet-running server, with everything the serving loop needs.
struct PreparedServer {
    /// The server itself, ready for `run()`.
    server: Server,
    /// Handle used to cancel it and to reach its subscription cache.
    server_handle: ServerHandle,
    /// The CIET node manager, used by the periodic push.
    node_manager: Arc<SimpleNodeManager>,
    /// Every CIET node id paired with what it reads.
    nodes: Arc<Vec<(NodeId, CietNode)>>,
}

/// Build the OPC-UA server, its address space and its callbacks.
///
/// **Must be called from inside a tokio runtime**: `ServerBuilder::build` spawns
/// a task internally and panics with "there is no reactor running" otherwise.
fn prepare_server(
    state: &SharedCietState,
    user_controls: &SharedUserControls,
    config: &OpcuaServerConfig,
    pki_dir: &std::path::Path,
) -> Result<PreparedServer, OpcuaServerError> {
    let (server, server_handle) = ServerBuilder::new_anonymous(config.application_name.clone())
        // Per-instance application URI. It MUST differ from CIET_NAMESPACE_URI
        // (see APPLICATION_URI), and making it differ per port also stops two
        // simulators on one machine presenting the same application identity.
        .application_uri(format!("{APPLICATION_URI}:{}", config.port))
        .product_uri(APPLICATION_URI)
        // A self-signed keypair, written under `own/` and `private/` in the PKI
        // directory on first run. It authenticates nothing here -- no endpoint
        // uses it -- but async-opcua expects the store to exist.
        .create_sample_keypair(true)
        .certificate_path("own/cert.der")
        .private_key_path("private/private.pem")
        .pki_dir(pki_dir.to_path_buf())
        .host(config.bind_address.clone())
        .port(config.port)
        // Replace the "/" endpoint `new_anonymous` installs with the CIET one,
        // so the URL is `opc.tcp://<host>:<port>/ciet`. Still SecurityPolicy
        // None + anonymous.
        .add_endpoint(
            "none",
            ServerEndpoint::new_none(ENDPOINT_PATH, &[ANONYMOUS_USER_TOKEN_ID.to_owned()]),
        )
        .discovery_urls(vec![ENDPOINT_PATH.to_owned()])
        .with_node_manager(simple_node_manager(
            NamespaceMetadata {
                namespace_uri: CIET_NAMESPACE_URI.to_owned(),
                ..Default::default()
            },
            NODE_MANAGER_NAME,
        ))
        .build()
        .map_err(OpcuaServerError::Build)?;

    let namespace_index = server_handle
        .get_namespace_index(CIET_NAMESPACE_URI)
        .ok_or(OpcuaServerError::NamespaceUnavailable)?;

    let node_manager = server_handle
        .node_managers()
        .get_of_type::<SimpleNodeManager>()
        .ok_or(OpcuaServerError::NodeManagerUnavailable)?;

    let nodes = Arc::new(build_address_space(&node_manager, namespace_index, state)?);
    register_callbacks(&node_manager, state, user_controls, &nodes);

    Ok(PreparedServer {
        server,
        server_handle,
        node_manager,
        nodes,
    })
}

/// Copy current plant state into the address space, notifying subscriptions.
///
/// One read lock, released before `set_values`, so a slow notification fan-out
/// never blocks the physics thread's next write.
fn push_values_into_address_space(
    node_manager: &SimpleNodeManager,
    subscriptions: &SubscriptionCache,
    state: &SharedCietState,
    nodes: &[(NodeId, CietNode)],
) {
    let snapshot = match state.read() {
        Ok(guard) => guard.clone(),
        // A poisoned lock means the physics thread panicked. Leave the last
        // published values in place rather than publishing nonsense.
        Err(_) => return,
    };
    let now = DateTime::now();

    let updates: Vec<(&NodeId, Option<&NumericRange>, DataValue)> = nodes
        .iter()
        .map(|(node_id, node)| (node_id, None, node.data_value(&snapshot, now)))
        .collect();

    if let Err(status) = node_manager.set_values(subscriptions, updates.into_iter()) {
        eprintln!("CIET v2 OPC-UA: could not publish plant values ({status})");
    }
}

/// Assemble the "how to connect" description for a configuration.
///
/// The LAN address comes from `local_ip_address::local_ip` (this machine's
/// primary non-loopback address); a failure there just leaves
/// [`OpcuaEndpointInfo::lan_url`] as `None`.
fn build_endpoint_info(config: &OpcuaServerConfig, pki_dir: &std::path::Path) -> OpcuaEndpointInfo {
    let loopback_url = format!("opc.tcp://127.0.0.1:{}{ENDPOINT_PATH}", config.port);

    let lan_url = match local_ip_address::local_ip() {
        Ok(IpAddr::V4(address)) => Some(format!(
            "opc.tcp://{address}:{}{ENDPOINT_PATH}",
            config.port
        )),
        Ok(IpAddr::V6(address)) => Some(format!(
            "opc.tcp://[{address}]:{}{ENDPOINT_PATH}",
            config.port
        )),
        Err(_) => None,
    };

    OpcuaEndpointInfo {
        loopback_url,
        lan_url,
        bound_to_all_interfaces: config.binds_all_interfaces(),
        namespace_uri: CIET_NAMESPACE_URI,
        node_count: node_map::total_node_count(),
        pki_dir_display: pki_dir.display().to_string(),
    }
}

/// Print one start-up line, plus a no-security warning when the endpoint is
/// reachable from other machines.
///
/// Deliberately terse: the CIET v2 binary prints the full connection banner, so
/// this only guarantees a headless or embedding caller still sees the one fact
/// that matters — that there is no security.
fn print_startup_banner(config: &OpcuaServerConfig, endpoint_info: &OpcuaEndpointInfo) {
    println!(
        "CIET v2 OPC-UA: serving {} variables at {}",
        endpoint_info.node_count,
        endpoint_info.primary_url()
    );

    if !config.is_loopback_only() {
        println!(
            "CIET v2 OPC-UA: WARNING -- bound to {}, NO security (SecurityPolicy::None, \
             anonymous): anyone who can reach port {} can write every control.",
            config.bind_address, config.port
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Verifies the default configuration is the documented one — it decides
    /// whether a freshly built simulator is network-reachable, and therefore
    /// whether every control is exposed.
    ///
    /// **Methodology.** Compare all four fields of `OpcuaServerConfig::default()`
    /// against the documented defaults, plus the derived predicates. Pass
    /// criterion: `0.0.0.0`, [`DEFAULT_OPCUA_PORT`], mDNS on, the v2 application
    /// name, `is_loopback_only() == false`, `binds_all_interfaces() == true`. No
    /// socket is opened.
    ///
    /// **Results (2026-07-28).** `"0.0.0.0"`, 4840, `true`,
    /// `"CIET Educational Simulator v2"`, `false`, `true` — 6/6 as documented.
    /// Interpretation: the default is the network-reachable configuration, i.e.
    /// exactly the case where the no-security warning fires.
    #[test]
    fn default_config_binds_all_interfaces_on_the_standard_port() {
        let config = OpcuaServerConfig::default();

        assert_eq!(config.bind_address, "0.0.0.0");
        assert_eq!(config.port, DEFAULT_OPCUA_PORT);
        assert_eq!(config.port, 4840, "DEFAULT_OPCUA_PORT should be 4840");
        assert!(config.advertise_over_mdns);
        assert_eq!(config.application_name, "CIET Educational Simulator v2");

        assert!(
            !config.is_loopback_only(),
            "0.0.0.0 must not be reported as loopback-only"
        );
        assert!(
            config.binds_all_interfaces(),
            "0.0.0.0 must be reported as all-interfaces"
        );
    }

    /// Verifies [`OpcuaServerConfig::is_loopback_only`] over the address forms a
    /// user might type. A false "yes" here is a silent network exposure.
    ///
    /// **Methodology.** Evaluate the predicate on eleven bind addresses: IPv4
    /// loopback (including a non-`.1` address in `127/8`), IPv6 loopback (bare
    /// and bracketed), `localhost` in two cases, both wildcards, a LAN address,
    /// and an unparseable string. Pass criterion: `true` for exactly the loopback
    /// forms; `false` otherwise, including the unparseable input where `false` is
    /// the conservative answer that produces a warning. No socket is opened.
    ///
    /// **Results (2026-07-28).** true for `127.0.0.1`, `127.0.0.53`, `::1`,
    /// `[::1]`, `localhost`, `LOCALHOST`; false for `0.0.0.0`, `::`,
    /// `192.168.1.42`, `not-an-address`, `""` — 11/11 as documented.
    #[test]
    fn is_loopback_only_recognises_every_loopback_form() {
        let loopback = [
            "127.0.0.1",
            "127.0.0.53",
            "::1",
            "[::1]",
            "localhost",
            "LOCALHOST",
        ];
        let not_loopback = ["0.0.0.0", "::", "192.168.1.42", "not-an-address", ""];

        for address in loopback {
            let config = OpcuaServerConfig {
                bind_address: address.to_owned(),
                ..Default::default()
            };
            assert!(
                config.is_loopback_only(),
                "{address} should be loopback-only"
            );
        }

        for address in not_loopback {
            let config = OpcuaServerConfig {
                bind_address: address.to_owned(),
                ..Default::default()
            };
            assert!(
                !config.is_loopback_only(),
                "{address} should NOT be loopback-only"
            );
        }
    }

    /// Verifies [`OpcuaEndpointInfo::primary_url`] picks a URL that can actually
    /// work — this is the string the GUI shows and a user types on another device.
    ///
    /// **Methodology.** All four combinations of `bound_to_all_interfaces` and
    /// `lan_url`. Pass criterion: the LAN URL only when bound to all interfaces
    /// *and* known; the loopback URL otherwise, since a `127.0.0.1` bind is not
    /// reachable at the LAN address even though that address exists. No socket.
    ///
    /// **Results (2026-07-28).** (all, known) →
    /// `opc.tcp://192.168.1.42:4840/ciet`; the other three →
    /// `opc.tcp://127.0.0.1:4840/ciet`. 4/4 as documented. Interpretation: the
    /// displayed URL never over-promises reachability.
    #[test]
    fn primary_url_prefers_a_reachable_address() {
        let loopback_url = "opc.tcp://127.0.0.1:4840/ciet".to_owned();
        let lan_url = "opc.tcp://192.168.1.42:4840/ciet".to_owned();

        let make = |bound_to_all_interfaces: bool, lan: Option<String>| OpcuaEndpointInfo {
            loopback_url: loopback_url.clone(),
            lan_url: lan,
            bound_to_all_interfaces,
            namespace_uri: CIET_NAMESPACE_URI,
            node_count: node_map::total_node_count(),
            pki_dir_display: "/tmp/does-not-matter".to_owned(),
        };

        assert_eq!(
            make(true, Some(lan_url.clone())).primary_url(),
            lan_url,
            "all interfaces + known LAN address should advertise the LAN URL"
        );
        assert_eq!(
            make(true, None).primary_url(),
            loopback_url,
            "all interfaces + unknown LAN address should fall back to loopback"
        );
        assert_eq!(
            make(false, Some(lan_url.clone())).primary_url(),
            loopback_url,
            "a loopback bind is not reachable at the LAN address"
        );
        assert_eq!(make(false, None).primary_url(), loopback_url);
    }

    /// Verifies that the write path accepts the numeric `Variant`s real clients
    /// send, refuses everything else, and parks accepted writes as *pending
    /// requests* rather than touching plant state — the property the whole
    /// deferred-apply design rests on.
    ///
    /// **Methodology.** [`variant_as_f64`] over ten numeric and three
    /// non-numeric variants (exact value vs `None`). Then
    /// [`record_control_request`] / [`record_switch_request`] against a fresh
    /// request mailbox and a default [`CietState`], checking that an accepted
    /// write appears in `pending_control` while plant state stays **unchanged**
    /// until `apply_and_clear`, that a wrong-typed write returns
    /// `BadTypeMismatch` and records nothing, and that an out-of-range 1000 kW
    /// request applies as the 15 kW ceiling. No socket, no server, no network.
    ///
    /// **Results (2026-07-28).** All ten numeric variants converted exactly
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
        let status = record_control_request(
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
        let status = record_control_request(
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
            record_switch_request(
                &requests,
                CietSwitch::CtahBranchBlocked,
                DataValue::new_now(true)
            ),
            StatusCode::Good
        );
        assert_eq!(
            record_switch_request(
                &requests,
                CietSwitch::CtahBranchBlocked,
                DataValue::new_now(1.0f64)
            ),
            StatusCode::BadTypeMismatch
        );

        // An out-of-range request is clamped when the physics thread applies it.
        assert_eq!(
            record_control_request(
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
}
