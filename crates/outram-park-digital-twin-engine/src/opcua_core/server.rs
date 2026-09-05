//! The shared OPC-UA server: address space, callbacks, and its own thread.
//!
//! [`spawn_opcua_server_thread`] is the whole entry point. Give it the shared
//! plant state, the shared remote-write mailbox and an [`OpcuaServerConfig`],
//! and it returns an [`OpcuaServerHandle`] describing where clients should
//! connect. Everything else here supports that one call.
//!
//! Nothing in this module knows what a reactor is. It is parameterised by the
//! simulator's [`OpcuaVariable`] enum, which supplies the variables, the
//! snapshot type they are read from, and the request type writes are recorded
//! in — see [`super::simulator`] for the seam.
//!
//! ```text
//! physics thread ──write outputs, apply pending requests──┐
//! GUI thread ─────write controls─────────────────────────►├─► Arc<RwLock<V::Snapshot>>
//!                                                         │        ▲        ▲
//! OPC-UA server thread   read callbacks ──────────────────────read──┘        │
//! (own tokio runtime)    200 ms updater ──────────────────────read───────────┘
//!                        write callbacks ──► Arc<RwLock<V::Requests>>
//! ```
//!
//! ## Threading
//!
//! The server runs on its **own `std::thread` with its own multi-threaded
//! `tokio` runtime**, created inside that thread. It shares nothing with a GUI's
//! event loop, so a stalled repaint cannot stall an OPC-UA client and a headless
//! build (Termux, CI) can serve OPC-UA with no GUI at all. Nothing here panics on
//! failure: a server that cannot start reports why and the simulator carries on
//! without it.
//!
//! ## Reads are live; writes are deferred
//!
//! **Reads** are served straight from the snapshot under a read lock, so a
//! client always sees the *effective* value the solver is using — write 1000 kW
//! and read back the 15 kW ceiling.
//!
//! **Writes** do not touch plant state. They are parked in the simulator's
//! request mailbox ([`OpcuaVariable::Requests`]) and applied by the physics
//! thread at the top of its next timestep, which is where clamping and NaN
//! rejection happen. That removes the lost-update race against a GUI's wholesale
//! state overwrite, and keeps a room full of clients off the plant-state lock.
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
//! control**.
//!
//! That is a deliberate choice for throwaway teaching demonstrators, so that
//! "point UaExpert at it and poke the loop" is a ten-second exercise. Hardening
//! (certificates, trust lists, user tokens, audit trails) is explicitly left to
//! security researchers rather than half-done here.
//!
//! The only mitigations you should rely on: a simulator is expected to **clamp**
//! every request on apply and ignore NaN, so a hostile client can annoy the
//! simulation but not destabilise it; the bind address can be set to loopback
//! ([`OpcuaServerConfig::is_loopback_only`]); and a warning is printed whenever
//! it binds wider. Do not describe this interface as secured, and do not run it
//! on a network you do not control.
//!
//! ## Units
//!
//! Nothing here is a physical quantity. Values cross this layer as OPC-UA
//! `Variant`s whose engineering unit is documented by the simulator's own node
//! map; the only dimensioned constant in the module is
//! [`SUBSCRIPTION_PUSH_INTERVAL`], which is wall-clock time.
//!
//! ## Scope (`RESPONSIBLE_USE.md`)
//!
//! This serves **offline educational simulators**. It must never be connected to
//! live operational systems, plant systems, safety-critical infrastructure,
//! real-time plant monitoring, or institutional production systems, and its
//! values are not authoritative for any operational, licensing or safety
//! purpose.

use std::net::IpAddr;
use std::sync::{Arc, RwLock};
use std::time::Duration;

use opcua::nodes::{AccessLevel, VariableBuilder};
use opcua::server::diagnostics::NamespaceMetadata;
use opcua::server::node_manager::memory::{simple_node_manager, SimpleNodeManager};
use opcua::server::{
    Server, ServerBuilder, ServerEndpoint, ServerHandle, SubscriptionCache, ANONYMOUS_USER_TOKEN_ID,
};
use opcua::types::{DataValue, DateTime, NodeId, NumericRange, ObjectId, QualifiedName, StatusCode};

