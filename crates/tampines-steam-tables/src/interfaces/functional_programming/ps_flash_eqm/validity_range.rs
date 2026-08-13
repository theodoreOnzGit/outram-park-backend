use uom::si::{f64::*, pressure::megapascal, thermodynamic_temperature::kelvin};

use crate::{
    interfaces::functional_programming::pt_flash_eqm::s_tp_eqm_single_phase,
    region_1_subcooled_liquid::s_tp_1, region_4_vap_liq_equilibrium::sat_pressure_4,
};

// checks if pressure is
// lower than saturation pressure at 273.15K or higher than 100 MPa
//
// if so, it falls outside the ps boundary
pub(crate) fn is_outside_pressure_range(p: Pressure) -> bool {
    // first determine if p,h point is outside pressure range
    let lower_pressure_limit: Pressure =
        sat_pressure_4(ThermodynamicTemperature::new::<kelvin>(273.15));

    let upper_pressure_limit: Pressure = Pressure::new::<megapascal>(100.0);

    if p < lower_pressure_limit {
        panic!(
            "p,s point is lower than acceptable pressure range: \
             p = {p:?}, lower limit = {lower_pressure_limit:?}"
        );
    };

    if p > upper_pressure_limit {
        panic!(
            "p,s point is higher than acceptable pressure range: \
             p = {p:?}, upper limit = {upper_pressure_limit:?}"
        );
    };

    return false;
}

// making a function to check if a p,s value is below the isotherm at
// 273.15K
//
pub(crate) fn is_below_isotherm_t_273_15(p: Pressure, s: SpecificHeatCapacity) -> bool {
    // first check if outside pressure range
    if is_outside_pressure_range(p) {
        panic!("outside pressure range");
    };

    // let's have the lower entropy range
    let lower_temp_bound = ThermodynamicTemperature::new::<kelvin>(273.15);

    // NOTE (p_sat(273.15 K) trap): the whole T = 273.15 K isotherm, for every
    // valid pressure p in [p_sat(273.15 K), 100 MPa], lies in Region 1
    // (compressed / saturated liquid). We therefore evaluate the lower-bound
    // entropy with the Region 1 forward equation `s_tp_1` directly, instead of
    // routing through `s_tp_eqm_single_phase`. The (T,p) region router returns
    // `Region4` when the pressure is *exactly* the saturation pressure of the
    // temperature (`pres == p_sat_reg4`), and the Region 4 (T,p) arm is
    // deliberately unsupported (two-phase (T,p) is under-determined without
    // steam quality). At p == p_sat(273.15 K) = 611.213 Pa that made every
    // (p,s) flash panic before it could even classify the point. Calling the
    // Region 1 forward equation here avoids that degeneracy: it is the correct
    // saturated-liquid entropy on the saturation line and the correct
    // compressed-liquid entropy above it. This mirrors the (p,h) sibling in
    // `ph_flash_eqm/validity_range.rs`, which uses `h_tp_1` for the same
    // reason (bead `op-znjx`).
    let lower_bound_entropy = s_tp_1(lower_temp_bound, p);

    if s < lower_bound_entropy {
        return true;
    };

    return false;
}

// making a function to check if p,h value is above the isotherm T = 1073.15K
pub(crate) fn is_above_isotherm_t_1073_15(p: Pressure, s: SpecificHeatCapacity) -> bool {
    // first check if outside pressure range
    if is_outside_pressure_range(p) {
        panic!("outside pressure range");
    };

    let upper_temp_bound = ThermodynamicTemperature::new::<kelvin>(1073.15);

    let upper_bound_entropy = s_tp_eqm_single_phase(upper_temp_bound, p);

    if s > upper_bound_entropy {
        // `dbg!` removed here (bead `op-2d5y`): this is an ordinary `true`
        // return meaning "outside the range", not an error, and it wrote to
        // stderr on every out-of-range query.
        return true;
    };

    return false;
}
