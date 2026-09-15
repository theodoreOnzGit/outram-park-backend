//! # Type 304L stainless steel, high-temperature correlation set (Kim, ANL-75-55)
//!
//! Thermophysical property correlations for **AISI Type 304L austenitic
//! stainless steel** valid from **300 K to 1700 K** (26.85 degC to
//! 1426.85 degC), dispatched from [`SolidMaterial::SteelSS304LHighTemp`].
//!
//! ## Why this exists alongside [`super::ss_304_l`]
//!
//! This module does **not** replace [`super::ss_304_l`], and the two must not
//! be conflated:
//!
//! | | [`super::ss_304_l`] ([`SolidMaterial::SteelSS304L`]) | this module ([`SolidMaterial::SteelSS304LHighTemp`]) |
//! |---|---|---|
//! | Lineage | Zou/Zweibaum splines (ANL/NSE-19/11), Graves et al. (ORNL) | Kim, ANL-75-55 (Argonne) |
//! | Range | 250 K - 1000 K | 300 K - 1700 K |
//! | Density | constant 8030 kg/m^3 | temperature-dependent, 7894 -> 7199 kg/m^3 |
//! | Validated against | CIET natural-circulation data, to ~6% | nothing measured in-workspace; see "Verification status" |
//!
//! The Zou/Zweibaum set is the one the CIET regression tests are validated
//! against and it **must not be changed**. But its 1000 K (726.85 degC) ceiling
//! is too low for HTGR work: the HTR-10's published phase-1 core outlet is
//! 700 degC (973.15 K), leaving only ~27 K of headroom before
//! [`TuasLibError::ThermophysicalPropertyTemperatureRangeError`] is returned,
//! and the 900 degC phase-2 condition is unreachable. This module supplies the
//! wider envelope for those components; existing CIET models keep using
//! [`SolidMaterial::SteelSS304L`] and are unaffected.
//!
//! ## Source
//!
//! Kim, C. S. (1975). *Thermophysical Properties of Stainless Steels*.
//! ANL-75-55, Argonne National Laboratory.
//! <https://www.osti.gov/servlets/purl/4152287>
//!
//! US Government work, distribution unlimited, public domain. Catalogued in
//! this workspace's KOVAN archive (Open tier) at
//! `crates/kovan-literature/open/reports/kim1975-thermophysical-properties-stainless-steels.pdf`.
//!
//! Only the **solid-region** Type 304L equations are implemented here:
//! Eq. (5) specific heat, Eq. (16) density, Eq. (28) thermal conductivity, and
//! Eq. (1) enthalpy (re-referenced to 273.15 K to match this crate's
//! convention). The liquid-region equations are recorded in
//! [`melting_and_liquid_region_notes`] for reference but are not evaluated by
//! any function here.
//!
//! ## Validity: measured versus extrapolated
//!
//! **The whole 300-1700 K range is NOT measured data.** Kim fitted his
//! equations by least squares to experimental data and then *extrapolated*
//! them into the melting range (1670-1730 K), which he states explicitly:
//!
//! - **Enthalpy / specific heat** — experimental data to **~1620 K** for
//!   Type 304L, "least-square techniques, and extrapolated to the melting
//!   range (1670-1730 K)" (report p. 2).
//! - **Thermal conductivity** — experimental data to **~1600 K**; "straight
//!   lines were drawn through these sets of data points and extended to the
//!   melting range" (report p. 15).
//! - **Density** — experimental data over **300-1600 K** (report p. 12,
//!   Table 6).
//!
//! So: **300 K to 1600 K is backed by measured data; 1600 K to 1700 K is
//! Kim's own extrapolation.** Do not describe results above 1600 K as
//! measurement-backed. The upper bound is set at 1700 K — Kim's melting
//! temperature `T_m` — because the solid-region equations are meaningless
//! above it, not because the data extends that far.
//!
//! ## Verification status
//!
//! These correlations are verified only against **Kim's own published tables**
//! (Tables 2, 7 and 10 of ANL-75-55) — that is, the implementation is checked
//! to reproduce the source, which is a *verification* result, not a
//! *validation* one. **No comparison against independent 304L measurements,
//! and no maintainer review, has been done.** Per `RESPONSIBLE_USE.md` this
//! is untrusted AI-assisted draft material until a human reviews it. See the
//! unit tests at the bottom of this file for the measured agreement.

use uom::si::available_energy::joule_per_kilogram;
use uom::si::f64::*;
use uom::si::mass_density::kilogram_per_cubic_meter;
use uom::si::specific_heat_capacity::joule_per_kilogram_kelvin;
use uom::si::thermal_conductivity::watt_per_meter_kelvin;
use uom::si::thermodynamic_temperature::kelvin;

use crate::boussinesq_thermophysical_properties::*;
use crate::tuas_lib_error::TuasLibError;

// ---------------------------------------------------------------------------
// Unit conversion factors
//
// Kim reports in cgs / thermochemical-calorie units. These constants convert
// his published coefficients to SI, and are kept explicit (rather than folded
// into pre-multiplied literals) so a reader can check each coefficient against
// the report without doing arithmetic.
// ---------------------------------------------------------------------------

/// Thermochemical calories per gram-kelvin -> joules per kilogram-kelvin.
///
/// The thermochemical calorie is defined as exactly 4.184 J, so
/// 1 cal/(g K) = 4.184 J / (1e-3 kg * K) = 4184 J/(kg K).
/// The same factor converts cal/g -> J/kg for specific enthalpy.
const CAL_PER_GRAM_TO_JOULE_PER_KILOGRAM: f64 = 4184.0;

/// Grams per cubic centimetre -> kilograms per cubic metre.
const GRAM_PER_CM3_TO_KG_PER_M3: f64 = 1000.0;

/// Watts per centimetre-kelvin -> watts per metre-kelvin.
const WATT_PER_CM_KELVIN_TO_WATT_PER_METER_KELVIN: f64 = 100.0;

// ---------------------------------------------------------------------------
// Kim's Type 304L solid-region coefficients, exactly as published
// ---------------------------------------------------------------------------

/// Eq. (5) constant term of specific heat, in cal/(g K).
const CP_A_CAL_PER_GRAM_KELVIN: f64 = 0.1122;

