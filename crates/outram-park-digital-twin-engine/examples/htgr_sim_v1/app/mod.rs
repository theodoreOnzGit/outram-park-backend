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
//!
//! ## Real-time pacing
//!
//! The physics thread is paced against wall clock by
//! [`RealTimePacer`](outram_park_digital_twin_engine::app_scaffold::RealTimePacer),
//! the workspace house pattern taken from `fhr_sim_v2`: a tick's wall budget is
//! [`PHYSICS_TICK`], the plant clock and wall clock are compared
//! *cumulatively*, and a tick sleeps out the rest of its budget only while the
//! plant clock is at or ahead of wall clock. [`SUBSTEPS_PER_TICK`] substeps of
//! [`PHYSICS_DT_S`] is exactly one [`PHYSICS_TICK`] of plant time, so a tick
//! that meets its deadline advances the plant 1:1 with wall clock.
//!
//! **This loop used to sleep [`PHYSICS_TICK`] unconditionally *after* the
//! work**, making the wall period `compute + 10 ms` for 10 ms of plant time, so
//! 1:1 was structurally unreachable however cheap the physics got (kopi-beans
//! `op-v5zb`).
//!
//! ## The plant timestep is not set here
//!
//! [`PHYSICS_DT_S`] is a **read** of
//! [`crate::physics::PLANT_TIMESTEP_S`], the one constant the whole simulator
//! -- application, steam-generator sub-clock and every whole-plant test --
//! derives its step size from. It used to be an independent literal that
//! happened to agree with a literal in the physics tests, which is how the
//! application came to drive the plant at a rate no test covered (kopi-beans
//! `op-fngw`).
//!
//! ## Where the compute actually goes -- measured 2026-08-13
//!
//! `physics::tests::the_whole_plant_steps_at_the_gui_timestep` advances 20 s of
//! plant time through the same `HtgrPlant::step` this thread calls. Timed in
//! release over that same 20 s window, with the plant timestep varied and
//! nothing else changed:
//!
//! | Plant timestep | compute per second of plant time | real-time ratio |
//! |---|---|---|
//! | 1 ms (the old value) | 2.0327 | 0.492 |
//! | 10 ms | 1.9816 | 0.505 |
//! | 50 ms | 1.9495 | 0.513 |
//! | **100 ms (now)** | **1.9469** | **0.514** |
//! | *the steam generator alone* | *1.9585* | *--* |
//!
//! **The steam generator is ~96% of the plant's compute, and its cost does not
//! depend on the plant timestep** -- it is a multi-rate sub-model that
//! accumulates whatever `dt` it is handed and advances its three coupled arrays
//! on its own fixed clock (`crate::physics::steam_generator`). So raising the
//! plant timestep a hundredfold removed essentially all of the *other* 4% and
//! moved the ratio only from 0.492 to 0.514. **The timestep was not where the
//! cost was**, and that is worth stating plainly because it is the opposite of
//! what the change set out to prove.
//!
//! ## What did reach real time
//!
//! Halving the exchanger's own PIMPLE **outer-corrector** count, 4 to 2. Its
//! cost is very nearly exactly linear in `n_outer / substep`, and measured the
//! same day the settled duty is identical at 1, 2, 3 and 4 correctors, so the
//! other two were being paid for nothing. See
//! [`crate::physics::steam_generator::PimpleCorrectors`].
//!
//! Measured with the whole-plant test run **alone** (`--test-threads=1`), four
//! runs: **0.9646-0.9677** compute per plant second, **real-time ratio
//! 1.033-1.037**. Under load
//! -- the rest of the suite on the other eleven cores -- the same test reads
//! 0.562, so the readout on the schematic will move with what else the machine
//! is doing. That is the readout doing its job.
//!
//! ## Why the substep cannot go further, and why more correctors do not help
//!
//! The exchanger's array substep is pinned by an advective **Courant** limit on
//! the helium side: measured `Co = 0.222` at the shipped 0.0125 s substep,
//! `0.444` at 0.025 s, `0.888` at 0.05 s, `1.776` at the full 0.1 s plant step.
//! Raising it is the only lever left, and **raising the arrays' outer-corrector
//! count does not unlock it**: measured 2026-08-13, a 0.05 s substep panics at
//! 4, 8 and 16 outer correctors, and a 0.1 s substep panics at 8 and 32. See
//! [`crate::physics::STEAM_GENERATOR_SUBSTEPS_PER_PLANT_STEP`] for why -- the
//! arrays' enthalpy convection is an explicit source inside the corrector loop,
//! so the loop's Picard contraction factor *is* the Courant number.

