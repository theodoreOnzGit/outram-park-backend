//! Starting the embedded OPC-UA server, and the human-readable connection
//! banner shared by the GUI page and the headless status output.
//!
//! **v2 addition** — v1 had no remote interface at all.
//!
//! `egui`-free on purpose: the headless Termux build prints the same banner this
//! module produces, so nothing here may reach for the GUI stack.
//!
//! ## Security (there is none, deliberately)
//!
//! The server runs with OPC-UA `SecurityPolicy::None` and anonymous access.
//! Bound to `0.0.0.0` — the default — anyone who can reach the port can read
//! every output and write every control. That is a deliberate choice for a
//! throwaway teaching demonstrator, and it is why [`connection_banner`] always
//! prints the warning rather than hiding it behind a verbosity flag.

use outram_park_digital_twin_engine::ciet_opcua::server::{
    spawn_opcua_server_thread, OpcuaServerConfig, OpcuaServerHandle,
};
use outram_park_digital_twin_engine::ciet_opcua::state::SharedCietState;
use outram_park_digital_twin_engine::ciet_opcua::user_controls::SharedUserControls;

use crate::cli::CliOptions;

/// The OPC-UA application name the server advertises.
///
/// Shows up in a client's endpoint list (UaExpert, `opcua-commander`, the
/// bundled `ciet_v2_opcua_client`), so it names the simulator, not the crate.
pub const OPCUA_APPLICATION_NAME: &str = "CIET Educational Simulator v2";

/// Whether the embedded OPC-UA server came up, and how to reach it if it did.
///
/// A failure is **not** fatal: the simulator must still run its physics and its
/// GUI when, say, port 4840 is already bound by another OPC-UA server on the
/// machine. The failure text is carried here and shown on the OPC-UA page.
///
/// Enum rather than `Result<OpcuaServerHandle, _>` so the "never started" case
/// is nameable and so the dispatch sites are exhaustive.
pub enum OpcuaStatus {
    /// The server thread is running. The handle carries the endpoint URLs, the
    /// namespace URI, the node count and the PKI directory.
    Running(OpcuaServerHandle),
    /// The server could not start. The string is the underlying error, rendered
    /// for display; the usual cause is the port already being in use.
    Failed(String),
    /// The server was never started. Reachable only in code paths that build an
    /// app without a server, which is why it exists as an explicit state rather
    /// than being papered over as a failure.
    NotStarted,
}

impl Default for OpcuaStatus {
    /// [`OpcuaStatus::NotStarted`] — no server, and no pretence that one failed.
    fn default() -> Self {
        Self::NotStarted
    }
}

/// Start the OPC-UA server on its own thread, honouring the command line.
///
/// Never panics and never returns an error: a startup failure becomes
/// [`OpcuaStatus::Failed`] so the caller can carry on and run the simulation
/// regardless.
///
/// Two shared handles go in, and the split between them matters:
///
/// - `state` is the same [`SharedCietState`] the physics thread integrates, so
///   the server's **read** callbacks serve live values with no copying.
/// - `user_controls` receives the server's **write** callbacks. Remote writes are
///   queued there as pending requests rather than applied to `state` directly,
///   and the physics thread drains them at the top of each timestep. That is
///   what stops a GUI repaint from silently erasing a client's write, and it
///   keeps remote writes off the plant-state lock entirely.
pub fn start_opcua_server(
    state: SharedCietState,
    user_controls: SharedUserControls,
    options: &CliOptions,
) -> OpcuaStatus {
    let config = OpcuaServerConfig {
        bind_address: options.bind_address.clone(),
        port: options.port,
        advertise_over_mdns: options.advertise_over_mdns,
        application_name: OPCUA_APPLICATION_NAME.to_string(),
    };

    match spawn_opcua_server_thread(state, user_controls, config) {
        Ok(handle) => OpcuaStatus::Running(handle),
        Err(error) => OpcuaStatus::Failed(error.to_string()),
    }
}

/// The plain-text "how to connect" banner.
///
/// Printed to stdout by the headless run loop and rendered (as labels) by the
/// GUI's OPC-UA page, so the two never drift apart. Includes the security
/// warning, the endpoint URLs, and the campus-WiFi caveat — a user who cannot
/// connect should find the reason here rather than conclude the software is
/// broken.
pub fn connection_banner(status: &OpcuaStatus) -> String {
    let mut banner = String::new();

    banner += "=====================================================================\n";
    banner += " CIET Educational Simulator v2 -- OPC-UA server\n";
    banner += "=====================================================================\n";

    match status {
        OpcuaStatus::Running(handle) => {
            let info = handle.endpoint_info();

            banner += &format!(" Connect a client to:   {}\n", info.primary_url());
            match &info.lan_url {
                Some(lan_url) => {
                    banner += &format!(" From another device:   {lan_url}\n");
                }
                None => {
                    banner += " From another device:   UNAVAILABLE -- this machine has no\n";
                    banner += "                        non-loopback network address up. Join a\n";
                    banner += "                        network or start a phone hotspot, then\n";
                    banner += "                        restart the simulator.\n";
                }
            }
            banner += &format!(" On this machine:       {}\n", info.loopback_url);
            banner += &format!(" Namespace URI:         {}\n", info.namespace_uri);
            banner += &format!(" Variables served:      {}\n", info.node_count);
            banner += &format!(" PKI directory:         {}\n", info.pki_dir_display);
            banner += " Security policy:       None, anonymous (no login, no encryption)\n";

            if info.bound_to_all_interfaces {
                banner += "---------------------------------------------------------------------\n";
                banner += " WARNING: bound to ALL network interfaces with NO authentication and\n";
                banner += " NO encryption. Anyone on this network can READ every value AND\n";
                banner += " WRITE every control -- heater power, pump pressure, set points,\n";
                banner += " valves. Do NOT run this on untrusted or public WiFi.\n";
                banner += " Restart with `--bind 127.0.0.1` to keep it on this machine.\n";
            }

            banner += "---------------------------------------------------------------------\n";
            banner += " Connecting from another device:\n";
            banner += "  * On campus or enterprise WiFi this will NOT work -- those networks\n";
            banner += "    isolate clients from each other and block mDNS.\n";
            banner += "  * Use a phone hotspot: connect both machines to it and the bundled\n";
            banner += "    `ciet_v2_opcua_client` will find this simulator automatically. A\n";
            banner += "    home router works too.\n";
            banner += "  * If discovery fails, type the URL above into any OPC-UA client by\n";
            banner += "    hand.\n";
        }
        OpcuaStatus::Failed(error) => {
            banner += &format!(" OPC-UA server did NOT start: {error}\n");
            banner += " The simulation is running anyway; only the remote interface is\n";
            banner += " unavailable. The usual cause is the port already being in use --\n";
            banner += " try `--port 4841`.\n";
        }
        OpcuaStatus::NotStarted => {
            banner += " OPC-UA server was not started.\n";
        }
    }

    banner += "=====================================================================\n";
    banner += " OFFLINE EDUCATIONAL DEMONSTRATION. Not for facility operation, reactor\n";
    banner += " control, licensing, safety-critical decisions or plant monitoring.\n";
    banner += "=====================================================================\n";

    banner
}
