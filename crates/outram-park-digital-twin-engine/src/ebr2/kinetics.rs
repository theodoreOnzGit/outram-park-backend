//! # SHRT-45R kinetics parameters and reactivity coefficients
//!
//! **Source:** IAEA (2017), IAEA-TECDOC-1819, *Benchmark Analysis of EBR-II
//! Shutdown Heat Removal Tests* (T1819), which prints the values Argonne
//! provided to the CRP participants:
//!
//! - TABLE 37 (printed p.271): k-eff, beta-eff and four feedback
//!   coefficients, computed by Argonne with DIF3D finite-difference
//!   diffusion and with VARIANT nodal transport;
//! - Section 7.19 (pp.272-273): the prompt generation time and lifetime,
//!   and two uniform-dilation worths.
//!
//! These numbers were transcribed by an AI agent into gh:#1314 (B2) on
//! 2026-11-10. **Not yet reviewed by a human, and not in any
//! `reference-data/ebr2` CSV**, so no test can pin them to a file; check
//! them against T1819 before relying on them. Used under the maintainer's
//! EBR-II data exception (`DATA_POLICY.md`, 2026-11-10).
//!
//! **Which Argonne set the model uses: VARIANT, chosen before any run.**
//! VARIANT is a nodal *transport* solver and DIF3D-FDD a finite-difference
//! *diffusion* solver. For a small, leaky fast core (35 cm across) transport
//! is the better approximation, and VARIANT's k-eff (0.9885) is also the
//! closer of the two to the critical core. DIF3D is available as
//! [`ArgonneSolver::Dif3dFdd`]; the model's total coefficient differs by
//! under 1 % between them (-3.87 vs -3.90 pcm/K).
//!
//! **What the sources do not give, and what this module does about it:**
//!
//! - **Six-group delayed-neutron data** were "provided to participants" but
//!   are not printed in T1819 (gh:#1314, GAP). [`argonne_point_kinetics`]
//!   therefore uses the U-235 six-group shape `teh-o-prke` tabulates
//!   ~~(U-235 *thermal* decay constants, Keepin-derived, source still under
//!   gh:#1319)~~ **(since 2026-11-10, gh:#1319: ENDF/B-VIII.0 U-235, MAT
//!   9228 MT=455, `lambda_i` and the abundances `p_i`; the abundances are
//!   energy-independent on that tape, so the shape is the same for thermal
//!   and fast fission)**, rescaled to Argonne's beta-eff of 705 pcm. This is
//!   an approximation, labelled as such: EBR-II's fissions are not all
//!   U-235 (U-238 fast fission has its own set, `teh_o_prke`'s
//!   `ENDF_B_VIII0_U238`), and Argonne's own group data are not printed.
//! - **Control-rod driveline expansion** is not in Argonne's TABLE 37. The
//!   default model has no driveline term; a caller may add one
//!   (`teh_o_prke::feedback_mechanisms::fast_reactor::DrivelineExpansionTerm`).
//!   Other participants' values (e.g. U. Fukui -0.8 pcm/K, PSI -0.45 pcm/K,
//!   T1819 TABLES 20 and 30) are in gh:#1314.
//! - **The temperature each coefficient multiplies** is not stated with the
//!   coefficients. The model's choices are documented in
//!   [`super::model::RadialExpansionTemperature`] and
//!   [`super::model::ReactivityCoefficients::argonne`].
//!
//! **Source inconsistency:** KIT's TABLE 9 (p.110) quotes Argonne's VARIANT
//! Doppler coefficient as -0.05 pcm/K; TABLE 37 prints -0.06. TABLE 37,
//! Argonne's own table, is used.

use teh_o_prke::prelude::{FissioningNuclideType, PointKinetics};
use uom::si::f64::*;
use uom::si::ratio::ratio;
use uom::si::temperature_coefficient::per_kelvin;
use uom::si::thermodynamic_temperature::degree_celsius;
use uom::si::time::second;

/// One pcm, 1e-5.
pub const PCM: f64 = 1.0e-5;

/// beta-eff, 705 pcm (DIF3D and VARIANT agree). T1819 TABLE 37 p.271
/// (gh:#1314).
pub const BETA_EFF_PCM: f64 = 705.0;

/// Prompt neutron generation time, 3.58238e-7 s. T1819 Section 7.19
/// pp.272-273 (gh:#1314).
pub const PROMPT_GENERATION_TIME_S: f64 = 3.58238e-7;

/// Prompt neutron lifetime, 3.46402e-7 s. T1819 Section 7.19 pp.272-273
/// (gh:#1314).
pub const PROMPT_NEUTRON_LIFETIME_S: f64 = 3.46402e-7;

/// Uniform radial dilation worth: -1019 pcm for a 1 % pitch increase (all
/// number densities except sodium divided by 1.01^2). T1819 Section 7.19
/// (gh:#1314).
pub const RADIAL_DILATION_WORTH_PCM_PER_PERCENT: f64 = -1019.0;

/// Uniform axial fuel expansion worth: -2019 pcm for heavy-metal density
/// -10 % and fuel height +10 %. T1819 Section 7.19 (gh:#1314).
pub const AXIAL_EXPANSION_WORTH_PCM_PER_10_PERCENT: f64 = -2019.0;

/// The nominal average fuel temperature at which the Doppler constant is
/// defined, 487 C. A228 p.25 **(CSV, `arc228_text_values.csv`)**.
pub const DOPPLER_NOMINAL_FUEL_TEMPERATURE_C: f64 = 487.0;

/// Argonne's two solvers in T1819 TABLE 37.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArgonneSolver {
    /// DIF3D finite-difference diffusion.
    Dif3dFdd,
    /// VARIANT nodal transport (the model's default; see the module doc).
    Variant,
}

