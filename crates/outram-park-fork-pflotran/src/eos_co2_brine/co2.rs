//! CO2 real-fluid properties via the **Redlich-Kwong (RK) cubic EOS**
//! (density, compressibility) plus the **Fenghour-Wakeham-Vesovic (1998)
//! dilute-gas viscosity** term.
//!
//! # Physics
//!
//! ## Density — Redlich-Kwong cubic EOS
//!
//! The RK EOS (Redlich & Kwong, 1949) in pressure-explicit form is
//!
//! ```text
//! P = R T / (v - b)  -  a / ( sqrt(T) * v * (v + b) )
//! ```
//!
//! with the CO2-specific constants derived from the critical point
//! (Tc = 304.13 K, Pc = 7.3773 MPa):
//!
//! ```text
//! a = 0.42748 * R^2 * Tc^2.5 / Pc
//! b = 0.08664 * R  * Tc      / Pc
//! ```
//!
//! Non-dimensionalising with `A = a P / (R^2 T^2.5)` and `B = b P / (R T)`, the
//! molar volume is replaced by the compressibility factor `Z = P v / (R T)`,
//! which satisfies the cubic
//!
//! ```text
//! Z^3 - Z^2 + (A - B - B^2) Z - A B = 0.
//! ```
//!
//! The **largest real root** is taken as the gas/supercritical compressibility
//! (the smallest positive root, when three exist, is the liquid branch). Density
//! then follows from the ideal-gas-corrected relation
//!
//! ```text
//! rho = P M / (Z R T),   M = 0.04401 kg/mol (CO2 molar mass).
//! ```
//!
//! **RK is a documented approximation to the Span & Wagner (1996) reference
//! EOS**, not a substitute for it. It captures the right *shape* (ideal-gas
//! limit `Z -> 1` as `P -> 0`, `rho` rising with `P`, falling with `T`) but is
//! quantitatively poor in the dense near-critical region: it under-predicts CO2
//! density there by ~10-15% (see the tests, which assert against NIST/Span-Wagner
//! values *with the RK deviation stated honestly*). The acentric factor
//! `omega = 0.22394` for CO2 is noted for readers who wish to upgrade to
//! Redlich-Kwong-Soave (RKS) or Peng-Robinson; **plain RK does not use it**, so
//! it is not a constant here.
//!
//! ## Viscosity — Fenghour-Wakeham-Vesovic (1998), dilute-gas term only
//!
//! The zero-density (dilute-gas) viscosity is
//!
//! ```text
//! eta_0(T) = 1.00697 * sqrt(T) / G*(T*)      [microPa.s]
//! ln G*(T*) = sum_{i=0..4} a_i (ln T*)^i,   T* = T / (epsilon/k),  epsilon/k = 251.196 K
//! ```
//!
//! with `a = [0.235156, -0.491266, 0.05211155, 0.05347906, -0.01537102]`. This
//! is **only the density-independent term**: the full FWV correlation adds an
//! excess viscosity `Delta eta(rho, T)` (and a small critical enhancement) that
//! is **omitted here**. Consequently [`Co2Properties::viscosity`] is a function
//! of temperature alone and materially under-predicts dense supercritical CO2
//! viscosity — use it only near the dilute limit and treat all dense-phase
//! output as approximate.
//!
//! # Follow-up (not implemented)
//!
//! - Replace RK density with a Span-Wagner Helmholtz-energy EOS port.
//! - Add the FWV excess-viscosity term for dense-phase accuracy.
//!
//! See the parent module [`crate::eos_co2_brine`] for the full accuracy warning
//! and provenance block. Untrusted AI-generated draft (`RESPONSIBLE_USE.md`).

use crate::error::PflotranError;

/// Universal gas constant, J/(mol.K) (CODATA).
const R_GAS: f64 = 8.314_462_618;

/// CO2 molar mass, kg/mol.
const M_CO2: f64 = 0.044_01;

/// CO2 critical temperature, K (Span & Wagner, 1996).
const TC_CO2: f64 = 304.13;

/// CO2 critical pressure, Pa (7.3773 MPa; Span & Wagner, 1996).
const PC_CO2: f64 = 7.377_3e6;

/// Upper pressure guard for the RK evaluation, Pa (500 MPa). Beyond this the RK
/// approximation is not meaningfully useful and inputs are rejected rather than
/// silently extrapolated.
const P_MAX_PA: f64 = 500.0e6;

