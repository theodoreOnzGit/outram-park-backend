//! The dynamics manager — the registry of schedules, integrators, event sets and
//! cause-and-effect matrices, plus the event-ramp evaluation.
//!
//! # Attribution
//!
//! Pure-Rust port of **DWSIM** `DWSIM.DynamicsManager/Manager.vb` (whole file,
//! lines 28-345) and the event processing in
//! `DWSIM/Forms/FlowsheetComponents/FormDynamicsIntegratorControl.vb:155-185`
//! (`ProcessEvents`), upstream commit
//! `1abf72d1b6b41d3e9a8cc770d3cc4e8fc76e5766` (branch `windows`), GPL-3.0.
//! Upstream copyright: 2020 Daniel Wagner O. de Medeiros and the DWSIM
//! contributors. This port is GPL-3.0-only. Independent OUTRAM PARK fork, **not**
//! the official DWSIM software.
//!
//! # The two halves of an event
//!
//! Reading `ProcessEvents` (:155-185) together with `GetPropertyValuesFromEvents`
//! (Manager.vb:211-343) is the only way to see what an event actually does:
//!
//! - **Ramping (before the event).** For every event whose timestamp is still in
//!   the future (`currenttime <= TimeStamp`) and whose transition is *not* a
//!   step change, the manager computes an intermediate value and the loop writes
//!   it. This happens on every step, so a linear transition is a genuine ramp.
//! - **Firing (at the event).** Every event whose timestamp falls in the window
//!   `[t - interval, t)` is applied outright at its stored value.
//!
//! A step-change event therefore only ever fires; a ramping event ramps *and*
//! then fires.
//!
//! # Excluded DWSIM behavior
//!
//! - **`GetChartModel`** (Manager.vb:107-191) — OxyPlot `PlotModel`, axes,
//!   legend placement, font sizes, line series and the display-unit conversion
//!   of the time axis. The data half survives as
//!   [`crate::dynamics::integrator::Integrator::monitored_series`].
//! - **XML serialization** — `SaveData` / `LoadData` (Manager.vb:52-105).
//! - **The delegate fields** `ToggleDynamicMode As Action(Of Boolean)` and
//!   `RunSchedule As Func(Of String, Task)` (Manager.vb:44-46). Upstream stores
//!   these so the GUI can inject the flowsheet's own implementations; a library
//!   does not need the indirection (and the workspace forbids `dyn`), so they
//!   are plain methods here: [`DynamicsManager::toggle_dynamic_mode`] and
//!   [`DynamicsManager::run_schedule`].
//! - **Script events** — [`crate::dynamics::event::DynamicsEventType::RunScript`]
//!   is matched and ignored, exactly as upstream's empty `Case` arm at
//!   FormDynamicsIntegratorControl.vb:81.
//!
//! # Port additions (not in upstream)
//!
//! - [`DynamicsManager::indicators`] — indicator alarm state, which upstream
//!   keeps on flowsheet objects. See [`crate::dynamics::cause_and_effect`].
//! - [`DynamicsManager::stored_states`] — named initial states, which upstream
//!   keeps in `Flowsheet.StoredSolutions`.
//! - [`DynamicsManager::random_seed`] — a seed for
//!   [`crate::dynamics::event::DynamicsEventTransitionType::RandomChange`].
//!   Upstream constructs `New Random()` *inside the loop* (Manager.vb:327),
//!   which is seeded from the system clock and therefore not reproducible; a
//!   seeded generator makes a random ramp repeatable, which a verification test
//!   needs.

use std::collections::BTreeMap;

use uom::si::f64::Time;
use uom::si::time::second;

use crate::dynamics::cause_and_effect::{CauseAndEffectMatrix, IndicatorState};
use crate::dynamics::errors::DynamicsError;
use crate::dynamics::event::{
    DynamicEvent, DynamicsEventTransitionReferenceType, DynamicsEventTransitionType,
    DynamicsEventType,
};
use crate::dynamics::event_set::EventSet;
use crate::dynamics::historian::{FlowsheetSnapshot, Historian};
use crate::dynamics::integrator::Integrator;
use crate::dynamics::property::{property_value, set_property_value, PropertyRef};
use crate::dynamics::runner::{run_integrator, IntegratorRunOptions, IntegratorRunSetup, RunReport};
use crate::dynamics::schedule::Schedule;
use crate::dynamics::sim_time::SimInstant;
use crate::flowsheet::graph::Flowsheet;
use crate::flowsheet::objects::{ObjectData, ObjectId};

/// A property write produced by an event ramp — one element of the
/// `List(Of Tuple(Of String, String, Double))` upstream's
/// `GetPropertyValuesFromEvents` returns (Manager.vb:211-213, :331).
#[derive(Debug, Clone, PartialEq)]
pub struct EventPropertyValue {
    /// Which object and property to write (upstream's `obj.Name` +
    /// `SimulationObjectProperty`).
    pub target: PropertyRef,
    /// The value to write, in DWSIM-internal units.
    pub value: f64,
}

