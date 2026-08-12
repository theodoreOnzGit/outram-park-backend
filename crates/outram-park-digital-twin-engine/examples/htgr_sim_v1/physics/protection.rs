//! Reactor protection system: automatic scram on measurable trip signals.
//!
//! Without this, withdrawing the control-rod bank fully exposes the published
//! cold clean excess of **+16.45 $** (see [`super::control_rods`]). Prompt
//! critical is 1 $, so that is a violent prompt excursion: power runs away,
//! the fuel heats in milliseconds, and the plant model leaves the range where
//! its property libraries are defined -- which surfaced as a panic and a
//! restart modal rather than as a diagnosis.
//!
//! # A protection system trips on what it can MEASURE
//!
//! The quantity actually being protected is the **fuel temperature**, whose
//! HTR-10 limit is 1230 degC ([`Htr10FuelTemperatureLimits::fuel_temperature_limit`],
//! Gao & Shi 2002). But no instrument measures fuel temperature in a pebble
//! bed -- the fuel is 27,000 tumbling spheres. A real protection system
//! therefore trips on signals it *can* see, chosen so the fuel limit is not
//! reached. This module does the same, and the distinction is deliberate: a
//! trip wired directly to a modelled fuel temperature would be a simulator
//! convenience with no counterpart in a real plant.
//!
//! Two signals, because they cover opposite timescales:
//!
//! | Trip | Catches | Why the other one cannot |
//! |---|---|---|
//! | **High neutron flux** | prompt excursions (rod withdrawal) | thermal signals lag by the bed's ~184 s time constant; the fuel is destroyed long before helium outlet moves |
//! | **High core outlet temperature** | loss of heat sink, loss of flow | power is *normal* in those events, so a flux trip never fires |
//!
//! A single trip would leave one of those two entirely unprotected.
//!
//! # What is REAL and what is INVENTED here
//!
//! **The setpoints are invented.** IAEA-TECDOC-1382 is a reactor-physics
//! benchmark and carries no protection-system setpoints, and none were found in
//! the plant-data sheet. The values below are plausible engineering choices
//! marked as such, and sourcing them is tracked as a bead. Do not quote them as
//! HTR-10 figures.
//!
//! **The fuel limit they protect is real**: 1230 degC, published, and read from
//! the library rather than restated here.
//!
//! # An important caveat about what this simulator is demonstrating
//!
//! HTR-10's actual safety demonstration tests include **control-rod withdrawal
//! WITHOUT scram** (see `docs/reactor-scoping/htr10.md`). The real reactor
//! survives that unprotected: negative temperature feedback turns the excursion
//! over before the fuel is damaged, and demonstrating exactly that is the point
//! of a modular HTGR.
//!
//! **This model does not reproduce that.** Its Doppler coefficient is an
//! illustrative -4.0e-5 per K, not an HTR-10 evaluation, and against +16.45 $
//! it would need a fuel temperature rise of order 2700 K to compensate -- far
//! past any material limit. So the scram here is genuinely protecting the
//! model, not demonstrating HTR-10's inherent safety. Adding a scram must not
//! be mistaken for fixing that: the underlying gap is the feedback coefficient,
//! tracked separately.

use outram_park_digital_twin_engine::htr10::design::{Htr10DesignPoint, Htr10FuelTemperatureLimits};
use uom::si::f64::{Power, ThermodynamicTemperature, Time};
use uom::si::power::megawatt;
use uom::si::thermodynamic_temperature::degree_celsius;
use uom::si::time::second;

/// High neutron flux trip setpoint, as a fraction of nominal thermal power.
///
/// **INVENTED, not an HTR-10 figure.** 120% of rated is a conventional choice
/// for a high-flux trip: far enough above rated that normal manoeuvring does
/// not spuriously trip, low enough to act early in an excursion. No published
/// HTR-10 setpoint was found.
pub const HIGH_FLUX_TRIP_FRACTION: f64 = 1.20;