/// CO2 real-fluid properties via the Redlich-Kwong cubic EOS (density,
/// compressibility) and the Fenghour-Wakeham-Vesovic dilute-gas viscosity.
///
/// **Stateless** — all methods are associated functions taking `(T [K], P [Pa])`
/// directly; there is nothing to construct. See the module documentation for the
/// physics, valid ranges, and the (important) accuracy limitations: RK
/// under-predicts dense CO2 density by ~10-15%, and the viscosity is a
/// temperature-only dilute-gas term.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Co2Properties;

impl Co2Properties {
    /// Compressibility factor `Z = P v / (R T)` (dimensionless) at temperature
    /// `temperature_k` (K) and pressure `pressure_pa` (Pa).
    ///
    /// Returns the **largest real root** of the RK cubic
    /// `Z^3 - Z^2 + (A - B - B^2) Z - A B = 0`, i.e. the gas/supercritical
    /// branch. Approaches `1` in the ideal-gas limit (`P -> 0`).
    ///
    /// # Errors
    ///
    /// [`PflotranError::InvalidInput`] if `T <= 0 K`, `P <= 0 Pa`, `P > 500 MPa`,
    /// or either input is non-finite.
    pub fn compressibility(
        temperature_k: f64,
        pressure_pa: f64,
    ) -> Result<f64, PflotranError> {
        Self::validate(temperature_k, pressure_pa)?;

        // Non-dimensional RK parameters.
        //   A = a P / (R^2 T^2.5) = 0.42748 (P/Pc) / (T/Tc)^2.5
        //   B = b P / (R T)       = 0.08664 (P/Pc) / (T/Tc)
        let pr = pressure_pa / PC_CO2;
        let tr = temperature_k / TC_CO2;
        let a_param = 0.42748 * pr / tr.powf(2.5);
        let b_param = 0.08664 * pr / tr;

        // Cubic in Z:  Z^3 + c2 Z^2 + c1 Z + c0 = 0
        let c2 = -1.0;
        let c1 = a_param - b_param - b_param * b_param;
        let c0 = -a_param * b_param;

        let z = largest_real_cubic_root(c2, c1, c0);

        // Physical guard: Z must exceed B (i.e. molar volume v > b, the RK
        // co-volume) and be positive. A root failing this signals a degenerate
        // solve rather than a usable state.
        if !(z.is_finite()) || z <= b_param || z <= 0.0 {
            return Err(PflotranError::InvalidInput(format!(
                "Co2Properties: RK cubic produced no physical gas root at T={temperature_k} K, \
                 P={pressure_pa} Pa (Z={z}, B={b_param})"
            )));
        }
        Ok(z)
    }

    /// CO2 mass density (kg/m^3) at temperature `temperature_k` (K) and pressure
    /// `pressure_pa` (Pa).
    ///
    /// Computed as `rho = P M / (Z R T)` with `M = 0.04401 kg/mol` and `Z` from
    /// [`Self::compressibility`]. Rises with pressure, falls with temperature.
    ///
    /// **Accuracy:** RK under-predicts dense supercritical CO2 density by
    /// ~10-15% (e.g. ~542 vs NIST ~632 kg/m^3 at 40 °C, 10 MPa). Not
    /// quantitatively reliable near the critical point — see the module warning.
    ///
    /// # Errors
    ///
    /// [`PflotranError::InvalidInput`] on the same conditions as
    /// [`Self::compressibility`].
    pub fn density(temperature_k: f64, pressure_pa: f64) -> Result<f64, PflotranError> {
        let z = Self::compressibility(temperature_k, pressure_pa)?;
        Ok(pressure_pa * M_CO2 / (z * R_GAS * temperature_k))
    }