/// A small deterministic pseudo-random generator for
/// [`DynamicsEventTransitionType::RandomChange`].
///
/// **Port addition.** Upstream calls `New Random().NextDouble()`
/// (Manager.vb:327), which reseeds from the system clock on every use and so
/// cannot be reproduced. This is a xorshift64\* generator: cheap, dependency-free
/// (the crate takes no RNG dependency), and seeded, so a random ramp is
/// repeatable within a run. It is **not** cryptographic and makes no claim to
/// statistical quality beyond "uniform enough for a nuisance ramp".
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RandomStream {
    state: u64,
}

impl RandomStream {
    /// Seed the generator. A zero seed is replaced by a non-zero constant, since
    /// xorshift is degenerate at zero.
    #[must_use]
    pub fn new(seed: u64) -> Self {
        RandomStream {
            state: if seed == 0 {
                0x9E37_79B9_7F4A_7C15
            } else {
                seed
            },
        }
    }

    /// The next value in `[0, 1)` — upstream's `NextDouble()`.
    pub fn next_f64(&mut self) -> f64 {
        // xorshift64*
        let mut x = self.state;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.state = x;
        let v = x.wrapping_mul(0x2545_F491_4F6C_DD1D);
        // Top 53 bits → a double in [0, 1).
        ((v >> 11) as f64) / ((1u64 << 53) as f64)
    }
}

impl Default for RandomStream {
    fn default() -> Self {
        RandomStream::new(0x2026_0811_DEAD_BEEF)
    }
}

/// The registry of everything a dynamic simulation needs — upstream's `Manager`
/// (Manager.vb:28-345).
#[derive(Debug, Clone, PartialEq)]
pub struct DynamicsManager {
    /// Human-readable label (Manager.vb:32).
    pub description: String,
    /// Schedules, keyed by [`Schedule::id`] (Manager.vb:34).
    pub schedule_list: BTreeMap<String, Schedule>,
    /// The ID of the schedule a run uses (Manager.vb:36, read at
    /// FormDynamicsIntegratorControl.vb:271-275).
    pub current_schedule: String,
    /// Cause-and-effect matrices, keyed by ID (Manager.vb:38).
    pub cause_and_effect_matrix_list: BTreeMap<String, CauseAndEffectMatrix>,
    /// Event sets, keyed by ID (Manager.vb:40).
    pub event_set_list: BTreeMap<String, EventSet>,
    /// Integrators, keyed by ID (Manager.vb:42).
    pub integrator_list: BTreeMap<String, Integrator>,
    /// Whether to record flowsheet snapshots each step (Manager.vb:48, default
    /// `True`; tested at FormDynamicsIntegratorControl.vb:490).
    pub enable_historian: bool,
    /// The historian's entry bound (Manager.vb:50, default 1000; enforced at
    /// FormDynamicsIntegratorControl.vb:493-495).
    pub max_historian_items: usize,

    /// **Port addition** — indicator alarm state, keyed by the indicator
    /// object's ID. Upstream keeps this on flowsheet objects implementing
    /// `IIndicator`; see [`crate::dynamics::cause_and_effect`].
    pub indicators: BTreeMap<ObjectId, IndicatorState>,
    /// **Port addition** — named initial states, keyed by the name a
    /// [`Schedule::initial_flowsheet_state_id`] refers to. Upstream keeps these
    /// in `Flowsheet.StoredSolutions`
    /// (FormDynamicsIntegratorControl.vb:227-243).
    pub stored_states: BTreeMap<String, FlowsheetSnapshot>,
    /// **Port addition** — the seed for random event ramps. See
    /// [`RandomStream`].
    pub random_seed: u64,
}

impl Default for DynamicsManager {
    /// Upstream's field initialisers verbatim (Manager.vb:32-50): empty
    /// registries, historian on, 1000-entry bound.
    fn default() -> Self {
        DynamicsManager {
            description: String::new(),
            schedule_list: BTreeMap::new(),
            current_schedule: String::new(),
            cause_and_effect_matrix_list: BTreeMap::new(),
            event_set_list: BTreeMap::new(),
            integrator_list: BTreeMap::new(),
            enable_historian: true,
            max_historian_items: 1000,
            indicators: BTreeMap::new(),
            stored_states: BTreeMap::new(),
            random_seed: 0x2026_0811_DEAD_BEEF,
        }
    }
}

impl DynamicsManager {
    /// An empty manager with upstream's defaults.
    #[must_use]
    pub fn new() -> Self {
        DynamicsManager::default()
    }

    /// Register a schedule under its own ID.
    pub fn add_schedule(&mut self, schedule: Schedule) -> Option<Schedule> {
        self.schedule_list.insert(schedule.id.clone(), schedule)
    }

    /// Register an integrator under its own ID.
    pub fn add_integrator(&mut self, integrator: Integrator) -> Option<Integrator> {
        self.integrator_list
            .insert(integrator.id.clone(), integrator)
    }

    /// Register an event set under its own ID.
    pub fn add_event_set(&mut self, event_set: EventSet) -> Option<EventSet> {
        self.event_set_list.insert(event_set.id.clone(), event_set)
    }

