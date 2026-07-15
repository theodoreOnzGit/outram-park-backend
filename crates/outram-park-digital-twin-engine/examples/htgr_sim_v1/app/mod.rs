//! GUI application layer, built on the engine's reusable scaffold.
//!
//! [`HtgrSimApp`] is the `eframe::App`. Its threading and panel dispatch use
//! the engine's [`app_scaffold`](outram_park_digital_twin_engine::app_scaffold)
//! module rather than hand-rolled `Arc<Mutex>`/`thread::spawn` boilerplate:
//!
//! - [`SharedState`](outram_park_digital_twin_engine::app_scaffold::SharedState)
//!   holds the physics snapshot and the plot ring buffers.
//! - [`spawn_physics_thread`](outram_park_digital_twin_engine::app_scaffold::spawn_physics_thread)
//!   runs the physics loop and the plot-sampler loop.
//! - [`panel_selector_ui`](outram_park_digital_twin_engine::app_scaffold::panel_selector_ui)
//!   / [`PanelSet`](outram_park_digital_twin_engine::app_scaffold::PanelSet)
//!   draw and dispatch the tab row.
//!
//! What belongs here: the app wiring and per-panel rendering. What does not:
//! the physics (that is [`crate::physics`]) or the visual widgets (those are
//! the engine crate's [`components`](outram_park_digital_twin_engine::components)).

pub mod panels;
pub mod schematic;
pub mod state;

use std::thread;
use std::time::Duration;

use uom::si::f64::{MassRate, Time};
use uom::si::mass_rate::kilogram_per_second;
use uom::si::time::second;

use outram_park_digital_twin_engine::app_scaffold::{
    panel_selector_ui, spawn_physics_thread, SharedState,
};

use crate::physics::HtgrPlant;
use panels::{
    draw_controls, draw_diagnostics_panel, draw_plots_panel, draw_schematic_panel, Panel,
};
use state::{HtgrPlotData, HtgrSnapshot};

/// Physics timestep per substep \[s\].
const PHYSICS_DT_S: f64 = 1.0e-3;
/// Physics substeps advanced per wall tick (a mild real-time speed-up).
const SUBSTEPS_PER_TICK: usize = 10;
/// Wall-clock sleep between physics ticks.
const PHYSICS_TICK: Duration = Duration::from_millis(10);
/// Wall-clock sleep between plot samples.
const PLOT_TICK: Duration = Duration::from_millis(50);

/// The HTGR simulator `eframe::App`.
pub struct HtgrSimApp {
    /// Shared scalar plant state (physics thread writes outputs, GUI writes
    /// control inputs).
    physics: SharedState<HtgrSnapshot>,
    /// Shared plot ring buffers (plot-sampler thread writes, GUI reads).
    plots: SharedState<HtgrPlotData>,
    /// Currently open panel.
    open_panel: Panel,
}

impl HtgrSimApp {
    /// Construct the app and spawn the physics + plot threads via the engine
    /// scaffold.
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        let physics = SharedState::new(HtgrSnapshot::default());
        let plots = SharedState::new(HtgrPlotData::default());

        // Physics thread: owns the (non-Clone) HtgrPlant, reads control inputs
        // from the shared state, steps the plant, writes outputs back.
        let mut plant = HtgrPlant::new();
        let dt = Time::new::<second>(PHYSICS_DT_S);
        spawn_physics_thread(physics.clone(), move |state| {
            let (rho, flow) = state.read_with(|s| {
                (
                    s.external_reactivity_dollars,
                    s.helium_flow_setpoint_kg_per_s,
                )
            });
            let flow_rate = MassRate::new::<kilogram_per_second>(flow);
            for _ in 0..SUBSTEPS_PER_TICK {
                plant.step(dt, rho, flow_rate);
            }
            state.update(|s| plant.write_snapshot(s));
            thread::sleep(PHYSICS_TICK);
        });

        // Plot-sampler thread: reads the physics snapshot, appends to the plot
        // buffers. Handed the physics SharedState; captures the plot one.
        let plots_for_sampler = plots.clone();
        spawn_physics_thread(physics.clone(), move |state| {
            let snapshot = state.snapshot();
            plots_for_sampler.update(|p| p.push_sample(&snapshot));
            thread::sleep(PLOT_TICK);
        });

        Self {
            physics,
            plots,
            open_panel: Panel::Schematic,
        }
    }
}

impl eframe::App for HtgrSimApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let snapshot = self.physics.snapshot();

        egui::Panel::top("htgr_top").show_inside(ui, |ui| {
            ui.heading(
                "HTGR Educational Simulator v1 -- scaffold (OUTRAM PARK digital-twin engine)",
            );
            ui.horizontal(|ui| {
                egui::global_theme_preference_buttons(ui);
                ui.separator();
                panel_selector_ui(ui, &mut self.open_panel);
            });
            ui.separator();
        });

        egui::Panel::right("htgr_controls").show_inside(ui, |ui| {
            draw_controls(ui, &self.physics, &snapshot);
        });

        egui::CentralPanel::default().show_inside(ui, |ui| {
            egui::ScrollArea::both().show(ui, |ui| match self.open_panel {
                Panel::Schematic => draw_schematic_panel(ui, &snapshot),
                Panel::Plots => {
                    let plots = self.plots.snapshot();
                    draw_plots_panel(ui, &plots);
                }
                Panel::Diagnostics => draw_diagnostics_panel(ui, &snapshot),
            });
        });

        // Keep animating while physics runs on its own threads.
        ui.ctx().request_repaint();
    }
}
