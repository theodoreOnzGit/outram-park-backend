//! IAPWS-IF97 water/steam properties — **the one substitution allowed inside
//! the reference translation**.
//!
//! # Provenance
//!
//! The MATLAB calls `IAPWS_IF97.m`, which is third-party
//! (`Copyright (c) 2013 Mark Mifofski`) and is **not** ported; see
//! `docs/bedok-port-scoping.md` §3. This module is a thin adapter that answers
//! the same entry points out of the workspace's own
//! [`tampines_steam_tables`] crate, which implements IAPWS-IF97 in Rust.
//!
//! Everything else in `src/reference/th/` is translated from Than Yan Ren's
//! (SNRSI) MATLAB, snapshot sha256 `e45cd6f57be2087c…`.
//!
//! # Units — the whole point of this module
//!
//! `tampines-steam-tables` is strictly SI (Pa, K, J/kg, m³/kg, W/(m·K), Pa·s)
//! and `uom`-typed. The MATLAB `IAPWS_IF97` wrapper uses the XSteam
//! convention: **MPa, K, kJ/kg, m³/kg, kJ/(kg·K), W/(m·K), Pa·s**, all bare
//! `f64`. Every function here takes and returns the *MATLAB* convention, so
//! the ported solvers read exactly like the original. The unit conversion
//! happens here and nowhere else.
//!
//! # Why this is a parity risk, and what to check
//!
//! Substituting an IF97 implementation *inside* the reference means every
//! downstream comparison silently inherits any disagreement. The gate for it
//! is not implementation-against-implementation but **both against the
//! published IAPWS-IF97 verification tables**, over the pressure/enthalpy
//! envelope the four benchmark cases exercise (PWR ~15.5 MPa, BWR ~6.7 MPa
//! plus the two-phase region). That check is `tampines-steam-tables`' own
//! responsibility and is **not** performed by this module.
//!
//! # The saturation-line dispatch hazard
//!
//! The MATLAB repeatedly evaluates liquid properties at
//! `min(temps, Tsat - 2*eps)`. As
//! [`MATLAB_EPS`](super::MATLAB_EPS) documents, `Tsat - 2*eps` is a **no-op**
//! in `f64` at reactor temperatures, so those calls land exactly on the
//! saturation temperature. `tampines-steam-tables`' `(T,p)` region dispatch
//! resolves such a point by comparing `p` against `p_sat(T)`; whether it lands
//! in region 1 or region 2 then depends on the last bit of the
//! `T_sat(p) -> p_sat(T)` round trip. [`thermal_conductivity_pt`] and
//! [`dynamic_viscosity_pt`] therefore go through the generic dispatch (as the
//! MATLAB does) but are guarded against the out-of-envelope panic; the
//! explicitly-regioned entry points ([`enthalpy_region1_pt`] and friends)
//! force their region and are immune.

use tampines_steam_tables::dynamic_viscosity::mu_tp_eqm_single_phase;
use tampines_steam_tables::interfaces::functional_programming::ph_flash_eqm::t_ph_eqm;
use tampines_steam_tables::region_1_subcooled_liquid::{cp_tp_1, h_tp_1, v_tp_1};
use tampines_steam_tables::region_2_vapour::{h_tp_2, v_tp_2};
use tampines_steam_tables::region_4_vap_liq_equilibrium::sat_temp_4;
use tampines_steam_tables::thermal_conductivity::lambda_tp_eqm_single_phase;

use uom::si::available_energy::kilojoule_per_kilogram;
use uom::si::dynamic_viscosity::pascal_second;
use uom::si::f64::{AvailableEnergy, Pressure, ThermodynamicTemperature};
use uom::si::pressure::megapascal;
use uom::si::specific_heat_capacity::kilojoule_per_kilogram_kelvin;
use uom::si::specific_volume::cubic_meter_per_kilogram;
use uom::si::thermal_conductivity::watt_per_meter_kelvin;
use uom::si::thermodynamic_temperature::kelvin;

/// Lowest temperature \[K\] the IF97 forward equations are defined at.
const T_MIN_KELVIN: f64 = 273.15;
/// Highest temperature \[K\] the region-1/2/5 forward equations cover.
const T_MAX_KELVIN: f64 = 1073.15;
/// Highest pressure \[MPa\] the IF97 forward equations are defined at.
const P_MAX_MEGAPASCAL: f64 = 100.0;

