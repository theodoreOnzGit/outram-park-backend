//! The Connect panel: discovered simulators, the manual fallback, and the
//! explanation of why the list is so often empty.
//!
//! ## The empty state is the most important thing in this file
//!
//! On a working network a simulator appears in a second and nobody reads a word of
//! this panel. On campus or enterprise WiFi — which is where a student most
//! plausibly first tries it — the list stays empty forever, and the panel is the
//! only thing standing between them and giving up.
//!
//! So when nothing has been heard for [`EMPTY_LIST_GRACE_SECONDS`] the panel says,
//! in this order: nothing found yet; **campus/enterprise WiFi will not work**, use
//! a phone hotspot or a home router; discovery is passive listening and this client
//! never scans the network; and then the manual box, labelled as the fallback it
//! is. Calm and short — instructions, not an error report.
//!
//! Keep that order and that content. It is a maintainer requirement, not a
//! stylistic choice.

use egui::{RichText, Ui};

use outram_park_digital_twin_engine::ciet_opcua::discovery::{
    DiscoveredSimulator, CIET_MDNS_SERVICE_TYPE,
};
use outram_park_digital_twin_engine::ciet_opcua::node_map::{DEFAULT_OPCUA_PORT, ENDPOINT_PATH};

use crate::browse::{DiscoveryPoller, DiscoveryStatus, EMPTY_LIST_GRACE_SECONDS};
use crate::endpoint::normalise_endpoint_url;
use crate::shared_state::{ClientSharedState, ConnectionState};
use crate::ui::{ERROR_TEXT, MUTED_TEXT, OK_TEXT, WARNING_TEXT};

/// What the user did in this panel, for the app to act on.
///
/// Returned rather than applied in place, so this function stays a pure renderer
/// and the app owns every mutation of the shared state — the same reason it is an
/// enum and not a set of out-parameters.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiscoveryAction {
    /// Nothing to do.
    None,
    /// Connect to this already-normalised URL.
    Connect {
        /// Canonical `opc.tcp://host:port/path`.
        endpoint_url: String,
    },
    /// Close the current session.
    Disconnect,
}