    /// Register a cause-and-effect matrix under its own ID.
    pub fn add_cause_and_effect_matrix(
        &mut self,
        matrix: CauseAndEffectMatrix,
    ) -> Option<CauseAndEffectMatrix> {
        self.cause_and_effect_matrix_list
            .insert(matrix.id.clone(), matrix)
    }

    /// Register indicator alarm state under the indicator object's ID
    /// (**port addition**, see the struct field).
    pub fn add_indicator(
        &mut self,
        object: ObjectId,
        indicator: IndicatorState,
    ) -> Option<IndicatorState> {
        self.indicators.insert(object, indicator)
    }

    /// Find a schedule by its **description** — upstream's `GetSchedule`
    /// (Manager.vb:193-195). Note that upstream deliberately matches the
    /// description, not the ID, and returns the first match.
    #[must_use]
    pub fn schedule_by_description(&self, description: &str) -> Option<&Schedule> {
        self.schedule_list
            .values()
            .find(|s| s.description == description)
    }

    /// Find an integrator by its **description** — upstream's `GetIntegrator`
    /// (Manager.vb:197-199).
    #[must_use]
    pub fn integrator_by_description(&self, description: &str) -> Option<&Integrator> {
        self.integrator_list
            .values()
            .find(|s| s.description == description)
    }

    /// Find an event set by its **description** — upstream's `GetEventSet`
    /// (Manager.vb:201-203).
    #[must_use]
    pub fn event_set_by_description(&self, description: &str) -> Option<&EventSet> {
        self.event_set_list
            .values()
            .find(|s| s.description == description)
    }

    /// Find a cause-and-effect matrix by its **description** — upstream's
    /// `GetCauseAndEffectMatrix` (Manager.vb:205-209).
    #[must_use]
    pub fn cause_and_effect_matrix_by_description(
        &self,
        description: &str,
    ) -> Option<&CauseAndEffectMatrix> {
        self.cause_and_effect_matrix_list
            .values()
            .find(|s| s.description == description)
    }

    /// Turn the flowsheet's dynamic mode on or off — upstream's
    /// `ToggleDynamicMode As Action(Of Boolean)` delegate (Manager.vb:44),
    /// which the flowsheet supplies and which ultimately sets
    /// `Flowsheet.DynamicMode`.
    pub fn toggle_dynamic_mode(&self, flowsheet: &mut Flowsheet, enabled: bool) {
        flowsheet.dynamic_mode = enabled;
    }

