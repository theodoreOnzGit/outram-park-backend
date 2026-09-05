//! Error types for the dynamics layer.
//!
//! # Attribution
//!
//! Pure-Rust port of part of **DWSIM** (<https://dwsim.org>), upstream commit
//! `1abf72d1b6b41d3e9a8cc770d3cc4e8fc76e5766` (branch `windows`), GPL-3.0.
//! Upstream copyright: 2020 Daniel Wagner O. de Medeiros and the DWSIM
//! contributors. This port is GPL-3.0-only. Independent OUTRAM PARK fork, **not**
//! the official DWSIM software.
//!
//! # What this replaces upstream
//!
//! DWSIM's dynamics layer signals every failure by throwing a .NET exception,
//! usually from deep inside a `Task` that the GUI later flattens and shows in a
//! message box:
//!
//! - `FormDynamicsIntegratorControl.vb:272` / `:278` — "Please select a valid
//!   schedule." / "Please select a valid integrator for the selected schedule."
//!   → [`DynamicsError::ScheduleNotFound`] / [`DynamicsError::IntegratorNotFound`].
//! - `FormDynamicsIntegratorControl.vb:141-143` — "At least one of the monitored
//!   variables is not configured correctly" → [`DynamicsError::ObjectNotFound`].
//! - `DWSIM.DynamicsManager/Manager.vb:269` — "could not find reference event for
//!   transition in event '{0}'" → [`DynamicsError::ReferenceEventNotFound`].
//! - Unchecked `Dictionary` indexing (`Manager.vb:227`, `:273`,
//!   `FormDynamicsIntegratorControl.vb:58`, `:69`, `:90`, `:94`, `:112`) throws
//!   `KeyNotFoundException` → the various `*NotFound` variants here.
//! - `Manager.vb:243`, `:251`, `:261`, `:275` — a historian lookup that finds
//!   nothing yields `FirstOrDefault()`'s default (`null`) and then dereferences
//!   it, so upstream raises `NullReferenceException` →
//!   [`DynamicsError::HistorianEmpty`] / [`DynamicsError::NoHistorianEntryBefore`].
//!
//! # Excluded DWSIM behavior
//!
//! - **Message boxes and the exception-list GUI** (`FormDynamicsIntegratorControl.vb:259`,
//!   `:583-641`, `SharedClasses.ExceptionProcessing.ExceptionList`) — a library
//!   returns errors, it does not display them.
//! - **`AggregateException` flattening** (`:611-633`) — Rust has no task
//!   aggregation here; a step failure carries its message list directly
//!   ([`StepFailure`]).
//! - **Silent `Try`/`Catch` swallowing** (`FormDynamicsIntegratorControl.vb:247-261`
//!   `RestoreHistorianState`, `DWSIM.UnitOperations/Indicators/AnalogGauge.vb:121-123`)
//!   — where upstream swallows, this port either returns an error or records a
//!   warning on the run report, never both silently.

use std::fmt;