/// Draw the Connect panel.
///
/// # Arguments
///
/// * `ui` — the egui context to draw into.
/// * `poller` — the mDNS browse poller, read for its list and status.
/// * `state` — shared client state, read for the connection state so the Connect
///   buttons can be disabled during an attempt.
/// * `manual_entry` — the app-owned text buffer for the manual address box.
/// * `manual_error` — the app-owned last parse error for that box, so the message
///   persists across frames instead of flashing for one.
///
/// Returns the [`DiscoveryAction`] the user requested this frame.
pub fn show(
    ui: &mut Ui,
    poller: &DiscoveryPoller,
    state: &ClientSharedState,
    manual_entry: &mut String,
    manual_error: &mut Option<String>,
) -> DiscoveryAction {
    let mut action = DiscoveryAction::None;
    let busy = state.connection.is_busy();

    ui.heading("Find a CIET Educational Simulator v2");
    ui.label(
        RichText::new(
            "This is an offline educational demonstration client. It drives a simulator, \
             never a real plant.",
        )
        .color(MUTED_TEXT),
    );
    ui.add_space(6.0);

    // ---- Current connection, and a way out of it. ----
    match &state.connection {
        ConnectionState::Connected { endpoint_url, .. } => {
            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new("Connected to").color(OK_TEXT).strong());
                ui.label(RichText::new(endpoint_url).monospace());
                if ui.button("Disconnect").clicked() {
                    action = DiscoveryAction::Disconnect;
                }
            });
            ui.add_space(6.0);
        }
        ConnectionState::Connecting { endpoint_url, .. } => {
            ui.horizontal_wrapped(|ui| {
                ui.spinner();
                ui.label(RichText::new("Connecting to").color(WARNING_TEXT).strong());
                ui.label(RichText::new(endpoint_url).monospace());
                if ui.button("Cancel").clicked() {
                    action = DiscoveryAction::Disconnect;
                }
            });
            ui.add_space(6.0);
        }
        ConnectionState::Failed { .. } => {
            show_failure(ui, &state.connection);
            ui.add_space(6.0);
        }
        ConnectionState::Disconnected => {}
    }

    ui.separator();

    // ---- The discovered list, or the explanation of its absence. ----
    match poller.status() {
        DiscoveryStatus::Found { count } => {
            ui.label(
                RichText::new(format!(
                    "{count} simulator{} announcing {}:",
                    if count == 1 { "" } else { "s" },
                    if count == 1 { "itself" } else { "themselves" }
                ))
                .strong(),
            );
            ui.add_space(4.0);
            if let Some(url) = simulator_table(ui, poller.simulators(), busy) {
                action = DiscoveryAction::Connect { endpoint_url: url };
            }
        }
        DiscoveryStatus::Listening => {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label(
                    RichText::new("Listening for simulators announcing themselves...")
                        .color(MUTED_TEXT),
                );
            });
        }
        DiscoveryStatus::NothingFound { listening_for } => {
            show_nothing_found(ui, listening_for.as_secs(), poller.has_ever_found());
        }
        DiscoveryStatus::Unavailable { message } => {
            show_discovery_unavailable(ui, &message);
        }
    }

    ui.add_space(10.0);
    ui.separator();

    // ---- The manual fallback. ----
    show_manual_entry(ui, busy, manual_entry, manual_error, &mut action);

    ui.add_space(10.0);
    ui.separator();
    ui.label(
        RichText::new(format!(
            "Discovery is passive: this client subscribes to mDNS announcements on \
             {CIET_MDNS_SERVICE_TYPE} and lists what arrives. It never scans, sweeps or \
             probes the network, and it only ever contacts an address a simulator \
             announced or you typed in."
        ))
        .color(MUTED_TEXT)
        .small(),
    );

    action
}

/// One row per discovered simulator: name, host, port, endpoint URL, Connect.
///
/// Returns the endpoint URL if a Connect button was clicked.
fn simulator_table(ui: &mut Ui, simulators: &[DiscoveredSimulator], busy: bool) -> Option<String> {
    let mut chosen = None;

    egui::Grid::new("ciet_discovered_simulators")
        .num_columns(5)
        .striped(true)
        .spacing([12.0, 4.0])
        .show(ui, |ui| {
            ui.label(RichText::new("Instance").strong());
            ui.label(RichText::new("Host").strong());
            ui.label(RichText::new("Port").strong());
            ui.label(RichText::new("Endpoint URL").strong());
            ui.label("");
            ui.end_row();

            for simulator in simulators {
                ui.label(&simulator.instance_name);
                ui.label(RichText::new(&simulator.host).monospace());
                ui.label(RichText::new(simulator.port.to_string()).monospace());
                ui.label(RichText::new(&simulator.endpoint_url).monospace());
                if ui
                    .add_enabled(!busy, egui::Button::new("Connect"))
                    .clicked()
                {
                    chosen = Some(simulator.endpoint_url.clone());
                }
                ui.end_row();

                if !simulator.addresses.is_empty() {
                    ui.label("");
                    ui.label(
                        RichText::new(
                            simulator
                                .addresses
                                .iter()
                                .map(|address| address.to_string())
                                .collect::<Vec<_>>()
                                .join(", "),
                        )
                        .color(MUTED_TEXT)
                        .small()
                        .monospace(),
                    );
                    ui.label("");
                    ui.label("");
                    ui.label("");
                    ui.end_row();
                }
            }
        });

    chosen
}