/// Whether `(pressure_mpa, temperature_kelvin)` is inside the IF97
/// forward-equation envelope that `tampines-steam-tables` will dispatch
/// without panicking.
fn inside_forward_envelope(pressure_mpa: f64, temperature_kelvin: f64) -> bool {
    pressure_mpa.is_finite()
        && temperature_kelvin.is_finite()
        && pressure_mpa > 0.0
        && pressure_mpa <= P_MAX_MEGAPASCAL
        && temperature_kelvin >= T_MIN_KELVIN
        && temperature_kelvin <= T_MAX_KELVIN
}

fn pressure(pressure_mpa: f64) -> Pressure {
    Pressure::new::<megapascal>(pressure_mpa)
}

fn temperature(temperature_kelvin: f64) -> ThermodynamicTemperature {
    ThermodynamicTemperature::new::<kelvin>(temperature_kelvin)
}

fn enthalpy(enthalpy_kj_per_kg: f64) -> AvailableEnergy {
    AvailableEnergy::new::<kilojoule_per_kilogram>(enthalpy_kj_per_kg)
}

/// Saturation temperature \[K\] at `pressure_mpa` \[MPa\].
///
/// MATLAB `IAPWS_IF97('Tsat_p', p)`. Valid from the triple point
/// (611.657 Pa) to the critical pressure (22.064 MPa).
#[must_use]
pub fn saturation_temperature(pressure_mpa: f64) -> f64 {
    sat_temp_4(pressure(pressure_mpa)).get::<kelvin>()
}

/// Region-1 (subcooled liquid) specific enthalpy \[kJ/kg\] at
/// `pressure_mpa` \[MPa\] and `temperature_kelvin` \[K\].
///
/// MATLAB `IAPWS_IF97('h1_pT', p, T)`. The region-1 equation is *forced*, not
/// dispatched, exactly as in the MATLAB. Valid 273.15–623.15 K up to 100 MPa.
#[must_use]
pub fn enthalpy_region1_pt(pressure_mpa: f64, temperature_kelvin: f64) -> f64 {
    h_tp_1(temperature(temperature_kelvin), pressure(pressure_mpa)).get::<kilojoule_per_kilogram>()
}

/// Region-2 (vapour) specific enthalpy \[kJ/kg\] at `pressure_mpa` \[MPa\] and
/// `temperature_kelvin` \[K\].
///
/// MATLAB `IAPWS_IF97('h2_pT', p, T)`. Valid to 1073.15 K.
#[must_use]
pub fn enthalpy_region2_pt(pressure_mpa: f64, temperature_kelvin: f64) -> f64 {
    h_tp_2(temperature(temperature_kelvin), pressure(pressure_mpa)).get::<kilojoule_per_kilogram>()
}

/// Region-1 specific volume \[m³/kg\] at `pressure_mpa` \[MPa\] and
/// `temperature_kelvin` \[K\].
///
/// MATLAB `IAPWS_IF97('v1_pT', p, T)`. Note the unit: the MATLAB keeps
/// specific volume in SI m³/kg even though everything around it is cgs, and
/// converts with `1/v/1000` to get g/cm³.
#[must_use]
pub fn specific_volume_region1_pt(pressure_mpa: f64, temperature_kelvin: f64) -> f64 {
    v_tp_1(temperature(temperature_kelvin), pressure(pressure_mpa))
        .get::<cubic_meter_per_kilogram>()
}

/// Region-2 specific volume \[m³/kg\] at `pressure_mpa` \[MPa\] and
/// `temperature_kelvin` \[K\]. MATLAB `IAPWS_IF97('v2_pT', p, T)`.
#[must_use]
pub fn specific_volume_region2_pt(pressure_mpa: f64, temperature_kelvin: f64) -> f64 {
    v_tp_2(temperature(temperature_kelvin), pressure(pressure_mpa))
        .get::<cubic_meter_per_kilogram>()
}

/// Region-1 isobaric specific heat \[kJ/(kg·K)\] at `pressure_mpa` \[MPa\] and
/// `temperature_kelvin` \[K\]. MATLAB `IAPWS_IF97('cp1_pT', p, T)`.
#[must_use]
pub fn isobaric_heat_capacity_region1_pt(pressure_mpa: f64, temperature_kelvin: f64) -> f64 {
    cp_tp_1(temperature(temperature_kelvin), pressure(pressure_mpa))
        .get::<kilojoule_per_kilogram_kelvin>()
}