/// High core-outlet-temperature trip setpoint \[degC\].
///
/// **INVENTED, not an HTR-10 figure.** 750 degC sits 50 K above the published
/// 700 degC rated outlet -- enough headroom for normal operation, early enough
/// to act on a loss of heat sink well before the fuel limit is approached.
pub const HIGH_OUTLET_TEMPERATURE_TRIP_DEGC: f64 = 750.0;

/// Time for the rod bank to travel from fully withdrawn to fully inserted on a
/// scram \[s\].
///
/// **INVENTED, not an HTR-10 figure.** 2 seconds is a plausible gravity-assisted
/// bank drop. Note this is deliberately much faster than a normal motor-driven
/// withdrawal: a scram releases the rods rather than driving them.
pub const SCRAM_INSERTION_TIME_S: f64 = 2.0;

/// Why the protection system tripped.
///
/// Enum rather than a string so every consumer must handle each case, per the
/// workspace's enum-dispatch rule.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TripReason {
    /// Reactor power exceeded [`HIGH_FLUX_TRIP_FRACTION`] of nominal. This is
    /// the trip that catches a prompt excursion.
    HighNeutronFlux,
    /// Core outlet helium exceeded [`HIGH_OUTLET_TEMPERATURE_TRIP_DEGC`]. This
    /// is the trip that catches a loss of heat sink or of flow.
    HighCoreOutletTemperature,
}

impl TripReason {
    /// Operator-facing description, suitable for a banner in the GUI.
    pub fn description(&self) -> &'static str {
        match self {
            Self::HighNeutronFlux => "High neutron flux (reactor power above 120% of rated)",
            Self::HighCoreOutletTemperature => {
                "High core outlet temperature (helium above 750 degC)"
            }
        }
    }
}

/// Latching reactor protection system.
///
/// Once tripped it stays tripped until [`ReactorProtectionSystem::reset`] is
/// called, and it drives the rod bank fully in regardless of the operator's
/// commanded position. That latching is the point: a protection system that
/// cleared itself as soon as the signal recovered would let a reactor
/// oscillate in and out of a trip condition.
#[derive(Clone, Debug, Default)]
pub struct ReactorProtectionSystem {
    /// `Some(reason)` once tripped; `None` while healthy.
    trip: Option<TripReason>,
    /// How far the scram has driven the bank in, `0.0..=1.0`. Ramps toward 1.0
    /// over [`SCRAM_INSERTION_TIME_S`] once tripped.
    scram_insertion: f64,
}

impl ReactorProtectionSystem {
    /// A healthy, untripped protection system.
    pub fn new() -> Self {
        Self::default()
    }

    /// Whether the reactor is currently tripped.
    pub fn is_tripped(&self) -> bool {
        self.trip.is_some()
    }

    /// Why it tripped, if it has.
    pub fn trip_reason(&self) -> Option<TripReason> {
        self.trip
    }

    /// How far the scram has driven the bank in, `0.0..=1.0`.
    pub fn scram_insertion(&self) -> f64 {
        self.scram_insertion
    }

    /// Clears the trip and retracts the scram demand, allowing the operator's
    /// rod command to take effect again.
    ///
    /// Does **not** move the rods itself -- after a reset the effective
    /// insertion returns to whatever the operator is commanding, so an operator
    /// who resets with the bank still commanded fully withdrawn will simply
    /// trip again. That is intentional and mirrors real practice: a reset is
    /// not a recovery.
    pub fn reset(&mut self) {
        self.trip = None;
        self.scram_insertion = 0.0;
    }