/// Eq. (5) linear-in-temperature coefficient of specific heat,
/// in cal/(g K) per K.
const CP_B_CAL_PER_GRAM_KELVIN_SQUARED: f64 = 3.222e-5;

/// Eq. (16) constant term of density, in g/cm^3.
const RHO_A_GRAM_PER_CM3: f64 = 7.9841;

/// Eq. (16) linear coefficient of density, in (g/cm^3)/K. Subtracted.
const RHO_B_GRAM_PER_CM3_PER_KELVIN: f64 = 2.6506e-4;

/// Eq. (16) quadratic coefficient of density, in (g/cm^3)/K^2. Subtracted.
const RHO_C_GRAM_PER_CM3_PER_KELVIN_SQUARED: f64 = 1.1580e-7;

/// Eq. (28) constant term of thermal conductivity, in W/(cm K).
const K_A_WATT_PER_CM_KELVIN: f64 = 8.116e-2;

/// Eq. (28) linear coefficient of thermal conductivity, in W/(cm K) per K.
const K_B_WATT_PER_CM_KELVIN_SQUARED: f64 = 1.618e-4;

/// Reference temperature at which specific enthalpy is defined to be zero,
/// in kelvin — 273.15 K (0 degrees Celsius).
///
/// This is **this crate's** convention (see
/// [`crate::boussinesq_thermophysical_properties::specific_enthalpy`]), not
/// Kim's. Kim's Eq. (1) is referenced to 298.15 K instead; the two differ by a
/// constant offset only, so enthalpy *differences* are identical either way.
const ENTHALPY_REFERENCE_TEMPERATURE_KELVIN: f64 = 273.15;

/// Upper bound of the coded validity range, in kelvin — 1700 K
/// (1426.85 degC), Kim's melting temperature `T_m` for Type 304L.
const MAX_TEMPERATURE_KELVIN: f64 = 1700.0;

/// Lower bound of the coded validity range, in kelvin — 300 K
/// (26.85 degC), the lowest temperature Kim tabulates.
const MIN_TEMPERATURE_KELVIN: f64 = 300.0;

/// Highest temperature backed by experimental data rather than by Kim's
/// extrapolation, in kelvin — 1600 K (1326.85 degC).
///
/// Taken as the most conservative of the three property data ranges: enthalpy
/// to ~1620 K, thermal conductivity to ~1600 K, density to ~1600 K. Above this
/// the correlations still evaluate (they are coded valid to 1700 K) but they
/// are the author's extrapolation into the melting range.
pub const MAX_MEASURED_DATA_TEMPERATURE_KELVIN: f64 = 1600.0;

// ---------------------------------------------------------------------------
// Range bounds
// ---------------------------------------------------------------------------

/// Returns the maximum temperature of the Kim ANL-75-55 Type 304L
/// solid-region correlations: **1700 K** (1426.85 degC).
///
/// This is Kim's melting temperature `T_m`. Note that only up to
/// [`MAX_MEASURED_DATA_TEMPERATURE_KELVIN`] (1600 K) is backed by experimental
/// data; 1600-1700 K is Kim's least-squares extrapolation into the melting
/// range (1670-1730 K). See the module documentation.
#[inline]
pub fn max_temp_ss_304l_high_temp_kim() -> ThermodynamicTemperature {
    ThermodynamicTemperature::new::<kelvin>(MAX_TEMPERATURE_KELVIN)
}

/// Returns the minimum temperature of the Kim ANL-75-55 Type 304L
/// solid-region correlations: **300 K** (26.85 degC).
///
/// Below this, use [`SolidMaterial::SteelSS304L`], whose Zou/Zweibaum splines
/// reach down to 250 K.
#[inline]
pub fn min_temp_ss_304l_high_temp_kim() -> ThermodynamicTemperature {
    ThermodynamicTemperature::new::<kelvin>(MIN_TEMPERATURE_KELVIN)
}

// ---------------------------------------------------------------------------
// Properties
// ---------------------------------------------------------------------------

/// Returns the **mass density** of Type 304L stainless steel, in kg/m^3.
///
/// Unlike [`super::ss_304_l::steel_ss_304_l_density`] (a constant
/// 8030 kg/m^3), this is temperature-dependent, falling from about
/// 7894 kg/m^3 at 300 K to about 7199 kg/m^3 at 1700 K.
///
/// # Correlation
///
/// Kim ANL-75-55, **Eq. (16)**, solid region:
///
/// ```text
/// rho = 7.9841 - 2.6506e-4 * T - 1.1580e-7 * T^2      [g/cm^3], T in K
/// ```
///
/// converted here to kg/m^3 by a factor of 1000.
///
/// # Arguments
///
/// * `temperature` — the steel temperature. Must lie between 300 K
///   (26.85 degC) and 1700 K (1426.85 degC).
///
/// # Errors
///
/// Returns [`TuasLibError::ThermophysicalPropertyTemperatureRangeError`] if
/// `temperature` falls outside 300-1700 K.
///
/// # Validity
///
/// Fitted by least squares to experimental density data over 300-1600 K
/// (Kim's Table 6, experimental column) and extrapolated to 1700 K.
#[inline]
pub fn steel_304_l_high_temp_density_kim(
    temperature: ThermodynamicTemperature,
) -> Result<MassDensity, TuasLibError> {
    range_check(
        &Material::Solid(SolidMaterial::SteelSS304LHighTemp),
        temperature,
        max_temp_ss_304l_high_temp_kim(),
        min_temp_ss_304l_high_temp_kim(),
    )?;

    let temperature_value_kelvin: f64 = temperature.get::<kelvin>();

    let density_gram_per_cm3 = RHO_A_GRAM_PER_CM3
        - RHO_B_GRAM_PER_CM3_PER_KELVIN * temperature_value_kelvin
        - RHO_C_GRAM_PER_CM3_PER_KELVIN_SQUARED
            * temperature_value_kelvin
            * temperature_value_kelvin;

    Ok(MassDensity::new::<kilogram_per_cubic_meter>(
        density_gram_per_cm3 * GRAM_PER_CM3_TO_KG_PER_M3,
    ))
}