/// The teaching block for an empty browse list.
///
/// Order is fixed by maintainer requirement: what happened, then the
/// campus-WiFi/hotspot fix, then the passive-listening statement. The manual box
/// follows it in [`show`].
fn show_nothing_found(ui: &mut Ui, listening_for_seconds: u64, has_ever_found: bool) {
    ui.label(
        RichText::new("No CIET simulators announced on this network yet.")
            .strong()
            .color(WARNING_TEXT),
    );
    ui.add_space(4.0);

    ui.label(
        RichText::new(
            "If you are on campus or enterprise WiFi, this will not work -- those networks \
             isolate clients from each other and block mDNS. Turn on a phone hotspot, \
             connect both machines to it, and try again. A home router works too.",
        )
        .strong(),
    );
    ui.add_space(4.0);

    ui.label(
        RichText::new(
            "Discovery is passive: we listen for simulators that announce themselves. This \
             client never scans or probes the network.",
        )
        .color(MUTED_TEXT),
    );
    ui.add_space(4.0);

    if has_ever_found {
        ui.label(
            RichText::new(
                "A simulator was visible earlier and has stopped announcing -- it may have \
                 been closed, or the link dropped.",
            )
            .color(MUTED_TEXT)
            .small(),
        );
    }
    ui.label(
        RichText::new(format!(
            "Listening for {listening_for_seconds} s (nothing is expected before about \
             {EMPTY_LIST_GRACE_SECONDS} s)."
        ))
        .color(MUTED_TEXT)
        .small(),
    );
}

/// The block shown when mDNS could not be started at all.
fn show_discovery_unavailable(ui: &mut Ui, message: &str) {
    ui.label(
        RichText::new("Discovery could not start on this machine.")
            .strong()
            .color(ERROR_TEXT),
    );
    ui.label(RichText::new(message).color(MUTED_TEXT).monospace().small());
    ui.add_space(4.0);
    ui.label(
        "Use the manual address box below. Read the URL off the simulator's own OPC-UA page \
         and paste it here.",
    );
}

/// The manual address box, with a Connect button and a persistent parse error.
fn show_manual_entry(
    ui: &mut Ui,
    busy: bool,
    manual_entry: &mut String,
    manual_error: &mut Option<String>,
    action: &mut DiscoveryAction,
) {
    ui.label(RichText::new("Fallback: type the address yourself").strong());
    ui.label(
        RichText::new(
            "Read the LAN URL off the simulator's own OPC-UA page and paste it here. This is \
             the route to use whenever discovery finds nothing.",
        )
        .color(MUTED_TEXT),
    );
    ui.add_space(4.0);

    let mut submitted = false;
    ui.horizontal(|ui| {
        let response = ui.add_enabled(
            !busy,
            egui::TextEdit::singleline(manual_entry)
                .hint_text(format!(
                    "opc.tcp://192.168.1.42:{DEFAULT_OPCUA_PORT}{ENDPOINT_PATH}"
                ))
                .desired_width(360.0),
        );
        submitted = response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
        if ui
            .add_enabled(!busy, egui::Button::new("Connect"))
            .clicked()
        {
            submitted = true;
        }
    });

    ui.label(
        RichText::new(format!(
            "A bare host works too -- \"192.168.1.42\" becomes \
             \"opc.tcp://192.168.1.42:{DEFAULT_OPCUA_PORT}{ENDPOINT_PATH}\"."
        ))
        .color(MUTED_TEXT)
        .small(),
    );

    if submitted {
        match normalise_endpoint_url(manual_entry) {
            Ok(endpoint_url) => {
                *manual_error = None;
                *action = DiscoveryAction::Connect { endpoint_url };
            }
            Err(error) => *manual_error = Some(error.to_string()),
        }
    }

    if let Some(message) = manual_error {
        ui.label(RichText::new(message.as_str()).color(ERROR_TEXT));
    }
}