/// The TABLE 37 numbers for one solver, in the printed units.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ArgonneReferenceValues {
    /// k-eff.
    pub k_eff: f64,
    /// beta-eff, pcm.
    pub beta_eff_pcm: f64,
    /// Axial (fuel) expansion coefficient, pcm/K.
    pub axial_expansion_pcm_per_k: f64,
    /// Radial expansion coefficient, pcm/K.
    pub radial_expansion_pcm_per_k: f64,
    /// Sodium density coefficient, pcm/K.
    pub sodium_density_pcm_per_k: f64,
    /// Doppler coefficient, pcm/K.
    pub doppler_pcm_per_k: f64,
}

impl ArgonneSolver {
    /// T1819 TABLE 37 p.271 (gh:#1314): DIF3D 0.9670, 705, -0.36, -1.78,
    /// -1.70, -0.06; VARIANT 0.9885, 705, -0.65, -1.67, -1.49, -0.06.
    pub fn reference_values(self) -> ArgonneReferenceValues {
        match self {
            ArgonneSolver::Dif3dFdd => ArgonneReferenceValues {
                k_eff: 0.9670,
                beta_eff_pcm: 705.0,
                axial_expansion_pcm_per_k: -0.36,
                radial_expansion_pcm_per_k: -1.78,
                sodium_density_pcm_per_k: -1.70,
                doppler_pcm_per_k: -0.06,
            },
            ArgonneSolver::Variant => ArgonneReferenceValues {
                k_eff: 0.9885,
                beta_eff_pcm: 705.0,
                axial_expansion_pcm_per_k: -0.65,
                radial_expansion_pcm_per_k: -1.67,
                sodium_density_pcm_per_k: -1.49,
                doppler_pcm_per_k: -0.06,
            },
        }
    }
}

/// A coefficient printed in pcm/K as a uom [`TemperatureCoefficient`].
pub fn pcm_per_kelvin(value: f64) -> TemperatureCoefficient {
    TemperatureCoefficient::new::<per_kelvin>(value * PCM)
}

/// [`BETA_EFF_PCM`] as a [`Ratio`].
pub fn beta_eff() -> Ratio {
    Ratio::new::<ratio>(BETA_EFF_PCM * PCM)
}

/// [`PROMPT_GENERATION_TIME_S`] as a [`Time`].
pub fn prompt_generation_time() -> Time {
    Time::new::<second>(PROMPT_GENERATION_TIME_S)
}

/// [`DOPPLER_NOMINAL_FUEL_TEMPERATURE_C`] as a temperature.
pub fn doppler_nominal_fuel_temperature() -> ThermodynamicTemperature {
    ThermodynamicTemperature::new::<degree_celsius>(DOPPLER_NOMINAL_FUEL_TEMPERATURE_C)
}

/// Six-group point kinetics with `teh-o-prke`'s U-235 group shape rescaled
/// to `beta_eff` (see the module doc for why, and what it approximates).
pub fn argonne_point_kinetics(beta_eff: Ratio) -> PointKinetics {
    PointKinetics::from_nuclide(FissioningNuclideType::U235).with_total_delayed_fraction(beta_eff)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The rescaled six-group set sums to Argonne's 705 pcm, and the two
    /// solvers' totals are within 1 % of each other.
    #[test]
    fn beta_and_totals() {
        let pk = argonne_point_kinetics(beta_eff());
        let total = pk.total_delayed_fraction().get::<ratio>();
        assert!((total - 705.0e-5).abs() < 1e-15, "{total}");
        assert_eq!(pk.group_count(), 6);
        let sum = |v: ArgonneReferenceValues| {
            v.axial_expansion_pcm_per_k
                + v.radial_expansion_pcm_per_k
                + v.sodium_density_pcm_per_k
                + v.doppler_pcm_per_k
        };
        let d = sum(ArgonneSolver::Dif3dFdd.reference_values());
        let v = sum(ArgonneSolver::Variant.reference_values());
        println!("Argonne total coefficient: DIF3D {d:.2}, VARIANT {v:.2} pcm/K");
        assert!((d - v).abs() / d.abs() < 0.01);
        assert_eq!(ArgonneSolver::Variant.reference_values().beta_eff_pcm, BETA_EFF_PCM);
        assert!((pcm_per_kelvin(-1.0).get::<per_kelvin>() + 1e-5).abs() < 1e-20);
        assert!((prompt_generation_time().get::<second>() - 3.58238e-7).abs() < 1e-20);
        assert!(PROMPT_NEUTRON_LIFETIME_S < PROMPT_GENERATION_TIME_S);
        assert!((doppler_nominal_fuel_temperature().value - 760.15).abs() < 1e-9);
    }

    /// The uniform-dilation worths and the pcm/K coefficients are
    /// consistent with a steel expansion coefficient of order 1.7e-5 /K:
    /// -1019 pcm per 1 % pitch / -1.67 pcm/K implies 1.64e-5 /K (VARIANT),
    /// -1.78 pcm/K implies 1.75e-5 /K (DIF3D). A consistency check of the
    /// transcription, not a validation.
    #[test]
    fn dilation_worth_implies_a_steel_expansion_coefficient() {
        for solver in [ArgonneSolver::Dif3dFdd, ArgonneSolver::Variant] {
            let a = solver.reference_values().radial_expansion_pcm_per_k;
            let alpha = 0.01 * a / RADIAL_DILATION_WORTH_PCM_PER_PERCENT;
            println!("{solver:?}: implied linear expansion {alpha:.3e} /K");
            assert!(alpha > 1.4e-5 && alpha < 2.0e-5);
        }
        assert!(AXIAL_EXPANSION_WORTH_PCM_PER_10_PERCENT < 0.0);
    }
}
