//! Steam secondary loop (scaffold).
//!
//! Models the Rankine secondary side: the IHX duty from the helium primary
//! loop boils and superheats feedwater in a steam generator, the steam drives
//! a turbine, and the exhaust is condensed. Steam/water properties are **real**
//! -- every state is an IAPWS-IF97 [`tampines::hem::HemSteamCv`]
//! (`tampines-steam-tables`' `TampinesSteamTableCV`) built from a genuine
//! `(p,h)` / `(p,s)` flash.
//!
//! ## What is real vs placeholder
//!
//! - **Real:** the steam-generator outlet state from an enthalpy balance
//!   `h_steam = h_feed + Q_ihx/m_dot` flashed at fixed steam pressure; the
//!   turbine outlet from an isentropic `(p,s)` flash de-rated by an adiabatic
//!   efficiency; turbine power `m_dot*(h_in - h_out)`; exhaust quality from the
//!   outlet `(p,h)` flash.
//! - **Placeholder** (see bead `op-wqk.9.3`): fixed steam pressure, fixed
//!   feedwater enthalpy, fixed secondary mass flow (no real feedwater
//!   pump/control), no condenser energy balance or cooling-water side, and no
//!   steam-generator UA / pinch limit on how much duty the steam can actually
//!   absorb. A v2 should solve the coupled steam cycle.

use tampines::hem::HemSteamCv;
use uom::si::available_energy::joule_per_kilogram;
use uom::si::f64::{
    AvailableEnergy, MassRate, Power, Pressure, SpecificHeatCapacity, ThermodynamicTemperature,
    Volume,
};
use uom::si::mass_rate::kilogram_per_second;
use uom::si::power::watt;
use uom::si::pressure::megapascal;
use uom::si::volume::cubic_meter;

/// Live steam pressure at the steam-generator outlet / turbine inlet
/// (placeholder), ~10 MPa, illustrative.
const STEAM_PRESSURE_MPA: f64 = 10.0;

/// Condenser back-pressure (placeholder), ~7 kPa (0.007 MPa), illustrative.
const CONDENSER_PRESSURE_MPA: f64 = 0.007;

/// Feedwater specific enthalpy entering the steam generator (placeholder),
/// ~1000 kJ/kg (subcooled liquid at feedwater conditions), illustrative.
const FEEDWATER_ENTHALPY_J_PER_KG: f64 = 1.0e6;

/// Secondary-side mass flow (placeholder), fixed at 80 kg/s.
const SECONDARY_MASS_FLOW_KG_PER_S: f64 = 80.0;

/// Turbine adiabatic (isentropic) efficiency (placeholder), 0.85.
const TURBINE_EFFICIENCY: f64 = 0.85;

/// Steam secondary-loop state.
pub struct SteamSecondaryLoop {
    steam_pressure: Pressure,
    condenser_pressure: Pressure,
    feedwater_enthalpy: AvailableEnergy,
    mass_flow: MassRate,
    /// Reference (extensive) control-volume size for the `HemSteamCv` states;
    /// intensive flash results do not depend on it.
    reference_volume: Volume,

    // Outputs (recomputed each step).
    steam_generator_outlet: HemSteamCv,
    turbine_inlet_temperature: ThermodynamicTemperature,
    turbine_power: Power,
    steam_quality_after_turbine: f64,
}