/// The failure block: what went wrong, the likely cause, what to try, and the raw
/// status code underneath.
///
/// The raw text is always shown verbatim. The diagnosis above it is presented as a
/// likely cause, because that is what it is.
fn show_failure(ui: &mut Ui, connection: &ConnectionState) {
    let ConnectionState::Failed {
        endpoint_url,
        message,
        status_name,
        cause,
        ..
    } = connection
    else {
        return;
    };

    egui::Frame::group(ui.style()).show(ui, |ui| {
        ui.label(RichText::new(cause.summary()).color(ERROR_TEXT).strong());
        ui.label(RichText::new(endpoint_url).monospace().small());
        ui.add_space(4.0);
        ui.label(RichText::new("Most likely cause and what to try:").strong());
        ui.label(cause.hint());
        ui.add_space(4.0);
        ui.collapsing("Exactly what the OPC-UA stack reported", |ui| {
            if !status_name.is_empty() {
                ui.label(
                    RichText::new(format!("status: {status_name}"))
                        .monospace()
                        .small(),
                );
            }
            ui.label(RichText::new(message.as_str()).monospace().small());
        });
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Verifies the discovered-simulator rows and the manual box agree on the URL
    /// they hand to the worker.
    ///
    /// **Methodology.** A discovered row passes `DiscoveredSimulator::endpoint_url`
    /// through unchanged, while the manual box passes its text through
    /// [`normalise_endpoint_url`]. If the two disagreed, connecting to the same
    /// simulator by click and by typing would produce two different URLs and two
    /// different `EndpointDescription`s. Construct a `DiscoveredSimulator` as the
    /// discovery module builds one and assert its `endpoint_url` is already a
    /// fixed point of the normaliser. Pass criterion: normalising the discovered
    /// URL returns it unchanged.
    ///
    /// **Results (2026-07-28).** A simulator advertised at host
    /// `ciet-laptop.local`, port 4840, endpoint
    /// `opc.tcp://ciet-laptop.local:4840/ciet` normalised to itself byte for
    /// byte. Interpretation: the click path and the typed path converge, so a
    /// failure reproduced one way reproduces the other.
    #[test]
    fn a_discovered_endpoint_url_is_already_canonical() {
        let simulator = DiscoveredSimulator {
            instance_name: "CIET-Educational-Simulator-v2 on ciet-laptop".to_string(),
            host: "ciet-laptop.local".to_string(),
            port: DEFAULT_OPCUA_PORT,
            endpoint_url: format!(
                "opc.tcp://ciet-laptop.local:{DEFAULT_OPCUA_PORT}{ENDPOINT_PATH}"
            ),
            addresses: vec!["192.168.1.42".parse().unwrap()],
        };

        assert_eq!(
            normalise_endpoint_url(&simulator.endpoint_url).unwrap(),
            simulator.endpoint_url
        );
    }

    /// Verifies the panel's action enum keeps Connect and Disconnect distinct and
    /// carries the URL.
    ///
    /// **Methodology.** The panel is a pure renderer returning a
    /// [`DiscoveryAction`] for the app to apply, so this checks the contract that
    /// makes that safe: `None`, `Connect` and `Disconnect` compare unequal, and
    /// `Connect` round-trips its URL. Pass criterion: three distinct values, URL
    /// preserved.
    ///
    /// **Results (2026-07-28).** All three variants distinct; `Connect`
    /// round-tripped `opc.tcp://host:4840/ciet`. Interpretation: the renderer
    /// cannot mutate shared state itself, so a click can only ever queue exactly
    /// one command.
    #[test]
    fn the_panel_action_distinguishes_connect_from_disconnect() {
        let connect = DiscoveryAction::Connect {
            endpoint_url: "opc.tcp://host:4840/ciet".to_string(),
        };
        assert_ne!(connect, DiscoveryAction::None);
        assert_ne!(connect, DiscoveryAction::Disconnect);
        assert_ne!(DiscoveryAction::None, DiscoveryAction::Disconnect);
        match &connect {
            DiscoveryAction::Connect { endpoint_url } => {
                assert_eq!(endpoint_url, "opc.tcp://host:4840/ciet");
            }
            other => panic!("expected Connect, got {other:?}"),
        }
    }
}