use super::discovery::{self, MdnsAdvertisement};
use super::pki::instance_pki_dir;
use super::simulator::{OpcuaFolder, OpcuaSimulator, OpcuaVariable};

/// How often current plant values are pushed into the address space so OPC-UA
/// **subscriptions and monitored items** report changes.
///
/// 200 ms of wall-clock time. This is a notification cadence, not a solver
/// timestep — it has no effect whatsoever on the physics, and a polling `Read`
/// is always served live regardless of this interval.
pub const SUBSCRIPTION_PUSH_INTERVAL: Duration = Duration::from_millis(200);

/// How a simulator's OPC-UA server should be brought up.
///
/// All four fields are transport / naming configuration; none is a physical
/// quantity, so none carries units. There is deliberately **no `Default`**:
/// build one with [`for_simulator`](Self::for_simulator), which seeds the
/// application name and port from the simulator's own profile, then override
/// what you need. A neutral default would silently advertise the wrong
/// simulator's name.
///
/// ```ignore
/// // loopback only, no announcement -- the safe default for a shared network
/// let config = OpcuaServerConfig {
///     bind_address: "127.0.0.1".to_owned(),
///     advertise_over_mdns: false,
///     ..OpcuaServerConfig::for_simulator::<MySimulator>()
/// };
/// ```
#[derive(Debug, Clone)]
pub struct OpcuaServerConfig {
    /// Local address to bind the listening socket to.
    ///
    /// `"0.0.0.0"` accepts connections on every IPv4 interface, so other
    /// machines on the same network can connect — which is the point of the
    /// demo, and also the case in which **anyone who can reach the port can
    /// write every control** (see the module docs). `"127.0.0.1"` keeps the
    /// interface on this machine; [`is_loopback_only`](Self::is_loopback_only)
    /// reports which of the two you have.
    pub bind_address: String,

    /// TCP port to listen on.
    ///
    /// [`for_simulator`](Self::for_simulator) seeds this from the simulator's
    /// `default_port`, normally 4840 — the IANA-registered `opcua-tcp` port,
    /// which is where OPC-UA tooling looks first. Use something above 1024 and
    /// unregistered if it is taken.
    pub port: u16,

    /// Whether to announce the running server on the local link over mDNS /
    /// DNS-SD, so a client can find it without a typed URL.
    ///
    /// Announcement is cooperative and one-way — see [`super::discovery`],
    /// which explains both that this never scans anything and that many
    /// campus/enterprise networks block it outright. A failure to announce is
    /// logged and otherwise ignored; the server still runs.
    pub advertise_over_mdns: bool,

    /// OPC-UA `ApplicationName`, shown by clients in their server list and in
    /// the endpoint description.
    ///
    /// Also used, sanitised, as the mDNS instance name.
    pub application_name: String,
}

