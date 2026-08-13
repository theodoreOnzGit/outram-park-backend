//! # Graphite / moderator reactivity feedback — its own channel
//!
//! The graphite moderator temperature of a pebble-bed HTGR, carried as an
//! independent state with its own thermal inertia and its own reactivity
//! coefficient.
//!
//! ## Why this is a separate channel and must stay one
//!
//! It is tempting to lump moderator feedback into the fuel Doppler channel —
//! one temperature, one coefficient, one term. For an HTR-10 that destroys the
//! physics that defines the reactor.
//!
//! The fuel and the graphite respond on completely different timescales. A UO2
//! kernel is micrometres across and follows a power change essentially
//! instantly; the graphite is the *bulk of the core mass* — pebble matrix,
//! pebble shells, dummy balls and reflector — and takes minutes to change
//! temperature. HTR-10's self-limiting response to a loss of flow is exactly
//! that separation: prompt negative Doppler arrests the excursion, then the
//! slow, large-mass graphite channel governs where the core settles and how
//! long it takes to get there. Collapse the two into one temperature and one
//! coefficient, and the model loses the long time constant altogether — it
//! will reach the right final state on entirely the wrong timescale, which is
//! the timescale a passive-safety argument is made on.
//!
//! This is recorded as bead **`op-jyyp.6`** and in
//! `docs/reactor-scoping/htr10-neutronics.md` section 4.4: the graphite
//! channel "must come down rung 3 as its own reactivity coefficient, not be
//! folded into Doppler."
//!
//! ## What this module does and does not do
//!
//! It provides [`GraphiteModeratorFeedback`]: a lumped graphite node with a
//! temperature, a thermal mass, and a linear reactivity coefficient. It can
//!
//! - report the reactivity its current temperature implies
//!   ([`GraphiteModeratorFeedback::reactivity`]),
//! - advance that temperature under a heat balance
//!   ([`GraphiteModeratorFeedback::step`]), and
//! - report the thermal time constant that balance implies
//!   ([`GraphiteModeratorFeedback::thermal_time_constant`]).
//!
//! It does **not** contain point kinetics. This crate's library is deliberately
//! free of [`teh_o_prke`](https://docs.rs/teh-o-prke) — that crate is only an
//! Android-gated *dev*-dependency of `tampines`, used by examples — so the
//! reactivity produced here is a plain dimensionless number for a caller to
//! feed into a kinetics solver. **Wiring this channel into PRKE is deliberate
//! future work** belonging in an example or in `nee_soon`, where the
//! neutronics dependency is appropriate; it is the remaining part of
//! `op-jyyp.6` and is tracked separately. Nothing here should grow a
//! dependency on a neutronics crate.
//!
//! ## The coefficient is the caller's, not this module's
//!
//! **No moderator temperature coefficient is invented here, and none is
//! supplied as a default.** A moderator-only coefficient is an output of a
//! neutronics calculation for a specific core state — loading, burnup, rod
//! position — and inventing one would produce a plausible-looking transient
//! that means nothing.
//!
//! The IAEA benchmark document *does* publish HTR-10 **isothermal** temperature
//! coefficients, and they are provided here as clearly-labelled constants (see
//! [`htr10_isothermal_coefficient_nrg_20_to_120c`] and its siblings). Read
//! their documentation before using them: an isothermal coefficient moves fuel
//! and moderator *together*, so it is the sum of the Doppler and moderator
//! channels, not the moderator channel alone. Substituting one for `alpha_m`
//! double-counts the fuel and defeats the entire purpose of this module. No
//! constructor in this file does that substitution for you.
//!
//! ## Status
//!
//! **NOT VALIDATED.** The ODE and the reactivity relation are verified against
//! analytic limits; no HTR-10 transient has been reproduced, and no coefficient
//! here has been checked against a neutronics calculation. AI-assisted draft
//! pending human review per `RESPONSIBLE_USE.md`.
//!
//! **Belongs here:** the moderator temperature state, its thermal balance, and
//! its reactivity mapping. **Does not belong here:** point kinetics, fuel
//! Doppler, decay heat, or the neutronics that produces the coefficient.

use uom::si::f64::{
    Area, HeatCapacity, HeatTransfer, Mass, Power, Ratio, SpecificHeatCapacity,
    TemperatureCoefficient, TemperatureInterval, ThermalConductance, ThermodynamicTemperature,
    Time,
};
use uom::si::mass::kilogram;
use uom::si::ratio::ratio;
use uom::si::temperature_coefficient::per_kelvin;
use uom::si::thermodynamic_temperature::kelvin;

use tuas_boussinesq_solver::boussinesq_thermophysical_properties::solid_database::nuclear_graphite::nuclear_graphite_specific_heat_capacity_butland_maddison_spline;

use super::triso::{MAX_TEMPERATURE_KELVIN, MIN_TEMPERATURE_KELVIN};
use crate::TampinesError;

/// A reactivity temperature coefficient: reactivity (dimensionless `dk/k`) per
/// kelvin of temperature change, 1/K.
///
/// An alias for `uom`'s [`TemperatureCoefficient`], named for what it means
/// here. Negative for a core with negative temperature feedback, which is the
/// physically desirable and the HTR-10 case. To read one in the reactor-physics
/// unit of **pcm per kelvin**, multiply the per-kelvin value by 1e5:
/// `-7.37e-5 /K` is `-7.37 pcm/K`.
pub type ReactivityTemperatureCoefficient = TemperatureCoefficient;

