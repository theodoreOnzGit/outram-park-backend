//! The integrator run loop — the stepping engine of a dynamic simulation.
//!
//! # Attribution
//!
//! Pure-Rust port of the run loop in **DWSIM**
//! `DWSIM/Forms/FlowsheetComponents/FormDynamicsIntegratorControl.vb`
//! (`RunIntegrator`, lines 265-653, with `StoreVariableValues` :132-153,
//! `ProcessEvents` :155-185, `ProcessCEMatrix` :187-207, `DoAlarmEffect`
//! :209-215 and `RestoreHistorianState` :245-263), upstream commit
//! `1abf72d1b6b41d3e9a8cc770d3cc4e8fc76e5766` (branch `windows`), GPL-3.0.
//! Upstream copyright: 2020 Daniel Wagner O. de Medeiros and the DWSIM
//! contributors. This port is GPL-3.0-only. Independent OUTRAM PARK fork, **not**
//! the official DWSIM software.
//!
//! The C# twin of the same loop,
//! `DWSIM.UI.Desktop.Editors/Dynamics/DynamicsIntegratorControls.cs:274-560`,
//! was used as a cross-check. Where the two differ the **VB** version is
//! followed; every divergence found is listed under "Two upstreams" below.
//!
//! # The loop, in order
//!
//! Per step (FormDynamicsIntegratorControl.vb:433-573):
//!
//! 1. Start the stopwatch (:435-436).
//! 2. Advance the three subsampling accumulators and set
//!    `ShouldCalculateControl` / `ShouldCalculateEquilibrium` /
//!    `ShouldCalculatePressureFlow` (:451-474).
//! 3. **Solve the whole flowsheet** (:476-480). Any error breaks the loop
//!    (:486).
//! 4. Record a historian snapshot, bounded by `MaxHistorianItems` (:490-496),
//!    and sample the monitored variables (:498). Both are skipped when stepping
//!    manually (`nstep ≠ 0`, :488).
//! 5. Advance the simulated clock by one interval — or retreat by one, for a
//!    backwards single step (:512-516).
//! 6. Step the controllers, but only if `ShouldCalculateControl` (:518-543).
//! 7. Pace: `waittime = RealTimeStepMs - stopwatch.ElapsedMilliseconds`, and
//!    sleep that long if it is positive **and** this is a real-time run
//!    (:545-549).
//! 8. Stop on abort or pause (:553).
//! 9. **Outside real-time only**, process the event list and the
//!    cause-and-effect matrix (:555-562).
//! 10. Advance the progress counter by one interval (:564).
//!
//! The loop condition is `While i <= final` where `i` accumulates `interval`
//! seconds and `final` is the run duration in seconds — or `Integer.MaxValue` in
//! real-time mode, i.e. effectively unbounded (:336-348).
//!
//! # Real-time mode is best-effort, with no deadline enforcement
//!
//! This is worth stating plainly because it decides whether the loop is usable
//! for anything hard-real-time: **it is not**. In real-time mode the loop
//! sleeps away whatever is left of the step budget and, if the step already
//! overran, simply does not sleep (`If waittime > 0 And realtime`, :547). An
//! overrun is not detected, not logged, not compensated on the next step, and
//! does not slow the simulated clock — which keeps advancing by exactly
//! `RealTimeStepMs` of simulated time per step (:302, :515) regardless of how
//! long the step actually took. Simulated and wall-clock time therefore drift
//! apart silently under load. This port keeps that behaviour exactly, but
//! *measures* it: every step produces a [`PacingRecord`] and the run ends with a
//! [`PacingSummary`] counting overruns.
//!
//! # Two upstreams: VB versus C#
//!
//! Differences found against `DynamicsIntegratorControls.cs`, all resolved in
//! favour of the VB file:
//!
//! - **The historian is unbounded in C#.** `:448` adds an entry with no
//!   `EnableHistorian` check, no `ContainsKey` guard and **no
//!   `MaxHistorianItems` eviction** — the VB version has all three
//!   (:490-496). Ported: VB.
//! - **C# has no manual stepping.** It takes no `nstep` parameter, so it has
//!   neither the backwards clock (`:512-513`) nor the historian/monitored-value
//!   suppression (`:488`). Ported: VB.
//! - **C# has no `Paused` flag**, only `Abort` (`:511-513` versus VB's
//!   `If Abort Or Paused`, :553). Ported: VB.
//! - **C# has no initial-state restore** and no `ResetContentsOfAllObjects`
//!   handling. Ported: VB.
//! - C# keeps a separate integer step counter `j` (`:450`, `:524`) where VB
//!   passes the elapsed-seconds accumulator `i` to `StoreVariableValues`; since
//!   that argument is **unused** by `StoreVariableValues` in both, it makes no
//!   difference.
//!
//! # Excluded DWSIM behavior
//!
//! - **All WinForms.** Progress bar setup and updates (:324-350, :442-449,
//!   :566-571, :596-606), the live OxyPlot chart (`SetupChart`/`UpdateChart`,
//!   :568-569), `UpdateHistorianDisplaySize` (:446), `Refresh`,
//!   `Application.DoEvents`, `RunCodeOnUIThread`, `UpdateOpenEditForms`,
//!   `FormDynamics.UpdateControllerList` / `UpdateIndicatorList` (:504-510),
//!   the message-box error reporting (:583-641) and the button-state juggling.
//! - **The `Task` wrapper.** `maintask`/`ContinueWith`/`RunSynchronously`
//!   (:407, :579-651). [`run_integrator`] runs synchronously on the calling
//!   thread; a caller who wants it off-thread can spawn it. `waittofinish` is
//!   therefore only consulted where upstream *also* uses it as a condition, at
//!   :306.
//! - **Scripts.** `ProcessScripts(IntegratorStarted / IntegratorPreStep /
//!   IntegratorStep / IntegratorFinished / IntegratorError / ObjectCalculation*)`
//!   (:404, :438, :502, :521-526, :589-594). The IronPython host is excluded
//!   crate-wide; the caller-supplied hooks are the extension points that replace
//!   them.
//! - **Analytics.** `AnalyticsProvider?.RegisterEvent(...)` (:286, :587, :592).
//! - **`GlobalSettings`.** `Settings.SolverMode`, `Settings.CalculatorBusy` and
//!   its 200 ms spin-wait (:479-484) — that is the caller's solver's business,
//!   behind the `solve` hook.
//! - **Python controllers.** `PythonController` (:296, :356-358, :531-542) — the
//!   `step_controllers` hook covers both controller kinds without naming either.
//! - **`GC.Collect()`** (:639) and `FlowsheetClone.Dispose()` (:636).
//! - **`Flowsheet.ClearLog` / `ShowMessage` / `SupressMessages`** (:283-284,
//!   :392-394).
//!
//! # Known upstream quirks, preserved or flagged
//!
//! - **`guiless` runs take exactly one step.** `Dim final As Integer` (:322) is
//!   only assigned inside `If Not guiless` (:324-350), so a headless run leaves
//!   `final = 0` and `While i <= 0` executes once. This port always uses the
//!   GUI branch's value (duration in seconds, or `i32::MAX` in real time),
//!   because a library has no progress bar to read it from — **a deliberate
//!   divergence**, since reproducing the quirk would make every run one step
//!   long.
//! - **"Step forward" does not step once.** The `nstep = 1` button (:842) enters
//!   the same `While i <= final` loop and runs to the end of the duration; only
//!   `Abort` stops it early. This port reproduces that, and adds
//!   [`IntegratorRunOptions::max_steps`] so a caller can get a genuine single
//!   step.
//! - **A zero integration step hangs upstream.** Guarded here — see
//!   [`crate::dynamics::errors::DynamicsError::NonPositiveInterval`].
//! - **Hook failures do not abort with an error.** Upstream breaks the loop and
//!   rethrows `exceptions(0)` (:486, :575). This port breaks the loop and
//!   records the failure on the [`RunReport`], so the historian, the recorded
//!   series and the step counters remain readable after a failed run. Check
//!   [`RunReport::succeeded`].

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use uom::si::f64::Time;
use uom::si::time::second;

