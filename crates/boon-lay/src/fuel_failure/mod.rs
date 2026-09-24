// SPDX-License-Identifier: GPL-3.0
//
// PANAMA-I reimplementation — provenance
// --------------------------------------
// Reference : Verfondern, K. & Nabielek, H., "The Mathematical Basis of the
//             PANAMA-I Code for Modeling Pressure Vessel Failure of TRISO
//             Coated Particles under Accident Conditions",
//             Forschungszentrum Jülich, HTA-IB-03/90, 1 August 1990.
//             Reprinted as Appendix C, printed pages -479- to -511-.
// Status    : the report is restricted literature with no reuse licence. Only
//             the governing EQUATIONS and their constants are reproduced here,
//             with citation, as scientific facts. No prose, figure or page of
//             that document is copied into this repository, and the PDF is not
//             tracked here. See DATA_POLICY.md.
// Nature    : an independent Rust implementation of the published model, not a
//             port of the PANAMA Fortran (which is closed-source and was never
//             consulted).

//! # TRISO coated-particle failure — the PANAMA-I pressure-vessel model
//!
//! A TRISO particle is a pressure vessel. Fission gas and CO accumulate inside
//! it, the SiC layer carries the hoop stress, and the particle fails when that
//! stress exceeds the SiC strength. PANAMA-I couples three failure populations:
//!

//! # TRISO coated-particle failure — the PANAMA-I pressure-vessel model
//!
//! A TRISO particle is a pressure vessel. Fission gas and CO accumulate inside
//! it, the SiC layer carries the hoop stress, and the particle fails when that
//! stress exceeds the SiC strength. PANAMA-I couples three failure populations:
//!
//! | Term | Mechanism | Where it comes from |
//! |---|---|---|
//! | `φ_o` | as-manufactured defects | not modelled; an input (see [`AS_MANUFACTURED_TARGET`]) |
//! | `φ₁` | **pressure-vessel overstress** | [`weibull_failure_fraction`], this module |
//! | `φ₂` | SiC **thermal decomposition** above ~2000 °C | NOT YET IMPLEMENTED — see below |
//!
//! combined by [`total_failure_fraction`].
//!
//! ## What is implemented here, and what is deliberately absent
//!
//! **Implemented — every equation below was read directly off the source scan
//! and is reproduced with its printed equation number:**
//!
//! | Item | Eq. | Page |
//! |---|---|---|
//! | Weibull failure probability | (1) | -483- |
//! | Induced SiC stress, thin shell | (2) | -484- |
//! | Internal gas pressure, ideal gas | (3) | -484- |
//! | Failure-population combination | unnumbered | -480- |
//!
//! **Absent, and taken as INPUTS rather than guessed.** The correlations that
//! supply `σ_o`, `m`, `OPF` and `F_d` — the fluence degradation laws (8a/8b,
//! 9a/9b), the oxygen-per-fission fits (5a–5f), the Booth `f(τ)` series, the
//! `D_S` diffusion correlations and the decomposition kinetics (11/12) — are
//! **not** in this module. Two have unresolved ambiguities in the source scan
//! (the `t_B` seconds-vs-full-power-days question on (5b)/(5c), and a grouping
//! ambiguity in the `f(τ)` series), and the rest have not yet been verified
//! against the scan by a human.
//!
//! This is the deliberate shape, not an unfinished one: the failure **chain**
//! is exact and testable today, and each correlation can be dropped in behind
//! the same signature once it is confirmed. A guessed exponent in a correlation
//! would propagate silently into an absolute pressure and then into a failure
//! fraction that still *looks* reasonable.
//!
//! ## The `ln2` in Eq (1) is load-bearing — do not reach for a stock Weibull
//!
//! Eq (1) is `φ₁ = 1 − exp[−ln2·(σ_t/σ_o)^m]`, **not** the textbook
//! `1 − exp[−(σ/σ_c)^m]`. The `ln2` normalisation makes `σ_o` the **median**
//! strength: at `σ_t = σ_o`, `φ₁ = 1 − e^(−ln2) = 0.5` exactly. A stock Weibull
//! treats its scale parameter as the *characteristic* strength, where
//! `φ = 1 − e^(−1) ≈ 0.632`. Substituting one for the other misplaces the
//! strength scale by a factor `(ln2)^(1/m)` — about 4 % at `m = 8` — in a
//! direction that flatters the answer and produces no error.
//! [`median_is_the_scale_parameter`] pins this.
//!
//! ## Units
//!
//! Public signatures are `uom`-typed. Two traps from the source's own symbol
//! list (-511-): the report prints irradiation and accident temperatures in
//! **°C** while every Arrhenius term needs **kelvin**, and it never states the
//! conversion. Using `uom` removes that ambiguity at the boundary — a caller
//! passes a `ThermodynamicTemperature` and cannot get it wrong.
//!
//! # Layout
//!
//! One module per equation group, because a single file was already past 700
//! lines with four of the report's correlations implemented and there are a
//! dozen more to come:
//!
//! | Module | Equations | Page |
//! |---|---|---|
//! | [`geometry`] | `r`, `d_o`, `d_act` | -484- |
//! | [`weibull`] | (1) | -483- |
//! | [`stress`] | (2) | -484- |
//! | [`pressure`] | (3) | -484-/-485- |
//! | [`strength`] | (8a), (8b), (9a), (9b) | -493-/-494- |
//!
//! The assembly (`phi_total`) stays here, since it is what binds them.
//! Everything is re-exported flat, so a caller writes
//! `boon_lay::fuel_failure::weibull_failure_fraction` and never needs to know
//! which file it lives in.
//!
//! # Units: what the report states, and what it does not
//!
//! Three of the report's own symbols are ambiguous or wrong as printed. Each
//! is resolved (or left open) at the point of use, and the register lives in
//! `docs/panama-i-units-and-open-questions.md`. In brief:
//!
//! | Symbol | Printed | Used here | How settled |
//! |---|---|---|---|
//! | `T_B` | degC (-511-) | **kelvin** | Table 1 reproduces 16/16 on kelvin, 0/16 on degC |
//! | `Gamma` | 10^25 m^-2 EDN | same, as bare `f64` | a `log10` fit is only valid in its own units |
//! | Eq (3) grouping | bar spans the denominator | `R*T` in the numerator | dimensions; the printed form makes `p` fall with `T` |
//! | `t_B` | seconds (-511-) | **UNRESOLVED** | Figs 3/7/8 say full-power days |

