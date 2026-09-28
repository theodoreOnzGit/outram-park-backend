//! # Nuclear graphite (HTR-10 / HTR-PM pebble matrix A3, and IG-110)
//!
//! Thermophysical property correlations for two grades of nuclear graphite:
//!
//! - **Matrix graphite, A3 grade** — the fuel-pebble matrix graphite of the
//!   HTR-10 / HTR-PM pebble-bed reactors
//!   ([`SolidMaterial::NuclearGraphiteMatrixA3`]), and its
//!   **high-temperature** sibling
//!   ([`SolidMaterial::NuclearGraphiteMatrixA3HighTemp`], 300-3000 K, added
//!   2026-09-28): Butland & Maddison polynomial cp (backed to 3000 K) with the
//!   same conductivity **extrapolated above 2000 K**.
//! - **IG-110** — the fine-grained isotropic reflector-grade graphite used in
//!   the HTTR and HTR-10 reflector structures
//!   ([`SolidMaterial::NuclearGraphiteIG110`]).
//!
//! **No high-temperature IG-110 variant, deliberately (2026-09-28).** The
//! IG-110 quadratic `66.32 - 4.994e-2 T + 1.712e-5 T^2` has its minimum at
//! T = 4.994e-2 / (2 x 1.712e-5) = 1458.5 K (k = 29.9 W/(m K)) and rises
//! from there — to 34.9 at 2000 K and 70.6 at 3000 K. Extending it would
//! return a conductivity that more than doubles past its minimum, an artefact
//! of fitting a parabola, so the extension was checked and rejected rather
//! than flagged. Reflector graphite stays far below 2000 K in every transient
//! `htgr_sim_v1` runs, so the base window suffices there.
//!
//! All correlations are transcribed from the openly licensed **Virtual Test
//! Bed (VTB)** input decks vendored in this workspace under
//! `reference-data/virtual_test_bed/` (CC-BY-4.0, Open tier), plus two Open
//! tier literature values for density. The exact deck file and line numbers
//! are cited on each function.
//!
//! Both grades share one specific-heat-capacity table (Butland & Maddison):
//! nuclear graphite cp is treated as grade-insensitive because all grades are
//! polycrystalline graphite and cp is dominated by the phonon spectrum of
//! graphite itself, not by grade-specific porosity or grain structure.
//! Thermal conductivity, by contrast, is strongly grade- and
//! irradiation-dependent, so each grade has its own correlation, each with an
//! optional fast-neutron-fluence damage factor.
//!
//! The enum arms of [`SolidMaterial`] dispatch to the **unirradiated /
//! zero-fluence** forms. The fluence-dependent free functions exist for
//! consumers (e.g. decay-heat / irradiated-core studies) that track fast
//! fluence themselves and want the degraded conductivity.
//!
//! **None of these correlations has been checked against HTR-10 measurements
//! by the maintainer** — they are transcriptions of the VTB decks and cited
//! literature values, verified only against hand evaluations of the same
//! formulas (see the unit tests at the bottom of this file).

use peroxide::fuga::{Calculus, CubicSpline, Spline};
use roots::{find_root_brent, SimpleConvergency};
use uom::si::available_energy::joule_per_kilogram;
use uom::si::f64::*;
use uom::si::length::micrometer;
use uom::si::mass_density::kilogram_per_cubic_meter;
use uom::si::ratio::ratio;
use uom::si::specific_heat_capacity::joule_per_kilogram_kelvin;
use uom::si::thermal_conductivity::watt_per_meter_kelvin;
use uom::si::thermodynamic_temperature::kelvin;
use crate::boussinesq_thermophysical_properties::*;
use crate::tuas_lib_error::TuasLibError;

/// Returns the mass density of A3-grade pebble matrix graphite,
/// 1730 kg/m^3 (1.73 g/cm^3), treated as temperature-independent.
///
/// Source: IAEA-TECDOC-1382 ("Evaluation of high temperature gas cooled
/// reactor performance: benchmark analysis related to initial testing of the
/// HTTR and HTR-10"), Chapter 4 — HTR-10 fuel-element matrix graphite density
/// 1.73 g/cm^3. Open tier (public IAEA benchmark document).
///
/// Note: the VTB HTR-PM pebble deck
/// (`reference-data/virtual_test_bed/htgr/htr-pm/core-multiphysics/updated_equilibrium_core/pebble_triso.i`,
/// line 193) embeds a matrix density of 1740 kg/m^3 inside its
/// conductivity Maxwell factor, and the VTB GPBR200 decks likewise use
/// 1740 kg/m^3. The 0.6% difference from the 1730 kg/m^3 returned here is
/// far below the uncertainty of any downstream thermal calculation.
#[inline]
pub fn nuclear_graphite_matrix_a3_density() -> Result<MassDensity, TuasLibError> {
    Ok(MassDensity::new::<kilogram_per_cubic_meter>(1730.0))
}

/// Returns the mass density of IG-110 reflector-grade graphite,
/// 1770 kg/m^3, treated as temperature-independent.
///
/// Source: the VTB HTTR deck
/// (`reference-data/virtual_test_bed/htgr/httr/steady_state_and_null_transient/fuel_elem_steady.i`,
/// line 340) which cites table 1.27 of NEA/NSC/DOC(2006)1 ("Evaluation of
/// High Temperature Gas Cooled Reactor Performance", OECD/NEA). Open tier
/// (CC-BY-4.0 VTB deck citing a public OECD/NEA benchmark document).
#[inline]
pub fn nuclear_graphite_ig_110_density() -> Result<MassDensity, TuasLibError> {
    Ok(MassDensity::new::<kilogram_per_cubic_meter>(1770.0))
}

/// Returns a nominal surface roughness for machined nuclear graphite,
/// 1.95 micrometres.
///
/// This is the same figure the in-workspace gFHR custom-graphite tutorial
/// uses (`src/lib/pre_built_components/insulated_pipes_and_fluid_components/tutorials/tutorial_6.rs`,
/// line 139, `gfhr_pipe_with_custom_graphite_material`), adopted here as the
/// in-workspace precedent. It is a **nominal machined-graphite figure, not a
/// measured HTR-10 value** — treat it as an order-of-magnitude estimate for
/// friction-factor purposes.
#[inline]
pub fn nuclear_graphite_surf_roughness() -> Length {
    Length::new::<micrometer>(1.95)
}

/// Minimum temperature, 300 K, of the coded nuclear-graphite correlations
/// (both grades). This is the lowest node of the Butland & Maddison cp table
/// (see [`nuclear_graphite_specific_heat_capacity_butland_maddison_spline`]).
#[inline]
pub fn min_temp_nuclear_graphite() -> ThermodynamicTemperature {
    ThermodynamicTemperature::new::<kelvin>(300.0)
}

/// Maximum temperature, 2000 K, of the coded nuclear-graphite correlations
/// (both grades). This is the highest node of the Butland & Maddison cp table
/// (see [`nuclear_graphite_specific_heat_capacity_butland_maddison_spline`]).
#[inline]
pub fn max_temp_nuclear_graphite() -> ThermodynamicTemperature {
    ThermodynamicTemperature::new::<kelvin>(2000.0)
}

/// Returns the specific heat capacity, in J/(kg K), of nuclear graphite
/// (both A3 matrix and IG-110 grades) at the given temperature.
///
/// Cubic-spline interpolation of the 18-node G-348 graphite cp table (300 K
/// to 2000 K in 100 K steps) from the VTB HTTF SAM ring model deck
/// (`reference-data/virtual_test_bed/htgr/httf/sam_ring_model/HTTF-SS.i`,
/// lines 368-372, `cpgraphite` block), which cites **Butland, A. T. D. &
/// Maddison, R. J., "The specific heat of graphite: an evaluation of
/// measurements", J. Nucl. Mater. 49 (1973/74) 45-56**. Open tier
/// (CC-BY-4.0 VTB deck citing open literature).
///
/// **Grade-insensitivity assumption:** this one cp table serves both the A3
/// matrix and IG-110 enum variants. All nuclear graphite grades are
/// polycrystalline graphite, and cp is dominated by the phonon spectrum of
/// graphite itself rather than by grade-specific porosity/grain structure,
/// so per-grade cp differences are small compared to the correlation's own
/// uncertainty.
///
/// Valid range: 300 K to 2000 K; outside it, returns
/// `TuasLibError::ThermophysicalPropertyTemperatureRangeError`. (The
/// out-of-range debug print names the `NuclearGraphiteMatrixA3` variant
/// because the table is shared between both graphite variants.)
#[inline]
pub fn nuclear_graphite_specific_heat_capacity_butland_maddison_spline(
    temperature: ThermodynamicTemperature,
) -> Result<SpecificHeatCapacity, TuasLibError> {
    range_check(
        &Material::Solid(SolidMaterial::NuclearGraphiteMatrixA3),
        temperature,
        max_temp_nuclear_graphite(),
        min_temp_nuclear_graphite(),
    )?;

    let temperature_value_kelvin: f64 = temperature.get::<kelvin>();

    let cp_temperature_values_kelvin = c!(
        300.0, 400.0, 500.0, 600.0, 700.0, 800.0, 900.0, 1000.0, 1100.0, 1200.0, 1300.0, 1400.0,
        1500.0, 1600.0, 1700.0, 1800.0, 1900.0, 2000.0
    );
    let cp_values_joule_per_kilogram_kelvin = c!(
        713.24, 991.02, 1218.45, 1390.79, 1520.85, 1620.52, 1698.40, 1760.40, 1810.64, 1851.96,
        1886.42, 1915.49, 1940.26, 1961.58, 1980.06, 1996.20, 2010.39, 2022.93
    );

    let s = CubicSpline::from_nodes(
        &cp_temperature_values_kelvin,
        &cp_values_joule_per_kilogram_kelvin,
    );

    let graphite_cp_value = s.unwrap().eval(temperature_value_kelvin);

    Ok(SpecificHeatCapacity::new::<joule_per_kilogram_kelvin>(
        graphite_cp_value,
    ))
}

