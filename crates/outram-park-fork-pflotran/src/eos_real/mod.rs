//! Real IAPWS-IF97 liquid-water equation of state — a higher-fidelity
//! alternative to the slightly-compressible correlations in
//! [`crate::properties`] (bead op-v6s.15.7).
//!
//! # What this module provides
//!
//! [`IapwsWater`] wraps the workspace `tampines-steam-tables` crate (an in-house
//! IAPWS-IF97 industrial-formulation implementation) and exposes liquid-water
//! **density**, **dynamic viscosity**, **specific enthalpy**, **specific
//! internal energy**, and **isobaric specific heat** as functions of pressure
//! and temperature. It is a drop-in, higher-fidelity replacement for
//! [`crate::properties::LiquidWaterEos`], which uses a single
//! slightly-compressible exponential density law and a constant viscosity.
//!
//! # Units — SI throughout, plain `f64` at the boundary
//!
//! Unlike the `uom`-typed `tampines-steam-tables` API this module talks to, the
//! public methods here take and return **plain `f64` SI values** so downstream
//! PFLOTRAN solver code (which assembles residuals/Jacobians in `f64`) can use
//! them directly. The `uom` conversions happen internally. Specifically:
//!
//! - pressure `p` — pascal (Pa)
//! - temperature `T` — kelvin (K)
//! - density — kilogram per cubic metre (kg/m^3)
//! - dynamic viscosity — pascal-second (Pa.s)
//! - specific enthalpy / internal energy — joule per kilogram (J/kg)
//! - isobaric specific heat `c_p` — joule per kilogram-kelvin (J/(kg.K))
//!
//! # Valid region — IAPWS-IF97 Region 1 (subcooled/compressed liquid) only
//!
//! v1 covers **single-phase liquid water only**, i.e. IAPWS-IF97 **Region 1**:
//!
//! - temperature `273.15 K <= T <= 623.15 K` (0 to ~350 °C), and
//! - pressure `p_sat(T) < p <= 100 MPa`, i.e. strictly above the saturation
//!   pressure at `T` (compressed/subcooled liquid) and up to 100 MPa.
//!
//! A `(p, T)` state that is on or below the saturation line (vapour or the
//! two-phase dome), above 623.15 K (Regions 2/3/5), or above 100 MPa is
//! **rejected** with [`PflotranError::InvalidInput`] rather than silently
//! returning an out-of-region extrapolation. Two-phase, vapour, near-critical,
//! and supercritical states are out of scope for this v1 liquid EOS.
//!
//! # Underlying `tampines-steam-tables` calls
//!
//! All property evaluations route through the crate's single-phase `(T,p)`
//! forward-flash functions in
//! `tampines_steam_tables::interfaces::functional_programming::pt_flash_eqm`:
//! `v_tp_eqm_single_phase` (specific volume -> density), `h_tp_eqm_single_phase`
//! (enthalpy), `u_tp_eqm_single_phase` (internal energy), and
//! `cp_tp_eqm_single_phase` (isobaric heat capacity); plus
//! `mu_tp_eqm_single_phase` from that crate's `dynamic_viscosity` module
//! (re-exported through `pt_flash_eqm`). The saturation-line guard uses
//! `tampines_steam_tables::region_4_vap_liq_equilibrium::sat_pressure_4`.
//!
//! # Verification status
//!
//! Untrusted AI-generated draft until a human reviews it (workspace
//! `RESPONSIBLE_USE.md`). The `tampines-steam-tables` crate is itself verified
//! against the International Steam Tables (Kretzschmar & Wagner, 2019); this
//! wrapper only re-expresses those calls in `f64` SI and adds the Region-1
//! input guard. The tests below assert the wrapper is wired correctly against
//! reference IAPWS liquid-water values — they are not a re-validation of IF97.

use crate::error::PflotranError;

use uom::si::available_energy::joule_per_kilogram;
use uom::si::dynamic_viscosity::pascal_second;
use uom::si::f64::{Pressure, ThermodynamicTemperature};
use uom::si::mass_density::kilogram_per_cubic_meter;
use uom::si::pressure::pascal;
use uom::si::specific_heat_capacity::joule_per_kilogram_kelvin;
use uom::si::thermodynamic_temperature::kelvin;

use tampines_steam_tables::interfaces::functional_programming::pt_flash_eqm::{
    cp_tp_eqm_single_phase, h_tp_eqm_single_phase, mu_tp_eqm_single_phase, u_tp_eqm_single_phase,
    v_tp_eqm_single_phase,
};
use tampines_steam_tables::region_4_vap_liq_equilibrium::sat_pressure_4;

