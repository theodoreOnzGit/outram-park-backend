//! Schedules — which integrator, event set and cause-and-effect matrix a
//! dynamic run uses, and what state it starts from.
//!
//! # Attribution
//!
//! Pure-Rust port of **DWSIM** `DWSIM.DynamicsManager/Schedule.vb` (whole file,
//! lines 21-54), upstream commit
//! `1abf72d1b6b41d3e9a8cc770d3cc4e8fc76e5766` (branch `windows`), GPL-3.0.
//! Upstream copyright: 2020 Daniel Wagner O. de Medeiros and the DWSIM
//! contributors. This port is GPL-3.0-only. Independent OUTRAM PARK fork, **not**
//! the official DWSIM software.
//!
//! # Excluded DWSIM behavior
//!
//! - **XML serialization** — `SaveData` / `LoadData` (Schedule.vb:45-52).
//! - **The stored-solution library.** `InitialFlowsheetStateID` indexes
//!   `Flowsheet.StoredSolutions`, a named-state store that lives on DWSIM's
//!   flowsheet and is restored with `LoadProcessData`
//!   (FormDynamicsIntegratorControl.vb:227-243). This crate's flowsheet has no
//!   such store, so [`crate::dynamics::manager::DynamicsManager::stored_states`]
//!   holds the named states instead — a **port addition**, documented there.

/// One dynamic-run configuration — upstream's `Schedule` (Schedule.vb:21-54).
#[derive(Debug, Clone, PartialEq)]
pub struct Schedule {
    /// Unique identifier, and the key the manager stores it under
    /// (Schedule.vb:25).
    pub id: String,
    /// Human-readable label. **This is what
    /// [`crate::dynamics::manager::DynamicsManager::schedule_by_description`]
    /// looks up by** — upstream's `GetSchedule` matches on `Description`
    /// (Manager.vb:193-195). (Schedule.vb:27.)
    pub description: String,
    /// ID of the integrator to run (Schedule.vb:29). Must be a key of
    /// [`crate::dynamics::manager::DynamicsManager::integrators`].
    pub current_integrator: String,
    /// Whether to process the cause-and-effect matrix each step
    /// (Schedule.vb:31, tested at FormDynamicsIntegratorControl.vb:559).
    pub uses_cause_and_effect_matrix: bool,
    /// Whether to process the event list each step (Schedule.vb:33, tested at
    /// FormDynamicsIntegratorControl.vb:556).
    pub uses_event_list: bool,
    /// ID of the cause-and-effect matrix to use (Schedule.vb:35).
    pub current_cause_and_effect_matrix: String,
    /// ID of the event set to use (Schedule.vb:37).
    pub current_event_list: String,
    /// Name of the stored flowsheet state to start from, when
    /// [`Schedule::use_current_state_as_initial`] is `false` (Schedule.vb:39,
    /// consumed at FormDynamicsIntegratorControl.vb:309).
    pub initial_flowsheet_state_id: String,
    /// Start from the flowsheet as it stands rather than from a stored state
    /// (Schedule.vb:41, default `True`; tested at :308).
    pub use_current_state_as_initial: bool,
    /// Ask every unit operation to reset its held contents before the run
    /// (Schedule.vb:43, default `False`; consumed at :361-381).
    pub reset_contents_of_all_objects: bool,
}

impl Default for Schedule {
    /// Upstream's field initialisers verbatim (Schedule.vb:25-43): empty IDs,
    /// no event list, no cause-and-effect matrix, start from the current state,
    /// do not reset object contents.
    fn default() -> Self {
        Schedule {
            id: String::new(),
            description: String::new(),
            current_integrator: String::new(),
            uses_cause_and_effect_matrix: false,
            uses_event_list: false,
            current_cause_and_effect_matrix: String::new(),
            current_event_list: String::new(),
            initial_flowsheet_state_id: String::new(),
            use_current_state_as_initial: true,
            reset_contents_of_all_objects: false,
        }
    }
}

impl Schedule {
    /// A schedule with the upstream defaults that runs the named integrator.
    #[must_use]
    pub fn new(
        id: impl Into<String>,
        description: impl Into<String>,
        integrator_id: impl Into<String>,
    ) -> Self {
        Schedule {
            id: id.into(),
            description: description.into(),
            current_integrator: integrator_id.into(),
            ..Schedule::default()
        }
    }

    /// Builder-style setter that turns the event list on and points it at an
    /// event set.
    #[must_use]
    pub fn with_event_list(mut self, event_set_id: impl Into<String>) -> Self {
        self.uses_event_list = true;
        self.current_event_list = event_set_id.into();
        self
    }

    /// Builder-style setter that turns the cause-and-effect matrix on and points
    /// it at a matrix.
    #[must_use]
    pub fn with_cause_and_effect_matrix(mut self, matrix_id: impl Into<String>) -> Self {
        self.uses_cause_and_effect_matrix = true;
        self.current_cause_and_effect_matrix = matrix_id.into();
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_upstream_initialisers() {
        let s = Schedule::default();
        assert!(s.use_current_state_as_initial);
        assert!(!s.reset_contents_of_all_objects);
        assert!(!s.uses_event_list);
        assert!(!s.uses_cause_and_effect_matrix);
    }

    #[test]
    fn builders_enable_the_matching_flag() {
        let s = Schedule::new("sch-1", "Startup", "int-1")
            .with_event_list("es-1")
            .with_cause_and_effect_matrix("cem-1");
        assert_eq!(s.current_integrator, "int-1");
        assert!(s.uses_event_list && s.current_event_list == "es-1");
        assert!(s.uses_cause_and_effect_matrix && s.current_cause_and_effect_matrix == "cem-1");
    }
}
