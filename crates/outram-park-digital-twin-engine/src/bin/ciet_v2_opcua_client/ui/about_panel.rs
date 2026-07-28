//! Scope, security and provenance — the honest statement of what this is not.
//!
//! This panel exists because OPC-UA is a *plant-connectivity* protocol. A tool
//! that speaks it looks, at a glance, like industrial software, and a student who
//! assumes that would be wrong in a way that matters. So the boundary is stated
//! plainly and in the app itself, not only in a repository document.
//!
//! Per `RESPONSIBLE_USE.md`, do not soften any of this text, and do not remove the
//! prohibited-use list while editing.

use egui::{RichText, Ui};

use outram_park_digital_twin_engine::ciet_opcua::discovery::CIET_MDNS_SERVICE_TYPE;
use outram_park_digital_twin_engine::ciet_opcua::node_map::{
    total_node_count, CietControl, CietSignal, CietSwitch, CIET_NAMESPACE_URI, DEFAULT_OPCUA_PORT,
    ENDPOINT_PATH,
};

use crate::shared_state::ClientSharedState;
use crate::ui::{security_banner, ERROR_TEXT, MUTED_TEXT, UNREAD_PLACEHOLDER};
use crate::worker::{APPLICATION_NAME, UPDATE_INTERVAL_MS};

/// Draw the About & scope panel.
///
/// # Arguments
///
/// * `ui` — the egui context to draw into.
/// * `state` — shared client state, read for the session facts quoted back to the
///   user (namespace index, transport mode).
pub fn show(ui: &mut Ui, state: &ClientSharedState) {
    egui::ScrollArea::vertical().show(ui, |ui| {
        ui.heading("What this is");
        ui.label(
            "An OFFLINE educational demonstration client for the CIET Educational Simulator \
             v2. It finds a simulator that has announced itself on the local network, \
             connects to it over OPC-UA, displays its outputs and writes its controls. Its \
             purpose is teaching, capability building and verification work -- nothing else.",
        );
        ui.add_space(8.0);

        ui.heading("Security: there is none");
        security_banner(ui);
        ui.add_space(4.0);
        ui.label(
            "The connection carries NO authentication and NO encryption. The session uses \
             SecurityPolicy None, MessageSecurityMode None and an anonymous identity token, \
             because that is what the simulator serves. Anyone who can reach the port can \
             read every output and write every control, including the heater power.",
        );
        ui.label(
            RichText::new(
                "Hardening this -- certificates, a trust list, user tokens, an audit trail \
                 -- is deliberately out of scope. Do not describe this client or the \
                 simulator's server as secured.",
            )
            .color(MUTED_TEXT),
        );
        ui.add_space(8.0);

        ui.heading("Where this must never be pointed");
        ui.label(
            RichText::new("Per RESPONSIBLE_USE.md, this client must NEVER be connected to:")
                .color(ERROR_TEXT)
                .strong(),
        );
        for prohibited in [
            "live operational systems or plant systems of any kind",
            "safety-critical infrastructure",
            "real-time plant monitoring or operational digital twin deployments",
            "reactor control, licensing or safety-critical decision-making",
            "institutional production systems or restricted infrastructure",
        ] {
            ui.label(RichText::new(format!("  -- {prohibited}")).color(ERROR_TEXT));
        }
        ui.add_space(4.0);
        ui.label(
            RichText::new(
                "Its outputs are not authoritative for any operational, licensing or safety \
                 purpose, and the simulator behind it is a demonstration model, not a \
                 validated model of a specific facility.",
            )
            .color(MUTED_TEXT),
        );
        ui.add_space(8.0);

        ui.heading("What this client does on the network");
        ui.label(format!(
            "It listens passively for mDNS announcements on {CIET_MDNS_SERVICE_TYPE} and \
             lists what arrives. It does NOT scan, sweep, probe or fingerprint the network, \
             and it never contacts an address that was not either announced by a simulator \
             or typed in by you."
        ));
        ui.add_space(8.0);

        ui.heading("No fabricated values");
        ui.label(format!(
            "Any quantity this client has not actually read from the server is shown as \
             \"{UNREAD_PLACEHOLDER}\". There is no placeholder number, no zero default, no \
             interpolation, and no carry-over from a previous session -- readings are \
             cleared whenever a new connection is started. A value whose OPC-UA type does \
             not match the node's declared type is dropped rather than coerced."
        ));
        ui.add_space(8.0);

        ui.heading("How it talks to the simulator");
        egui::Grid::new("ciet_client_facts")
            .num_columns(2)
            .striped(true)
            .spacing([14.0, 3.0])
            .show(ui, |ui| {
                ui.label("Client application name");
                ui.label(RichText::new(APPLICATION_NAME).monospace());
                ui.end_row();

                ui.label("Default endpoint form");
                ui.label(
                    RichText::new(format!(
                        "opc.tcp://<host>:{DEFAULT_OPCUA_PORT}{ENDPOINT_PATH}"
                    ))
                    .monospace(),
                );
                ui.end_row();

                ui.label("CIET namespace URI");
                ui.label(RichText::new(CIET_NAMESPACE_URI).monospace());
                ui.end_row();

                ui.label("Namespace index");
                ui.label(match &state.connection {
                    crate::shared_state::ConnectionState::Connected {
                        namespace_index, ..
                    } => RichText::new(format!(
                        "ns={namespace_index}, read from this server's namespace array"
                    ))
                    .monospace(),
                    _ => RichText::new(
                        "resolved from the server at connect time -- never hard-coded",
                    )
                    .color(MUTED_TEXT),
                });
                ui.end_row();

                ui.label("Variables");
                ui.label(format!(
                    "{} total: {} read-only outputs, {} continuous controls, {} switches",
                    total_node_count(),
                    CietSignal::ALL.len(),
                    CietControl::ALL.len(),
                    CietSwitch::ALL.len()
                ));
                ui.end_row();

                ui.label("Update mechanism");
                ui.label(match &state.connection {
                    crate::shared_state::ConnectionState::Connected { transport, .. } => {
                        RichText::new(transport.label())
                    }
                    _ => RichText::new(format!(
                        "subscription at {UPDATE_INTERVAL_MS} ms, falling back to polling if \
                         the server refuses one"
                    ))
                    .color(MUTED_TEXT),
                });
                ui.end_row();

                ui.label("Set-point clamping");
                ui.label(
                    "Performed by the SERVER, to each control's documented valid range. \
                     This client sends values unclamped so the clamp is visible in the \
                     read-back.",
                );
                ui.end_row();
            });

        ui.add_space(8.0);
        ui.heading("Status of this software");
        ui.label(
            RichText::new(
                "AI-assisted implementation. Treat it as untrusted draft material until a \
                 human maintainer has reviewed it: it has unit tests for its non-GUI logic, \
                 but the end-to-end path against a live simulator on a real network has not \
                 been verified in the environment it was written in (no display, no \
                 multicast). See the crate README's bookkeeping status.",
            )
            .color(MUTED_TEXT),
        );
        ui.add_space(4.0);
        ui.label(
            RichText::new("Licence: GPL-3.0, as part of the OUTRAM PARK backend workspace.")
                .color(MUTED_TEXT)
                .small(),
        );
    });
}