    /// Compute the ramped property values implied by every pending non-step
    /// event.
    ///
    /// Direct port of `GetPropertyValuesFromEvents` (Manager.vb:211-343). The
    /// shape of the algorithm, per event, in timestamp order:
    ///
    /// 1. Skip it unless it is still pending (`current_time <= timestamp`), is a
    ///    [`DynamicsEventType::ChangeProperty`], and has a non-step transition
    ///    (:223-225).
    /// 2. Resolve a **reference state** from the historian and decide whether the
    ///    ramp is `active` yet, per
    ///    [`DynamicsEventTransitionReferenceType`] (:239-279).
    /// 3. Restore that state onto `scratch` and read the property's value there —
    ///    this is `y0`, the ramp's start (:283-285).
    /// 4. Interpolate between `y0` and the event's target `y1` at the fractional
    ///    position `xt = dt / span` (:287-329), where `span` and `dt` are
    ///    measured in milliseconds either from the reference event or, if there
    ///    is none, from the start of the simulation (:289-299).
    ///
    /// Upstream's zero-guards are kept verbatim: `y0` and `y1` are each replaced
    /// by `1e-30` if they are exactly zero (:305-306), because the log
    /// interpolations take `ln(y)`.
    ///
    /// `scratch` is upstream's `FlowsheetClone` (FormDynamicsIntegratorControl.vb:395,
    /// :166) — a throwaway copy the reference states are restored onto so the
    /// live flowsheet is never disturbed. It is **overwritten** by this call.
    ///
    /// # Divergences
    ///
    /// - Upstream is an instance method that never touches `Me`; this is an
    ///   associated function, so the run loop does not need to hold a borrow of
    ///   the manager.
    /// - Upstream fetches the reference state before testing `active` and
    ///   dereferences `FirstOrDefault()`'s `null` when the historian cannot
    ///   satisfy the lookup; this port fetches it only when the ramp is active
    ///   and returns [`DynamicsError::HistorianEmpty`] /
    ///   [`DynamicsError::NoHistorianEntryBefore`] instead of panicking.
    /// - `Math.Sign` is reproduced explicitly for the random ramp:
    ///   `Math.Sign(0) = 0` in .NET, whereas Rust's `f64::signum(0.0)` is `1.0`.
    ///
    /// # Errors
    ///
    /// [`DynamicsError::ObjectNotFound`] (upstream: `KeyNotFoundException` at
    /// :227), [`DynamicsError::ReferenceEventNotFound`] (upstream's explicit
    /// throw at :269), the two historian errors above, and anything
    /// [`property_value`] reports.
    pub fn property_values_from_events(
        scratch: &mut Flowsheet,
        current_time: SimInstant,
        historian: &Historian,
        event_set: &EventSet,
        random: &mut RandomStream,
    ) -> Result<Vec<EventPropertyValue>, DynamicsError> {
        let events = event_set.events_by_time();
        let mut properties = Vec::new();

        for (i, current) in events.iter().enumerate() {
            // Manager.vb:223
            if !(current_time <= current.timestamp
                && current.event_type == DynamicsEventType::ChangeProperty)
            {
                continue;
            }
            // Manager.vb:225
            if current.transition_type == DynamicsEventTransitionType::StepChange {
                continue;
            }

            // Manager.vb:227 — upstream throws KeyNotFoundException here.
            if !scratch.contains(&current.target.object) {
                return Err(DynamicsError::ObjectNotFound(
                    current.target.object.0.clone(),
                ));
            }

            let value = current.value_in_internal_units(); // :228-231

            // Manager.vb:239-279 — pick the reference state and decide activity.
            let mut active = false;
            let mut reference_event: Option<&DynamicEvent> = None;
            let mut reference_instant: Option<SimInstant> = None;
            let mut use_oldest = false;

            match current.transition_reference {
                DynamicsEventTransitionReferenceType::InitialState => {
                    use_oldest = true;
                    active = true;
                }
                DynamicsEventTransitionReferenceType::PreviousEvent => {
                    if i == 0 {
                        use_oldest = true;
                        active = true;
                    } else {
                        let previous = events[i - 1];
                        reference_event = Some(previous);
                        if previous.timestamp < current_time {
                            active = true;
                        }
                        reference_instant = Some(previous.timestamp);
                    }
                }
                DynamicsEventTransitionReferenceType::SpecificEvent => {
                    let referenced = event_set
                        .events
                        .get(&current.transition_reference_event_id)
                        .ok_or_else(|| {
                            DynamicsError::ReferenceEventNotFound(
                                current.transition_reference_event_id.clone(),
                            )
                        })?;
                    reference_event = Some(referenced);
                    reference_instant = Some(referenced.timestamp);
                    if referenced.timestamp <= current_time {
                        active = true;
                    }
                }
            }

            if !active {
                continue;
            }

            // Manager.vb:283 — restore the reference state onto the scratch clone.
            let snapshot = if use_oldest {
                historian.oldest().ok_or(DynamicsError::HistorianEmpty)?
            } else {
                let at = reference_instant.expect("set on every non-oldest branch");
                historian
                    .newest_at_or_before(at)
                    .ok_or(DynamicsError::NoHistorianEntryBefore(at.ticks()))?
            };
            snapshot.restore_into(scratch);

            // Manager.vb:285
            let value0 = property_value(scratch, &current.target)?;

            // Manager.vb:287-299
            let (span, dt) = match reference_event {
                None => (
                    current.timestamp.millis_since(SimInstant::ZERO),
                    current_time.millis_since(SimInstant::ZERO),
                ),
                Some(reference) => (
                    current.timestamp.millis_since(reference.timestamp),
                    current_time.millis_since(reference.timestamp),
                ),
            };

            let xt = dt / span; // :301
            let mut y0 = value0; // :302
            let mut y1 = value; // :303
            if y0 == 0.0 {
                y0 = 1.0e-30; // :305
            }
            if y1 == 0.0 {
                y1 = 1.0e-30; // :306
            }

            // Manager.vb:310-329
            let yt = match current.transition_type {
                DynamicsEventTransitionType::LinearChange => {
                    linear_interpolate([1.0e-30, 1.0], [y0, y1], xt)
                }
                DynamicsEventTransitionType::LogChange => {
                    log_linear_interpolate([1.0e-30, 1.0], [y0, y1], xt)
                }
                DynamicsEventTransitionType::InverseLogChange => {
                    let yt = log_linear_interpolate([1.0, 1.0e-30], [y0, y1], xt);
                    y1 - (yt - y0)
                }
                DynamicsEventTransitionType::RandomChange => {
                    let d = y1 - y0;
                    let sign = if d > 0.0 {
                        1.0
                    } else if d < 0.0 {
                        -1.0
                    } else {
                        0.0
                    };
                    y0 + sign * random.next_f64() * d.abs()
                }
                // Filtered out at :225; unreachable, but a `match` must be total.
                DynamicsEventTransitionType::StepChange => continue,
            };

            properties.push(EventPropertyValue {
                target: current.target.clone(),
                value: yt,
            });
        }

        Ok(properties)
    }