pub mod panels;
pub mod schematic;
pub mod state;

use std::thread;
use std::time::{Duration, Instant};

use uom::si::f64::{MassRate, Time};
use uom::si::mass_rate::kilogram_per_second;
use uom::si::time::second;

use outram_park_digital_twin_engine::app_scaffold::{
    panel_selector_ui, show_crash_modal_if_crashed, spawn_physics_thread_monitored, RealTimePacer,
    SharedState, ThreadHealth,
};

use crate::physics::HtgrPlant;
use panels::{draw_controls, draw_diagnostics_panel, draw_plots_panel, draw_schematic_panel, Panel};
use schematic::SchematicTracers;
use state::{HtgrPlotData, HtgrSnapshot};

/// Physics timestep per substep \[s\] -- **a read of the global plant
/// timestep, not an independent value.**
///
/// This is the rate the *application* drives the plant at, and every test and
/// sub-model reads the same constant, so there is nothing left to keep in step
/// by hand. A simulator stepping at a rate no test covers is how the physics
/// thread came to die within 30 s of launch on 2026-08-12 (kopi-beans
/// `op-fngw`); that class of bug is now unrepresentable rather than merely
/// tested for.
const PHYSICS_DT_S: f64 = crate::physics::PLANT_TIMESTEP_S;
/// Physics substeps advanced per wall tick.
///
/// **One**, now that the plant timestep is 0.1 s: one substep of
/// [`PHYSICS_DT_S`] is 100 ms of plant time, which is exactly
/// [`PHYSICS_TICK`], so a tick that meets its deadline advances the plant at
/// **1:1 with wall clock**. See [`plant_time_per_tick`].
///
/// It was ten while the plant stepped at 1 ms. The constant is kept (rather
/// than folded away) because it is one of the three terms in the pacing
/// identity [`tests::the_physics_tick_is_paced_for_real_time`] checks, and
/// because a future timestep change may want a tick that spans several steps
/// again.
const SUBSTEPS_PER_TICK: usize = 1;
/// Wall-clock budget for one physics tick, work *and* sleep.
///
/// A **deadline**, not a fixed sleep: the thread sleeps whatever is left of
/// this after the work, via
/// [`RealTimePacer`](outram_park_digital_twin_engine::app_scaffold::RealTimePacer).
/// Sleeping this long *after* the work -- which is what this loop used to do --
/// makes the wall period `compute + tick` for one tick of plant time, so real
/// time is unreachable for any nonzero compute cost (kopi-beans `op-v5zb`).
///
/// 100 ms, matching `PHYSICS_DT_S * SUBSTEPS_PER_TICK`. The GUI's displayed
/// scalars therefore refresh at 10 Hz; the schematic's flow tracers are
/// unaffected because they are advanced from the plant clock at the repaint
/// rate, not at this one.
const PHYSICS_TICK: Duration = Duration::from_millis(100);
/// Wall-clock sleep between plot samples.
///
/// Matched to [`PHYSICS_TICK`]: sampling faster than the physics thread
/// publishes just writes the same snapshot into the ring buffer twice, which
/// halves the plot window for no extra information.
const PLOT_TICK: Duration = Duration::from_millis(100);

/// Plant time advanced by one physics tick \[s\].
///
/// Equal to [`PHYSICS_TICK`] when the simulator is paced 1:1, which is what
/// [`tests::the_physics_tick_is_paced_for_real_time`] pins.
fn plant_time_per_tick() -> Time {
    Time::new::<second>(PHYSICS_DT_S * SUBSTEPS_PER_TICK as f64)
}

/// The HTGR simulator `eframe::App`.
pub struct HtgrSimApp {
    /// Shared scalar plant state (physics thread writes outputs, GUI writes
    /// control inputs).
    physics: SharedState<HtgrSnapshot>,
    /// Shared plot ring buffers (plot-sampler thread writes, GUI reads).
    plots: SharedState<HtgrPlotData>,
    /// Crash flag shared with the physics + plot threads: if either panics, the
    /// GUI raises the restart modal (see [`show_crash_modal_if_crashed`]).
    thread_health: ThreadHealth,
    /// Currently open panel.
    open_panel: Panel,
    /// Flow-tracer trains for the schematic's connector runs. Owned here (not
    /// by the widgets, which are rebuilt every repaint) and advanced once per
    /// frame from the real loop residence times -- see
    /// [`outram_park_digital_twin_engine::animation`].
    tracers: SchematicTracers,
    /// Plant clock reading at the previous repaint \[s\].
    ///
    /// The tracers are advanced by the **simulated** time that elapsed between
    /// frames, so a mark travels at the transport speed of the plant it depicts
    /// even when the physics thread is not keeping up with wall clock.
    last_sim_time_s: f64,
}

