//! Starting the CIET v2 OPC-UA server.
//!
//! The transport, the server thread, the address space and the callbacks all
//! live in [`opcua_core::server`](crate::opcua_core::server), which serves any
//! OUTRAM PARK simulator. This module is CIET's binding to it: the shared types
//! under their CIET names, plus a concrete
//! [`spawn_opcua_server_thread`] so callers never write a turbofish.
//!
//! [`spawn_opcua_server_thread`] is the whole entry point. Give it the shared
//! plant state, the shared remote-write mailbox and an [`OpcuaServerConfig`], and
//! it returns an [`OpcuaServerHandle`] describing where clients should connect.
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
//! ## Reads are live; writes are deferred
//!
//! **Reads** are served straight from [`CietState`](super::state::CietState)
//! under a read lock, so a
//! client always sees the *effective* value the solver is using — write 1000 kW
//! and read back the 15 kW ceiling.
//!
//! **Writes** do not touch plant state. They are parked in
//! [`CietUserControls`](super::user_controls::CietUserControls) and applied by
//! the physics thread at the top of its next timestep, which is where clamping
//! and NaN rejection happen. That removes the lost-update race against the GUI's
//! wholesale `overwrite_state`, and keeps a room full of clients off the
//! plant-state lock. The mapping from a node to the field it reads, and to the
//! request slot it writes, is [`super::simulator::CietNode`].
//!
//! ## Security: there is none, deliberately
//!
//! The endpoint uses **`SecurityPolicy::None`, `MessageSecurityMode::None` and
//! anonymous user tokens**: traffic is unencrypted and unsigned, no client
//! certificate is checked, no credential is required, and therefore **anyone who
//! can reach the TCP port can write every control** — heater power, pump
//! pressure, both branch valves, the timestep.
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

use crate::opcua_core::server as core_server;

use super::simulator::{CietNode, CietOpcuaSimulator};
use super::state::SharedCietState;
use super::user_controls::SharedUserControls;

pub use crate::opcua_core::server::{
    OpcuaEndpointInfo, OpcuaServerConfig, OpcuaServerError, OpcuaServerHandle,
    SUBSCRIPTION_PUSH_INTERVAL,
};

/// The CIET v2 server configuration with CIET's own defaults filled in.
///
/// Bind every interface on [`DEFAULT_OPCUA_PORT`](super::node_map::DEFAULT_OPCUA_PORT)
/// (4840), announce over mDNS, and call ourselves
/// `"CIET Educational Simulator v2"`.
///
/// Binding all interfaces is the default because the demonstration is "connect
/// from the phone in your hand". It is also the configuration that exposes every
/// control to the network, which is why the simulator prints a warning in that
/// case — pass `bind_address: "127.0.0.1".to_owned()` to keep it on this
/// machine.
///
/// ```ignore
/// // loopback only, no announcement -- the safe default for a shared network
/// let config = OpcuaServerConfig {
///     bind_address: "127.0.0.1".to_owned(),
///     advertise_over_mdns: false,
///     ..default_ciet_server_config()
/// };
/// ```
pub fn default_ciet_server_config() -> OpcuaServerConfig {
    OpcuaServerConfig::for_simulator::<CietOpcuaSimulator>()
}

/// Start the CIET v2 OPC-UA server on its own thread.
///
/// A thin binding of
/// [`opcua_core::server::spawn_opcua_server_thread`](crate::opcua_core::server::spawn_opcua_server_thread)
/// to [`CietNode`]: it spawns a dedicated `std::thread` with its own
/// multi-threaded `tokio` runtime, builds the server, populates its address
/// space from [`super::node_map`], wires the callbacks, and serves connections.
/// It blocks only until the server thread reports whether construction succeeded
/// (a certificate load and 36 node insertions), then optionally announces over
/// mDNS and returns. The signature is synchronous, so a GUI can call it with no
/// `async` in sight.
///
/// `state` is read to serve reads; `user_controls` receives writes, which the
/// physics thread applies on its next timestep (see the module docs). Neither
/// lock is held across an `await`.
///
/// # What lands in the address space
///
/// Under `Objects`, two folders: `Outputs` holds one read-only `Double` per
/// [`CietSignal`](super::node_map::CietSignal); `Controls` holds one writable
/// `Double` per [`CietControl`](super::node_map::CietControl) and one writable
/// `Boolean` per [`CietSwitch`](super::node_map::CietSwitch). Node ids are
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
    core_server::spawn_opcua_server_thread::<CietNode>(state, user_controls, config)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ciet_opcua::node_map::DEFAULT_OPCUA_PORT;

    /// Verifies the CIET default configuration is the documented one — it
    /// decides whether a freshly built simulator is network-reachable, and
    /// therefore whether every control is exposed.
    ///
    /// **Methodology.** Compare all four fields of
    /// [`default_ciet_server_config`] against the documented defaults, plus the
    /// derived predicates. Pass criterion: `0.0.0.0`, [`DEFAULT_OPCUA_PORT`],
    /// mDNS on, the v2 application name, `is_loopback_only() == false`,
    /// `binds_all_interfaces() == true`. No socket is opened.
    ///
    /// **Results (2026-07-28, unchanged 2026-08-12 after the shared-layer
    /// extraction).** `"0.0.0.0"`, 4840, `true`,
    /// `"CIET Educational Simulator v2"`, `false`, `true` — 6/6 as documented.
    /// Interpretation: the default is the network-reachable configuration, i.e.
    /// exactly the case where the no-security warning fires, and CIET's identity
    /// still reaches the endpoint through the shared layer's profile.
    #[test]
    fn default_config_binds_all_interfaces_on_the_standard_port() {
        let config = default_ciet_server_config();

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
}
