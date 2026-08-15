//! The `eframe::App` for the distillation-column simulator: physics-thread
//! orchestration, panel dispatch, and the restart-on-crash flow. Mirrors
//! `htgr_sim_v1::app`'s structure (`HtgrSimApp` / `start_simulation` /
//! `restart_simulation`) for a column instead of a reactor.
//!
//! # Pacing
//!
//! [`PHYSICS_TICK`] is the wall-clock deadline per tick, matching
//! `htgr_sim_v1`'s 100 ms. Unlike that simulator, this one does **not** run
//! 1:1 with wall time -- [`crate::physics::DistillationPlant::step`] advances
//! [`crate::physics::SUBSTEPS_PER_TICK`] RK4 steps internally per call, so one
//! [`PHYSICS_TICK`] of wall clock advances ~10 s of plant time (see
//! `physics`'s module doc for why: the validated relaxation transient takes
//! ~4300 s of plant time, which would otherwise take over an hour to watch).

pub mod panels;
pub mod schematic;
pub mod state;

use std::thread;
use std::time::{Duration, Instant};

use outram_park_digital_twin_engine::app_scaffold::{
    panel_selector_ui, show_crash_modal_with_restart, spawn_physics_thread_monitored,
    CrashModalOutcome, RealTimePacer, SharedState, ThreadHealth,
};
use uom::si::f64::Time;
use uom::si::time::second;

use crate::physics::{DistillationPlant, PlantCommands};
use panels::{draw_controls, draw_diagnostics_panel, draw_plots_panel, draw_schematic_panel, Panel};
use state::{ColumnPlotData, ColumnSnapshot};

/// Wall-clock budget for one physics tick, work *and* sleep -- a deadline,
/// not a fixed sleep (same reasoning as `htgr_sim_v1::app::PHYSICS_TICK`).
const PHYSICS_TICK: Duration = Duration::from_millis(100);
/// Wall-clock sleep between plot samples, matched to [`PHYSICS_TICK`].
const PLOT_TICK: Duration = Duration::from_millis(100);

/// Plant time advanced by one physics tick \[s\] -- what [`RealTimePacer`]
/// paces against. `SUBSTEPS_PER_TICK * RK4_DT_S` of plant time per
/// [`PHYSICS_TICK`] of wall time.
fn plant_time_per_tick() -> Time {
    Time::new::<second>(crate::physics::SUBSTEPS_PER_TICK as f64 * crate::physics::RK4_DT_S)
}

/// Read the operator's [`PlantCommands`] off the shared snapshot -- the same
/// "control inputs live on the snapshot, physics reads them back" convention
/// `htgr_sim_v1::app::plant_commands_from` uses.
fn plant_commands_from(s: &ColumnSnapshot) -> PlantCommands {
    PlantCommands {
        reflux_ratio: s.reflux_ratio,
        reboiler_duty_watts: s.reboiler_duty_watts,
    }
}

/// The distillation simulator `eframe::App`.
pub struct DistColSimApp {
    /// Shared scalar/per-stage plant state (physics thread writes outputs,
    /// GUI writes control inputs).
    physics: SharedState<ColumnSnapshot>,
    /// Shared plot ring buffers (plot-sampler thread writes, GUI reads).
    plots: SharedState<ColumnPlotData>,
    /// Health of the two background threads; drives the crash modal.
    thread_health: ThreadHealth,
    /// Which central-panel tab is open.
    open_panel: Panel,
}

/// The three handles one simulation run owns -- bundled so
/// [`start_simulation`] and [`DistColSimApp::restart_simulation`] build and
/// tear down all three together, never partially.
struct SimulationRun {
    physics: SharedState<ColumnSnapshot>,
    plots: SharedState<ColumnPlotData>,
    thread_health: ThreadHealth,
}

/// Build a fresh simulation run: construct the plant, spawn the physics
/// thread and the plot-sampler thread, return the handles.
///
/// Mirrors `htgr_sim_v1::app::start_simulation`'s two-thread pattern exactly:
/// one thread owns the non-`Clone` plant model and steps it, reading commands
/// off the shared snapshot and writing outputs back; a second, independent
/// thread only samples snapshots into the plot ring buffers on its own
/// cadence. Both share one [`ThreadHealth`].
fn start_simulation() -> SimulationRun {
    let physics = SharedState::new(ColumnSnapshot::default());
    let plots = SharedState::new(ColumnPlotData::default());
    let thread_health = ThreadHealth::new();

    let mut plant = DistillationPlant::new();
    let mut pacer = RealTimePacer::new(plant_time_per_tick(), PHYSICS_TICK);
    let loop_start = Instant::now();

    spawn_physics_thread_monitored(
        "distillation-physics",
        physics.clone(),
        thread_health.clone(),
        move |state| {
            let tick_start = Instant::now();
            let commands = state.read_with(plant_commands_from);
            plant.step(commands);
            let pacing = pacer.pace(tick_start.elapsed(), loop_start.elapsed());
            state.update(|s| plant.write_snapshot(s));
            thread::sleep(pacing.sleep_for);
        },
    );

    let plots_for_sampler = plots.clone();
    spawn_physics_thread_monitored(
        "distillation-plot-sampler",
        physics.clone(),
        thread_health.clone(),
        move |state| {
            let snapshot = state.snapshot();
            plots_for_sampler.update(|p| p.push_sample(&snapshot));
            thread::sleep(PLOT_TICK);
        },
    );

    SimulationRun {
        physics,
        plots,
        thread_health,
    }
}