    /// CO2 dynamic viscosity (Pa.s) at temperature `temperature_k` (K).
    ///
    /// The Fenghour-Wakeham-Vesovic (1998) **zero-density (dilute-gas) term
    /// only**; the `pressure_pa` argument is accepted for a uniform signature
    /// (it is validated) but **does not affect the result** because the excess
    /// density-dependent term is omitted. For CO2 near 40 °C this is
    /// ~1.56e-5 Pa.s. It increases with temperature.
    ///
    /// **Accuracy:** materially under-predicts the viscosity of *dense*
    /// supercritical CO2 (which is dominated by the omitted excess term). Use
    /// only near the dilute-gas limit; treat dense-phase use as approximate.
    ///
    /// # Errors
    ///
    /// [`PflotranError::InvalidInput`] on the same conditions as
    /// [`Self::compressibility`].
    pub fn viscosity(temperature_k: f64, pressure_pa: f64) -> Result<f64, PflotranError> {
        Self::validate(temperature_k, pressure_pa)?;

        // FWV zero-density term. epsilon/k = 251.196 K.
        let t_star = temperature_k / 251.196;
        let ln_ts = t_star.ln();
        // ln G* = sum a_i (ln T*)^i, i = 0..4
        let a = [
            0.235_156,
            -0.491_266,
            0.052_111_55,
            0.053_479_06,
            -0.015_371_02,
        ];
        let mut ln_g = 0.0;
        let mut p = 1.0;
        for coeff in a {
            ln_g += coeff * p;
            p *= ln_ts;
        }
        // eta_0 in microPa.s -> Pa.s (x 1e-6).
        let eta0_micro = 1.006_97 * temperature_k.sqrt() / ln_g.exp();
        Ok(eta0_micro * 1.0e-6)
    }

    /// Validate `(T [K], P [Pa])` for the RK/FWV evaluation.
    ///
    /// Accepts `T > 0 K` and `0 < P <= 500 MPa`, both finite. Returns
    /// [`PflotranError::InvalidInput`] otherwise. (RK is a global cubic; this
    /// guard only rejects nonphysical/degenerate inputs, not "inaccurate" ones —
    /// accuracy is documented per method.)
    fn validate(temperature_k: f64, pressure_pa: f64) -> Result<(), PflotranError> {
        if !temperature_k.is_finite() || !pressure_pa.is_finite() {
            return Err(PflotranError::InvalidInput(format!(
                "Co2Properties: temperature and pressure must be finite, got T={temperature_k} K, \
                 P={pressure_pa} Pa"
            )));
        }
        if temperature_k <= 0.0 {
            return Err(PflotranError::InvalidInput(format!(
                "Co2Properties: temperature must be positive (K), got {temperature_k} K"
            )));
        }
        if pressure_pa <= 0.0 || pressure_pa > P_MAX_PA {
            return Err(PflotranError::InvalidInput(format!(
                "Co2Properties: pressure {pressure_pa} Pa is outside the valid range \
                 (0, {P_MAX_PA}] Pa"
            )));
        }
        Ok(())
    }
}

