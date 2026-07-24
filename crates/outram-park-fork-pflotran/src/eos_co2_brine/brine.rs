//! NaCl-brine (aqueous-phase) density and viscosity via the **Batzle & Wang
//! (1992)** empirical correlations.
//!
//! # Physics
//!
//! ## Density — Batzle-Wang (1992), Eqs. 27a/27b
//!
//! First the (Batzle-Wang) pure-water density `rho_w` (g/cm^3) as a function of
//! temperature `T` (°C) and pressure `P` (MPa):
//!
//! ```text
//! rho_w = 1 + 1e-6 * ( -80 T - 3.3 T^2 + 0.00175 T^3 + 489 P - 2 T P
//!                      + 0.016 T^2 P - 1.3e-5 T^3 P - 0.333 P^2 - 0.002 T P^2 )
//! ```
//!
//! then the brine density with NaCl mass fraction `w` (0..1):
//!
//! ```text
//! rho_b = rho_w + w * ( 0.668 + 0.44 w
//!         + 1e-6 * ( 300 P - 2400 P w
//!                    + T ( 80 + 3 T - 3300 w - 13 P + 47 P w ) ) )
//! ```
//!
//! The result (g/cm^3) is multiplied by 1000 to return kg/m^3. At fixed `T, P`,
//! `rho_b` rises monotonically with `w`, so brine is always denser than pure
//! water at the same state — the buoyancy contrast that drives CO2 override in
//! GCS.
//!
//! ## Viscosity — Batzle-Wang (1992), Eq. 29
//!
//! ```text
//! eta = 0.1 + 0.333 w + (1.65 + 91.9 w^3)
//!       * exp( -( 0.42 (w^0.8 - 0.17)^2 + 0.045 ) * T^0.8 )    [centipoise]
//! ```
//!
//! with `w` the NaCl mass fraction and `T` in °C. The result (cP) is multiplied
//! by 1e-3 to return Pa.s. Viscosity rises with salinity and falls with
//! temperature.
//!
//! # Accuracy / scope
//!
//! Batzle-Wang are empirical fits (exploration-geophysics fidelity), valid
//! roughly over `T` in 20-350 °C, `P` up to ~100 MPa, and NaCl up to
//! saturation. They are reasonable but are **not** a reference EOS. Near ambient
//! the pure-water term gives ~0.997 g/cm^3 at 20 °C (vs IAPWS 0.9982) — a ~0.1%
//! low bias inherent to the fit. Higher-fidelity brine viscosity models
//! (Kestin et al. 1981; Mao & Duan 2009) are noted as follow-up but not
//! implemented here.
//!
//! Untrusted AI-generated draft (`RESPONSIBLE_USE.md`); see the parent module
//! [`crate::eos_co2_brine`] for the full warning and provenance.

use crate::error::PflotranError;

/// Maximum NaCl mass fraction accepted (~ halite saturation near ambient).
const W_MAX: f64 = 0.26;

/// Minimum brine temperature accepted, °C (Batzle-Wang lower validity edge).
const T_MIN_C: f64 = 0.0;

/// Maximum brine temperature accepted, °C (Batzle-Wang upper validity edge).
const T_MAX_C: f64 = 350.0;

/// Maximum brine pressure accepted, MPa.
const P_MAX_MPA: f64 = 100.0;

/// NaCl brine (aqueous phase) with a fixed NaCl mass fraction, using the
/// Batzle & Wang (1992) density and viscosity correlations.
///
/// The salinity is stored as [`Brine::nacl_mass_fraction`] (mass fraction, not
/// molality — dimensionless, `0 <= w <= 0.26`). Because the field is public,
/// the salinity range is validated inside each method rather than at
/// construction. See the module documentation for the correlations, units
/// (`T` in °C, `P` in MPa), valid ranges, and accuracy notes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Brine {
    /// NaCl mass fraction (dimensionless), valid `0 <= w <= 0.26`
    /// (0.26 ≈ halite saturation near ambient). `0.0` is pure water; seawater
    /// is ≈ `0.035`.
    pub nacl_mass_fraction: f64,
}

impl Brine {
    /// Construct a brine with the given NaCl mass fraction (dimensionless).
    ///
    /// This does **not** validate the fraction (the field is public and can be
    /// set directly either way); the range check happens in [`Self::density`]
    /// and [`Self::viscosity`]. Provided as a convenience over the struct
    /// literal.
    pub fn new(nacl_mass_fraction: f64) -> Self {
        Self { nacl_mass_fraction }
    }