impl DistColSimApp {
    /// Build the app and launch its first simulation run.
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        let run = start_simulation();
        Self {
            physics: run.physics,
            plots: run.plots,
            thread_health: run.thread_health,
            open_panel: Panel::Schematic,
        }
    }

    /// Tear down the current run and start a brand-new one at the default
    /// operating point -- called only from the crash modal's **Restart
    /// simulation** button.
    ///
    /// Mirrors `htgr_sim_v1::app::HtgrSimApp::restart_simulation`: retires
    /// (does not join) the old [`ThreadHealth`] so both old threads exit at
    /// their next loop check, then rebuilds `physics`/`plots`/`thread_health`
    /// via a fresh [`start_simulation`] call. `open_panel` (a display
    /// preference, not plant state) is left as the operator had it.
    fn restart_simulation(&mut self) {
        self.thread_health.retire();

        let run = start_simulation();
        self.physics = run.physics;
        self.plots = run.plots;
        self.thread_health = run.thread_health;
    }
}

impl eframe::App for DistColSimApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        // If a physics/plot thread panicked, show the crash modal and stop --
        // do not render the (now frozen, possibly half-written) plant behind
        // it. The modal offers an in-app restart; honour the click here, then
        // still return: the new run has not taken its first step yet.
        let crash = show_crash_modal_with_restart(ui.ctx(), &self.thread_health);
        if crash == CrashModalOutcome::RestartRequested {
            self.restart_simulation();
        }
        if crash.is_crashed() {
            ui.ctx().request_repaint();
            return;
        }

        let snapshot = self.physics.snapshot();

        egui::Panel::top("dist_top").show_inside(ui, |ui| {
            ui.heading("Distillation Column Simulator v1 -- OUTRAM PARK digital-twin engine");
            ui.horizontal(|ui| {
                egui::global_theme_preference_buttons(ui);
                ui.separator();
                panel_selector_ui(ui, &mut self.open_panel);
            });
            ui.separator();
        });

        egui::Panel::right("dist_controls").show_inside(ui, |ui| {
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

        ui.ctx().request_repaint();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// V&V -- restarting starts a fresh run and abandons the old one's state,
    /// mirroring `htgr_sim_v1`'s own
    /// `restarting_starts_a_fresh_run_and_abandons_the_old_one` test and the
    /// safety argument for an in-app restart button it documents.
    ///
    /// **Methodology.** Let run 1 advance past `sim_time_s = 0`, force a
    /// crash via `thread_health`, restart, and check the new run's clock did
    /// not carry the old run's `sim_time_s` forward.
    ///
    /// **Result (measured 2026-08-15):** run 1 reaches several ticks'
    /// worth of `sim_time_s` before the restart; after `restart_simulation`,
    /// the new run's `sim_time_s` is at most one tick's advance rather than
    /// continuing from run 1's value -- confirming the restart discards the
    /// crashed run's state rather than resuming it.
    ///
    /// **Test-design note.** `sim_time_s` only ever takes multiples of
    /// [`crate::physics::SUBSTEPS_PER_TICK`] `*`
    /// [`crate::physics::RK4_DT_S`] (10 s per tick here), because a whole
    /// tick's substeps complete before the physics thread publishes a
    /// snapshot. An earlier version of this test captured `advanced_to` at
    /// exactly one tick (10 s) and asserted `after_restart < advanced_to`,
    /// which is not a robust bound at that quantisation: the new run's own
    /// first tick can complete inside the post-restart sleep and also read
    /// 10 s, failing an otherwise-correct restart. Letting run 1 advance
    /// several ticks before restarting gives real headroom regardless of
    /// exact tick timing.
    #[test]
    fn restarting_starts_a_fresh_run_and_abandons_the_old_one() {
        let run = start_simulation();
        let mut app = DistColSimApp {
            physics: run.physics,
            plots: run.plots,
            thread_health: run.thread_health,
            open_panel: Panel::Schematic,
        };

        // Give the physics thread several ticks to advance well past one
        // tick's worth of plant time.
        let min_advance_s =
            3.0 * crate::physics::SUBSTEPS_PER_TICK as f64 * crate::physics::RK4_DT_S;
        let deadline = Instant::now() + Duration::from_secs(5);
        let advanced_to = loop {
            let t = app.physics.snapshot().sim_time_s;
            if t >= min_advance_s || Instant::now() > deadline {
                break t;
            }
            thread::sleep(Duration::from_millis(10));
        };
        assert!(
            advanced_to >= min_advance_s,
            "test setup failed: physics thread only reached {advanced_to} s in the time \
             budget (would pass even if restart did nothing)"
        );

        app.restart_simulation();

        thread::sleep(Duration::from_millis(50));
        let after_restart = app.physics.snapshot().sim_time_s;
        let one_tick_s = crate::physics::SUBSTEPS_PER_TICK as f64 * crate::physics::RK4_DT_S;
        assert!(
            after_restart <= one_tick_s,
            "the restarted plant clock ({after_restart} s) must not carry over run 1's \
             ({advanced_to} s) -- at most one fresh tick ({one_tick_s} s) is expected"
        );

        app.thread_health.retire();
    }
}
