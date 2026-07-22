//! Liquid-water equation of state (v1) — slightly-compressible, constant
//! viscosity.
//!
//! RICHARDS flow needs the liquid density `rho_l(p_l)` and its pressure
//! derivative (for the accumulation term's Jacobian), plus a viscosity `mu_l`.
//! For the v1 vertical slice the liquid is modelled as **slightly
//! compressible** with a **constant** dynamic viscosity — the standard
//! isothermal-groundwater simplification. Temperature dependence and a real
//! IAPWS-IF97 EOS are later work (TH mode, bead op-v6s.10); this module is
//! deliberately the simplest physically-honest choice.
//!
//! All quantities are `uom`-typed at the public boundary and plain `f64`
//! (SI base units) internally.

use crate::error::PflotranError;
use crate::units::{FluidDensity, FluidPressure, FluidViscosity};
use uom::si::dynamic_viscosity::pascal_second;
use uom::si::f64::{DynamicViscosity, MassDensity};
use uom::si::mass_density::kilogram_per_cubic_meter;
use uom::si::pressure::pascal;

/// Liquid-water equation of state: slightly-compressible, constant viscosity.
///
/// # Density model
///
/// The **exact exponential** slightly-compressible form is used (not the
/// linearised expansion):
///
/// $$ \rho(p) = \rho_{\text{ref}} \, \exp\!\left[\, c \, (p - p_{\text{ref}}) \,\right] $$
///
/// with pressure derivative (used by the accumulation-term Jacobian)
///
/// $$ \frac{d\rho}{dp} = c \, \rho(p) = c \, \rho_{\text{ref}} \, \exp\!\left[\, c \, (p - p_{\text{ref}}) \,\right] $$
///
/// where `c` is the isothermal fluid compressibility. The exponential form is
/// chosen over the linearised `rho_ref * (1 + c (p - p_ref))` because it stays
/// strictly positive for all pressures and its derivative is trivially
/// `c * rho`, which keeps the Jacobian consistent with the residual. Over the
/// pressure ranges of groundwater flow the two agree to well under a percent.
///
/// # Viscosity model
///
/// The dynamic viscosity `mu` is a **constant** in v1 (no pressure or
/// temperature dependence).
///
/// # Units and ranges
///
/// - `reference_density` — kg/m^3, the density at `reference_pressure`. Must be
///   strictly positive.
/// - `reference_pressure` — Pa, the pressure at which `rho = reference_density`.
///   Any finite value (commonly atmospheric, 101325 Pa).
/// - `compressibility` — 1/Pa, the isothermal compressibility `c >= 0`.
/// - `viscosity` — Pa·s, the constant dynamic viscosity. Must be strictly
///   positive.
///
/// Construct via [`LiquidWaterEos::new`] (validated) or [`LiquidWaterEos::water`]
/// (liquid-water defaults). The fields are public for ergonomic read access and
/// literal construction, but bypassing `new` skips validation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LiquidWaterEos {
    /// Reference density `rho_ref` at [`reference_pressure`](Self::reference_pressure).
    /// SI unit: kg/m^3. Strictly positive.
    pub reference_density: f64,
    /// Reference pressure `p_ref` at which `rho = reference_density`.
    /// SI unit: Pa. Any finite value.
    pub reference_pressure: f64,
    /// Isothermal fluid compressibility `c`. SI unit: 1/Pa. Non-negative.
    pub compressibility: f64,
    /// Constant dynamic viscosity `mu`. SI unit: Pa·s. Strictly positive.
    pub viscosity: f64,
}

impl LiquidWaterEos {
    /// Sensible liquid-water defaults near atmospheric conditions and ~20 °C.
    ///
    /// - `reference_density = 1000` kg/m^3
    /// - `reference_pressure = 101325` Pa (1 atm)
    /// - `compressibility = 4.5e-10` 1/Pa (isothermal compressibility of water)
    /// - `viscosity = 1.0e-3` Pa·s (1 cP)
    ///
    /// These are within-range by construction, so this is infallible.
    pub fn water() -> Self {
        Self {
            reference_density: 1000.0,
            reference_pressure: 101_325.0,
            compressibility: 4.5e-10,
            viscosity: 1.0e-3,
        }
    }

    /// Construct a validated liquid EOS from raw SI-unit parameters.
    ///
    /// # Parameters (SI units)
    ///
    /// - `reference_density` — kg/m^3, must be finite and strictly positive.
    /// - `reference_pressure` — Pa, must be finite.
    /// - `compressibility` — 1/Pa, must be finite and non-negative.
    /// - `viscosity` — Pa·s, must be finite and strictly positive.
    ///
    /// # Errors
    ///
    /// Returns [`PflotranError::InvalidInput`] if any parameter is non-finite or
    /// outside the ranges above.
    pub fn new(
        reference_density: f64,
        reference_pressure: f64,
        compressibility: f64,
        viscosity: f64,
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
        if !compressibility.is_finite() || compressibility < 0.0 {
            return Err(PflotranError::InvalidInput(format!(
                "compressibility must be finite and >= 0 1/Pa, got {compressibility}"
            )));
        }
        if !viscosity.is_finite() || viscosity <= 0.0 {
            return Err(PflotranError::InvalidInput(format!(
                "viscosity must be finite and > 0 Pa·s, got {viscosity}"
            )));
        }
        Ok(Self {
            reference_density,
            reference_pressure,
            compressibility,
            viscosity,
        })
    }