/// Largest real root of the monic cubic `x^3 + c2 x^2 + c1 x + c0 = 0`.
///
/// Uses Cardano's method on the depressed cubic `t^3 + p t + q = 0`
/// (`x = t - c2/3`): when the discriminant `Delta = (q/2)^2 + (p/3)^3 > 0` there
/// is a single real root (returned directly); when `Delta <= 0` there are three
/// real roots, computed by the trigonometric formula and reduced to their
/// maximum. This is exactly what the RK gas/supercritical branch needs (largest
/// root = gas compressibility). Internal helper — not part of the public API.
fn largest_real_cubic_root(c2: f64, c1: f64, c0: f64) -> f64 {
    let shift = c2 / 3.0;
    let p = c1 - c2 * c2 / 3.0;
    let q = 2.0 * c2 * c2 * c2 / 27.0 - c2 * c1 / 3.0 + c0;

    let disc = (q / 2.0).powi(2) + (p / 3.0).powi(3);

    if disc > 0.0 {
        // One real root.
        let sqrt_disc = disc.sqrt();
        let u = (-q / 2.0 + sqrt_disc).cbrt();
        let v = (-q / 2.0 - sqrt_disc).cbrt();
        (u + v) - shift
    } else {
        // Three real roots (Delta <= 0). Trigonometric solution.
        let m = 2.0 * (-p / 3.0).sqrt();
        // Guard against a degenerate p ~ 0 (m ~ 0 => all roots equal to -shift).
        if m.abs() < 1.0e-300 {
            return -shift;
        }
        let mut arg = 3.0 * q / (p * m);
        arg = arg.clamp(-1.0, 1.0);
        let theta = arg.acos();
        let two_pi = std::f64::consts::TAU;
        let r0 = m * (theta / 3.0).cos() - shift;
        let r1 = m * ((theta - two_pi) / 3.0).cos() - shift;
        let r2 = m * ((theta - 2.0 * two_pi) / 3.0).cos() - shift;
        r0.max(r1).max(r2)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Supercritical CO2 at 40 °C, 10 MPa.
    ///
    /// Reference (NIST WebBook / Span-Wagner 1996): rho ≈ 632 kg/m^3.
    /// **RK is a low-order approximation and under-predicts here**: it yields
    /// ~542 kg/m^3, a deviation of about **-14.3%**. This test asserts the RK
    /// value is within 15% of the reference *and documents the honest deviation*
    /// — it is NOT fudged to hit 632. See the module accuracy warning.
    #[test]
    fn co2_density_supercritical_40c_10mpa_rk_underpredicts() {
        let rho = Co2Properties::density(313.15, 10.0e6).unwrap();
        // Measured RK result ~ 541.7 kg/m^3.
        assert!(
            (rho - 541.7).abs() < 2.0,
            "expected RK ~541.7 kg/m^3, got {rho}"
        );
        // Honest deviation from the Span-Wagner reference (~632 kg/m^3).
        let rel = (rho - 632.0).abs() / 632.0;
        assert!(
            rel < 0.15,
            "RK CO2 density {rho} deviates {:.1}% from NIST ~632 kg/m^3 (expected ~14% under)",
            rel * 100.0
        );
        assert!(
            rho < 632.0,
            "RK is known to UNDER-predict dense CO2 density; got {rho} >= 632"
        );
    }

    /// Ideal-gas limit: `Z -> 1` as `P -> 0`.
    #[test]
    fn co2_compressibility_approaches_unity_at_low_pressure() {
        let z = Co2Properties::compressibility(313.15, 100.0).unwrap();
        assert!(
            (z - 1.0).abs() < 1.0e-3,
            "Z should approach 1 as P -> 0, got {z}"
        );
    }

    /// Density rises with pressure at fixed temperature (313.15 K).
    #[test]
    fn co2_density_increases_with_pressure() {
        let rho_lo = Co2Properties::density(313.15, 5.0e6).unwrap();
        let rho_hi = Co2Properties::density(313.15, 15.0e6).unwrap();
        assert!(
            rho_hi > rho_lo,
            "density must rise with P: 5 MPa -> {rho_lo}, 15 MPa -> {rho_hi}"
        );
    }

    /// Density falls with temperature at fixed pressure (10 MPa).
    #[test]
    fn co2_density_decreases_with_temperature() {
        let rho_cold = Co2Properties::density(313.15, 10.0e6).unwrap();
        let rho_hot = Co2Properties::density(373.15, 10.0e6).unwrap();
        assert!(
            rho_hot < rho_cold,
            "density must fall with T: 313 K -> {rho_cold}, 373 K -> {rho_hot}"
        );
    }

    /// Dilute-gas viscosity near 40 °C is ~1.56e-5 Pa.s and rises with T.
    ///
    /// FWV zero-density term: eta_0(313.15 K) ≈ 15.65 microPa.s ≈ 1.565e-5 Pa.s,
    /// consistent with the CO2 dilute-gas reference. The excess (dense-phase)
    /// term is omitted — see the module warning.
    #[test]
    fn co2_viscosity_dilute_gas_value_and_temperature_trend() {
        let mu = Co2Properties::viscosity(313.15, 1.0e5).unwrap();
        assert!(
            (mu - 1.565e-5).abs() < 5.0e-7,
            "expected ~1.565e-5 Pa.s dilute-gas viscosity at 40 C, got {mu}"
        );
        // Pressure-independent (dilute term only): same T, higher P -> same mu.
        let mu_dense = Co2Properties::viscosity(313.15, 10.0e6).unwrap();
        assert!(
            (mu - mu_dense).abs() < 1.0e-12,
            "dilute-only viscosity must be P-independent, got {mu} vs {mu_dense}"
        );
        // eta_0 increases with temperature.
        let mu_hot = Co2Properties::viscosity(350.0, 1.0e5).unwrap();
        assert!(
            mu_hot > mu,
            "dilute-gas viscosity must rise with T: 313 K -> {mu}, 350 K -> {mu_hot}"
        );
    }

    /// Input validation: non-finite, negative, and over-range inputs rejected.
    #[test]
    fn co2_rejects_invalid_inputs() {
        assert!(Co2Properties::density(-10.0, 10.0e6).is_err()); // negative T
        assert!(Co2Properties::density(313.15, -1.0e6).is_err()); // negative P
        assert!(Co2Properties::density(313.15, 0.0).is_err()); // zero P
        assert!(Co2Properties::density(313.15, 600.0e6).is_err()); // over P_MAX
        assert!(Co2Properties::compressibility(f64::NAN, 10.0e6).is_err());
        assert!(Co2Properties::viscosity(313.15, f64::INFINITY).is_err());
    }
}
