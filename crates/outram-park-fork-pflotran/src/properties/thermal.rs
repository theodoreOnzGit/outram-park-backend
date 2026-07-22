//! Thermal properties for the TH (thermal-hydraulic) flow mode (bead op-v6s.10).
//!
//! The RICHARDS slice (see [`LiquidWaterEos`](super::LiquidWaterEos)) is
//! isothermal: density depends on
//! pressure only and viscosity is constant. The **TH** flow mode adds an energy
//! balance, so it needs:
//!
//! - **Temperature-dependent liquid water** — density `rho(p, T)` with both a
//!   pressure (compressibility) and a temperature (thermal-expansion)
//!   dependence, a temperature-dependent viscosity `mu(T)`, and the transport
//!   coefficients `c_w` (specific heat) and `k_w` (thermal conductivity) that
//!   the fluid enthalpy flux and conduction terms require. See
//!   [`ThermalWaterProperties`].
//! - **Solid-matrix (rock) thermal properties** — the density, specific heat,
//!   and thermal conductivity of the porous solid that carry the solid part of
//!   the bulk energy storage and conduction. See [`RockThermalProperties`].
//!
//! All quantities are SI throughout; the `uom`-typed entry points take/return
//! [`FluidPressure`], [`FluidTemperature`], [`FluidDensity`], and
//! [`FluidViscosity`], and the scalar transport coefficients are plain `f64` in
//! their SI units (documented per method) because they have no `uom` alias here.
//!
//! # Scope & honesty
//!
//! These are deliberately **simple, closed-form** correlations chosen so their
//! analytic derivatives are exact and finite-difference-checkable — they are for
//! **verification only**, not validation. They are **not** IAPWS-IF97 and must
//! not be used where accurate steam/water properties matter; a real IF97 EOS
//! lives in the workspace's `tampines-steam-tables` crate. Per the workspace
//! `RESPONSIBLE_USE.md`, this is untrusted AI-generated draft material until a
//! human reviews it: no human V&V has been done and the correlations have not
//! been checked against a published PFLOTRAN TH reference case.

use crate::error::PflotranError;
use crate::units::{FluidDensity, FluidPressure, FluidTemperature, FluidViscosity};
use uom::si::dynamic_viscosity::pascal_second;
use uom::si::f64::{DynamicViscosity, MassDensity};
use uom::si::mass_density::kilogram_per_cubic_meter;
use uom::si::pressure::pascal;
use uom::si::thermodynamic_temperature::kelvin;

/// Andrade (Arrhenius-type) activation constant `B` for the liquid-water
/// viscosity correlation, in kelvin. See [`ThermalWaterProperties::viscosity`].
///
/// A value of ~1800 K reproduces the roughly 2:1 drop in liquid-water viscosity
/// between 20 °C and 60 °C (mu(293.15 K) ≈ 1.0e-3 Pa·s down to mu(333.15 K) ≈
/// 4.7e-4 Pa·s), which is close to the tabulated water values (~1.00e-3 vs
/// ~4.67e-4 Pa·s). It is a fitted lumped constant, not a first-principles
/// activation energy.
const VISCOSITY_ANDRADE_B_KELVIN: f64 = 1800.0;