/// A lumped graphite moderator node: one temperature, one thermal mass, one
/// linear reactivity coefficient.
///
/// Plain data with methods — no trait objects, no interior mutability, no
/// lifetimes, per the workspace Rust design rules. The temperature is the only
/// mutable state; [`GraphiteModeratorFeedback::step`] advances it and
/// everything else is a pure function of it.
///
/// Construct with [`GraphiteModeratorFeedback::new`], which requires the
/// coefficient explicitly — see the module documentation for why no default is
/// offered.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GraphiteModeratorFeedback {
    /// Current bulk temperature of the graphite moderator, kelvin. This is the
    /// state variable: a single lumped temperature standing for the whole
    /// graphite mass, which is an approximation whose validity rests on the
    /// graphite being far more conductive and far more massive than the
    /// gradients within it are steep.
    pub moderator_temperature: ThermodynamicTemperature,
    /// Reference temperature at which this channel contributes exactly zero
    /// reactivity, kelvin. Usually the temperature at which the core's `k_eff`
    /// was evaluated — a critical steady state, say — so that the channel
    /// expresses a *departure* from that state.
    pub reference_temperature: ThermodynamicTemperature,
    /// Moderator temperature coefficient of reactivity, `dk/k` per kelvin.
    /// **Caller-supplied; no default exists.** See the module documentation.
    pub temperature_coefficient: ReactivityTemperatureCoefficient,
    /// Mass of graphite this node represents, kilograms. Sets the thermal
    /// inertia, and therefore the time constant that is the whole point of
    /// keeping this channel separate from fuel Doppler.
    pub graphite_mass: Mass,
}

impl GraphiteModeratorFeedback {
    /// Builds a graphite moderator feedback channel.
    ///
    /// - `moderator_temperature` — initial bulk graphite temperature, kelvin;
    ///   must lie in the 300 K to 2000 K window of the graphite property
    ///   correlations.
    /// - `reference_temperature` — the temperature at which this channel
    ///   contributes zero reactivity, kelvin; same window.
    /// - `temperature_coefficient` — `dk/k` per kelvin. **This must come from
    ///   the caller's neutronics.** It is not defaulted, not guessed, and not
    ///   substituted from an isothermal coefficient; see the module
    ///   documentation and [`htr10_isothermal_coefficient_nrg_20_to_120c`] for
    ///   why the published isothermal figures are *not* this quantity. A
    ///   positive value is accepted — some cores genuinely have one over part
    ///   of their range — but it is physically the dangerous sign, so it is
    ///   the caller's responsibility to mean it.
    /// - `graphite_mass` — kilograms; must be strictly positive.
    ///
    /// Returns [`TampinesError::InvalidInput`] for a non-positive mass, a
    /// non-finite coefficient, or a temperature outside 300 K to 2000 K.
    pub fn new(
        moderator_temperature: ThermodynamicTemperature,
        reference_temperature: ThermodynamicTemperature,
        temperature_coefficient: ReactivityTemperatureCoefficient,
        graphite_mass: Mass,
    ) -> Result<Self, TampinesError> {
        for (name, temperature) in [
            ("moderator", moderator_temperature),
            ("reference", reference_temperature),
        ] {
            let value = temperature.get::<kelvin>();
            if !(MIN_TEMPERATURE_KELVIN..=MAX_TEMPERATURE_KELVIN).contains(&value) {
                return Err(TampinesError::InvalidInput(format!(
                    "{name} temperature {value} K is outside the graphite property \
                     range {MIN_TEMPERATURE_KELVIN} K to {MAX_TEMPERATURE_KELVIN} K"
                )));
            }
        }

        if graphite_mass.get::<kilogram>() <= 0.0 {
            return Err(TampinesError::InvalidInput(format!(
                "graphite mass must be strictly positive, got {} kg",
                graphite_mass.get::<kilogram>()
            )));
        }

        if !temperature_coefficient.get::<per_kelvin>().is_finite() {
            return Err(TampinesError::InvalidInput(format!(
                "moderator temperature coefficient must be finite, got {} 1/K",
                temperature_coefficient.get::<per_kelvin>()
            )));
        }

        Ok(Self {
            moderator_temperature,
            reference_temperature,
            temperature_coefficient,
            graphite_mass,
        })
    }

    /// Temperature of the graphite above its reference, kelvin — positive when
    /// the moderator is hotter than the state the coefficient was defined at.
    pub fn temperature_excursion(&self) -> TemperatureInterval {
        super::temperature_difference(self.moderator_temperature, self.reference_temperature)
    }

    /// Reactivity contributed by this channel, dimensionless `dk/k`:
    ///
    /// `rho_m = alpha_m (T_m - T_ref)`
    ///
    /// Exactly zero when the moderator sits at its reference temperature, and
    /// linear in the excursion either side of it. The linearity is an explicit
    /// modelling choice: a real moderator coefficient varies with temperature,
    /// and a caller who needs that should re-evaluate
    /// [`Self::temperature_coefficient`] between steps rather than expecting
    /// this method to do it.
    ///
    /// This value is **only** the moderator channel. It must be *added* to a
    /// separately computed fuel Doppler reactivity, not used in place of one.
    pub fn reactivity(&self) -> Ratio {
        self.temperature_coefficient * self.temperature_excursion()
    }

    /// Reactivity contributed by this channel in **pcm** (per cent mille,
    /// 1e-5 `dk/k`) — the unit reactor physics is usually quoted in.
    ///
    /// Simply `1e5` times [`Self::reactivity`]; provided because reading a
    /// reactivity of `-3.7e-3` as "-370 pcm" mentally is exactly where sign
    /// and magnitude errors creep in.
    pub fn reactivity_pcm(&self) -> f64 {
        1.0e5 * self.reactivity().get::<ratio>()
    }

    /// Specific heat capacity of the graphite at its current temperature,
    /// J/(kg K).
    ///
    /// Consumed from [`tuas_boussinesq_solver`]'s nuclear-graphite database
    /// (a cubic spline through the Butland & Maddison table, J. Nucl. Mater.
    /// 49 (1973/74) 45-56, via the CC-BY-4.0 Virtual Test Bed HTTF deck)
    /// rather than hardcoded. Graphite `cp` is strongly temperature-dependent
    /// — it nearly triples between 300 K and 2000 K — so treating it as
    /// constant across a transient would be a real error, not a rounding one.
    ///
    /// Valid range: 300 K to 2000 K; outside it, returns
    /// [`TampinesError::InvalidInput`].
    pub fn specific_heat_capacity(&self) -> Result<SpecificHeatCapacity, TampinesError> {
        nuclear_graphite_specific_heat_capacity_butland_maddison_spline(self.moderator_temperature)
            .map_err(|error| {
                TampinesError::InvalidInput(format!(
                    "TUAS nuclear-graphite cp rejected temperature {} K: {error:?}",
                    self.moderator_temperature.get::<kelvin>()
                ))
            })
    }