use crate::dynamics::cause_and_effect::{process_ce_matrix, CauseAndEffectMatrix, IndicatorState};
use crate::dynamics::errors::{DynamicsError, StepFailure};
use crate::dynamics::event_set::EventSet;
use crate::dynamics::historian::{FlowsheetSnapshot, Historian};
use crate::dynamics::integrator::Integrator;
use crate::dynamics::manager::{DynamicsManager, RandomStream};
use crate::dynamics::schedule::Schedule;
use crate::flowsheet::graph::Flowsheet;
use crate::flowsheet::objects::ObjectId;

/// Which of upstream's three `nstep` modes a run is in
/// (FormDynamicsIntegratorControl.vb:265, `Optional nstep As Integer = 0`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StepMode {
    /// `nstep = 0` — a normal run. Only this mode records historian snapshots
    /// and monitored-variable samples (:488), resets the clock (:383-386) and
    /// honours the initial-state selection (:304-316).
    #[default]
    Normal,
    /// `nstep = 1` — the GUI's "step forward" button (:842). Suppresses
    /// recording; **does not actually stop after one step** (see the module
    /// header).
    SingleForward,
    /// `nstep = -1` — the GUI's "step backwards" button (:838). Restores the
    /// historian state at `CurrentTime - 2·interval` before starting
    /// (:317-320) and runs the clock **backwards** (:512-513).
    SingleBackward,
}

impl StepMode {
    /// `true` for [`StepMode::Normal`] — upstream's `nstep = 0` test.
    #[must_use]
    pub fn is_normal(self) -> bool {
        matches!(self, StepMode::Normal)
    }
}

/// Which hook the run loop is calling, so one hook signature can serve both the
/// solver and the controllers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepPhase {
    /// The per-step full-flowsheet solve (FormDynamicsIntegratorControl.vb:476-480).
    Solve,
    /// A controller step, gated on `ShouldCalculateControl` (:518-543).
    Controller,
    /// The one-off controller reset before a fresh run (:352-359,
    /// `controller.Reset()`).
    ControllerReset,
}

/// Everything a hook needs to know about the step it is being called for.
///
/// The three `should_calculate_*` flags are the ones upstream writes onto the
/// integrator so the flowsheet solver can read them (:451-474); a solver hook
/// should honour them the same way — skip the flash when
/// `should_calculate_equilibrium` is `false`, skip the pressure-flow network
/// when `should_calculate_pressure_flow` is `false`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StepContext {
    /// Zero-based index of this step within the run.
    pub step_index: u64,
    /// Which hook is being called.
    pub phase: StepPhase,
    /// The simulated clock \[s\]. For [`StepPhase::Solve`] this is the value
    /// *before* the step's advance; for [`StepPhase::Controller`] it is the
    /// value *after*, because upstream steps controllers after
    /// `CurrentTime = CurrentTime.AddSeconds(interval)` (:512-516, :518).
    pub current_time: Time,
    /// Simulated time this step advances by \[s\] — the integration step, or
    /// `real_time_step_ms / 1000` in real-time mode (:300-302).
    pub interval: Time,
    /// Elapsed simulated time since the start of the run \[s\] — upstream's
    /// loop accumulator `i` (:411, :564).
    pub elapsed: Time,
    /// Whether controllers are stepped this step (:455-460).
    pub should_calculate_control: bool,
    /// Whether the solver should run its equilibrium (flash) calculations this
    /// step (:462-467).
    pub should_calculate_equilibrium: bool,
    /// Whether the solver should run its pressure-flow network this step
    /// (:469-474).
    pub should_calculate_pressure_flow: bool,
    /// Whether this is a real-time run (:292).
    pub real_time: bool,
}

/// The wall-clock accounting for one step of a run.
///
/// Computed exactly as upstream computes its sleep
/// (FormDynamicsIntegratorControl.vb:545-549), in **whole milliseconds**,
/// because `Stopwatch.ElapsedMilliseconds` is an integer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PacingRecord {
    /// Which step this describes.
    pub step_index: u64,
    /// The per-step wall-clock budget, `RealTimeStepMs`.
    pub budget: Duration,
    /// How long the step body actually took.
    pub elapsed: Duration,
    /// How long is left of the budget — `max(budget - elapsed, 0)`. Zero when
    /// the step overran.
    pub wait: Duration,
    /// By how much the step exceeded its budget — `max(elapsed - budget, 0)`.
    pub overrun: Duration,
    /// Whether upstream would actually have slept here: only when the run is
    /// real-time **and** `wait` is non-zero (`If waittime > 0 And realtime`).
    pub sleep_requested: bool,
}

impl PacingRecord {
    /// Compute the record for a step that took `elapsed` against a budget of
    /// `budget_ms`.
    ///
    /// Millisecond truncation is deliberate: upstream's
    /// `integrator.RealTimeStepMs - sw.ElapsedMilliseconds` is integer
    /// arithmetic, so a step that overruns by 200 µs registers as an overrun of
    /// 0 ms and still sleeps the full remainder.
    #[must_use]
    pub fn compute(step_index: u64, budget_ms: u32, elapsed: Duration, real_time: bool) -> Self {
        let budget_ms_i = i64::from(budget_ms);
        let elapsed_ms = i64::try_from(elapsed.as_millis()).unwrap_or(i64::MAX);
        let wait_ms = budget_ms_i - elapsed_ms;
        let (wait, overrun) = if wait_ms > 0 {
            (Duration::from_millis(wait_ms as u64), Duration::ZERO)
        } else {
            (
                Duration::ZERO,
                Duration::from_millis(wait_ms.unsigned_abs()),
            )
        };
        PacingRecord {
            step_index,
            budget: Duration::from_millis(u64::from(budget_ms)),
            elapsed,
            wait,
            overrun,
            sleep_requested: real_time && wait_ms > 0,
        }
    }
}

/// Aggregate pacing statistics for a whole run.
///
/// **Port addition.** Upstream measures nothing; this is what makes the
/// best-effort nature of real-time mode visible after the fact.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PacingSummary {
    /// Steps executed.
    pub steps: u64,
    /// Steps where a sleep was requested (real-time, budget not exhausted).
    pub sleeps_requested: u64,
    /// Steps that exceeded their budget by at least 1 ms.
    pub overruns: u64,
    /// Total requested sleep time.
    pub total_wait: Duration,
    /// Total time spent over budget.
    pub total_overrun: Duration,
    /// Worst single overrun.
    pub max_overrun: Duration,
}

impl PacingSummary {
    /// Fold one step's record into the summary.
    pub fn record(&mut self, record: &PacingRecord) {
        self.steps += 1;
        if record.sleep_requested {
            self.sleeps_requested += 1;
            self.total_wait += record.wait;
        }
        if !record.overrun.is_zero() {
            self.overruns += 1;
            self.total_overrun += record.overrun;
            if record.overrun > self.max_overrun {
                self.max_overrun = record.overrun;
            }
        }
    }
}

/// Why the run loop stopped.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StopReason {
    /// The progress accumulator passed the run duration — the normal end
    /// (FormDynamicsIntegratorControl.vb:433). Unreachable in real-time mode,
    /// whose bound is `Integer.MaxValue`.
    DurationReached,
    /// The abort flag was raised (:553, `If Abort ... Exit While`).
    Aborted,
    /// The pause flag was raised (:553, `If ... Paused Then Exit While`).
    Paused,
    /// [`IntegratorRunOptions::max_steps`] was reached. **Port addition.**
    MaxStepsReached,
    /// The `solve` hook reported a failure (:486).
    SolverFailed,
    /// The `step_controllers` hook reported a failure (:525-527).
    ControllerFailed,
}