    /// Liquid density `rho(p)` at fluid pressure `p`.
    ///
    /// Uses the exact exponential form
    /// `rho = reference_density * exp(compressibility * (p - reference_pressure))`.
    /// Strictly positive and monotonically increasing in `p` for
    /// `compressibility >= 0`. Returns a [`FluidDensity`] (kg/m^3).
    pub fn density(&self, p: FluidPressure) -> FluidDensity {
        let rho = self.density_si(p.get::<pascal>());
        MassDensity::new::<kilogram_per_cubic_meter>(rho)
    }

    /// Pressure derivative `d(rho)/dp` at fluid pressure `p`.
    ///
    /// Equals `compressibility * rho(p)`, in kg/m^3 per Pa. Non-negative for
    /// `compressibility >= 0`. Returned as a plain `f64` (the derivative is a
    /// mixed-unit quantity that has no convenient `uom` alias here).
    pub fn d_density_d_pressure(&self, p: FluidPressure) -> f64 {
        let p_pa = p.get::<pascal>();
        self.compressibility * self.density_si(p_pa)
    }

    /// Constant dynamic viscosity `mu`. Returns a [`FluidViscosity`] (Pa·s).
    pub fn viscosity(&self) -> FluidViscosity {
        DynamicViscosity::new::<pascal_second>(self.viscosity)
    }

    /// Internal: density in SI (kg/m^3) from pressure in SI (Pa).
    #[inline]
    fn density_si(&self, p_pa: f64) -> f64 {
        self.reference_density * (self.compressibility * (p_pa - self.reference_pressure)).exp()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uom::si::f64::Pressure;

    fn pressure(pa: f64) -> FluidPressure {
        Pressure::new::<pascal>(pa)
    }

    #[test]
    fn water_defaults_are_valid_and_match_new() {
        let w = LiquidWaterEos::water();
        let n = LiquidWaterEos::new(1000.0, 101_325.0, 4.5e-10, 1.0e-3).unwrap();
        assert_eq!(w, n);
    }

    #[test]
    fn density_equals_reference_at_reference_pressure() {
        let eos = LiquidWaterEos::water();
        let rho = eos
            .density(pressure(eos.reference_pressure))
            .get::<kilogram_per_cubic_meter>();
        assert!((rho - eos.reference_density).abs() < 1e-9);
    }

    #[test]
    fn density_increases_with_pressure() {
        let eos = LiquidWaterEos::water();
        let lo = eos.density(pressure(1.0e5)).get::<kilogram_per_cubic_meter>();
        let mid = eos.density(pressure(5.0e6)).get::<kilogram_per_cubic_meter>();
        let hi = eos.density(pressure(1.0e7)).get::<kilogram_per_cubic_meter>();
        assert!(lo < mid && mid < hi, "density must be monotone increasing in p");
    }

    #[test]
    fn d_density_d_pressure_matches_finite_difference() {
        let eos = LiquidWaterEos::water();
        for &p0 in &[1.0e5, 1.0e6, 5.0e6, 2.0e7] {
            let h = p0 * 1e-6;
            let fd = (eos.density(pressure(p0 + h)).get::<kilogram_per_cubic_meter>()
                - eos.density(pressure(p0 - h)).get::<kilogram_per_cubic_meter>())
                / (2.0 * h);
            let analytic = eos.d_density_d_pressure(pressure(p0));
            let rel = (fd - analytic).abs() / analytic.abs();
            assert!(rel < 1e-6, "FD mismatch at p={p0}: fd={fd}, analytic={analytic}, rel={rel}");
        }
    }

    #[test]
    fn viscosity_is_constant_and_positive() {
        let eos = LiquidWaterEos::water();
        assert!((eos.viscosity().get::<pascal_second>() - 1.0e-3).abs() < 1e-15);
    }

    #[test]
    fn new_rejects_bad_inputs() {
        assert!(LiquidWaterEos::new(0.0, 1e5, 4.5e-10, 1e-3).is_err());
        assert!(LiquidWaterEos::new(1000.0, 1e5, -1.0, 1e-3).is_err());
        assert!(LiquidWaterEos::new(1000.0, 1e5, 4.5e-10, 0.0).is_err());
        assert!(LiquidWaterEos::new(1000.0, f64::NAN, 4.5e-10, 1e-3).is_err());
        assert!(LiquidWaterEos::new(1000.0, 1e5, 4.5e-10, 1e-3).is_ok());
    }
}