/// Returns the **constant-pressure specific heat capacity** of Type 304L
/// stainless steel, in J/(kg K).
///
/// Rises from about 510 J/(kg K) at 300 K to about 699 J/(kg K) at 1700 K.
///
/// # Correlation
///
/// Kim ANL-75-55, **Eq. (5)**, solid region:
///
/// ```text
/// c_p = 0.1122 + 3.222e-5 * T        [cal/(g K)], T in K
/// ```
///
/// converted here to J/(kg K) by a factor of 4184 (the thermochemical
/// calorie is exactly 4.184 J).
///
/// # Arguments
///
/// * `temperature` — the steel temperature. Must lie between 300 K
///   (26.85 degC) and 1700 K (1426.85 degC).
///
/// # Errors
///
/// Returns [`TuasLibError::ThermophysicalPropertyTemperatureRangeError`] if
/// `temperature` falls outside 300-1700 K.
///
/// # Validity
///
/// Derived by Kim from enthalpy data measured to ~1620 K, then extrapolated
/// into the melting range. This is the **solid-region** `c_p` only; the liquid
/// value is a constant 0.190 cal/(g K) (795 J/(kg K)) and is not evaluated
/// here — see [`melting_and_liquid_region_notes`].
#[inline]
pub fn steel_304_l_high_temp_specific_heat_capacity_kim(
    temperature: ThermodynamicTemperature,
) -> Result<SpecificHeatCapacity, TuasLibError> {
    range_check(
        &Material::Solid(SolidMaterial::SteelSS304LHighTemp),
        temperature,
        max_temp_ss_304l_high_temp_kim(),
        min_temp_ss_304l_high_temp_kim(),
    )?;

    let temperature_value_kelvin: f64 = temperature.get::<kelvin>();

    let cp_cal_per_gram_kelvin =
        CP_A_CAL_PER_GRAM_KELVIN + CP_B_CAL_PER_GRAM_KELVIN_SQUARED * temperature_value_kelvin;

    Ok(SpecificHeatCapacity::new::<joule_per_kilogram_kelvin>(
        cp_cal_per_gram_kelvin * CAL_PER_GRAM_TO_JOULE_PER_KILOGRAM,
    ))
}

/// Returns the **thermal conductivity** of Type 304L stainless steel,
/// in W/(m K).
///
/// Rises linearly from about 12.97 W/(m K) at 300 K to about 35.62 W/(m K)
/// at 1700 K.
///
/// # Correlation
///
/// Kim ANL-75-55, **Eq. (28)**, solid region:
///
/// ```text
/// k = 8.116e-2 + 1.618e-4 * T        [W/(cm K)], T in K
/// ```
///
/// converted here to W/(m K) by a factor of 100.
///
/// # Arguments
///
/// * `temperature` — the steel temperature. Must lie between 300 K
///   (26.85 degC) and 1700 K (1426.85 degC).
///
/// # Errors
///
/// Returns [`TuasLibError::ThermophysicalPropertyTemperatureRangeError`] if
/// `temperature` falls outside 300-1700 K.
///
/// # Validity
///
/// Kim drew straight lines through experimental conductivity data available
/// to ~1600 K and extended them to the melting range, so 1600-1700 K is
/// extrapolation.
#[inline]
pub fn steel_304_l_high_temp_thermal_conductivity_kim(
    temperature: ThermodynamicTemperature,
) -> Result<ThermalConductivity, TuasLibError> {
    range_check(
        &Material::Solid(SolidMaterial::SteelSS304LHighTemp),
        temperature,
        max_temp_ss_304l_high_temp_kim(),
        min_temp_ss_304l_high_temp_kim(),
    )?;

    let temperature_value_kelvin: f64 = temperature.get::<kelvin>();

    let k_watt_per_cm_kelvin =
        K_A_WATT_PER_CM_KELVIN + K_B_WATT_PER_CM_KELVIN_SQUARED * temperature_value_kelvin;

    Ok(ThermalConductivity::new::<watt_per_meter_kelvin>(
        k_watt_per_cm_kelvin * WATT_PER_CM_KELVIN_TO_WATT_PER_METER_KELVIN,
    ))
}

/// Returns the **specific enthalpy** of Type 304L stainless steel, in J/kg,
/// taking h = 0 J/kg at 273.15 K (0 degrees Celsius).
///
/// # Correlation
///
/// The analytic integral of the Eq. (5) specific heat, which is Kim's
/// **Eq. (1)** re-referenced from his 298.15 K datum to this crate's 273.15 K
/// datum:
///
/// ```text
/// h(T) = 0.1122 * (T - 273.15) + 1.611e-5 * (T^2 - 273.15^2)   [cal/g]
/// ```
///
/// where 1.611e-5 is exactly half of the Eq. (5) slope 3.222e-5, as it must be
/// for the integral of a linear `c_p`. Converted here to J/kg by a factor of
/// 4184. Kim's own published form,
/// `H_T - H_298.15 = -34.885 + 0.1122 T + 1.611e-5 T^2`, is the same curve
/// with a different additive constant.
///
/// # Arguments
///
/// * `temperature` — the steel temperature, nominally within 300-1700 K.
///
/// # Range behaviour
///
/// This function deliberately performs **no range check** and returns a bare
/// [`AvailableEnergy`] rather than a `Result`, matching the signature the
/// crate's [`specific_enthalpy`] dispatch requires of every solid. Because the
/// correlation is a plain quadratic it extrapolates smoothly and monotonically
/// outside 300-1700 K rather than failing — which is intentional here, since
/// solid-array energy solvers call this on intermediate iterates that may
/// briefly stray outside the range. Bounds are still enforced on the `c_p`,
/// density and conductivity paths. Values far outside 300-1700 K are
/// arithmetically valid but physically meaningless.
#[inline]
pub fn steel_304_l_high_temp_specific_enthalpy_kim(
    temperature: ThermodynamicTemperature,
) -> AvailableEnergy {
    let temperature_value_kelvin: f64 = temperature.get::<kelvin>();
    let reference = ENTHALPY_REFERENCE_TEMPERATURE_KELVIN;

    // integral of (a + b T) dT from reference to T
    //   = a (T - T_ref) + (b/2) (T^2 - T_ref^2)
    let enthalpy_cal_per_gram = CP_A_CAL_PER_GRAM_KELVIN * (temperature_value_kelvin - reference)
        + 0.5
            * CP_B_CAL_PER_GRAM_KELVIN_SQUARED
            * (temperature_value_kelvin * temperature_value_kelvin - reference * reference);

    AvailableEnergy::new::<joule_per_kilogram>(
        enthalpy_cal_per_gram * CAL_PER_GRAM_TO_JOULE_PER_KILOGRAM,
    )
}

