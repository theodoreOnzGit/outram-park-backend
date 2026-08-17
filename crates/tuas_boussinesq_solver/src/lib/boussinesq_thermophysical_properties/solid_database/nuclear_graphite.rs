//! # Nuclear graphite (HTR-10 / HTR-PM pebble matrix A3, and IG-110)
//!
//! Thermophysical property correlations for two grades of nuclear graphite:
//!
//! - **Matrix graphite, A3 grade** — the fuel-pebble matrix graphite of the
//!   HTR-10 / HTR-PM pebble-bed reactors
//!   ([`SolidMaterial::NuclearGraphiteMatrixA3`]).
//! - **IG-110** — the fine-grained isotropic reflector-grade graphite used in
//!   the HTTR and HTR-10 reflector structures
//!   ([`SolidMaterial::NuclearGraphiteIG110`]).
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
/// `TuasLibError::ThermophysicalPropertyTemperatureRangeError` for `gam`
/// outside [0, 15]: at `gam = 15` the factor is still physically positive
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
/// Valid ranges enforced: temperature 300 K to 2000 K (the deck states no
/// range; this range matches the sibling graphite cp table so all
/// nuclear-graphite properties share one coded validity window), and fluence
/// `gam` in [0, 15] (beyond which the damage factor heads to an unphysical
/// zero crossing at `gam ~ 19`). Out-of-range inputs return
/// `TuasLibError::ThermophysicalPropertyTemperatureRangeError`.
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

    let t: f64 = temperature.get::<kelvin>();
    let damage_factor: f64 = nuclear_graphite_fluence_damage_factor(fluence)?.get::<ratio>();

    let temperature_factor: f64 =
        1.0 - 9.7556e-4 * (t - 373.15) * f64::exp(-6.036e-4 * (t - 273.15));
    let density_maxwell_factor: f64 = 1740.0 / (2.2 * (1700.0 - 1740.0) + 1740.0);

    let k_value: f64 = 47.4 * temperature_factor * density_maxwell_factor * damage_factor;

    Ok(ThermalConductivity::new::<watt_per_meter_kelvin>(k_value))
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
) -> ThermodynamicTemperature {
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
        Err(_) => panic!("{:?}", h_graphite),
    };

    ThermodynamicTemperature::new::<kelvin>(temperature_from_enthalpy_kelvin)
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