/// Lowest temperature, 250 K, at which Butland & Maddison's polynomial 3 may
/// be used "with confidence" (J. Nucl. Mater. 49 (1973/74) 45-56, sect. 4).
#[inline]
pub fn min_temp_butland_maddison_polynomial() -> ThermodynamicTemperature {
    ThermodynamicTemperature::new::<kelvin>(250.0)
}

/// Highest temperature, 3000 K, at which Butland & Maddison's polynomial 3
/// may be used "with confidence" (same paper, sect. 4).
#[inline]
pub fn max_temp_butland_maddison_polynomial() -> ThermodynamicTemperature {
    ThermodynamicTemperature::new::<kelvin>(3000.0)
}

/// Returns the specific heat capacity of nuclear graphite from **Butland &
/// Maddison's polynomial 3**, valid **250 K to 3000 K**.
///
/// **Source:** Butland, A. T. D. & Maddison, R. J., "The specific heat of
/// graphite: an evaluation of measurements", *Journal of Nuclear Materials*
/// **49** (1973/74) 45-56, polynomial 3 (p. 55), with T in K and cp in
/// cal/(g K):
///
/// ```text
/// cp = 0.54212 - 2.42667e-6 T - 90.2725 T^-1 - 43449.3 T^-2
///      + 1.59309e7 T^-3 - 1.43688e9 T^-4
/// ```
///
/// Polynomial 3 is the authors' final recommendation: their least-squares
/// fit to well-graphitised specimens (polynomial 2), with the constant and
/// linear terms adjusted so the derived Cv at 1800 K agrees with Page's
/// phonon-spectrum prediction while Cp at 300 K is unchanged. The fit used
/// data spanning 200-3500 K, but the authors state it "may only be used with
/// confidence over the range 250 K - 3000 K"; outside that range this returns
/// `TuasLibError::ThermophysicalPropertyTemperatureRangeError`.
///
/// Coefficients read from the paper itself (maintainer's copy, restricted
/// literature, 2026-09-28) and cross-checked against the same correlation as
/// implemented in the MOOSE framework's `ThermalGraphiteProperties`
/// (`mooseframework.inl.gov/source/solidproperties/ThermalGraphiteProperties.html`,
/// Idaho National Laboratory, LGPL-2.1), which also cites Butland & Maddison.
/// Only the published formula is used here; no MOOSE code was copied. The
/// MOOSE source is reproduced verbatim, with its LGPL-2.1 attribution, in
/// `crates/tuas_boussinesq_solver/docs/moose-thermal-graphite-properties-lgpl.md`
/// (commit `9952567b`), for comparison.
///
/// **Unit conversion:** 1 cal = **4.184 J** (thermochemical calorie), as MOOSE
/// uses. The paper does not say which calorie it means. The VTB cp table in
/// [`nuclear_graphite_specific_heat_capacity_butland_maddison_spline`] is this
/// same polynomial evaluated with the International Table calorie
/// (4.1868 J); the two conversions differ by 0.067 %, far inside the
/// correlation's own spread (the paper reports up to 10 % scatter between
/// measurements above ~800 K).
///
/// **Grade:** cp of well-graphitised nuclear graphite, treated as
/// grade-insensitive for both the A3 matrix and IG-110 (see the module doc).
#[inline]
pub fn nuclear_graphite_specific_heat_capacity_butland_maddison_polynomial(
    temperature: ThermodynamicTemperature,
) -> Result<SpecificHeatCapacity, TuasLibError> {
    range_check(
        &Material::Solid(SolidMaterial::NuclearGraphiteMatrixA3),
        temperature,
        max_temp_butland_maddison_polynomial(),
        min_temp_butland_maddison_polynomial(),
    )?;
    let t = temperature.get::<kelvin>();
    let cp_cal_per_gram_kelvin = 0.54212 - 2.42667e-6 * t - 90.2725 / t - 43449.3 / t.powi(2)
        + 1.59309e7 / t.powi(3)
        - 1.43688e9 / t.powi(4);
    const JOULE_PER_THERMOCHEMICAL_CALORIE: f64 = 4.184;
    // cal/(g K) -> J/(kg K): x 4.184 J/cal x 1000 g/kg.
    Ok(SpecificHeatCapacity::new::<joule_per_kilogram_kelvin>(
        cp_cal_per_gram_kelvin * JOULE_PER_THERMOCHEMICAL_CALORIE * 1000.0,
    ))
}

/// Returns the fast-neutron-fluence conductivity damage factor
/// (dimensionless) for nuclear graphite:
///
/// factor = 1 - 0.336*(1 - exp(-1.005*gam)) - 3.50e-2*gam
///
/// where `gam` is the fast-neutron fluence. **Unit interpretation:** `gam`
/// is interpreted as the fluence in units of 10^25 n/m^2 (E > 0.1 MeV).
/// This is an *interpretation, not a deck-stated fact* — the VTB HTR-PM deck
/// declares no unit for `gam`. It is supported by (a) the VTB GPBR200 decks
/// using `fast_neutron_fluence = 10e25` n/m^2 with graphite grade A3_3_1800,
/// and (b) the deck deriving `gam` from burnup with magnitudes of order 10,
/// consistent with fluences of order 10^26 n/m^2.
///
/// Source: the fluence factor of the `gmatrix_k` function in the VTB HTR-PM
/// pebble model
/// (`reference-data/virtual_test_bed/htgr/htr-pm/core-multiphysics/updated_equilibrium_core/pebble_triso.i`,
/// lines 193-198). CC-BY-4.0, Open tier. The deck names no upstream
/// literature source for this correlation.
///
/// Behaviour: exactly 1 at `gam = 0`; monotonically decreasing in `gam`.
/// The saturating exponential term levels off near 0.336 by `gam ~ 5`, after
/// which the linear `3.5e-2*gam` term dominates and drives the factor
/// through zero at `gam ~ 19` — which is unphysical (conductivity cannot be
/// negative). This function therefore returns
/// ~~`TuasLibError::ThermophysicalPropertyTemperatureRangeError`~~
/// `TuasLibError::CorrelationRangeError` (**CORRECTED 2026-09-28** to match
/// the code below) for `gam` outside [0, 15]: at `gam = 15` the factor is still physically positive
/// (measured 0.1390, see the unit test), leaving margin before the
/// unphysical zero crossing at `gam ~ 19.0`.
#[inline]
pub fn nuclear_graphite_fluence_damage_factor(fluence: Ratio) -> Result<Ratio, TuasLibError> {
    let gam: f64 = fluence.get::<ratio>();

    // This is a FLUENCE bound, not a temperature bound. It previously returned
    // `ThermophysicalPropertyTemperatureRangeError`, which told a caller a
    // temperature had left its range when no temperature is involved here.
    // `CorrelationRangeError` reports the quantity that actually failed, so the
    // `println!` is no longer needed to carry the detail.
    if !(0.0..=15.0).contains(&gam) {
        return Err(TuasLibError::CorrelationRangeError {
            parameter: "nuclear graphite fluence damage factor: fluence gam".to_string(),
            value: gam,
            lower_bound: 0.0,
            upper_bound: 15.0,
            units: "10^25 n/m^2, E > 0.1 MeV (beyond gam ~ 19 the correlation goes negative, \
                    which is unphysical)"
                .to_string(),
        });
    }

    let damage_factor: f64 = 1.0 - 0.336 * (1.0 - f64::exp(-1.005 * gam)) - 3.50e-2 * gam;

    Ok(Ratio::new::<ratio>(damage_factor))
}

