//! IG-110 graphite oxidation by air, `C + O2 -> CO2` (and `C + 1/2 O2 -> CO`).
//!
//! # Source
//!
//! C. I. Contescu et al., *Oxidation of nuclear graphite* (review), ORNL/TM-2022/1839
//! (open corpus: `theodore-open-corpus/contescu2022ornltm20221839.pdf`).
//!
//! - **Rate law:** Regime-1 (chemical control) Arrhenius form, their Eqs. (8)-(9):
//!   `Rate = A exp(-E_a/RT)`, with `A = k0 P_O2^n` measured in air (21 % O2).
//! - **Parameters:** Table 3 (p. 39), "Selected kinetic parameters oxidation of
//!   IG-110 by air", row **Contescu (2011)**: `E_a = 191 kJ/mol`,
//!   `ln A = 13.0` (A in 1/s, fraction of mass per second), 597-694 degC,
//!   21 % O2, ASTM D7542 method. Chosen as the ORNL row the review builds its
//!   comparison on (its text, p. 46: IG-110 191-195 kJ/mol, ln A 13.0-13.1).
//! - **Oxygen order `n`: not given for IG-110 in the review.** This module
//!   scales linearly with the O2 partial pressure (`n = 1`), an **assumption,
//!   labelled**. It matters only where the kinetic rate limits; in an HTGR
//!   air-ingress transient at core temperatures the kinetic rate on the bed's
//!   graphite exceeds any credible O2 supply by orders of magnitude, so the
//!   supply limit binds ([`gasification_rate`]).
//! - **Validity:** 597-694 degC, 21 % O2, kinetic regime. Above it the real
//!   rate is limited by in-pore diffusion (Regime 2) and boundary-layer mass
//!   transfer (Regime 3, the review's s.4.2); the Arrhenius extrapolation
//!   OVER-states the kinetic rate there, which is why the supply limit is
//!   applied.
//!
//! # Heat
//!
//! [`CO2_REACTION_ENTHALPY_J_PER_MOL`] = -393.7 kJ/mol and
//! [`CO_REACTION_ENTHALPY_J_PER_MOL`] = -111.4 kJ/mol (the review's s.4.2.1,
//! p. 11). The CO/CO2 product split is not given there; the caller chooses,
//! and CO2 is the bounding choice for heat per mole of O2 consumed (393.7
//! against 2 x 111.4 = 222.8 kJ).

use uom::si::f64::{Frequency, Pressure, ThermodynamicTemperature};
use uom::si::frequency::hertz;
use uom::si::pressure::pascal;
use uom::si::thermodynamic_temperature::kelvin;

/// `C + O2 -> CO2` \[J/mol\], exothermic (Contescu review s.4.2.1).
pub const CO2_REACTION_ENTHALPY_J_PER_MOL: f64 = -393.7e3;
/// `C + 1/2 O2 -> CO` \[J/mol\], exothermic (same source).
pub const CO_REACTION_ENTHALPY_J_PER_MOL: f64 = -111.4e3;
/// Apparent activation energy \[J/mol\] (Table 3, Contescu 2011).
pub const ACTIVATION_ENERGY_J_PER_MOL: f64 = 191.0e3;
/// `ln A`, A in 1/s (Table 3, Contescu 2011), measured in air.
pub const LN_PRE_EXPONENTIAL_PER_S: f64 = 13.0;
/// O2 partial pressure of the measurement: 21 % of 1 atm \[Pa\].
pub const REFERENCE_O2_PA: f64 = 0.21 * 101_325.0;
const R_J_PER_MOL_K: f64 = 8.314;

/// Whether a kinetic rate was evaluated inside the measured range.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Validity {
    /// 597-694 degC.
    InsideMeasuredRange,
    /// Outside it (extrapolated Arrhenius; over-states the rate above it).
    Extrapolated,
}

/// The kinetic (Regime-1) specific rate \[1/s\] at `temperature` and O2
/// partial pressure `oxygen`, first order in O2 (assumption, module doc).
pub fn kinetic_specific_rate(
    temperature: ThermodynamicTemperature,
    oxygen: Pressure,
) -> (Frequency, Validity) {
    let t = temperature.get::<kelvin>();
    let p = oxygen.get::<pascal>().max(0.0);
    let rate = LN_PRE_EXPONENTIAL_PER_S.exp()
        * (-ACTIVATION_ENERGY_J_PER_MOL / (R_J_PER_MOL_K * t)).exp()
        * p
        / REFERENCE_O2_PA;
    let v = if (870.15..=967.15).contains(&t) {
        Validity::InsideMeasuredRange
    } else {
        Validity::Extrapolated
    };
    (Frequency::new::<hertz>(rate), v)
}

/// Which limit set a gasification rate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Limit {
    /// The chemical kinetics.
    Kinetic,
    /// The oxygen supply (every arriving O2 molecule reacts).
    OxygenSupply,
}

/// Carbon gasified \[mol/s\] from `graphite_mass_kg` of graphite: the lesser of
/// the kinetic rate and the O2 supply `oxygen_supply_mol_per_s` (one C per O2,
/// the CO2 product). Zero supply gives zero.
pub fn gasification_rate(
    temperature: ThermodynamicTemperature,
    oxygen: Pressure,
    graphite_mass_kg: f64,
    oxygen_supply_mol_per_s: f64,
) -> (f64, Limit, Validity) {
    let (k, v) = kinetic_specific_rate(temperature, oxygen);
    let kinetic = k.get::<hertz>() * graphite_mass_kg / 12.011e-3;
    let supply = oxygen_supply_mol_per_s.max(0.0);
    if kinetic <= supply {
        (kinetic, Limit::Kinetic, v)
    } else {
        (supply, Limit::OxygenSupply, v)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **Table 3 is transcribed, and at core temperatures the O2 supply
    /// binds.** Methodology: at 650 degC in air, compare with a hand
    /// evaluation (Python, 2026-09-29: exp(13.0) exp(-191000/(8.314 x 923.15))
    /// = 6.8877e-6 /s) to 1e-4; then at 1173 K, 5000 kg graphite and an O2
    /// supply of 1 mol/s the supply limit binds. Results (2026-09-29): pass.
    #[test]
    fn table_3_is_transcribed_and_supply_binds_at_core_temperature() {
        let t = |c: f64| ThermodynamicTemperature::new::<kelvin>(c + 273.15);
        let air = Pressure::new::<pascal>(REFERENCE_O2_PA);
        let (r, v) = kinetic_specific_rate(t(650.0), air);
        let r = r.get::<hertz>();
        println!("IG-110 in air at 650 degC: {r:.4e} /s");
        assert_eq!(v, Validity::InsideMeasuredRange);
        assert!((r - 6.8877e-6).abs() / 6.8877e-6 < 1e-4, "{r:e}");
        let (mol, limit, v) = gasification_rate(t(900.0), air, 5000.0, 1.0);
        assert_eq!((limit, v), (Limit::OxygenSupply, Validity::Extrapolated));
        assert_eq!(mol, 1.0);
        assert_eq!(gasification_rate(t(900.0), air, 5000.0, 0.0).0, 0.0);
    }
}
