//! The integrator — step size, run duration, subsampling rates, the simulated
//! clock, and the recorded monitored-variable series.
//!
//! # Attribution
//!
//! Pure-Rust port of **DWSIM** `DWSIM.DynamicsManager/Integrator.vb` (whole
//! file, lines 21-106), upstream commit
//! `1abf72d1b6b41d3e9a8cc770d3cc4e8fc76e5766` (branch `windows`), GPL-3.0.
//! Upstream copyright: 2020 Daniel Wagner O. de Medeiros and the DWSIM
//! contributors. This port is GPL-3.0-only. Independent OUTRAM PARK fork, **not**
//! the official DWSIM software.
//!
//! # What this type is (and is not)
//!
//! Upstream's `Integrator` is **pure data** — a settings record plus the
//! recorded series. It contains no stepping code at all; the loop that consumes
//! it lives in the GUI host and is ported separately in
//! [`crate::dynamics::runner`]. Keep that separation in mind when reading: the
//! `should_calculate_*` flags below are *written* by the run loop each step and
//! *read* by the flowsheet solver, so on this type they are a communication
//! channel, not a configuration.
//!
//! # Excluded DWSIM behavior
//!
//! - **XML serialization** — `SaveData` / `LoadData` (Integrator.vb:55-104),
//!   including the commented-out `MonitoredVariableValues` round trip
//!   (:57-65, :78-93) that upstream disabled.
//!
//! # Divergences
//!
//! - **`Duration` and `IntegrationStep` are `uom` [`Time`]**, not `TimeSpan`.
//! - **`CurrentTime` is a [`SimInstant`]**, not a `Date`; see that type's header.
//! - **`MonitoredVariableValues` is a [`BTreeMap`]**, not a
//!   `Dictionary(Of Long, List(Of …))`. Same key (the tick count,
//!   FormDynamicsIntegratorControl.vb:135) and same value shape, but ordered, so
//!   the recorded series can be read back in time order without a sort.

use std::collections::BTreeMap;

use uom::si::f64::Time;
use uom::si::time::{minute, second};

use crate::dynamics::monitored_variable::MonitoredVariable;
use crate::dynamics::sim_time::SimInstant;

/// One integrator configuration — upstream's `Integrator`
/// (Integrator.vb:21-106).
///
/// Units: [`Integrator::integration_step`] and [`Integrator::duration`] are
/// `uom` times \[s\]; [`Integrator::real_time_step_ms`] is a wall-clock budget in
/// **milliseconds** (upstream keeps it as a raw `Integer`, Integrator.vb:53).
#[derive(Debug, Clone, PartialEq)]
pub struct Integrator {
    /// Unique identifier, and the key the manager stores it under
    /// (Integrator.vb:25).
    pub id: String,
    /// Human-readable label. **This is what
    /// [`crate::dynamics::manager::DynamicsManager::integrator_by_description`]
    /// looks up by** — upstream's `GetIntegrator` matches on `Description`
    /// (Manager.vb:197-199). (Integrator.vb:27.)
    pub description: String,

    /// Set by the run loop each step: whether the flowsheet solver should run
    /// its equilibrium (flash) calculations this step (Integrator.vb:29, written
    /// at FormDynamicsIntegratorControl.vb:462-467).
    pub should_calculate_equilibrium: bool,
    /// Set by the run loop each step: whether the solver should run its
    /// pressure-flow network this step (Integrator.vb:31, written at :469-474).
    pub should_calculate_pressure_flow: bool,
    /// Set by the run loop each step: whether controllers should be stepped this
    /// step (Integrator.vb:33, written at :455-460, read at :518).
    pub should_calculate_control: bool,

