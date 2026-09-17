use uom::si::{f64::*, pressure::megapascal, thermodynamic_temperature::kelvin};

use crate::{
    interfaces::functional_programming::pt_flash_eqm::h_tp_eqm_single_phase,
    region_4_vap_liq_equilibrium::sat_pressure_4,
};
use crate::region_1_subcooled_liquid::h_tp_1;
use crate::region_5_steam_at_800_plus_degc::h_tp_5;

// checks if pressure is
// lower than saturation pressure at 273.15K or higher than 100 MPa
//
// if so, it falls outside the ph boundary
pub(crate) fn is_outside_pressure_range(p: Pressure) -> bool {
    // first determine if p,h point is outside pressure range
    let lower_pressure_limit: Pressure =
        sat_pressure_4(ThermodynamicTemperature::new::<kelvin>(273.15));

    let upper_pressure_limit: Pressure = Pressure::new::<megapascal>(100.0);

    if p < lower_pressure_limit {
        return true;
    };

    if p > upper_pressure_limit {
        return true;
    };

    return false;
}

// making a function to check if a p,h value is below the isotherm at
// 273.15K
//
pub(crate) fn is_below_isotherm_t_273_15(p: Pressure, h: AvailableEnergy) -> bool {
    // first check if outside pressure range
    if is_outside_pressure_range(p) {
        panic!("outside pressure range");
    };

    // let's have the lower enthalpy range
    let lower_temp_bound = ThermodynamicTemperature::new::<kelvin>(273.15);

    // NOTE (p_sat(273.15 K) trap): the whole T = 273.15 K isotherm, for every
    // valid pressure p in [p_sat(273.15 K), 100 MPa], lies in Region 1
    // (compressed / saturated liquid). We therefore evaluate the lower-bound
    // enthalpy with the Region 1 forward equation `h_tp_1` directly, instead of
    // routing through `h_tp_eqm_single_phase`. The (T,p) region router returns
    // `Region4` when the pressure is *exactly* the saturation pressure of the
    // temperature (`pres == p_sat_reg4`), and the Region 4 (T,p) arm is
    // deliberately unsupported (two-phase (T,p) is under-determined without
    // steam quality). At p == p_sat(273.15 K) = 611.213 Pa that made every
    // (p,h) flash panic before it could even classify the point. Calling the
    // Region 1 forward equation here avoids that degeneracy: it is the correct
    // saturated-liquid enthalpy on the saturation line and the correct
    // compressed-liquid enthalpy above it.
    let lower_bound_enthalpy = h_tp_1(lower_temp_bound, p);

    if h < lower_bound_enthalpy {
        return true;
    };

    return false;
}

// making a function to check if p,h value is above the isotherm T = 1073.15K
pub(crate) fn is_above_isotherm_t_1073_15(p: Pressure, h: AvailableEnergy) -> bool {
    // first check if outside pressure range
    if is_outside_pressure_range(p) {
        panic!("outside pressure range");
    };

    let upper_temp_bound = ThermodynamicTemperature::new::<kelvin>(1073.15);

    let upper_bound_enthalpy = h_tp_eqm_single_phase(upper_temp_bound, p);

    if h > upper_bound_enthalpy {
        return true;
    };

    return false;
}

/// Upper temperature bound of IAPWS-IF97 Region 5, in kelvin.
pub(crate) const REGION_5_MAX_TEMP_KELVIN: f64 = 2273.15;

/// Upper pressure bound of IAPWS-IF97 Region 5, in MPa.
///
/// Region 5 is defined only to 50 MPa, where Regions 1 to 4 extend to 100 MPa.
/// A `(p,h)` point above the 1073.15 K isotherm but above this pressure is
/// therefore outside the formulation entirely, not merely outside a backward
/// equation.
pub(crate) const REGION_5_MAX_PRESSURE_MPA: f64 = 50.0;

/// Returns `true` when the pressure is inside Region 5's own pressure range.
pub(crate) fn is_within_region_5_pressure_range(p: Pressure) -> bool {
    p <= Pressure::new::<megapascal>(REGION_5_MAX_PRESSURE_MPA)
}

/// Returns `true` when the `(p,h)` point lies above Region 5's upper isotherm,
/// `T = 2273.15 K`, and is therefore outside IAPWS-IF97 altogether.
///
/// `p` is pressure and `h` specific enthalpy. Evaluated with the Region 5
/// forward equation, which is the formulation that owns that isotherm.
pub(crate) fn is_above_isotherm_t_2273_15(p: Pressure, h: AvailableEnergy) -> bool {
    let upper_temp_bound = ThermodynamicTemperature::new::<kelvin>(REGION_5_MAX_TEMP_KELVIN);
    h > h_tp_5(upper_temp_bound, p)
}