    /// Total heat capacity of the graphite node, `m cp`, J/K — the thermal
    /// inertia that gives this channel its long time constant.
    ///
    /// Evaluated at the current moderator temperature; see
    /// [`Self::specific_heat_capacity`] for the property source and its valid
    /// range.
    pub fn thermal_capacity(&self) -> Result<HeatCapacity, TampinesError> {
        Ok(self.graphite_mass * self.specific_heat_capacity()?)
    }

    /// Advances the moderator temperature by one explicit timestep under a
    /// lumped heat balance:
    ///
    /// `m cp dT_m/dt = Q_in - Q_out`
    ///
    /// integrated as `T_m <- T_m + (Q_in - Q_out) dt / (m cp)`, with `cp`
    /// evaluated at the temperature at the *start* of the step.
    ///
    /// **Scheme and its consequences.** This is forward (explicit) Euler. It
    /// is first-order accurate, so the timestep must be small against the
    /// node's thermal time constant ([`Self::thermal_time_constant`]) for the
    /// result to be meaningful, and it is conditionally stable — a timestep
    /// beyond `2 tau` in a negatively-fed-back balance will oscillate and
    /// diverge. Nothing here clamps or limits the step: a caller choosing
    /// `dt > tau` gets the wrong answer visibly rather than a silently damped
    /// one. For the graphite mass of a real core `tau` is of order minutes, so
    /// second-scale steps are comfortably accurate.
    ///
    /// `heat_in` and `heat_out` are both powers in watts — typically fission
    /// and decay power deposited in the graphite, and heat removed to the
    /// coolant. `timestep` must be strictly positive.
    ///
    /// Returns the new moderator temperature, and updates
    /// [`Self::moderator_temperature`]. Returns
    /// [`TampinesError::InvalidInput`] for a non-positive timestep or if the
    /// property lookup fails, and [`TampinesError::Unphysical`] if the step
    /// would take the graphite outside the 300 K to 2000 K correlation range —
    /// reported rather than clamped, because a clamped temperature produces a
    /// plausible-looking transient that is wrong.
    pub fn step(
        &mut self,
        heat_in: Power,
        heat_out: Power,
        timestep: Time,
    ) -> Result<ThermodynamicTemperature, TampinesError> {
        if !(timestep.value > 0.0) || !timestep.value.is_finite() {
            return Err(TampinesError::InvalidInput(format!(
                "timestep must be strictly positive and finite, got {} s",
                timestep.value
            )));
        }

        let thermal_capacity = self.thermal_capacity()?;
        let net_power = heat_in - heat_out;
        let temperature_change: TemperatureInterval = net_power * timestep / thermal_capacity;

        let updated = self.moderator_temperature + temperature_change;
        let updated_kelvin = updated.get::<kelvin>();

        if !(MIN_TEMPERATURE_KELVIN..=MAX_TEMPERATURE_KELVIN).contains(&updated_kelvin) {
            return Err(TampinesError::Unphysical(format!(
                "graphite moderator temperature stepped to {updated_kelvin} K, \
                 outside the property range {MIN_TEMPERATURE_KELVIN} K to \
                 {MAX_TEMPERATURE_KELVIN} K; net power was {} W over {} s",
                net_power.value, timestep.value
            )));
        }

        self.moderator_temperature = updated;
        Ok(updated)
    }

    /// Thermal time constant of the graphite node against a given heat-removal
    /// conductance, `tau = m cp / (h A)`, seconds.
    ///
    /// This is the e-folding time of a temperature excursion when the heat
    /// removed is proportional to the excursion, `Q_out = h A (T_m - T_sink)`:
    /// the lumped balance then reads `dT/dt = -(T - T_sink)/tau`. It is the
    /// number that makes the graphite channel worth separating — for a
    /// core-sized graphite mass it runs to minutes, against the effectively
    /// instantaneous fuel Doppler response.
    ///
    /// `conductance` is the heat-removal conductance in W/K. It must be
    /// strictly positive; a zero conductance is an adiabatic node with no
    /// finite time constant, and returns
    /// [`TampinesError::InvalidInput`] rather than an infinity.
    pub fn thermal_time_constant(
        &self,
        conductance: ThermalConductance,
    ) -> Result<Time, TampinesError> {
        if !(conductance.value > 0.0) || !conductance.value.is_finite() {
            return Err(TampinesError::InvalidInput(format!(
                "heat-removal conductance must be strictly positive and finite, \
                 got {} W/K",
                conductance.value
            )));
        }

        Ok(self.thermal_capacity()? / conductance)
    }

    /// Thermal time constant against a heat transfer coefficient acting over
    /// an area, `tau = m cp / (h A)`, seconds — the same quantity as
    /// [`Self::thermal_time_constant`], built from the two factors a
    /// convective closure naturally produces.
    ///
    /// `heat_transfer_coefficient` is in W/(m^2 K) — for a pebble bed, what
    /// [`super::cht::PackedBedConvection::heat_transfer_coefficient`] returns —
    /// and `area` is the heat transfer area in m^2. Both must be strictly
    /// positive.
    pub fn thermal_time_constant_from_coefficient(
        &self,
        heat_transfer_coefficient: HeatTransfer,
        area: Area,
    ) -> Result<Time, TampinesError> {
        if !(heat_transfer_coefficient.value > 0.0) || !(area.value > 0.0) {
            return Err(TampinesError::InvalidInput(format!(
                "heat transfer coefficient ({} W/(m^2 K)) and area ({} m^2) must \
                 both be strictly positive",
                heat_transfer_coefficient.value, area.value
            )));
        }

        self.thermal_time_constant(heat_transfer_coefficient * area)
    }
}