/// Returns the thermal conductivity, in W/(m K), of A3-grade pebble matrix
/// graphite at the given temperature and fast-neutron fluence.
///
/// Implements the `gmatrix_k` function of the VTB HTR-PM pebble model
/// (`reference-data/virtual_test_bed/htgr/htr-pm/core-multiphysics/updated_equilibrium_core/pebble_triso.i`,
/// lines 193-198; CC-BY-4.0, Open tier), with temperature `t` in kelvin:
///
/// k(t, gam) = 47.4 * (1 - 9.7556e-4*(t - 373.15)*exp(-6.036e-4*(t - 273.15)))
///           * (1740/(2.2*(1700 - 1740) + 1740))
///           * (1 - 0.336*(1 - exp(-1.005*gam)) - 3.50e-2*gam)
///
/// The three factors are: a temperature factor (equal to 1 at 373.15 K); a
/// constant density/Maxwell porosity factor 1740/1652 ~= 1.0533; and the
/// fluence damage factor (see
/// [`nuclear_graphite_fluence_damage_factor`], including the unit
/// interpretation of `gam` as fluence in 10^25 n/m^2, E > 0.1 MeV — an
/// interpretation, since the deck declares no unit). The correlation is
/// **transcribed from the VTB HTR-PM pebble model, which names no upstream
/// source** for it.
///
/// **Upstream provenance, traced 2026-09-28 (maintainer).** The constants are
/// the **A3-27, 1800 degC heat-treatment** row of Table 3.3 of PNNL-31427
/// (Wells, D., Phillips, B., Geelhood, K., June 2021; US NRC ADAMS
/// ML21175A152), which takes them from Hales et al. (2020); the functional
/// form is Gontard & Nabielek (1990), via Miller et al. (2018). **Not
/// re-checked against PNNL-31427 in this change** — the report is not held in
/// this workspace, so this records the maintainer's trace rather than a
/// reading made here. What the trace does NOT settle is the measured
/// temperature range of Gontard & Nabielek's data (the maintainer is
/// obtaining the paper).
///
/// Valid ranges enforced: temperature 300 K to 2000 K — **a coding choice, not
/// a stated validity range**: the deck states no range, and 300-2000 K was
/// adopted to match the sibling graphite cp table so all nuclear-graphite
/// properties share one coded validity window. The high-temperature sibling
/// [`nuclear_graphite_matrix_a3_thermal_conductivity_high_temp_fluence_dependent`]
/// evaluates the same formula to 3000 K, flagged as extrapolation above
/// 2000 K. Fluence `gam` is enforced in [0, 15] (beyond which the damage
/// factor heads to an unphysical zero crossing at `gam ~ 19`). An
/// out-of-range temperature returns
/// `TuasLibError::ThermophysicalPropertyTemperatureRangeError`; an
/// out-of-range fluence returns `TuasLibError::CorrelationRangeError`
/// (~~"Out-of-range inputs return
/// `TuasLibError::ThermophysicalPropertyTemperatureRangeError`"~~ **CORRECTED
/// 2026-09-28** — the fluence check in
/// [`nuclear_graphite_fluence_damage_factor`] returns `CorrelationRangeError`).
#[inline]
pub fn nuclear_graphite_matrix_a3_thermal_conductivity_fluence_dependent(
    temperature: ThermodynamicTemperature,
    fluence: Ratio,
) -> Result<ThermalConductivity, TuasLibError> {
    range_check(
        &Material::Solid(SolidMaterial::NuclearGraphiteMatrixA3),
        temperature,
        max_temp_nuclear_graphite(),
        min_temp_nuclear_graphite(),
    )?;

    let damage_factor: f64 = nuclear_graphite_fluence_damage_factor(fluence)?.get::<ratio>();

    // One formula shared with the high-temperature variant, so the two
    // cannot drift apart.
    Ok(nuclear_graphite_matrix_a3_conductivity_formula(
        temperature,
        damage_factor,
    ))
}

/// Returns the thermal conductivity, in W/(m K), of **unirradiated**
/// (zero-fluence) A3-grade pebble matrix graphite at the given temperature.
///
/// This is [`nuclear_graphite_matrix_a3_thermal_conductivity_fluence_dependent`]
/// evaluated at `gam = 0`, where the fluence damage factor is exactly 1 —
/// see that function for the correlation, its VTB HTR-PM source
/// (`pebble_triso.i` lines 193-198, CC-BY-4.0, Open tier; no upstream
/// source named in the deck), and the enforced 300 K to 2000 K range. The
/// [`SolidMaterial::NuclearGraphiteMatrixA3`] enum arm dispatches here.
#[inline]
pub fn nuclear_graphite_matrix_a3_thermal_conductivity_zero_fluence(
    temperature: ThermodynamicTemperature,
) -> Result<ThermalConductivity, TuasLibError> {
    nuclear_graphite_matrix_a3_thermal_conductivity_fluence_dependent(
        temperature,
        Ratio::new::<ratio>(0.0),
    )
}

/// Returns the thermal conductivity, in W/(m K), of **unirradiated** IG-110
/// reflector-grade graphite at the given temperature.
///
/// Implements the `IG110_k` quadratic of the VTB HTTR deck
/// (`reference-data/virtual_test_bed/htgr/httr/steady_state_and_null_transient/fuel_elem_steady.i`,
/// lines 301-306; CC-BY-4.0, Open tier), with temperature `t` in kelvin:
///
/// k(t) = 66.32 - 4.994e-2*t + 1.712e-5*t^2
///
/// The deck **names no upstream literature source** for this quadratic, and
/// **states no validity range**; this implementation enforces 300 K to
/// 2000 K, consistent with the sibling nuclear-graphite cp table, so all
/// nuclear-graphite properties share one coded validity window.
/// Out-of-range temperatures return
/// `TuasLibError::ThermophysicalPropertyTemperatureRangeError`. The
/// [`SolidMaterial::NuclearGraphiteIG110`] enum arm dispatches here.
#[inline]
pub fn nuclear_graphite_ig_110_thermal_conductivity_unirradiated(
    temperature: ThermodynamicTemperature,
) -> Result<ThermalConductivity, TuasLibError> {
    range_check(
        &Material::Solid(SolidMaterial::NuclearGraphiteIG110),
        temperature,
        max_temp_nuclear_graphite(),
        min_temp_nuclear_graphite(),
    )?;

    let t: f64 = temperature.get::<kelvin>();

    let k_value: f64 = 66.32 - 4.994e-2 * t + 1.712e-5 * t * t;

    Ok(ThermalConductivity::new::<watt_per_meter_kelvin>(k_value))
}

/// Returns the thermal conductivity, in W/(m K), of IG-110 reflector-grade
/// graphite at the given temperature and fast-neutron fluence, by applying
/// the A3 matrix-graphite fluence damage factor to the unirradiated IG-110
/// quadratic.
///
/// **Assumption of this implementation:** the saturating damage factor
/// (1 - 0.336*(1 - exp(-1.005*gam)) - 3.50e-2*gam) is taken from the VTB
/// HTR-PM pebble model, where it is applied to matrix/buffer/PyC materials —
/// **the VTB does not apply it to IG-110**. It is applied here because the
/// reflector dose-degradation path needs a fluence-degraded IG-110
/// conductivity and no better open correlation is vendored in this
/// workspace. Treat results at `gam > 0` as an engineering estimate only.
///
/// `gam` is interpreted as fluence in units of 10^25 n/m^2 (E > 0.1 MeV);
/// see [`nuclear_graphite_fluence_damage_factor`] for that interpretation
/// and for the enforced fluence range [0, 15]. Temperature range enforced:
/// 300 K to 2000 K (via
/// [`nuclear_graphite_ig_110_thermal_conductivity_unirradiated`]).
#[inline]
pub fn nuclear_graphite_ig_110_thermal_conductivity_fluence_dependent(
    temperature: ThermodynamicTemperature,
    fluence: Ratio,
) -> Result<ThermalConductivity, TuasLibError> {
    let unirradiated_k = nuclear_graphite_ig_110_thermal_conductivity_unirradiated(temperature)?;
    let damage_factor = nuclear_graphite_fluence_damage_factor(fluence)?;

    Ok(unirradiated_k * damage_factor)
}

/// Returns the specific enthalpy, in J/kg, of nuclear graphite (both A3
/// matrix and IG-110 grades, which share one cp table) at the given
/// temperature, with h = 0 at 273.15 K.
///
/// Integrates the Butland & Maddison cp cubic spline (see
/// [`nuclear_graphite_specific_heat_capacity_butland_maddison_spline`];
/// VTB HTTF deck `HTTF-SS.i` lines 368-372, CC-BY-4.0, Open tier) from
/// 273.15 K to the given temperature, per this database's house convention
/// (compare `copper_specific_enthalpy` /
/// `steel_304_l_spline_specific_enthalpy_ciet_zweibaum`).
///
/// **Below-table extrapolation note:** the cp table starts at 300 K but the
/// integration reference is 273.15 K, so the integral's first 26.85 K uses
/// the cubic spline's natural extrapolation below its lowest node. This only
/// shifts the (arbitrary) enthalpy datum by a constant; enthalpy
/// *differences* between any two temperatures at or above 300 K are
/// unaffected. User-facing dispatch keeps `min_temperature()` at 300 K, so
/// in-range calls never rely on extrapolated cp except through this shared
/// datum offset.
///
/// Like its siblings this function performs no range check of its own
/// (range enforcement happens in the cp/conductivity accessors and via
/// `min_temperature()`/`max_temperature()`).
#[inline]
pub fn nuclear_graphite_specific_enthalpy(
    temperature: ThermodynamicTemperature,
) -> AvailableEnergy {
    let temperature_value_kelvin: f64 = temperature.get::<kelvin>();

    let cp_temperature_values_kelvin = c!(
        300.0, 400.0, 500.0, 600.0, 700.0, 800.0, 900.0, 1000.0, 1100.0, 1200.0, 1300.0, 1400.0,
        1500.0, 1600.0, 1700.0, 1800.0, 1900.0, 2000.0
    );
    let cp_values_joule_per_kilogram_kelvin = c!(
        713.24, 991.02, 1218.45, 1390.79, 1520.85, 1620.52, 1698.40, 1760.40, 1810.64, 1851.96,
        1886.42, 1915.49, 1940.26, 1961.58, 1980.06, 1996.20, 2010.39, 2022.93
    );

    let s = CubicSpline::from_nodes(
        &cp_temperature_values_kelvin,
        &cp_values_joule_per_kilogram_kelvin,
    );

    let graphite_specific_enthalpy_value = s.unwrap().integrate((273.15, temperature_value_kelvin));

    AvailableEnergy::new::<joule_per_kilogram>(graphite_specific_enthalpy_value)
}

