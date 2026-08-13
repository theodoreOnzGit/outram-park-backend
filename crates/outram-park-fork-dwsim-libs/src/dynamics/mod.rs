//! Dynamics — transient (time-marching) simulation of a flowsheet: schedules,
//! integrators, scheduled events, alarm-driven cause-and-effect matrices,
//! monitored-variable recording, a bounded state historian, and the run loop
//! that drives them.
//!
//! # Start here
//!
//! ```text
//!   DynamicsManager                        (manager.rs)   the registry
//!    +- schedule_list:  Schedule           (schedule.rs)  what to run
//!    +- integrator_list: Integrator        (integrator.rs) step size, duration, rates
//!    |    +- monitored_variables: MonitoredVariable  (monitored_variable.rs)
//!    +- event_set_list: EventSet           (event_set.rs)
//!    |    +- events: DynamicEvent          (event.rs)
//!    +- cause_and_effect_matrix_list: CauseAndEffectMatrix (cause_and_effect.rs)
//!    |    +- items: CauseAndEffectItem  <- IndicatorState
//!    +- indicators, stored_states                        (port additions)
//!
//!   DynamicsManager::run_schedule(...)  ->  runner::run_integrator(...)
//!                                           +- Historian    (historian.rs)
//!                                           +- StepContext, PacingRecord, RunReport
//! ```
//!
//! A minimal run, with a synthetic solver standing in for the real one:
//!
//! ```
//! use std::collections::BTreeMap;
//! use uom::si::f64::Time;
//! use uom::si::time::second;
//! use outram_park_fork_dwsim_libs::dynamics::historian::Historian;
//! use outram_park_fork_dwsim_libs::dynamics::integrator::Integrator;
//! use outram_park_fork_dwsim_libs::dynamics::manager::DynamicsManager;
//! use outram_park_fork_dwsim_libs::dynamics::monitored_variable::MonitoredVariable;
//! use outram_park_fork_dwsim_libs::dynamics::property::DynamicProperty;
//! use outram_park_fork_dwsim_libs::dynamics::runner::IntegratorRunOptions;
//! use outram_park_fork_dwsim_libs::dynamics::schedule::Schedule;
//! use outram_park_fork_dwsim_libs::flowsheet::graph::Flowsheet;
//! use outram_park_fork_dwsim_libs::flowsheet::objects::ObjectType;
//!
//! let mut flowsheet = Flowsheet::new();
//! let stream = flowsheet.add_object(ObjectType::MaterialStream, Some("S-1"));
//!
//! let mut integrator = Integrator::new("int-1", "Default")
//!     .with_schedule_times(Time::new::<second>(1.0), Time::new::<second>(10.0));
//! integrator.monitor(MonitoredVariable::new(
//!     "T", stream, DynamicProperty::Temperature, "K",
//! ));
//!
//! let mut manager = DynamicsManager::new();
//! manager.add_integrator(integrator);
//! manager.add_schedule(Schedule::new("sch-1", "Startup", "int-1"));
//! manager.current_schedule = "sch-1".to_string();
//!
//! let mut historian = Historian::new();
//! let report = manager.run_schedule(
//!     &mut flowsheet,
//!     &mut historian,
//!     IntegratorRunOptions::new(),
//!     |_fs, _ctx| Ok(()),   // solve hook: wire the flowsheet solver here
//!     |_fs, _ctx| Ok(()),   // controller hook
//!     |_record| {},         // pacing hook: sleep here for real-time runs
//! ).unwrap();
//!
//! assert_eq!(report.steps_completed, 11);
//! assert!(report.succeeded());
//! ```
//!
//! # Attribution
//!
//! Pure-Rust port of **DWSIM**'s dynamics layer (<https://dwsim.org>), upstream
//! commit `1abf72d1b6b41d3e9a8cc770d3cc4e8fc76e5766` (branch `windows`),
//! GPL-3.0. Upstream copyright: 2020 Daniel Wagner O. de Medeiros and the DWSIM
//! contributors. This port is GPL-3.0-only. Independent OUTRAM PARK fork,
//! **not** the official DWSIM software (see `TRADEMARKS.md`).
//!
//! Sources, by submodule:
//!
//! | Submodule | Upstream source |
//! |---|---|
//! | [`integrator`] | `DWSIM.DynamicsManager/Integrator.vb` (whole file) |
//! | [`manager`] | `DWSIM.DynamicsManager/Manager.vb` (whole file); `DWSIM/Forms/FlowsheetComponents/FormDynamicsIntegratorControl.vb:155-185` (`ProcessEvents`) |
//! | [`schedule`] | `DWSIM.DynamicsManager/Schedule.vb` (whole file) |
//! | [`event`] | `DWSIM.DynamicsManager/Event.vb` (whole file); `DWSIM.Interfaces/Enums.vb:27-51` |
//! | [`event_set`] | `DWSIM.DynamicsManager/EventSet.vb` (whole file) |
//! | [`monitored_variable`] | `DWSIM.DynamicsManager/MonitoredVariable.vb` (whole file); `FormDynamicsIntegratorControl.vb:132-153` (`StoreVariableValues`) |
//! | [`cause_and_effect`] | `DWSIM.DynamicsManager/CauseAndEffectItem.vb`, `CauseAndEffectMatrix.vb` (whole files); `DWSIM.Interfaces/IIndicator.vb`; `DWSIM.UnitOperations/Indicators/AnalogGauge.vb:103-127`; `FormDynamicsIntegratorControl.vb:187-215` |
//! | [`runner`] | `FormDynamicsIntegratorControl.vb:265-653` (`RunIntegrator`), `:245-263` (`RestoreHistorianState`); cross-checked against `DWSIM.UI.Desktop.Editors/Dynamics/DynamicsIntegratorControls.cs:274-560` |
//! | [`historian`] | `FormDynamicsIntegratorControl.vb:398`, `:490-496`, `:638`; `Manager.vb:48-50` |
//! | [`property`] | `DWSIM.SharedClasses/UnitsOfMeasure/SystemsOfUnits.vb:1335-1919` (`ConvertToSI`) and its `ConvertFromSI` twin; the `SimulationObjectProperty` string fields of the dynamics classes |
//! | [`sim_time`] | `Integrator.vb:39`; `FormDynamicsIntegratorControl.vb:135`, `:512-516` |
//! | [`errors`] | the exception sites listed in that module's header |
//!
//! The four DWSIM `DynamicsManager` files not named above have no independent
//! content: they are covered where they are consumed.
//!
//! # Upstream's split, and why the loop lives here
//!
//! DWSIM's `DWSIM.DynamicsManager` assembly is **pure data** — eight classes of
//! properties plus XML serialization, with no stepping code at all. The loop
//! that consumes them lives in the GUI host
//! (`FormDynamicsIntegratorControl.vb`, WinForms, and again in
//! `DynamicsIntegratorControls.cs`, Eto). This port keeps the data model and
//! lifts the **loop semantics** out of the GUI into [`runner`], leaving every
//! form, chart, progress bar and message box behind. [`runner`]'s header lists
//! the exclusions and the VB/C# divergences line by line.
//!
//! # Decoupled from the solver, on purpose
//!
//! Upstream's loop calls `FlowsheetSolver.SolveFlowsheet(Flowsheet, SolverMode)`
//! directly (FormDynamicsIntegratorControl.vb:476-480). This port takes the
//! solve as a **caller-supplied generic hook** instead, so the dynamics layer
//! compiles and is testable without a solver, and so
//! [`crate::flowsheet_solver`] can be wired in without a circular dependency.
//! The controller step and the real-time sleep are hooks for the same reason —
//! and, for the sleep, so a test can exercise the pacing arithmetic without
//! actually waiting.
//!
//! # Real-time mode: best effort, no deadlines
//!
//! DWSIM's "real time" is wall-clock **pacing**, not a real-time guarantee.
//! There is no deadline, no overrun detection and no compensation; the simulated
//! clock advances by a fixed step regardless of how long the step took. The
//! detail, with line citations, is in [`runner`]'s header. Anyone assessing this
//! crate for real-time use should read that section first.
//!
//! # Units
//!
//! Public API is `uom`-typed where a quantity is genuinely typed: times are
//! [`uom::si::f64::Time`], and property reads/writes go through the flowsheet's
//! `uom` accessors. Values that upstream stores as a bare number plus a unit
//! *string* — event targets, alarm setpoints, monitored samples — keep exactly
//! that shape (`property_value: f64` + `property_units: String`), because the
//! string is user data that must survive the round trip. [`property`] documents
//! the conversion at both ends, and notes that DWSIM's "SI" is SI-with-kilo for
//! energy (kJ/kg, kW).
//!
//! # Honest scope
//!
//! This is **AI-assisted draft material and has had no human V&V**. The inline
//! tests are *verification* against the transcribed upstream logic and
//! hand-computed arithmetic — "did we port it correctly?", not "does it
//! represent physical reality?". No DWSIM dynamic case has been run and
//! compared. There is **no controller model** in this crate yet, so the
//! controller hook has nothing to drive it (upstream's loop steps
//! `PIDController` and `PythonController` instances; the workspace crate
//! `chem-eng-real-time-process-control-simulator` is the natural candidate to
//! fill that gap). Per the workspace `RESPONSIBLE_USE.md`, treat all of it as
//! untrusted until reviewed. Not for nuclear facility operation, reactor
//! control, safety-critical decisions, or licensing.

pub mod cause_and_effect;
pub mod errors;
pub mod event;
pub mod event_set;
pub mod historian;
pub mod integrator;
pub mod manager;
pub mod monitored_variable;
pub mod property;
pub mod runner;
pub mod schedule;
pub mod sim_time;

pub use cause_and_effect::{
    CauseAndEffectItem, CauseAndEffectMatrix, DynamicsAlarmType, IndicatorState,
};
pub use errors::{DynamicsError, StepFailure};
pub use event::{
    DynamicEvent, DynamicsEventTransitionReferenceType, DynamicsEventTransitionType,
    DynamicsEventType,
};
pub use event_set::EventSet;
pub use historian::{FlowsheetSnapshot, Historian};
pub use integrator::Integrator;
pub use manager::{DynamicsManager, EventPropertyValue, RandomStream};
pub use monitored_variable::MonitoredVariable;
pub use property::{DynamicProperty, PropertyRef};
pub use runner::{
    run_integrator, IntegratorRunOptions, IntegratorRunSetup, PacingRecord, PacingSummary,
    RunReport, StepContext, StepMode, StepPhase, StopReason,
};
pub use schedule::Schedule;
pub use sim_time::SimInstant;