/// HTR-10 **isothermal** temperature coefficient of reactivity over
/// 20-120 degrees Celsius as calculated by **NRG**: -7.37e-5 per degree
/// (equivalently -7.37 pcm per kelvin).
///
/// Source: IAEA-TECDOC-1382 part 2, Chapter 4, Table 4-33 (Open tier;
/// catalogued at
/// `crates/kovan-literature/open/reports/iaea-tecdoc-1382-part2.pdf`). The
/// document tabulates `delta-k/k` per degree Celsius; a coefficient *per
/// degree Celsius* and *per kelvin* are numerically identical, since only the
/// size of the degree matters.
///
/// # This is NOT a moderator-only coefficient
///
/// An **isothermal** coefficient is measured or calculated by changing the
/// temperature of the *entire core at once* — fuel, moderator and reflector
/// together. It is therefore the **sum** of the fuel Doppler channel and the
/// moderator channel (and everything else that moves with temperature).
///
/// Using it as [`GraphiteModeratorFeedback::temperature_coefficient`] would
/// count the fuel's contribution twice — once in whatever Doppler channel the
/// caller already has, and again here — and would give the graphite the fuel's
/// prompt feedback on the graphite's slow timescale. That is precisely the
/// error this module exists to prevent.
///
/// A genuine moderator-only coefficient must come from a neutronics
/// calculation that perturbs the moderator temperature alone. These constants
/// are provided for **validation of a whole-core isothermal calculation**
/// (compare your model's total isothermal coefficient against this number),
/// not as a stand-in for `alpha_m`.
pub fn htr10_isothermal_coefficient_nrg_20_to_120c() -> ReactivityTemperatureCoefficient {
    ReactivityTemperatureCoefficient::new::<per_kelvin>(-7.37e-5)
}

/// HTR-10 **isothermal** temperature coefficient over 200-250 degrees Celsius
/// as calculated by **NRG**: -8.05e-5 per degree.
///
/// Same source, same caveat, as
/// [`htr10_isothermal_coefficient_nrg_20_to_120c`] — read that function's
/// documentation before using this value. Note the coefficient becomes more
/// negative with temperature, which is the expected and desirable trend.
pub fn htr10_isothermal_coefficient_nrg_200_to_250c() -> ReactivityTemperatureCoefficient {
    ReactivityTemperatureCoefficient::new::<per_kelvin>(-8.05e-5)
}

/// HTR-10 **isothermal** temperature coefficient over 20-120 degrees Celsius
/// as calculated by **INET with VSOP**: -7.49e-5 per degree.
///
/// Same source, same caveat, as
/// [`htr10_isothermal_coefficient_nrg_20_to_120c`]. INET's figure sits 1.6%
/// below NRG's over the same interval — a useful sense of the spread between
/// independent calculations of the same quantity, and of how much precision it
/// is reasonable to claim.
pub fn htr10_isothermal_coefficient_inet_20_to_120c() -> ReactivityTemperatureCoefficient {
    ReactivityTemperatureCoefficient::new::<per_kelvin>(-7.49e-5)
}

/// HTR-10 **isothermal** temperature coefficient over 120-250 degrees Celsius
/// as calculated by **INET with VSOP**: -9.15e-5 per degree.
///
/// Same source, same caveat, as
/// [`htr10_isothermal_coefficient_nrg_20_to_120c`]. Note this covers a
/// different interval from the NRG 200-250 C figure, so the two are not
/// directly comparable.
pub fn htr10_isothermal_coefficient_inet_120_to_250c() -> ReactivityTemperatureCoefficient {
    ReactivityTemperatureCoefficient::new::<per_kelvin>(-9.15e-5)
}

#[cfg(test)]
mod tests {
    use super::*;
    use uom::si::area::square_meter;
    use uom::si::power::watt;
    use uom::si::specific_heat_capacity::joule_per_kilogram_kelvin;
    use uom::si::thermal_conductance::watt_per_kelvin;
    use uom::si::time::second;

    /// Asserts that `measured` matches `expected` to within `max_relative`
    /// relative error. See the identical macro in [`super::super::triso`] for
    /// why `tampines` does not use the `approx` crate here.
    macro_rules! assert_relative_eq {
        ($expected:expr, $measured:expr, max_relative = $tolerance:expr) => {{
            let expected: f64 = $expected;
            let measured: f64 = $measured;
            let relative_error = if expected == 0.0 {
                measured.abs()
            } else {
                ((measured - expected) / expected).abs()
            };
            assert!(
                relative_error < $tolerance,
                "expected {expected}, measured {measured}, relative error \
                 {relative_error:e} exceeds {}",
                $tolerance
            );
        }};
    }

    /// Builds a test channel: 5129 kg of graphite (the HTR-10 pebble-graphite
    /// inventory derived in [`graphite_thermal_time_constant_is_long`]) at
    /// 900 K, referenced to 900 K, with a caller-supplied coefficient.
    fn test_channel(coefficient_per_kelvin: f64) -> GraphiteModeratorFeedback {
        GraphiteModeratorFeedback::new(
            ThermodynamicTemperature::new::<kelvin>(900.0),
            ThermodynamicTemperature::new::<kelvin>(900.0),
            ReactivityTemperatureCoefficient::new::<per_kelvin>(coefficient_per_kelvin),
            Mass::new::<kilogram>(5129.0),
        )
        .unwrap()
    }