    /// Apply an event set to the flowsheet for one step: the pending ramps, then
    /// the events whose timestamp falls in this step's window.
    ///
    /// Direct port of `ProcessEvents`
    /// (FormDynamicsIntegratorControl.vb:155-185). Order matters and is
    /// upstream's: **ramps first** (:166-172), then **fired events** (:174-184),
    /// so an event that fires this step overwrites its own ramp value.
    ///
    /// Returns `(ramped writes, fired events)`.
    ///
    /// # Errors
    ///
    /// Anything [`DynamicsManager::property_values_from_events`] or
    /// [`set_property_value`] reports.
    pub fn process_events(
        event_set: &EventSet,
        flowsheet: &mut Flowsheet,
        scratch: &mut Flowsheet,
        historian: &Historian,
        current_position: SimInstant,
        interval: Time,
        random: &mut RandomStream,
    ) -> Result<(usize, usize), DynamicsError> {
        // :160-164 — the half-open window [current - interval, current).
        let window = event_set.events_in_window(current_position, interval.get::<second>());

        // :166-172 — ramps, computed on the clone, written to the live flowsheet.
        let ramped = DynamicsManager::property_values_from_events(
            scratch,
            current_position,
            historian,
            event_set,
            random,
        )?;
        for property in &ramped {
            set_property_value(flowsheet, &property.target, property.value)?;
        }

        // :174-184 — fire the events in this window.
        let mut fired = 0usize;
        for event in window {
            if !event.enabled {
                continue;
            }
            match event.event_type {
                DynamicsEventType::ChangeProperty => {
                    set_property_value(flowsheet, &event.target, event.value_in_internal_units())?;
                    fired += 1;
                }
                // :81 / :181 — upstream's RunScript arm is empty.
                DynamicsEventType::RunScript => {}
            }
        }

        Ok((ramped.len(), fired))
    }

    /// Run [`DynamicsManager::current_schedule`] to completion.
    ///
    /// This is the entry point that corresponds to upstream's
    /// `RunSchedule As Func(Of String, Task)` delegate (Manager.vb:46) being
    /// invoked, and it performs the setup half of `RunIntegrator`
    /// (FormDynamicsIntegratorControl.vb:271-290) before handing the stepping
    /// loop to [`run_integrator`].
    ///
    /// The `solve`, `step_controllers` and `pace` hooks are described on
    /// [`run_integrator`]; supply the flowsheet solver as `solve`.
    ///
    /// The `historian` is owned by the caller because upstream's is a field on
    /// the integrator form that survives pausing and restarting
    /// (FormDynamicsIntegratorControl.vb:397-399, :638).
    ///
    /// # Errors
    ///
    /// [`DynamicsError::ScheduleNotFound`] / [`DynamicsError::IntegratorNotFound`]
    /// / [`DynamicsError::EventSetNotFound`] /
    /// [`DynamicsError::CauseAndEffectMatrixNotFound`] for a mis-wired
    /// configuration, plus anything the loop itself reports.
    ///
    /// A failure *inside* a hook is not an error here — it is recorded on the
    /// returned [`RunReport`]; see [`RunReport::succeeded`].
    pub fn run_schedule<S, C, P>(
        &mut self,
        flowsheet: &mut Flowsheet,
        historian: &mut Historian,
        options: IntegratorRunOptions,
        solve: S,
        step_controllers: C,
        pace: P,
    ) -> Result<RunReport, DynamicsError>
    where
        S: FnMut(
            &mut Flowsheet,
            &crate::dynamics::runner::StepContext,
        ) -> Result<(), crate::dynamics::errors::StepFailure>,
        C: FnMut(
            &mut Flowsheet,
            &crate::dynamics::runner::StepContext,
        ) -> Result<(), crate::dynamics::errors::StepFailure>,
        P: FnMut(&crate::dynamics::runner::PacingRecord),
    {
        // FormDynamicsIntegratorControl.vb:271-275
        let schedule = self
            .schedule_list
            .get(&self.current_schedule)
            .cloned()
            .ok_or_else(|| DynamicsError::ScheduleNotFound(self.current_schedule.clone()))?;

        // :277-279
        let mut integrator = self
            .integrator_list
            .get(&schedule.current_integrator)
            .cloned()
            .ok_or_else(|| {
                DynamicsError::IntegratorNotFound(schedule.current_integrator.clone())
            })?;

        let event_set = if schedule.uses_event_list {
            Some(
                self.event_set_list
                    .get(&schedule.current_event_list)
                    .cloned()
                    .ok_or_else(|| {
                        DynamicsError::EventSetNotFound(schedule.current_event_list.clone())
                    })?,
            )
        } else {
            None
        };

        let cause_and_effect_matrix = if schedule.uses_cause_and_effect_matrix {
            Some(
                self.cause_and_effect_matrix_list
                    .get(&schedule.current_cause_and_effect_matrix)
                    .cloned()
                    .ok_or_else(|| {
                        DynamicsError::CauseAndEffectMatrixNotFound(
                            schedule.current_cause_and_effect_matrix.clone(),
                        )
                    })?,
            )
        } else {
            None
        };

        let initial_state = self
            .stored_states
            .get(&schedule.initial_flowsheet_state_id)
            .cloned();

        let setup = IntegratorRunSetup {
            schedule,
            event_set,
            cause_and_effect_matrix,
            initial_state,
            enable_historian: self.enable_historian,
            max_historian_items: self.max_historian_items,
            options,
            random_seed: self.random_seed,
        };

        // The indicator map is taken out for the duration of the run so the loop
        // needs no borrow of the manager, then put back.
        let mut indicators = std::mem::take(&mut self.indicators);
        let outcome = run_integrator(
            &setup,
            &mut integrator,
            &mut indicators,
            flowsheet,
            historian,
            solve,
            step_controllers,
            pace,
        );
        self.indicators = indicators;
        self.integrator_list
            .insert(integrator.id.clone(), integrator);
        outcome
    }

