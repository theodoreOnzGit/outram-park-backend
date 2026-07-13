//! Humid-air (moist-air) psychrometrics for cooling towers and secondary-loop
//! air-side calculations.
//!
//! Thin `uom`-typed wrapper over [`outram_park_fork_coolprop::humid_air`],
//! this workspace's `HAPropsSI`-equivalent port (ASHRAE RP-1485). See that
//! module's own documentation for the physics being wrapped -- coverage,
//! valid range (liquid-water branch only, `T > 273.16 K`), and caveats
//! (notably: `entropy`'s absolute reference-state convention has not been
//! independently cross-checked, though its temperature dependence is
//! verified).

use outram_park_fork_coolprop::humid_air::{ha_props, HaInput, HumidAirParam};
use uom::si::available_energy::joule_per_kilogram;
use uom::si::f64::{
    AvailableEnergy, Pressure, Ratio, SpecificHeatCapacity, SpecificVolume, ThermodynamicTemperature,
};
use uom::si::pressure::pascal;
use uom::si::ratio::ratio;
use uom::si::specific_heat_capacity::joule_per_kilogram_kelvin;
use uom::si::specific_volume::cubic_meter_per_kilogram;
use uom::si::thermodynamic_temperature::kelvin;

pub use outram_park_fork_coolprop::humid_air::{HumidAirError, HumidAirParam as CoolPropHumidAirParam};

/// A fully-resolved humid-air (moist-air) state, dimensioned via `uom`.
///
/// All extensive properties ([`Self::enthalpy`], [`Self::entropy`],
/// [`Self::volume`]) are per kilogram of *dry* air, matching CoolProp's
/// `HAPropsSI` convention.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HumidAirState {
    /// Dry-bulb temperature.
    pub t_dry_bulb: ThermodynamicTemperature,
    /// Total (barometric) pressure.
    pub pressure: Pressure,
    /// Water-vapour mole fraction `ψ_w` [-].
    pub water_mole_fraction: Ratio,
    /// Humidity ratio `W` [kg water / kg dry air].
    pub humidity_ratio: Ratio,
    /// Relative humidity `R` [0, 1].
    pub relative_humidity: Ratio,
    /// Specific enthalpy, per kg dry air.
    pub enthalpy: AvailableEnergy,
    /// Specific entropy, per kg dry air. See the module doc's caveat on this
    /// port's absolute reference-state convention.
    pub entropy: SpecificHeatCapacity,
    /// Specific volume, per kg dry air.
    pub volume: SpecificVolume,
}

/// Resolve a humid-air state from dry-bulb temperature, pressure, and
/// humidity ratio `W` [kg water / kg dry air] -- the input triple most
/// HVAC/cooling-tower and FHR-secondary-loop-air calculations use.
pub fn state_from_t_p_w(
    t: ThermodynamicTemperature,
    p: Pressure,
    w: Ratio,
) -> Result<HumidAirState, HumidAirError> {
    state_from_inputs(
        (HumidAirParam::TDryBulb, t.get::<kelvin>()),
        (HumidAirParam::Pressure, p.get::<pascal>()),
        (HumidAirParam::HumidityRatio, w.get::<ratio>()),
    )
}

/// Resolve a humid-air state from dry-bulb temperature, pressure, and
/// relative humidity `R` [0, 1].
pub fn state_from_t_p_r(
    t: ThermodynamicTemperature,
    p: Pressure,
    r: Ratio,
) -> Result<HumidAirState, HumidAirError> {
    state_from_inputs(
        (HumidAirParam::TDryBulb, t.get::<kelvin>()),
        (HumidAirParam::Pressure, p.get::<pascal>()),
        (HumidAirParam::RelativeHumidity, r.get::<ratio>()),
    )
}

/// Resolve a full [`HumidAirState`] from any three input constraints
/// `outram_park_fork_coolprop::humid_air` accepts (`(T, p, {W|R|ψ_w|T_dp|T_wb})`
/// in any order) -- the general escape hatch for callers who need the dew-point
/// or wet-bulb input forms that [`state_from_t_p_w`]/[`state_from_t_p_r`] don't
/// cover. Note: performs one backend solve per output field (8 total), since
/// the backend only exposes a single-output `ha_props` entry point.
pub fn state_from_inputs(
    in1: HaInput,
    in2: HaInput,
    in3: HaInput,
) -> Result<HumidAirState, HumidAirError> {
    Ok(HumidAirState {
        t_dry_bulb: ThermodynamicTemperature::new::<kelvin>(ha_props(
            HumidAirParam::TDryBulb,
            in1,
            in2,
            in3,
        )?),
        pressure: Pressure::new::<pascal>(ha_props(HumidAirParam::Pressure, in1, in2, in3)?),
        water_mole_fraction: Ratio::new::<ratio>(ha_props(
            HumidAirParam::WaterMoleFraction,
            in1,
            in2,
            in3,
        )?),
        humidity_ratio: Ratio::new::<ratio>(ha_props(HumidAirParam::HumidityRatio, in1, in2, in3)?),
        relative_humidity: Ratio::new::<ratio>(ha_props(
            HumidAirParam::RelativeHumidity,
            in1,
            in2,
            in3,
        )?),
        enthalpy: AvailableEnergy::new::<joule_per_kilogram>(ha_props(
            HumidAirParam::Enthalpy,
            in1,
            in2,
            in3,
        )?),
        entropy: SpecificHeatCapacity::new::<joule_per_kilogram_kelvin>(ha_props(
            HumidAirParam::Entropy,
            in1,
            in2,
            in3,
        )?),
        volume: SpecificVolume::new::<cubic_meter_per_kilogram>(ha_props(
            HumidAirParam::Volume,
            in1,
            in2,
            in3,
        )?),
    })
}