    /// V&V test: reactivity is exactly zero at the reference temperature,
    /// linear in the excursion, and carries the sign of the coefficient.
    ///
    /// **Methodology:** with a coefficient of -5.0e-5 /K and a reference of
    /// 900 K, require (a) exactly zero reactivity at 900 K — bitwise, since
    /// the excursion is exactly zero and the product must be too; (b) negative
    /// reactivity above the reference and positive below it, the physically
    /// stabilising sign for a negative coefficient; (c) exact linearity, i.e.
    /// the reactivity at excursions of 10, 20, 50 and 100 K equals
    /// `alpha * dT` to 1e-12 relative, and doubling the excursion exactly
    /// doubles the reactivity; and (d) that
    /// [`GraphiteModeratorFeedback::reactivity_pcm`] is 1e5 times
    /// [`GraphiteModeratorFeedback::reactivity`].
    ///
    /// **Results (2026-08-11):** reactivity at the reference measured zero
    /// (printed as `-0`, the negative zero produced by a negative coefficient
    /// times an exactly zero excursion; `-0.0 == 0.0` holds, so the bitwise
    /// equality assertion passes). At +100 K the reactivity measured
    /// **-500 pcm** and at -100 K **+500 pcm**, equal and opposite to within
    /// 1e-12 relative. Across excursions of 10/20/50/100 K the reactivities
    /// were **-50, -100, -250 and -500 pcm**, each matching `1e5 alpha dT` to
    /// 1e-12 relative — exactly proportional, with the 100 K value exactly
    /// twice the 50 K value.
    ///
    /// **Interpretation:** the channel behaves as the linear map it claims to
    /// be, with no offset and no hidden clamping. Algebraic verification only;
    /// the *value* of the coefficient is the caller's and is not tested here,
    /// because this module deliberately supplies none.
    #[test]
    fn reactivity_is_zero_at_reference_and_linear_in_the_excursion() {
        let mut channel = test_channel(-5.0e-5);

        // (a) exactly zero at the reference
        println!(
            "at reference: reactivity {}, {} pcm",
            channel.reactivity().get::<ratio>(),
            channel.reactivity_pcm()
        );
        assert_eq!(channel.reactivity().get::<ratio>(), 0.0);
        assert_eq!(channel.reactivity_pcm(), 0.0);

        // (b) sign follows the excursion
        channel.moderator_temperature = ThermodynamicTemperature::new::<kelvin>(1000.0);
        let hot = channel.reactivity_pcm();
        channel.moderator_temperature = ThermodynamicTemperature::new::<kelvin>(800.0);
        let cold = channel.reactivity_pcm();
        println!("at +100 K: {hot} pcm; at -100 K: {cold} pcm");
        assert!(
            hot < 0.0,
            "a negative coefficient must give negative \
            reactivity when the moderator heats up"
        );
        assert!(cold > 0.0);
        assert_relative_eq!(-hot, cold, max_relative = 1e-12);

        // (c) exact linearity
        let mut previous_pcm = 0.0;
        for excursion in [10.0, 20.0, 50.0, 100.0] {
            channel.moderator_temperature =
                ThermodynamicTemperature::new::<kelvin>(900.0 + excursion);
            let measured_pcm = channel.reactivity_pcm();
            let hand_pcm = 1.0e5 * -5.0e-5 * excursion;
            println!("excursion +{excursion} K: {measured_pcm} pcm (hand {hand_pcm} pcm)");
            assert_relative_eq!(hand_pcm, measured_pcm, max_relative = 1e-12);
            previous_pcm = measured_pcm;
        }
        // the 100 K value is exactly twice the 50 K value
        channel.moderator_temperature = ThermodynamicTemperature::new::<kelvin>(950.0);
        let at_fifty = channel.reactivity_pcm();
        assert_relative_eq!(2.0 * at_fifty, previous_pcm, max_relative = 1e-12);

        // (d) pcm is 1e5 times the dimensionless reactivity
        assert_relative_eq!(
            1.0e5 * channel.reactivity().get::<ratio>(),
            channel.reactivity_pcm(),
            max_relative = 1e-12
        );
    }

    /// V&V test: the thermal time constant `m cp / (h A)` emerges from the ODE
    /// itself, rather than only being asserted algebraically.
    ///
    /// **Methodology.** Set up a graphite node cooled by a conductance
    /// proportional to its excursion, `Q_out = C (T_m - T_sink)` with
    /// `C = 1.0e5 W/K` and `T_sink = 900 K`, and no heat in. The lumped balance
    /// is then `dT/dt = -(T - T_sink) / tau` with `tau = m cp / C`, whose
    /// solution decays exponentially. Start the node 50 K above the sink,
    /// integrate with [`GraphiteModeratorFeedback::step`] at
    /// `dt = tau / 2000`, and **measure** the elapsed time at which the
    /// excursion first falls to `1/e` of its initial value, by linear
    /// interpolation across the bracketing step. Compare that measured
    /// e-folding time against `tau` from
    /// [`GraphiteModeratorFeedback::thermal_time_constant`]. Pass criterion:
    /// within 1%, which forward Euler at `dt = tau/2000` and a mildly
    /// temperature-dependent `cp` must comfortably meet.
    ///
    /// **Results (2026-08-11):** with 5129 kg of graphite at 950 K,
    /// `cp` = 1731.0962873030599 J/(kg K), the analytic time constant measured
    /// **tau = 88.78792857577395 s** (`m cp / C` = 5129 * 1731.096 / 1.0e5).
    /// Integrating from a 50 K excursion at `dt` = 0.04439396428788697 s, the
    /// excursion reached `1/e` of its initial value at a **measured
    /// 88.16767582118588 s** — a relative difference of **0.6986%** from
    /// `tau`, below the 1% criterion. The measured time is *shorter* than
    /// `tau`, which is the direction the temperature dependence of `cp`
    /// predicts: `cp` falls from 1731.0962873030599 to 1710.8489025073623
    /// J/(kg K) (a **1.17%** drop) as the node cools through the excursion, so
    /// the node's true heat capacity over the decay is below the value at the
    /// starting temperature that `tau` was formed from. The residual's sign
    /// and magnitude are both consistent with that being the dominant source,
    /// with forward Euler's own first-order error a smaller contribution at
    /// this step size.
    ///
    /// **Interpretation:** the time constant is a *property of the integrated
    /// ODE*, not merely of the algebra, and it comes out at the expected value.
    /// Nearly 90 seconds for this conductance is the whole argument of this
    /// module: a fuel Doppler channel responds essentially instantly, so
    /// lumping the two would replace a 90-second graphite response with an
    /// instantaneous one. Verification against an analytic solution; not a
    /// validation against any HTR-10 measurement.
    #[test]
    fn thermal_time_constant_emerges_from_the_integrated_ode() {
        let sink = ThermodynamicTemperature::new::<kelvin>(900.0);
        let initial_excursion = 50.0;
        let conductance = ThermalConductance::new::<watt_per_kelvin>(1.0e5);

        let mut channel = GraphiteModeratorFeedback::new(
            ThermodynamicTemperature::new::<kelvin>(900.0 + initial_excursion),
            sink,
            ReactivityTemperatureCoefficient::new::<per_kelvin>(-5.0e-5),
            Mass::new::<kilogram>(5129.0),
        )
        .unwrap();

        let initial_cp = channel
            .specific_heat_capacity()
            .unwrap()
            .get::<joule_per_kilogram_kelvin>();
        let tau = channel.thermal_time_constant(conductance).unwrap();
        let tau_seconds = tau.get::<second>();
        println!("at 950 K: cp = {initial_cp} J/(kg K), tau = m cp / C = {tau_seconds} s");
        assert_relative_eq!(
            5129.0 * initial_cp / 1.0e5,
            tau_seconds,
            max_relative = 1e-12
        );

        let timestep = Time::new::<second>(tau_seconds / 2000.0);
        let target_excursion = initial_excursion / std::f64::consts::E;

        let mut elapsed = 0.0;
        let mut previous_excursion = initial_excursion;
        let mut measured_e_folding_time = f64::NAN;

        for _ in 0..20_000 {
            let excursion = channel.temperature_excursion().value;
            let heat_out = Power::new::<watt>(1.0e5 * excursion);
            channel
                .step(Power::new::<watt>(0.0), heat_out, timestep)
                .unwrap();
            elapsed += timestep.get::<second>();

            let new_excursion = channel.temperature_excursion().value;
            if new_excursion <= target_excursion && previous_excursion > target_excursion {
                // linear interpolation across the bracketing step
                let fraction =
                    (previous_excursion - target_excursion) / (previous_excursion - new_excursion);
                measured_e_folding_time = elapsed - timestep.get::<second>() * (1.0 - fraction);
                break;
            }
            previous_excursion = new_excursion;
        }

        let final_cp = channel
            .specific_heat_capacity()
            .unwrap()
            .get::<joule_per_kilogram_kelvin>();
        println!(
            "dt = {} s; measured e-folding time {measured_e_folding_time} s against \
             tau = {tau_seconds} s (relative difference {:.4}%); cp fell from \
             {initial_cp} to {final_cp} J/(kg K) ({:.2}%)",
            timestep.get::<second>(),
            100.0 * (measured_e_folding_time - tau_seconds).abs() / tau_seconds,
            100.0 * (initial_cp - final_cp) / initial_cp
        );

        assert!(
            measured_e_folding_time.is_finite(),
            "the excursion never reached 1/e within the integration window"
        );
        assert_relative_eq!(tau_seconds, measured_e_folding_time, max_relative = 1e-2);
    }