/// Temperature-dependent liquid water properties for TH flow (SI throughout).
///
/// # Density model
///
/// Density combines the slightly-compressible pressure dependence of
/// [`LiquidWaterEos`](super::LiquidWaterEos) with a **linear volumetric thermal-expansion**
/// term, both taken as exact exponentials about a single reference state
/// `(p_ref, T_ref)`:
///
/// $$ \rho(p, T) = \rho_{\text{ref}} \, \exp\!\left[\, c\,(p - p_{\text{ref}}) - \beta\,(T - T_{\text{ref}}) \,\right] $$
///
/// where `c` is the isothermal compressibility (1/Pa) and `beta` the volumetric
/// thermal-expansion coefficient (1/K). The exponential (rather than linearised
/// `rho_ref * (1 + c dp - beta dT)`) form is used so density stays strictly
/// positive for all `(p, T)` and its derivatives are trivially proportional to
/// `rho`, keeping any Jacobian consistent with the residual. The temperature
/// derivative used by the energy-balance Jacobian is exact:
///
/// $$ \frac{\partial \rho}{\partial T} = -\beta \, \rho(p, T) $$
///
/// This is a Boussinesq-style linear-`beta` model with a **constant** `beta`; it
/// is accurate only over a modest temperature window about `T_ref` (real water's
/// `beta` itself varies strongly with `T` and even changes sign below 4 °C).
///
/// # Viscosity model
///
/// An **Andrade / Arrhenius** correlation with a single fitted constant `B`
/// ([`VISCOSITY_ANDRADE_B_KELVIN`]):
///
/// $$ \mu(T) = \mu_{\text{ref}} \, \exp\!\left[\, B\left(\tfrac{1}{T} - \tfrac{1}{T_{\text{ref}}}\right) \,\right] $$
///
/// This is monotonically **decreasing** in `T` (viscosity falls as water warms),
/// equals `mu_ref` at `T_ref`, and stays strictly positive. It is the classic
/// two-parameter Andrade form `mu = A exp(B/T)` rewritten about the reference so
/// that `mu(T_ref) = mu_ref` exactly; `B ≈ 1800 K` reproduces water's ~2:1 drop
/// from 20 °C to 60 °C. It is a fit, not IAPWS data.
///
/// # Transport coefficients
///
/// `specific_heat` (`c_w`, J/(kg·K)) and `thermal_conductivity` (`k_w`,
/// W/(m·K)) are treated as **constants** in this verification model.
///
/// # Units and ranges
///
/// - `reference_density` — kg/m^3 at `(reference_pressure, reference_temperature)`; strictly positive.
/// - `reference_pressure` — Pa; any finite value (commonly 101325 Pa).
/// - `reference_temperature` — K; strictly positive (absolute temperature).
/// - `compressibility` — 1/Pa; non-negative.
/// - `thermal_expansion` — 1/K; non-negative (volumetric `beta`).
/// - `reference_viscosity` — Pa·s at `reference_temperature`; strictly positive.
/// - `specific_heat` — J/(kg·K); strictly positive.
/// - `thermal_conductivity` — W/(m·K); strictly positive.
///
/// Construct via [`ThermalWaterProperties::new`] (validated) or
/// [`ThermalWaterProperties::water`] (liquid-water defaults). The fields are
/// public for read access and literal construction, but bypassing `new` skips
/// validation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ThermalWaterProperties {
    /// Reference density `rho_ref` at
    /// (`reference_pressure`, `reference_temperature`). SI unit: kg/m^3. Strictly positive.
    pub reference_density: f64,
    /// Reference pressure `p_ref`. SI unit: Pa. Any finite value.
    pub reference_pressure: f64,
    /// Reference (absolute) temperature `T_ref`. SI unit: K. Strictly positive.
    pub reference_temperature: f64,
    /// Isothermal compressibility `c`. SI unit: 1/Pa. Non-negative.
    pub compressibility: f64,
    /// Volumetric thermal-expansion coefficient `beta`. SI unit: 1/K. Non-negative.
    pub thermal_expansion: f64,
    /// Reference dynamic viscosity `mu_ref` at `reference_temperature`.
    /// SI unit: Pa·s. Strictly positive.
    pub reference_viscosity: f64,
    /// Liquid specific heat `c_w` (constant). SI unit: J/(kg·K). Strictly positive.
    pub specific_heat: f64,
    /// Liquid thermal conductivity `k_w` (constant). SI unit: W/(m·K). Strictly positive.
    pub thermal_conductivity: f64,
}

impl ThermalWaterProperties {
    /// Liquid-water defaults near atmospheric pressure and ~20 °C.
    ///
    /// - `reference_density = 1000` kg/m^3
    /// - `reference_pressure = 101325` Pa (1 atm)
    /// - `reference_temperature = 293.15` K (20 °C)
    /// - `compressibility = 4.5e-10` 1/Pa
    /// - `thermal_expansion = 2.1e-4` 1/K (volumetric `beta` of water near 20 °C)
    /// - `reference_viscosity = 1.0e-3` Pa·s (1 cP)
    /// - `specific_heat = 4182` J/(kg·K)
    /// - `thermal_conductivity = 0.6` W/(m·K)
    ///
    /// All values are within range by construction, so this is infallible.
    pub fn water() -> Self {
        Self {
            reference_density: 1000.0,
            reference_pressure: 101_325.0,
            reference_temperature: 293.15,
            compressibility: 4.5e-10,
            thermal_expansion: 2.1e-4,
            reference_viscosity: 1.0e-3,
            specific_heat: 4182.0,
            thermal_conductivity: 0.6,
        }
    }