    /// Brine density (kg/m^3) at temperature `temperature_c` (°C) and pressure
    /// `pressure_mpa` (MPa).
    ///
    /// Batzle-Wang (1992) Eqs. 27a/27b (see the module docs). Rises with both
    /// salinity and pressure; brine is denser than pure water at the same state.
    /// For seawater-like brine (`w = 0.035`) at 20 °C, 0.1 MPa this is
    /// ≈ 1021 kg/m^3.
    ///
    /// # Errors
    ///
    /// [`PflotranError::InvalidInput`] if `w` is outside `[0, 0.26]`, `T` is
    /// outside `[0, 350] °C`, `P` is outside `(0, 100] MPa`, or any input is
    /// non-finite.
    pub fn density(
        &self,
        temperature_c: f64,
        pressure_mpa: f64,
    ) -> Result<f64, PflotranError> {
        self.validate(temperature_c, pressure_mpa)?;
        let t = temperature_c;
        let p = pressure_mpa;
        let w = self.nacl_mass_fraction;

        // Pure-water density (g/cm^3), Batzle-Wang.
        let rho_w = 1.0
            + 1.0e-6
                * (-80.0 * t - 3.3 * t * t + 0.001_75 * t * t * t + 489.0 * p - 2.0 * t * p
                    + 0.016 * t * t * p
                    - 1.3e-5 * t * t * t * p
                    - 0.333 * p * p
                    - 0.002 * t * p * p);

        // Brine correction (g/cm^3), Batzle-Wang.
        let rho_b = rho_w
            + w * (0.668
                + 0.44 * w
                + 1.0e-6
                    * (300.0 * p - 2400.0 * p * w
                        + t * (80.0 + 3.0 * t - 3300.0 * w - 13.0 * p + 47.0 * p * w)));

        // g/cm^3 -> kg/m^3.
        Ok(rho_b * 1000.0)
    }

    /// Brine dynamic viscosity (Pa.s) at temperature `temperature_c` (°C).
    ///
    /// Batzle-Wang (1992) Eq. 29 (see the module docs). The `pressure_mpa`
    /// argument is validated but does **not** enter the correlation (Eq. 29 is
    /// pressure-independent). Rises with salinity, falls with temperature. For
    /// pure water at 20 °C this is ≈ 9.8e-4 Pa.s.
    ///
    /// # Errors
    ///
    /// [`PflotranError::InvalidInput`] on the same conditions as
    /// [`Self::density`].
    pub fn viscosity(
        &self,
        temperature_c: f64,
        pressure_mpa: f64,
    ) -> Result<f64, PflotranError> {
        self.validate(temperature_c, pressure_mpa)?;
        let t = temperature_c;
        let w = self.nacl_mass_fraction;

        // Batzle-Wang Eq. 29, result in centipoise.
        let eta_cp = 0.1
            + 0.333 * w
            + (1.65 + 91.9 * w * w * w)
                * (-(0.42 * (w.powf(0.8) - 0.17).powi(2) + 0.045) * t.powf(0.8)).exp();

        // cP -> Pa.s.
        Ok(eta_cp * 1.0e-3)
    }