    /// V&V test: the lumped balance conserves energy, and zero net power
    /// leaves the temperature exactly unchanged.
    ///
    /// **Methodology:** (a) step the node with `Q_in = Q_out = 1.0e6 W` for 100
    /// steps of 1 s and require the temperature to be *bitwise* unchanged — a
    /// zero net power multiplied by any timestep is exactly zero, so any drift
    /// would indicate an accumulation bug. (b) Step with a constant net power
    /// of 5.0e5 W for 600 s in 1 s steps and check the total temperature rise
    /// against `E / (m cp)` with `cp` at the mean of the start and end
    /// temperatures, requiring agreement within 0.5% (the residual being the
    /// curvature of `cp` over the interval).
    ///
    /// **Results (2026-08-11):** (a) after 100 balanced steps the temperature
    /// was **exactly 900 K**, unchanged bit for bit. (b) 5.0e5 W for 600 s
    /// deposited 3.0e8 J into 5129 kg of graphite initially at 900 K, raising
    /// it to **934.207101551191 K**, a rise of **34.207101551191045 K**; the
    /// energy check `E / (m cp_mean)` with `cp_mean` = 1709.9978184904965
    /// J/(kg K) at the mean temperature 917.1035507755955 K gives
    /// **34.20526814290188 K**, agreeing to **5.4e-5 relative**.
    ///
    /// **Interpretation:** the integrator deposits the energy it is given, and
    /// the small residual is entirely the temperature dependence of `cp`,
    /// which the mid-point evaluation only approximately captures — not a
    /// conservation error. Verification against an energy balance; no physical
    /// data involved.
    #[test]
    fn the_lumped_balance_conserves_energy() {
        // (a) balanced power leaves the temperature exactly unchanged
        let mut balanced = test_channel(-5.0e-5);
        for _ in 0..100 {
            balanced
                .step(
                    Power::new::<watt>(1.0e6),
                    Power::new::<watt>(1.0e6),
                    Time::new::<second>(1.0),
                )
                .unwrap();
        }
        println!(
            "after 100 balanced steps: {} K",
            balanced.moderator_temperature.get::<kelvin>()
        );
        assert_eq!(balanced.moderator_temperature.get::<kelvin>(), 900.0);

        // (b) constant net power deposits the right energy
        let mut heated = test_channel(-5.0e-5);
        let net_power = 5.0e5;
        let duration = 600.0;
        for _ in 0..(duration as usize) {
            heated
                .step(
                    Power::new::<watt>(net_power),
                    Power::new::<watt>(0.0),
                    Time::new::<second>(1.0),
                )
                .unwrap();
        }

        let final_temperature = heated.moderator_temperature.get::<kelvin>();
        let rise = final_temperature - 900.0;
        let mean_temperature = 0.5 * (900.0 + final_temperature);
        let mut at_mean = heated;
        at_mean.moderator_temperature = ThermodynamicTemperature::new::<kelvin>(mean_temperature);
        let mean_cp = at_mean
            .specific_heat_capacity()
            .unwrap()
            .get::<joule_per_kilogram_kelvin>();
        let energy_rise = net_power * duration / (5129.0 * mean_cp);

        println!(
            "{net_power} W for {duration} s: final {final_temperature} K \
             (rise {rise} K); energy check E/(m cp_mean) = {energy_rise} K with \
             cp_mean = {mean_cp} J/(kg K) at {mean_temperature} K"
        );
        assert_relative_eq!(energy_rise, rise, max_relative = 5e-3);
    }