    /// Evaluates the trip signals and advances the scram ramp by `dt`.
    ///
    /// `reactor_power` and `core_outlet_temperature` are the measured signals.
    /// Returns nothing; call [`ReactorProtectionSystem::effective_rod_insertion`]
    /// for the rod position the plant should actually use.
    pub fn update(
        &mut self,
        dt: Time,
        reactor_power: Power,
        core_outlet_temperature: ThermodynamicTemperature,
    ) {
        if self.trip.is_none() {
            let nominal_mw = Htr10DesignPoint::iaea_benchmark()
                .thermal_power
                .get::<megawatt>();
            let power_mw = reactor_power.get::<megawatt>();
            let outlet_degc = core_outlet_temperature.get::<degree_celsius>();

            // Flux first: it is the fast trip, and in an excursion both may be
            // satisfied eventually, so the reported cause should be the one
            // that actually acted.
            if power_mw > HIGH_FLUX_TRIP_FRACTION * nominal_mw {
                self.trip = Some(TripReason::HighNeutronFlux);
            } else if outlet_degc > HIGH_OUTLET_TEMPERATURE_TRIP_DEGC {
                self.trip = Some(TripReason::HighCoreOutletTemperature);
            }
        }

        if self.trip.is_some() && self.scram_insertion < 1.0 {
            let step = dt.get::<second>() / SCRAM_INSERTION_TIME_S;
            self.scram_insertion = (self.scram_insertion + step).min(1.0);
        }
    }

    /// The rod insertion the plant should use: the operator's command, or the
    /// scram demand if that is deeper.
    ///
    /// Taking the **maximum** rather than overriding outright means an operator
    /// who is already inserting rods faster than the scram ramp is not held
    /// back by it -- the protection system can only ever add shutdown margin,
    /// never remove it.
    pub fn effective_rod_insertion(&self, operator_insertion: f64) -> f64 {
        operator_insertion.clamp(0.0, 1.0).max(self.scram_insertion)
    }

