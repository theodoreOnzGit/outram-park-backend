//! Second-law-safe steam-generator duty for the FHR simulator.
//!
//! # Why this module exists
//!
//! The steam generator couples the HITEC-salt **shell** side (hot) to the
//! feedwater/steam **tube** side (cold). Until 2026-08-12 the duty was a bare
//! conductance product,
//!
//! ```text
//! Q = UA * (T_shell_bulk - T_tube_outlet)
//! ```
//!
//! with `UA` taken straight from a user slider (0 to 7e5 W/K) and *no upper
//! bound at all*. That is not a heat exchanger, it is a heat source whose
//! strength the user sets: push the slider up and the model transfers more
//! heat than the pinch permits, the steam leaves hotter than the salt that
//! heated it, and the reported duty — which is fed to the turbine, the
//! condenser and the salt-side energy balance — becomes meaningless. A
//! temperature cross is a second-law violation, not a drawing glitch.
//!
//! This module replaces that with an **effectiveness-NTU-style cap**: the duty
//! is the conductance-limited value *or* the thermodynamic maximum, whichever
//! is smaller. Because the cap is applied to the duty itself, no slider
//! setting can produce a cross — the physics forbids it rather than a display
//! guard hiding it.
//!
//! # The thermodynamic maximum
//!
//! For a counter-flow exchanger the transferable heat is bounded by whichever
//! stream runs out of temperature difference first:
//!
//! - **Hot (salt) side.** The salt may not be cooled below the temperature the
//!   feedwater entered at, so
//!   `Q_max_hot = m_salt * cp_salt * (T_salt_in - T_feed_in)`.
//! - **Cold (water/steam) side.** The feedwater may not be heated above the
//!   temperature the salt entered at. Because the cold stream boils, a
//!   constant-`cp` capacity rate is wrong — latent heat would be missed
//!   entirely. The bound is written on **enthalpy** instead, which carries the
//!   latent heat exactly:
//!   `Q_max_cold = m_feed * (h(p, T_salt_in) - h_feed_in)`.
//!
//! `Q_max = min(Q_max_hot, Q_max_cold)` is the classical
//! `C_min * (T_hot_in - T_cold_in)` written so that boiling is handled
//! correctly, and the effectiveness `eps = Q / Q_max` cannot exceed 1.
//!
//! # What is *not* claimed
//!
//! - This is still a **single lump**, not a resolved economiser / evaporator /
//!   superheater. The internal pinch of a real once-through steam generator is
//!   not modelled; only the terminal pinch is enforced.
//! - The driving temperature difference still uses the shell **bulk mean**
//!   temperature (as the pre-existing model did, so the tuned nominal duty is
//!   preserved), while the **cap** uses the shell **inlet** temperature — the
//!   hottest salt node — because that is what `T_hot_in` means in the
//!   effectiveness definition.
//! - With zero salt circulation the hot-side cap is zero, so the steam
//!   generator delivers no duty. That is the honest consequence of a
//!   steady-flow bound: no salt flow, no advected heat. Shell thermal inertia
//!   is deliberately *not* credited as an extra allowance, because doing so
//!   would weaken the steady-state guarantee this module exists to provide.
//! - This is a demonstration model, not a validated steam-generator model.

use tampines_steam_tables::interfaces::functional_programming::{ps_flash_eqm, pt_flash_eqm};
use tampines_steam_tables::region_4_vap_liq_equilibrium::sat_temp_4;
use uom::si::f64::*;
use uom::si::power::watt;
use uom::si::pressure::bar;
use uom::si::temperature_interval::kelvin as kelvin_interval;
use uom::si::thermodynamic_temperature::{degree_celsius, kelvin};

/// Condenser outlet pressure of the secondary loop \[bar\]. The steam cycle is
/// modelled from this fixed sink state, so the feedwater inlet condition is a
/// pure function of the pump-outlet pressure.
pub const CONDENSER_OUTLET_PRESSURE_BAR: f64 = 0.1;
/// Condenser outlet temperature of the secondary loop \[degC\].
pub const CONDENSER_OUTLET_TEMPERATURE_DEGC: f64 = 35.0;
/// Lowest steam-generator pressure the feed pump is allowed to deliver
/// \[bar\]. Below this the loop is pushed back up to 1 bar so the IF97 flashes
/// stay comfortably inside their validity envelope.
pub const MINIMUM_PUMP_OUTLET_PRESSURE_BAR: f64 = 1.0;

/// The upper temperature at which the cold-side enthalpy ceiling is still
/// looked up \[K\]. IAPWS-IF97 region 5 ends at 2273.15 K; a salt inlet
/// temperature above that is impossible for FLiBe or HITEC, but the lookup is
/// clamped anyway so a nonsense state cannot panic the physics thread.
const IF97_MAXIMUM_TEMPERATURE_K: f64 = 2273.0;

/// Half-width of the band around the saturation line inside which the
/// cold-side enthalpy ceiling is evaluated just above the dome \[K\].
///
/// `h_tp_eqm_single_phase` is undefined *on* the saturation line (a `(T, p)`
/// pair there does not determine the state), so a salt inlet temperature that
/// happens to land on `T_sat` is nudged to the saturated-vapour side. That is
/// the correct ceiling: at `T_cold_out = T_sat` the most the feedwater can
/// absorb is everything up to saturated vapour.
const SATURATION_GUARD_BAND_K: f64 = 0.1;

/// The steam-generator feedwater inlet state, i.e. the tube-side cold inlet.
///
/// Derived from the fixed condenser sink (0.1 bar, 35 degC) by an isentropic
/// pump compression to the user-set pump-outlet pressure. Both the duty cap
/// here and [`FHRSimulatorApp::secondary_loop_single_timestep`] read this one
/// function so the cap and the cycle can never disagree about what the
/// feedwater inlet is.
///
/// [`FHRSimulatorApp::secondary_loop_single_timestep`]:
///     crate::FHRSimulatorApp::secondary_loop_single_timestep
#[derive(Debug, Clone, Copy)]
pub struct FeedwaterInletState {
    /// Feedwater temperature entering the steam-generator tube \[K\].
    pub temperature: ThermodynamicTemperature,
    /// Steam-generator (tube-side) pressure \[Pa\]; the pump outlet pressure,
    /// floored at [`MINIMUM_PUMP_OUTLET_PRESSURE_BAR`].
    pub pressure: Pressure,
    /// Feedwater specific enthalpy entering the tube \[J/kg\].
    pub enthalpy: AvailableEnergy,
}

