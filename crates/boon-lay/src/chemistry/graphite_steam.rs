//! IG-110 graphite oxidation by steam, `C + H2O -> CO + H2`.
//!
//! # Source
//!
//! C. Wang and X. Sun, "Experimental study on kinetic oxidation of graphite
//! IG-110 by steam", *Nuclear Engineering and Design* **410** (2023) 112382
//! (proprietary tier: cited, not redistributed).
//!
//! - **Model:** the Boltzmann-enhanced Langmuir-Hinshelwood (BLH) form, their
//!   Eqs. (2)-(3):
//!
//!   ```text
//!   R_spe = k1 exp(-E1/RT) P_H2O^m(T)
//!         / [1 + k2 exp(-E2/RT) P_H2^n + k3 exp(-E3/RT) P_H2O^m(T)]
//!   m(T)  = m_max + (m_min - m_max) / (1 + exp((T - T0)/theta))
//!   ```
//!
//!   with `R_spe` the **specific** oxidation rate (fraction of the graphite
//!   mass per second, 1/s) and the partial pressures in Pa.
//! - **Coefficients:** Table 8, p. 11, the **"Unknown n"** column (the
//!   eleven-coefficient fit, n optimised to 0.801; MRD 23.9 %). Read off the
//!   rendered page 2026-09-29, not only the text layer.
//! - **Validity:** their measurements -- **850-1100 degC, P_H2O 0.5-20 kPa,
//!   P_H2 0-2 kPa**, graphite IG-110 thin disks in the **chemical-kinetics
//!   regime** (their Section 3: the rate is not limited by in-pore or
//!   boundary-layer diffusion). Outside that box the fit is an extrapolation,
//!   and [`Validity`] says so.
//!
//! # The reaction enthalpy
//!
//! [`REACTION_ENTHALPY_J_PER_MOL`] = **+131.3 kJ/mol** (endothermic), from the
//! standard enthalpies of formation at 298.15 K: CO(g) -110.53 kJ/mol and
//! H2O(g) -241.83 kJ/mol (NIST Chemistry WebBook, SRD 69; public data). Its
//! temperature dependence (a few kJ/mol up to 1300 K) is not carried.

use uom::si::f64::{Frequency, Pressure, ThermodynamicTemperature};
use uom::si::frequency::hertz;
use uom::si::pressure::pascal;
use uom::si::thermodynamic_temperature::kelvin;

/// Universal gas constant \[J/(mol K)\], as Wang & Sun use it (8.314).
const R_J_PER_MOL_K: f64 = 8.314;

/// Standard enthalpy of `C(s) + H2O(g) -> CO(g) + H2(g)` \[J/mol\]:
/// `-110.53 - (-241.83)` kJ/mol = **+131.30 kJ/mol** (NIST WebBook formation
/// enthalpies at 298.15 K). Positive: the reaction absorbs heat.
pub const REACTION_ENTHALPY_J_PER_MOL: f64 = 131.30e3;

/// Molar mass of carbon \[kg/mol\] (12.011 g/mol, IUPAC standard atomic weight).
pub const CARBON_MOLAR_MASS_KG_PER_MOL: f64 = 12.011e-3;

/// Wang & Sun (2023) Table 8, "Unknown n" column.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BlhCoefficients {
    /// `k1` \[Pa^-m s^-1\].
    pub k1: f64,
    /// `E1` \[J/mol\].
    pub e1: f64,
    /// `k2` \[Pa^-n\].
    pub k2: f64,
    /// `E2` \[J/mol\].
    pub e2: f64,
    /// `k3` \[Pa^-m\].
    pub k3: f64,
    /// `E3` \[J/mol\].
    pub e3: f64,
    /// `m_min` \[-\].
    pub m_min: f64,
    /// `m_max` \[-\].
    pub m_max: f64,
    /// `T0` \[K\].
    pub t0: f64,
    /// `theta` \[K\].
    pub theta: f64,
    /// Hydrogen reaction order `n` \[-\].
    pub n: f64,
}

impl BlhCoefficients {
    /// Table 8 (p. 11), "Unknown n": k1 79.15, E1 258.22 kJ/mol, k2
    /// 8.232e-4, E2 -19.31 kJ/mol, k3 3.496e-2, E3 313.86 kJ/mol, m_min
    /// 0.364, m_max 0.742, T0 1250.1 K, theta 34.08 K, n 0.801.
    pub fn wang_sun_2023_ig110() -> Self {
        Self {
            k1: 79.15,
            e1: 258.22e3,
            k2: 8.232e-4,
            e2: -19.31e3,
            k3: 3.496e-2,
            e3: 313.86e3,
            m_min: 0.364,
            m_max: 0.742,
            t0: 1250.1,
            theta: 34.08,
            n: 0.801,
        }
    }
}