/// Returns the temperature, in K, of nuclear graphite (both A3 matrix and
/// IG-110 grades, which share one enthalpy curve) from its specific enthalpy
/// in J/kg.
///
/// Follows the `ss_304_l` house pattern
/// (`steel_304_l_spline_temp_attempt_3_from_specific_enthalpy_ciet_zweibaum`):
/// build an inverted cubic spline (enthalpy -> temperature) through the
/// enthalpies evaluated at the cp-table node temperatures (300 K to 2000 K)
/// as an initial guess, then refine with Brent-Dekker root finding on
/// h(T) - h_target within a +/- 5 K bracket around the guess. Panics (like
/// its siblings) if the root find fails, which indicates an enthalpy far
/// outside the correlation range.
///
/// Enthalpy data source: Butland & Maddison cp table via the VTB HTTF deck
/// (`HTTF-SS.i` lines 368-372, CC-BY-4.0, Open tier); see
/// [`nuclear_graphite_specific_enthalpy`].
#[inline]
pub(crate) fn nuclear_graphite_spline_temp_from_specific_enthalpy(
    h_graphite: AvailableEnergy,
) -> Result<ThermodynamicTemperature, TuasLibError> {
    // evaluate enthalpy at the cp-table node temperatures
    let temperature_values_kelvin: Vec<f64> = c!(
        300.0, 400.0, 500.0, 600.0, 700.0, 800.0, 900.0, 1000.0, 1100.0, 1200.0, 1300.0, 1400.0,
        1500.0, 1600.0, 1700.0, 1800.0, 1900.0, 2000.0
    );

    let temperature_vec_len = temperature_values_kelvin.len();

    let mut enthalpy_vector = vec![0.0; temperature_vec_len];

    for index_i in 0..temperature_vec_len {
        let temperature_value = temperature_values_kelvin[index_i];

        let graphite_temp = ThermodynamicTemperature::new::<kelvin>(temperature_value);

        // both graphite variants share this enthalpy curve, so call the
        // free function directly rather than dispatching through the enum
        let graphite_enthalpy_value =
            nuclear_graphite_specific_enthalpy(graphite_temp).get::<joule_per_kilogram>();

        enthalpy_vector[index_i] = graphite_enthalpy_value;
    }

    // inverted spline: enthalpy in, temperature out (initial guess)
    let enthalpy_to_temperature_spline =
        CubicSpline::from_nodes(&enthalpy_vector, &temperature_values_kelvin);

    let h_graphite_joules_per_kg = h_graphite.get::<joule_per_kilogram>();

    let temperature_from_enthalpy_kelvin = enthalpy_to_temperature_spline
        .unwrap()
        .eval(h_graphite_joules_per_kg);

    // refine with brent dekker
    let enthalpy_root = |temp_kelvin_value: f64| -> f64 {
        let lhs_value = h_graphite.get::<joule_per_kilogram>();

        let graphite_temp = ThermodynamicTemperature::new::<kelvin>(temp_kelvin_value);

        let rhs_value =
            nuclear_graphite_specific_enthalpy(graphite_temp).get::<joule_per_kilogram>();

        lhs_value - rhs_value
    };

    // the inverted-spline guess is accurate to well under a kelvin over
    // the smooth cp curve; a 5 K bracket gives comfortable margin, and
    // the bare enthalpy integral is defined slightly outside the node
    // range so bracket overhang at the range ends is safe
    let brent_error_bound: f64 = 5.0;

    let upper_limit: f64 = temperature_from_enthalpy_kelvin + brent_error_bound;

    let lower_limit: f64 = temperature_from_enthalpy_kelvin - brent_error_bound;

    let mut convergency = SimpleConvergency {
        eps: 1e-8f64,
        max_iter: 30,
    };
    let graphite_temperature_result =
        find_root_brent(upper_limit, lower_limit, enthalpy_root, &mut convergency);

    let temperature_from_enthalpy_kelvin: f64 = match graphite_temperature_result {
        Ok(temperature_val) => temperature_val,
        // The Brent solve failed to bracket a root for this enthalpy.
        Err(_) => {
            return Err(TuasLibError::GenericStringError(format!(
                "nuclear graphite: could not invert specific enthalpy {h_graphite:?} to a temperature"
            )))
        }
    };

    Ok(ThermodynamicTemperature::new::<kelvin>(temperature_from_enthalpy_kelvin))
}

// ---------------------------------------------------------------------------
// High-temperature A3 matrix graphite (`SolidMaterial::NuclearGraphiteMatrixA3HighTemp`)
// ---------------------------------------------------------------------------
//
// Added 2026-09-28 at the maintainer's request ("add an enum variant", rather
// than switching the existing variants) so a pebble-bed transient that takes
// the fuel past 2000 K keeps a property set instead of refusing (gh:#350,
// gh:#351). The base `NuclearGraphiteMatrixA3` variant is unchanged.
//
// What is and is not backed by a source above 2000 K:
//
// - **cp** — Butland & Maddison polynomial 3, which the authors state may be
//   used "with confidence" over 250-3000 K. Backed.
// - **k** — the SAME A3 correlation as the base variant, with its coded window
//   widened to 3000 K. **Above 2000 K this is extrapolation.** The
//   correlation's measured range (Gontard & Nabielek 1990) is not yet
//   confirmed. See `nuclear_graphite_matrix_a3_thermal_conductivity_high_temp`
//   for what the fitted form does up there (it has a minimum at 2029.9 K and
//   RISES above it, which is a property of the form, not evidence).
// - **density** — the base variant's constant 1730 kg/m^3.

/// Lowest temperature, 300 K, of the high-temperature A3 matrix-graphite
/// variant's coded window.
///
/// The cp polynomial is good from 250 K, but the conductivity correlation's
/// coded window starts at 300 K (see
/// [`nuclear_graphite_matrix_a3_thermal_conductivity_fluence_dependent`]), and
/// a variant must not advertise a floor one of its properties refuses.
#[inline]
pub fn min_temp_nuclear_graphite_high_temp() -> ThermodynamicTemperature {
    ThermodynamicTemperature::new::<kelvin>(300.0)
}

/// Highest temperature, 3000 K, of the high-temperature A3 matrix-graphite
/// variant's coded window — the published upper limit of Butland & Maddison's
/// polynomial 3 ([`max_temp_butland_maddison_polynomial`]).
///
/// **Only the cp is backed to 3000 K.** The conductivity above 2000 K is
/// extrapolated; see
/// [`nuclear_graphite_matrix_a3_thermal_conductivity_high_temp`].
#[inline]
pub fn max_temp_nuclear_graphite_high_temp() -> ThermodynamicTemperature {
    ThermodynamicTemperature::new::<kelvin>(3000.0)
}

/// Returns the specific heat capacity of A3 matrix graphite for the
/// high-temperature variant: Butland & Maddison polynomial 3
/// ([`nuclear_graphite_specific_heat_capacity_butland_maddison_polynomial`]),
/// evaluated inside the variant's 300-3000 K window.
///
/// The [`SolidMaterial::NuclearGraphiteMatrixA3HighTemp`] enum arm dispatches
/// here. Out of window returns
/// `TuasLibError::ThermophysicalPropertyTemperatureRangeError`.
#[inline]
pub fn nuclear_graphite_matrix_a3_high_temp_specific_heat_capacity(
    temperature: ThermodynamicTemperature,
) -> Result<SpecificHeatCapacity, TuasLibError> {
    range_check(
        &Material::Solid(SolidMaterial::NuclearGraphiteMatrixA3HighTemp),
        temperature,
        max_temp_nuclear_graphite_high_temp(),
        min_temp_nuclear_graphite_high_temp(),
    )?;
    nuclear_graphite_specific_heat_capacity_butland_maddison_polynomial(temperature)
}