/// Returns the **temperature** of Type 304L stainless steel, in kelvin, given
/// its specific enthalpy in J/kg (referenced to h = 0 at 273.15 K).
///
/// This is the exact analytic inverse of
/// [`steel_304_l_high_temp_specific_enthalpy_kim`]. Because that enthalpy is a
/// quadratic in temperature, the inverse is the positive root of the quadratic
/// formula — no spline, no Brent-Dekker iteration, and no failure mode. That
/// makes it both cheaper and more robust than the root-finding inverses used
/// for the tabulated materials in this database.
///
/// # Derivation
///
/// With `a = 0.1122`, `b/2 = 1.611e-5` and `T_ref = 273.15`, solving
/// `h = a (T - T_ref) + (b/2)(T^2 - T_ref^2)` for `T` gives
///
/// ```text
/// T = [ -a + sqrt( a^2 + 4 (b/2) ( a T_ref + (b/2) T_ref^2 + h ) ) ] / (2 (b/2))
/// ```
///
/// taking the positive root, which is the physical branch for `T > 0`.
///
/// # Arguments
///
/// * `h_steel` — specific enthalpy in J/kg, with h = 0 at 273.15 K.
///
/// # Range behaviour
///
/// Performs no range check, matching the crate's solid enthalpy-inverse
/// dispatch signature (which returns a bare temperature, not a `Result`). The
/// discriminant stays positive for every enthalpy above roughly -0.9 MJ/kg,
/// far below any physically reachable value, so this does not produce NaN in
/// practice.
#[inline]
pub fn steel_304_l_high_temp_temp_from_specific_enthalpy_kim(
    h_steel: AvailableEnergy,
) -> ThermodynamicTemperature {
    let enthalpy_cal_per_gram =
        h_steel.get::<joule_per_kilogram>() / CAL_PER_GRAM_TO_JOULE_PER_KILOGRAM;

    let linear_coefficient = CP_A_CAL_PER_GRAM_KELVIN;
    let quadratic_coefficient = 0.5 * CP_B_CAL_PER_GRAM_KELVIN_SQUARED;
    let reference = ENTHALPY_REFERENCE_TEMPERATURE_KELVIN;

    // constant term of  quadratic_coefficient T^2 + linear_coefficient T - c = 0
    let constant_term = linear_coefficient * reference
        + quadratic_coefficient * reference * reference
        + enthalpy_cal_per_gram;

    let discriminant =
        linear_coefficient * linear_coefficient + 4.0 * quadratic_coefficient * constant_term;

    let temperature_kelvin =
        (-linear_coefficient + discriminant.sqrt()) / (2.0 * quadratic_coefficient);

    ThermodynamicTemperature::new::<kelvin>(temperature_kelvin)
}

/// Melting and liquid-region data from Kim ANL-75-55, recorded for reference.
///
/// **Nothing in this module evaluates these** — only the solid region is
/// implemented, and [`SolidMaterial::SteelSS304LHighTemp`] is bounded at
/// 1700 K precisely so that the liquid region is never entered. This function
/// exists so the numbers are documented where a reader will meet them rather
/// than being lost in a commit message.
///
/// For Type 304L:
///
/// - Melting range **1670-1730 K**, with `T_m` = **1700 K**.
/// - Heat of fusion **64.0 cal/g** = **267.8 kJ/kg**.
/// - Liquid specific heat, constant, **0.190 cal/(g K)** = **795 J/(kg K)**
///   (Kim's Eq. 9 enthalpy slope).
/// - Liquid density, Eq. (17):
///   `rho = 7.5512 - 1.1167e-4 T - 1.5063e-7 T^2` [g/cm^3].
/// - Liquid thermal conductivity, Eq. (29):
///   `k = 1.229e-1 + 3.248e-5 T` [W/(cm K)].
///
/// # OCR correction on Eq. (29) — deliberate, and verified
///
/// The scanned report renders the Eq. (29) slope as **`3.248e-3`**. That is an
/// OCR error in the superscript and **`3.248e-5` is correct**. Two independent
/// checks confirm it:
///
/// 1. **Kim's own stated rule.** He writes that "at the melting points, the
///    thermal conductivities of solid steels were reduced by half to give the
///    values in the liquid state". Solid Eq. (28) at `T_m` = 1700 K gives
///    0.356220 W/(cm K), half of which is **0.178110**. Eq. (29) with
///    3.248e-5 gives **0.178116** — agreement to six significant figures.
///    With 3.248e-3 it gives 5.6445, off by a factor of ~32.
/// 2. **The parallel Type 316L equation.** Eq. (31) is
///    `k = 1.241e-1 + 3.279e-5 T`, whose exponent survived OCR intact, and
///    which satisfies the same halving rule at `T_m`.
///
/// Kim's own Table 10 also lists the 304L liquid conductivity at 1700 K as
/// 0.1781 W/(cm K), matching the corrected form.
#[inline]
pub fn melting_and_liquid_region_notes() {}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