    /// Simulated time advanced per step \[s\]. Upstream default:
    /// `New TimeSpan(0, 0, 5)` = 5 s (Integrator.vb:35).
    ///
    /// **Ignored in real-time mode**, where the step size becomes
    /// [`Integrator::real_time_step_ms`] (FormDynamicsIntegratorControl.vb:302).
    pub integration_step: Time,
    /// Total simulated time to run \[s\]. Upstream default:
    /// `New TimeSpan(0, 10, 0)` = 10 min (Integrator.vb:37).
    ///
    /// **Ignored in real-time mode**, where the run is unbounded
    /// (FormDynamicsIntegratorControl.vb:338, `ProgressBar1.Maximum = Integer.MaxValue`).
    pub duration: Time,
    /// The simulated clock (Integrator.vb:39; `New Date()` =
    /// [`SimInstant::ZERO`]).
    pub current_time: SimInstant,

    /// Run the equilibrium calculation once every N steps (Integrator.vb:41,
    /// default 1 = every step; consumed at
    /// FormDynamicsIntegratorControl.vb:462).
    pub calculation_rate_equilibrium: u32,
    /// Run the pressure-flow calculation once every N steps
    /// (Integrator.vb:43, default 1; consumed at :469).
    pub calculation_rate_pressure_flow: u32,
    /// Step the controllers once every N steps (Integrator.vb:45, default 1;
    /// consumed at :455).
    pub calculation_rate_control: u32,

    /// Whether the last run was a real-time run (Integrator.vb:47). The run loop
    /// writes this from its own argument (:292); it is a record of the mode, not
    /// a request for it.
    pub real_time: bool,
    /// Wall-clock budget per step in real-time mode \[ms\] (Integrator.vb:53,
    /// default 1000). Doubles as the simulated step size in real-time mode
    /// (:302).
    pub real_time_step_ms: u32,

    /// The recorded series: monitored-variable samples keyed by the simulated
    /// time they were taken at (Integrator.vb:49; keyed by `tstamp.Ticks` at
    /// FormDynamicsIntegratorControl.vb:135, :150).
    ///
    /// Each entry holds one sample per template in
    /// [`Integrator::monitored_variables`], in the same order.
    pub monitored_variable_values: BTreeMap<SimInstant, Vec<MonitoredVariable>>,
    /// The sampling templates (Integrator.vb:51).
    pub monitored_variables: Vec<MonitoredVariable>,
}

impl Default for Integrator {
    /// Upstream's field initialisers verbatim (Integrator.vb:25-53): 5 s step,
    /// 10 min duration, clock at zero, all three subsampling rates 1, not
    /// real-time, 1000 ms real-time budget.
    fn default() -> Self {
        Integrator {
            id: String::new(),
            description: String::new(),
            should_calculate_equilibrium: false,
            should_calculate_pressure_flow: false,
            should_calculate_control: false,
            integration_step: Time::new::<second>(5.0),
            duration: Time::new::<minute>(10.0),
            current_time: SimInstant::ZERO,
            calculation_rate_equilibrium: 1,
            calculation_rate_pressure_flow: 1,
            calculation_rate_control: 1,
            real_time: false,
            real_time_step_ms: 1000,
            monitored_variable_values: BTreeMap::new(),
            monitored_variables: Vec::new(),
        }
    }
}

impl Integrator {
    /// An integrator with the upstream defaults, an ID and a description.
    #[must_use]
    pub fn new(id: impl Into<String>, description: impl Into<String>) -> Self {
        Integrator {
            id: id.into(),
            description: description.into(),
            ..Integrator::default()
        }
    }

    /// Builder-style setter for the step size and total duration.
    #[must_use]
    pub fn with_schedule_times(mut self, integration_step: Time, duration: Time) -> Self {
        self.integration_step = integration_step;
        self.duration = duration;
        self
    }

    /// Builder-style setter for the three subsampling rates (control,
    /// equilibrium, pressure-flow), each "once every N steps".
    ///
    /// A rate of `0` or `1` means "every step" — see
    /// [`crate::dynamics::runner`] for the exact accumulator arithmetic, which
    /// is upstream's and is not a plain modulo.
    #[must_use]
    pub fn with_calculation_rates(
        mut self,
        control: u32,
        equilibrium: u32,
        pressure_flow: u32,
    ) -> Self {
        self.calculation_rate_control = control;
        self.calculation_rate_equilibrium = equilibrium;
        self.calculation_rate_pressure_flow = pressure_flow;
        self
    }