/// Saturated-liquid specific enthalpy \[kJ/kg\] at `pressure_mpa` \[MPa\].
///
/// MATLAB `IAPWS_IF97('hL_p', p)`, evaluated as the region-1 equation on the
/// saturation line.
#[must_use]
pub fn saturated_liquid_enthalpy(pressure_mpa: f64) -> f64 {
    enthalpy_region1_pt(pressure_mpa, saturation_temperature(pressure_mpa))
}

/// Saturated-vapour specific enthalpy \[kJ/kg\] at `pressure_mpa` \[MPa\].
///
/// MATLAB `IAPWS_IF97('hV_p', p)`, evaluated as the region-2 equation on the
/// saturation line.
#[must_use]
pub fn saturated_vapour_enthalpy(pressure_mpa: f64) -> f64 {
    enthalpy_region2_pt(pressure_mpa, saturation_temperature(pressure_mpa))
}

/// Saturated-vapour specific volume \[m³/kg\] at `pressure_mpa` \[MPa\].
/// MATLAB `IAPWS_IF97('vV_p', p)`.
#[must_use]
pub fn saturated_vapour_specific_volume(pressure_mpa: f64) -> f64 {
    specific_volume_region2_pt(pressure_mpa, saturation_temperature(pressure_mpa))
}

/// Temperature \[K\] from pressure \[MPa\] and specific enthalpy \[kJ/kg\].
///
/// MATLAB `IAPWS_IF97('T_ph', p, h)` — the backward `(p,h)` flash, dispatched
/// across regions 1–4. In the two-phase region it returns `Tsat(p)`.
///
/// Returns `NaN` rather than panicking when the state is outside the IF97
/// backward-equation envelope. The MATLAB returns `NaN` there too, and
/// `singleflow1devap.m:117` explicitly guards against it
/// (`temps(~isfinite(temps)) = Tsat`).
#[must_use]
pub fn temperature_ph(pressure_mpa: f64, enthalpy_kj_per_kg: f64) -> f64 {
    if !pressure_mpa.is_finite()
        || !enthalpy_kj_per_kg.is_finite()
        || pressure_mpa <= 0.0
        || pressure_mpa > P_MAX_MEGAPASCAL
    {
        return f64::NAN;
    }
    t_ph_eqm(pressure(pressure_mpa), enthalpy(enthalpy_kj_per_kg)).get::<kelvin>()
}

/// Thermal conductivity \[W/(m·K)\] at `pressure_mpa` \[MPa\] and
/// `temperature_kelvin` \[K\].
///
/// MATLAB `IAPWS_IF97('k_pT', p, T)`. The MATLAB divides the result by 100 to
/// reach W/(cm·K); this function keeps the SI value so the call site reads the
/// same as the original.
///
/// Returns `NaN` outside the IF97 forward envelope
/// (273.15–1073.15 K, 0 < p ≤ 100 MPa) instead of panicking — see the module
/// note on the saturation-line dispatch hazard.
#[must_use]
pub fn thermal_conductivity_pt(pressure_mpa: f64, temperature_kelvin: f64) -> f64 {
    if !inside_forward_envelope(pressure_mpa, temperature_kelvin) {
        return f64::NAN;
    }
    lambda_tp_eqm_single_phase(temperature(temperature_kelvin), pressure(pressure_mpa))
        .get::<watt_per_meter_kelvin>()
}