/// **Verification of density, Eq. (16), against Kim's own Table 7.**
///
/// *Methodology.* Kim's Table 7 ("Densities and Linear Expansion Coefficients
/// of Stainless Steels Type 304L and Type 316L", ANL-75-55 p. 14) lists the
/// author's recommended Type 304L densities at 100 K intervals from 300 K to
/// 1700 K(s), printed to four significant figures in g/cm^3. This test
/// evaluates [`steel_304_l_high_temp_density_kim`] at all 15 of those
/// temperatures and compares against the tabulated values. Because the table
/// is printed to 4 s.f., round-off alone permits ~0.007% deviation; the pass
/// criterion is therefore **0.05% relative** on every point, and the test also
/// asserts none of the calls returns a range error.
///
/// *Results (measured 2026-08-13, run under `cargo test --release`).* All 15
/// points pass. The **maximum deviation is 0.0061%** (at 1600 K: correlation
/// 7.26356 g/cm^3 versus tabulated 7.264 g/cm^3), and the deviation is below
/// 0.007% everywhere, i.e. entirely attributable to the table's print
/// precision. Endpoints: 300 K gives 7894.16 kg/m^3 versus 7.894 g/cm^3
/// tabulated; 1700 K gives 7198.84 kg/m^3 versus 7.199 g/cm^3 tabulated.
///
/// *Interpretation.* Eq. (16) is transcribed correctly and reproduces the
/// author's published density table exactly to its printed precision across
/// the full coded range. This verifies the implementation against the source;
/// it is not validation against independent measurements.
#[test]
fn kim_304l_density_reproduces_kim_table_7() {
    // (temperature in K, Kim Table 7 density in g/cm^3)
    let kim_table_7: [(f64, f64); 15] = [
        (300.0, 7.894),
        (400.0, 7.860),
        (500.0, 7.823),
        (600.0, 7.783),
        (700.0, 7.742),
        (800.0, 7.698),
        (900.0, 7.652),
        (1000.0, 7.603),
        (1100.0, 7.552),
        (1200.0, 7.499),
        (1300.0, 7.444),
        (1400.0, 7.386),
        (1500.0, 7.326),
        (1600.0, 7.264),
        (1700.0, 7.199),
    ];

    for (temperature_kelvin, kim_density_gram_per_cm3) in kim_table_7.iter() {
        let temperature = ThermodynamicTemperature::new::<kelvin>(*temperature_kelvin);

        let density = steel_304_l_high_temp_density_kim(temperature)
            .expect("density must not return a range error inside 300-1700 K");

        let density_gram_per_cm3 = density.get::<kilogram_per_cubic_meter>() / 1000.0;

        approx::assert_relative_eq!(
            *kim_density_gram_per_cm3,
            density_gram_per_cm3,
            max_relative = 0.0005
        );
    }
}

/// **Verification of specific heat, Eq. (5), against Kim's own Table 2.**
///
/// *Methodology.* Kim's Table 2 ("Thermodynamic Properties of Stainless Steels
/// Type 304L and Type 316L", ANL-75-55 p. 5) lists the author's Type 304L
/// specific heats at 100 K intervals from 300 K to 1700 K(s), printed to four
/// decimal places in cal/(g K). This test evaluates
/// [`steel_304_l_high_temp_specific_heat_capacity_kim`] at all 15 points,
/// converts back from J/(kg K) to cal/(g K) by dividing by 4184, and compares.
/// Pass criterion: **0.05% relative** on every point (the table's own 4-d.p.
/// print precision allows ~0.04% at the low-temperature end).
///
/// *Results (measured 2026-08-13, run under `cargo test --release`).* All 15
/// points pass. The **maximum deviation is 0.0341%** (at 700 K: correlation
/// 0.134754 cal/(g K) versus tabulated 0.1348), and the largest deviations
/// occur where the printed value has fewest significant digits, as expected
/// from rounding. In SI the correlation gives 509.89 J/(kg K) at 300 K and
/// 698.62 J/(kg K) at 1700 K.
///
/// *Interpretation.* Eq. (5) is transcribed correctly and reproduces Kim's
/// published `c_p` table to its printed precision. Verification against the
/// source only, not validation.
#[test]
fn kim_304l_specific_heat_reproduces_kim_table_2() {
    // (temperature in K, Kim Table 2 specific heat in cal/(g K))
    let kim_table_2: [(f64, f64); 15] = [
        (300.0, 0.1219),
        (400.0, 0.1251),
        (500.0, 0.1283),
        (600.0, 0.1315),
        (700.0, 0.1348),
        (800.0, 0.1380),
        (900.0, 0.1412),
        (1000.0, 0.1444),
        (1100.0, 0.1476),
        (1200.0, 0.1509),
        (1300.0, 0.1541),
        (1400.0, 0.1573),
        (1500.0, 0.1605),
        (1600.0, 0.1638),
        (1700.0, 0.1670),
    ];

    for (temperature_kelvin, kim_cp_cal_per_gram_kelvin) in kim_table_2.iter() {
        let temperature = ThermodynamicTemperature::new::<kelvin>(*temperature_kelvin);

        let cp = steel_304_l_high_temp_specific_heat_capacity_kim(temperature)
            .expect("cp must not return a range error inside 300-1700 K");

        let cp_cal_per_gram_kelvin =
            cp.get::<joule_per_kilogram_kelvin>() / CAL_PER_GRAM_TO_JOULE_PER_KILOGRAM;

        approx::assert_relative_eq!(
            *kim_cp_cal_per_gram_kelvin,
            cp_cal_per_gram_kelvin,
            max_relative = 0.0005
        );
    }
}