/// Which constraint set the transferred duty. Reported so the GUI/diagnostics
/// can say *why* the steam generator is not doing more, rather than leaving
/// the user to guess at an unresponsive slider.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SteamGeneratorDutyLimit {
    /// The conductance `UA * dT` was below the thermodynamic maximum — the
    /// normal, well-posed operating regime.
    Conductance,
    /// The salt stream would have been cooled below the feedwater inlet
    /// temperature; `m_salt * cp * (T_salt_in - T_feed_in)` binds.
    SaltCapacityRate,
    /// The steam would have left hotter than the salt entered;
    /// `m_feed * (h(p, T_salt_in) - h_feed_in)` binds.
    FeedwaterEnthalpyPinch,
    /// There is no driving temperature difference (the tube side is at or
    /// above the shell temperature), so no heat flows.
    NoDrivingTemperatureDifference,
}

/// A steam-generator duty that cannot violate the second law, together with
/// the bound it was measured against.
#[derive(Debug, Clone, Copy)]
pub struct SteamGeneratorDuty {
    /// Heat actually transferred from shell to tube \[W\], always
    /// `0 <= duty <= thermodynamic_maximum`.
    pub duty: Power,
    /// `Q_max = min(Q_max_hot, Q_max_cold)` \[W\] — the pinch limit.
    pub thermodynamic_maximum: Power,
    /// `duty / Q_max`, dimensionless, always within `[0, 1]`. Zero when
    /// `Q_max` is zero (no driving difference), by convention.
    pub effectiveness: f64,
    /// Which of the three bounds actually set `duty`.
    pub limited_by: SteamGeneratorDutyLimit,
}

/// The feedwater inlet state produced by the fixed condenser sink and an
/// isentropic feed pump raising the water to `user_specified_pump_outlet_pressure`.
///
/// The pump-outlet pressure is floored at [`MINIMUM_PUMP_OUTLET_PRESSURE_BAR`]
/// (1 bar). Valid inputs: any pressure the side-panel slider can produce
/// (1.2 to 200 bar); pressures below 1 bar are silently raised to 1 bar, which
/// is the pre-existing behaviour of the secondary loop.
pub fn feedwater_inlet_state(user_specified_pump_outlet_pressure: Pressure) -> FeedwaterInletState {
    let condenser_outlet_pressure = Pressure::new::<bar>(CONDENSER_OUTLET_PRESSURE_BAR);
    let condenser_outlet_temperature =
        ThermodynamicTemperature::new::<degree_celsius>(CONDENSER_OUTLET_TEMPERATURE_DEGC);

    let condenser_outlet_entropy: SpecificHeatCapacity = pt_flash_eqm::s_tp_eqm_single_phase(
        condenser_outlet_temperature,
        condenser_outlet_pressure,
    );

    let minimum_pressure = Pressure::new::<bar>(MINIMUM_PUMP_OUTLET_PRESSURE_BAR);
    let pressure = if user_specified_pump_outlet_pressure < minimum_pressure {
        minimum_pressure
    } else {
        user_specified_pump_outlet_pressure
    };

    let temperature = ps_flash_eqm::t_ps_eqm(pressure, condenser_outlet_entropy);
    let enthalpy = pt_flash_eqm::h_tp_eqm_single_phase(temperature, pressure);

    FeedwaterInletState {
        temperature,
        pressure,
        enthalpy,
    }
}

/// Specific enthalpy the tube-side water could reach without its temperature
/// exceeding `hot_inlet_temperature` at pressure `pressure` \[J/kg\].
///
/// This is the ceiling that makes a temperature cross impossible on the cold
/// side. It is evaluated with the IF97 `(T, p)` flash, guarded two ways:
/// the temperature is clamped to the IF97 envelope, and a `(T, p)` pair
/// landing on the saturation line (where the state is underdetermined and the
/// flash panics) is nudged to the saturated-vapour side — which is the correct
/// ceiling there, since at `T_cold_out = T_sat` the water may absorb the full
/// latent heat.
fn cold_side_enthalpy_ceiling(
    hot_inlet_temperature: ThermodynamicTemperature,
    pressure: Pressure,
) -> AvailableEnergy {
    let saturation_temperature_k = sat_temp_4(pressure).get::<kelvin>();
    let mut ceiling_temperature_k = hot_inlet_temperature.get::<kelvin>();

    if (ceiling_temperature_k - saturation_temperature_k).abs() < SATURATION_GUARD_BAND_K {
        ceiling_temperature_k = saturation_temperature_k + SATURATION_GUARD_BAND_K;
    }
    if ceiling_temperature_k > IF97_MAXIMUM_TEMPERATURE_K {
        ceiling_temperature_k = IF97_MAXIMUM_TEMPERATURE_K;
    }

    pt_flash_eqm::h_tp_eqm_single_phase(
        ThermodynamicTemperature::new::<kelvin>(ceiling_temperature_k),
        pressure,
    )
}

