//! The `eframe` application for the CIET Educational Simulator v2.
//!
//! ## Provenance
//!
//! The `CIETApp` struct, its panel layout and its page dispatch are ported from
//! the CIET Educational Simulator **v1**
//! (`crates/tuas_boussinesq_solver/examples/ciet_educational_simulator/ciet_simulator_v1/app.rs`),
//! GPL-3.0, same licence.
//!
//! ## What v2 changed
//!
//! - **The app no longer owns the simulation.** v1's `CIETApp::new` spawned the
//!   physics thread. In v2 `main.rs` spawns the physics thread *and* the OPC-UA
//!   server thread before the window opens, then hands the app the shared state
//!   handle. This is what lets the same physics run headless on Termux, and it
//!   means the GUI is one client of the plant state rather than its owner.
//! - **`Arc<Mutex<CIETState>>` became `Arc<RwLock<CietState>>`.** Read-only page
//!   accesses take a read lock, so the GUI and the OPC-UA read callbacks do not
//!   serialise against each other.
//! - **Frequency response moved out of the repaint callback.** v1 evaluated the
//!   advanced-heater-control signal here, in `eframe::App::ui`, once per frame.
//!   v2 evaluates it in the physics thread once per timestep — see
//!   `panels_and_pages::full_simulation`. The same applies to the heater-mesh
//!   choice, which v1 pushed into shared state every frame and v2 writes only on
//!   change.
//! - **A new "OPC-UA Server" page** ([`Panel::OpcuaServer`]) explains how to
//!   connect a client, what the (absent) security model is, and lists every
//!   node.
//! - **`serde` persistence dropped.** v1 derived `Serialize`/`Deserialize` for
//!   `eframe`'s app-state blob but its restore path was commented out, so the
//!   blob was written and never read. Keeping it would have forced a `Default`
//!   impl handing out a shared-state handle disconnected from the physics
//!   thread.
//!
//! The physics itself is unchanged from v1. Per `RESPONSIBLE_USE.md` this is an
//! **offline educational demonstration** and, as a port, its **equivalence to
//! v1 has not yet been verified**.

use std::{thread, time::Duration};

use outram_park_digital_twin_engine::ciet_opcua::state::SharedCietState;

use crate::opcua_startup::OpcuaStatus;

use super::panels_and_pages::{
    ciet_data::PagePlotData, frequency_response_and_transients::FreqResponseAndTransientSettings,
    Panel,
};
use super::useful_functions::update_ciet_plot_from_ciet_state;
use std::sync::{Arc, Mutex};

/// The CIET Educational Simulator v2 desktop application.
///
/// Holds no plant state of its own: [`Self::ciet_state`] is a handle onto the
/// state the physics thread integrates and the OPC-UA server serves. Everything
/// else here is presentation — which page is open, the plot history snapshot the
/// current frame is drawing, and the two GUI-local step-response fields.
pub struct CIETApp {
    /// Which page the user has open. Written by the main page's shortcut
    /// button as well as by the selector row at the top.
    pub(crate) open_panel: Panel,

    /// Handle onto the shared plant state. Read by every page; written by the
    /// control widgets. Owned jointly with the physics thread and the OPC-UA
    /// server.
    pub(crate) ciet_state: SharedCietState,

    /// Handle onto the plot/CSV history, written by the recorder thread spawned
    /// in [`CIETApp::new`]. Still an `Arc<Mutex<_>>` rather than an `RwLock`:
    /// there is exactly one writer and one reader, so there is no
    /// reader-reader contention for an `RwLock` to relieve.
    pub(crate) ciet_plot_data_mutex_ptr_for_parallel_data_transfer: Arc<Mutex<PagePlotData>>,

    /// The snapshot of the plot history this frame draws from. Refreshed from
    /// the shared handle when the user presses "Update CSV Data".
    pub(crate) ciet_plot_data: PagePlotData,

    /// The GUI-local remainder of the advanced-heater-control settings (the
    /// step-response button state). Everything else now lives in shared state.
    pub(crate) frequency_response_settings: FreqResponseAndTransientSettings,