/// **Verification of thermal conductivity, Eq. (28), against Kim's Table 10.**
///
/// *Methodology.* Kim's Table 10 ("Thermal Conductivities and Thermal
/// Diffusivities of Stainless Steels Type 304L and Type 316L", ANL-75-55
/// p. 21) lists the author's Type 304L solid-region conductivities at 100 K
/// intervals from 300 K to 1700 K(s), printed to four decimal places in
/// W/(cm K). This test evaluates
/// [`steel_304_l_high_temp_thermal_conductivity_kim`] at all 15 points,
/// converts back from W/(m K) to W/(cm K) by dividing by 100, and compares.
/// Pass criterion: **0.05% relative** on every point.
///
/// *Results (measured 2026-08-13, run under `cargo test --release`).* All 15
/// points pass. The **maximum deviation is 0.0370%** (at 500 K: correlation
/// 0.162060 W/(cm K) versus tabulated 0.1620); at 300 K and 800 K the
/// agreement is exact to all printed digits. In SI the correlation gives
/// 12.970 W/(m K) at 300 K and 35.622 W/(m K) at 1700 K.
///
/// *Interpretation.* Eq. (28) is transcribed correctly and reproduces Kim's
/// published conductivity table to its printed precision. Verification against
/// the source only, not validation.
#[test]
fn kim_304l_thermal_conductivity_reproduces_kim_table_10() {
    // (temperature in K, Kim Table 10 conductivity in W/(cm K))
    let kim_table_10: [(f64, f64); 15] = [
        (300.0, 0.1297),
        (400.0, 0.1459),
        (500.0, 0.1620),
        (600.0, 0.1782),
        (700.0, 0.1944),
        (800.0, 0.2106),
        (900.0, 0.2267),
        (1000.0, 0.2429),
        (1100.0, 0.2591),
        (1200.0, 0.2753),
        (1300.0, 0.2914),
        (1400.0, 0.3076),
        (1500.0, 0.3238),
        (1600.0, 0.3400),
        (1700.0, 0.3561),
    ];

    for (temperature_kelvin, kim_k_watt_per_cm_kelvin) in kim_table_10.iter() {
        let temperature = ThermodynamicTemperature::new::<kelvin>(*temperature_kelvin);

        let thermal_conductivity = steel_304_l_high_temp_thermal_conductivity_kim(temperature)
            .expect("thermal conductivity must not return a range error inside 300-1700 K");

        let k_watt_per_cm_kelvin = thermal_conductivity.get::<watt_per_meter_kelvin>()
            / WATT_PER_CM_KELVIN_TO_WATT_PER_METER_KELVIN;

        approx::assert_relative_eq!(
            *kim_k_watt_per_cm_kelvin,
            k_watt_per_cm_kelvin,
            max_relative = 0.0005
        );
    }
}

/// **Verification of specific enthalpy against Kim's Table 2 enthalpy column.**
///
/// *Methodology.* Kim's Table 2 lists `H_T - H_298.15` for Type 304L in cal/g
/// at 100 K intervals, printed to two decimal places. This crate references
/// enthalpy to 273.15 K instead, so the test forms the *difference*
/// `h(T) - h(298.15 K)` from [`steel_304_l_high_temp_specific_enthalpy_kim`],
/// which cancels the differing datum, and compares to the tabulated value.
/// Pass criterion: **0.01 cal/g absolute** (one unit in the table's last
/// printed place) at all 15 points. The test additionally asserts
/// `h(273.15 K) = 0` exactly, confirming the crate's datum convention.
///
/// *Results (measured 2026-08-13, run under `cargo test --release`).* All 15
/// points pass. The **maximum absolute deviation is 0.0049 cal/g** (at 300 K:
/// 0.2254 versus tabulated 0.23), i.e. under half of one unit in the table's
/// last place at every point. `h(273.15 K)` is exactly 0.0 J/kg. For scale,
/// `h(973.15 K)` — the HTR-10 phase-1 core outlet, 700 degC — is
/// 387 415 J/kg.
///
/// *Interpretation.* The analytic integral of Eq. (5) reproduces Kim's
/// published enthalpy table, confirming both the `c_p` coefficients and the
/// re-referencing from his 298.15 K datum to this crate's 273.15 K datum.
#[test]
fn kim_304l_specific_enthalpy_reproduces_kim_table_2() {
    // enthalpy datum: zero at 273.15 K, this crate's convention
    let datum = steel_304_l_high_temp_specific_enthalpy_kim(
        ThermodynamicTemperature::new::<kelvin>(273.15),
    );
    approx::assert_abs_diff_eq!(datum.get::<joule_per_kilogram>(), 0.0, epsilon = 1e-9);

    // Kim's table is referenced to 298.15 K, so subtract h(298.15 K)
    let kim_reference = steel_304_l_high_temp_specific_enthalpy_kim(
        ThermodynamicTemperature::new::<kelvin>(298.15),
    );

    // (temperature in K, Kim Table 2 enthalpy H_T - H_298.15 in cal/g)
    let kim_table_2_enthalpy: [(f64, f64); 15] = [
        (300.0, 0.23),
        (400.0, 12.57),
        (500.0, 25.24),
        (600.0, 38.24),
        (700.0, 51.55),
        (800.0, 65.19),
        (900.0, 79.14),
        (1000.0, 93.43),
        (1100.0, 108.03),
        (1200.0, 122.95),
        (1300.0, 138.20),
        (1400.0, 153.77),
        (1500.0, 169.66),
        (1600.0, 185.88),
        (1700.0, 202.41),
    ];

    for (temperature_kelvin, kim_enthalpy_cal_per_gram) in kim_table_2_enthalpy.iter() {
        let temperature = ThermodynamicTemperature::new::<kelvin>(*temperature_kelvin);

        let enthalpy_difference =
            steel_304_l_high_temp_specific_enthalpy_kim(temperature) - kim_reference;

        let enthalpy_difference_cal_per_gram =
            enthalpy_difference.get::<joule_per_kilogram>() / CAL_PER_GRAM_TO_JOULE_PER_KILOGRAM;

        approx::assert_abs_diff_eq!(
            *kim_enthalpy_cal_per_gram,
            enthalpy_difference_cal_per_gram,
            epsilon = 0.01
        );
    }
}

/// **Verification that the analytic enthalpy inverse round-trips.**
///
/// *Methodology.* [`steel_304_l_high_temp_temp_from_specific_enthalpy_kim`] is
/// claimed to be the exact algebraic inverse of
/// [`steel_304_l_high_temp_specific_enthalpy_kim`], not an iterative
/// approximation. This test walks ten temperatures spanning the coded range
/// (300 K to 1700 K, including 973.15 K = the HTR-10 phase-1 outlet and
/// 1473.15 K = the maintainer's 1200 degC requirement), maps each to enthalpy
/// and back, and requires the recovered temperature to match the original.
/// Pass criterion: **1e-9 K absolute** — a tolerance only an exact inverse can
/// meet, chosen deliberately so that silently substituting an iterative solver
/// would fail this test.
///
/// *Results (measured 2026-08-13, run under `cargo test --release`).* All ten
/// points pass. The **worst observed round-trip error is 4.5e-13 K**, i.e.
/// pure f64 round-off, some four orders of magnitude inside the tolerance and
/// eleven orders below any physically meaningful temperature resolution.
/// Several points round-trip to exactly 0.0 K error.
///
/// *Interpretation.* The quadratic-formula inverse is exact, so no
/// root-finding tolerance or bracketing failure can enter the
/// enthalpy-to-temperature path for this material — unlike the Brent-Dekker
/// inverse used for the Zou/Zweibaum spline material.
#[test]
fn kim_304l_enthalpy_temperature_round_trip_is_exact() {
    let temperature_values_kelvin: [f64; 10] = [
        300.0, 400.0, 500.0, 700.0, 973.15, 1000.0, 1200.0, 1473.15, 1600.0, 1700.0,
    ];

    for temperature_value_kelvin in temperature_values_kelvin.iter() {
        let temperature = ThermodynamicTemperature::new::<kelvin>(*temperature_value_kelvin);

        let enthalpy = steel_304_l_high_temp_specific_enthalpy_kim(temperature);

        let recovered = steel_304_l_high_temp_temp_from_specific_enthalpy_kim(enthalpy);

        approx::assert_abs_diff_eq!(
            recovered.get::<kelvin>(),
            *temperature_value_kelvin,
            epsilon = 1e-9
        );
    }
}

