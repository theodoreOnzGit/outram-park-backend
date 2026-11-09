//! MSRE coolant and flush salt, LiF-BeF₂ 66-34 mol % ("L₂B"), from
//! ORNL-TM-2316 (MSRE TH-6, gh:#1133).
//!
//! # Source
//!
//! S. Cantor (ed.), *Physical Properties of Molten-Salt Reactor Fuel,
//! Coolant, and Flush Salts*, ORNL-TM-2316 (August 1968). Values read from
//! the scan on 2026-11-09; they agree with the MSRE repository's AI
//! transcription on every L₂B row. Page numbers are report / PDF:
//!
//! | Property | TM-2316 | Uncertainty | Basis | Page |
//! |---|---|---|---|---|
//! | Density | ρ = 2.214 − 4.2e-4 t g/cm³, t in °C | ±2 % | additive molar volumes, about the mean of two measured sets | 28 / 34 |
//! | Viscosity | μ = 0.116 exp(3755/T) cP, T in K | ±15 % | measured | 8 / 14 |
//! | Thermal conductivity | 0.010 W/(cm·°C), temperature-independent | ±10 % | measured | 11 / 17 |
//! | Heat capacity, liquid | 0.57 cal/(g·°C) | ±3 % | measured | 22 / 28 |
//! | Liquidus | 458 °C (peritectic) | ±1 °C | | 2 / 8 |
//!
//! The report calls L₂B the "present MSRE coolant" and the flush salt.
//!
//! # Range
//!
//! From the liquidus, 458 °C, to 800 °C. **The upper bound is an assumption,
//! not a stated range:** it is the highest temperature of the molar volumes
//! the density equation is built on (TM-2316 p. 28). Outside the range,
//! TUAS's getters return an error.
//!
//! The calorie is the thermochemical calorie, 4.184 J. The report does not
//! say which calorie it uses; the IT calorie (4.1868 J) would be 0.07 %
//! higher, far inside the ±3 %.
//!
//! AI-drafted (Claude Opus 5.5 in Claude Code), untrusted until reviewed.
//! Research, education and V&V only.

use petir::mathf::RealMath;
use uom::si::dynamic_viscosity::centipoise;
use uom::si::f64::*;
use uom::si::mass_density::gram_per_cubic_centimeter;
use uom::si::specific_heat_capacity::joule_per_kilogram_kelvin;
use uom::si::thermal_conductivity::watt_per_meter_kelvin;
use uom::si::thermodynamic_temperature::{degree_celsius, kelvin};

use crate::boussinesq_thermophysical_properties::LiquidMaterial;

/// Lower bound: the L₂B liquidus, 458 °C (TM-2316 p. 2).
pub fn msre_coolant_salt_lower_bound() -> ThermodynamicTemperature {
    ThermodynamicTemperature::new::<degree_celsius>(458.0)
}

/// Upper bound, 800 °C: an assumption (see the module doc).
pub fn msre_coolant_salt_upper_bound() -> ThermodynamicTemperature {
    ThermodynamicTemperature::new::<degree_celsius>(800.0)
}

/// Liquid heat capacity, 0.57 cal/(g·°C) (thermochemical calorie).
pub fn msre_coolant_salt_cp(_temperature: ThermodynamicTemperature) -> SpecificHeatCapacity {
    SpecificHeatCapacity::new::<joule_per_kilogram_kelvin>(0.57 * 4184.0)
}

/// Thermal conductivity, 0.010 W/(cm·°C) = 1.0 W/(m·K), temperature-independent.
pub fn msre_coolant_salt_k(_temperature: ThermodynamicTemperature) -> ThermalConductivity {
    ThermalConductivity::new::<watt_per_meter_kelvin>(1.0)
}

/// Viscosity, `0.116 exp(3755/T)` cP with T in K.
pub fn msre_coolant_salt_mu(temperature: ThermodynamicTemperature) -> DynamicViscosity {
    DynamicViscosity::new::<centipoise>(0.116 * (3755.0 / temperature.get::<kelvin>()).r_exp())
}

/// Density, `2.214 − 4.2e-4 t` g/cm³ with t in °C.
pub fn msre_coolant_salt_rho(temperature: ThermodynamicTemperature) -> MassDensity {
    MassDensity::new::<gram_per_cubic_centimeter>(
        2.214 - 4.2e-4 * temperature.get::<degree_celsius>(),
    )
}