/// Lower temperature bound of IAPWS-IF97 Region 1 (the triple-point temperature),
/// in kelvin.
const T_MIN_K: f64 = 273.15;

/// Upper temperature bound of IAPWS-IF97 Region 1, in kelvin (~350 °C).
const T_MAX_K: f64 = 623.15;

/// Upper pressure bound of IAPWS-IF97 Region 1, in pascal (100 MPa).
const P_MAX_PA: f64 = 100.0e6;

/// Real IAPWS-IF97 liquid-water properties, wrapping `tampines-steam-tables`.
///
/// A drop-in, higher-fidelity alternative to
/// [`crate::properties::LiquidWaterEos`] (which uses simple
/// slightly-compressible correlations). SI units throughout; all methods take
/// plain `f64` pressure (Pa) and temperature (K) and return plain `f64` SI
/// results, converting to/from `uom` internally.
///
/// The evaluator is **stateless** — it holds no configuration — so a single
/// [`IapwsWater::new`] (or [`IapwsWater::default`]) instance can be shared and
/// called freely. All methods validate that the requested `(p, T)` lies in
/// IAPWS-IF97 Region 1 (single-phase liquid) and return
/// [`PflotranError::InvalidInput`] otherwise; see the module documentation for
/// the exact region.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct IapwsWater {}

impl IapwsWater {
    /// Construct the (stateless) IAPWS-IF97 liquid-water evaluator.
    ///
    /// Infallible — there is no configuration to validate. Equivalent to
    /// [`IapwsWater::default`].
    pub fn new() -> Self {
        Self {}
    }

    /// Liquid density (kg/m^3) at pressure `p` (Pa) and temperature `T` (K).
    ///
    /// Computed as `1 / v`, where `v` is the IAPWS-IF97 Region-1 specific
    /// volume (m^3/kg). For liquid water near ambient this is ~998 kg/m^3.
    ///
    /// # Errors
    ///
    /// [`PflotranError::InvalidInput`] if `(p, T)` is not in IAPWS-IF97
    /// Region 1 (single-phase liquid) — see the module documentation.
    pub fn density(&self, pressure_pa: f64, temperature_k: f64) -> Result<f64, PflotranError> {
        let (t, p) = self.region1_inputs(pressure_pa, temperature_k)?;
        // SpecificVolume.recip() yields a MassDensity (kg/m^3) directly.
        let rho = v_tp_eqm_single_phase(t, p).recip();
        Ok(rho.get::<kilogram_per_cubic_meter>())
    }

    /// Dynamic viscosity (Pa.s) at pressure `p` (Pa) and temperature `T` (K).
    ///
    /// Uses the IAPWS R12-08 correlation (critical-enhancement term omitted, per
    /// the upstream fast path) evaluated at the Region-1 liquid density. For
    /// liquid water at 20 °C this is ~1.0e-3 Pa.s.
    ///
    /// # Errors
    ///
    /// [`PflotranError::InvalidInput`] if `(p, T)` is not in IAPWS-IF97
    /// Region 1 (single-phase liquid).
    pub fn viscosity(&self, pressure_pa: f64, temperature_k: f64) -> Result<f64, PflotranError> {
        let (t, p) = self.region1_inputs(pressure_pa, temperature_k)?;
        Ok(mu_tp_eqm_single_phase(t, p).get::<pascal_second>())
    }

    /// Specific enthalpy (J/kg) at pressure `p` (Pa) and temperature `T` (K).
    ///
    /// The IAPWS-IF97 Region-1 specific enthalpy `h(T, p)`. It uses the IF97
    /// reference state (`h = s = 0` for the saturated liquid at the triple
    /// point), so absolute values are only meaningful as differences.
    /// Monotonically increasing with `T` at fixed `p`.
    ///
    /// # Errors
    ///
    /// [`PflotranError::InvalidInput`] if `(p, T)` is not in IAPWS-IF97
    /// Region 1 (single-phase liquid).
    pub fn enthalpy(&self, pressure_pa: f64, temperature_k: f64) -> Result<f64, PflotranError> {
        let (t, p) = self.region1_inputs(pressure_pa, temperature_k)?;
        Ok(h_tp_eqm_single_phase(t, p).get::<joule_per_kilogram>())
    }

    /// Specific internal energy (J/kg) at pressure `p` (Pa) and temperature
    /// `T` (K).
    ///
    /// The IAPWS-IF97 Region-1 specific internal energy `u(T, p) = h - p v`,
    /// on the same IF97 reference state as [`Self::enthalpy`].
    ///
    /// # Errors
    ///
    /// [`PflotranError::InvalidInput`] if `(p, T)` is not in IAPWS-IF97
    /// Region 1 (single-phase liquid).
    pub fn internal_energy(
        &self,
        pressure_pa: f64,
        temperature_k: f64,
    ) -> Result<f64, PflotranError> {
        let (t, p) = self.region1_inputs(pressure_pa, temperature_k)?;
        Ok(u_tp_eqm_single_phase(t, p).get::<joule_per_kilogram>())
    }