    /// Construct validated temperature-dependent water properties from raw SI
    /// parameters.
    ///
    /// # Parameters (SI units)
    ///
    /// - `reference_density` — kg/m^3, finite and strictly positive.
    /// - `reference_pressure` — Pa, finite.
    /// - `reference_temperature` — K, finite and strictly positive (absolute).
    /// - `compressibility` — 1/Pa, finite and non-negative.
    /// - `thermal_expansion` — 1/K, finite and non-negative.
    /// - `reference_viscosity` — Pa·s, finite and strictly positive.
    /// - `specific_heat` — J/(kg·K), finite and strictly positive.
    /// - `thermal_conductivity` — W/(m·K), finite and strictly positive.
    ///
    /// # Errors
    ///
    /// Returns [`PflotranError::InvalidInput`] if any parameter is non-finite or
    /// outside the ranges above.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        reference_density: f64,
        reference_pressure: f64,
        reference_temperature: f64,
        compressibility: f64,
        thermal_expansion: f64,
        reference_viscosity: f64,
        specific_heat: f64,
        thermal_conductivity: f64,
    ) -> Result<Self, PflotranError> {
        if !reference_density.is_finite() || reference_density <= 0.0 {
            return Err(PflotranError::InvalidInput(format!(
                "reference_density must be finite and > 0 kg/m^3, got {reference_density}"
            )));
        }
        if !reference_pressure.is_finite() {
            return Err(PflotranError::InvalidInput(format!(
                "reference_pressure must be finite (Pa), got {reference_pressure}"
            )));
        }
        if !reference_temperature.is_finite() || reference_temperature <= 0.0 {
            return Err(PflotranError::InvalidInput(format!(
                "reference_temperature must be finite and > 0 K, got {reference_temperature}"
            )));
        }
        if !compressibility.is_finite() || compressibility < 0.0 {
            return Err(PflotranError::InvalidInput(format!(
                "compressibility must be finite and >= 0 1/Pa, got {compressibility}"
            )));
        }
        if !thermal_expansion.is_finite() || thermal_expansion < 0.0 {
            return Err(PflotranError::InvalidInput(format!(
                "thermal_expansion must be finite and >= 0 1/K, got {thermal_expansion}"
            )));
        }
        if !reference_viscosity.is_finite() || reference_viscosity <= 0.0 {
            return Err(PflotranError::InvalidInput(format!(
                "reference_viscosity must be finite and > 0 Pa·s, got {reference_viscosity}"
            )));
        }
        if !specific_heat.is_finite() || specific_heat <= 0.0 {
            return Err(PflotranError::InvalidInput(format!(
                "specific_heat must be finite and > 0 J/(kg·K), got {specific_heat}"
            )));
        }
        if !thermal_conductivity.is_finite() || thermal_conductivity <= 0.0 {
            return Err(PflotranError::InvalidInput(format!(
                "thermal_conductivity must be finite and > 0 W/(m·K), got {thermal_conductivity}"
            )));
        }
        Ok(Self {
            reference_density,
            reference_pressure,
            reference_temperature,
            compressibility,
            thermal_expansion,
            reference_viscosity,
            specific_heat,
            thermal_conductivity,
        })
    }

    /// Liquid density `rho(p, T)` at fluid pressure `p` and temperature `T`.
    ///
    /// Uses the exact exponential form
    /// `rho = reference_density * exp(compressibility * (p - reference_pressure)
    /// - thermal_expansion * (T - reference_temperature))`. Strictly positive;
    /// increasing in `p` and decreasing in `T` (for non-negative `c`, `beta`).
    /// Returns a [`FluidDensity`] (kg/m^3).
    pub fn density(&self, p: FluidPressure, t: FluidTemperature) -> FluidDensity {
        let rho = self.density_si(p.get::<pascal>(), t.get::<kelvin>());
        MassDensity::new::<kilogram_per_cubic_meter>(rho)
    }

    /// Temperature derivative `d(rho)/dT` at `(p, T)`.
    ///
    /// Exact analytic derivative of [`density`](Self::density):
    /// `d(rho)/dT = -thermal_expansion * rho(p, T)`, in kg/m^3 per K.
    /// Non-positive for `beta >= 0`. Returned as a plain `f64` (mixed-unit
    /// quantity with no convenient `uom` alias here).
    pub fn d_density_d_temperature(&self, p: FluidPressure, t: FluidTemperature) -> f64 {
        let rho = self.density_si(p.get::<pascal>(), t.get::<kelvin>());
        -self.thermal_expansion * rho
    }

    /// Temperature-dependent dynamic viscosity `mu(T)`.
    ///
    /// Andrade / Arrhenius correlation
    /// `mu = reference_viscosity * exp(B * (1/T - 1/reference_temperature))`
    /// with `B` = [`VISCOSITY_ANDRADE_B_KELVIN`] (~1800 K). Monotonically
    /// decreasing in `T`, equal to `reference_viscosity` at `reference_temperature`,
    /// and strictly positive. `T` must be a positive absolute temperature.
    /// Returns a [`FluidViscosity`] (Pa·s).
    pub fn viscosity(&self, t: FluidTemperature) -> FluidViscosity {
        let t_k = t.get::<kelvin>();
        let mu = self.reference_viscosity
            * (VISCOSITY_ANDRADE_B_KELVIN * (1.0 / t_k - 1.0 / self.reference_temperature)).exp();
        DynamicViscosity::new::<pascal_second>(mu)
    }

    /// Constant liquid specific heat `c_w`. SI unit: J/(kg·K). Returned as `f64`.
    pub fn specific_heat(&self) -> f64 {
        self.specific_heat
    }

    /// Constant liquid thermal conductivity `k_w`. SI unit: W/(m·K). Returned as `f64`.
    pub fn thermal_conductivity(&self) -> f64 {
        self.thermal_conductivity
    }

    /// Internal: density in SI (kg/m^3) from pressure (Pa) and temperature (K).
    #[inline]
    fn density_si(&self, p_pa: f64, t_k: f64) -> f64 {
        let exponent = self.compressibility * (p_pa - self.reference_pressure)
            - self.thermal_expansion * (t_k - self.reference_temperature);
        self.reference_density * exponent.exp()
    }
}