impl HtgrSimApp {
    /// Construct the app and spawn the physics + plot threads via the engine
    /// scaffold.
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        let physics = SharedState::new(HtgrSnapshot::default());
        let plots = SharedState::new(HtgrPlotData::default());
        let thread_health = ThreadHealth::new();

        // Physics thread: owns the (non-Clone) HtgrPlant, reads control inputs
        // from the shared state, steps the plant, writes outputs back. Spawned
        // *monitored* so a panic (e.g. a steam-property call out of range) trips
        // the shared crash flag instead of silently freezing the sim.
        let mut plant = HtgrPlant::new();
        let dt = Time::new::<second>(PHYSICS_DT_S);
        // Real-time pacing, in the workspace house pattern lifted from
        // `fhr_sim_v2`: each tick gets PHYSICS_TICK of wall clock for its work
        // *and* its sleep, and the plant clock is compared with wall clock
        // CUMULATIVELY, so a slow patch is worked off by the ticks that follow
        // rather than lost. Ahead of wall clock, the tick sleeps out its
        // budget; behind it, the tick takes only a token sleep and presses on.
        // See `app_scaffold::real_time_pacing` for why falling behind (and
        // saying so) is the right answer rather than skipping simulated time or
        // growing `dt`.
        let mut pacer = RealTimePacer::new(plant_time_per_tick(), PHYSICS_TICK);
        let loop_start = Instant::now();
        spawn_physics_thread_monitored(
            "htgr-physics",
            physics.clone(),
            thread_health.clone(),
            move |state| {
                let tick_start = Instant::now();
                let (rod_insertion, flow, reset_requested, rps_enabled) = state.read_with(|s| {
                    (
                        s.control_rod_insertion_fraction,
                        s.helium_flow_setpoint_kg_per_s,
                        s.trip_reset_requested,
                        s.rps_enabled,
                    )
                });
                // Arming state is owned by the GUI; disarming also clears any
                // latched trip so the operator regains rod control at once.
                if plant.protection.is_enabled() != rps_enabled {
                    plant.protection.set_enabled(rps_enabled);
                }
                // Consume the reset request on the physics thread, which owns
                // the protection system, then clear the flag so one click
                // cannot reset repeatedly.
                if reset_requested {
                    plant.protection.reset();
                    state.update(|s| s.trip_reset_requested = false);
                }
                let flow_rate = MassRate::new::<kilogram_per_second>(flow);
                for _ in 0..SUBSTEPS_PER_TICK {
                    plant.step(dt, rod_insertion, flow_rate);
                }
                // Publish the reactivity the rods actually bought, so the GUI
                // shows a consequence rather than echoing a command back.
                let rho = plant.external_reactivity_dollars(rod_insertion);
                // Advance the pacer's plant clock and decide this tick's sleep.
                // `pace` returns a zero sleep -- never a wrapped or sign-flipped
                // one -- when the work already used the budget up.
                let pacing = pacer.pace(tick_start.elapsed(), loop_start.elapsed());
                state.update(|s| {
                    plant.write_snapshot(s);
                    s.external_reactivity_dollars = rho;
                    s.real_time_ratio = pacer.measured_real_time_ratio();
                    s.real_time_deficit_s = pacer.real_time_deficit().as_secs_f64();
                    s.behind_real_time = pacer.is_behind_real_time();
                });
                thread::sleep(pacing.sleep_for);
            },
        );

        // Plot-sampler thread: reads the physics snapshot, appends to the plot
        // buffers. Handed the physics SharedState; captures the plot one. Also
        // monitored, sharing the same crash flag.
        let plots_for_sampler = plots.clone();
        spawn_physics_thread_monitored(
            "htgr-plot-sampler",
            physics.clone(),
            thread_health.clone(),
            move |state| {
                let snapshot = state.snapshot();
                plots_for_sampler.update(|p| p.push_sample(&snapshot));
                thread::sleep(PLOT_TICK);
            },
        );

