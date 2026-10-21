//! The full CIET educational simulator physics loop: three-branch primary loop
//! (heater / CTAH / DHX) coupled to the DRACS natural-circulation loop, forced or
//! natural circulation.
//!
//! This is the physics thread. It is deliberately **`egui`-free**, so the same
//! code runs headless on Android/Termux, where there is no windowing stack.
//!
//! ## Provenance
//!
//! Ported from the CIET Educational Simulator **v1**
//! (`crates/tuas_boussinesq_solver/examples/ciet_educational_simulator/ciet_simulator_v1/app/panels_and_pages/full_simulation/mod.rs`),
//! GPL-3.0, same licence. **The physics is unchanged**: the same TUAS
//! components, the same nodalisation, the same calibrated parasitic heat losses,
//! the same proportional controllers on the CTAH and TCHX air-side heat transfer
//! coefficients, the same 150/160/350 degC killswitch thresholds, the same
//! Courant-limited timestep and the same real-time / fast-forward / slow-motion
//! pacing rules.
//!
//! ## What v2 changed here (and nothing else)
//!
//! 1. **`Arc<Mutex<CIETState>>` became `Arc<RwLock<CietState>>`** (the library's
//!    shared state). The two accesses to it per timestep are a `read()` at the
//!    top and a `write()` at the bottom, exactly as v1 did with `lock()`.
//! 2. **The advanced-heater-control / frequency-response signal is applied
//!    here**, once per timestep, immediately after the state is read and before
//!    the killswitch. In v1 this lived in `eframe::App::ui` and ran once per
//!    repaint, so it could only be driven by a human at the keyboard; moving it
//!    into the physics loop is what lets an OPC-UA client, or the headless
//!    build, run a frequency-response experiment.
//! 3. **The per-timestep temperature `dbg!` dump is off by default.** It is now
//!    controlled by the `verbose_temperatures` argument (`--verbose-temperatures`
//!    on the command line) instead of v1's hardcoded `true`, because in headless
//!    mode it buries the status output.
//!
//! ## Verification status
//!
//! The maintainer has done validation work on **v1's** physics. This is a
//! faithful port of it, but the **port equivalence has not been verified** — no
//! v1-versus-v2 trajectory comparison has been run. Do not describe the numbers
//! this loop produces as validated v2 results.

use std::{
    thread,
    time::{Duration, SystemTime},
};

use outram_park_digital_twin_engine::ciet_opcua::state::SharedCietState;
use outram_park_digital_twin_engine::ciet_opcua::user_controls::SharedUserControls;

use stepper::{max_timestep, CietLoop};

