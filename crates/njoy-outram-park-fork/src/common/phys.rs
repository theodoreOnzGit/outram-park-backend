//! Physical constants used across NJOY modules.
//!
//! Values are from CODATA 2014 as given in ENDF-102 Appendix H (Feb 2018),
//! identical to NJOY2016 `src/phys.f90`. All quantities are in CGS units
//! (as NJOY uses internally) unless stated otherwise.
//!
//! NJOY's `kr` kind is `real(8)` = `f64`, so all values are `f64`.

/// π
pub const PI: f64 = std::f64::consts::PI;

/// Euler–Mascheroni constant γ ≈ 0.5772156…
pub const EULER: f64 = 0.577_215_664_901_532_86;

/// Boltzmann constant \[eV / K\]
pub const BK_EV_PER_K: f64 = 8.617_333_262e-5;

/// Conversion factor: 1 eV in ergs \[erg/eV\]
pub const EV_ERG: f64 = 1.602_176_634e-12;

/// Speed of light \[cm/s\]
pub const CLIGHT_CM_S: f64 = 2.997_924_58e10;

/// Atomic mass unit \[g/amu\]
///
/// Derived as `AMU_EV × EV_ERG / CLIGHT_CM_S²` where `AMU_EV = 931.494 10242 MeV/c²`.
pub const AMU_G: f64 = 931.494_102_42e6 * EV_ERG / (CLIGHT_CM_S * CLIGHT_CM_S);

/// Reduced Planck constant ℏ \[erg·s\]
///
/// Derived as `6.582119569e−16 eV·s × EV_ERG`.
pub const HBAR_ERG_S: f64 = 6.582_119_569e-16 * EV_ERG;

/// Inverse fine-structure constant α⁻¹ ≈ 137.036 (dimensionless)
pub const INV_FINE_STRUCT: f64 = 1.0e16 * HBAR_ERG_S / (EV_ERG * EV_ERG * CLIGHT_CM_S);

// --- Particle masses in amu ---

/// Neutron mass \[amu\]
pub const AMASSN_AMU: f64 = 1.008_664_915_95;
/// Proton mass \[amu\]
pub const AMASSP_AMU: f64 = 1.007_276_466_621;
/// Deuteron mass \[amu\]
pub const AMASSD_AMU: f64 = 2.013_553_212_745;
/// Triton mass \[amu\]
pub const AMASST_AMU: f64 = 3.015_500_716_21;
/// Helion (³He) mass \[amu\]
pub const AMASSH_AMU: f64 = 3.014_932_247_175;
/// Alpha (⁴He) mass \[amu\]
pub const AMASSA_AMU: f64 = 4.001_506_179_127;
/// Electron mass \[amu\]
pub const AMASSE_AMU: f64 = 5.485_799_090_65e-4;

// --- Mass ratios relative to neutron ---

/// Proton / neutron mass ratio
pub const PNRATIO: f64 = AMASSP_AMU / AMASSN_AMU;
/// Deuteron / neutron mass ratio
pub const DNRATIO: f64 = AMASSD_AMU / AMASSN_AMU;
/// Triton / neutron mass ratio
pub const TNRATIO: f64 = AMASST_AMU / AMASSN_AMU;
/// Helion / neutron mass ratio
pub const HNRATIO: f64 = AMASSH_AMU / AMASSN_AMU;
/// Alpha / neutron mass ratio
pub const ANRATIO: f64 = AMASSA_AMU / AMASSN_AMU;

/// Electron rest-mass energy \[eV\]
///
/// `m_e c² = AMASSE_AMU × AMU_G × CLIGHT² / EV_ERG`
pub const EPAIR_EV: f64 = AMASSE_AMU * AMU_G * CLIGHT_CM_S * CLIGHT_CM_S / EV_ERG;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn boltzmann_constant() {
        // Sanity: room temperature (293.15 K) in eV ≈ 0.02526 eV
        let kt = BK_EV_PER_K * 293.15;
        assert!((kt - 0.02526).abs() < 0.0001, "kT(293K) = {} eV", kt);
    }

    #[test]
    fn electron_mass_energy() {
        // m_e c² ≈ 510.999 keV = 510999 eV
        assert!(
            (EPAIR_EV - 510_999.0).abs() < 1.0,
            "m_e c² = {} eV",
            EPAIR_EV
        );
    }

    #[test]
    fn neutron_mass_ratio_unity() {
        // AMASSN / AMASSN = 1.0 (trivially)
        assert_eq!(AMASSN_AMU / AMASSN_AMU, 1.0);
    }
}