    /// Ask every unit operation on the flowsheet to reset its held contents.
    ///
    /// Ports `schedule.ResetContentsOfAllObjects`
    /// (FormDynamicsIntegratorControl.vb:361-381), where upstream probes each
    /// object for the dynamic properties `"Reset Content"`, `"Reset Contents"`,
    /// `"Initialize using Inlet Stream"` and `"Initialize using Inlet Streams"`
    /// and sets whichever exist to `1`.
    ///
    /// **Divergence.** This crate's flowsheet has no dynamic-property dictionary,
    /// so the port writes a single [`RESET_CONTENT_FLAG`] entry with value `1.0`
    /// into every unit operation's `results` map and marks the object dirty. A
    /// solver hook that honours held-up inventory should look for that key; one
    /// that does not is unaffected. Returns the objects flagged.
    pub fn reset_contents_of_all_objects(flowsheet: &mut Flowsheet) -> Vec<ObjectId> {
        let ids: Vec<ObjectId> = flowsheet.object_ids().to_vec();
        let mut flagged = Vec::new();
        for id in ids {
            let Some(object) = flowsheet.object_mut(&id) else {
                continue;
            };
            if let ObjectData::UnitOperation { results, .. } = &mut object.data {
                results.insert(RESET_CONTENT_FLAG.to_string(), 1.0);
                object.dirty = true;
                flagged.push(id);
            }
        }
        flagged
    }
}

/// The result key [`DynamicsManager::reset_contents_of_all_objects`] writes —
/// this port's stand-in for upstream's `SetDynamicProperty("Reset Content", 1)`
/// (FormDynamicsIntegratorControl.vb:366-368).
pub const RESET_CONTENT_FLAG: &str = "Reset Content";

/// Two-point linear interpolation, reproducing
/// `MathNet.Numerics.Interpolate.Linear` as upstream calls it through
/// `DWSIM.Math/Interpolation.vb:66-80` (`LinearInterpolation.Interpolate`).
///
/// **The sort matters.** MathNet's `LinearSpline.Interpolate(x, y)` sorts the
/// node arrays by `x` before building the spline, so passing a descending `x`
/// silently swaps the two `y` values. Upstream relies on that when it builds an
/// inverse-log ramp with `x = {1.0, 1e-30}` (Manager.vb:322), so this port sorts
/// too. Values outside `[x0, x1]` are extrapolated along the same line, as
/// MathNet does.
#[must_use]
pub fn linear_interpolate(x: [f64; 2], y: [f64; 2], xt: f64) -> f64 {
    let (x, y) = sorted_by_x(x, y);
    let dx = x[1] - x[0];
    if dx == 0.0 {
        return y[0];
    }
    y[0] + (y[1] - y[0]) * (xt - x[0]) / dx
}

/// Two-point log-linear interpolation, reproducing
/// `MathNet.Numerics.Interpolate.LogLinear` as upstream calls it through
/// `DWSIM.Math/Interpolation.vb:82-96` (`LogLinearInterpolation.Interpolate`):
/// linear in `ln(y)` against `x`, i.e. geometric in `y`.
///
/// Sorting behaves as in [`linear_interpolate`]. **Non-positive `y` values give
/// `NaN`** (`ln` of a non-positive number), which is why upstream replaces exact
/// zeros with `1e-30` before calling it (Manager.vb:305-306) — but note that
/// upstream has **no** guard for *negative* values, so a ramp towards a negative
/// target produces `NaN` in DWSIM too.
#[must_use]
pub fn log_linear_interpolate(x: [f64; 2], y: [f64; 2], xt: f64) -> f64 {
    let (x, y) = sorted_by_x(x, y);
    let dx = x[1] - x[0];
    if dx == 0.0 {
        return y[0];
    }
    let ly0 = y[0].ln();
    let ly1 = y[1].ln();
    (ly0 + (ly1 - ly0) * (xt - x[0]) / dx).exp()
}