/// Everything that can go wrong while configuring or running a dynamic
/// simulation.
///
/// Failures *inside* a caller-supplied hook (the flowsheet solve, the controller
/// step) are **not** here — they are [`StepFailure`] and are reported on
/// [`crate::dynamics::runner::RunReport`] rather than aborting with an error, so
/// the historian and the step counters stay inspectable after a failed run.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum DynamicsError {
    /// The manager's `current_schedule` names a schedule that is not registered
    /// (FormDynamicsIntegratorControl.vb:271-273).
    #[error("dynamics schedule '{0}' is not registered ('Please select a valid schedule.')")]
    ScheduleNotFound(String),

    /// The schedule's `current_integrator` names an integrator that is not
    /// registered (FormDynamicsIntegratorControl.vb:277-279).
    #[error("dynamics integrator '{0}' is not registered ('Please select a valid integrator for the selected schedule.')")]
    IntegratorNotFound(String),

    /// The schedule enables the event list but names an unregistered event set
    /// (FormDynamicsIntegratorControl.vb:58).
    #[error("dynamics event set '{0}' is not registered")]
    EventSetNotFound(String),

    /// The schedule enables the cause-and-effect matrix but names an
    /// unregistered matrix (FormDynamicsIntegratorControl.vb:90).
    #[error("cause-and-effect matrix '{0}' is not registered")]
    CauseAndEffectMatrixNotFound(String),

    /// A schedule, event, monitored variable, or cause-and-effect item refers to
    /// a flowsheet object that does not exist
    /// (FormDynamicsIntegratorControl.vb:141-143, Manager.vb:227).
    #[error("flowsheet object '{0}' does not exist")]
    ObjectNotFound(String),

    /// The named property is not carried by that kind of flowsheet object, or is
    /// carried but has never been set (`None`). Upstream's property-grid
    /// reflection returns `Nothing` here and the caller's `Convert.ToDouble`
    /// throws.
    #[error("object '{object}' has no readable property '{property}'")]
    PropertyNotAvailable {
        /// The flowsheet object's ID.
        object: String,
        /// The property that was requested.
        property: String,
    },

    /// A composition property was addressed without a compound index, or with an
    /// index past the end of the stream's compound list.
    #[error("property '{property}' on object '{object}' needs a valid compound index (given {given:?}, stream has {count} compounds)")]
    CompoundIndex {
        /// The flowsheet object's ID.
        object: String,
        /// The property that was requested.
        property: String,
        /// The index supplied, if any.
        given: Option<usize>,
        /// How many compounds the stream actually carries.
        count: usize,
    },

    /// An event's transition references an event ID that is not in the event set
    /// (Manager.vb:267-271).
    #[error("could not find reference event '{0}' for an event transition")]
    ReferenceEventNotFound(String),

    /// A transition needs the initial flowsheet state but the historian holds
    /// nothing (Manager.vb:243 / :251, where upstream dereferences `null`).
    #[error("the historian is empty; cannot resolve an event-transition reference state")]
    HistorianEmpty,

    /// A transition needs the newest historian entry at or before an instant and
    /// there is none (Manager.vb:261 / :275, again a `null` dereference
    /// upstream).
    #[error("no historian entry at or before tick {0}; cannot resolve an event-transition reference state")]
    NoHistorianEntryBefore(i64),

    /// A cause-and-effect item names an indicator with no registered state.
    /// Upstream casts the flowsheet object to `IIndicator`
    /// (FormDynamicsIntegratorControl.vb:94) and throws `InvalidCastException`
    /// if it is not one; this port keeps indicator state on the manager (see
    /// [`crate::dynamics::cause_and_effect::IndicatorState`]).
    #[error("no indicator state registered for object '{0}'")]
    IndicatorNotFound(String),

    /// The integration interval is zero or negative, which would spin the run
    /// loop forever.
    ///
    /// **Port addition.** Upstream has no such guard: with
    /// `IntegrationStep = 0` its `While i <= final` loop never terminates
    /// (FormDynamicsIntegratorControl.vb:433, :564).
    #[error("integration interval must be strictly positive, got {0} s")]
    NonPositiveInterval(f64),
}

/// A failure reported by a caller-supplied hook — the per-step flowsheet solve
/// or the controller step.
///
/// Ports the `List(Of Exception)` that `SolveFlowsheet` returns and that the run
/// loop tests with `If exceptions.Count > 0 Then Exit While`
/// (FormDynamicsIntegratorControl.vb:401, :476-486, :575).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct StepFailure {
    /// One message per underlying failure, in the order the hook produced them.
    /// Upstream rethrows `exceptions(0)`, so the **first** message is the one a
    /// DWSIM user would have seen.
    pub messages: Vec<String>,
}

impl StepFailure {
    /// A failure carrying a single message.
    #[must_use]
    pub fn new(message: impl Into<String>) -> Self {
        StepFailure {
            messages: vec![message.into()],
        }
    }

    /// A failure carrying several messages, in hook order.
    #[must_use]
    pub fn from_messages(messages: Vec<String>) -> Self {
        StepFailure { messages }
    }

    /// `true` when the hook reported nothing — the equivalent of upstream's
    /// `exceptions.Count = 0`.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.messages.is_empty()
    }
}

impl fmt::Display for StepFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.messages.is_empty() {
            write!(f, "step failed (no message given)")
        } else {
            write!(f, "{}", self.messages.join("; "))
        }
    }
}

impl std::error::Error for StepFailure {}

impl From<String> for StepFailure {
    fn from(message: String) -> Self {
        StepFailure::new(message)
    }
}

impl From<&str> for StepFailure {
    fn from(message: &str) -> Self {
        StepFailure::new(message)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn step_failure_joins_messages_in_hook_order() {
        let f = StepFailure::from_messages(vec!["first".into(), "second".into()]);
        assert_eq!(f.to_string(), "first; second");
        assert!(!f.is_empty());
        // Upstream rethrows exceptions(0): the first message is the visible one.
        assert_eq!(f.messages[0], "first");
    }

    #[test]
    fn step_failure_from_str_and_string() {
        assert_eq!(StepFailure::from("boom"), StepFailure::new("boom"));
        assert_eq!(
            StepFailure::from("boom".to_string()),
            StepFailure::new("boom")
        );
        assert!(StepFailure::default().is_empty());
    }

    #[test]
    fn errors_name_the_missing_item() {
        let e = DynamicsError::ScheduleNotFound("sched-1".into());
        assert!(e.to_string().contains("sched-1"));
        let e = DynamicsError::NonPositiveInterval(0.0);
        assert!(e.to_string().contains("strictly positive"));
    }
}
