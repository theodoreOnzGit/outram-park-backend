//! Temperature input: degrees Celsius or Kelvin, toggled by the user (op-dzl).
//!
//! The plot screen lets the user type a temperature and pick which unit that
//! number is in; this module owns the (trivial, exact) conversion so the rest
//! of the app always works in kelvin, the unit
//! [`njoy_outram_park_fork::wmp::WindowedMultipole::evaluate`] takes.

/// Which unit the user's typed temperature number is currently in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TempUnit {
    #[default]
    Celsius,
    Kelvin,
}

impl TempUnit {
    /// Flip to the other unit.
    pub fn toggled(self) -> Self {
        match self {
            TempUnit::Celsius => TempUnit::Kelvin,
            TempUnit::Kelvin => TempUnit::Celsius,
        }
    }

    /// Short label for the unit toggle button, e.g. `"degC"` / `"K"`.
    pub fn label(self) -> &'static str {
        match self {
            TempUnit::Celsius => "degC",
            TempUnit::Kelvin => "K",
        }
    }
}

/// Exact offset between the Celsius and Kelvin scales (ITS-90), matching the
/// conversion `uom`'s `thermodynamic_temperature` units use internally.
pub const CELSIUS_TO_KELVIN_OFFSET: f64 = 273.15;

/// Convert a temperature value `t` **in the unit `unit`** to kelvin.
///
/// `t` in Celsius may be negative (down to absolute zero at -273.15 degC);
/// the result is not clamped here — [`to_kelvin_clamped`] is the
/// physically-valid entry point for feeding a Doppler-broadening evaluator.
pub fn to_kelvin(t: f64, unit: TempUnit) -> f64 {
    match unit {
        TempUnit::Celsius => t + CELSIUS_TO_KELVIN_OFFSET,
        TempUnit::Kelvin => t,
    }
}

/// [`to_kelvin`], clamped to `>= 0 K` (absolute zero) — the physically valid
/// domain for [`njoy_outram_park_fork::wmp::WindowedMultipole::evaluate`],
/// which treats `0 K` as its exact zero-broadening asymptotic case.
pub fn to_kelvin_clamped(t: f64, unit: TempUnit) -> f64 {
    to_kelvin(t, unit).max(0.0)
}

/// Convert a kelvin value back to Celsius (for redisplay after a unit
/// toggle, so the number the user sees updates rather than being
/// reinterpreted in the new unit).
pub fn kelvin_to_celsius(k: f64) -> f64 {
    k - CELSIUS_TO_KELVIN_OFFSET
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn celsius_room_temperature_is_about_298_kelvin() {
        let k = to_kelvin(25.0, TempUnit::Celsius);
        assert!((k - 298.15).abs() < 1e-9);
    }

    #[test]
    fn kelvin_passes_through_unchanged() {
        assert_eq!(to_kelvin(900.0, TempUnit::Kelvin), 900.0);
    }

    #[test]
    fn negative_celsius_below_absolute_zero_clamps_to_zero_kelvin() {
        assert_eq!(to_kelvin_clamped(-300.0, TempUnit::Celsius), 0.0);
    }

    #[test]
    fn round_trip_kelvin_to_celsius() {
        let k = to_kelvin(1000.0, TempUnit::Celsius);
        assert!((kelvin_to_celsius(k) - 1000.0).abs() < 1e-9);
    }

    #[test]
    fn toggle_flips_and_back() {
        assert_eq!(TempUnit::Celsius.toggled(), TempUnit::Kelvin);
        assert_eq!(TempUnit::Kelvin.toggled(), TempUnit::Celsius);
    }
}
