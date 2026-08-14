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

use uom::si::f64::{MassRate, Pressure, ThermodynamicTemperature, Time};
use uom::si::mass_rate::kilogram_per_second;
use uom::si::pressure::kilopascal;
use uom::si::thermodynamic_temperature::kelvin;
use uom::si::time::second;

use outram_park_digital_twin_engine::app_scaffold::{
    panel_selector_ui, show_crash_modal_with_restart, spawn_physics_thread_monitored,
    CrashModalOutcome, RealTimePacer, SharedState, ThreadHealth,
};
use outram_park_digital_twin_engine::components::LegendUnit;

use crate::physics::secondary_loop::{FeedwaterCommand, SecondaryCommands};
use crate::physics::{HtgrPlant, PlantCommands};
use panels::{draw_controls, draw_diagnostics_panel, draw_plots_panel, draw_schematic_panel, Panel};
use schematic::SchematicTracers;
use state::{HtgrPlotData, HtgrSnapshot};

/// Assemble the physics layer's [`PlantCommands`] from the GUI-owned scalar
/// control inputs on the shared [`HtgrSnapshot`].
///
/// **This is the only place the two representations meet**, and the split is
/// deliberate. [`HtgrSnapshot`] is cloned every frame across a thread boundary,
/// so it holds plain `f64`/`bool` scalars; [`PlantCommands`] is what the model
/// consumes, so it is `uom`-typed and its feedwater mode is a real enum rather
/// than a boolean plus two numbers that might disagree. Converting here means a
/// unit error would have to be written in this one function rather than
/// anywhere a control is read.
///
/// Nothing is clamped here on purpose: the physics clamps every field itself
/// (see [`PlantCommands`]), so a value arriving from an OPC-UA write or a test
/// gets exactly the same bounds as one from a slider.
fn plant_commands_from(s: &HtgrSnapshot) -> PlantCommands {
    PlantCommands {
        control_rod_insertion_fraction: s.control_rod_insertion_fraction,
        helium_flow_setpoint: MassRate::new::<kilogram_per_second>(s.helium_flow_setpoint_kg_per_s),
        secondary: SecondaryCommands {
            feedwater: if s.feedwater_manual {
                FeedwaterCommand::Manual {
                    mass_flow_demand: MassRate::new::<kilogram_per_second>(
                        s.feedwater_manual_flow_kg_per_s,
                    ),
                }
            } else {
                FeedwaterCommand::Auto {
                    target_steam_temperature: ThermodynamicTemperature::new::<kelvin>(
                        s.feedwater_target_steam_temp_k,
                    ),
                }
            },
            condenser_pressure: Pressure::new::<kilopascal>(s.condenser_pressure_setpoint_kpa),
        },
    }
}

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
    /// Temperature display unit for **every readout on screen** -- the
    /// operator's degC/K toggle (kopi-beans `op-qpgw`).
    ///
    /// **Lives here, in the GUI struct, and nowhere else.** It is passed by
    /// value into the `draw_*` functions and is never written into
    /// [`HtgrSnapshot`] (the only channel to the physics thread) nor into
    /// [`crate::physics::PlantCommands`] (the only channel into the plant
    /// model), so there is no path by which a display preference could reach a
    /// correlation, a controller or a solver input. See
    /// [`panels::temperature_display`].
    ///
    /// [`LegendUnit`] is the engine's existing display-unit enum, reused rather
    /// than duplicated; its default is kelvin, which is what this simulator
    /// displayed before the toggle existed.
    display_unit: LegendUnit,
    /// Plant clock reading at the previous repaint \[s\].
    ///
    /// The tracers are advanced by the **simulated** time that elapsed between
    /// frames, so a mark travels at the transport speed of the plant it depicts
    /// even when the physics thread is not keeping up with wall clock.
    last_sim_time_s: f64,
}

