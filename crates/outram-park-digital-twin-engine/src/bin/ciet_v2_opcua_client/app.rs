//! The `eframe` application: panel dispatch, repaint pacing, and the one place
//! shared state is mutated from the GUI side.
//!
//! ## The repaint never waits on the network
//!
//! [`CietOpcUaClientApp::update`] does three things: read the shared state under a
//! short read lock, draw, and push any commands the user generated under a short
//! write lock. It never awaits, never connects, and never reads a socket — all of
//! that is on the worker thread ([`crate::worker`]).
//!
//! It also asks for a repaint on a timer ([`REPAINT_INTERVAL_MS`]) rather than
//! relying on input events, because the interesting changes come from the *server*
//! and would otherwise not be drawn until the user moved the mouse.
//!
//! ## Panels are pure renderers
//!
//! Each panel takes `&ClientSharedState` and returns what the user asked for
//! (a [`DiscoveryAction`], a `Vec<ClientCommand>`). None of them holds the lock or
//! mutates shared state. That is what keeps the lock hold times short enough that
//! the worker is never blocked by a repaint.

use std::sync::Arc;

use crate::browse::DiscoveryPoller;
use crate::drafts::ControlDrafts;
use crate::shared_state::{ClientCommand, SharedClientState};
use crate::ui::discovery_panel::DiscoveryAction;
use crate::ui::{security_banner, status_strip, Panel};

/// How often a repaint is requested while the window is open, milliseconds.
///
/// Matched to the client's 250 ms value-update rate: repainting faster would
/// redraw identical frames, and slower would make a live reading look laggy.
pub const REPAINT_INTERVAL_MS: u64 = 250;

/// The demo client's `eframe` application.
///
/// Owns everything GUI-local — the selected tab, the manual-address text buffer,
/// the control drafts, the mDNS poller — and shares exactly one thing with the
/// OPC-UA worker: [`SharedClientState`].
///
/// No lifetime parameters and no `Box<dyn Trait>`: the shared state is behind an
/// `Arc`, the panels are an enum, and the worker handle is owned by value
/// (workspace Rust design rules).
pub struct CietOpcUaClientApp {
    /// State shared with the OPC-UA worker thread.
    shared: SharedClientState,

    /// The worker's join handle, kept so the thread is owned rather than
    /// detached. Never joined — the thread runs until the process exits.
    _worker: std::thread::JoinHandle<()>,

    /// Passive mDNS discovery, polled once per second from `update`.
    discovery: DiscoveryPoller,

    /// The selected tab.
    panel: Panel,

    /// Text in the manual-address box, kept across frames so a typing user does
    /// not lose it on a repaint.
    manual_entry: String,

    /// Last manual-address parse error, kept across frames so the message
    /// persists instead of flashing for one frame.
    manual_error: Option<String>,

    /// Pending slider / numeric-box values per control.
    drafts: ControlDrafts,
}

impl CietOpcUaClientApp {
    /// Build the app, start the OPC-UA worker thread, and start listening for
    /// mDNS announcements.
    ///
    /// Neither the worker nor the mDNS browser connects to anything on start-up:
    /// the client is [`Disconnected`](crate::shared_state::ConnectionState::Disconnected)
    /// until the user picks a simulator or types an address.
    ///
    /// # Arguments
    ///
    /// * `context` — the egui context, used only to make the default font a
    ///   little larger; no state is stored in it.
    pub fn new(context: &egui::Context) -> Self {
        context.set_pixels_per_point(1.1);

        let shared = crate::shared_state::new_shared_client_state();
        let worker = crate::worker::spawn_client_worker(Arc::clone(&shared));

        Self {
            shared,
            _worker: worker,
            discovery: DiscoveryPoller::start(),
            panel: Panel::Connect,
            manual_entry: String::new(),
            manual_error: None,
            drafts: ControlDrafts::new(),
        }
    }

    /// Queue commands for the worker under a short write lock.
    fn queue(&self, commands: Vec<ClientCommand>) {
        if commands.is_empty() {
            return;
        }
        if let Ok(mut state) = self.shared.write() {
            for command in commands {
                state.push_command(command);
            }
        }
    }
}

impl eframe::App for CietOpcUaClientApp {
    /// Draw one frame.
    ///
    /// Takes the shared read lock once, draws from that snapshot, releases it, then
    /// takes the write lock only if the user actually asked for something.
    fn ui(&mut self, root_ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        // Server-driven changes need a timed repaint; input events are not enough.
        root_ui
            .ctx()
            .request_repaint_after(std::time::Duration::from_millis(REPAINT_INTERVAL_MS));

        self.discovery.poll();

        let mut commands: Vec<ClientCommand> = Vec::new();
        let mut selected_panel = self.panel;

        // One read lock for the whole frame's drawing. The panels only read.
        let Ok(state) = self.shared.read() else {
            // A poisoned lock means the worker panicked. Say so rather than
            // drawing a frame that would silently show stale values.
            egui::CentralPanel::default().show_inside(root_ui, |ui| {
                ui.heading("The OPC-UA worker thread has failed");
                ui.label(
                    "The shared state lock is poisoned, which means the client's network \
                     thread panicked. No values shown would be trustworthy. Restart the \
                     client.",
                );
            });
            return;
        };

        egui::Panel::top("ciet_client_top").show_inside(root_ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.heading("CIET v2 OPC-UA demo client");
                ui.label(
                    egui::RichText::new("offline educational demonstration")
                        .color(crate::ui::MUTED_TEXT)
                        .small(),
                );
            });

            // Always-on security banner while a session is up. Not dismissible.
            if state.connection.is_connected() {
                security_banner(ui);
            }

            ui.add_space(4.0);
            ui.horizontal_wrapped(|ui| {
                for panel in Panel::ALL {
                    ui.selectable_value(&mut selected_panel, *panel, panel.label());
                }
            });
        });

        egui::Panel::bottom("ciet_client_status").show_inside(root_ui, |ui| {
            status_strip(ui, &state);
        });

        egui::CentralPanel::default().show_inside(root_ui, |ui| match selected_panel {
            Panel::Connect => {
                let action = crate::ui::discovery_panel::show(
                    ui,
                    &self.discovery,
                    &state,
                    &mut self.manual_entry,
                    &mut self.manual_error,
                );
                match action {
                    DiscoveryAction::None => {}
                    DiscoveryAction::Connect { endpoint_url } => {
                        commands.push(ClientCommand::Connect { endpoint_url });
                    }
                    DiscoveryAction::Disconnect => {
                        commands.push(ClientCommand::Disconnect);
                    }
                }
            }
            Panel::Outputs => crate::ui::outputs_panel::show(ui, &state),
            Panel::Trends => crate::ui::trends_panel::show(ui, &state),
            Panel::Controls => {
                commands.extend(crate::ui::controls_panel::show(
                    ui,
                    &state,
                    &mut self.drafts,
                ));
            }
            Panel::About => crate::ui::about_panel::show(ui, &state),
        });

        // Release the read lock before taking the write lock.
        drop(state);

        self.panel = selected_panel;

        // A new connection or a disconnect invalidates the proposed set points.
        if commands.iter().any(|command| {
            matches!(
                command,
                ClientCommand::Connect { .. } | ClientCommand::Disconnect
            )
        }) {
            self.drafts.clear_all();
        }

        self.queue(commands);
    }
}