/// Returns the thermal conductivity, in W/(m K), of A3 matrix graphite at the
/// given temperature and fast-neutron fluence, over the **high-temperature
/// variant's 300-3000 K window**.
///
/// **Identical formula to**
/// [`nuclear_graphite_matrix_a3_thermal_conductivity_fluence_dependent`]
/// (same constants, same Maxwell/density factor, same fluence factor), so at
/// or below 2000 K the two return the same number to the last bit. Only the
/// coded temperature window differs.
///
/// # Above 2000 K this is EXTRAPOLATION — read before quoting a number
///
/// **The correlation's measured range (Gontard & Nabielek 1990) is not yet
/// confirmed; above 2000 K this is extrapolated.** The base variant's
/// 2000 K ceiling was itself a coding choice (it matched the sibling cp
/// table), not a stated validity limit, so there is no published range to
/// widen to — the widening to 3000 K follows the cp polynomial's range and
/// nothing about the conductivity data.
///
/// The fitted temperature factor `1 - 9.7556e-4 (T - 373.15) exp(-6.036e-4 (T - 273.15))`
/// has its **minimum at T = 373.15 + 1/6.036e-4 = 2029.9 K** and rises above
/// it. Measured at zero fluence (2026-09-28, see the unit test): k =
/// 21.98 W/(m K) at 2000 K, 22.91 at 2500 K and 25.25 at 3000 K — a
/// **+14.9 %** rise from 2000 to 3000 K that comes from the shape of the fit,
/// not from any data in that range. If the real conductivity keeps falling or
/// flattens, this **over-states k** above ~2030 K, which under-states the
/// temperature rise inside a pebble (the kernel reads cooler than it would).
///
/// Fluence window `gam` in [0, 15] as for the base form. Out of window returns
/// `TuasLibError::ThermophysicalPropertyTemperatureRangeError` (temperature)
/// or `TuasLibError::CorrelationRangeError` (fluence).
#[inline]
pub fn nuclear_graphite_matrix_a3_thermal_conductivity_high_temp_fluence_dependent(
    temperature: ThermodynamicTemperature,
    fluence: Ratio,
) -> Result<ThermalConductivity, TuasLibError> {
    range_check(
        &Material::Solid(SolidMaterial::NuclearGraphiteMatrixA3HighTemp),
        temperature,
        max_temp_nuclear_graphite_high_temp(),
        min_temp_nuclear_graphite_high_temp(),
    )?;
    Ok(nuclear_graphite_matrix_a3_conductivity_formula(
        temperature,
        nuclear_graphite_fluence_damage_factor(fluence)?.get::<ratio>(),
    ))
}

/// Zero-fluence form of
/// [`nuclear_graphite_matrix_a3_thermal_conductivity_high_temp_fluence_dependent`]
/// — **extrapolated above 2000 K**, see there. The
/// [`SolidMaterial::NuclearGraphiteMatrixA3HighTemp`] enum arm dispatches here.
#[inline]
pub fn nuclear_graphite_matrix_a3_thermal_conductivity_high_temp(
    temperature: ThermodynamicTemperature,
) -> Result<ThermalConductivity, TuasLibError> {
    nuclear_graphite_matrix_a3_thermal_conductivity_high_temp_fluence_dependent(
        temperature,
        Ratio::new::<ratio>(0.0),
    )
}

/// The A3 conductivity formula itself, with no range check — shared by the
/// base and high-temperature variants so the two cannot drift apart. See
/// [`nuclear_graphite_matrix_a3_thermal_conductivity_fluence_dependent`] for
/// the provenance of every constant.
#[inline]
fn nuclear_graphite_matrix_a3_conductivity_formula(
    temperature: ThermodynamicTemperature,
    damage_factor: f64,
) -> ThermalConductivity {
    let t: f64 = temperature.get::<kelvin>();
    let temperature_factor: f64 =
        1.0 - 9.7556e-4 * (t - 373.15) * f64::exp(-6.036e-4 * (t - 273.15));
    let density_maxwell_factor: f64 = 1740.0 / (2.2 * (1700.0 - 1740.0) + 1740.0);
    let k_value: f64 = 47.4 * temperature_factor * density_maxwell_factor * damage_factor;
    ThermalConductivity::new::<watt_per_meter_kelvin>(k_value)
}

/// Butland & Maddison polynomial 3 integrated analytically, in J/kg, from
/// 273.15 K to `t_kelvin` (thermochemical calorie, as the cp function).
///
/// With `cp = a + b T + c/T + d/T^2 + e/T^3 + f/T^4` (cal/(g K)) the
/// antiderivative is `a T + b T^2/2 + c ln T - d/T - e/(2 T^2) - f/(3 T^3)`.
/// Exact — no quadrature, no spline.
#[inline]
fn butland_maddison_polynomial_enthalpy_joule_per_kg(t_kelvin: f64) -> f64 {
    let antiderivative = |t: f64| {
        0.54212 * t - 2.42667e-6 * t * t / 2.0 - 90.2725 * t.ln() + 43449.3 / t
            - 1.59309e7 / (2.0 * t * t)
            + 1.43688e9 / (3.0 * t * t * t)
    };
    (antiderivative(t_kelvin) - antiderivative(273.15)) * 4.184 * 1000.0
}

/// Returns the specific enthalpy, in J/kg, of the high-temperature A3
/// matrix-graphite variant, `h = integral of cp dT` from 273.15 K (the
/// database's house datum) with cp the Butland & Maddison polynomial 3.
///
/// The integral is taken **analytically** (see
/// `butland_maddison_polynomial_enthalpy_joule_per_kg`), so it is exact
/// to f64 roundoff at every temperature and is consistent with
/// [`nuclear_graphite_matrix_a3_high_temp_specific_heat_capacity`] by
/// construction: `dh/dT = cp`. The 273.15-300 K stretch below the variant's
/// coded floor is inside the polynomial's own 250-3000 K range, so the
/// datum involves no extrapolation (unlike the spline variant's).
///
/// Like its siblings this performs no range check; range enforcement lives in
/// the cp and conductivity accessors.
#[inline]
pub fn nuclear_graphite_matrix_a3_high_temp_specific_enthalpy(
    temperature: ThermodynamicTemperature,
) -> AvailableEnergy {
    AvailableEnergy::new::<joule_per_kilogram>(butland_maddison_polynomial_enthalpy_joule_per_kg(
        temperature.get::<kelvin>(),
    ))
}

/// Inverts [`nuclear_graphite_matrix_a3_high_temp_specific_enthalpy`]:
/// temperature from specific enthalpy, by Brent-Dekker root finding over the
/// polynomial's whole 250-3000 K range.
///
/// cp is strictly positive over that range, so `h(T)` is strictly increasing
/// and the bracket holds exactly one root; no initial-guess spline is needed.
/// An enthalpy outside `[h(250 K), h(3000 K)]` returns a
/// `TuasLibError::GenericStringError` rather than panicking.
#[inline]
pub(crate) fn nuclear_graphite_matrix_a3_high_temp_temp_from_specific_enthalpy(
    h_graphite: AvailableEnergy,
) -> Result<ThermodynamicTemperature, TuasLibError> {
    let target = h_graphite.get::<joule_per_kilogram>();
    let low = min_temp_butland_maddison_polynomial().get::<kelvin>();
    let high = max_temp_butland_maddison_polynomial().get::<kelvin>();
    let residual = |t: f64| butland_maddison_polynomial_enthalpy_joule_per_kg(t) - target;
    let mut convergency = SimpleConvergency {
        eps: 1e-9f64,
        max_iter: 100,
    };
    find_root_brent(low, high, residual, &mut convergency)
        .map(ThermodynamicTemperature::new::<kelvin>)
        .map_err(|_| {
            TuasLibError::GenericStringError(format!(
                "high-temperature A3 graphite: specific enthalpy {target} J/kg lies outside \
                 the Butland & Maddison polynomial's 250-3000 K range"
            ))
        })
}

/// V&V test: cp spline reproduces the Butland & Maddison table nodes.
///
/// **Methodology:** evaluate
/// [`nuclear_graphite_specific_heat_capacity_butland_maddison_spline`] at
/// every one of the 18 table node temperatures (300 K to 2000 K in 100 K
/// steps) from the VTB HTTF deck (`HTTF-SS.i` lines 368-372, citing Butland
/// & Maddison, J. Nucl. Mater. 49 (1973/74) 45-56) and compare against the
/// tabulated cp values in J/(kg K). Pass criterion: maximum relative error
/// across all nodes below 1e-12 (a cubic spline must interpolate its own
/// nodes exactly, up to floating-point roundoff).
///
/// **Results (2026-08-11):** maximum relative error across all 18 nodes
/// measured 4.36e-15 (f64 roundoff) — spline interpolation reproduces
/// every table node. Numerical roundoff only; no physical uncertainty is
/// probed by this test (it verifies transcription + interpolation, not the
/// underlying data).
#[test]
pub fn nuclear_graphite_cp_spline_reproduces_butland_maddison_nodes() {
    let node_temperatures_kelvin = [
        300.0, 400.0, 500.0, 600.0, 700.0, 800.0, 900.0, 1000.0, 1100.0, 1200.0, 1300.0, 1400.0,
        1500.0, 1600.0, 1700.0, 1800.0, 1900.0, 2000.0,
    ];
    let node_cp_joule_per_kg_kelvin = [
        713.24, 991.02, 1218.45, 1390.79, 1520.85, 1620.52, 1698.40, 1760.40, 1810.64, 1851.96,
        1886.42, 1915.49, 1940.26, 1961.58, 1980.06, 1996.20, 2010.39, 2022.93,
    ];

    let mut max_rel_error: f64 = 0.0;

    for (temp_kelvin, cp_expected) in node_temperatures_kelvin
        .iter()
        .zip(node_cp_joule_per_kg_kelvin.iter())
    {
        let cp_measured = nuclear_graphite_specific_heat_capacity_butland_maddison_spline(
            ThermodynamicTemperature::new::<kelvin>(*temp_kelvin),
        )
        .unwrap()
        .get::<joule_per_kilogram_kelvin>();

        let rel_error = ((cp_measured - cp_expected) / cp_expected).abs();

        if rel_error > max_rel_error {
            max_rel_error = rel_error;
        }
    }

    println!(
        "cp spline max relative error across nodes: {:e}",
        max_rel_error
    );
    assert!(
        max_rel_error < 1e-12,
        "cp spline should reproduce table nodes exactly, \
        max rel error was {:e}",
        max_rel_error
    );
}