/// The MSRE coolant/flush salt L₂B as a TUAS `CustomLiquid`, 458-800 °C.
/// Use it as `Material::Liquid(msre_coolant_salt_tm2316())`.
pub fn msre_coolant_salt_tm2316() -> LiquidMaterial {
    LiquidMaterial::CustomLiquid(
        (msre_coolant_salt_lower_bound(), msre_coolant_salt_upper_bound()),
        msre_coolant_salt_cp,
        msre_coolant_salt_k,
        msre_coolant_salt_mu,
        msre_coolant_salt_rho,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::boussinesq_thermophysical_properties::density::try_get_rho;
    use crate::boussinesq_thermophysical_properties::dynamic_viscosity::try_get_mu_viscosity;
    use crate::boussinesq_thermophysical_properties::specific_heat_capacity::try_get_cp;
    use crate::boussinesq_thermophysical_properties::thermal_conductivity::try_get_kappa_thermal_conductivity;
    use crate::boussinesq_thermophysical_properties::Material;
    use uom::si::pressure::atmosphere;

    fn l2b() -> Material {
        Material::Liquid(msre_coolant_salt_tm2316())
    }

    fn celsius(t: f64) -> ThermodynamicTemperature {
        ThermodynamicTemperature::new::<degree_celsius>(t)
    }

    /// TH-6 verification: through TUAS's getters, each property equals the
    /// TM-2316 equation evaluated by hand, and out-of-range calls error.
    #[test]
    fn the_getters_reproduce_the_tm2316_equations() {
        let p = Pressure::new::<atmosphere>(1.0);
        for t_c in [458.0, 600.0, 700.0, 800.0] {
            let t = celsius(t_c);
            let t_k = t_c + 273.15;
            approx::assert_relative_eq!(
                try_get_rho(l2b(), t, p).unwrap().get::<gram_per_cubic_centimeter>(),
                2.214 - 4.2e-4 * t_c,
                max_relative = 1e-12
            );
            approx::assert_relative_eq!(
                try_get_mu_viscosity(l2b(), t, p).unwrap().get::<centipoise>(),
                0.116 * (3755.0 / t_k).r_exp(),
                max_relative = 1e-12
            );
            approx::assert_relative_eq!(
                try_get_cp(l2b(), t, p).unwrap().get::<joule_per_kilogram_kelvin>(),
                2384.88,
                max_relative = 1e-12
            );
            approx::assert_relative_eq!(
                try_get_kappa_thermal_conductivity(l2b(), t, p)
                    .unwrap()
                    .get::<watt_per_meter_kelvin>(),
                1.0,
                max_relative = 1e-12
            );
        }
        for t_c in [450.0, 810.0] {
            assert!(try_get_rho(l2b(), celsius(t_c), p).is_err());
            assert!(try_get_mu_viscosity(l2b(), celsius(t_c), p).is_err());
        }
    }

    /// TH-6 / MS-01: TUAS FLiBe (67-33) against L₂B (66-34), 500-700 °C
    /// every 25 °C. Criteria pre-registered on gh:#1133: |FLiBe/L₂B − 1| ≤
    /// the TM-2316 uncertainty (ρ 2 %, μ 15 %, c_p 3 %, k 10 %). ρ, μ and
    /// c_p are gated. k is reported: it was predicted to fail above about
    /// 667 °C, because FLiBe's Sohal conductivity rises with T and
    /// TM-2316's measured L₂B value is constant.
    #[test]
    fn flibe_against_the_msre_coolant_salt() {
        let p = Pressure::new::<atmosphere>(1.0);
        let flibe = Material::Liquid(LiquidMaterial::FLiBe);
        println!("FLiBe/L2B - 1 [%]: t_C, rho, mu, cp, k");
        let mut worst = [0.0_f64; 4];
        for i in 0..=8 {
            let t = celsius(500.0 + 25.0 * i as f64);
            let ratios = [
                try_get_rho(flibe, t, p).unwrap() / try_get_rho(l2b(), t, p).unwrap(),
                try_get_mu_viscosity(flibe, t, p).unwrap()
                    / try_get_mu_viscosity(l2b(), t, p).unwrap(),
                try_get_cp(flibe, t, p).unwrap() / try_get_cp(l2b(), t, p).unwrap(),
                try_get_kappa_thermal_conductivity(flibe, t, p).unwrap()
                    / try_get_kappa_thermal_conductivity(l2b(), t, p).unwrap(),
            ]
            .map(|r| r.value - 1.0);
            println!(
                "FLiBe/L2B: {:5.0} {:+7.3} {:+7.3} {:+7.3} {:+7.3}",
                t.get::<degree_celsius>(),
                100.0 * ratios[0],
                100.0 * ratios[1],
                100.0 * ratios[2],
                100.0 * ratios[3]
            );
            for j in 0..4 {
                if ratios[j].abs() > worst[j].abs() {
                    worst[j] = ratios[j];
                }
            }
        }
        println!(
            "FLiBe/L2B worst [%]: rho {:+.3} (2), mu {:+.3} (15), cp {:+.3} (3), k {:+.3} (10, reported)",
            100.0 * worst[0],
            100.0 * worst[1],
            100.0 * worst[2],
            100.0 * worst[3]
        );
        assert!(worst[0].abs() <= 0.02);
        assert!(worst[1].abs() <= 0.15);
        assert!(worst[2].abs() <= 0.03);
    }
}