    /// Checkbox state: user wants faster-than-real-time pacing.
    pub(crate) user_wants_fast_fwd_on: bool,

    /// Checkbox state: user wants slower-than-real-time pacing.
    pub(crate) user_wants_slow_motion_on: bool,

    /// Whether the embedded OPC-UA server started, and how to reach it.
    /// Displayed by the "OPC-UA Server" page. A failure here never stops the
    /// simulator.
    pub(crate) opcua_status: OpcuaStatus,
}

impl CIETApp {
    /// Build the app around an already-running simulation.
    ///
    /// `ciet_state` must be the same handle the physics thread is integrating —
    /// `main.rs` spawns that thread first and passes its handle in here.
    /// `opcua_status` records whether the OPC-UA server came up; a
    /// [`OpcuaStatus::Failed`] is displayed on the OPC-UA page and otherwise
    /// ignored, so the simulator is usable with no server at all.
    ///
    /// Spawns one thread of its own: the plot-history recorder, which samples
    /// the shared state at the user-chosen interval so the trend plots have a
    /// history to draw.
    pub fn new(
        _cc: &eframe::CreationContext<'_>,
        ciet_state: SharedCietState,
        opcua_status: OpcuaStatus,
    ) -> Self {
        let ciet_plot_data = Arc::new(Mutex::new(PagePlotData::default()));

        // this is the current state of ciet for plotting
        // like the instantaneous temperature and such
        let ciet_state_ptr_for_plotting: SharedCietState = ciet_state.clone();
        // for data recording,
        // this contains arrays with historical data
        let ciet_plot_ptr: Arc<Mutex<PagePlotData>> = ciet_plot_data.clone();

        // spawn a thread to update the plotting bits
        thread::spawn(move || {
            update_ciet_plot_from_ciet_state(ciet_state_ptr_for_plotting, ciet_plot_ptr);
        });

        Self {
            open_panel: Panel::MainPage,
            ciet_state,
            ciet_plot_data_mutex_ptr_for_parallel_data_transfer: ciet_plot_data,
            ciet_plot_data: PagePlotData::default(),
            frequency_response_settings: FreqResponseAndTransientSettings::default(),
            user_wants_fast_fwd_on: false,
            user_wants_slow_motion_on: false,
            opcua_status,
        }
    }
}

impl eframe::App for CIETApp {
    /// Called each time the UI needs repainting, which may be many times per
    /// second.
    ///
    /// This is presentation only. Unlike v1, nothing here drives the plant: the
    /// advanced-heater-control signal is applied by the physics thread, so
    /// closing the window is the only thing that stops the simulation.
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        // eframe 0.34 hands us a root `Ui`; panels are nested into it with
        // `show_inside`, and the `CentralPanel` must come last.
        // For inspiration and more examples, go to https://emilk.github.io/egui