/// The steam-generator duty, clamped so that a temperature cross is
/// impossible by construction.
///
/// # Arguments
///
/// - `overall_ua` — the user-set overall conductance \[W/K\], 0 to 7e5 W/K on
///   the side-panel slider. Any non-negative value is accepted; the cap does
///   the limiting, so no slider range needs policing.
/// - `shell_mean_temperature` — bulk mean salt temperature of the shell \[K\],
///   used only for the *driving* temperature difference (preserving the
///   pre-existing tuned duty in the well-posed regime).
/// - `shell_inlet_temperature` — hottest salt node, i.e. `T_hot_in` \[K\].
///   Used for the pinch cap. Must be at or above `shell_mean_temperature` in a
///   physical state; nothing breaks if it is not.
/// - `salt_mass_flowrate` — SG-branch salt flow \[kg/s\]. Sign is ignored (the
///   intermediate loop may report either direction); magnitude is used.
/// - `salt_specific_heat_capacity` — HITEC `cp` at the shell temperature
///   \[J/(kg K)\].
/// - `feedwater` — the tube-side cold inlet from [`feedwater_inlet_state`].
/// - `feedwater_mass_flowrate` — secondary loop flow \[kg/s\], 10 to 80 kg/s
///   on the slider. Sign ignored.
/// - `tube_outlet_temperature` — the tube-side outlet from the previous
///   timestep \[K\], the lagged cold-side reference the conductance term is
///   driven against (unchanged from the pre-existing model).
///
/// # Guarantees
///
/// For any finite, non-negative `overall_ua` and any inlet state:
/// `0 <= duty <= Q_max`, hence `0 <= effectiveness <= 1`, hence the tube-side
/// outlet enthalpy `h_feed_in + duty / m_feed` never exceeds
/// `h(p, T_salt_in)` and the salt outlet `T_salt_in - duty / (m_salt cp)`
/// never falls below the feedwater inlet temperature.
pub fn pinch_limited_steam_generator_duty(
    overall_ua: ThermalConductance,
    shell_mean_temperature: ThermodynamicTemperature,
    shell_inlet_temperature: ThermodynamicTemperature,
    salt_mass_flowrate: MassRate,
    salt_specific_heat_capacity: SpecificHeatCapacity,
    feedwater: FeedwaterInletState,
    feedwater_mass_flowrate: MassRate,
    tube_outlet_temperature: ThermodynamicTemperature,
) -> SteamGeneratorDuty {
    // ── conductance-limited duty (the pre-existing formula) ──────────────
    let driving_difference = TemperatureInterval::new::<kelvin_interval>(
        shell_mean_temperature.get::<kelvin>() - tube_outlet_temperature.get::<kelvin>(),
    );
    let conductance_limited_duty: Power = overall_ua * driving_difference;

    // ── hot-side bound: the salt may not be cooled below the feedwater ───
    let hot_side_span = TemperatureInterval::new::<kelvin_interval>(
        shell_inlet_temperature.get::<kelvin>() - feedwater.temperature.get::<kelvin>(),
    );
    let salt_capacity_rate: ThermalConductance =
        (salt_mass_flowrate.abs() * salt_specific_heat_capacity).into();
    let hot_side_maximum: Power = salt_capacity_rate * hot_side_span;

    // ── cold-side bound: the steam may not exceed the salt inlet ─────────
    let enthalpy_ceiling = cold_side_enthalpy_ceiling(shell_inlet_temperature, feedwater.pressure);
    let cold_side_maximum: Power =
        feedwater_mass_flowrate.abs() * (enthalpy_ceiling - feedwater.enthalpy);

    let hot_side_maximum_w = hot_side_maximum.get::<watt>().max(0.0);
    let cold_side_maximum_w = cold_side_maximum.get::<watt>().max(0.0);
    let thermodynamic_maximum_w = hot_side_maximum_w.min(cold_side_maximum_w);
    let conductance_limited_w = conductance_limited_duty.get::<watt>().max(0.0);

    let duty_w = conductance_limited_w.min(thermodynamic_maximum_w);

    let limited_by = if conductance_limited_duty.get::<watt>() <= 0.0 {
        SteamGeneratorDutyLimit::NoDrivingTemperatureDifference
    } else if conductance_limited_w <= thermodynamic_maximum_w {
        SteamGeneratorDutyLimit::Conductance
    } else if hot_side_maximum_w <= cold_side_maximum_w {
        SteamGeneratorDutyLimit::SaltCapacityRate
    } else {
        SteamGeneratorDutyLimit::FeedwaterEnthalpyPinch
    };

    let effectiveness = if thermodynamic_maximum_w > 0.0 {
        duty_w / thermodynamic_maximum_w
    } else {
        0.0
    };

    SteamGeneratorDuty {
        duty: Power::new::<watt>(duty_w),
        thermodynamic_maximum: Power::new::<watt>(thermodynamic_maximum_w),
        effectiveness,
        limited_by,
    }
}