/// Solid-matrix (rock) thermal properties for the TH energy balance.
///
/// The bulk (fluid + solid) energy balance needs the solid's volumetric heat
/// capacity `rho_s * c_s` (storage) and its thermal conductivity `k_s`
/// (conduction). All three are treated as **constants** in this verification
/// model — no temperature or porosity dependence.
///
/// # Units and ranges
///
/// - `density` — kg/m^3, strictly positive.
/// - `specific_heat` — J/(kg·K), strictly positive.
/// - `thermal_conductivity` — W/(m·K), strictly positive.
///
/// Construct via [`RockThermalProperties::new`] (validated) or
/// [`RockThermalProperties::generic_rock`] (generic-rock defaults).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RockThermalProperties {
    /// Solid grain/matrix density `rho_s`. SI unit: kg/m^3. Strictly positive.
    pub density: f64,
    /// Solid specific heat `c_s`. SI unit: J/(kg·K). Strictly positive.
    pub specific_heat: f64,
    /// Solid thermal conductivity `k_s`. SI unit: W/(m·K). Strictly positive.
    pub thermal_conductivity: f64,
}

impl RockThermalProperties {
    /// Generic crustal-rock defaults (order-of-magnitude, not site-specific):
    ///
    /// - `density = 2650` kg/m^3 (quartz/granite grain density)
    /// - `specific_heat = 830` J/(kg·K)
    /// - `thermal_conductivity = 2.5` W/(m·K)
    ///
    /// All values are within range by construction, so this is infallible.
    pub fn generic_rock() -> Self {
        Self {
            density: 2650.0,
            specific_heat: 830.0,
            thermal_conductivity: 2.5,
        }
    }