        egui::Panel::top("top_panel").show(ui, |ui| {
            // The top panel is often a good place for a menu bar:

            egui::MenuBar::new().ui(ui, |ui| {
                egui::widgets::global_theme_preference_buttons(ui);
            });

            ui.heading("CIET Educational Simulator v2");
            ui.separator();
            // allow user to select which panel is open
            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.open_panel, Panel::MainPage, "Main Page");
                ui.selectable_value(&mut self.open_panel, Panel::Heater, "Heater");
                ui.selectable_value(&mut self.open_panel, Panel::CTAH, "CTAH");
                ui.selectable_value(&mut self.open_panel, Panel::CTAHPump, "CTAH Pump");
                ui.selectable_value(&mut self.open_panel, Panel::TCHX, "TCHX");
                ui.selectable_value(&mut self.open_panel, Panel::DHX, "DHX STHE");
                ui.selectable_value(
                    &mut self.open_panel,
                    Panel::FrequencyResponseAndTransients,
                    "Frequency Response and Transients",
                );
                ui.selectable_value(
                    &mut self.open_panel,
                    Panel::OnlineCalibration,
                    "Online Calibration",
                );
                ui.selectable_value(
                    &mut self.open_panel,
                    Panel::NodalisedDiagram,
                    "CIET Nodalised Diagram",
                );
                ui.selectable_value(&mut self.open_panel, Panel::OpcuaServer, "OPC-UA Server");
            });
            ui.separator();
        });

        egui::Panel::right("Supplementary Info").show(ui, |ui| match self.open_panel {
            Panel::MainPage => {
                egui::ScrollArea::both().show(ui, |ui| {
                    self.ciet_main_page_side_panel(ui);
                    self.citation_disclaimer_and_acknowledgements(ui);
                });
            }
            Panel::CTAHPump => {
                self.ciet_sim_ctah_pump_page_csv(ui);
            }
            Panel::CTAH => {
                self.ciet_sim_ctah_page_csv(ui);
            }
            Panel::Heater => {
                // display csv file on side panel when heater page
                // is open
                self.ciet_sim_heater_page_csv(ui);
            }
            Panel::DHX => {
                self.ciet_sim_dhx_page_csv(ui);
            }
            Panel::TCHX => {
                self.ciet_sim_tchx_page_csv(ui);
            }
            Panel::FrequencyResponseAndTransients => {
                self.ciet_sim_heater_page_csv(ui);
            }
            Panel::NodalisedDiagram => {}
            Panel::OnlineCalibration => {}
            Panel::OpcuaServer => {
                self.ciet_sim_opcua_page_side_panel(ui);
            }
        });

        egui::Panel::bottom("github").show(ui, |ui| {
            ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                powered_by_egui_and_eframe(ui);
                egui::warn_if_debug_build(ui);
            });
        });

        // CentralPanel must be added last so it fills the remaining space.
        egui::CentralPanel::default().show(ui, |ui| {
            // show correct panel or page based on user selection

            match self.open_panel {
                Panel::FrequencyResponseAndTransients => {
                    self.ciet_sim_transients_and_freq_response_page(ui);
                }
                Panel::MainPage => {
                    self.ciet_sim_main_page_central_panel(ui);
                }
                Panel::CTAHPump => {
                    self.ciet_sim_ctah_pump_page_and_graphs(ui);
                }
                Panel::CTAH => {
                    self.ciet_sim_ctah_page_graph(ui);
                }
                Panel::Heater => {
                    self.ciet_sim_heater_page_graph(ui);
                }
                Panel::DHX => {
                    self.ciet_sim_dhx_branch_page_graph(ui);
                }
                Panel::TCHX => {
                    self.ciet_sim_tchx_page_graph(ui);
                }
                Panel::NodalisedDiagram => {
                    // enables scrolling within the image
                    egui::ScrollArea::both().show(ui, |ui| {
                        ui.image(egui::include_image!("../ciet_sam_diagram_replica.jpg"));
                    });
                }
                Panel::OnlineCalibration => {
                    self.ciet_sim_online_calibration_page(ui);
                }
                Panel::OpcuaServer => {
                    self.ciet_sim_opcua_page(ui);
                }
            }

            ui.add(egui::github_link_file!(
                "https://github.com/theodoreOnzGit/outram-park-backend/blob/main/",
                "OUTRAM PARK backend Github Repo (TUAS)"
            ));
        });

        // NOTE (v2): v1 applied the advanced-heater-control / frequency-response
        // signal here, and pushed the heater-mesh choice into shared state, once
        // per repaint. Both moved: the signal is now evaluated by the physics
        // thread once per timestep (so it also works headless and under OPC-UA
        // control), and the mesh choice is written by the Online Calibration
        // page only when the user changes it.

        // request update every 50 ms
        ui.ctx().request_repaint_after(Duration::from_millis(50));
    }
}

/// The egui/eframe attribution footer, as in v1.
fn powered_by_egui_and_eframe(ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        ui.label("Powered by ");
        ui.hyperlink_to("egui", "https://github.com/emilk/egui");
        ui.label(" and ");
        ui.hyperlink_to(
            "eframe",
            "https://github.com/emilk/egui/tree/master/crates/eframe",
        );
        ui.label(".");
    });
}