/// V&V test: A3 matrix conductivity at zero fluence matches the closed form.
///
/// **Methodology:** evaluate
/// [`nuclear_graphite_matrix_a3_thermal_conductivity_zero_fluence`] at
/// 373.15 K and at 600 K, and compare against the `gmatrix_k` closed form
/// (VTB HTR-PM `pebble_triso.i` lines 193-198) evaluated by hand in this
/// test: at 373.15 K the temperature factor is exactly 1 and k reduces to
/// 47.4 * 1740/1652 W/(m K); at 600 K the full closed form is recomputed
/// from its constants. Pass criterion: relative agreement to 1e-12.
///
/// **Results (2026-08-11):** at 373.15 K, measured
/// k = 49.92493946731234 W/(m K) against hand value 47.4*1740/1652 =
/// 49.92493946731235 W/(m K) (agreement to 1 ulp, relative error ~1.4e-16);
/// at 600 K, measured k = 40.854469121754626 W/(m K), identical to the
/// recomputed closed form. Transcription check only — no comparison against
/// measured HTR-10 graphite data.
#[test]
pub fn nuclear_graphite_matrix_a3_k_zero_fluence_matches_closed_form() {
    // at 373.15 K, temperature factor = 1 exactly, so
    // k = 47.4 * 1740/1652
    let k_hand_373: f64 = 47.4 * 1740.0 / (2.2 * (1700.0 - 1740.0) + 1740.0);

    let k_measured_373 = nuclear_graphite_matrix_a3_thermal_conductivity_zero_fluence(
        ThermodynamicTemperature::new::<kelvin>(373.15),
    )
    .unwrap()
    .get::<watt_per_meter_kelvin>();

    println!(
        "matrix A3 k at 373.15 K: measured {}, hand {}",
        k_measured_373, k_hand_373
    );

    approx::assert_relative_eq!(k_hand_373, k_measured_373, max_relative = 1e-12);

    // at 600 K, full closed form
    let t: f64 = 600.0;
    let k_hand_600: f64 = 47.4
        * (1.0 - 9.7556e-4 * (t - 373.15) * f64::exp(-6.036e-4 * (t - 273.15)))
        * (1740.0 / (2.2 * (1700.0 - 1740.0) + 1740.0));

    let k_measured_600 = nuclear_graphite_matrix_a3_thermal_conductivity_zero_fluence(
        ThermodynamicTemperature::new::<kelvin>(t),
    )
    .unwrap()
    .get::<watt_per_meter_kelvin>();

    println!(
        "matrix A3 k at 600 K: measured {}, hand {}",
        k_measured_600, k_hand_600
    );

    approx::assert_relative_eq!(k_hand_600, k_measured_600, max_relative = 1e-12);
}

/// V&V test: fluence damage factor behaviour.
///
/// **Methodology:** check three properties of
/// [`nuclear_graphite_fluence_damage_factor`] against the closed form
/// 1 - 0.336*(1 - exp(-1.005*gam)) - 3.50e-2*gam (VTB HTR-PM
/// `pebble_triso.i` lines 193-198): (1) at gam = 0 the factor equals 1
/// exactly; (2) sampled at gam = 0, 0.5, 1, ..., 15 (31 samples, step 0.5)
/// the factor is strictly monotonically decreasing; (3) the irradiated A3
/// matrix conductivity at 600 K is strictly below the unirradiated value for
/// gam = 1, 5, 10, 15. Also checks that gam just outside [0, 15] (-0.1 and
/// 15.1) returns the range error.
///
/// **Results (2026-08-11):** factor at gam = 0 measured exactly 1.0;
/// strictly decreasing across all 31 samples; factor at gam = 15 measured
/// 0.13900009535642543 (still positive, confirming the [0, 15] gate leaves
/// margin before the ~19 zero crossing); k(600 K, gam = 1) =
/// 30.72219297879128 W/(m K) < k(600 K, 0) = 40.854469121754626 W/(m K),
/// and likewise decreasing through gam = 5 (20.068), 10 (12.829), and
/// 15 (5.679 W/(m K)); out-of-range gam values (-0.1 and 15.1) returned
/// `ThermophysicalPropertyTemperatureRangeError` as required.
#[test]
pub fn nuclear_graphite_fluence_damage_factor_behaviour() {
    use uom::si::ratio::ratio;

    // (1) exactly 1 at gam = 0
    let factor_at_zero = nuclear_graphite_fluence_damage_factor(Ratio::new::<ratio>(0.0))
        .unwrap()
        .get::<ratio>();
    println!("damage factor at gam=0: {}", factor_at_zero);
    assert_eq!(
        factor_at_zero, 1.0,
        "damage factor at zero fluence must be exactly 1"
    );

    // (2) monotonically decreasing on [0, 15], sampled at step 0.5
    let mut previous_factor = f64::INFINITY;
    let mut gam_sample: f64 = 0.0;
    while gam_sample <= 15.0 {
        let factor = nuclear_graphite_fluence_damage_factor(Ratio::new::<ratio>(gam_sample))
            .unwrap()
            .get::<ratio>();
        assert!(
            factor < previous_factor,
            "damage factor not strictly decreasing at gam = {}",
            gam_sample
        );
        previous_factor = factor;
        gam_sample += 0.5;
    }
    println!("damage factor at gam=15: {}", previous_factor);
    assert!(
        previous_factor > 0.0,
        "damage factor at gam=15 should still be positive"
    );

    // (3) irradiated k < unirradiated k at 600 K
    let temp_600 = ThermodynamicTemperature::new::<kelvin>(600.0);
    let k_unirradiated = nuclear_graphite_matrix_a3_thermal_conductivity_zero_fluence(temp_600)
        .unwrap()
        .get::<watt_per_meter_kelvin>();
    for gam_value in [1.0, 5.0, 10.0, 15.0] {
        let k_irradiated = nuclear_graphite_matrix_a3_thermal_conductivity_fluence_dependent(
            temp_600,
            Ratio::new::<ratio>(gam_value),
        )
        .unwrap()
        .get::<watt_per_meter_kelvin>();
        println!(
            "matrix A3 k at 600 K, gam={}: {} (unirradiated {})",
            gam_value, k_irradiated, k_unirradiated
        );
        assert!(
            k_irradiated < k_unirradiated,
            "irradiated k must be below unirradiated k at gam = {}",
            gam_value
        );
    }

    // out-of-range fluence returns the range error
    assert!(nuclear_graphite_fluence_damage_factor(Ratio::new::<ratio>(-0.1)).is_err());
    assert!(nuclear_graphite_fluence_damage_factor(Ratio::new::<ratio>(15.1)).is_err());
}

/// V&V test: IG-110 conductivity quadratic against hand evaluation.
///
/// **Methodology:** evaluate
/// [`nuclear_graphite_ig_110_thermal_conductivity_unirradiated`] at 300 K
/// and 1000 K and compare against hand evaluations of the `IG110_k`
/// quadratic k(t) = 66.32 - 4.994e-2 t + 1.712e-5 t^2 (VTB HTTR
/// `fuel_elem_steady.i` lines 301-306): at 300 K,
/// 66.32 - 14.982 + 1.5408 = 52.8788 W/(m K); at 1000 K,
/// 66.32 - 49.94 + 17.12 = 33.5 W/(m K). Pass criterion: relative agreement
/// to 1e-12.
///
/// **Results (2026-08-11):** measured k(300 K) = 52.87879999999999 W/(m K)
/// (i.e. 52.8788 to f64 roundoff) and k(1000 K) = 33.5 W/(m K) exactly,
/// both identical to the in-test hand evaluations; the fluence-degraded
/// variant at gam = 0 also reproduced the unirradiated value to 1e-12
/// relative. Transcription check only — no comparison against measured
/// IG-110 data.
#[test]
pub fn nuclear_graphite_ig_110_k_quadratic_hand_evaluation() {
    let k_hand_300: f64 = 66.32 - 4.994e-2 * 300.0 + 1.712e-5 * 300.0 * 300.0;
    let k_measured_300 =
        nuclear_graphite_ig_110_thermal_conductivity_unirradiated(ThermodynamicTemperature::new::<
            kelvin,
        >(300.0))
        .unwrap()
        .get::<watt_per_meter_kelvin>();
    println!(
        "IG-110 k at 300 K: measured {}, hand {}",
        k_measured_300, k_hand_300
    );
    approx::assert_relative_eq!(k_hand_300, k_measured_300, max_relative = 1e-12);

    let k_hand_1000: f64 = 66.32 - 4.994e-2 * 1000.0 + 1.712e-5 * 1000.0 * 1000.0;
    let k_measured_1000 =
        nuclear_graphite_ig_110_thermal_conductivity_unirradiated(ThermodynamicTemperature::new::<
            kelvin,
        >(1000.0))
        .unwrap()
        .get::<watt_per_meter_kelvin>();
    println!(
        "IG-110 k at 1000 K: measured {}, hand {}",
        k_measured_1000, k_hand_1000
    );
    approx::assert_relative_eq!(k_hand_1000, k_measured_1000, max_relative = 1e-12);

    // the fluence-degraded variant at gam=0 equals the unirradiated form
    use uom::si::ratio::ratio;
    let k_fluence_zero = nuclear_graphite_ig_110_thermal_conductivity_fluence_dependent(
        ThermodynamicTemperature::new::<kelvin>(1000.0),
        Ratio::new::<ratio>(0.0),
    )
    .unwrap()
    .get::<watt_per_meter_kelvin>();
    approx::assert_relative_eq!(k_measured_1000, k_fluence_zero, max_relative = 1e-12);
}