/// Dynamic viscosity \[Pa·s\] at `pressure_mpa` \[MPa\] and
/// `temperature_kelvin` \[K\]. MATLAB `IAPWS_IF97('mu_pT', p, T)`.
///
/// Returns `NaN` outside the IF97 forward envelope instead of panicking.
#[must_use]
pub fn dynamic_viscosity_pt(pressure_mpa: f64, temperature_kelvin: f64) -> f64 {
    if !inside_forward_envelope(pressure_mpa, temperature_kelvin) {
        return f64::NAN;
    }
    mu_tp_eqm_single_phase(temperature(temperature_kelvin), pressure(pressure_mpa))
        .get::<pascal_second>()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Saturation temperature at the two benchmark pressures, against the
    /// published IAPWS-IF97 saturation line.
    ///
    /// **Methodology.** `Tsat(p)` from [`saturation_temperature`] compared with
    /// published steam-table saturation values (Wagner & Kretzschmar,
    /// *International Steam Tables*). At 15.5 MPa the tabulated value is
    /// 617.94 K. 6.7 MPa falls between table entries, so the reference is
    /// interpolated between 6.5 MPa (280.86 °C = 554.01 K) and 7.0 MPa
    /// (285.83 °C = 558.98 K), giving about 556.0 K. Pass criterion: within
    /// 0.1 K of 617.94 K and within 0.2 K of 556.0 K (the wider band absorbing
    /// the interpolation).
    ///
    /// **Result (2026-08-05, `tampines-steam-tables` 0.2.5).** 617.94 K at
    /// 15.5 MPa and 556.03 K at 6.7 MPa. Interpretation: the substituted IF97
    /// reproduces the saturation line at the PWR (NEACRP A1/A2) and BWR
    /// (NEACRP D1) operating pressures. This is a spot check, **not** the §3
    /// parity gate, which needs the full verification tables over the
    /// benchmarks' envelope.
    #[test]
    fn saturation_temperature_at_the_benchmark_pressures() {
        let pwr = saturation_temperature(15.5);
        let bwr = saturation_temperature(6.7);
        assert!((pwr - 617.94).abs() < 0.1, "15.5 MPa: got {pwr} K");
        assert!((bwr - 556.0).abs() < 0.2, "6.7 MPa: got {bwr} K");
    }

    /// Saturated liquid and vapour enthalpy bracket the latent heat sensibly.
    ///
    /// **Methodology.** 6.7 MPa lies between table entries; interpolating the
    /// published saturation table between 6.5 MPa (`hf = 1241.1`,
    /// `hg = 2778.6` kJ/kg) and 7.0 MPa (`hf = 1267.4`, `hg = 2772.6` kJ/kg)
    /// gives about `hf = 1251.6` and `hg = 2776.2` kJ/kg — note `hg` is already
    /// past its maximum and falling with pressure. Pass criterion: within
    /// 5 kJ/kg of each.
    ///
    /// **Result (2026-08-05).** `hf = 1251.81`, `hg = 2776.15` kJ/kg — 0.2 and
    /// 0.05 kJ/kg from the interpolated references, i.e. inside the
    /// interpolation error itself. Latent heat 1524.3 kJ/kg.
    #[test]
    fn saturation_enthalpies_at_the_bwr_pressure() {
        let h_l = saturated_liquid_enthalpy(6.7);
        let h_v = saturated_vapour_enthalpy(6.7);
        assert!((h_l - 1251.6).abs() < 5.0, "hL: got {h_l} kJ/kg");
        assert!((h_v - 2776.2).abs() < 5.0, "hV: got {h_v} kJ/kg");
        assert!(h_v > h_l);
    }

    #[test]
    fn liquid_density_at_pwr_inlet_is_physical() {
        // 15.5 MPa, 559.15 K (NEACRP A2 inlet). Pressurised water there is
        // about 0.75 g/cm^3.
        let v = specific_volume_region1_pt(15.5, 559.15);
        let rho_g_per_cm3 = 1.0 / v / 1000.0;
        assert!(
            (0.70..0.80).contains(&rho_g_per_cm3),
            "got {rho_g_per_cm3} g/cm3"
        );
    }

    #[test]
    fn temperature_ph_round_trips_through_the_region1_enthalpy() {
        let t_in = 559.15;
        let h = enthalpy_region1_pt(15.5, t_in);
        let t_out = temperature_ph(15.5, h);
        assert!((t_out - t_in).abs() < 0.05, "got {t_out} K from {t_in} K");
    }

    #[test]
    fn transport_properties_are_physical_at_the_pwr_inlet() {
        // Liquid water at 15.5 MPa / 559.15 K: k ~ 0.58 W/(m K),
        // mu ~ 9e-5 Pa s.
        let k = thermal_conductivity_pt(15.5, 559.15);
        let mu = dynamic_viscosity_pt(15.5, 559.15);
        assert!((0.4..0.8).contains(&k), "k = {k} W/(m K)");
        assert!((5e-5..2e-4).contains(&mu), "mu = {mu} Pa s");
    }

    #[test]
    fn out_of_envelope_states_give_nan_rather_than_panicking() {
        assert!(thermal_conductivity_pt(15.5, 5000.0).is_nan());
        assert!(dynamic_viscosity_pt(-1.0, 559.15).is_nan());
        assert!(temperature_ph(f64::NAN, 1200.0).is_nan());
    }
}