fn sorted_by_x(x: [f64; 2], y: [f64; 2]) -> ([f64; 2], [f64; 2]) {
    if x[0] <= x[1] {
        (x, y)
    } else {
        ([x[1], x[0]], [y[1], y[0]])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dynamics::event::DynamicEvent;
    use crate::dynamics::property::DynamicProperty;
    use crate::flowsheet::objects::ObjectType;

    fn flowsheet_with_stream() -> (Flowsheet, ObjectId) {
        let mut fs = Flowsheet::new();
        let id = fs.add_object(ObjectType::MaterialStream, Some("S-1"));
        (fs, id)
    }

    #[test]
    fn registries_round_trip_by_id_and_lookup_by_description() {
        let mut manager = DynamicsManager::new();
        manager.add_integrator(Integrator::new("int-1", "Default integrator"));
        manager.add_schedule(Schedule::new("sch-1", "Startup", "int-1"));
        manager.add_event_set(EventSet::new("es-1", "Startup events"));
        manager.add_cause_and_effect_matrix(CauseAndEffectMatrix::new("cem-1", "Trips"));

        assert_eq!(manager.integrator_list.len(), 1);
        assert_eq!(manager.schedule_list["sch-1"].current_integrator, "int-1");
        // Upstream's Get* helpers match on Description, not ID.
        assert_eq!(
            manager
                .integrator_by_description("Default integrator")
                .unwrap()
                .id,
            "int-1"
        );
        assert_eq!(
            manager.schedule_by_description("Startup").unwrap().id,
            "sch-1"
        );
        assert_eq!(
            manager
                .event_set_by_description("Startup events")
                .unwrap()
                .id,
            "es-1"
        );
        assert_eq!(
            manager
                .cause_and_effect_matrix_by_description("Trips")
                .unwrap()
                .id,
            "cem-1"
        );
        assert!(manager.integrator_by_description("nope").is_none());
        // Defaults from Manager.vb:48-50.
        assert!(manager.enable_historian);
        assert_eq!(manager.max_historian_items, 1000);
    }

    #[test]
    fn toggle_dynamic_mode_sets_the_flowsheet_flag() {
        let manager = DynamicsManager::new();
        let (mut fs, _) = flowsheet_with_stream();
        assert!(!fs.dynamic_mode);
        manager.toggle_dynamic_mode(&mut fs, true);
        assert!(fs.dynamic_mode);
        manager.toggle_dynamic_mode(&mut fs, false);
        assert!(!fs.dynamic_mode);
    }

    #[test]
    fn linear_interpolation_matches_hand_arithmetic_and_sorts_its_nodes() {
        // Ascending nodes: plain lerp between (0,10) and (1,20).
        assert!((linear_interpolate([0.0, 1.0], [10.0, 20.0], 0.25) - 12.5).abs() < 1e-12);
        // Descending nodes get sorted, which swaps the y values -- the behaviour
        // upstream relies on for an inverse-log ramp.
        assert!((linear_interpolate([1.0, 0.0], [10.0, 20.0], 0.25) - 17.5).abs() < 1e-12);
        // Degenerate span returns the first value rather than dividing by zero.
        assert!((linear_interpolate([1.0, 1.0], [10.0, 20.0], 5.0) - 10.0).abs() < 1e-12);
    }

    #[test]
    fn log_linear_interpolation_is_geometric() {
        // ln-linear between (0,1) and (1,100): midpoint is 10.
        assert!((log_linear_interpolate([0.0, 1.0], [1.0, 100.0], 0.5) - 10.0).abs() < 1e-9);
        assert!(log_linear_interpolate([0.0, 1.0], [-1.0, 100.0], 0.5).is_nan());
    }

    #[test]
    fn random_stream_is_seeded_and_stays_in_the_unit_interval() {
        let mut a = RandomStream::new(42);
        let mut b = RandomStream::new(42);
        for _ in 0..64 {
            let v = a.next_f64();
            assert!((0.0..1.0).contains(&v));
            assert!((v - b.next_f64()).abs() < 1e-18, "same seed, same stream");
        }
        assert_ne!(
            RandomStream::new(7).next_f64(),
            RandomStream::new(8).next_f64()
        );
    }

    #[test]
    fn a_linear_ramp_walks_the_property_towards_the_target() {
        let (mut fs, stream) = flowsheet_with_stream();
        let target = PropertyRef::new(stream.clone(), DynamicProperty::Temperature);
        set_property_value(&mut fs, &target, 300.0).unwrap();

        // One historian entry at t=0 holding T = 300 K: the ramp's reference.
        let mut historian = Historian::new();
        historian.insert_bounded(SimInstant::ZERO, FlowsheetSnapshot::capture(&fs), 10);

        let mut event_set = EventSet::new("es-1", "Ramp");
        event_set.insert(
            DynamicEvent::change_property(
                "ev-1",
                SimInstant::from_seconds(100.0),
                stream,
                DynamicProperty::Temperature,
                400.0,
                "K",
            )
            .with_transition(
                DynamicsEventTransitionType::LinearChange,
                DynamicsEventTransitionReferenceType::InitialState,
            ),
        );

        let mut scratch = fs.clone();
        let mut random = RandomStream::default();
        // Half way to the event: the linear ramp should sit half way to 400 K.
        let props = DynamicsManager::property_values_from_events(
            &mut scratch,
            SimInstant::from_seconds(50.0),
            &historian,
            &event_set,
            &mut random,
        )
        .unwrap();
        assert_eq!(props.len(), 1);
        assert!(
            (props[0].value - 350.0).abs() < 1e-6,
            "expected ~350 K, got {}",
            props[0].value
        );

        // Past the event timestamp, the ramp no longer applies (current_time > timestamp).
        let props = DynamicsManager::property_values_from_events(
            &mut scratch,
            SimInstant::from_seconds(150.0),
            &historian,
            &event_set,
            &mut random,
        )
        .unwrap();
        assert!(props.is_empty());
    }

    #[test]
    fn a_step_change_event_produces_no_ramp() {
        let (mut fs, stream) = flowsheet_with_stream();
        let mut historian = Historian::new();
        historian.insert_bounded(SimInstant::ZERO, FlowsheetSnapshot::capture(&fs), 10);
        let mut event_set = EventSet::new("es-1", "Step");
        event_set.insert(DynamicEvent::change_property(
            "ev-1",
            SimInstant::from_seconds(100.0),
            stream,
            DynamicProperty::Temperature,
            400.0,
            "K",
        ));
        let mut scratch = fs.clone();
        let props = DynamicsManager::property_values_from_events(
            &mut scratch,
            SimInstant::from_seconds(50.0),
            &historian,
            &event_set,
            &mut RandomStream::default(),
        )
        .unwrap();
        assert!(props.is_empty(), "Manager.vb:225 skips step changes");
        // The live flowsheet was never touched.
        set_property_value(
            &mut fs,
            &PropertyRef::new(ObjectId::from("S-1"), DynamicProperty::Temperature),
            300.0,
        )
        .ok();
    }

    #[test]
    fn a_missing_reference_event_is_reported() {
        let (fs, stream) = flowsheet_with_stream();
        let mut historian = Historian::new();
        historian.insert_bounded(SimInstant::ZERO, FlowsheetSnapshot::capture(&fs), 10);
        let mut event_set = EventSet::new("es-1", "Broken");
        let mut event = DynamicEvent::change_property(
            "ev-1",
            SimInstant::from_seconds(100.0),
            stream,
            DynamicProperty::Temperature,
            400.0,
            "K",
        )
        .with_transition(
            DynamicsEventTransitionType::LinearChange,
            DynamicsEventTransitionReferenceType::SpecificEvent,
        );
        event.transition_reference_event_id = "does-not-exist".into();
        event_set.insert(event);
        let mut scratch = fs.clone();
        assert!(matches!(
            DynamicsManager::property_values_from_events(
                &mut scratch,
                SimInstant::ZERO,
                &historian,
                &event_set,
                &mut RandomStream::default(),
            ),
            Err(DynamicsError::ReferenceEventNotFound(_))
        ));
    }

    #[test]
    fn process_events_fires_only_inside_the_step_window() {
        let (mut fs, stream) = flowsheet_with_stream();
        let target = PropertyRef::new(stream.clone(), DynamicProperty::Temperature);
        set_property_value(&mut fs, &target, 300.0).unwrap();
        let mut historian = Historian::new();
        historian.insert_bounded(SimInstant::ZERO, FlowsheetSnapshot::capture(&fs), 10);

        let mut event_set = EventSet::new("es-1", "Step");
        event_set.insert(DynamicEvent::change_property(
            "ev-1",
            SimInstant::from_seconds(10.0),
            stream,
            DynamicProperty::Temperature,
            77.0,
            "C",
        ));

        let mut scratch = fs.clone();
        let mut random = RandomStream::default();
        let interval = Time::new::<second>(5.0);

        // Window [0, 5): nothing fires.
        let (ramped, fired) = DynamicsManager::process_events(
            &event_set,
            &mut fs,
            &mut scratch,
            &historian,
            SimInstant::from_seconds(5.0),
            interval,
            &mut random,
        )
        .unwrap();
        assert_eq!((ramped, fired), (0, 0));
        assert!((property_value(&fs, &target).unwrap() - 300.0).abs() < 1e-9);

        // Window [10, 15): the event fires, converting 77 C to 350.15 K.
        let (_, fired) = DynamicsManager::process_events(
            &event_set,
            &mut fs,
            &mut scratch,
            &historian,
            SimInstant::from_seconds(15.0),
            interval,
            &mut random,
        )
        .unwrap();
        assert_eq!(fired, 1);
        assert!((property_value(&fs, &target).unwrap() - 350.15).abs() < 1e-6);
    }

    #[test]
    fn a_disabled_event_never_fires() {
        let (mut fs, stream) = flowsheet_with_stream();
        let target = PropertyRef::new(stream.clone(), DynamicProperty::Temperature);
        set_property_value(&mut fs, &target, 300.0).unwrap();
        let historian = Historian::new();
        let mut event_set = EventSet::new("es-1", "Step");
        let mut event = DynamicEvent::change_property(
            "ev-1",
            SimInstant::from_seconds(10.0),
            stream,
            DynamicProperty::Temperature,
            400.0,
            "K",
        );
        event.enabled = false;
        event_set.insert(event);
        let mut scratch = fs.clone();
        let (_, fired) = DynamicsManager::process_events(
            &event_set,
            &mut fs,
            &mut scratch,
            &historian,
            SimInstant::from_seconds(15.0),
            Time::new::<second>(5.0),
            &mut RandomStream::default(),
        )
        .unwrap();
        assert_eq!(fired, 0);
        assert!((property_value(&fs, &target).unwrap() - 300.0).abs() < 1e-9);
    }

    #[test]
    fn reset_contents_flags_every_unit_operation() {
        let mut fs = Flowsheet::new();
        fs.add_object(ObjectType::MaterialStream, Some("S-1"));
        let pump = fs.add_object(ObjectType::Pump, Some("P-1"));
        let flagged = DynamicsManager::reset_contents_of_all_objects(&mut fs);
        assert_eq!(flagged, vec![pump.clone()]);
        let value = property_value(
            &fs,
            &PropertyRef::new(
                pump,
                DynamicProperty::UnitOperationResult(RESET_CONTENT_FLAG.to_string()),
            ),
        )
        .unwrap();
        assert!((value - 1.0).abs() < 1e-12);
    }
}