impl OpcuaServerConfig {
    /// The configuration a simulator ships with: bind every interface on its
    /// own default port, announce over mDNS, and use its own application name.
    ///
    /// Binding all interfaces is the default because the demonstration is
    /// "connect from the phone in your hand". It is also the configuration that
    /// exposes every control to the network, which is why the server prints a
    /// warning in that case.
    pub fn for_simulator<S: OpcuaSimulator>() -> Self {
        Self {
            bind_address: "0.0.0.0".to_owned(),
            port: S::PROFILE.default_port,
            advertise_over_mdns: true,
            application_name: S::PROFILE.default_application_name.to_owned(),
        }
    }

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
    pub fn binds_all_interfaces(&self) -> bool {
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
/// Produced by [`spawn_opcua_server_thread`] and displayed by a simulator's
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

    /// Namespace URI every one of this simulator's variables lives in, i.e. its
    /// profile's `namespace_uri`.
    ///
    /// A client resolves this to a running namespace *index* (usually 2) from the
    /// server's namespace array; the index is not stable across versions and must
    /// not be hard-coded.
    pub namespace_uri: &'static str,

    /// Number of variables served, i.e. `V::all().len()`.
    pub node_count: usize,

    /// The PKI directory, ready to print. See [`super::pki`] — it holds a
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

/// A running OPC-UA server.
///
/// Holding this value keeps the server up: it owns the `async-opcua` server
/// handle and, when mDNS announcement is enabled, the announcement guard. No
/// lifetime parameters — everything is owned or shared through `Arc`.
///
/// Dropping the handle withdraws the mDNS announcement (its guard's `Drop`) but
/// does **not** stop the server; call [`shutdown`](Self::shutdown) for that.
/// That split is deliberate: a simulator's GUI keeps the handle for the whole
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

/// Things that can stop an OPC-UA server from starting.
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

    /// The simulator's node manager was not present on the built server. This
    /// would mean `with_node_manager` did not take effect, i.e. an `async-opcua`
    /// version change rather than a user error.
    #[error(
        "the simulator's node manager is missing from the built server (async-opcua API change?)"
    )]
    NodeManagerUnavailable,

    /// The simulator's namespace URI was not registered, so no node id could be
    /// formed. Same class of cause as
    /// [`NodeManagerUnavailable`](Self::NodeManagerUnavailable). The payload is
    /// the URI that was expected.
    #[error("the namespace {0} was not registered by the server")]
    NamespaceUnavailable(&'static str),

    /// A folder or variable could not be inserted into the address space —
    /// almost always a duplicate node id, which a simulator's own
    /// "node identifiers are unique" test is there to prevent.
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

/// Start simulator `V`'s OPC-UA server on its own thread.
///
/// Spawns a dedicated `std::thread` with its own multi-threaded `tokio` runtime;
/// that thread builds the server, populates its address space from
/// [`V::all()`](OpcuaVariable::all), wires the callbacks, and serves
/// connections. This function blocks only until the thread reports whether
/// construction succeeded (a certificate load and one insertion per variable),
/// then optionally announces over mDNS and returns.
///
/// **The build must happen on the server thread**, not here: `ServerBuilder::build`
/// spawns a tokio task internally (`ServerStatusWrapper::new`) and panics with
/// "there is no reactor running" outside a runtime. The result therefore comes
/// back over a `std::sync::mpsc` channel, which is what keeps this signature
/// synchronous so a GUI can call it with no `async` in sight.
///
/// `state` is read to serve reads; `requests` receives writes, which the physics
/// thread applies on its next timestep (see the module docs). Neither lock is
/// held across an `await`.
///
/// The type parameter is not inferable from the arguments (they are its
/// associated types), so call it with a turbofish —
/// `spawn_opcua_server_thread::<CietNode>(..)` — or wrap it in a concrete
/// per-simulator function, which is what `ciet_opcua::server` does.
///
/// # What lands in the address space
///
/// Under `Objects`, two folders named by the simulator's profile: the outputs
/// folder holds every [`OpcuaFolder::Outputs`] variable, the controls folder
/// every [`OpcuaFolder::Controls`] one. Node ids are
/// `ns=<index>;s=<node_identifier()>`, with display names and unit-bearing
/// descriptions from the variable. The namespace index is assigned at start-up
/// (2 in this configuration) — clients must resolve it, never hard-code it.
///
/// # Security
///
/// There is none, deliberately: `SecurityPolicy::None` and anonymous access, so
/// **anyone who can reach the port can write every control**. Read the module
/// documentation before running this anywhere but a bench.
///
/// # Errors
///
/// See [`OpcuaServerError`]. A failure to announce over mDNS is *not* an error —
/// it is reported and the server runs regardless.
pub fn spawn_opcua_server_thread<V: OpcuaVariable>(
    state: Arc<RwLock<V::Snapshot>>,
    requests: Arc<RwLock<V::Requests>>,
    config: OpcuaServerConfig,
) -> Result<OpcuaServerHandle, OpcuaServerError> {
    let profile = <V::Simulator as OpcuaSimulator>::PROFILE;

    // Isolate the certificate store per port. Two servers that can coexist on
    // one machine necessarily differ in port, so this makes concurrent instances
    // -- notably headless simulator tests -- stop racing on one shared keypair.
    let pki_dir = instance_pki_dir(profile.pki_dir_name, &format!("port-{}", config.port));

    // The thread owns the runtime, the server and the address space, so it is
    // independent of any GUI event loop. The `JoinHandle` is deliberately
    // dropped (detached): the simulator's lifetime is the process lifetime, and
    // `shutdown()` on the returned handle is how the thread is asked to stop.
    let (report_sender, report_receiver) = std::sync::mpsc::channel();
    let thread_state = Arc::clone(&state);
    let thread_requests = Arc::clone(&requests);
    let thread_config = config.clone();
    let thread_pki_dir = pki_dir.clone();
    std::thread::Builder::new()
        .name(format!("{}-opcua", profile.node_manager_name))
        .spawn(move || {
            run_server_thread::<V>(
                thread_state,
                thread_requests,
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

    let endpoint_info = build_endpoint_info::<V>(&config, &pki_dir);
    print_startup_banner::<V>(&config, &endpoint_info);

    let mdns_advertisement = if config.advertise_over_mdns {
        let instance_name = if config.application_name.trim().is_empty() {
            profile.mdns_instance_prefix
        } else {
            config.application_name.as_str()
        };
        match discovery::advertise_simulator::<V::Simulator>(config.port, instance_name) {
            Ok(advertisement) => {
                println!(
                    "{} OPC-UA: announced over mDNS as {}",
                    profile.log_prefix,
                    advertisement.instance_name()
                );
                Some(advertisement)
            }
            Err(error) => {
                // Not fatal: the endpoint still works with a typed URL.
                eprintln!(
                    "{} OPC-UA: mDNS announcement unavailable -- {error}",
                    profile.log_prefix
                );
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
/// variable, returning the node ids paired with what they read.
///
/// The returned vector is the single list the read callbacks, the write
/// callbacks and the periodic push all iterate, so a variable cannot be present
/// in one and missing from another.
fn build_address_space<V: OpcuaVariable>(
    node_manager: &SimpleNodeManager,
    namespace_index: u16,
    state: &Arc<RwLock<V::Snapshot>>,
) -> Result<Vec<(NodeId, V)>, OpcuaServerError> {
    let profile = <V::Simulator as OpcuaSimulator>::PROFILE;

    let objects_folder: NodeId = ObjectId::ObjectsFolder.into();
    let outputs_folder = NodeId::new(namespace_index, profile.outputs_folder_node_id);
    let controls_folder = NodeId::new(namespace_index, profile.controls_folder_node_id);

    // Take one snapshot so every variable's initial value is consistent, and so
    // the lock is held once rather than once per variable. A poisoned lock means
    // the physics thread panicked; fall back to the poisoned snapshot rather
    // than propagating a panic into the OPC-UA layer.
    let snapshot = match state.read() {
        Ok(guard) => guard.clone(),
        Err(poisoned) => {
            eprintln!(
                "{} OPC-UA: plant state lock was poisoned at start-up; \
                 initialising the address space from the poisoned snapshot",
                profile.log_prefix
            );
            poisoned.into_inner().clone()
        }
    };

    // No timestamp is needed here: `VariableBuilder::value` takes the bare
    // `Variant`, and the address space stamps its own source/server timestamps.
    // Live timestamps come from the read callbacks and the periodic push.
    let variables = V::all();
    let mut address_space = node_manager.address_space().write();
    let mut nodes: Vec<(NodeId, V)> = Vec::with_capacity(variables.len());

    if !address_space.add_folder(
        &outputs_folder,
        QualifiedName::new(namespace_index, profile.outputs_folder_name),
        profile.outputs_folder_name,
        &objects_folder,
    ) {
        return Err(OpcuaServerError::AddressSpace(format!(
            "the {} folder",
            profile.outputs_folder_name
        )));
    }
    if !address_space.add_folder(
        &controls_folder,
        QualifiedName::new(namespace_index, profile.controls_folder_name),
        profile.controls_folder_name,
        &objects_folder,
    ) {
        return Err(OpcuaServerError::AddressSpace(format!(
            "the {} folder",
            profile.controls_folder_name
        )));
    }

    for node in variables {
        let node_id = NodeId::new(namespace_index, node.node_identifier());
        let parent = match node.folder() {
            OpcuaFolder::Outputs => &outputs_folder,
            OpcuaFolder::Controls => &controls_folder,
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
        .value(node.read(&snapshot))
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
///
/// Writability is decided by [`OpcuaVariable::access_level`] alone — a variable
/// without `CURRENT_WRITE` gets no write callback, so there is no second flag
/// that could disagree with the access level the address space advertises.
fn register_callbacks<V: OpcuaVariable>(
    node_manager: &SimpleNodeManager,
    state: &Arc<RwLock<V::Snapshot>>,
    requests: &Arc<RwLock<V::Requests>>,
    nodes: &[(NodeId, V)],
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
                Ok(DataValue::new_at(node.read(&guard), DateTime::now()))
            },
        );

        if node.access_level().contains(AccessLevel::CURRENT_WRITE) {
            let write_requests = Arc::clone(requests);
            inner.add_write_callback(node_id.clone(), move |value, _index_range| {
                let Ok(mut guard) = write_requests.write() else {
                    return StatusCode::BadInternalError;
                };
                node.record_write(&mut guard, value)
            });
        }
    }
}

/// Build and run the server to completion on this thread, in a runtime it owns.
///
/// Reports the outcome of construction back over `report` — exactly once, on
/// every code path — then serves connections until cancelled. Never panics: a
/// runtime that will not build, a rejected configuration, or a server that exits
/// with an error is reported and the thread returns, leaving the simulator
/// running without its OPC-UA interface.
fn run_server_thread<V: OpcuaVariable>(
    state: Arc<RwLock<V::Snapshot>>,
    requests: Arc<RwLock<V::Requests>>,
    config: OpcuaServerConfig,
    pki_dir: std::path::PathBuf,
    report: std::sync::mpsc::Sender<Result<ServerHandle, OpcuaServerError>>,
) {
    let profile = <V::Simulator as OpcuaSimulator>::PROFILE;

    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .thread_name(format!("{}-opcua-rt", profile.node_manager_name))
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
        let prepared = match prepare_server::<V>(&state, &requests, &config, &pki_dir) {
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
                push_values_into_address_space::<V>(
                    &node_manager,
                    &subscriptions,
                    &updater_state,
                    &nodes,
                );
            }
        });

        match server.run().await {
            Ok(()) => println!("{} OPC-UA: server stopped", profile.log_prefix),
            Err(error) => eprintln!(
                "{} OPC-UA: server stopped with an error ({error})",
                profile.log_prefix
            ),
        }

        updater.abort();
    });
}

/// A built-but-not-yet-running server, with everything the serving loop needs.
struct PreparedServer<V: OpcuaVariable> {
    /// The server itself, ready for `run()`.
    server: Server,
    /// Handle used to cancel it and to reach its subscription cache.
    server_handle: ServerHandle,
    /// The simulator's node manager, used by the periodic push.
    node_manager: Arc<SimpleNodeManager>,
    /// Every node id paired with what it reads.
    nodes: Arc<Vec<(NodeId, V)>>,
}

/// Build the OPC-UA server, its address space and its callbacks.
///
/// **Must be called from inside a tokio runtime**: `ServerBuilder::build` spawns
/// a task internally and panics with "there is no reactor running" otherwise.
fn prepare_server<V: OpcuaVariable>(
    state: &Arc<RwLock<V::Snapshot>>,
    requests: &Arc<RwLock<V::Requests>>,
    config: &OpcuaServerConfig,
    pki_dir: &std::path::Path,
) -> Result<PreparedServer<V>, OpcuaServerError> {
    let profile = <V::Simulator as OpcuaSimulator>::PROFILE;

    let (server, server_handle) = ServerBuilder::new_anonymous(config.application_name.clone())
        // Per-instance application URI. It MUST differ from the namespace URI
        // (see `OpcuaSimulatorProfile`), and making it differ per port also
        // stops two simulators on one machine presenting the same application
        // identity.
        .application_uri(format!("{}:{}", profile.application_uri, config.port))
        .product_uri(profile.application_uri)
        // A self-signed keypair, written under `own/` and `private/` in the PKI
        // directory on first run. It authenticates nothing here -- no endpoint
        // uses it -- but async-opcua expects the store to exist.
        .create_sample_keypair(true)
        .certificate_path("own/cert.der")
        .private_key_path("private/private.pem")
        .pki_dir(pki_dir.to_path_buf())
        .host(config.bind_address.clone())
        .port(config.port)
        // Replace the "/" endpoint `new_anonymous` installs with the simulator's
        // one, so the URL is `opc.tcp://<host>:<port>/<path>`. Still
        // SecurityPolicy None + anonymous.
        .add_endpoint(
            "none",
            ServerEndpoint::new_none(profile.endpoint_path, &[ANONYMOUS_USER_TOKEN_ID.to_owned()]),
        )
        .discovery_urls(vec![profile.endpoint_path.to_owned()])
        .with_node_manager(simple_node_manager(
            NamespaceMetadata {
                namespace_uri: profile.namespace_uri.to_owned(),
                ..Default::default()
            },
            profile.node_manager_name,
        ))
        .build()
        .map_err(OpcuaServerError::Build)?;

    let namespace_index = server_handle
        .get_namespace_index(profile.namespace_uri)
        .ok_or(OpcuaServerError::NamespaceUnavailable(
            profile.namespace_uri,
        ))?;

    let node_manager = server_handle
        .node_managers()
        .get_of_type::<SimpleNodeManager>()
        .ok_or(OpcuaServerError::NodeManagerUnavailable)?;

    let nodes = Arc::new(build_address_space::<V>(
        &node_manager,
        namespace_index,
        state,
    )?);
    register_callbacks::<V>(&node_manager, state, requests, &nodes);

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
fn push_values_into_address_space<V: OpcuaVariable>(
    node_manager: &SimpleNodeManager,
    subscriptions: &SubscriptionCache,
    state: &Arc<RwLock<V::Snapshot>>,
    nodes: &[(NodeId, V)],
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
        .map(|(node_id, node)| (node_id, None, DataValue::new_at(node.read(&snapshot), now)))
        .collect();

    if let Err(status) = node_manager.set_values(subscriptions, updates.into_iter()) {
        eprintln!(
            "{} OPC-UA: could not publish plant values ({status})",
            <V::Simulator as OpcuaSimulator>::PROFILE.log_prefix
        );
    }
}

/// Assemble the "how to connect" description for a configuration.
///
/// The LAN address comes from `local_ip_address::local_ip` (this machine's
/// primary non-loopback address); a failure there just leaves
/// [`OpcuaEndpointInfo::lan_url`] as `None`.
fn build_endpoint_info<V: OpcuaVariable>(
    config: &OpcuaServerConfig,
    pki_dir: &std::path::Path,
) -> OpcuaEndpointInfo {
    let profile = <V::Simulator as OpcuaSimulator>::PROFILE;
    let path = profile.endpoint_path;

    let loopback_url = format!("opc.tcp://127.0.0.1:{}{path}", config.port);

    let lan_url = match local_ip_address::local_ip() {
        Ok(IpAddr::V4(address)) => Some(format!("opc.tcp://{address}:{}{path}", config.port)),
        Ok(IpAddr::V6(address)) => Some(format!("opc.tcp://[{address}]:{}{path}", config.port)),
        Err(_) => None,
    };

    OpcuaEndpointInfo {
        loopback_url,
        lan_url,
        bound_to_all_interfaces: config.binds_all_interfaces(),
        namespace_uri: profile.namespace_uri,
        node_count: V::all().len(),
        pki_dir_display: pki_dir.display().to_string(),
    }
}

/// Print one start-up line, plus a no-security warning when the endpoint is
/// reachable from other machines.
///
/// Deliberately terse: a simulator binary prints its own full connection
/// banner, so this only guarantees a headless or embedding caller still sees the
/// one fact that matters — that there is no security.
fn print_startup_banner<V: OpcuaVariable>(
    config: &OpcuaServerConfig,
    endpoint_info: &OpcuaEndpointInfo,
) {
    let prefix = <V::Simulator as OpcuaSimulator>::PROFILE.log_prefix;

    println!(
        "{prefix} OPC-UA: serving {} variables at {}",
        endpoint_info.node_count,
        endpoint_info.primary_url()
    );

    if !config.is_loopback_only() {
        println!(
            "{prefix} OPC-UA: WARNING -- bound to {}, NO security (SecurityPolicy::None, \
             anonymous): anyone who can reach port {} can write every control.",
            config.bind_address, config.port
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::opcua_core::simulator::OpcuaSimulatorProfile;

    /// A simulator that exists only to exercise the shared layer's own logic.
    ///
    /// It belongs to no reactor, publishes no variables and is never served; it
    /// supplies a profile so the configuration tests below have identity strings
    /// to check against without depending on CIET.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    struct TestSimulator;

    impl OpcuaSimulator for TestSimulator {
        const PROFILE: OpcuaSimulatorProfile = OpcuaSimulatorProfile {
            namespace_uri: "urn:outram-park:opcua-core-test",
            application_uri: "urn:outram-park:opcua-core-test:server",
            endpoint_path: "/test",
            default_application_name: "OPC-UA Core Test Simulator",
            default_port: 4840,
            node_manager_name: "opcua-core-test",
            outputs_folder_name: "Outputs",
            outputs_folder_node_id: "Test.Outputs",
            controls_folder_name: "Controls",
            controls_folder_node_id: "Test.Controls",
            pki_dir_name: "opcua-core-test-pki",
            mdns_instance_prefix: "OPCUA-Core-Test",
            mdns_product_marker: "opcua-core-test",
            log_prefix: "OPC-UA core test",
        };
    }

    /// Verifies that a configuration built from a simulator's profile is the
    /// documented one — it decides whether a freshly built simulator is
    /// network-reachable, and therefore whether every control is exposed.
    ///
    /// **Methodology.** Compare all four fields of
    /// `OpcuaServerConfig::for_simulator::<TestSimulator>()` against the
    /// documented rule (all interfaces, the profile's port and application name,
    /// mDNS on), plus the derived predicates. Pass criterion: `0.0.0.0`,
    /// `PROFILE.default_port`, mDNS on, `PROFILE.default_application_name`,
    /// `is_loopback_only() == false`, `binds_all_interfaces() == true`. No
    /// socket is opened.
    ///
    /// **Results (2026-08-12).** `"0.0.0.0"`, 4840, `true`,
    /// `"OPC-UA Core Test Simulator"`, `false`, `true` — 6/6 as documented.
    /// Interpretation: `for_simulator` produces the network-reachable
    /// configuration, i.e. exactly the case where the no-security warning fires,
    /// and it takes its identity from the profile rather than from any built-in
    /// default.
    #[test]
    fn for_simulator_binds_all_interfaces_on_the_profile_port() {
        let config = OpcuaServerConfig::for_simulator::<TestSimulator>();

        assert_eq!(config.bind_address, "0.0.0.0");
        assert_eq!(config.port, TestSimulator::PROFILE.default_port);
        assert!(config.advertise_over_mdns);
        assert_eq!(
            config.application_name,
            TestSimulator::PROFILE.default_application_name
        );

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
    /// **Results (2026-07-28, unchanged 2026-08-12).** true for `127.0.0.1`,
    /// `127.0.0.53`, `::1`, `[::1]`, `localhost`, `LOCALHOST`; false for
    /// `0.0.0.0`, `::`, `192.168.1.42`, `not-an-address`, `""` — 11/11 as
    /// documented.
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
                ..OpcuaServerConfig::for_simulator::<TestSimulator>()
            };
            assert!(
                config.is_loopback_only(),
                "{address} should be loopback-only"
            );
        }

        for address in not_loopback {
            let config = OpcuaServerConfig {
                bind_address: address.to_owned(),
                ..OpcuaServerConfig::for_simulator::<TestSimulator>()
            };
            assert!(
                !config.is_loopback_only(),
                "{address} should NOT be loopback-only"
            );
        }
    }

    /// Verifies [`OpcuaEndpointInfo::primary_url`] picks a URL that can actually
    /// work — this is the string a GUI shows and a user types on another device.
    ///
    /// **Methodology.** All four combinations of `bound_to_all_interfaces` and
    /// `lan_url`. Pass criterion: the LAN URL only when bound to all interfaces
    /// *and* known; the loopback URL otherwise, since a `127.0.0.1` bind is not
    /// reachable at the LAN address even though that address exists. No socket.
    ///
    /// **Results (2026-07-28, unchanged 2026-08-12).** (all, known) →
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
            namespace_uri: TestSimulator::PROFILE.namespace_uri,
            node_count: 0,
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
}