        Self {
            physics,
            plots,
            thread_health,
            open_panel: Panel::Schematic,
            tracers: SchematicTracers::new(),
            last_sim_time_s: 0.0,
        }
    }
}

impl eframe::App for HtgrSimApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        // If a physics/plot thread panicked, show the restart modal and stop --
        // do not render the (now frozen) plant behind it.
        if show_crash_modal_if_crashed(ui.ctx(), &self.thread_health) {
            ui.ctx().request_repaint();
            return;
        }

        let snapshot = self.physics.snapshot();

        // Advance the schematic's flow tracers by the PLANT time that elapsed
        // between frames, read from the snapshot's own clock -- not by GUI
        // frame time. Frame time was the wrong clock: it made the tracers run
        // at wall-clock speed while the physics thread ran slower than wall
        // clock, so the marks moved faster than the plant they depict
        // (kopi-beans `op-v5zb`). Taking the delta of the plant clock is exact
        // and self-correcting: if the physics falls behind, so do the tracers.
        let sim_dt = Time::new::<second>((snapshot.sim_time_s - self.last_sim_time_s).max(0.0));
        self.last_sim_time_s = snapshot.sim_time_s;
        self.tracers.advance(sim_dt, &snapshot);

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
                Panel::Schematic => draw_schematic_panel(ui, &snapshot, &self.tracers),
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

#[cfg(test)]
mod tests {
    use super::*;

    /// V&V: the wall-clock budget for a tick equals the plant time that tick
    /// advances, so meeting the deadline *is* real time.
    ///
    /// **Methodology.** Real-time pacing here is not a runtime measurement, it
    /// is an arithmetic identity between three constants:
    /// `PHYSICS_DT_S * SUBSTEPS_PER_TICK == PHYSICS_TICK`. Change any one of
    /// them without the others and the simulator silently runs fast or slow
    /// while the pacer dutifully reports 1.0 -- exactly the kind of drift
    /// kopi-beans `op-fngw` records (the GUI driving the plant at a rate no
    /// test covered). Pin the identity.
    ///
    /// **Results (re-measured 2026-08-13, after the plant timestep became
    /// global and 0.1 s).** `PHYSICS_DT_S = 0.1 s`, `SUBSTEPS_PER_TICK = 1`,
    /// product 0.100000 s; `PHYSICS_TICK = 100 ms = 0.100000 s`. Difference
    /// 0.0 s. Interpretation: a tick that meets its deadline advances the plant
    /// 1:1 with wall clock. (The previous reading of this identity was
    /// `1.0e-3 s x 10 = 10 ms`; the identity held then too, which is the point
    /// -- it constrains the *ratio*, not the values.)
    #[test]
    fn the_physics_tick_is_paced_for_real_time() {
        let plant_seconds = plant_time_per_tick().get::<second>();
        let wall_seconds = PHYSICS_TICK.as_secs_f64();
        assert!(
            (plant_seconds - wall_seconds).abs() < 1e-12,
            "a tick advances {plant_seconds} s of plant time in a {wall_seconds} s budget"
        );
    }

    /// V&V: the timestep the GUI drives is the one the whole-plant regression
    /// test drives -- **now by construction, not by agreement.**
    ///
    /// **Methodology.** `physics::tests::the_whole_plant_steps_at_the_gui_timestep`
    /// exists because a green suite at 0.05 s coexisted with a simulator that
    /// killed its physics thread at 1 ms (kopi-beans `op-fngw`). That test is
    /// only meaningful while it steps at the *same* timestep as this module.
    /// Until 2026-08-13 both sides carried their own literal and a test
    /// compared them. Both now read [`crate::physics::PLANT_TIMESTEP_S`], so
    /// the two cannot disagree; this test is what pins the *identity of the
    /// source*, and it fails if someone reintroduces a local literal here.
    ///
    /// **Results (2026-08-13).** `PHYSICS_DT_S = 0.1 s` and
    /// `crate::physics::PLANT_TIMESTEP_S = 0.1 s`, the same constant.
    /// Interpretation: the rate the application drives the plant at is, by
    /// construction, a rate the whole-plant tests drive.
    #[test]
    fn the_gui_substep_matches_the_tested_substep() {
        assert!((PHYSICS_DT_S - crate::physics::PLANT_TIMESTEP_S).abs() < 1e-12);
        assert!((PHYSICS_DT_S - 0.1).abs() < 1e-12);
    }
}