/// What a run did.
#[derive(Debug, Clone, PartialEq)]
pub struct RunReport {
    /// How many steps executed.
    pub steps_completed: u64,
    /// Why the loop stopped.
    pub stop_reason: StopReason,
    /// The messages a failing hook reported, first one first (upstream rethrows
    /// `exceptions(0)`).
    pub failure_messages: Vec<String>,
    /// The simulated clock when the run ended \[s\].
    pub final_time: Time,
    /// The elapsed simulated time the progress accumulator reached \[s\].
    pub elapsed: Time,
    /// How many steps had `should_calculate_control` set.
    pub control_steps: u64,
    /// How many steps had `should_calculate_equilibrium` set.
    pub equilibrium_steps: u64,
    /// How many steps had `should_calculate_pressure_flow` set.
    pub pressure_flow_steps: u64,
    /// How many historian snapshots were recorded (not how many survived the
    /// [`crate::dynamics::manager::DynamicsManager::max_historian_items`]
    /// bound).
    pub snapshots_recorded: u64,
    /// How many monitored-variable samples were recorded — one per step, each
    /// holding one reading per template.
    pub samples_recorded: u64,
    /// How many event ramp values were written.
    pub event_ramps_applied: u64,
    /// How many events fired.
    pub events_fired: u64,
    /// How many cause-and-effect alarm effects were applied.
    pub alarm_effects_applied: u64,
    /// Wall-clock pacing statistics.
    pub pacing: PacingSummary,
    /// Non-fatal problems, where upstream would have shown a message box and
    /// carried on (e.g. `RestoreHistorianState`'s `Catch`, :257-261).
    pub warnings: Vec<String>,
}

impl RunReport {
    /// `true` if no hook failed — i.e. upstream would not have thrown.
    #[must_use]
    pub fn succeeded(&self) -> bool {
        !matches!(
            self.stop_reason,
            StopReason::SolverFailed | StopReason::ControllerFailed
        )
    }
}

/// How to run: the flags upstream passes to `RunIntegrator`
/// (FormDynamicsIntegratorControl.vb:265) plus two port additions.
#[derive(Debug, Clone)]
pub struct IntegratorRunOptions {
    /// Real-time mode: pace to the wall clock, take the step size from
    /// `RealTimeStepMs`, run unbounded, and **skip events and the
    /// cause-and-effect matrix entirely** (:302, :338, :555-562).
    pub real_time: bool,
    /// Upstream's `waittofinish`. In this port the loop is always synchronous,
    /// so this flag survives only where upstream uses it as a *condition* —
    /// suppressing the initial-state restore (:306).
    pub wait_to_finish: bool,
    /// Upstream's `restarting`: resume after a pause. Suppresses clearing the
    /// recorded series, resetting the clock, resetting the controllers, clearing
    /// the historian and restoring the initial state (:298, :305, :352, :361,
    /// :383, :397).
    pub restarting: bool,
    /// Upstream's `nstep`.
    pub step_mode: StepMode,
    /// Where the progress accumulator starts \[s\]. Upstream reads it off the
    /// progress bar when restarting or stepping manually (:413-431); a library
    /// takes it from the caller.
    pub start_elapsed: Time,
    /// Stop after this many steps. **Port addition** — the honest way to get the
    /// single step upstream's "step forward" button only appears to take.
    pub max_steps: Option<u64>,
    /// Upstream's `Abort` flag (:269, :553). Shared so another thread can stop
    /// the run; cleared at the start of every run, as upstream does at :269.
    pub abort: Arc<AtomicBool>,
    /// Upstream's `Paused` flag (:553). Never written by the loop.
    pub paused: Arc<AtomicBool>,
}

impl Default for IntegratorRunOptions {
    fn default() -> Self {
        IntegratorRunOptions {
            real_time: false,
            wait_to_finish: false,
            restarting: false,
            step_mode: StepMode::Normal,
            start_elapsed: Time::new::<second>(0.0),
            max_steps: None,
            abort: Arc::new(AtomicBool::new(false)),
            paused: Arc::new(AtomicBool::new(false)),
        }
    }
}

impl IntegratorRunOptions {
    /// A normal, non-real-time run from the start.
    #[must_use]
    pub fn new() -> Self {
        IntegratorRunOptions::default()
    }

    /// Turn real-time mode on.
    #[must_use]
    pub fn real_time(mut self) -> Self {
        self.real_time = true;
        self
    }

    /// Stop after `steps` steps (port addition).
    #[must_use]
    pub fn with_max_steps(mut self, steps: u64) -> Self {
        self.max_steps = Some(steps);
        self
    }

    /// Share an abort flag with another thread.
    #[must_use]
    pub fn with_abort_flag(mut self, abort: Arc<AtomicBool>) -> Self {
        self.abort = abort;
        self
    }
}

/// Everything the loop needs that is *not* the integrator, the flowsheet or the
/// historian: the schedule and the objects it names, resolved once at the start
/// of the run.
///
/// [`crate::dynamics::manager::DynamicsManager::run_schedule`] builds this for
/// you; build it by hand only if you are driving [`run_integrator`] directly.
#[derive(Debug, Clone)]
pub struct IntegratorRunSetup {
    /// The schedule being run (FormDynamicsIntegratorControl.vb:275).
    pub schedule: Schedule,
    /// The event set the schedule names, if
    /// [`Schedule::uses_event_list`] is on.
    pub event_set: Option<EventSet>,
    /// The matrix the schedule names, if
    /// [`Schedule::uses_cause_and_effect_matrix`] is on.
    pub cause_and_effect_matrix: Option<CauseAndEffectMatrix>,
    /// The state named by [`Schedule::initial_flowsheet_state_id`], if any
    /// (upstream: `Flowsheet.StoredSolutions`, :231).
    pub initial_state: Option<FlowsheetSnapshot>,
    /// Whether to record historian snapshots (Manager.vb:48).
    pub enable_historian: bool,
    /// The historian bound (Manager.vb:50).
    pub max_historian_items: usize,
    /// How to run.
    pub options: IntegratorRunOptions,
    /// Seed for random event ramps (port addition; see
    /// [`crate::dynamics::manager::RandomStream`]).
    pub random_seed: u64,
}