impl SteamSecondaryLoop {
    /// Construct the loop at its nominal (fixed) operating point, with the
    /// steam-generator outlet seeded from the feedwater enthalpy alone (zero
    /// IHX duty).
    pub fn new() -> Self {
        let steam_pressure = Pressure::new::<megapascal>(STEAM_PRESSURE_MPA);
        let condenser_pressure = Pressure::new::<megapascal>(CONDENSER_PRESSURE_MPA);
        let feedwater_enthalpy =
            AvailableEnergy::new::<joule_per_kilogram>(FEEDWATER_ENTHALPY_J_PER_KG);
        let reference_volume = Volume::new::<cubic_meter>(1.0);

        let steam_generator_outlet =
            HemSteamCv::new_from_ph(steam_pressure, feedwater_enthalpy, reference_volume);
        let turbine_inlet_temperature = steam_generator_outlet.get_temperature();

        Self {
            steam_pressure,
            condenser_pressure,
            feedwater_enthalpy,
            mass_flow: MassRate::new::<kilogram_per_second>(SECONDARY_MASS_FLOW_KG_PER_S),
            reference_volume,
            steam_generator_outlet,
            turbine_inlet_temperature,
            turbine_power: Power::new::<watt>(0.0),
            steam_quality_after_turbine: 0.0,
        }
    }

    /// Advance the loop by absorbing `ihx_duty` into the steam and expanding it
    /// through the turbine.
    ///
    /// `_primary_hot_temperature` is accepted for a future pinch/UA-limited
    /// steam-generator model; the current scaffold ignores it (all IHX duty is
    /// assumed transferable to the steam). Marked with a leading underscore so
    /// the coupling point is explicit at the call site.
    pub fn step(&mut self, ihx_duty: Power, _primary_hot_temperature: ThermodynamicTemperature) {
        let m_dot = self.mass_flow.get::<kilogram_per_second>();
        let h_feed = self.feedwater_enthalpy.get::<joule_per_kilogram>();

        // Steam-generator outlet enthalpy from the secondary energy balance.
        let h_steam = h_feed + ihx_duty.get::<watt>() / m_dot;
        self.steam_generator_outlet = HemSteamCv::new_from_ph(
            self.steam_pressure,
            AvailableEnergy::new::<joule_per_kilogram>(h_steam),
            self.reference_volume,
        );
        self.turbine_inlet_temperature = self.steam_generator_outlet.get_temperature();

        // Isentropic expansion to condenser pressure, then de-rate by the
        // adiabatic efficiency.
        let s_in: SpecificHeatCapacity = self.steam_generator_outlet.get_specific_entropy();
        let isentropic_outlet =
            HemSteamCv::new_from_ps(self.condenser_pressure, s_in, self.reference_volume);
        let h_in = h_steam;
        let h_out_isentropic = isentropic_outlet
            .get_specific_enthalpy()
            .get::<joule_per_kilogram>();
        let h_out = h_in - TURBINE_EFFICIENCY * (h_in - h_out_isentropic);

        // Actual turbine outlet state for quality readout.
        let turbine_outlet = HemSteamCv::new_from_ph(
            self.condenser_pressure,
            AvailableEnergy::new::<joule_per_kilogram>(h_out),
            self.reference_volume,
        );
        self.steam_quality_after_turbine = turbine_outlet.get_quality();

        self.turbine_power = Power::new::<watt>((m_dot * (h_in - h_out)).max(0.0));
    }

    /// The steam-generator secondary-side outlet state (real `HemSteamCv`).
    pub fn steam_generator_outlet(&self) -> HemSteamCv {
        self.steam_generator_outlet
    }

    /// Turbine inlet temperature (= steam-generator outlet temperature).
    pub fn turbine_inlet_temperature(&self) -> ThermodynamicTemperature {
        self.turbine_inlet_temperature
    }

    /// Live steam pressure (placeholder constant).
    pub fn steam_pressure(&self) -> Pressure {
        self.steam_pressure
    }

    /// Condenser back-pressure (placeholder constant).
    pub fn condenser_pressure(&self) -> Pressure {
        self.condenser_pressure
    }

    /// Turbine mechanical power output.
    pub fn turbine_power(&self) -> Power {
        self.turbine_power
    }

    /// Steam quality at the turbine exhaust `[0, 1]`.
    pub fn steam_quality_after_turbine(&self) -> f64 {
        self.steam_quality_after_turbine
    }
}

impl Default for SteamSecondaryLoop {
    fn default() -> Self {
        Self::new()
    }
}