pub mod geometry;
pub mod pressure;
pub mod strength;
pub mod stress;
pub mod weibull;

pub use geometry::SicLayer;
pub use pressure::{internal_gas_pressure, GAS_CONSTANT_J_PER_MOL_K, STABLE_FISSION_GAS_YIELD};
pub use strength::{
    irradiated_strength, irradiated_weibull_modulus, MIN_TENSILE_STRENGTH_MPA, MIN_WEIBULL_MODULUS,
};
pub use stress::{induced_stress, induced_stress_exact};
pub use weibull::weibull_failure_fraction;

use uom::si::f64::Ratio;
use uom::si::ratio::ratio;

/// A failed-particle fraction, dimensionless and in `[0, 1]`.
pub type FailureFraction = Ratio;

/// The as-manufactured defective fraction `φ_o` used for reactor studies,
/// `6·10⁻⁵` (page -480-).
///
/// The report's own calculations set `φ_o = 0`; this is the value it states may
/// "without difficulty" be used as a target when considering reactor concepts.
/// It is offered as a named constant, never as a default — which of the two
/// applies is the caller's modelling decision.
pub const AS_MANUFACTURED_TARGET: f64 = 6.0e-5;

/// The combination of the three failure populations (page -480-).
///
/// ```text
/// φ_total = 1 − (1 − φ_o)·(1 − φ₁)·(1 − φ₂)
/// ```
///
/// A particle survives only if it survives all three mechanisms, so the
/// *survival* probabilities multiply. This is why the result is not the sum:
/// summing would double-count particles failed by more than one mechanism and
/// can exceed 1.
///
/// - `as_manufactured` — `φ_o`. The report's own runs use `0`; see
///   [`AS_MANUFACTURED_TARGET`].
/// - `pressure_vessel` — `φ₁`, from [`weibull_failure_fraction`].
/// - `thermal_decomposition` — `φ₂`. **Not implemented in this module** (the
///   action-integral model, Eqs (11)/(12)); pass `Ratio::new::<ratio>(0.0)` to
///   model pressure-vessel failure alone, and be aware that doing so is
///   non-conservative above ~2000 °C, where the report attributes failure
///   principally to SiC decomposition.
pub fn total_failure_fraction(
    as_manufactured: FailureFraction,
    pressure_vessel: FailureFraction,
    thermal_decomposition: FailureFraction,
) -> FailureFraction {
    let survive = (1.0 - as_manufactured.get::<ratio>())
        * (1.0 - pressure_vessel.get::<ratio>())
        * (1.0 - thermal_decomposition.get::<ratio>());
    Ratio::new::<ratio>(1.0 - survive)
}