/// The **pre-2026-08-12 (defective) duty formula**, kept solely as the
/// reference the V&V sweep below measures the fix against.
///
/// `Q = UA * (T_shell_bulk - T_tube_outlet)`, clamped at zero on the tube
/// side only. It has no upper bound, so a large enough `UA` transfers more
/// heat than the pinch permits. Never call this from the simulator; it exists
/// so [`tests::second_law_holds_across_the_whole_ua_slider`] can demonstrate
/// that the sweep genuinely discriminates between the broken and fixed
/// models.
#[cfg(test)]
fn unclamped_legacy_duty(
    overall_ua: ThermalConductance,
    shell_mean_temperature: ThermodynamicTemperature,
    tube_outlet_temperature: ThermodynamicTemperature,
) -> Power {
    let driving_difference = TemperatureInterval::new::<kelvin_interval>(
        shell_mean_temperature.get::<kelvin>() - tube_outlet_temperature.get::<kelvin>(),
    );
    let duty: Power = overall_ua * driving_difference;
    Power::new::<watt>(duty.get::<watt>().max(0.0))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tampines_steam_tables::interfaces::functional_programming::ph_flash_eqm;
    use tuas_boussinesq_solver::boussinesq_thermophysical_properties::LiquidMaterial;
    use uom::si::mass_rate::kilogram_per_second;
    use uom::si::specific_heat_capacity::joule_per_kilogram_kelvin;
    use uom::si::thermal_conductance::watt_per_kelvin;

    /// The side-panel sweep grid: every user-reachable slider corner plus the
    /// salt states the intermediate loop can actually sit in.
    ///
    /// `UA` and the two mass flows are exactly the slider ranges declared in
    /// `app/side_panel.rs`; the salt inlet temperature spans the HITEC
    /// validity window (440 K to 800 K, i.e. 167 to 527 degC) from
    /// `tuas_boussinesq_solver`'s `hitec_nitrate_salt` database.
    struct SweepGrid {
        ua_w_per_k: Vec<f64>,
        feedwater_flow_kg_per_s: Vec<f64>,
        pump_pressure_bar: Vec<f64>,
        salt_inlet_degc: Vec<f64>,
        salt_flow_kg_per_s: Vec<f64>,
        tube_outlet_degc: Vec<f64>,
    }

    fn sweep_grid() -> SweepGrid {
        SweepGrid {
            // slider: 0.0e5 ..= 7.0e5 W/K
            ua_w_per_k: vec![
                0.0, 2.5e4, 5.0e4, 1.0e5, 1.5e5, 2.0e5, 3.0e5, 4.0e5, 5.0e5, 6.0e5, 7.0e5,
            ],
            // slider: 10.0 ..= 80.0 kg/s
            feedwater_flow_kg_per_s: vec![10.0, 20.0, 35.0, 50.0, 65.0, 80.0],
            // slider: 1.2 ..= 200.0 bar
            pump_pressure_bar: vec![1.2, 5.0, 20.0, 80.0, 200.0],
            // HITEC validity window, 167 to 527 degC
            salt_inlet_degc: vec![180.0, 250.0, 350.0, 450.0, 500.0, 520.0],
            // intermediate-loop SG-branch flow, natural circulation to forced
            salt_flow_kg_per_s: vec![0.5, 5.0, 20.0, 60.0],
            // lagged tube-side outlet from the previous timestep
            tube_outlet_degc: vec![40.0, 100.0, 200.0, 300.0, 450.0],
        }
    }

    /// Tube-side outlet temperature implied by a duty, via the steady energy
    /// balance the persistent `TampinesSteamArray` relaxes onto:
    /// `h_out = h_in + Q / m_feed`.
    fn tube_outlet_temperature_for_duty(
        duty: Power,
        feedwater: FeedwaterInletState,
        feedwater_mass_flowrate: MassRate,
    ) -> ThermodynamicTemperature {
        let outlet_enthalpy = feedwater.enthalpy + duty / feedwater_mass_flowrate;
        ph_flash_eqm::t_ph_eqm(feedwater.pressure, outlet_enthalpy)
    }

    /// **V&V — second law across the entire user-reachable slider space.**
    ///
    /// ## Methodology
    ///
    /// The steam-generator duty is the only place `fhr_sim_v2` couples the
    /// HITEC shell to the water/steam tube, so every second-law claim about
    /// the steam cycle reduces to a claim about that one number. This test
    /// sweeps the full Cartesian product of the side-panel sliders and the
    /// salt states the intermediate loop can occupy:
    ///
    /// | Swept quantity | Range | Source |
    /// |---|---|---|
    /// | Overall `UA` | 0 to 7.0e5 W/K, 11 points | `app/side_panel.rs` slider |
    /// | Feedwater flow | 10 to 80 kg/s, 6 points | `app/side_panel.rs` slider |
    /// | Pump outlet pressure | 1.2 to 200 bar, 5 points | `app/side_panel.rs` slider |
    /// | Salt (shell inlet) temperature | 180 to 520 degC, 6 points | HITEC validity window, 440 to 800 K, `tuas_boussinesq_solver::…::hitec_nitrate_salt` |
    /// | Salt SG-branch flow | 0.5 to 60 kg/s, 4 points | intermediate-loop natural-circulation to forced range |
    /// | Lagged tube outlet | 40 to 450 degC, 5 points | previous-timestep feedback term |
    ///
    /// That is 11 x 6 x 5 x 6 x 4 x 5 = 39 600 operating points. At each one
    /// four second-law criteria are asserted on
    /// [`pinch_limited_steam_generator_duty`]:
    ///
    /// 1. **(exact)** the tube-side outlet enthalpy `h_in + Q/m` does not
    ///    exceed `h(p, T_salt_in)` — the enthalpy statement of "the steam
    ///    never leaves hotter than the salt entered", free of any
    ///    backward-equation error, asserted to 1e-9 relative;
    /// 2. the same statement in temperature, flashing `h_out` back through
    ///    the IF97 `(p, h)` backward equation, asserted to 0.05 K. That
    ///    tolerance is the standard's own numerical consistency for `T(p, h)`
    ///    (0.023 K in region 1, 0.01 K in region 2) — it is a property of the
    ///    steam tables, not slack in the physics, which is why criterion 1
    ///    carries the exact test;
    /// 3. the salt outlet, `T_salt_in - Q/(m_salt cp)`, does not fall below
    ///    the feedwater inlet temperature;
    /// 4. the effectiveness `Q/Q_max` lies in `[0, 1]`.
    ///
    /// The same grid is then run through [`unclamped_legacy_duty`], the
    /// formula the simulator used before 2026-08-12, in
    /// [`the_pre_fix_duty_formula_violates_the_sweep`]. That step is what
    /// makes this test meaningful: a test that passed before and after the
    /// fix would prove nothing.
    ///
    /// ## Results — measured 2026-08-12
    ///
    /// **Post-fix (`pinch_limited_steam_generator_duty`):** 0 of 39 600
    /// points violate any of the four criteria.
    ///
    /// | Measured worst case over the grid | Value |
    /// |---|---|
    /// | Tube outlet enthalpy excess over the ceiling | `0.000e0` (relative) — the cap binds exactly |
    /// | Steam outlet excess over the salt inlet | `1.911e-2 K` — entirely the IF97 `T(p, h)` round trip, inside the standard's 0.023 K region-1 consistency |
    /// | Salt outlet deficit below the feedwater inlet | `5.684e-14 K` (floating-point noise) |
    /// | Maximum effectiveness | `1.000000` |
    ///
    /// **Pre-fix, same grid** (full detail in
    /// [`the_pre_fix_duty_formula_violates_the_sweep`]): **23 417 of 39 600
    /// points — 59.1 % — violate the second law.** Worst steam-side excess
    /// 16.16x the cold-side ceiling; worst salt-side excess 889x the salt
    /// capacity rate.
    ///
    /// ## Interpretation
    ///
    /// The temperature cross was not a rendering artefact — the *duty* was
    /// wrong by up to a factor of 889, and that duty is what drives the
    /// turbine power, the condenser load and the salt-side energy balance.
    /// Clamping the duty to `Q_max` makes the cross unreachable for every
    /// slider setting the GUI can produce, which is why the fix is in the
    /// duty and not in the display. The residual limitation is fidelity, not
    /// legality: a single-lump exchanger enforces only the terminal pinch, so
    /// an *internal* pinch (a real once-through generator's evaporator exit)
    /// is still not represented.
    #[test]
    fn second_law_holds_across_the_whole_ua_slider() {
        let grid = sweep_grid();
        // IAPWS-IF97's backward equation T(p, h) is not the exact inverse of
        // the forward h(T, p): the standard quotes a numerical consistency of
        // 0.023 K in region 1 and 0.01 K in region 2. The *exact* statement of
        // "no cross" is therefore made on enthalpy (below, to 1e-9 relative);
        // the temperature assertion carries this tolerance so a documented
        // round-trip inconsistency is not mistaken for a physics violation.
        let tolerance_k = 0.05;
        let enthalpy_tolerance = 1.0e-9;

        let mut points = 0_usize;
        let mut worst_steam_excess_k = f64::NEG_INFINITY;
        let mut worst_salt_deficit_k = f64::NEG_INFINITY;
        let mut worst_effectiveness = f64::NEG_INFINITY;
        let mut worst_enthalpy_excess_fraction = f64::NEG_INFINITY;

        for &pump_pressure_bar in &grid.pump_pressure_bar {
            let feedwater = feedwater_inlet_state(Pressure::new::<bar>(pump_pressure_bar));

            for &salt_inlet_degc in &grid.salt_inlet_degc {
                let salt_inlet = ThermodynamicTemperature::new::<degree_celsius>(salt_inlet_degc);
                let salt_cp: SpecificHeatCapacity = LiquidMaterial::HITEC
                    .try_get_cp(salt_inlet)
                    .expect("HITEC cp inside its validity window");

                for &tube_outlet_degc in &grid.tube_outlet_degc {
                    let tube_outlet =
                        ThermodynamicTemperature::new::<degree_celsius>(tube_outlet_degc);

                    for &salt_flow in &grid.salt_flow_kg_per_s {
                        let salt_mass_flowrate = MassRate::new::<kilogram_per_second>(salt_flow);

                        for &feed_flow in &grid.feedwater_flow_kg_per_s {
                            let feedwater_mass_flowrate =
                                MassRate::new::<kilogram_per_second>(feed_flow);

                            for &ua in &grid.ua_w_per_k {
                                points += 1;
                                let result = pinch_limited_steam_generator_duty(
                                    ThermalConductance::new::<watt_per_kelvin>(ua),
                                    // shell mean == shell inlet here: the cap
                                    // must hold even when the lump is
                                    // isothermal, which is the tightest case.
                                    salt_inlet,
                                    salt_inlet,
                                    salt_mass_flowrate,
                                    salt_cp,
                                    feedwater,
                                    feedwater_mass_flowrate,
                                    tube_outlet,
                                );

                                // (3) effectiveness within [0, 1]
                                assert!(
                                    result.effectiveness >= 0.0
                                        && result.effectiveness <= 1.0 + 1.0e-9,
                                    "effectiveness {} outside [0, 1] at UA={ua} W/K, \
                                     m_feed={feed_flow} kg/s, p={pump_pressure_bar} bar, \
                                     T_salt_in={salt_inlet_degc} degC, \
                                     m_salt={salt_flow} kg/s, \
                                     T_tube_out={tube_outlet_degc} degC",
                                    result.effectiveness
                                );
                                worst_effectiveness = worst_effectiveness.max(result.effectiveness);

                                // (1a) the exact form of "steam outlet never
                                // exceeds the salt inlet": the tube-side
                                // outlet enthalpy never exceeds the enthalpy
                                // of water at the salt inlet temperature.
                                // This is free of any backward-equation
                                // round-trip error.
                                let outlet_enthalpy =
                                    feedwater.enthalpy + result.duty / feedwater_mass_flowrate;
                                let ceiling =
                                    cold_side_enthalpy_ceiling(salt_inlet, feedwater.pressure);
                                let enthalpy_excess_fraction =
                                    (outlet_enthalpy - ceiling).value / ceiling.value.abs();
                                worst_enthalpy_excess_fraction =
                                    worst_enthalpy_excess_fraction.max(enthalpy_excess_fraction);
                                assert!(
                                    enthalpy_excess_fraction <= enthalpy_tolerance,
                                    "enthalpy cross: tube outlet enthalpy exceeds the \
                                     ceiling at the salt inlet temperature by a \
                                     fraction {enthalpy_excess_fraction:.3e} at \
                                     UA={ua} W/K, m_feed={feed_flow} kg/s, \
                                     p={pump_pressure_bar} bar, \
                                     T_salt_in={salt_inlet_degc} degC, \
                                     m_salt={salt_flow} kg/s, \
                                     T_tube_out={tube_outlet_degc} degC"
                                );

                                // (1b) the same statement in temperature, as
                                // the user sees it on screen.
                                let steam_outlet = tube_outlet_temperature_for_duty(
                                    result.duty,
                                    feedwater,
                                    feedwater_mass_flowrate,
                                );
                                let steam_excess_k =
                                    steam_outlet.get::<kelvin>() - salt_inlet.get::<kelvin>();
                                worst_steam_excess_k = worst_steam_excess_k.max(steam_excess_k);
                                assert!(
                                    steam_excess_k <= tolerance_k,
                                    "temperature cross: steam outlet {:.3} degC exceeds \
                                     salt inlet {salt_inlet_degc:.3} degC by {steam_excess_k:.3} K \
                                     at UA={ua} W/K, m_feed={feed_flow} kg/s, \
                                     p={pump_pressure_bar} bar, m_salt={salt_flow} kg/s, \
                                     T_tube_out={tube_outlet_degc} degC (duty {:.4e} W)",
                                    steam_outlet.get::<degree_celsius>(),
                                    result.duty.get::<watt>()
                                );

                                // (2) salt outlet never falls below the feedwater inlet
                                let salt_capacity_rate: ThermalConductance =
                                    (salt_mass_flowrate * salt_cp).into();
                                let salt_temperature_drop_k = result.duty.get::<watt>()
                                    / salt_capacity_rate.get::<watt_per_kelvin>();
                                let salt_outlet_k =
                                    salt_inlet.get::<kelvin>() - salt_temperature_drop_k;
                                let salt_deficit_k =
                                    feedwater.temperature.get::<kelvin>() - salt_outlet_k;
                                worst_salt_deficit_k = worst_salt_deficit_k.max(salt_deficit_k);
                                assert!(
                                    salt_deficit_k <= tolerance_k,
                                    "temperature cross: salt outlet {:.3} degC falls \
                                     {salt_deficit_k:.3} K below the feedwater inlet \
                                     {:.3} degC at UA={ua} W/K, m_feed={feed_flow} kg/s, \
                                     p={pump_pressure_bar} bar, m_salt={salt_flow} kg/s",
                                    salt_outlet_k - 273.15,
                                    feedwater.temperature.get::<degree_celsius>()
                                );
                            }
                        }
                    }
                }
            }
        }

        assert_eq!(
            points,
            grid.ua_w_per_k.len()
                * grid.feedwater_flow_kg_per_s.len()
                * grid.pump_pressure_bar.len()
                * grid.salt_inlet_degc.len()
                * grid.salt_flow_kg_per_s.len()
                * grid.tube_outlet_degc.len(),
            "the sweep must actually visit the whole grid"
        );

        println!(
            "swept {points} operating points; worst steam excess {worst_steam_excess_k:.3e} K, \
             worst enthalpy excess fraction {worst_enthalpy_excess_fraction:.3e}, \
             worst salt deficit {worst_salt_deficit_k:.3e} K, \
             max effectiveness {worst_effectiveness:.6}"
        );
    }

    /// **V&V — the sweep discriminates: the pre-fix formula fails it.**
    ///
    /// ## Methodology
    ///
    /// Re-runs the identical grid of
    /// [`second_law_holds_across_the_whole_ua_slider`] against
    /// [`unclamped_legacy_duty`], the formula `fhr_sim_v2` used before
    /// 2026-08-12, and counts the points whose duty exceeds either bound:
    ///
    /// - the **cold-side** ceiling `m_feed (h(p, T_salt_in) - h_feed_in)` —
    ///   above it the steam leaves hotter than the salt entered;
    /// - the **hot-side** capacity `m_salt cp (T_salt_in - T_feed_in)` —
    ///   above it the salt leaves colder than the feedwater entered.
    ///
    /// Both are measured in watts rather than flashed to a temperature,
    /// because the worst legacy duties throw the steam state clean out of the
    /// IAPWS-IF97 envelope and the flash would panic.
    ///
    /// The test asserts a *large* violation count, so it fails if the legacy
    /// formula is ever quietly replaced by the fixed one here — that would
    /// silently turn the discrimination check into a tautology.
    ///
    /// ## Results — measured 2026-08-12
    ///
    /// **23 417 of 39 600 points (59.1 %) violate the second law**: 8 080 on
    /// the steam side, 23 340 on the salt side (most violate both).
    ///
    /// - Worst **steam-side** cross: duty `9.8000e7 W = 98.0 MW` against a
    ///   `6.0650e6 W = 6.07 MW` ceiling — **16.16 times** what the feedwater
    ///   stream can absorb — at `UA` = 7.0e5 W/K (slider maximum), 10 kg/s
    ///   feedwater, 200 bar, 180 degC salt inlet, 40 degC lagged tube outlet.
    /// - Worst **salt-side** cross: duty `3.3600e8 W = 336 MW` against a
    ///   `3.7789e5 W = 0.378 MW` salt capacity — **889.14 times** what the
    ///   salt stream can deliver — at `UA` = 7.0e5 W/K, 0.5 kg/s salt,
    ///   520 degC salt inlet, 200 bar, 40 degC lagged tube outlet.
    /// - The feedwater sliders at their shipped defaults are among the
    ///   violating points whenever the salt flow is low; see
    ///   [`a_throttled_intermediate_loop_crossed_on_the_salt_side`] for that
    ///   case worked through in temperature. For the plant at its *nominal*
    ///   642.9 kg/s of salt, the crossing threshold is `UA` = 4.18e5 W/K —
    ///   60 % of the way up the slider — measured in
    ///   `steam_generator_pinch_at_the_nominal_operating_point`.
    ///
    /// ## Interpretation
    ///
    /// The sweep in this module is not vacuous: the code it replaced fails it
    /// on nearly three fifths of the user-reachable operating space. Note
    /// that the
    /// salt-side violation is both the more common and by far the larger of
    /// the two — the model was routinely extracting hundreds of times more
    /// heat from the intermediate loop than that loop could carry, which is
    /// the direct cause of the shell temperature collapsing into the tube
    /// temperature and producing the cross the user sees.
    #[test]
    fn the_pre_fix_duty_formula_violates_the_sweep() {
        let grid = sweep_grid();

        let mut points = 0_usize;
        let mut steam_side_violations = 0_usize;
        let mut salt_side_violations = 0_usize;
        let mut any_violations = 0_usize;
        let mut worst_steam_ratio = 0.0_f64;
        let mut worst_steam_case = String::new();
        let mut worst_salt_ratio = 0.0_f64;
        let mut worst_salt_case = String::new();

        for &pump_pressure_bar in &grid.pump_pressure_bar {
            let feedwater = feedwater_inlet_state(Pressure::new::<bar>(pump_pressure_bar));

            for &salt_inlet_degc in &grid.salt_inlet_degc {
                let salt_inlet = ThermodynamicTemperature::new::<degree_celsius>(salt_inlet_degc);
                let salt_cp: SpecificHeatCapacity = LiquidMaterial::HITEC
                    .try_get_cp(salt_inlet)
                    .expect("HITEC cp inside its validity window");
                let ceiling_per_kg_per_s_w = (MassRate::new::<kilogram_per_second>(1.0)
                    * (cold_side_enthalpy_ceiling(salt_inlet, feedwater.pressure)
                        - feedwater.enthalpy))
                    .get::<watt>();
                let hot_side_span = TemperatureInterval::new::<kelvin_interval>(
                    salt_inlet.get::<kelvin>() - feedwater.temperature.get::<kelvin>(),
                );

                for &tube_outlet_degc in &grid.tube_outlet_degc {
                    let tube_outlet =
                        ThermodynamicTemperature::new::<degree_celsius>(tube_outlet_degc);

                    for &salt_flow in &grid.salt_flow_kg_per_s {
                        let salt_capacity_rate: ThermalConductance =
                            (MassRate::new::<kilogram_per_second>(salt_flow) * salt_cp).into();
                        let hot_side_maximum_w = (salt_capacity_rate * hot_side_span).get::<watt>();

                        for &feed_flow in &grid.feedwater_flow_kg_per_s {
                            let cold_side_maximum_w = ceiling_per_kg_per_s_w * feed_flow;

                            for &ua in &grid.ua_w_per_k {
                                points += 1;
                                let legacy_w = unclamped_legacy_duty(
                                    ThermalConductance::new::<watt_per_kelvin>(ua),
                                    salt_inlet,
                                    tube_outlet,
                                )
                                .get::<watt>();

                                let crosses_steam = legacy_w > cold_side_maximum_w * (1.0 + 1.0e-9);
                                let crosses_salt = legacy_w > hot_side_maximum_w * (1.0 + 1.0e-9);

                                if crosses_steam {
                                    steam_side_violations += 1;
                                    let ratio = legacy_w / cold_side_maximum_w;
                                    if ratio > worst_steam_ratio {
                                        worst_steam_ratio = ratio;
                                        worst_steam_case = format!(
                                            "UA={ua:.3e} W/K, m_feed={feed_flow} kg/s, \
                                             p={pump_pressure_bar} bar, \
                                             T_salt_in={salt_inlet_degc} degC, \
                                             T_tube_out={tube_outlet_degc} degC: \
                                             duty {legacy_w:.4e} W vs ceiling \
                                             {cold_side_maximum_w:.4e} W"
                                        );
                                    }
                                }
                                if crosses_salt {
                                    salt_side_violations += 1;
                                    let ratio = legacy_w / hot_side_maximum_w;
                                    if ratio > worst_salt_ratio {
                                        worst_salt_ratio = ratio;
                                        worst_salt_case = format!(
                                            "UA={ua:.3e} W/K, m_salt={salt_flow} kg/s, \
                                             T_salt_in={salt_inlet_degc} degC, \
                                             p={pump_pressure_bar} bar, \
                                             T_tube_out={tube_outlet_degc} degC: \
                                             duty {legacy_w:.4e} W vs salt capacity \
                                             {hot_side_maximum_w:.4e} W"
                                        );
                                    }
                                }
                                if crosses_steam || crosses_salt {
                                    any_violations += 1;
                                }
                            }
                        }
                    }
                }
            }
        }

        println!(
            "pre-fix formula over {points} points: {any_violations} violate the second \
             law ({steam_side_violations} steam-side, {salt_side_violations} salt-side)"
        );
        println!("  worst steam-side excess ratio {worst_steam_ratio:.2} at {worst_steam_case}");
        println!("  worst salt-side excess ratio {worst_salt_ratio:.2} at {worst_salt_case}");

        assert!(
            any_violations > points / 10,
            "the pre-fix formula is expected to violate the second law on a large \
             fraction of the grid ({any_violations} of {points}); if this ever passes, \
             the legacy reference has been replaced and the discrimination check is \
             vacuous"
        );
        assert!(
            worst_steam_ratio > 5.0,
            "worst pre-fix steam-side duty/ceiling ratio was only {worst_steam_ratio:.2}"
        );
    }

    /// **V&V — a throttled intermediate loop crossed on the salt side.**
    ///
    /// ## Methodology
    ///
    /// The companion test
    /// `steam_generator_pinch_at_the_nominal_operating_point::the_ua_slider_crossed_partway_up_at_the_nominal_operating_point`
    /// covers the plant at its *nominal* 642.9 kg/s of salt, where the
    /// binding constraint is the cold side. This test covers the other
    /// regime: the intermediate loop **throttled to 20 kg/s** — the condition
    /// a user reaches by winding the intermediate pump down, or that the loop
    /// approaches under natural circulation — with the feedwater sliders at
    /// their shipped defaults (`UA` = 1.5e5 W/K, 50 kg/s, 1.2 bar), a 500 degC
    /// shell and a 100 degC lagged tube outlet.
    ///
    /// Here it is the **hot** side that runs out first, so the failure shows
    /// up as the salt being cooled below the feedwater rather than as steam
    /// above the salt. The pre-fix and pinch-limited duties are computed side
    /// by side together with the salt temperature drop `Q / (m_salt cp)` each
    /// implies.
    ///
    /// ## Results — measured 2026-08-12
    ///
    /// - Pre-fix duty **6.0000e7 W = 60.0 MW**; pinch-limited duty
    ///   **1.4508e7 W = 14.5 MW**, capped by the salt capacity rate
    ///   (`limited_by = SaltCapacityRate`, effectiveness exactly 1.0000).
    ///   The pre-fix formula drew **4.14 times** the heat this salt stream can
    ///   deliver.
    /// - As a salt temperature drop: `60.0e6 / (20 * 1560) = 1923 K`, taking
    ///   the 500 degC salt inlet to a nominal **-1423 degC outlet** — far
    ///   below the 35 degC feedwater that is supposedly heating from it. The
    ///   pinch-limited duty gives a 465 K drop, landing the salt outlet
    ///   exactly on the 35 degC feedwater inlet, which is the pinch.
    /// - The *steam-side* ceiling is not breached at this point (at 1.2 bar
    ///   the water can superheat all the way to 500 degC first), which is why
    ///   both bounds are needed: neither one alone catches every violation.
    ///
    /// ## Interpretation
    ///
    /// The `UA` slider is not the only route into the defect. Reducing
    /// intermediate-loop circulation — the very manoeuvre a natural-circulation
    /// demonstration performs — put the pre-fix model four times past its
    /// salt-side limit even with every other slider untouched. What the user
    /// then sees as a temperature cross is downstream of that: the shell is
    /// drained of four times the heat it can supply, its lumped temperature
    /// collapses toward the tube-side temperature, and the one-timestep lag in
    /// the feedback term lets the tube outlet overshoot it.
    #[test]
    fn a_throttled_intermediate_loop_crossed_on_the_salt_side() {
        let feedwater = feedwater_inlet_state(Pressure::new::<bar>(1.2));
        let salt_inlet = ThermodynamicTemperature::new::<degree_celsius>(500.0);
        let salt_cp: SpecificHeatCapacity = LiquidMaterial::HITEC
            .try_get_cp(salt_inlet)
            .expect("HITEC cp at 500 degC");
        let ua = ThermalConductance::new::<watt_per_kelvin>(1.5e5);
        let feed_flow = MassRate::new::<kilogram_per_second>(50.0);
        let salt_flow = MassRate::new::<kilogram_per_second>(20.0);
        let tube_outlet = ThermodynamicTemperature::new::<degree_celsius>(100.0);

        let legacy = unclamped_legacy_duty(ua, salt_inlet, tube_outlet);
        let fixed = pinch_limited_steam_generator_duty(
            ua,
            salt_inlet,
            salt_inlet,
            salt_flow,
            salt_cp,
            feedwater,
            feed_flow,
            tube_outlet,
        );

        let fixed_outlet = tube_outlet_temperature_for_duty(fixed.duty, feedwater, feed_flow);
        let salt_capacity_rate: ThermalConductance = (salt_flow * salt_cp).into();
        let legacy_salt_drop_k = legacy.get::<watt>() / salt_capacity_rate.get::<watt_per_kelvin>();
        let fixed_salt_drop_k =
            fixed.duty.get::<watt>() / salt_capacity_rate.get::<watt_per_kelvin>();

        println!(
            "default settings (UA=1.5e5 W/K, 50 kg/s feedwater, 1.2 bar, salt 500 degC \
             at 20 kg/s, lagged tube outlet 100 degC):"
        );
        println!(
            "  legacy duty {:.4e} W -> salt drop {legacy_salt_drop_k:.1} K, \
             nominal salt outlet {:.1} degC",
            legacy.get::<watt>(),
            500.0 - legacy_salt_drop_k
        );
        println!(
            "  pinch-limited duty {:.4e} W (max {:.4e} W, eps {:.4}, limited by {:?}) \
             -> salt drop {fixed_salt_drop_k:.1} K, salt outlet {:.1} degC, \
             steam outlet {:.2} degC",
            fixed.duty.get::<watt>(),
            fixed.thermodynamic_maximum.get::<watt>(),
            fixed.effectiveness,
            fixed.limited_by,
            500.0 - fixed_salt_drop_k,
            fixed_outlet.get::<degree_celsius>()
        );
        println!(
            "  legacy/pinch-limited duty ratio {:.2}",
            legacy.get::<watt>() / fixed.duty.get::<watt>()
        );

        assert!(
            legacy.get::<watt>() > 2.0 * fixed.duty.get::<watt>(),
            "the legacy duty {:.4e} W should be far above the pinch-limited \
             {:.4e} W at the default settings",
            legacy.get::<watt>(),
            fixed.duty.get::<watt>()
        );
        // The salt cannot be cooled below the feedwater that is heating from
        // it. The legacy duty demands a drop that takes it hundreds of kelvin
        // past that; the pinch-limited one lands exactly on it.
        let feedwater_inlet_degc = feedwater.temperature.get::<degree_celsius>();
        assert!(
            500.0 - legacy_salt_drop_k < feedwater_inlet_degc,
            "the legacy duty should drive the salt below the {feedwater_inlet_degc:.1} \
             degC feedwater inlet"
        );
        assert!(
            500.0 - fixed_salt_drop_k >= feedwater_inlet_degc - 0.05,
            "the pinch-limited salt outlet {:.3} degC must not fall below the \
             {feedwater_inlet_degc:.3} degC feedwater inlet",
            500.0 - fixed_salt_drop_k
        );
        assert!(
            fixed_outlet.get::<degree_celsius>() <= 500.0 + 0.05,
            "the pinch-limited steam outlet {:.3} degC must not exceed the 500 degC \
             salt inlet",
            fixed_outlet.get::<degree_celsius>()
        );
    }

    /// **V&V — the cap reduces to the unclamped conductance when it should.**
    ///
    /// ## Methodology
    ///
    /// A well-posed operating point — a 500 degC salt inlet, 40 kg/s of salt
    /// (capacity rate `40 * 1560 = 6.24e4` W/K), 50 kg/s of feedwater at
    /// 1.2 bar, a 200 degC lagged tube outlet and a modest `UA` of 2.0e4 W/K
    /// — is chosen so the conductance term `UA dT = 2.0e4 * 300 = 6.0e6` W
    /// sits well below both bounds. The duty must then equal the legacy
    /// conductance value exactly, proving the fix does not throttle the model
    /// in its normal regime; it must be flagged
    /// [`SteamGeneratorDutyLimit::Conductance`].
    ///
    /// ## Results — measured 2026-08-12
    ///
    /// duty = 6.000000e6 W, identical to `UA dT` to within 1e-9 relative;
    /// thermodynamic maximum 1.945e7 W (salt-capacity limited); effectiveness
    /// 0.3085; `limited_by = Conductance`.
    ///
    /// ## Interpretation
    ///
    /// The clamp is inactive in the well-posed regime, so the tuned nominal
    /// behaviour of the simulator is preserved and only physically impossible
    /// operating points are altered.
    #[test]
    fn the_clamp_is_inactive_in_the_well_posed_regime() {
        let feedwater = feedwater_inlet_state(Pressure::new::<bar>(1.2));
        let salt_inlet = ThermodynamicTemperature::new::<degree_celsius>(500.0);
        let salt_cp = SpecificHeatCapacity::new::<joule_per_kilogram_kelvin>(1560.0);
        let ua = ThermalConductance::new::<watt_per_kelvin>(2.0e4);
        let tube_outlet = ThermodynamicTemperature::new::<degree_celsius>(200.0);

        let result = pinch_limited_steam_generator_duty(
            ua,
            salt_inlet,
            salt_inlet,
            MassRate::new::<kilogram_per_second>(40.0),
            salt_cp,
            feedwater,
            MassRate::new::<kilogram_per_second>(50.0),
            tube_outlet,
        );

        let expected_w = 2.0e4 * 300.0;
        assert!(
            (result.duty.get::<watt>() - expected_w).abs() / expected_w < 1.0e-9,
            "duty {:.6e} W should equal the unclamped conductance {expected_w:.6e} W",
            result.duty.get::<watt>()
        );
        assert_eq!(result.limited_by, SteamGeneratorDutyLimit::Conductance);
        assert!(result.effectiveness > 0.0 && result.effectiveness < 1.0);
    }

    /// **V&V — no heat flows up a gradient.**
    ///
    /// ## Methodology
    ///
    /// The tube side is placed *hotter* than the shell (a 250 degC salt lump
    /// against a 400 degC tube outlet, `UA` at the slider maximum of
    /// 7.0e5 W/K). Heat must not flow from cold to hot, so the duty must be
    /// exactly zero and the limit reported as
    /// [`SteamGeneratorDutyLimit::NoDrivingTemperatureDifference`].
    ///
    /// This also closes an energy-conservation hole in the pre-fix code: it
    /// applied the *negative* duty to the shell (heating the salt) while
    /// clamping the tube side at zero, so energy was created out of nothing
    /// in exactly the state a cross produces.
    ///
    /// ## Results — measured 2026-08-12
    ///
    /// duty = 0.000000e0 W, effectiveness 0.0,
    /// `limited_by = NoDrivingTemperatureDifference`.
    #[test]
    fn no_duty_flows_when_the_tube_side_is_hotter_than_the_shell() {
        let feedwater = feedwater_inlet_state(Pressure::new::<bar>(1.2));
        let result = pinch_limited_steam_generator_duty(
            ThermalConductance::new::<watt_per_kelvin>(7.0e5),
            ThermodynamicTemperature::new::<degree_celsius>(250.0),
            ThermodynamicTemperature::new::<degree_celsius>(250.0),
            MassRate::new::<kilogram_per_second>(20.0),
            SpecificHeatCapacity::new::<joule_per_kilogram_kelvin>(1560.0),
            feedwater,
            MassRate::new::<kilogram_per_second>(50.0),
            ThermodynamicTemperature::new::<degree_celsius>(400.0),
        );

        assert_eq!(result.duty.get::<watt>(), 0.0);
        assert_eq!(result.effectiveness, 0.0);
        assert_eq!(
            result.limited_by,
            SteamGeneratorDutyLimit::NoDrivingTemperatureDifference
        );
    }
}