    /// V&V test: the graphite time constant of an HTR-10-sized pebble
    /// inventory, composed with the level-3 convective closure.
    ///
    /// **Methodology.** Derive the pebble-graphite mass of an HTR-10 core from
    /// published data alone: 27 000 fuel elements (IAEA-TECDOC-1382 part 2,
    /// Chapter 4), each a 6.0 cm sphere of volume 113.097 cm^3 less the
    /// 3.2887 cm^3 the 8335 coated particles occupy (from
    /// [`super::super::pebble::Pebble::triso_volume_fraction`]), at a graphite
    /// density of 1.73 g/cm^3. Take the bed's heat-removal conductance from
    /// [`super::super::cht::PackedBedConvection::volumetric_heat_transfer_coefficient`]
    /// at the stated operating point (Re = 1000, Pr = 0.71, k_f = 0.30
    /// W/(m K)) multiplied by the published 5.0 m^3 core volume. Then measure
    /// [`GraphiteModeratorFeedback::thermal_time_constant`] at 900 K. Pass
    /// criterion: the time constant is of order a minute or more — long enough
    /// that lumping it with prompt fuel feedback would be a qualitative error,
    /// which is the claim this module rests on.
    ///
    /// **Results (2026-08-11):** derived pebble-graphite inventory
    /// **5129.159899383655 kg** (27 000 x 109.8086041400911 cm^3 x
    /// 1.73 g/cm^3). Bed conductance 19494.78192367869 W/(m^3 K) x 5.0 m^3 =
    /// **97473.90961839345 W/K**. Graphite `cp` at 900 K =
    /// 1698.4000000000026 J/(kg K), so `m cp` = **8711365.173113214 J/K** and
    /// the thermal time constant measures **89.37125028859383 s**.
    ///
    /// **Interpretation:** about **90 seconds** for the fuel pebbles alone,
    /// and the pebbles are only part of the graphite in an HTR-10 — the side
    /// reflector is 1 m thick and dwarfs the bed, so the *core's* graphite time
    /// constant is longer still. Against a fuel Doppler response that is
    /// effectively instantaneous, this is a difference of several orders of
    /// magnitude in timescale, and it is why `op-jyyp.6` requires the two to be
    /// separate reactivity channels. **This is a derived figure, not a
    /// validated one:** it uses a stated, representative operating point rather
    /// than an HTR-10 operating state, treats the whole pebble inventory as one
    /// lumped node, and ignores the reflector entirely.
    #[test]
    fn graphite_thermal_time_constant_is_long() {
        use super::super::cht::PackedBedConvection;
        use super::super::pebble::Pebble;
        use uom::si::thermal_conductivity::watt_per_meter_kelvin;
        use uom::si::volume::cubic_centimeter;

        // derived pebble-graphite inventory
        let pebble = Pebble::htr10();
        let pebble_volume_cm3 = pebble.pebble_volume().get::<cubic_centimeter>();
        let particle_volume_cm3 = pebble.particles_per_pebble
            * pebble.particle.particle_volume().get::<cubic_centimeter>();
        let graphite_volume_cm3 = pebble_volume_cm3 - particle_volume_cm3;
        let graphite_mass_kg = 27_000.0 * graphite_volume_cm3 * 1.73 / 1000.0;
        println!(
            "pebble {pebble_volume_cm3} cm^3 less {particle_volume_cm3} cm^3 of \
             particles = {graphite_volume_cm3} cm^3 of graphite; 27000 pebbles = \
             {graphite_mass_kg} kg"
        );

        // bed conductance at the stated operating point
        let bed = PackedBedConvection::htr10();
        let volumetric = bed
            .volumetric_heat_transfer_coefficient(
                Ratio::new::<ratio>(1000.0),
                Ratio::new::<ratio>(0.71),
                uom::si::f64::ThermalConductivity::new::<watt_per_meter_kelvin>(0.30),
            )
            .unwrap()
            .value;
        let core_volume_m3 = 5.0;
        let conductance = ThermalConductance::new::<watt_per_kelvin>(volumetric * core_volume_m3);
        println!(
            "bed h a_v = {volumetric} W/(m^3 K) over {core_volume_m3} m^3 = {} W/K",
            conductance.get::<watt_per_kelvin>()
        );

        let channel = GraphiteModeratorFeedback::new(
            ThermodynamicTemperature::new::<kelvin>(900.0),
            ThermodynamicTemperature::new::<kelvin>(900.0),
            ReactivityTemperatureCoefficient::new::<per_kelvin>(-5.0e-5),
            Mass::new::<kilogram>(graphite_mass_kg),
        )
        .unwrap();

        let cp = channel
            .specific_heat_capacity()
            .unwrap()
            .get::<joule_per_kilogram_kelvin>();
        let capacity = channel.thermal_capacity().unwrap().value;
        let tau = channel
            .thermal_time_constant(conductance)
            .unwrap()
            .get::<second>();
        println!("cp at 900 K = {cp} J/(kg K); m cp = {capacity} J/K; tau = {tau} s");

        assert!(
            tau > 60.0,
            "the graphite time constant must be of order a minute or more for \
             the separate-channel argument to hold; measured {tau} s"
        );
    }