/// V&V test: enthalpy round trip T -> h -> T for both graphite variants.
///
/// **Methodology:** for each of `NuclearGraphiteMatrixA3` and
/// `NuclearGraphiteIG110`, at temperatures 350 K, 600 K, 1200 K, and
/// 1900 K, compute h = `try_get_h`(material, T) then recover
/// T' = `try_get_temperature_from_h`(material, h) (inverted-spline initial
/// guess + Brent-Dekker refinement, eps 1e-8) and require |T' - T| <=
/// 0.005 K — the same absolute epsilon the copper round-trip test uses.
///
/// **Results (2026-08-11):** maximum absolute round-trip error across all
/// 8 (variant, temperature) combinations measured 2.61e-12 K (at 600 K,
/// both variants) — the Brent-Dekker refinement recovers the forward
/// temperature far inside the 0.005 K criterion at every point.
#[test]
pub fn nuclear_graphite_enthalpy_round_trip() {
    use uom::si::pressure::atmosphere;
    use crate::boussinesq_thermophysical_properties::specific_enthalpy::{
        try_get_h, try_get_temperature_from_h,
    };

    let pressure = Pressure::new::<atmosphere>(1.0);

    let mut max_abs_error_kelvin: f64 = 0.0;

    for material in [
        Material::Solid(SolidMaterial::NuclearGraphiteMatrixA3),
        Material::Solid(SolidMaterial::NuclearGraphiteIG110),
    ] {
        for temp_kelvin in [350.0, 600.0, 1200.0, 1900.0] {
            let temperature = ThermodynamicTemperature::new::<kelvin>(temp_kelvin);

            let enthalpy = try_get_h(material, temperature, pressure).unwrap();

            let temperature_recovered =
                try_get_temperature_from_h(material, enthalpy, pressure).unwrap();

            let abs_error = (temperature_recovered.get::<kelvin>() - temp_kelvin).abs();

            println!(
                "{:?} round trip at {} K: recovered {} K (error {:e} K)",
                material,
                temp_kelvin,
                temperature_recovered.get::<kelvin>(),
                abs_error
            );

            if abs_error > max_abs_error_kelvin {
                max_abs_error_kelvin = abs_error;
            }

            approx::assert_abs_diff_eq!(
                temp_kelvin,
                temperature_recovered.get::<kelvin>(),
                epsilon = 0.005
            );
        }
    }

    println!("max round-trip error: {:e} K", max_abs_error_kelvin);
}

/// V&V test: both new enum variants dispatch through the public property
/// entry points.
///
/// **Methodology:** for each of `NuclearGraphiteMatrixA3` and
/// `NuclearGraphiteIG110` at 600 K and 1 atm, call the public dispatchers
/// `try_get_kappa_thermal_conductivity`, `try_get_cp`, and `try_get_rho`
/// and require all to return `Ok` with strictly positive values that match
/// the underlying free functions of this module to 1e-12 relative.
///
/// **Results (2026-08-11):** all six dispatch calls returned `Ok`. Measured
/// at 600 K: matrix A3 k = 40.854469121754626 W/(m K), IG-110 k =
/// 42.5192 W/(m K), shared cp = 1390.7900000000013 J/(kg K) (the 600 K
/// table node, reproduced to f64 roundoff), matrix rho = 1730 kg/m^3,
/// IG-110 rho = 1770 kg/m^3 — each equal to its free-function value.
#[test]
pub fn nuclear_graphite_enum_variants_dispatch_at_600_k() {
    use uom::si::pressure::atmosphere;
    use crate::boussinesq_thermophysical_properties::density::try_get_rho;
    use crate::boussinesq_thermophysical_properties::specific_heat_capacity::try_get_cp;
    use crate::boussinesq_thermophysical_properties::thermal_conductivity::try_get_kappa_thermal_conductivity;

    let temperature = ThermodynamicTemperature::new::<kelvin>(600.0);
    let pressure = Pressure::new::<atmosphere>(1.0);

    let matrix = Material::Solid(SolidMaterial::NuclearGraphiteMatrixA3);
    let ig_110 = Material::Solid(SolidMaterial::NuclearGraphiteIG110);

    // thermal conductivity
    let k_matrix = try_get_kappa_thermal_conductivity(matrix, temperature, pressure).unwrap();
    let k_ig_110 = try_get_kappa_thermal_conductivity(ig_110, temperature, pressure).unwrap();
    println!(
        "dispatch at 600 K: matrix k = {} W/(m K), IG-110 k = {} W/(m K)",
        k_matrix.get::<watt_per_meter_kelvin>(),
        k_ig_110.get::<watt_per_meter_kelvin>()
    );
    assert!(k_matrix.get::<watt_per_meter_kelvin>() > 0.0);
    assert!(k_ig_110.get::<watt_per_meter_kelvin>() > 0.0);
    approx::assert_relative_eq!(
        k_matrix.get::<watt_per_meter_kelvin>(),
        nuclear_graphite_matrix_a3_thermal_conductivity_zero_fluence(temperature)
            .unwrap()
            .get::<watt_per_meter_kelvin>(),
        max_relative = 1e-12
    );
    approx::assert_relative_eq!(
        k_ig_110.get::<watt_per_meter_kelvin>(),
        nuclear_graphite_ig_110_thermal_conductivity_unirradiated(temperature)
            .unwrap()
            .get::<watt_per_meter_kelvin>(),
        max_relative = 1e-12
    );

    // specific heat capacity (shared table; 600 K is a node: 1390.79)
    let cp_matrix = try_get_cp(matrix, temperature, pressure).unwrap();
    let cp_ig_110 = try_get_cp(ig_110, temperature, pressure).unwrap();
    println!(
        "dispatch at 600 K: cp = {} J/(kg K)",
        cp_matrix.get::<joule_per_kilogram_kelvin>()
    );
    assert!(cp_matrix.get::<joule_per_kilogram_kelvin>() > 0.0);
    approx::assert_relative_eq!(
        cp_matrix.get::<joule_per_kilogram_kelvin>(),
        cp_ig_110.get::<joule_per_kilogram_kelvin>(),
        max_relative = 1e-12
    );
    approx::assert_relative_eq!(
        cp_matrix.get::<joule_per_kilogram_kelvin>(),
        1390.79,
        max_relative = 1e-12
    );

    // density
    let rho_matrix = try_get_rho(matrix, temperature, pressure).unwrap();
    let rho_ig_110 = try_get_rho(ig_110, temperature, pressure).unwrap();
    println!(
        "dispatch at 600 K: matrix rho = {} kg/m^3, IG-110 rho = {} kg/m^3",
        rho_matrix.get::<kilogram_per_cubic_meter>(),
        rho_ig_110.get::<kilogram_per_cubic_meter>()
    );
    approx::assert_relative_eq!(
        rho_matrix.get::<kilogram_per_cubic_meter>(),
        1730.0,
        max_relative = 1e-12
    );
    approx::assert_relative_eq!(
        rho_ig_110.get::<kilogram_per_cubic_meter>(),
        1770.0,
        max_relative = 1e-12
    );
}

/// Butland & Maddison polynomial 3 against the VTB cp table it underlies.
///
/// **Methodology:** evaluate the polynomial (1 cal = 4.184 J) at the table's
/// 18 nodes (300-2000 K) and compare with the VTB table, which is the same
/// polynomial evaluated with the International Table calorie (4.1868 J).
/// **Pass criterion:** every node agrees to within 0.1 % (the calorie ratio
/// 4.1868/4.184 = 1.00067 accounts for all of it).
/// **Result (2026-09-28):** the ratio table/polynomial is 1.000669 at 300,
/// 1000 and 2000 K (i.e. exactly the calorie ratio), so the table IS
/// polynomial 3, and its 2000 K ceiling was where the deck stopped
/// tabulating, not a limit of the correlation (valid to 3000 K).
#[test]
fn butland_maddison_polynomial_reproduces_the_vtb_table_up_to_the_calorie() {
    for t in (300..=2000).step_by(100) {
        let temp = ThermodynamicTemperature::new::<kelvin>(t as f64);
        let poly = nuclear_graphite_specific_heat_capacity_butland_maddison_polynomial(temp)
            .unwrap()
            .get::<joule_per_kilogram_kelvin>();
        let table = nuclear_graphite_specific_heat_capacity_butland_maddison_spline(temp)
            .unwrap()
            .get::<joule_per_kilogram_kelvin>();
        let table_over_poly = table / poly;
        assert!(
            (table_over_poly - 4.1868 / 4.184).abs() < 1e-4,
            "T = {t} K: table/polynomial {table_over_poly}"
        );
    }
}