    /// Construct validated rock thermal properties from raw SI parameters.
    ///
    /// # Parameters (SI units)
    ///
    /// - `density` — kg/m^3, finite and strictly positive.
    /// - `specific_heat` — J/(kg·K), finite and strictly positive.
    /// - `thermal_conductivity` — W/(m·K), finite and strictly positive.
    ///
    /// # Errors
    ///
    /// Returns [`PflotranError::InvalidInput`] if any parameter is non-finite or
    /// outside the ranges above.
    pub fn new(
        density: f64,
        specific_heat: f64,
        thermal_conductivity: f64,
    ) -> Result<Self, PflotranError> {
        if !density.is_finite() || density <= 0.0 {
            return Err(PflotranError::InvalidInput(format!(
                "density must be finite and > 0 kg/m^3, got {density}"
            )));
        }
        if !specific_heat.is_finite() || specific_heat <= 0.0 {
            return Err(PflotranError::InvalidInput(format!(
                "specific_heat must be finite and > 0 J/(kg·K), got {specific_heat}"
            )));
        }
        if !thermal_conductivity.is_finite() || thermal_conductivity <= 0.0 {
            return Err(PflotranError::InvalidInput(format!(
                "thermal_conductivity must be finite and > 0 W/(m·K), got {thermal_conductivity}"
            )));
        }
        Ok(Self {
            density,
            specific_heat,
            thermal_conductivity,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uom::si::f64::{Pressure, ThermodynamicTemperature};

    fn pressure(pa: f64) -> FluidPressure {
        Pressure::new::<pascal>(pa)
    }

    fn temperature(k: f64) -> FluidTemperature {
        ThermodynamicTemperature::new::<kelvin>(k)
    }

    #[test]
    fn water_defaults_are_valid_and_match_new() {
        let w = ThermalWaterProperties::water();
        let n = ThermalWaterProperties::new(
            1000.0, 101_325.0, 293.15, 4.5e-10, 2.1e-4, 1.0e-3, 4182.0, 0.6,
        )
        .unwrap();
        assert_eq!(w, n);
    }

    #[test]
    fn density_equals_reference_at_reference_state() {
        let w = ThermalWaterProperties::water();
        let rho = w
            .density(pressure(w.reference_pressure), temperature(w.reference_temperature))
            .get::<kilogram_per_cubic_meter>();
        assert!((rho - w.reference_density).abs() < 1e-9);
    }

    #[test]
    fn density_decreases_with_temperature() {
        let w = ThermalWaterProperties::water();
        let p = pressure(w.reference_pressure);
        let cold = w.density(p, temperature(283.15)).get::<kilogram_per_cubic_meter>();
        let mid = w.density(p, temperature(303.15)).get::<kilogram_per_cubic_meter>();
        let hot = w.density(p, temperature(333.15)).get::<kilogram_per_cubic_meter>();
        assert!(cold > mid && mid > hot, "density must fall as T rises (thermal expansion)");
    }

    #[test]
    fn density_increases_with_pressure() {
        let w = ThermalWaterProperties::water();
        let t = temperature(w.reference_temperature);
        let lo = w.density(pressure(1.0e5), t).get::<kilogram_per_cubic_meter>();
        let hi = w.density(pressure(1.0e7), t).get::<kilogram_per_cubic_meter>();
        assert!(lo < hi, "density must rise with p (compressibility)");
    }

    #[test]
    fn d_density_d_temperature_matches_finite_difference() {
        let w = ThermalWaterProperties::water();
        for &p0 in &[1.0e5, 5.0e6] {
            for &t0 in &[280.0, 293.15, 320.0, 350.0] {
                let h = 1e-3; // K
                let fd = (w
                    .density(pressure(p0), temperature(t0 + h))
                    .get::<kilogram_per_cubic_meter>()
                    - w.density(pressure(p0), temperature(t0 - h))
                        .get::<kilogram_per_cubic_meter>())
                    / (2.0 * h);
                let analytic = w.d_density_d_temperature(pressure(p0), temperature(t0));
                let rel = (fd - analytic).abs() / analytic.abs();
                assert!(
                    rel < 1e-6,
                    "FD mismatch at p={p0}, T={t0}: fd={fd}, analytic={analytic}, rel={rel}"
                );
            }
        }
    }

    #[test]
    fn d_density_d_temperature_is_non_positive() {
        let w = ThermalWaterProperties::water();
        let d = w.d_density_d_temperature(pressure(1.0e5), temperature(300.0));
        assert!(d < 0.0, "drho/dT must be negative for positive beta");
    }

    #[test]
    fn viscosity_equals_reference_at_reference_temperature() {
        let w = ThermalWaterProperties::water();
        let mu = w.viscosity(temperature(w.reference_temperature)).get::<pascal_second>();
        assert!((mu - w.reference_viscosity).abs() < 1e-15);
    }

    #[test]
    fn viscosity_decreases_with_temperature() {
        let w = ThermalWaterProperties::water();
        let cold = w.viscosity(temperature(283.15)).get::<pascal_second>();
        let mid = w.viscosity(temperature(303.15)).get::<pascal_second>();
        let hot = w.viscosity(temperature(333.15)).get::<pascal_second>();
        assert!(cold > mid && mid > hot, "viscosity must fall as T rises");
        // Sanity: ~2:1 drop from 20 C to 60 C with B~1800 K.
        let at20 = w.viscosity(temperature(293.15)).get::<pascal_second>();
        let at60 = w.viscosity(temperature(333.15)).get::<pascal_second>();
        assert!((at20 / at60) > 1.8 && (at20 / at60) < 2.4, "ratio={}", at20 / at60);
    }

    #[test]
    fn transport_coefficients_are_returned() {
        let w = ThermalWaterProperties::water();
        assert!((w.specific_heat() - 4182.0).abs() < 1e-12);
        assert!((w.thermal_conductivity() - 0.6).abs() < 1e-12);
    }

    #[test]
    fn water_new_rejects_bad_inputs() {
        // reference_density <= 0
        assert!(ThermalWaterProperties::new(0.0, 1e5, 293.15, 4.5e-10, 2.1e-4, 1e-3, 4182.0, 0.6).is_err());
        // reference_temperature <= 0
        assert!(ThermalWaterProperties::new(1000.0, 1e5, 0.0, 4.5e-10, 2.1e-4, 1e-3, 4182.0, 0.6).is_err());
        // negative compressibility
        assert!(ThermalWaterProperties::new(1000.0, 1e5, 293.15, -1.0, 2.1e-4, 1e-3, 4182.0, 0.6).is_err());
        // negative thermal_expansion
        assert!(ThermalWaterProperties::new(1000.0, 1e5, 293.15, 4.5e-10, -1.0, 1e-3, 4182.0, 0.6).is_err());
        // viscosity <= 0
        assert!(ThermalWaterProperties::new(1000.0, 1e5, 293.15, 4.5e-10, 2.1e-4, 0.0, 4182.0, 0.6).is_err());
        // specific_heat <= 0
        assert!(ThermalWaterProperties::new(1000.0, 1e5, 293.15, 4.5e-10, 2.1e-4, 1e-3, 0.0, 0.6).is_err());
        // thermal_conductivity <= 0
        assert!(ThermalWaterProperties::new(1000.0, 1e5, 293.15, 4.5e-10, 2.1e-4, 1e-3, 4182.0, 0.0).is_err());
        // non-finite pressure
        assert!(ThermalWaterProperties::new(1000.0, f64::NAN, 293.15, 4.5e-10, 2.1e-4, 1e-3, 4182.0, 0.6).is_err());
        // all valid
        assert!(ThermalWaterProperties::new(1000.0, 1e5, 293.15, 4.5e-10, 2.1e-4, 1e-3, 4182.0, 0.6).is_ok());
    }

    #[test]
    fn generic_rock_defaults_are_valid_and_match_new() {
        let r = RockThermalProperties::generic_rock();
        let n = RockThermalProperties::new(2650.0, 830.0, 2.5).unwrap();
        assert_eq!(r, n);
    }

    #[test]
    fn rock_new_rejects_bad_inputs() {
        assert!(RockThermalProperties::new(0.0, 830.0, 2.5).is_err());
        assert!(RockThermalProperties::new(2650.0, -1.0, 2.5).is_err());
        assert!(RockThermalProperties::new(2650.0, 830.0, 0.0).is_err());
        assert!(RockThermalProperties::new(2650.0, 830.0, f64::INFINITY).is_err());
        assert!(RockThermalProperties::new(2650.0, 830.0, 2.5).is_ok());
    }
}