    /// Add a sampling template.
    pub fn monitor(&mut self, variable: MonitoredVariable) {
        self.monitored_variables.push(variable);
    }

    /// The simulated step size actually used for a run \[s\]: the integration
    /// step normally, or `real_time_step_ms / 1000` in real-time mode.
    ///
    /// Ports FormDynamicsIntegratorControl.vb:300-302:
    ///
    /// ```text
    /// Dim interval = integrator.IntegrationStep.TotalSeconds
    /// If realtime Then interval = Convert.ToDouble(integrator.RealTimeStepMs) / 1000.0
    /// ```
    #[must_use]
    pub fn effective_interval(&self, real_time: bool) -> Time {
        if real_time {
            Time::new::<second>(f64::from(self.real_time_step_ms) / 1000.0)
        } else {
            self.integration_step
        }
    }

    /// The recorded series for the monitored variable at index `i`, as
    /// `(elapsed simulated time, value in that variable's display units)` pairs
    /// in time order.
    ///
    /// This is the data half of upstream's `GetChartModel`
    /// (Manager.vb:107-191): the x-values are
    /// `New TimeSpan(item.Key).TotalMilliseconds / 1000.0` (:119) and the
    /// y-values are the i-th sample of each entry (:173-179). The OxyPlot model,
    /// axes, legend and unit-system conversion of the x-axis are excluded.
    ///
    /// Returns an empty vector if `i` is past the end of
    /// [`Integrator::monitored_variables`].
    #[must_use]
    pub fn monitored_series(&self, i: usize) -> Vec<(Time, f64)> {
        self.monitored_variable_values
            .iter()
            .filter_map(|(t, samples)| samples.get(i).map(|s| (t.elapsed(), s.property_value)))
            .collect()
    }

    /// How many samples have been recorded (one per step, not per variable).
    #[must_use]
    pub fn sample_count(&self) -> usize {
        self.monitored_variable_values.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dynamics::property::DynamicProperty;
    use crate::flowsheet::objects::ObjectId;

    #[test]
    fn defaults_match_upstream_initialisers() {
        let i = Integrator::default();
        assert!((i.integration_step.get::<second>() - 5.0).abs() < 1e-12);
        assert!((i.duration.get::<second>() - 600.0).abs() < 1e-9);
        assert_eq!(i.current_time, SimInstant::ZERO);
        assert_eq!(i.calculation_rate_control, 1);
        assert_eq!(i.calculation_rate_equilibrium, 1);
        assert_eq!(i.calculation_rate_pressure_flow, 1);
        assert!(!i.real_time);
        assert_eq!(i.real_time_step_ms, 1000);
    }

    #[test]
    fn real_time_mode_replaces_the_integration_step() {
        let i = Integrator {
            real_time_step_ms: 250,
            ..Integrator::default()
        };
        assert!((i.effective_interval(false).get::<second>() - 5.0).abs() < 1e-12);
        assert!((i.effective_interval(true).get::<second>() - 0.25).abs() < 1e-12);
    }

    #[test]
    fn monitored_series_reads_back_in_time_order() {
        let mut integrator = Integrator::new("int-1", "Default");
        integrator.monitor(MonitoredVariable::new(
            "T",
            ObjectId::from("S-1"),
            DynamicProperty::Temperature,
            "K",
        ));
        for (n, seconds) in [(300.0, 10.0), (310.0, 5.0), (320.0, 15.0)] {
            let mut sample = integrator.monitored_variables[0].clone();
            sample.property_value = n;
            integrator
                .monitored_variable_values
                .insert(SimInstant::from_seconds(seconds), vec![sample]);
        }
        let series = integrator.monitored_series(0);
        let times: Vec<f64> = series.iter().map(|(t, _)| t.get::<second>()).collect();
        let values: Vec<f64> = series.iter().map(|(_, v)| *v).collect();
        assert_eq!(times, vec![5.0, 10.0, 15.0]);
        assert_eq!(values, vec![310.0, 300.0, 320.0]);
        assert_eq!(integrator.sample_count(), 3);
        assert!(integrator.monitored_series(9).is_empty());
    }
}