    /// Validate salinity and `(T [°C], P [MPa])` for the Batzle-Wang
    /// correlations.
    ///
    /// Accepts `0 <= w <= 0.26`, `0 <= T <= 350 °C`, `0 < P <= 100 MPa`, all
    /// finite. Returns [`PflotranError::InvalidInput`] otherwise.
    fn validate(
        &self,
        temperature_c: f64,
        pressure_mpa: f64,
    ) -> Result<(), PflotranError> {
        let w = self.nacl_mass_fraction;
        if !w.is_finite() || !temperature_c.is_finite() || !pressure_mpa.is_finite() {
            return Err(PflotranError::InvalidInput(format!(
                "Brine: salinity, temperature and pressure must be finite, got w={w}, \
                 T={temperature_c} C, P={pressure_mpa} MPa"
            )));
        }
        if !(0.0..=W_MAX).contains(&w) {
            return Err(PflotranError::InvalidInput(format!(
                "Brine: NaCl mass fraction {w} is outside the valid range [0, {W_MAX}]"
            )));
        }
        if !(T_MIN_C..=T_MAX_C).contains(&temperature_c) {
            return Err(PflotranError::InvalidInput(format!(
                "Brine: temperature {temperature_c} C is outside the Batzle-Wang range \
                 [{T_MIN_C}, {T_MAX_C}] C"
            )));
        }
        if pressure_mpa <= 0.0 || pressure_mpa > P_MAX_MPA {
            return Err(PflotranError::InvalidInput(format!(
                "Brine: pressure {pressure_mpa} MPa is outside the valid range (0, {P_MAX_MPA}] MPa"
            )));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Seawater-salinity brine (~3.5% NaCl) at 20 °C, 0.1 MPa.
    ///
    /// Reference (Batzle & Wang 1992, and near real seawater): ≈ 1024 kg/m^3.
    /// The correlation yields ≈ 1021 kg/m^3 here (~0.3% low, within the ~1%
    /// tolerance the reference is quoted to).
    #[test]
    fn brine_seawater_density_about_1024() {
        let brine = Brine::new(0.035);
        let rho = brine.density(20.0, 0.1).unwrap();
        // Measured Batzle-Wang value ≈ 1021.1 kg/m^3.
        assert!(
            (rho - 1021.1).abs() < 1.0,
            "expected Batzle-Wang ~1021.1 kg/m^3, got {rho}"
        );
        // Within ~1% of the quoted seawater reference (~1024 kg/m^3).
        assert!(
            (rho - 1024.0).abs() < 12.0,
            "brine density {rho} should be within ~1% of ~1024 kg/m^3"
        );
    }

    /// Brine is denser than pure water at the same state, and density rises
    /// with NaCl fraction.
    #[test]
    fn brine_density_increases_with_salinity() {
        let pure = Brine::new(0.0).density(20.0, 0.1).unwrap();
        let low = Brine::new(0.05).density(20.0, 0.1).unwrap();
        let high = Brine::new(0.15).density(20.0, 0.1).unwrap();
        assert!(
            low > pure,
            "brine must be denser than pure water: pure {pure}, 5% {low}"
        );
        assert!(
            high > low,
            "density must rise with NaCl fraction: 5% {low}, 15% {high}"
        );
        // Pure-water term near 997 kg/m^3 at 20 C (Batzle-Wang water fit).
        assert!(
            (pure - 997.1).abs() < 1.0,
            "expected ~997.1 kg/m^3 pure water, got {pure}"
        );
    }

    /// Brine viscosity rises with salinity and falls with temperature.
    #[test]
    fn brine_viscosity_salinity_and_temperature_trends() {
        let pure_20 = Brine::new(0.0).viscosity(20.0, 0.1).unwrap();
        let salt_20 = Brine::new(0.10).viscosity(20.0, 0.1).unwrap();
        let pure_80 = Brine::new(0.0).viscosity(80.0, 0.1).unwrap();
        // Pure water at 20 C: Batzle-Wang gives ~0.981 cP = 9.81e-4 Pa.s.
        assert!(
            (pure_20 - 9.81e-4).abs() < 3.0e-5,
            "expected ~9.81e-4 Pa.s pure water at 20 C, got {pure_20}"
        );
        assert!(
            salt_20 > pure_20,
            "viscosity must rise with salinity: pure {pure_20}, 10% {salt_20}"
        );
        assert!(
            pure_80 < pure_20,
            "viscosity must fall with temperature: 20 C {pure_20}, 80 C {pure_80}"
        );
    }

    /// Input validation: out-of-range salinity, temperature, pressure, and
    /// non-finite inputs are rejected.
    #[test]
    fn brine_rejects_invalid_inputs() {
        assert!(Brine::new(0.30).density(20.0, 0.1).is_err()); // w > 0.26
        assert!(Brine::new(-0.01).density(20.0, 0.1).is_err()); // w < 0
        assert!(Brine::new(0.035).density(-5.0, 0.1).is_err()); // negative T (below range)
        assert!(Brine::new(0.035).density(20.0, -1.0).is_err()); // negative P
        assert!(Brine::new(0.035).density(20.0, 0.0).is_err()); // zero P
        assert!(Brine::new(0.035).density(400.0, 0.1).is_err()); // T over range
        assert!(Brine::new(0.035).viscosity(f64::NAN, 0.1).is_err());
        assert!(Brine::new(f64::INFINITY).density(20.0, 0.1).is_err());
    }
}