/// The shared state and crash flag belonging to **one run** of the simulator.
///
/// Bundled rather than returned as a loose tuple because the three handles are
/// only meaningful together: the physics snapshot, the plot buffers and the
/// crash flag are all written by the same pair of threads, and a restart that
/// swapped one without the others would leave the GUI reading a new run's
/// numbers against the old run's crash flag (so the modal would never clear) or
/// vice versa. [`start_simulation`] hands out all three or none.
struct SimulationRun {
    /// Scalar plant state; physics thread writes outputs, GUI writes commands.
    physics: SharedState<HtgrSnapshot>,
    /// Plot ring buffers; the plot-sampler thread writes, the GUI reads.
    plots: SharedState<HtgrPlotData>,
    /// Crash flag shared by both threads of this run.
    thread_health: ThreadHealth,
}

/// Build a fresh [`HtgrPlant`] at its default operating point and spawn this
/// run's physics + plot-sampler threads against brand-new shared state.
///
/// # Why this is a function and not just the body of `HtgrSimApp::new`
///
/// It is called twice: once when the window opens, and again whenever the
/// operator clicks **Restart simulation** in the crash modal
/// ([`HtgrSimApp::restart_simulation`]). Having one function build a run means
/// a restarted plant is *identical* to a freshly launched one -- same plant
/// constructor, same pacing, same thread names -- rather than a second,
/// slightly-different startup path that could drift from this one.
///
/// # It starts a new run; it never resumes one
///
/// Every handle here is new. Nothing from a previous run is read, cloned or
/// carried over -- which is the point: a physics thread that panicked may have
/// done so partway through a write, poisoning the `RwLock` and leaving the
/// snapshot internally inconsistent (a power from after the step against a
/// temperature from before it). Recovering that state is not possible in
/// general, so a restart deliberately discards it, including the plot histories
/// and the simulated clock. The engine's crash modal says so in as many words
/// before the operator clicks.
///
/// Stopping the previous run is [`HtgrSimApp::restart_simulation`]'s job, not
/// this function's -- it retires that run's [`ThreadHealth`] before calling
/// here. The old threads are never *joined*: [`spawn_physics_thread_monitored`]
/// returns from its loop at the next iteration, and the old `Arc`s are freed
/// when the last of them drops its handle, so blocking the GUI thread on a join
/// would buy nothing but a stutter.
fn start_simulation() -> SimulationRun {
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
            let (commands, reset_requested, rps_enabled) = state.read_with(|s| {
                (
                    plant_commands_from(s),
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
            for _ in 0..SUBSTEPS_PER_TICK {
                plant.step(dt, commands);
            }
            // Publish the reactivity the rods actually bought, so the GUI
            // shows a consequence rather than echoing a command back.
            let rho = plant.external_reactivity_dollars(commands.control_rod_insertion_fraction);
            // BUMPLESS TRANSFER. While the feedwater station is in AUTO,
            // keep the manual demand slaved to the flow the controller has
            // actually reached, so flipping to MANUAL picks the plant up
            // where it is instead of stepping the feed flow to whatever the
            // slider was last left at. The reverse direction needs nothing:
            // AUTO recomputes its own demand from the offered duty every
            // step. Same discipline as the CIET v2 simulator's advanced
            // heater control.
            let achieved_feed_flow = plant.secondary.mass_flow().get::<kilogram_per_second>();
            // Advance the pacer's plant clock and decide this tick's sleep.
            // `pace` returns a zero sleep -- never a wrapped or sign-flipped
            // one -- when the work already used the budget up.
            let pacing = pacer.pace(tick_start.elapsed(), loop_start.elapsed());
            state.update(|s| {
                plant.write_snapshot(s);
                s.external_reactivity_dollars = rho;
                if !s.feedwater_manual {
                    s.feedwater_manual_flow_kg_per_s = achieved_feed_flow;
                }
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

    SimulationRun {
        physics,
        plots,
        thread_health,
    }
}

impl HtgrSimApp {
    /// Construct the app and start the first run via [`start_simulation`].
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        Self::start()
    }

    /// The body of [`new`](Self::new), without the `eframe` argument.
    ///
    /// `new` takes a [`eframe::CreationContext`] because that is the signature
    /// `eframe::run_native` expects, and ignores it. Splitting the real
    /// constructor out means the tests below can build a real app -- and
    /// exercise [`restart_simulation`](Self::restart_simulation) against it --
    /// without standing up a window and a graphics context to produce an
    /// argument nothing reads.
    fn start() -> Self {
        let run = start_simulation();
        Self {
            physics: run.physics,
            plots: run.plots,
            thread_health: run.thread_health,
            open_panel: Panel::Schematic,
            display_unit: LegendUnit::default(),
            tracers: SchematicTracers::new(),
            last_sim_time_s: 0.0,
        }
    }

    /// Discard the crashed run and start a new plant from defaults, in-app,
    /// without the operator closing the window.
    ///
    /// Called only from the crash modal's **Restart simulation** button (see
    /// [`show_crash_modal_with_restart`]).
    ///
    /// # Why this is safe after a panic
    ///
    /// Because it does not read the crashed run's state. [`start_simulation`]
    /// builds new `SharedState` handles, a new [`ThreadHealth`] and new threads;
    /// the three old handles are then *overwritten*, so the last GUI-side
    /// reference to the dead run drops without ever being read again. That
    /// matters specifically because the physics thread may have panicked
    /// mid-write: the old snapshot can hold a half-updated plant (a power from
    /// after the step against a temperature from before it), and the old
    /// `RwLock` is poisoned. Nothing here tries to recover a value from either.
    ///
    /// The old run is [`retire`](outram_park_digital_twin_engine::app_scaffold::ThreadHealth::retire)d
    /// first so **both** its threads stop. Retiring is a signal, not a read, so
    /// it is safe on a poisoned run; and it is needed rather than merely tidy,
    /// because a crash stops only the thread that panicked -- its sibling would
    /// otherwise keep stepping and keep burning a core behind the run the
    /// operator is actually watching, once per restart.
    ///
    /// # What resets and what does not
    ///
    /// **Plant state resets** -- power, temperatures, the simulated clock, the
    /// plot histories and the operator's commands all return to
    /// [`HtgrSnapshot::default`] (the published HTR-10 design point). So do the
    /// schematic's flow tracers and [`Self::last_sim_time_s`], because a tracer
    /// phase carried across a restart would animate the new plant from the dead
    /// one's position, and a stale `last_sim_time_s` would hand the first frame
    /// a large negative plant-time delta.
    ///
    /// **Display preferences do not reset** -- the open panel and the degC/K
    /// toggle are the operator's own view settings, not plant state. Resetting
    /// them would silently undo a choice the crash had nothing to do with.
    fn restart_simulation(&mut self) {
        // Stop the old run's threads before replacing the handles. A signal,
        // not a read -- safe against a poisoned lock.
        self.thread_health.retire();

        let run = start_simulation();
        self.physics = run.physics;
        self.plots = run.plots;
        self.thread_health = run.thread_health;
        self.tracers = SchematicTracers::new();
        self.last_sim_time_s = 0.0;
    }
}

impl eframe::App for HtgrSimApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        // If a physics/plot thread panicked, show the crash modal and stop --
        // do not render the (now frozen, possibly half-written) plant behind it.
        //
        // The modal offers an in-app restart. Honour the click here, then still
        // return: the new run has not taken its first step yet, so there is
        // nothing of it to paint this frame either way.
        let crash = show_crash_modal_with_restart(ui.ctx(), &self.thread_health);
        if crash == CrashModalOutcome::RestartRequested {
            self.restart_simulation();
        }
        if crash.is_crashed() {
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
            draw_controls(ui, &self.physics, &snapshot, &mut self.display_unit);
        });

        // Copied out before the closures below borrow `self` -- the unit is a
        // `Copy` display setting, so the panels take it by value.
        let display_unit = self.display_unit;
        egui::CentralPanel::default().show_inside(ui, |ui| {
            egui::ScrollArea::both().show(ui, |ui| match self.open_panel {
                Panel::Schematic => {
                    draw_schematic_panel(ui, &snapshot, &self.tracers, display_unit)
                }
                Panel::Plots => {
                    let plots = self.plots.snapshot();
                    draw_plots_panel(ui, &plots, display_unit);
                }
                Panel::Diagnostics => draw_diagnostics_panel(ui, &snapshot, display_unit),
            });
        });

        // Keep animating while physics runs on its own threads.
        ui.ctx().request_repaint();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// V&V: **restarting starts a new plant; it never resumes the crashed one.**
    ///
    /// # Why this exists
    ///
    /// The whole safety argument for an in-app restart button (kopi-beans
    /// `op-wqk.18`) is that the new run shares *nothing* with the old one. A
    /// physics thread can panic partway through a write, leaving the snapshot
    /// internally inconsistent and its `RwLock` poisoned; `SharedState` is
    /// poison-safe, so reusing that state would not crash — it would quietly
    /// display a half-updated plant as though it were real, which is worse. A
    /// restart that reused *any* of the three handles would do exactly that, and
    /// would do it silently. This pins that all three are replaced, and that the
    /// GUI-side clock and tracer phase are reset with them.
    ///
    /// # Methodology
    ///
    /// Build the app (which starts run 1), let the physics thread advance the
    /// plant clock past zero, and dirty the GUI-side frame state the way a
    /// running simulator would (`last_sim_time_s`) and an operator would
    /// (`open_panel`, `display_unit`). Then call `restart_simulation` and check:
    ///
    /// - the simulated clock went backwards (a resumed run could only go
    ///   forwards) and the plot histories are empty again;
    /// - the *old* run reports itself no longer running, and the *new* one does
    ///   -- i.e. the health handle was swapped, not shared, and retiring the old
    ///   run did not leave the new one dead on arrival;
    /// - the frame-to-frame plant clock is back at zero, so the first frame
    ///   after a restart cannot compute a huge negative tracer step;
    /// - the operator's display preferences survived, because a crash is no
    ///   reason to undo them.
    ///
    /// Both runs are retired at the end, so this test leaves no thread behind to
    /// compete with the timing-sensitive tests in `crate::physics`.
    ///
    /// # Results (2026-08-14)
    ///
    /// Run 1 reached a non-zero `sim_time_s` before the restart; afterwards the
    /// snapshot read `sim_time_s = 0.0` with empty plot buffers,
    /// `last_sim_time_s = 0.0`, the old health `is_running() == false` with no
    /// crash recorded (retired, not faulted), the new health `is_running() ==
    /// true`, and the panel/unit selections unchanged. Interpretation: the
    /// restart is a genuine start-from-defaults, and the crashed run's state is
    /// unreachable from the app afterwards.
    #[test]
    fn restarting_starts_a_fresh_run_and_abandons_the_old_one() {
        let mut app = HtgrSimApp::start();
        let old_health = app.thread_health.clone();

        // Let run 1 actually advance, so "the clock went backwards" is a real
        // observation rather than a comparison of two zeroes.
        let mut advanced_to = 0.0;
        for _ in 0..40 {
            thread::sleep(Duration::from_millis(50));
            advanced_to = app.physics.snapshot().sim_time_s;
            if advanced_to > 0.0 {
                break;
            }
        }
        assert!(
            advanced_to > 0.0,
            "run 1's physics thread never advanced the plant clock, so this test \
             would pass even if restart did nothing"
        );

        // Frame state a running simulator and an operator would have left.
        app.last_sim_time_s = advanced_to;
        app.open_panel = Panel::Diagnostics;
        app.display_unit = LegendUnit::Celsius;

        app.restart_simulation();

        // The plant state is new, not resumed.
        let fresh = app.physics.snapshot();
        assert!(
            fresh.sim_time_s < advanced_to,
            "the restarted plant clock ({} s) must not carry over run 1's ({advanced_to} s)",
            fresh.sim_time_s
        );
        assert!(
            app.plots.snapshot().reactor_power_mw.is_empty(),
            "the restarted run must not inherit run 1's plot history"
        );
        assert_eq!(
            app.last_sim_time_s, 0.0,
            "a stale last_sim_time_s would hand the first frame a large negative \
             plant-time delta"
        );

        // The health flag was swapped, and the old run was told to stop.
        assert!(
            !old_health.is_running(),
            "the abandoned run's threads must be told to stop, or every restart \
             leaks one"
        );
        assert!(
            !old_health.has_crashed(),
            "retiring a run is not a fault -- a crash flag here would raise the \
             modal on the new run"
        );
        assert!(
            app.thread_health.is_running(),
            "the new run must be live; sharing the old handle would freeze it"
        );

        // Display preferences are the operator's, not the plant's.
        assert_eq!(app.open_panel, Panel::Diagnostics);
        assert_eq!(app.display_unit, LegendUnit::Celsius);

        // Leave no threads spinning behind this test.
        app.thread_health.retire();
    }

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
    /// V&V: **the GUI's opening state is the plant's own default command set**,
    /// field by field, through the real conversion the physics thread uses.
    ///
    /// # Why this exists
    ///
    /// [`HtgrSnapshot`] holds the operator's commands as plain scalars, and
    /// [`PlantCommands`] holds them `uom`-typed with the feedwater mode as an
    /// enum. Two representations of the same thing is exactly where a unit error
    /// or a mode inversion hides -- a boolean read the wrong way round would put
    /// the simulator in MANUAL at whatever the slider happened to be, silently,
    /// on the opening frame.
    ///
    /// # Methodology
    ///
    /// The default snapshot is pushed through [`plant_commands_from`] -- the
    /// same function the physics thread calls every tick, not a re-derivation --
    /// and compared with [`PlantCommands::default`]. Then the MANUAL branch is
    /// exercised, because the AUTO comparison alone would pass even if the
    /// boolean were ignored entirely.
    ///
    /// Pass criteria: the AUTO case matches `PlantCommands::default()` exactly
    /// (`PartialEq` on the whole struct, so a new field cannot be forgotten);
    /// setting `feedwater_manual` produces
    /// [`FeedwaterCommand::Manual`] carrying the manual slider's value, in kg/s;
    /// and the condenser and steam-temperature scalars survive their unit
    /// conversions to 1e-9.
    ///
    /// # Results (measured 2026-08-13)
    ///
    /// The default snapshot maps to exactly `PlantCommands::default()`: rods
    /// 0.6035, helium 4.3 kg/s, feedwater AUTO at 713.15 K (440 degC), condenser
    /// 7.000 kPa. Flipping `feedwater_manual` yields
    /// `Manual { mass_flow_demand: 3.47 kg/s }`. Interpretation: the opening
    /// frame commands the published operating point, and the mode boolean is
    /// read in the right direction.
    #[test]
    fn the_gui_defaults_are_the_plant_command_defaults() {
        let snapshot = HtgrSnapshot::default();
        let commands = plant_commands_from(&snapshot);
        assert_eq!(
            commands,
            PlantCommands::default(),
            "the GUI's opening state must be the plant's default command set"
        );

        // The scalars really did survive their unit conversions.
        assert!(
            (commands.helium_flow_setpoint.get::<kilogram_per_second>()
                - snapshot.helium_flow_setpoint_kg_per_s)
                .abs()
                < 1e-9
        );
        match commands.secondary.feedwater {
            FeedwaterCommand::Auto {
                target_steam_temperature,
            } => assert!(
                (target_steam_temperature.get::<kelvin>() - snapshot.feedwater_target_steam_temp_k)
                    .abs()
                    < 1e-9
            ),
            FeedwaterCommand::Manual { .. } => panic!("the GUI must open in AUTO"),
        }
        assert!(
            (commands.secondary.condenser_pressure.get::<kilopascal>()
                - snapshot.condenser_pressure_setpoint_kpa)
                .abs()
                < 1e-9
        );

        // The MANUAL branch, so the boolean is shown to be read at all.
        let manual_snapshot = HtgrSnapshot {
            feedwater_manual: true,
            ..HtgrSnapshot::default()
        };
        match plant_commands_from(&manual_snapshot).secondary.feedwater {
            FeedwaterCommand::Manual { mass_flow_demand } => assert!(
                (mass_flow_demand.get::<kilogram_per_second>()
                    - manual_snapshot.feedwater_manual_flow_kg_per_s)
                    .abs()
                    < 1e-9
            ),
            FeedwaterCommand::Auto { .. } => {
                panic!("feedwater_manual = true must produce a MANUAL command")
            }
        }
    }

    #[test]
    fn the_gui_substep_matches_the_tested_substep() {
        assert!((PHYSICS_DT_S - crate::physics::PLANT_TIMESTEP_S).abs() < 1e-12);
        assert!((PHYSICS_DT_S - 0.1).abs() < 1e-12);
    }
}
