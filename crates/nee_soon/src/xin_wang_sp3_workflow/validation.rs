//! Stage 4 — Figure-4.29 validation (the reproduction target).
//!
//! Compares the Stage-3 coupled SP3 transient against the digitised Figure-4.29
//! reference curve (maximum fuel temperature vs time during the control-rod-
//! removal transient; [`super::case::fig_4_29_reference_curve`]). Also cross-
//! checks Fig. 4.27 (full-core power: ~236 MW → ~+30 % peak → ~+30 % settle).
//!
//! **Nature of the check.** Wang's own reference is a code-to-code result
//! (Serpent + COMSOL), and PB-FHR has no experimental data — so this is
//! code-to-code **verification**, not experimental validation. The V&V write-up
//! must state methodology *and* measured numbers with uncertainty (workspace V&V
//! rule).
//!
//! **Placeholder stage.** Depends on Stages 1–3 (none of which run yet). Tracked
//! by bead **op-fr2.2.5**.

use uom::si::f64::Power;
use uom::si::power::watt;

use super::case::{self, Fig429Point};
use super::{WorkflowError, WorkflowStage};

/// Bead tracking the Figure-4.29 validation stage.
pub const STAGE_BEAD: &str = "op-fr2.2.5";

/// The expected qualitative features of Figure 4.29 the reproduction must match.
#[derive(Debug, Clone, Copy)]
pub struct Fig429Target {
    /// Approximate peak maximum-fuel-temperature (~988 °C near 8 s), °C.
    pub peak_max_fuel_temperature_celsius: f64,
    /// Approximate end-of-transient value (~1006 °C at 100 s), °C.
    pub end_max_fuel_temperature_celsius: f64,
    /// Fig. 4.27 peak power as a fraction above initial (~+30 %).
    pub peak_power_fraction_above_initial: f64,
    /// Full-core initial power (236 MW).
    pub initial_power: Power,
}

impl Fig429Target {
    /// The Figure-4.29 / 4.27 acceptance features (digitised, approximate).
    pub fn digitised() -> Self {
        Self {
            peak_max_fuel_temperature_celsius: 988.0,
            end_max_fuel_temperature_celsius: 1006.0,
            peak_power_fraction_above_initial: 0.30,
            initial_power: Power::new::<watt>(236.0e6),
        }
    }
}

/// Stage-4 driver: loads the reference curve and defines the comparison.
#[derive(Debug, Clone)]
pub struct Fig429ValidationStage {
    reference_curve: Vec<Fig429Point>,
    target: Fig429Target,
}

impl Default for Fig429ValidationStage {
    fn default() -> Self {
        Self::new()
    }
}

impl Fig429ValidationStage {
    /// Creates the stage with the digitised Figure-4.29 reference curve loaded.
    pub fn new() -> Self {
        Self {
            reference_curve: case::fig_4_29_reference_curve(),
            target: Fig429Target::digitised(),
        }
    }

    /// The digitised Figure-4.29 reference curve (max fuel temperature vs time).
    pub fn reference_curve(&self) -> &[Fig429Point] {
        &self.reference_curve
    }

    /// The qualitative acceptance features to match.
    pub fn target(&self) -> Fig429Target {
        self.target
    }

    /// Run the Stage-3 transient and compare its max-fuel-temperature history
    /// against the reference curve.
    ///
    /// # Placeholder
    ///
    /// Returns [`WorkflowError::NotYetImplemented`]. The intended body runs the
    /// [`super::sp3_multiphysics::Sp3MultiphysicsStage`] transient, extracts the
    /// max-fuel-temperature time series, interpolates the reference curve at the
    /// simulated times, computes the deviation with uncertainty, and records a
    /// methodology-plus-results V&V report (code-to-code verification).
    // TODO(op-fr2.2.5): run Stage-3 transient, compare max-fuel-temp history vs
    // fig_4_29_reference_curve(), emit a methodology+results V&V report. Depends
    // on Stages 1-3.
    pub fn run(&self) -> Result<(), WorkflowError> {
        Err(WorkflowError::NotYetImplemented {
            stage: WorkflowStage::Fig429Validation,
            bead: STAGE_BEAD,
        })
    }
}