/// **Verification of the HTGR temperature envelope through the public API.**
///
/// *Methodology.* This is the test that speaks directly to the defect this
/// material was added for (issue `op-x0v1`, blocking `op-m1jz`): the
/// Zou/Zweibaum [`SolidMaterial::SteelSS304L`] correlations stop at 1000 K
/// (726.85 degC), only ~27 K above the HTR-10's published 700 degC phase-1
/// core outlet, so HTGR transients leave the range and the property call
/// fails. The test drives the **public generic entry points**
/// ([`density::try_get_rho`], [`specific_heat_capacity::try_get_cp`],
/// [`thermal_conductivity::try_get_kappa_thermal_conductivity`]) — not the
/// module-private correlations — with `Material::Solid(SteelSS304LHighTemp)`
/// at four temperatures: 300 K (lower bound), 1000 K (where the old material
/// stops), 1473.15 K (1200 degC, the maintainer's stated requirement) and
/// 1700 K (upper bound). It requires every call to return `Ok`, and asserts
/// each property lands inside a physically sensible band for austenitic
/// stainless steel. It also asserts that 299 K and 1701 K **do** produce
/// [`TuasLibError::ThermophysicalPropertyTemperatureRangeError`], so the
/// bounds are proven live rather than merely declared.
///
/// *Results (measured 2026-08-13, run under `cargo test --release`).* No call
/// in 300-1700 K returned a range error. Measured values:
///
/// | T (K) | T (degC) | rho (kg/m^3) | c_p (J/(kg K)) | k (W/(m K)) |
/// |---|---|---|---|---|
/// | 300.00 | 26.85 | 7894.16 | 509.89 | 12.970 |
/// | 1000.00 | 726.85 | 7603.24 | 604.25 | 24.296 |
/// | 1473.15 | 1200.00 | 7342.32 | 668.04 | 31.952 |
/// | 1700.00 | 1426.85 | 7198.84 | 698.62 | 35.622 |
///
/// Density falls monotonically 7.894 -> 7.199 g/cm^3, `c_p` rises 510 -> 699
/// J/(kg K), and `k` rises 12.97 -> 35.62 W/(m K) — all characteristic of
/// austenitic stainless steel over this span. Both out-of-range probes
/// returned the expected range error.
///
/// *Interpretation.* The new variant covers the full HTR-10 envelope including
/// the 900 degC phase-2 outlet (1173.15 K) and the 1200 degC requirement, with
/// 227 K of headroom above the latter, and does so through the same public API
/// the simulators already call. Note this verifies the *range and dispatch*;
/// the property values themselves are verified against Kim's tables by the
/// four tests above, and remain unvalidated against independent measurements.
#[test]
fn kim_304l_covers_htgr_envelope_through_public_api() {
    use crate::boussinesq_thermophysical_properties::density::try_get_rho;
    use crate::boussinesq_thermophysical_properties::specific_heat_capacity::try_get_cp;
    use crate::boussinesq_thermophysical_properties::thermal_conductivity::try_get_kappa_thermal_conductivity;
    use uom::si::pressure::atmosphere;
    use uom::si::thermodynamic_temperature::degree_celsius;

    let steel = Material::Solid(SolidMaterial::SteelSS304LHighTemp);
    let pressure = Pressure::new::<atmosphere>(1.0);

    // 1473.15 K is 1200 degC, the maintainer's stated requirement
    let temperature_values_kelvin: [f64; 4] = [300.0, 1000.0, 1473.15, 1700.0];

    let mut previous_density_kg_per_m3 = f64::INFINITY;

    for temperature_value_kelvin in temperature_values_kelvin.iter() {
        let temperature = ThermodynamicTemperature::new::<kelvin>(*temperature_value_kelvin);

        let density = try_get_rho(steel, temperature, pressure)
            .expect("density must be available across the whole HTGR envelope");
        let cp = try_get_cp(steel, temperature, pressure)
            .expect("cp must be available across the whole HTGR envelope");
        let thermal_conductivity = try_get_kappa_thermal_conductivity(steel, temperature, pressure)
            .expect("conductivity must be available across the whole HTGR envelope");

        let density_kg_per_m3 = density.get::<kilogram_per_cubic_meter>();
        let cp_joule_per_kg_kelvin = cp.get::<joule_per_kilogram_kelvin>();
        let k_watt_per_meter_kelvin = thermal_conductivity.get::<watt_per_meter_kelvin>();

        println!(
            "T = {:8.2} K ({:8.2} degC): rho = {:8.2} kg/m^3, \
             cp = {:7.2} J/(kg K), k = {:7.3} W/(m K)",
            temperature_value_kelvin,
            temperature.get::<degree_celsius>(),
            density_kg_per_m3,
            cp_joule_per_kg_kelvin,
            k_watt_per_meter_kelvin
        );

        // physically sensible bands for austenitic stainless over 300-1700 K
        assert!(
            (7100.0..=7950.0).contains(&density_kg_per_m3),
            "density {density_kg_per_m3} kg/m^3 outside the austenitic-stainless band"
        );
        assert!(
            (500.0..=710.0).contains(&cp_joule_per_kg_kelvin),
            "cp {cp_joule_per_kg_kelvin} J/(kg K) outside the austenitic-stainless band"
        );
        assert!(
            (12.0..=36.0).contains(&k_watt_per_meter_kelvin),
            "k {k_watt_per_meter_kelvin} W/(m K) outside the austenitic-stainless band"
        );

        // density must fall monotonically with temperature
        assert!(
            density_kg_per_m3 < previous_density_kg_per_m3,
            "density must decrease monotonically with temperature"
        );
        previous_density_kg_per_m3 = density_kg_per_m3;
    }

    // the bounds must actually bite, just outside the range
    let below_range = ThermodynamicTemperature::new::<kelvin>(299.0);
    let above_range = ThermodynamicTemperature::new::<kelvin>(1701.0);

    assert!(
        matches!(
            try_get_rho(steel, below_range, pressure),
            Err(TuasLibError::ThermophysicalPropertyTemperatureRangeError { .. })
        ),
        "299 K must be rejected as below the 300 K lower bound"
    );
    assert!(
        matches!(
            try_get_rho(steel, above_range, pressure),
            Err(TuasLibError::ThermophysicalPropertyTemperatureRangeError { .. })
        ),
        "1701 K must be rejected as above the 1700 K upper bound"
    );
}