/// The polynomial extends past the old 2000 K ceiling to its published
/// 3000 K limit, stays physical (positive, rising towards the Dulong-Petit
/// region, below 3R/M = 2077 J/(kg K) x a small margin), and refuses outside
/// 250-3000 K.
#[test]
fn butland_maddison_polynomial_covers_250_to_3000_k_and_refuses_outside() {
    let cp = |t: f64| {
        nuclear_graphite_specific_heat_capacity_butland_maddison_polynomial(
            ThermodynamicTemperature::new::<kelvin>(t),
        )
    };
    // Hand evaluation of the published formula (x 4184 J/kg per cal/g).
    let hand = |t: f64| {
        (0.54212 - 2.42667e-6 * t - 90.2725 / t - 43449.3 / (t * t) + 1.59309e7 / t.powi(3)
            - 1.43688e9 / t.powi(4))
            * 4184.0
    };
    for t in [250.0, 300.0, 1000.0, 2000.0, 2500.0, 3000.0] {
        let v = cp(t).unwrap().get::<joule_per_kilogram_kelvin>();
        assert!((v - hand(t)).abs() < 1e-9 * hand(t), "T = {t} K");
    }
    // 2500 K and 3000 K: 2066.8 and 2094.1 J/(kg K).
    let v2500 = cp(2500.0).unwrap().get::<joule_per_kilogram_kelvin>();
    let v3000 = cp(3000.0).unwrap().get::<joule_per_kilogram_kelvin>();
    assert!((v2500 - 2066.79).abs() < 0.01 && (v3000 - 2094.07).abs() < 0.01);
    let mut last = 0.0;
    for t in (250..=3000).step_by(50) {
        let v = cp(t as f64).unwrap().get::<joule_per_kilogram_kelvin>();
        assert!(v > last, "cp must rise monotonically, T = {t} K");
        last = v;
    }
    assert!(cp(249.0).is_err());
    assert!(cp(3001.0).is_err());
}


/// V&V test: the high-temperature A3 variant
/// ([`SolidMaterial::NuclearGraphiteMatrixA3HighTemp`], added 2026-09-28).
///
/// **Methodology:** through the public dispatchers (`try_get_cp`,
/// `try_get_kappa_thermal_conductivity`, `try_get_rho`, the enum's
/// `max_temperature`/`min_temperature`) check that
///
/// 1. cp is Butland & Maddison polynomial 3 at every 100 K from 300 to 3000 K
///    (relative 1e-12 against the free function);
/// 2. k is **bit-identical** to the base `NuclearGraphiteMatrixA3` variant at
///    every 50 K from 300 to 2000 K — the variant must not move any number
///    inside the old window;
/// 3. k above 2000 K is pinned against a hand evaluation of the published form
///    at 2500 and 3000 K;
/// 4. density is the base variant's 1730 kg/m^3;
/// 5. the window is 300-3000 K and cp, k both refuse at 299 K and 3001 K.
///
/// **Results (2026-09-28):** all pass. Pinned values at zero fluence:
/// k = 21.983914920210555 W/(m K) at 2000 K, 22.91263711774591 at 2500 K,
/// 25.253758433921842 at 3000 K (+14.87 % over 2000-3000 K — the fitted
/// form's minimum is at 2029.9 K, k = 21.9793). **Above 2000 K these are
/// extrapolations of the correlation, not validated values** — the test pins
/// the arithmetic, not the physics.
#[test]
pub fn nuclear_graphite_a3_high_temp_variant_dispatch_and_window() {
    use uom::si::pressure::atmosphere;
    use crate::boussinesq_thermophysical_properties::density::try_get_rho;
    use crate::boussinesq_thermophysical_properties::specific_heat_capacity::try_get_cp;
    use crate::boussinesq_thermophysical_properties::thermal_conductivity::try_get_kappa_thermal_conductivity;

    let p = Pressure::new::<atmosphere>(1.0);
    let high = Material::Solid(SolidMaterial::NuclearGraphiteMatrixA3HighTemp);
    let base = Material::Solid(SolidMaterial::NuclearGraphiteMatrixA3);
    let t = |k: f64| ThermodynamicTemperature::new::<kelvin>(k);

    // 1. cp is the polynomial over the whole window.
    for tk in (300..=3000).step_by(100) {
        let tk = tk as f64;
        let dispatched = try_get_cp(high, t(tk), p)
            .unwrap()
            .get::<joule_per_kilogram_kelvin>();
        let poly = nuclear_graphite_specific_heat_capacity_butland_maddison_polynomial(t(tk))
            .unwrap()
            .get::<joule_per_kilogram_kelvin>();
        approx::assert_relative_eq!(dispatched, poly, max_relative = 1e-12);
    }

    // 2. k identical to the base variant inside the old window.
    for tk in (300..=2000).step_by(50) {
        let tk = tk as f64;
        let k_high = try_get_kappa_thermal_conductivity(high, t(tk), p)
            .unwrap()
            .get::<watt_per_meter_kelvin>();
        let k_base = try_get_kappa_thermal_conductivity(base, t(tk), p)
            .unwrap()
            .get::<watt_per_meter_kelvin>();
        assert_eq!(k_high.to_bits(), k_base.to_bits(), "T = {tk} K");
    }

    // 3. k pinned above 2000 K (extrapolated -- see the function doc).
    let k_at = |tk: f64| {
        try_get_kappa_thermal_conductivity(high, t(tk), p)
            .unwrap()
            .get::<watt_per_meter_kelvin>()
    };
    approx::assert_relative_eq!(k_at(2000.0), 21.983914920210555, max_relative = 1e-12);
    approx::assert_relative_eq!(k_at(2500.0), 22.91263711774591, max_relative = 1e-12);
    approx::assert_relative_eq!(k_at(3000.0), 25.253758433921842, max_relative = 1e-12);

    // 4. density as the base variant.
    approx::assert_relative_eq!(
        try_get_rho(high, t(2500.0), p)
            .unwrap()
            .get::<kilogram_per_cubic_meter>(),
        1730.0,
        max_relative = 1e-12
    );

    // 5. the window, and refusal outside it.
    assert_eq!(
        SolidMaterial::NuclearGraphiteMatrixA3HighTemp
            .max_temperature()
            .get::<kelvin>(),
        3000.0
    );
    assert_eq!(
        SolidMaterial::NuclearGraphiteMatrixA3HighTemp
            .min_temperature()
            .get::<kelvin>(),
        300.0
    );
    for tk in [299.0, 3001.0] {
        assert!(try_get_cp(high, t(tk), p).is_err(), "cp at {tk} K");
        assert!(
            try_get_kappa_thermal_conductivity(high, t(tk), p).is_err(),
            "k at {tk} K"
        );
    }
    // the base variant still refuses above 2000 K -- it is unchanged
    assert!(try_get_kappa_thermal_conductivity(base, t(2001.0), p).is_err());
    assert!(try_get_cp(base, t(2001.0), p).is_err());
}

/// V&V test: the high-temperature variant's enthalpy is the exact integral of
/// its cp, and inverts.
///
/// **Methodology:** (a) central finite difference of
/// [`nuclear_graphite_matrix_a3_high_temp_specific_enthalpy`] with a 1e-3 K
/// half-step against the polynomial cp at 300, 1000, 2000, 2500 and 2990 K,
/// relative tolerance 1e-6; (b) round trip T -> h -> T through
/// [`nuclear_graphite_matrix_a3_high_temp_temp_from_specific_enthalpy`] every
/// 10 K from 300 to 3000 K, tolerance 1e-6 K; (c) an enthalpy 1 % above
/// h(3000 K) is refused.
///
/// **Results (2026-09-28):** all pass; worst round-trip error
/// 6.8e-10 K.
#[test]
pub fn nuclear_graphite_a3_high_temp_enthalpy_is_the_integral_of_cp_and_inverts() {
    let t = |k: f64| ThermodynamicTemperature::new::<kelvin>(k);
    for tk in [300.0, 1000.0, 2000.0, 2500.0, 2990.0] {
        let dh = nuclear_graphite_matrix_a3_high_temp_specific_enthalpy(t(tk + 1e-3))
            .get::<joule_per_kilogram>()
            - nuclear_graphite_matrix_a3_high_temp_specific_enthalpy(t(tk - 1e-3))
                .get::<joule_per_kilogram>();
        let cp_fd = dh / 2e-3;
        let cp = nuclear_graphite_specific_heat_capacity_butland_maddison_polynomial(t(tk))
            .unwrap()
            .get::<joule_per_kilogram_kelvin>();
        approx::assert_relative_eq!(cp_fd, cp, max_relative = 1e-6);
    }
    let mut worst: f64 = 0.0;
    for tk in (300..=3000).step_by(10) {
        let tk = tk as f64;
        let h = nuclear_graphite_matrix_a3_high_temp_specific_enthalpy(t(tk));
        let back = nuclear_graphite_matrix_a3_high_temp_temp_from_specific_enthalpy(h)
            .unwrap()
            .get::<kelvin>();
        worst = worst.max((back - tk).abs());
    }
    println!("high-temp A3 enthalpy round trip: worst error {worst:e} K");
    assert!(worst < 1e-6);
    // an enthalpy beyond h(3000 K) is refused, not extrapolated
    let too_hot = nuclear_graphite_matrix_a3_high_temp_specific_enthalpy(t(3000.0)) * 1.01;
    assert!(nuclear_graphite_matrix_a3_high_temp_temp_from_specific_enthalpy(too_hot).is_err());
}