#[cfg(test)]
mod tests {
    use super::*;
    use uom::si::f64::{Length, MolarVolume, Pressure, Time, Velocity, Volume};
    use uom::si::length::micrometer;
    use uom::si::molar_volume::cubic_meter_per_mole;
    use uom::si::pressure::{megapascal, pascal};
    use uom::si::thermodynamic_temperature::degree_celsius;
    use uom::si::f64::ThermodynamicTemperature;
    use uom::si::time::second;
    use uom::si::velocity::meter_per_second;
    use uom::si::volume::cubic_meter;

    fn r(x: f64) -> Ratio {
        Ratio::new::<ratio>(x)
    }

    /// A representative TRISO SiC layer: a 35 um shell whose inner surface
    /// sits at 250 um. Dimensions only; nothing is calibrated to them.
    fn layer() -> SicLayer {
        SicLayer {
            inner_radius: Length::new::<micrometer>(250.0),
            outer_radius: Length::new::<micrometer>(285.0),
        }
    }

    #[test]
    fn failure_populations_combine_by_survival() {
        let (a, b, c) = (r(6.0e-5), r(0.3), r(0.2));
        let total = total_failure_fraction(a, b, c).get::<ratio>();
        let expected = 1.0 - (1.0 - 6.0e-5) * 0.7 * 0.8;
        assert!((total - expected).abs() < 1e-15);

        // Not the sum -- that would double-count.
        let sum = 6.0e-5 + 0.3 + 0.2;
        assert!(total < sum, "{total} should be below the naive sum {sum}");

        // Order cannot matter.
        assert!(
            (total - total_failure_fraction(c, a, b).get::<ratio>()).abs() < 1e-15,
            "combination must be symmetric"
        );

        // Any certain mechanism gives certain failure.
        assert_eq!(
            total_failure_fraction(r(0.0), r(1.0), r(0.0)).get::<ratio>(),
            1.0
        );
        // All-zero gives zero.
        assert_eq!(
            total_failure_fraction(r(0.0), r(0.0), r(0.0)).get::<ratio>(),
            0.0
        );
    }

    #[test]
    fn the_chain_composes_end_to_end() {
        let l = layer();
        let p = internal_gas_pressure(
            r(0.5),
            r(STABLE_FISSION_GAS_YIELD),
            r(0.4),
            r(0.10),
            Volume::new::<cubic_meter>(9.0e-13),
            Volume::new::<cubic_meter>(1.8e-13),
            MolarVolume::new::<cubic_meter_per_mole>(2.46e-5),
            ThermodynamicTemperature::new::<degree_celsius>(1600.0),
        );
        let sigma_t = induced_stress(
            &l,
            p,
            Velocity::new::<meter_per_second>(0.0),
            Time::new::<second>(0.0),
        );
        let phi1 = weibull_failure_fraction(sigma_t, Pressure::new::<megapascal>(200.0), 8.0);
        let total = total_failure_fraction(r(0.0), phi1, r(0.0));
        let v = total.get::<ratio>();
        assert!(
            (0.0..=1.0).contains(&v),
            "failure fraction out of range: {v}"
        );
        assert!(p.get::<pascal>() > 0.0 && sigma_t.get::<pascal>() > 0.0);
    }
}