use super::ciet_data::CIETState;
/// controller has been roughly validated using set A1 of the coupled nat circ
/// and a constant forced circulation flow of about 0.18 kg/s.
/// At this flowrate, the heater temperature in is 80C, heater temp out is
/// 110C
///
/// The CTAH cools DowthermA/Therminol VP1 to 80C,
///
///
/// for the parasitic heat loss calibration
/// At 0.18 kg/s and 7.15 kW
/// Heater outlet, CTAH inlet, CTAH Outlet and Heater inlet in degc:
/// 101.674, 100.626, 79.928, 79.004
///
/// Each branch should be such that there is about a 1 K drop from
/// heater to CTAH and 0.9 K drop from CTAH to heater respectively
///
/// # Arguments
///
/// * `global_ciet_state_ptr` — the shared plant state. Read once at the top of
///   each timestep (controls in) and written once at the bottom (measurements
///   out). This is the *only* rendezvous with the GUI and the OPC-UA server.
/// * `verbose_temperatures` — when `true`, `dbg!` the loop temperatures
///   (heater inlet/outlet, DHX shell and tube inlet/outlet) on **every**
///   timestep. v1 hardcoded this on; v2 defaults it off
///   (`--verbose-temperatures` turns it back on) because at 10 timesteps per
///   second it buries the headless status line.
///
/// # Panics
///
/// Panics if the shared-state lock is poisoned, or if any TUAS solver call
/// fails — both indicate a bug rather than a recoverable condition, and v1
/// unwrapped in the same places.
///
/// # Never returns
///
/// Runs until the process exits. Intended to be spawned on its own thread.
pub fn educational_ciet_loop_version_4(
    global_ciet_state_ptr: SharedCietState,
    remote_user_controls: SharedUserControls,
    verbose_temperatures: bool,
) {
    use uom::si::f64::*;
    use uom::si::time::{millisecond, second};

    // obtain local ciet state for reading and writing

    let local_ciet_state_initial: CIETState = global_ciet_state_ptr.read().unwrap().clone();

    // gh:#924: the components, controllers and lagged flows now live in a
    // `CietLoop`, and one timestep is `CietLoop::step` (v1's loop body,
    // moved unchanged). This function keeps v1's control intake and
    // real-time pacing around it.
    let mut ciet_loop = CietLoop::new(&local_ciet_state_initial);

    // timestep settings are user set but only if slow motions settings
    // are on

    let mut timestep = Time::new::<second>(local_ciet_state_initial.get_timestep_seconds() as f64);

    // max timestep is for stability due to courant number and such
    let max_timestep = max_timestep();

    // calculation loop (indefinite)
    //
    // to be done once every timestep
    let loop_time = SystemTime::now();

    loop {
        // so now, let's do the necessary things
        // first, timestep and loop time
        //
        // second, read and update the local_ciet_state

        let loop_time_start = loop_time.elapsed().unwrap();
        // obtain local ciet state for reading and writing

        // v2 addition: drain any control requests that arrived over OPC-UA since
        // the last timestep, applying them to the plant state before we read the
        // controls we are about to integrate with.
        //
        // Doing it HERE, on the physics thread, is the whole point of keeping
        // remote intent in a separate struct. A client's write is queued rather
        // than stored into the plant state, so a GUI repaint that republishes a
        // stale snapshot cannot erase it -- the request is still pending and
        // lands on this line instead. `apply_and_clear` also does the clamping
        // and drops NaN, so the plant state never holds an out-of-envelope value
        // even momentarily.
        //
        // The `has_pending_requests` check keeps the common case (nobody
        // connected) down to one cheap read lock on a small struct instead of a
        // write lock on both.
        {
            let has_requests = remote_user_controls.read().unwrap().has_pending_requests();

            if has_requests {
                let mut requests = remote_user_controls.write().unwrap();
                let mut shared_state = global_ciet_state_ptr.write().unwrap();
                requests.apply_and_clear(&mut shared_state);
            }
        }

        let mut local_ciet_state: CIETState = global_ciet_state_ptr.read().unwrap().clone();

        // v2 addition: apply the advanced-heater-control / frequency-response
        // signal here, once per timestep. v1 did this in `eframe::App::ui`, once
        // per repaint, which meant it only worked with a human at the keyboard
        // and a window open. Evaluating it on the physics thread instead is what
        // lets an OPC-UA client -- or the headless Termux build, which has no GUI
        // at all -- run a frequency-response experiment. The signal itself is
        // unchanged: steady power plus `amplitude * sin(omega * t)`, with `t` the
        // SIMULATION time, so the experiment is reproducible regardless of how
        // fast the machine happens to be.
        //
        // Deliberately before the killswitch below, so an over-temperature trip
        // still wins over the demanded power.
        // set timestep from local ciet state, it is user set
        let slow_motion_settings_turned_on: bool = local_ciet_state.slow_motion_settings_turned_on;

        if slow_motion_settings_turned_on {
            timestep = Time::new::<second>(local_ciet_state.get_timestep_seconds() as f64);
            // as a safety precaution, never let the user set
            // timestep exceed the max timestep
            // for stability issues with Courant number and such
            if timestep > max_timestep {
                timestep = max_timestep;
            }
        }

        // one timestep of the physics (gh:#924, `stepper`)
        ciet_loop.step(&mut local_ciet_state, timestep, verbose_temperatures);
        let current_simulation_time = ciet_loop.simulation_time();

        // i want the calculation thread to sleep for awhile
        // so that the simulation is in sync with real-time
        //
        // I'll give it 1 extra millisecond to do all this calculation

        let simulation_time_seconds = current_simulation_time.get::<second>();
        local_ciet_state.simulation_time_seconds = (simulation_time_seconds * 10.0).round() / 10.0;

        // conditions for thread sleeping
        let fast_forward_button_on: bool = local_ciet_state.is_fast_fwd_on();

        let elapsed_time_seconds =
            (loop_time.elapsed().unwrap().as_secs_f64() * 100.0).round() / 100.0;
        local_ciet_state.elapsed_time_seconds = elapsed_time_seconds;

        let overall_simulation_in_realtime_or_faster: bool =
            simulation_time_seconds > elapsed_time_seconds;

        // also if fast fwd button is on, i want the simulation to be at
        // least a number of times the real speed. here is 1.3 for
        // example

        let fast_fwd_button_speedup_factor: f64 = 1.3;

        let overall_simulation_has_min_fast_fwd_speed =
            simulation_time_seconds > (fast_fwd_button_speedup_factor * elapsed_time_seconds);

        // now update the ciet state
        let loop_time_end = loop_time.elapsed().unwrap();
        let time_taken_for_calculation_loop_milliseconds: f64 =
            (loop_time_end - loop_time_start).as_millis() as f64;
        local_ciet_state.calc_time_ms = time_taken_for_calculation_loop_milliseconds;

        // Remaining budget for this timestep, computed sign-safely and
        // wrap-safely (kopi-beans `op-xvye`; see
        // `outram_park_digital_twin_engine::app_scaffold::real_time_pacing`
        // for the shared version of this arithmetic).
        //
        // What was here before had two defects:
        //   (timestep_ms - calc_ms).round().abs() as u64
        // whose `.abs()` turned an OVERRUN into a positive sleep budget -- a
        // 150 ms step against a 100 ms timestep slept a further 49 ms after
        // already blowing the budget -- and
        //   Duration::from_millis(that - 1)
        // which was evaluated BEFORE the `> 1` guard below and wrapped
        // `0u64 - 1` to `u64::MAX` (a ~584-million-year Duration under the
        // mandatory release profile; a panic in debug).
        //
        // `Duration::saturating_sub` gives zero on an overrun instead, and the
        // 1 ms margin this loop has always allowed itself is now subtracted
        // with `saturating_sub` too, so it cannot underflow either.
        let timestep_budget =
            Duration::from_secs_f64((timestep.get::<millisecond>() * 1.0e-3).max(0.0));
        let calculation_cost = Duration::from_secs_f64(
            (time_taken_for_calculation_loop_milliseconds * 1.0e-3).max(0.0),
        );
        let remaining_budget: Duration = timestep_budget.saturating_sub(calculation_cost);

        let time_to_sleep: Duration = remaining_budget.saturating_sub(Duration::from_millis(1));

        // last condition for sleeping: unchanged in meaning -- only sleep when
        // more than a whole millisecond of the budget is left.
        let real_time_in_current_timestep: bool = remaining_budget > Duration::from_millis(1);

        // check if slow mo settings turned on

        // publish this timestep's results in one write, holding the write lock
        // for as short a time as possible (v1 did the same with `lock()`).
        global_ciet_state_ptr
            .write()
            .unwrap()
            .overwrite_state(local_ciet_state);

        // only sleep if simulation time is greater or equal to elapsed time
        // or if the fast fwd button is off
        // if the current timestep took longer to calculate than real-time
        // then don't sleep either

        // only sleep if real_time_in_current_timestep
        // and global simulation is in real-time and fast fwd button is off
        //
        if real_time_in_current_timestep
            && overall_simulation_in_realtime_or_faster
            && !fast_forward_button_on
        {
            thread::sleep(time_to_sleep);
        } else if real_time_in_current_timestep
            && overall_simulation_has_min_fast_fwd_speed
            && fast_forward_button_on
        {
            // though with fast forward on, it can be very hard
            // to toggle off the fast forward setting due to
            // the shared state issue
            // I won't sleep so much in this case
            // just maybe 40 ms max
            let time_to_sleep: Duration = Duration::from_millis(15);
            thread::sleep(time_to_sleep);
        } else if slow_motion_settings_turned_on {
            // if slow motion settings turned on, just sleep,
            // doesn't matter
            //
            let time_to_sleep: Duration = Duration::from_millis(50);
            thread::sleep(time_to_sleep);
        } else {
            // don't sleep otherwise
            //
            // timestep also needs to be adjusted back to max timestep
            timestep = max_timestep;
        }
    }
}

/// if we want to separate fluid mechanics and heat transfer using
/// different time steps, it is also possible. However, it will need
/// quite some effort
///
/// this is under construction
pub mod separated_solvers;

/// for older versions of the simulator
pub mod archive;

/// One timestep of the loop as a value, shared by this thread and the
/// browser demo (gh:#924).
pub mod stepper;

/// Threads natively, inline on `wasm32` (gh:#924).
pub mod fork;

pub mod fluid_solvers;

pub mod heat_transfer_solvers;

/// displays and prints temperature in command line interface (CLI)
pub mod diagnostics;