    /// Isobaric specific heat `c_p` (J/(kg.K)) at pressure `p` (Pa) and
    /// temperature `T` (K).
    ///
    /// The IAPWS-IF97 Region-1 isobaric specific heat capacity. For liquid
    /// water near ambient this is ~4180 J/(kg.K).
    ///
    /// # Errors
    ///
    /// [`PflotranError::InvalidInput`] if `(p, T)` is not in IAPWS-IF97
    /// Region 1 (single-phase liquid).
    pub fn specific_heat(
        &self,
        pressure_pa: f64,
        temperature_k: f64,
    ) -> Result<f64, PflotranError> {
        let (t, p) = self.region1_inputs(pressure_pa, temperature_k)?;
        Ok(cp_tp_eqm_single_phase(t, p).get::<joule_per_kilogram_kelvin>())
    }

    /// Validate `(pressure_pa, temperature_k)` as an IAPWS-IF97 Region-1
    /// (single-phase liquid) state and return the corresponding `uom`
    /// quantities ready for the `tampines-steam-tables` forward flashes.
    ///
    /// This guard exists because the upstream single-phase `(T,p)` dispatcher
    /// **panics** on an out-of-range state; validating here converts every such
    /// case into a recoverable [`PflotranError::InvalidInput`]. The accepted
    /// region is `273.15 K <= T <= 623.15 K` and `p_sat(T) < p <= 100 MPa`
    /// (strictly compressed liquid, so the underdetermined saturation line is
    /// excluded).
    fn region1_inputs(
        &self,
        pressure_pa: f64,
        temperature_k: f64,
    ) -> Result<(ThermodynamicTemperature, Pressure), PflotranError> {
        if !pressure_pa.is_finite() || !temperature_k.is_finite() {
            return Err(PflotranError::InvalidInput(format!(
                "IapwsWater: pressure and temperature must be finite, got p={pressure_pa} Pa, \
                 T={temperature_k} K"
            )));
        }
        if !(T_MIN_K..=T_MAX_K).contains(&temperature_k) {
            return Err(PflotranError::InvalidInput(format!(
                "IapwsWater: temperature {temperature_k} K is outside IAPWS-IF97 Region 1 liquid \
                 range [{T_MIN_K}, {T_MAX_K}] K"
            )));
        }
        if pressure_pa <= 0.0 || pressure_pa > P_MAX_PA {
            return Err(PflotranError::InvalidInput(format!(
                "IapwsWater: pressure {pressure_pa} Pa is outside IAPWS-IF97 Region 1 range \
                 (0, {P_MAX_PA}] Pa"
            )));
        }

        let t = ThermodynamicTemperature::new::<kelvin>(temperature_k);
        let p = Pressure::new::<pascal>(pressure_pa);

        // Reject non-liquid states: the pressure must sit strictly above the
        // saturation pressure at this temperature. On or below p_sat the state
        // is on the two-phase line or a vapour, which Region 1 cannot represent
        // (and which would trip the upstream underdetermined-state panic).
        let p_sat_pa = sat_pressure_4(t).get::<pascal>();
        if pressure_pa <= p_sat_pa {
            return Err(PflotranError::InvalidInput(format!(
                "IapwsWater: pressure {pressure_pa} Pa is at or below the saturation pressure \
                 {p_sat_pa} Pa at T={temperature_k} K — not single-phase liquid (Region 1)"
            )));
        }

        Ok((t, p))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::properties::LiquidWaterEos;
    use uom::si::f64::Pressure as UomPressure;
    use uom::si::mass_density::kilogram_per_cubic_meter as kg_per_m3;
    use uom::si::pressure::pascal as pa_unit;

    const P_ATM: f64 = 101_325.0;

    #[test]
    fn density_liquid_water_at_20c_is_about_998() {
        let eos = IapwsWater::new();
        let rho = eos.density(P_ATM, 293.15).unwrap();
        // IAPWS reference: rho(101325 Pa, 20 C) ~ 998.2 kg/m^3.
        assert!(
            (rho - 998.2).abs() < 1.0,
            "expected ~998.2 kg/m^3 at 20 C, got {rho}"
        );
    }

    #[test]
    fn density_decreases_with_temperature_at_fixed_pressure() {
        let eos = IapwsWater::new();
        let rho_20c = eos.density(P_ATM, 293.15).unwrap();
        let rho_80c = eos.density(P_ATM, 353.15).unwrap();
        // IAPWS reference: rho(101325 Pa, 80 C) ~ 971.8 kg/m^3.
        assert!(
            (rho_80c - 971.8).abs() < 1.5,
            "expected ~971.8 kg/m^3 at 80 C, got {rho_80c}"
        );
        assert!(
            rho_80c < rho_20c,
            "density must fall as T rises at fixed p: 80 C -> {rho_80c}, 20 C -> {rho_20c}"
        );
    }

    #[test]
    fn viscosity_at_20c_is_about_1_millipascal_second_and_falls_with_temperature() {
        let eos = IapwsWater::new();
        let mu_20c = eos.viscosity(P_ATM, 293.15).unwrap();
        let mu_80c = eos.viscosity(P_ATM, 353.15).unwrap();
        // IAPWS reference: mu(20 C) ~ 1.0016e-3 Pa.s.
        assert!(
            (mu_20c - 1.0e-3).abs() < 0.1e-3,
            "expected ~1.0e-3 Pa.s at 20 C, got {mu_20c}"
        );
        assert!(
            mu_80c < mu_20c,
            "viscosity must fall as T rises: 80 C -> {mu_80c}, 20 C -> {mu_20c}"
        );
    }

    #[test]
    fn enthalpy_and_internal_energy_increase_with_temperature() {
        let eos = IapwsWater::new();
        let h_20c = eos.enthalpy(P_ATM, 293.15).unwrap();
        let h_80c = eos.enthalpy(P_ATM, 353.15).unwrap();
        assert!(
            h_80c > h_20c,
            "enthalpy must rise with T: 80 C -> {h_80c}, 20 C -> {h_20c}"
        );
        // Liquid c_p ~ 4180 J/(kg.K) over a 60 K rise => ~2.5e5 J/kg.
        assert!(
            (h_80c - h_20c) > 2.0e5 && (h_80c - h_20c) < 3.0e5,
            "enthalpy rise over 60 K should be ~2.5e5 J/kg, got {}",
            h_80c - h_20c
        );

        let u_20c = eos.internal_energy(P_ATM, 293.15).unwrap();
        let u_80c = eos.internal_energy(P_ATM, 353.15).unwrap();
        assert!(
            u_80c > u_20c,
            "internal energy must rise with T: 80 C -> {u_80c}, 20 C -> {u_20c}"
        );
    }

    #[test]
    fn specific_heat_near_ambient_is_about_4180() {
        let eos = IapwsWater::new();
        let cp = eos.specific_heat(P_ATM, 293.15).unwrap();
        assert!(
            (cp - 4180.0).abs() < 60.0,
            "expected ~4180 J/(kg.K) at 20 C, got {cp}"
        );
    }

    #[test]
    fn agrees_with_simple_slightly_compressible_model_at_25c() {
        // Sanity that the IAPWS wrapper is wired correctly: at 25 C, 1 atm the
        // real density must land within a few percent of the simple model's
        // reference density (1000 kg/m^3 at p_ref = 1 atm).
        let real = IapwsWater::new().density(P_ATM, 298.15).unwrap();

        let simple = LiquidWaterEos::water()
            .density(UomPressure::new::<pa_unit>(P_ATM))
            .get::<kg_per_m3>();

        let rel = (real - simple).abs() / simple;
        assert!(
            rel < 0.03,
            "real IAPWS density {real} vs simple model {simple} differ by {:.3}% (> 3%)",
            rel * 100.0
        );
        // And the real value should be the physical ~997 kg/m^3 at 25 C.
        assert!(
            (real - 997.0).abs() < 1.5,
            "expected ~997.0 kg/m^3 at 25 C, got {real}"
        );
    }

    #[test]
    fn rejects_states_outside_region_1() {
        let eos = IapwsWater::new();
        // Too hot (Region 2/3 territory).
        assert!(eos.density(P_ATM, 700.0).is_err());
        // Too cold (below the triple-point temperature).
        assert!(eos.density(P_ATM, 250.0).is_err());
        // Over 100 MPa.
        assert!(eos.density(150.0e6, 293.15).is_err());
        // On/below the saturation line at 150 C (p_sat ~ 4.76e5 Pa): 1 atm is
        // vapour there, not liquid.
        assert!(eos.density(P_ATM, 423.15).is_err());
        // Non-finite inputs.
        assert!(eos.density(f64::NAN, 293.15).is_err());
        assert!(eos.viscosity(P_ATM, f64::INFINITY).is_err());
    }
}