    /// The published HTR-10 fuel temperature limit this system exists to
    /// protect: 1230 degC (Gao & Shi 2002), read from the library.
    ///
    /// Provided so the GUI can show the margin against a **real, sourced**
    /// limit. Note this is *not* the generic 1600 degC modular-HTR retention
    /// figure -- `Htr10DesignPoint`'s own docs warn the two must not be
    /// conflated, and any HTR-10 margin uses 1230.
    pub fn protected_fuel_limit() -> ThermodynamicTemperature {
        Htr10FuelTemperatureLimits::gao_shi_2002().fuel_temperature_limit
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dt() -> Time {
        Time::new::<second>(0.05)
    }

    fn cool_outlet() -> ThermodynamicTemperature {
        ThermodynamicTemperature::new::<degree_celsius>(700.0)
    }

    /// A healthy plant at rated conditions must not trip.
    ///
    /// **Methodology.** Run 2000 steps at exactly rated power (10 MWth) and the
    /// published 700 degC outlet -- the normal operating point. Assert no trip
    /// and no scram demand. A protection system that trips at rated conditions
    /// is worse than none.
    ///
    /// **Results (2026-08-12).** No trip over 2000 steps; scram insertion
    /// remained 0.0. Interpretation: both setpoints sit clear of the rated
    /// operating point, so normal operation is not disturbed.
    #[test]
    fn rated_conditions_do_not_trip() {
        let mut rps = ReactorProtectionSystem::new();
        for _ in 0..2000 {
            rps.update(dt(), Power::new::<megawatt>(10.0), cool_outlet());
        }
        assert!(!rps.is_tripped());
        assert_eq!(rps.scram_insertion(), 0.0);
    }

    /// A power excursion trips on flux and drives the bank fully in.
    ///
    /// **Methodology.** Feed 15 MWth (150% of rated) with a normal outlet
    /// temperature, so only the flux trip can fire. Assert it trips on
    /// [`TripReason::HighNeutronFlux`], and that the scram reaches full
    /// insertion within [`SCRAM_INSERTION_TIME_S`] plus one timestep.
    ///
    /// **Results (2026-08-12).** Tripped on the first update with
    /// `HighNeutronFlux`. Scram insertion reached 1.0 after 40 steps of 0.05 s
    /// = 2.00 s, matching the specified insertion time exactly.
    /// Interpretation: the fast trip acts on the signal that actually moves
    /// during a prompt excursion, and the bank travel is rate-limited rather
    /// than teleporting to fully inserted.
    #[test]
    fn a_power_excursion_trips_on_flux_and_scrams() {
        let mut rps = ReactorProtectionSystem::new();
        rps.update(dt(), Power::new::<megawatt>(15.0), cool_outlet());
        assert_eq!(rps.trip_reason(), Some(TripReason::HighNeutronFlux));

        for _ in 0..40 {
            rps.update(dt(), Power::new::<megawatt>(15.0), cool_outlet());
        }
        assert!(
            (rps.scram_insertion() - 1.0).abs() < 1e-9,
            "scram insertion = {}",
            rps.scram_insertion()
        );
    }

    /// A loss of heat sink trips on outlet temperature, which the flux trip
    /// cannot catch.
    ///
    /// **Methodology.** Feed rated power -- so the flux trip is explicitly not
    /// satisfied -- with an 800 degC outlet. Assert the trip fires and reports
    /// [`TripReason::HighCoreOutletTemperature`], proving the second signal is
    /// doing independent work rather than being redundant.
    ///
    /// **Results (2026-08-12).** Tripped on `HighCoreOutletTemperature` at
    /// rated power. Interpretation: an event where power is normal but heat is
    /// not being removed is covered; a flux-only protection system would have
    /// missed it entirely.
    #[test]
    fn a_loss_of_heat_sink_trips_on_temperature_at_normal_power() {
        let mut rps = ReactorProtectionSystem::new();
        rps.update(
            dt(),
            Power::new::<megawatt>(10.0),
            ThermodynamicTemperature::new::<degree_celsius>(800.0),
        );
        assert_eq!(
            rps.trip_reason(),
            Some(TripReason::HighCoreOutletTemperature)
        );
    }

    /// The trip latches, and a scram can only ever add shutdown margin.
    ///
    /// **Methodology.** Trip on flux, then feed healthy signals for 2000 steps
    /// and assert it stays tripped. Then check `effective_rod_insertion` takes
    /// the maximum of operator and scram demand: with the scram fully in, an
    /// operator commanding 0.0 (fully withdrawn) must still see 1.0; with the
    /// operator commanding deeper than a partial scram, the operator wins.
    ///
    /// **Results (2026-08-12).** Remained tripped across 2000 healthy steps.
    /// With scram at 1.0 and operator at 0.0, effective insertion was 1.0. With
    /// scram at 0.5 and operator at 0.8, effective insertion was 0.8.
    /// Interpretation: the reactor cannot oscillate in and out of a trip, and
    /// the protection system never removes margin an operator has already
    /// added.
    #[test]
    fn the_trip_latches_and_never_removes_shutdown_margin() {
        let mut rps = ReactorProtectionSystem::new();
        rps.update(dt(), Power::new::<megawatt>(15.0), cool_outlet());
        for _ in 0..2000 {
            rps.update(dt(), Power::new::<megawatt>(1.0), cool_outlet());
        }
        assert!(rps.is_tripped(), "the trip must latch");
        assert_eq!(rps.effective_rod_insertion(0.0), 1.0);

        rps.reset();
        assert!(!rps.is_tripped());
        assert_eq!(rps.effective_rod_insertion(0.0), 0.0);

        rps.trip = Some(TripReason::HighNeutronFlux);
        rps.scram_insertion = 0.5;
        assert_eq!(rps.effective_rod_insertion(0.8), 0.8);
        assert_eq!(rps.effective_rod_insertion(0.1), 0.5);
    }

    /// The limit this system protects is the HTR-10's own 1230 degC, not the
    /// generic 1600 degC modular-HTR figure.
    ///
    /// **Methodology.** `Htr10DesignPoint`'s docs warn explicitly that the two
    /// must not be conflated and that any HTR-10 margin calculation uses 1230.
    /// Assert the accessor returns 1230 degC and not 1600.
    ///
    /// **Results (2026-08-12).** Returned 1230.0 degC. Interpretation: the GUI
    /// margin readout is against the published HTR-10 limit, so a displayed
    /// margin cannot silently be 370 K more generous than the plant allows.
    #[test]
    fn the_protected_limit_is_the_htr10_one_not_the_generic_figure() {
        let limit = ReactorProtectionSystem::protected_fuel_limit().get::<degree_celsius>();
        assert!((limit - 1230.0).abs() < 1e-6, "limit = {limit} degC");
        assert!((limit - 1600.0).abs() > 300.0);
    }
}