/// **Verification that the existing SteelSS304L material is untouched.**
///
/// *Methodology.* The hard constraint on issue `op-x0v1` is that adding this
/// material must not perturb [`SolidMaterial::SteelSS304L`], whose
/// Zou/Zweibaum correlations underpin CIET regression tests validated against
/// published Zweibaum (2015) and Zou et al. (2019) SAM data to ~6%. A silent
/// change there would move validated results without any test failing loudly.
/// This test pins the old material's behaviour from the *outside*: it asserts
/// its constant density is still 8030 kg/m^3, its `c_p` at 350 K is still
/// 469.4894 J/(kg K), its coded bounds are still 250-1000 K, and — critically
/// — that it still **rejects** 1100 K. That last assertion is what would fail
/// if someone "helpfully" widened the old material's range instead of using
/// this new one.
///
/// It then asserts the two materials are genuinely distinct: at 800 K, a
/// temperature both cover, their densities differ substantially, confirming
/// the dispatch is not accidentally aliasing one to the other.
///
/// *Results (measured 2026-08-13, run under `cargo test --release`).* All
/// assertions pass. `SteelSS304L` density is 8030.00 kg/m^3 (constant,
/// unchanged); its `c_p` at 350 K is 469.49 J/(kg K); it still returns
/// [`TuasLibError::ThermophysicalPropertyTemperatureRangeError`] at 1100 K.
/// At 800 K the two materials give **8030.00 kg/m^3** (old, constant) versus
/// **7697.94 kg/m^3** (Kim) — a 4.1% difference, as expected since the old
/// material ignores thermal expansion entirely.
///
/// *Interpretation.* The Zou/Zweibaum correlations and their 250-1000 K bounds
/// are unmodified, so no CIET regression result can have moved. The 4.1%
/// density gap at 800 K is not a defect in either material: it is the known
/// cost of the old material's constant-density simplification, and is a reason
/// to prefer this variant for high-temperature work rather than a reason to
/// distrust it.
#[test]
fn existing_ss304l_zou_zweibaum_material_is_unchanged() {
    use crate::boussinesq_thermophysical_properties::density::try_get_rho;
    use crate::boussinesq_thermophysical_properties::specific_heat_capacity::try_get_cp;
    use uom::si::pressure::atmosphere;

    let old_steel = Material::Solid(SolidMaterial::SteelSS304L);
    let pressure = Pressure::new::<atmosphere>(1.0);

    // constant density, 8030 kg/m^3, unchanged
    let old_density = try_get_rho(
        old_steel,
        ThermodynamicTemperature::new::<kelvin>(396.0),
        pressure,
    )
    .unwrap();
    approx::assert_relative_eq!(
        8030.0,
        old_density.get::<kilogram_per_cubic_meter>(),
        max_relative = 1e-12
    );

    // cp at 350 K, unchanged
    let old_cp = try_get_cp(
        old_steel,
        ThermodynamicTemperature::new::<kelvin>(350.0),
        pressure,
    )
    .unwrap();
    approx::assert_relative_eq!(
        469.4894,
        old_cp.get::<joule_per_kilogram_kelvin>(),
        max_relative = 0.0055
    );

    // bounds still 250-1000 K
    approx::assert_relative_eq!(
        1000.0,
        SolidMaterial::SteelSS304L.max_temperature().get::<kelvin>(),
        max_relative = 1e-12
    );
    approx::assert_relative_eq!(
        250.0,
        SolidMaterial::SteelSS304L.min_temperature().get::<kelvin>(),
        max_relative = 1e-12
    );

    // and it must STILL reject 1100 K -- if this ever passes, someone widened
    // the Zou/Zweibaum range instead of using SteelSS304LHighTemp
    assert!(
        matches!(
            try_get_cp(
                old_steel,
                ThermodynamicTemperature::new::<kelvin>(1100.0),
                pressure
            ),
            Err(TuasLibError::ThermophysicalPropertyTemperatureRangeError { .. })
        ),
        "SteelSS304L must still reject 1100 K; its 250-1000 K range is load-bearing \
         for the CIET regression tests"
    );

    // the two materials are distinct at a temperature both cover
    let shared_temperature = ThermodynamicTemperature::new::<kelvin>(800.0);
    let old_density_800k = try_get_rho(old_steel, shared_temperature, pressure)
        .unwrap()
        .get::<kilogram_per_cubic_meter>();
    let kim_density_800k = try_get_rho(
        Material::Solid(SolidMaterial::SteelSS304LHighTemp),
        shared_temperature,
        pressure,
    )
    .unwrap()
    .get::<kilogram_per_cubic_meter>();

    println!(
        "at 800 K: SteelSS304L rho = {old_density_800k:.2} kg/m^3, \
         SteelSS304LHighTemp rho = {kim_density_800k:.2} kg/m^3"
    );

    assert!(
        (old_density_800k - kim_density_800k).abs() > 100.0,
        "the two steel materials must be distinct; the constant-density old \
         material and the Kim correlation should differ by ~330 kg/m^3 at 800 K"
    );
}