    /// V&V test: the published HTR-10 isothermal coefficients are transcribed
    /// correctly and are clearly not moderator-only coefficients.
    ///
    /// **Methodology:** check the four constants against the values recorded in
    /// `docs/reactor-scoping/htr10-neutronics.md` section 6.3 from
    /// IAEA-TECDOC-1382 part 2 Table 4-33 — NRG -7.37e-5 (20-120 C) and
    /// -8.05e-5 (200-250 C), INET/VSOP -7.49e-5 (20-120 C) and -9.15e-5
    /// (120-250 C) per degree — to 1e-12 relative. Require all four to be
    /// negative, and record the NRG-INET spread over the common 20-120 C
    /// interval.
    ///
    /// **Results (2026-08-11):** all four constants reproduced their published
    /// values exactly: -7.37 pcm/K, -8.05 pcm/K, -7.49 pcm/K and -9.15 pcm/K.
    /// Over the common 20-120 C interval INET's figure is **1.63%** more
    /// negative than NRG's.
    ///
    /// **Interpretation:** a 1.6% spread between two independent calculations
    /// of the *same* published quantity sets a floor on the precision anyone
    /// should claim from it. More importantly, these are **isothermal**
    /// coefficients — fuel and moderator moved together — so none of them is
    /// the moderator-only `alpha_m` this module's channel needs; they are here
    /// for validating a whole-core isothermal calculation. Transcription check
    /// only.
    #[test]
    fn published_isothermal_coefficients_are_transcribed_correctly() {
        let nrg_low = htr10_isothermal_coefficient_nrg_20_to_120c().get::<per_kelvin>();
        let nrg_high = htr10_isothermal_coefficient_nrg_200_to_250c().get::<per_kelvin>();
        let inet_low = htr10_isothermal_coefficient_inet_20_to_120c().get::<per_kelvin>();
        let inet_high = htr10_isothermal_coefficient_inet_120_to_250c().get::<per_kelvin>();

        println!(
            "NRG 20-120 C {} pcm/K, NRG 200-250 C {} pcm/K, INET 20-120 C {} pcm/K, \
             INET 120-250 C {} pcm/K",
            1.0e5 * nrg_low,
            1.0e5 * nrg_high,
            1.0e5 * inet_low,
            1.0e5 * inet_high
        );

        assert_relative_eq!(-7.37e-5, nrg_low, max_relative = 1e-12);
        assert_relative_eq!(-8.05e-5, nrg_high, max_relative = 1e-12);
        assert_relative_eq!(-7.49e-5, inet_low, max_relative = 1e-12);
        assert_relative_eq!(-9.15e-5, inet_high, max_relative = 1e-12);

        for coefficient in [nrg_low, nrg_high, inet_low, inet_high] {
            assert!(coefficient < 0.0, "all four published figures are negative");
        }

        println!(
            "INET is {:.2}% more negative than NRG over the common 20-120 C interval",
            100.0 * (inet_low - nrg_low) / nrg_low
        );
    }

    /// V&V test: invalid inputs and out-of-range excursions are rejected
    /// rather than clamped.
    ///
    /// **Methodology:** require [`TampinesError::InvalidInput`] from
    /// [`GraphiteModeratorFeedback::new`] for a zero mass and for a
    /// temperature below the 300 K property floor; from
    /// [`GraphiteModeratorFeedback::step`] for a zero and a negative timestep;
    /// and from [`GraphiteModeratorFeedback::thermal_time_constant`] for a zero
    /// conductance. Require [`TampinesError::Unphysical`] from a step whose net
    /// power would carry the graphite past the 2000 K ceiling, and confirm the
    /// node's temperature is left *unchanged* by that failed step rather than
    /// clamped to the ceiling.
    ///
    /// **Results (2026-08-11):** all six invalid inputs returned
    /// `InvalidInput` (zero mass, a 200 K temperature, a zero and a negative
    /// timestep, a zero conductance, and a zero area). A step of 1e12 W for
    /// 100 s returned `Unphysical("graphite moderator temperature stepped to
    /// 11480514.913103431 K, outside the property range 300 K to 2000 K; net
    /// power was 1000000000000 W over 100 s")` and the node was still at
    /// exactly 900 K afterwards.
    ///
    /// **Interpretation:** a failed step is a reported failure, not a silently
    /// clamped state — a clamped temperature would let a diverging transient
    /// look like a converged one, which is the failure mode this crate's V&V
    /// rules exist to prevent. Input-validation check only.
    #[test]
    fn invalid_inputs_are_rejected_and_a_failed_step_does_not_clamp() {
        let coefficient = ReactivityTemperatureCoefficient::new::<per_kelvin>(-5.0e-5);
        let temperature = ThermodynamicTemperature::new::<kelvin>(900.0);

        let zero_mass = GraphiteModeratorFeedback::new(
            temperature,
            temperature,
            coefficient,
            Mass::new::<kilogram>(0.0),
        );
        println!("zero mass: {zero_mass:?}");
        assert!(matches!(zero_mass, Err(TampinesError::InvalidInput(_))));

        let too_cold = GraphiteModeratorFeedback::new(
            ThermodynamicTemperature::new::<kelvin>(200.0),
            temperature,
            coefficient,
            Mass::new::<kilogram>(5129.0),
        );
        println!("below the property floor: {too_cold:?}");
        assert!(matches!(too_cold, Err(TampinesError::InvalidInput(_))));

        let mut channel = test_channel(-5.0e-5);
        for bad_timestep in [0.0, -1.0] {
            let result = channel.step(
                Power::new::<watt>(0.0),
                Power::new::<watt>(0.0),
                Time::new::<second>(bad_timestep),
            );
            println!("timestep {bad_timestep} s: {result:?}");
            assert!(matches!(result, Err(TampinesError::InvalidInput(_))));
        }

        let zero_conductance =
            channel.thermal_time_constant(ThermalConductance::new::<watt_per_kelvin>(0.0));
        println!("zero conductance: {zero_conductance:?}");
        assert!(matches!(
            zero_conductance,
            Err(TampinesError::InvalidInput(_))
        ));

        let bad_area = channel.thermal_time_constant_from_coefficient(
            HeatTransfer::new::<uom::si::heat_transfer::watt_per_square_meter_kelvin>(300.0),
            Area::new::<square_meter>(0.0),
        );
        println!("zero area: {bad_area:?}");
        assert!(matches!(bad_area, Err(TampinesError::InvalidInput(_))));

        // an over-large step is reported, and does not clamp the state
        let runaway = channel.step(
            Power::new::<watt>(1.0e12),
            Power::new::<watt>(0.0),
            Time::new::<second>(100.0),
        );
        println!("runaway step: {runaway:?}");
        assert!(matches!(runaway, Err(TampinesError::Unphysical(_))));
        assert_eq!(channel.moderator_temperature.get::<kelvin>(), 900.0);
    }
}