/// Run the integrator loop to completion.
///
/// This is the port of `RunIntegrator`'s body
/// (FormDynamicsIntegratorControl.vb:265-653) minus the GUI and the `Task`
/// wrapper. It runs **synchronously on the calling thread**.
///
/// # The three hooks
///
/// The loop is deliberately decoupled from the solver, so it can be tested and
/// reused without one. All three are compile-time generics — no `dyn`, per the
/// workspace rules.
///
/// - `solve: FnMut(&mut Flowsheet, &StepContext) -> Result<(), StepFailure>` —
///   the per-step **full flowsheet solve** (:476-480). Called once per step,
///   before anything is recorded. Returning a failure stops the run with
///   [`StopReason::SolverFailed`]. Honour the `should_calculate_*` flags on the
///   [`StepContext`]: they are how upstream's subsampling actually saves work.
/// - `step_controllers: FnMut(&mut Flowsheet, &StepContext) -> Result<(), StepFailure>` —
///   called once before the loop with [`StepPhase::ControllerReset`] (upstream's
///   `controller.Reset()`, :352-359) and then once per step with
///   [`StepPhase::Controller`], **only when `should_calculate_control` is set**
///   (:518). This port has no controller model of its own; see the crate-level
///   note about `chem-eng-real-time-process-control-simulator`.
/// - `pace: FnMut(&PacingRecord)` — called once per step with the wall-clock
///   accounting. **This is where upstream sleeps** (`Task.Delay(waittime).Wait()`,
///   :548). Pass a closure that calls [`std::thread::sleep`] with
///   `record.wait` when `record.sleep_requested` for upstream's behaviour, or
///   one that only records, for a test that must not sleep.
///
/// # Errors
///
/// [`DynamicsError::NonPositiveInterval`] for an unrunnable step size, and
/// anything monitored-variable sampling, event processing or cause-and-effect
/// processing reports (upstream throws in all these cases). A **hook** failure
/// is not an error — see [`RunReport::succeeded`].
// The loop mirrors a single 390-line upstream function and needs its whole
// working set: splitting the parameters into a struct would only rename the
// coupling, and splitting the body would scatter the step order this port
// exists to preserve.
#[allow(clippy::too_many_lines, clippy::too_many_arguments)]
pub fn run_integrator<S, C, P>(
    setup: &IntegratorRunSetup,
    integrator: &mut Integrator,
    indicators: &mut BTreeMap<ObjectId, IndicatorState>,
    flowsheet: &mut Flowsheet,
    historian: &mut Historian,
    mut solve: S,
    mut step_controllers: C,
    mut pace: P,
) -> Result<RunReport, DynamicsError>
where
    S: FnMut(&mut Flowsheet, &StepContext) -> Result<(), StepFailure>,
    C: FnMut(&mut Flowsheet, &StepContext) -> Result<(), StepFailure>,
    P: FnMut(&PacingRecord),
{
    let options = &setup.options;
    let schedule = &setup.schedule;

    // :269 -- Abort = False
    options.abort.store(false, Ordering::SeqCst);

    // :292 -- integrator.RealTime = realtime
    integrator.real_time = options.real_time;

    let normal = options.step_mode.is_normal();
    // Upstream's recurring "Not restarting And nstep = 0" guard.
    let fresh = normal && !options.restarting;

    // :298
    if fresh {
        integrator.monitored_variable_values.clear();
    }

    // :300-302
    let interval = integrator.effective_interval(options.real_time);
    let interval_s = interval.get::<second>();
    if interval_s.is_nan() || interval_s <= 0.0 {
        return Err(DynamicsError::NonPositiveInterval(interval_s));
    }

    let mut warnings: Vec<String> = Vec::new();

    // :304-320 -- pick the starting state.
    if normal {
        if !options.restarting && !options.wait_to_finish && !options.real_time {
            if schedule.use_current_state_as_initial {
                // :311-313 -- "Initializing dynamic schedule from current state."
            } else if let Some(state) = &setup.initial_state {
                state.restore_into(flowsheet);
            } else {
                warnings.push(format!(
                    "initial flowsheet state '{}' is not stored; starting from the current state",
                    schedule.initial_flowsheet_state_id
                ));
            }
        }
    } else if options.step_mode == StepMode::SingleBackward {
        // :317-320 -- RestoreHistorianState(CurrentTime - 2 * interval)
        let reference = integrator.current_time.add_seconds(-2.0 * interval_s);
        if let Some(state) = historian.get_exact(reference) {
            state.restore_into(flowsheet);
        } else {
            // :257-261 -- upstream shows a message box and carries on.
            warnings.push(format!(
                "no historian state at tick {} to step back to",
                reference.ticks()
            ));
        }
    }

    // :322-348 -- the loop bound. See the module header for the `guiless` quirk.
    let final_progress = if options.real_time {
        f64::from(i32::MAX)
    } else {
        integrator.duration.get::<second>().trunc()
    };

    let mut report = RunReport {
        steps_completed: 0,
        stop_reason: StopReason::DurationReached,
        failure_messages: Vec::new(),
        final_time: integrator.current_time.elapsed(),
        elapsed: options.start_elapsed,
        control_steps: 0,
        equilibrium_steps: 0,
        pressure_flow_steps: 0,
        snapshots_recorded: 0,
        samples_recorded: 0,
        event_ramps_applied: 0,
        events_fired: 0,
        alarm_effects_applied: 0,
        pacing: PacingSummary::default(),
        warnings,
    };

    // :352-359 -- reset the controllers on a fresh run.
    if fresh {
        let context = StepContext {
            step_index: 0,
            phase: StepPhase::ControllerReset,
            current_time: integrator.current_time.elapsed(),
            interval,
            elapsed: options.start_elapsed,
            should_calculate_control: false,
            should_calculate_equilibrium: false,
            should_calculate_pressure_flow: false,
            real_time: options.real_time,
        };
        if let Err(failure) = step_controllers(flowsheet, &context) {
            report.stop_reason = StopReason::ControllerFailed;
            report.failure_messages = failure.messages;
            return Ok(report);
        }
    }

    // :361-381
    if schedule.reset_contents_of_all_objects && fresh {
        DynamicsManager::reset_contents_of_all_objects(flowsheet);
    }

    // :383-386
    if fresh {
        integrator.current_time = crate::dynamics::sim_time::SimInstant::ZERO;
        integrator.monitored_variable_values.clear();
    }

    // :388-390 -- the subsampling accumulators start high so that every rate
    // fires on the very first step.
    let mut controllers_check = 100_000.0_f64;
    let mut streams_check = 100_000.0_f64;
    let mut pf_check = 100_000.0_f64;

    // :395 -- FlowsheetClone, the scratch copy event ramps restore states onto.
    let mut scratch = flowsheet.clone();

    // :397-399
    if fresh {
        historian.clear();
    }

    let mut random = RandomStream::new(setup.random_seed);
    let mut progress = options.start_elapsed.get::<second>();
    let mut step_index: u64 = 0;

    // :433 -- While i <= final
    loop {
        if progress > final_progress {
            report.stop_reason = StopReason::DurationReached;
            break;
        }
        if let Some(max) = options.max_steps {
            if step_index >= max {
                report.stop_reason = StopReason::MaxStepsReached;
                break;
            }
        }

        // :435-436
        let started = Instant::now();

        // :451-474 -- subsampling.
        controllers_check += interval_s;
        streams_check += interval_s;
        pf_check += interval_s;

        let rate = |n: u32| f64::from(n) * interval_s;

        if controllers_check >= rate(integrator.calculation_rate_control) {
            controllers_check = 0.0;
            integrator.should_calculate_control = true;
        } else {
            integrator.should_calculate_control = false;
        }
        if streams_check >= rate(integrator.calculation_rate_equilibrium) {
            streams_check = 0.0;
            integrator.should_calculate_equilibrium = true;
        } else {
            integrator.should_calculate_equilibrium = false;
        }
        if pf_check >= rate(integrator.calculation_rate_pressure_flow) {
            pf_check = 0.0;
            integrator.should_calculate_pressure_flow = true;
        } else {
            integrator.should_calculate_pressure_flow = false;
        }

        if integrator.should_calculate_control {
            report.control_steps += 1;
        }
        if integrator.should_calculate_equilibrium {
            report.equilibrium_steps += 1;
        }
        if integrator.should_calculate_pressure_flow {
            report.pressure_flow_steps += 1;
        }

        let mut context = StepContext {
            step_index,
            phase: StepPhase::Solve,
            current_time: integrator.current_time.elapsed(),
            interval,
            elapsed: Time::new::<second>(progress),
            should_calculate_control: integrator.should_calculate_control,
            should_calculate_equilibrium: integrator.should_calculate_equilibrium,
            should_calculate_pressure_flow: integrator.should_calculate_pressure_flow,
            real_time: options.real_time,
        };

        // :476-486 -- solve the whole flowsheet; any failure ends the run.
        if let Err(failure) = solve(flowsheet, &context) {
            report.stop_reason = StopReason::SolverFailed;
            report.failure_messages = failure.messages;
            break;
        }

        // :488-500 -- record, but only on a normal run.
        if normal {
            if setup.enable_historian
                && historian.insert_bounded(
                    integrator.current_time,
                    FlowsheetSnapshot::capture(flowsheet),
                    setup.max_historian_items,
                )
            {
                report.snapshots_recorded += 1;
            }

            // :498 / :132-153 -- StoreVariableValues
            let stamp = integrator.current_time;
            if !integrator.monitored_variable_values.contains_key(&stamp) {
                let mut samples = Vec::with_capacity(integrator.monitored_variables.len());
                for template in &integrator.monitored_variables {
                    samples.push(template.sample(flowsheet, stamp)?);
                }
                integrator.monitored_variable_values.insert(stamp, samples);
                report.samples_recorded += 1;
            }
        }

        // :512-516 -- advance (or retreat) the simulated clock.
        integrator.current_time = if options.step_mode == StepMode::SingleBackward {
            integrator.current_time.add_seconds(-interval_s)
        } else {
            integrator.current_time.add_seconds(interval_s)
        };

        // :518-543 -- step the controllers.
        if integrator.should_calculate_control {
            context.phase = StepPhase::Controller;
            context.current_time = integrator.current_time.elapsed();
            if let Err(failure) = step_controllers(flowsheet, &context) {
                report.stop_reason = StopReason::ControllerFailed;
                report.failure_messages = failure.messages;
                report.steps_completed = step_index + 1;
                report.final_time = integrator.current_time.elapsed();
                report.elapsed = Time::new::<second>(progress);
                return Ok(report);
            }
        }

        // :545-551 -- best-effort pacing. No deadline enforcement: an overrun is
        // simply not slept off, and nothing compensates for it later.
        let record = PacingRecord::compute(
            step_index,
            integrator.real_time_step_ms,
            started.elapsed(),
            options.real_time,
        );
        pace(&record);
        report.pacing.record(&record);

        step_index += 1;
        report.steps_completed = step_index;

        // :553
        if options.abort.load(Ordering::SeqCst) {
            report.stop_reason = StopReason::Aborted;
            break;
        }
        if options.paused.load(Ordering::SeqCst) {
            report.stop_reason = StopReason::Paused;
            break;
        }

        // :555-562 -- events and alarms, NEVER in real-time mode.
        if !options.real_time {
            if schedule.uses_event_list {
                if let Some(event_set) = &setup.event_set {
                    let (ramped, fired) = DynamicsManager::process_events(
                        event_set,
                        flowsheet,
                        &mut scratch,
                        historian,
                        integrator.current_time,
                        integrator.integration_step,
                        &mut random,
                    )?;
                    report.event_ramps_applied += ramped as u64;
                    report.events_fired += fired as u64;
                }
            }
            if schedule.uses_cause_and_effect_matrix {
                if let Some(matrix) = &setup.cause_and_effect_matrix {
                    report.alarm_effects_applied +=
                        process_ce_matrix(matrix, indicators, flowsheet)? as u64;
                }
            }
        }

        // :564
        progress += interval_s;
    }

    report.final_time = integrator.current_time.elapsed();
    report.elapsed = Time::new::<second>(progress);
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dynamics::cause_and_effect::{CauseAndEffectItem, DynamicsAlarmType};
    use crate::dynamics::event::DynamicEvent;
    use crate::dynamics::manager::DynamicsManager;
    use crate::dynamics::monitored_variable::MonitoredVariable;
    use crate::dynamics::property::{property_value, set_property_value, DynamicProperty, PropertyRef};
    use crate::flowsheet::objects::ObjectType;
    use uom::si::time::second;

    /// A flowsheet with one material stream at 300 K, and the reference to its
    /// temperature.
    fn test_flowsheet() -> (Flowsheet, ObjectId, PropertyRef) {
        let mut fs = Flowsheet::new();
        let id = fs.add_object(ObjectType::MaterialStream, Some("S-1"));
        let temperature = PropertyRef::new(id.clone(), DynamicProperty::Temperature);
        set_property_value(&mut fs, &temperature, 300.0).unwrap();
        (fs, id, temperature)
    }

    /// A synthetic first-order process: each step the temperature relaxes
    /// towards a setpoint held in the stream's pressure field. Stands in for the
    /// real flowsheet solver.
    fn first_order_solver(
        temperature: PropertyRef,
        setpoint: f64,
        tau_steps: f64,
    ) -> impl FnMut(&mut Flowsheet, &StepContext) -> Result<(), StepFailure> {
        move |fs: &mut Flowsheet, _ctx: &StepContext| {
            let t =
                property_value(fs, &temperature).map_err(|e| StepFailure::new(e.to_string()))?;
            let next = t + (setpoint - t) / tau_steps;
            set_property_value(fs, &temperature, next)
                .map_err(|e| StepFailure::new(e.to_string()))?;
            Ok(())
        }
    }

    fn no_controllers(_: &mut Flowsheet, _: &StepContext) -> Result<(), StepFailure> {
        Ok(())
    }

    fn setup_for(schedule: Schedule, options: IntegratorRunOptions) -> IntegratorRunSetup {
        IntegratorRunSetup {
            schedule,
            event_set: None,
            cause_and_effect_matrix: None,
            initial_state: None,
            enable_historian: true,
            max_historian_items: 1000,
            options,
            random_seed: 7,
        }
    }

    fn ten_second_integrator() -> Integrator {
        Integrator::new("int-1", "Test")
            .with_schedule_times(Time::new::<second>(1.0), Time::new::<second>(10.0))
    }

    #[test]
    fn a_plain_run_steps_until_the_duration_is_reached() {
        let (mut fs, _, temperature) = test_flowsheet();
        let mut integrator = ten_second_integrator();
        let setup = setup_for(
            Schedule::new("sch-1", "Test", "int-1"),
            IntegratorRunOptions::new(),
        );
        let mut historian = Historian::new();
        let mut indicators = BTreeMap::new();

        let report = run_integrator(
            &setup,
            &mut integrator,
            &mut indicators,
            &mut fs,
            &mut historian,
            first_order_solver(temperature.clone(), 400.0, 5.0),
            no_controllers,
            |_| {},
        )
        .unwrap();

        // i runs 0, 1, ..., 10 inclusive: 11 steps for a 10 s duration at 1 s.
        assert_eq!(report.steps_completed, 11);
        assert_eq!(report.stop_reason, StopReason::DurationReached);
        assert!(report.succeeded());
        assert!((report.final_time.get::<second>() - 11.0).abs() < 1e-6);
        // The synthetic process actually moved.
        let t = property_value(&fs, &temperature).unwrap();
        assert!(
            t > 300.0 && t < 400.0,
            "expected a partial approach, got {t}"
        );
    }

    #[test]
    fn subsampling_flags_follow_the_exact_every_n_pattern() {
        let (mut fs, _, temperature) = test_flowsheet();
        let mut integrator = ten_second_integrator().with_calculation_rates(2, 3, 4);
        let setup = setup_for(
            Schedule::new("sch-1", "Test", "int-1"),
            IntegratorRunOptions::new(),
        );
        let mut historian = Historian::new();
        let mut indicators = BTreeMap::new();

        let mut control = Vec::new();
        let mut equilibrium = Vec::new();
        let mut pressure_flow = Vec::new();
        {
            let mut inner = first_order_solver(temperature, 400.0, 5.0);
            let report = run_integrator(
                &setup,
                &mut integrator,
                &mut indicators,
                &mut fs,
                &mut historian,
                |fs: &mut Flowsheet, ctx: &StepContext| {
                    control.push(ctx.should_calculate_control);
                    equilibrium.push(ctx.should_calculate_equilibrium);
                    pressure_flow.push(ctx.should_calculate_pressure_flow);
                    inner(fs, ctx)
                },
                no_controllers,
                |_| {},
            )
            .unwrap();
            assert_eq!(report.steps_completed, 11);
        }

        // The accumulators start at 100000 (FormDynamicsIntegratorControl.vb:388-390),
        // so step 0 always fires; afterwards it is every N-th step.
        let expected = |n: usize| -> Vec<bool> { (0..11).map(|i| i % n == 0).collect() };
        assert_eq!(control, expected(2), "CalculationRateControl = 2");
        assert_eq!(equilibrium, expected(3), "CalculationRateEquilibrium = 3");
        assert_eq!(
            pressure_flow,
            expected(4),
            "CalculationRatePressureFlow = 4"
        );

        // And the counters agree.
        assert_eq!(integrator.calculation_rate_control, 2);
    }

    #[test]
    fn a_rate_of_one_fires_every_step() {
        let (mut fs, _, temperature) = test_flowsheet();
        let mut integrator = ten_second_integrator();
        let setup = setup_for(
            Schedule::new("sch-1", "Test", "int-1"),
            IntegratorRunOptions::new(),
        );
        let mut historian = Historian::new();
        let mut indicators = BTreeMap::new();
        let report = run_integrator(
            &setup,
            &mut integrator,
            &mut indicators,
            &mut fs,
            &mut historian,
            first_order_solver(temperature, 400.0, 5.0),
            no_controllers,
            |_| {},
        )
        .unwrap();
        assert_eq!(report.control_steps, report.steps_completed);
        assert_eq!(report.equilibrium_steps, report.steps_completed);
        assert_eq!(report.pressure_flow_steps, report.steps_completed);
    }

    #[test]
    fn monitored_variables_are_recorded_once_per_step() {
        let (mut fs, id, temperature) = test_flowsheet();
        let mut integrator = ten_second_integrator();
        integrator.monitor(MonitoredVariable::new(
            "T",
            id,
            DynamicProperty::Temperature,
            "K",
        ));
        let setup = setup_for(
            Schedule::new("sch-1", "Test", "int-1"),
            IntegratorRunOptions::new(),
        );
        let mut historian = Historian::new();
        let mut indicators = BTreeMap::new();

        let report = run_integrator(
            &setup,
            &mut integrator,
            &mut indicators,
            &mut fs,
            &mut historian,
            first_order_solver(temperature, 400.0, 5.0),
            no_controllers,
            |_| {},
        )
        .unwrap();

        assert_eq!(report.samples_recorded, report.steps_completed);
        let series = integrator.monitored_series(0);
        assert_eq!(series.len(), report.steps_completed as usize);
        // Timestamps are the pre-advance clock: 0, 1, 2, ... seconds.
        assert!((series[0].0.get::<second>() - 0.0).abs() < 1e-9);
        assert!((series[1].0.get::<second>() - 1.0).abs() < 1e-9);
        // The sample is taken *after* the step's solve (:476 then :498), so the
        // first recorded value already includes one step of the process:
        // 300 + (400 - 300)/5 = 320 K. The series then climbs monotonically.
        assert!((series[0].1 - 320.0).abs() < 1e-9, "got {}", series[0].1);
        for pair in series.windows(2) {
            assert!(pair[1].1 > pair[0].1, "the first-order process must climb");
        }
    }

    #[test]
    fn the_historian_is_bounded_and_evicts_the_oldest() {
        let (mut fs, _, temperature) = test_flowsheet();
        let mut integrator = ten_second_integrator();
        let mut setup = setup_for(
            Schedule::new("sch-1", "Test", "int-1"),
            IntegratorRunOptions::new(),
        );
        setup.max_historian_items = 4;
        let mut historian = Historian::new();
        let mut indicators = BTreeMap::new();

        let report = run_integrator(
            &setup,
            &mut integrator,
            &mut indicators,
            &mut fs,
            &mut historian,
            first_order_solver(temperature, 400.0, 5.0),
            no_controllers,
            |_| {},
        )
        .unwrap();

        assert_eq!(report.snapshots_recorded, 11, "one attempt per step");
        assert_eq!(historian.len(), 4, "but only MaxHistorianItems survive");
        let instants: Vec<f64> = historian.instants().iter().map(|i| i.seconds()).collect();
        assert_eq!(
            instants,
            vec![7.0, 8.0, 9.0, 10.0],
            "the four newest survive; eviction takes the oldest first"
        );
    }

    #[test]
    fn disabling_the_historian_records_nothing() {
        let (mut fs, _, temperature) = test_flowsheet();
        let mut integrator = ten_second_integrator();
        let mut setup = setup_for(
            Schedule::new("sch-1", "Test", "int-1"),
            IntegratorRunOptions::new(),
        );
        setup.enable_historian = false;
        let mut historian = Historian::new();
        let mut indicators = BTreeMap::new();
        let report = run_integrator(
            &setup,
            &mut integrator,
            &mut indicators,
            &mut fs,
            &mut historian,
            first_order_solver(temperature, 400.0, 5.0),
            no_controllers,
            |_| {},
        )
        .unwrap();
        assert_eq!(report.snapshots_recorded, 0);
        assert!(historian.is_empty());
    }

    #[test]
    fn an_event_fires_at_the_right_simulated_time() {
        let (mut fs, id, temperature) = test_flowsheet();
        let mut integrator = ten_second_integrator();
        integrator.monitor(MonitoredVariable::new(
            "T",
            id.clone(),
            DynamicProperty::Temperature,
            "K",
        ));

        let mut event_set = EventSet::new("es-1", "Step");
        // A step change to 500 K at t = 5 s. It must land in the window
        // [5, 6) -- i.e. be visible from the sample taken at t = 6 s onwards.
        event_set.insert(DynamicEvent::change_property(
            "ev-1",
            crate::dynamics::sim_time::SimInstant::from_seconds(5.0),
            id,
            DynamicProperty::Temperature,
            500.0,
            "K",
        ));

        let mut setup = setup_for(
            Schedule::new("sch-1", "Test", "int-1").with_event_list("es-1"),
            IntegratorRunOptions::new(),
        );
        setup.event_set = Some(event_set);
        let mut historian = Historian::new();
        let mut indicators = BTreeMap::new();

        // A solver that holds the temperature steady, so only the event moves it.
        let report = run_integrator(
            &setup,
            &mut integrator,
            &mut indicators,
            &mut fs,
            &mut historian,
            |_: &mut Flowsheet, _: &StepContext| Ok(()),
            no_controllers,
            |_| {},
        )
        .unwrap();

        assert_eq!(report.events_fired, 1, "a step event fires exactly once");
        assert_eq!(report.event_ramps_applied, 0, "a step change never ramps");

        let series = integrator.monitored_series(0);
        // Samples are taken before the clock advances, so sample k is at t = k s.
        for (instant, value) in &series {
            let t = instant.get::<second>();
            if t <= 5.0 {
                assert!(
                    (value - 300.0).abs() < 1e-9,
                    "unchanged before t=5, at t={t}"
                );
            } else {
                assert!((value - 500.0).abs() < 1e-9, "changed after t=5, at t={t}");
            }
        }
        assert!((property_value(&fs, &temperature).unwrap() - 500.0).abs() < 1e-9);
    }

    #[test]
    fn a_cause_and_effect_alarm_trips_when_the_threshold_is_crossed() {
        let (mut fs, id, temperature) = test_flowsheet();
        let mut integrator = ten_second_integrator();
        let indicator_id = ObjectId::from("IND-1");

        let mut indicators = BTreeMap::new();
        indicators.insert(
            indicator_id.clone(),
            IndicatorState::new(temperature.clone(), "K").with_high_alarms(350.0, 450.0),
        );

        // Effect: when T is high, force the mass flow to zero.
        let flow = PropertyRef::new(id, DynamicProperty::MassFlow);
        let mut matrix = CauseAndEffectMatrix::new("cem-1", "Trips");
        matrix.insert(CauseAndEffectItem::new(
            "trip-1",
            indicator_id,
            DynamicsAlarmType::High,
            flow.clone(),
            0.0,
            "kg/s",
        ));

        let mut setup = setup_for(
            Schedule::new("sch-1", "Test", "int-1").with_cause_and_effect_matrix("cem-1"),
            IntegratorRunOptions::new(),
        );
        setup.cause_and_effect_matrix = Some(matrix);
        let mut historian = Historian::new();

        // Ramp the temperature up by 20 K per step: it crosses 350 K on step 3.
        let report = run_integrator(
            &setup,
            &mut integrator,
            &mut indicators,
            &mut fs,
            &mut historian,
            {
                let temperature = temperature.clone();
                move |fs: &mut Flowsheet, _: &StepContext| {
                    let t = property_value(fs, &temperature).unwrap();
                    set_property_value(fs, &temperature, t + 20.0).unwrap();
                    Ok(())
                }
            },
            no_controllers,
            |_| {},
        )
        .unwrap();

        assert!(
            report.alarm_effects_applied > 0,
            "the high alarm must trip once the ramp crosses 350 K"
        );
        // 11 steps of +20 K from 300 K, the solve running before the matrix is
        // processed: T is 320, 340, 360, ... so it first crosses the 350 K
        // setpoint on step index 2, leaving 9 of the 11 passes with the alarm up.
        assert_eq!(report.alarm_effects_applied, 9);
        assert!(
            (property_value(&fs, &flow).unwrap() - 0.0).abs() < 1e-12,
            "the effect zeroed the flow"
        );
        assert!(indicators[&ObjectId::from("IND-1")].high_alarm_active);
    }

    #[test]
    fn real_time_mode_skips_events_and_alarms_entirely() {
        let (mut fs, id, _) = test_flowsheet();
        let mut integrator = ten_second_integrator();
        integrator.real_time_step_ms = 1; // keep the test fast

        let mut event_set = EventSet::new("es-1", "Step");
        event_set.insert(DynamicEvent::change_property(
            "ev-1",
            crate::dynamics::sim_time::SimInstant::from_seconds(0.002),
            id.clone(),
            DynamicProperty::Temperature,
            500.0,
            "K",
        ));
        let mut setup = setup_for(
            Schedule::new("sch-1", "Test", "int-1").with_event_list("es-1"),
            IntegratorRunOptions::new().real_time().with_max_steps(20),
        );
        setup.event_set = Some(event_set);
        let mut historian = Historian::new();
        let mut indicators = BTreeMap::new();

        let report = run_integrator(
            &setup,
            &mut integrator,
            &mut indicators,
            &mut fs,
            &mut historian,
            |_: &mut Flowsheet, _: &StepContext| Ok(()),
            no_controllers,
            |_| {},
        )
        .unwrap();

        assert_eq!(report.stop_reason, StopReason::MaxStepsReached);
        assert_eq!(
            report.events_fired, 0,
            "FormDynamicsIntegratorControl.vb:555 skips events in real time"
        );
        let t = property_value(&fs, &PropertyRef::new(id, DynamicProperty::Temperature)).unwrap();
        assert!((t - 300.0).abs() < 1e-9);
        assert!(
            integrator.real_time,
            "the mode is recorded on the integrator"
        );
    }

    #[test]
    fn pacing_arithmetic_is_millisecond_truncated_and_never_negative() {
        // Fast step against a 1000 ms budget: the remainder is the wait.
        let fast = PacingRecord::compute(0, 1000, Duration::from_millis(200), true);
        assert_eq!(fast.wait, Duration::from_millis(800));
        assert_eq!(fast.overrun, Duration::ZERO);
        assert!(fast.sleep_requested);

        // Overrun: wait clamps to zero, and nothing compensates for it.
        let slow = PacingRecord::compute(1, 1000, Duration::from_millis(1500), true);
        assert_eq!(slow.wait, Duration::ZERO);
        assert_eq!(slow.overrun, Duration::from_millis(500));
        assert!(!slow.sleep_requested);

        // Exactly on budget: waittime = 0, and `> 0` is false, so no sleep.
        let exact = PacingRecord::compute(2, 1000, Duration::from_millis(1000), true);
        assert_eq!(exact.wait, Duration::ZERO);
        assert!(!exact.sleep_requested);

        // Outside real-time the wait is still computed but never applied.
        let offline = PacingRecord::compute(3, 1000, Duration::from_millis(10), false);
        assert_eq!(offline.wait, Duration::from_millis(990));
        assert!(!offline.sleep_requested);

        // Sub-millisecond elapsed truncates to zero, as Stopwatch.ElapsedMilliseconds does.
        let submilli = PacingRecord::compute(4, 5, Duration::from_micros(900), true);
        assert_eq!(submilli.wait, Duration::from_millis(5));
    }

    #[test]
    fn a_pacing_record_is_emitted_once_per_step_and_nothing_sleeps_in_the_loop() {
        let mut records = Vec::new();
        let (mut fs, _, temperature) = test_flowsheet();
        let mut integrator = ten_second_integrator();
        // A 1 ms budget keeps the test fast; the loop itself never sleeps -- the
        // pacing closure decides, and this one only records.
        integrator.real_time_step_ms = 1;
        let setup = setup_for(
            Schedule::new("sch-1", "Test", "int-1"),
            IntegratorRunOptions::new().real_time().with_max_steps(5),
        );
        let mut historian = Historian::new();
        let mut indicators = BTreeMap::new();

        let started = Instant::now();
        let report = run_integrator(
            &setup,
            &mut integrator,
            &mut indicators,
            &mut fs,
            &mut historian,
            first_order_solver(temperature, 400.0, 5.0),
            no_controllers,
            |record: &PacingRecord| records.push(*record),
        )
        .unwrap();

        assert_eq!(records.len(), 5);
        assert_eq!(report.pacing.steps, 5);
        assert_eq!(records[0].budget, Duration::from_millis(1));
        assert!(
            started.elapsed() < Duration::from_millis(5),
            "the loop must not sleep on its own"
        );
    }

    #[test]
    fn pacing_summary_aggregates_waits_and_overruns() {
        let mut summary = PacingSummary::default();
        summary.record(&PacingRecord::compute(
            0,
            100,
            Duration::from_millis(40),
            true,
        ));
        summary.record(&PacingRecord::compute(
            1,
            100,
            Duration::from_millis(130),
            true,
        ));
        summary.record(&PacingRecord::compute(
            2,
            100,
            Duration::from_millis(350),
            true,
        ));
        summary.record(&PacingRecord::compute(
            3,
            100,
            Duration::from_millis(10),
            false,
        ));

        assert_eq!(summary.steps, 4);
        assert_eq!(summary.sleeps_requested, 1, "only the real-time fast step");
        assert_eq!(summary.total_wait, Duration::from_millis(60));
        assert_eq!(summary.overruns, 2);
        assert_eq!(summary.total_overrun, Duration::from_millis(280));
        assert_eq!(summary.max_overrun, Duration::from_millis(250));
    }

    #[test]
    fn a_zero_integration_step_is_refused_instead_of_hanging() {
        let (mut fs, _, temperature) = test_flowsheet();
        let mut integrator = ten_second_integrator()
            .with_schedule_times(Time::new::<second>(0.0), Time::new::<second>(10.0));
        let setup = setup_for(
            Schedule::new("sch-1", "Test", "int-1"),
            IntegratorRunOptions::new(),
        );
        let mut historian = Historian::new();
        let mut indicators = BTreeMap::new();
        let outcome = run_integrator(
            &setup,
            &mut integrator,
            &mut indicators,
            &mut fs,
            &mut historian,
            first_order_solver(temperature, 400.0, 5.0),
            no_controllers,
            |_| {},
        );
        assert!(matches!(
            outcome,
            Err(DynamicsError::NonPositiveInterval(_))
        ));
    }

    #[test]
    fn aborting_mid_run_stops_the_loop_and_keeps_the_partial_results() {
        let (mut fs, id, temperature) = test_flowsheet();
        let mut integrator = Integrator::new("int-1", "Test")
            .with_schedule_times(Time::new::<second>(1.0), Time::new::<second>(1000.0));
        integrator.monitor(MonitoredVariable::new(
            "T",
            id,
            DynamicProperty::Temperature,
            "K",
        ));
        let abort = Arc::new(AtomicBool::new(false));
        let setup = setup_for(
            Schedule::new("sch-1", "Test", "int-1"),
            IntegratorRunOptions::new().with_abort_flag(Arc::clone(&abort)),
        );
        let mut historian = Historian::new();
        let mut indicators = BTreeMap::new();

        let report = {
            let abort_from_solver = Arc::clone(&abort);
            let mut inner = first_order_solver(temperature, 400.0, 5.0);
            run_integrator(
                &setup,
                &mut integrator,
                &mut indicators,
                &mut fs,
                &mut historian,
                move |fs: &mut Flowsheet, ctx: &StepContext| {
                    if ctx.step_index == 3 {
                        abort_from_solver.store(true, Ordering::SeqCst);
                    }
                    inner(fs, ctx)
                },
                no_controllers,
                |_| {},
            )
            .unwrap()
        };

        assert_eq!(report.stop_reason, StopReason::Aborted);
        assert_eq!(
            report.steps_completed, 4,
            "the aborting step still finishes"
        );
        assert!(report.succeeded(), "an abort is not a failure");
        assert_eq!(integrator.monitored_series(0).len(), 4);
        assert_eq!(historian.len(), 4);
    }

    #[test]
    fn pausing_mid_run_stops_the_loop() {
        let (mut fs, _, temperature) = test_flowsheet();
        let mut integrator = Integrator::new("int-1", "Test")
            .with_schedule_times(Time::new::<second>(1.0), Time::new::<second>(1000.0));
        let paused = Arc::new(AtomicBool::new(false));
        let mut options = IntegratorRunOptions::new();
        options.paused = Arc::clone(&paused);
        let setup = setup_for(Schedule::new("sch-1", "Test", "int-1"), options);
        let mut historian = Historian::new();
        let mut indicators = BTreeMap::new();

        let report = {
            let flag = Arc::clone(&paused);
            let mut inner = first_order_solver(temperature, 400.0, 5.0);
            run_integrator(
                &setup,
                &mut integrator,
                &mut indicators,
                &mut fs,
                &mut historian,
                move |fs: &mut Flowsheet, ctx: &StepContext| {
                    if ctx.step_index == 1 {
                        flag.store(true, Ordering::SeqCst);
                    }
                    inner(fs, ctx)
                },
                no_controllers,
                |_| {},
            )
            .unwrap()
        };
        assert_eq!(report.stop_reason, StopReason::Paused);
        assert_eq!(report.steps_completed, 2);
    }

    #[test]
    fn a_solver_failure_stops_the_run_and_is_reported_not_thrown() {
        let (mut fs, _, _) = test_flowsheet();
        let mut integrator = ten_second_integrator();
        let setup = setup_for(
            Schedule::new("sch-1", "Test", "int-1"),
            IntegratorRunOptions::new(),
        );
        let mut historian = Historian::new();
        let mut indicators = BTreeMap::new();

        let report = run_integrator(
            &setup,
            &mut integrator,
            &mut indicators,
            &mut fs,
            &mut historian,
            |_: &mut Flowsheet, ctx: &StepContext| {
                if ctx.step_index == 2 {
                    Err(StepFailure::from_messages(vec![
                        "flash did not converge".into(),
                        "downstream unit skipped".into(),
                    ]))
                } else {
                    Ok(())
                }
            },
            no_controllers,
            |_| {},
        )
        .unwrap();

        assert_eq!(report.stop_reason, StopReason::SolverFailed);
        assert!(!report.succeeded());
        assert_eq!(report.failure_messages[0], "flash did not converge");
        assert_eq!(report.steps_completed, 2, "the failing step does not count");
        // Everything recorded before the failure survives, unlike upstream's throw.
        assert_eq!(historian.len(), 2);
    }

    #[test]
    fn controllers_are_reset_once_and_then_stepped_on_their_rate() {
        let (mut fs, _, temperature) = test_flowsheet();
        let mut integrator = ten_second_integrator().with_calculation_rates(3, 1, 1);
        let setup = setup_for(
            Schedule::new("sch-1", "Test", "int-1"),
            IntegratorRunOptions::new(),
        );
        let mut historian = Historian::new();
        let mut indicators = BTreeMap::new();

        let mut phases = Vec::new();
        {
            let report = run_integrator(
                &setup,
                &mut integrator,
                &mut indicators,
                &mut fs,
                &mut historian,
                first_order_solver(temperature, 400.0, 5.0),
                |_: &mut Flowsheet, ctx: &StepContext| {
                    phases.push((ctx.phase, ctx.step_index));
                    Ok(())
                },
                |_| {},
            )
            .unwrap();
            assert_eq!(report.steps_completed, 11);
            assert_eq!(report.control_steps, 4, "steps 0, 3, 6, 9");
        }

        assert_eq!(phases[0].0, StepPhase::ControllerReset);
        let stepped: Vec<u64> = phases[1..].iter().map(|(_, i)| *i).collect();
        assert_eq!(stepped, vec![0, 3, 6, 9]);
        assert!(phases[1..]
            .iter()
            .all(|(phase, _)| *phase == StepPhase::Controller));
    }

    #[test]
    fn a_controller_failure_stops_the_run() {
        let (mut fs, _, temperature) = test_flowsheet();
        let mut integrator = ten_second_integrator();
        let setup = setup_for(
            Schedule::new("sch-1", "Test", "int-1"),
            IntegratorRunOptions::new(),
        );
        let mut historian = Historian::new();
        let mut indicators = BTreeMap::new();
        let report = run_integrator(
            &setup,
            &mut integrator,
            &mut indicators,
            &mut fs,
            &mut historian,
            first_order_solver(temperature, 400.0, 5.0),
            |_: &mut Flowsheet, ctx: &StepContext| {
                if ctx.phase == StepPhase::Controller && ctx.step_index == 1 {
                    Err(StepFailure::new("controller diverged"))
                } else {
                    Ok(())
                }
            },
            |_| {},
        )
        .unwrap();
        assert_eq!(report.stop_reason, StopReason::ControllerFailed);
        assert!(!report.succeeded());
        assert_eq!(report.failure_messages, vec!["controller diverged"]);
    }

    #[test]
    fn manual_stepping_records_nothing_and_can_walk_the_clock_backwards() {
        let (mut fs, _, temperature) = test_flowsheet();
        let mut integrator = ten_second_integrator();
        integrator.current_time = crate::dynamics::sim_time::SimInstant::from_seconds(5.0);

        let mut options = IntegratorRunOptions::new().with_max_steps(2);
        options.step_mode = StepMode::SingleBackward;
        let setup = setup_for(Schedule::new("sch-1", "Test", "int-1"), options);
        let mut historian = Historian::new();
        let mut indicators = BTreeMap::new();

        let report = run_integrator(
            &setup,
            &mut integrator,
            &mut indicators,
            &mut fs,
            &mut historian,
            first_order_solver(temperature, 400.0, 5.0),
            no_controllers,
            |_| {},
        )
        .unwrap();

        assert_eq!(
            report.snapshots_recorded, 0,
            "nstep != 0 suppresses recording"
        );
        assert_eq!(report.samples_recorded, 0);
        assert!(
            (report.final_time.get::<second>() - 3.0).abs() < 1e-6,
            "the clock ran backwards from 5 s to 3 s"
        );
        // No historian entry to restore from: reported as a warning, not an error.
        assert_eq!(report.warnings.len(), 1);
        assert!(report.warnings[0].contains("historian"));
    }

    #[test]
    fn run_schedule_wires_the_registries_and_reports_a_bad_configuration() {
        let (mut fs, id, temperature) = test_flowsheet();
        let mut manager = DynamicsManager::new();
        let mut historian = Historian::new();

        // No current schedule yet.
        let outcome = manager.run_schedule(
            &mut fs,
            &mut historian,
            IntegratorRunOptions::new(),
            first_order_solver(temperature.clone(), 400.0, 5.0),
            no_controllers,
            |_| {},
        );
        assert!(matches!(outcome, Err(DynamicsError::ScheduleNotFound(_))));

        // A schedule pointing at a missing integrator.
        manager.add_schedule(Schedule::new("sch-1", "Test", "int-1"));
        manager.current_schedule = "sch-1".into();
        let outcome = manager.run_schedule(
            &mut fs,
            &mut historian,
            IntegratorRunOptions::new(),
            first_order_solver(temperature.clone(), 400.0, 5.0),
            no_controllers,
            |_| {},
        );
        assert!(matches!(outcome, Err(DynamicsError::IntegratorNotFound(_))));

        // Now wire it up properly and run.
        let mut integrator = ten_second_integrator();
        integrator.monitor(MonitoredVariable::new(
            "T",
            id,
            DynamicProperty::Temperature,
            "K",
        ));
        manager.add_integrator(integrator);
        let report = manager
            .run_schedule(
                &mut fs,
                &mut historian,
                IntegratorRunOptions::new(),
                first_order_solver(temperature, 400.0, 5.0),
                no_controllers,
                |_| {},
            )
            .unwrap();
        assert_eq!(report.steps_completed, 11);
        // The recorded series was written back onto the manager's integrator.
        assert_eq!(
            manager.integrator_list["int-1"].monitored_series(0).len(),
            11
        );
    }

    #[test]
    fn run_schedule_reports_a_schedule_naming_a_missing_event_set() {
        let (mut fs, _, temperature) = test_flowsheet();
        let mut manager = DynamicsManager::new();
        manager.add_integrator(ten_second_integrator());
        manager.add_schedule(Schedule::new("sch-1", "Test", "int-1").with_event_list("nope"));
        manager.current_schedule = "sch-1".into();
        let mut historian = Historian::new();
        let outcome = manager.run_schedule(
            &mut fs,
            &mut historian,
            IntegratorRunOptions::new(),
            first_order_solver(temperature, 400.0, 5.0),
            no_controllers,
            |_| {},
        );
        assert!(matches!(outcome, Err(DynamicsError::EventSetNotFound(_))));
    }
}