/// Whether a rate was evaluated inside the fit's measured range.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Validity {
    /// 850-1100 degC, P_H2O 0.5-20 kPa, P_H2 <= 2 kPa.
    InsideMeasuredRange,
    /// Outside it: an extrapolation of a kinetic-regime fit, which for a
    /// thick graphite body at high temperature OVER-states the rate (the real
    /// rate becomes limited by in-pore and boundary-layer diffusion).
    Extrapolated,
}

/// The specific oxidation rate `R_spe` \[1/s\] (fraction of the graphite mass
/// per second) and whether it is inside the fit's range. Zero steam gives
/// zero rate.
pub fn specific_rate(
    temperature: ThermodynamicTemperature,
    steam: Pressure,
    hydrogen: Pressure,
    c: &BlhCoefficients,
) -> (Frequency, Validity) {
    let t = temperature.get::<kelvin>();
    let p_h2o = steam.get::<pascal>().max(0.0);
    let p_h2 = hydrogen.get::<pascal>().max(0.0);
    let m = c.m_max + (c.m_min - c.m_max) / (1.0 + ((t - c.t0) / c.theta).exp());
    let rt = R_J_PER_MOL_K * t;
    let steam_term = p_h2o.powf(m);
    let numerator = c.k1 * (-c.e1 / rt).exp() * steam_term;
    let denominator =
        1.0 + c.k2 * (-c.e2 / rt).exp() * p_h2.powf(c.n) + c.k3 * (-c.e3 / rt).exp() * steam_term;
    let inside =
        (1123.15..=1373.15).contains(&t) && (500.0..=20_000.0).contains(&p_h2o) && p_h2 <= 2_000.0;
    (
        Frequency::new::<hertz>(numerator / denominator),
        if inside {
            Validity::InsideMeasuredRange
        } else {
            Validity::Extrapolated
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rate(t_c: f64, p_h2o: f64, p_h2: f64) -> (f64, Validity) {
        let (r, v) = specific_rate(
            ThermodynamicTemperature::new::<kelvin>(t_c + 273.15),
            Pressure::new::<pascal>(p_h2o),
            Pressure::new::<pascal>(p_h2),
            &BlhCoefficients::wang_sun_2023_ig110(),
        );
        (r.get::<hertz>(), v)
    }

    /// **The transcription reproduces an independent evaluation, and the
    /// rate sits inside the band the paper plots.**
    ///
    /// Methodology: evaluate at 1050 degC, 5 kPa steam, no hydrogen, and
    /// compare with the same formula evaluated by hand (Python, 2026-09-29:
    /// m = 0.702332, numerator 2.00498e-6 /s, denominator 1 + 5.6e-12) to 1e-4
    /// relative; then check the value lies in 1e-7..1e-5 /s, the range of
    /// Wang & Sun's measured specific rates at 1050 degC (Fig. 17, p. 11,
    /// read off the rendered page: the ~1050 degC points span about 1e-6 to
    /// 1e-5 /s). Also: hydrogen inhibits, temperature accelerates, zero steam
    /// gives zero, and the validity flag tracks the measured box.
    ///
    /// Results (2026-09-29): printed below; pass.
    #[test]
    fn the_blh_fit_is_transcribed_and_behaves() {
        let (r, v) = rate(1050.0, 5_000.0, 0.0);
        println!("R_spe(1050 degC, 5 kPa H2O, 0 H2) = {r:.4e} /s ({v:?})");
        assert_eq!(v, Validity::InsideMeasuredRange);
        assert!((r - 2.004984e-6).abs() / 2.004984e-6 < 1e-4, "{r:e}");
        assert!(r > 1e-7 && r < 1e-5);
        assert!(rate(1050.0, 5_000.0, 1_000.0).0 < r);
        assert!(rate(950.0, 5_000.0, 0.0).0 < r);
        assert_eq!(rate(1050.0, 0.0, 0.0).0, 0.0);
        assert_eq!(rate(1050.0, 400_000.0, 0.0).1, Validity::Extrapolated);
        assert_eq!(rate(700.0, 5_000.0, 0.0).1, Validity::Extrapolated);
    }

    /// The reaction enthalpy is the difference of the two cited formation
    /// enthalpies.
    #[test]
    fn the_reaction_enthalpy_is_the_formation_enthalpy_difference() {
        assert!((REACTION_ENTHALPY_J_PER_MOL - (-110.53e3 + 241.83e3)).abs() < 1e-9);
    }
}
