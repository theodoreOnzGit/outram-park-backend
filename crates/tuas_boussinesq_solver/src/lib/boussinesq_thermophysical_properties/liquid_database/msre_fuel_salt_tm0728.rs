//! MSRE U-235 fuel salt, LiF-BeF₂-ZrF₄-UF₄ 65-29.1-5-0.9 mol %, from the
//! MSRE design report's single-temperature values (MSRE TH-6, gh:#1133).
//!
//! # Source
//!
//! R. C. Robertson, *MSRE Design and Operations Report, Part I: Description
//! of Reactor Design*, ORNL-TM-0728 (1965), Table 2.1, "Composition and
//! Physical Properties of the Fuel, Flush, and Coolant Salts" (PDF p. 40,
//! read from the scan on 2026-11-09). The "Partially Enriched Uranium" fuel
//! column, at 1200 °F:
//!
//! | Property | TM-0728 | SI |
//! |---|---|---|
//! | Density | 134 lb/ft³ | 2146.5 kg/m³ |
//! | Viscosity | 20 lb/ft·hr | 8.268e-3 Pa·s |
//! | Heat capacity | 0.47 Btu/lb·°F | 1967.8 J/kg·K |
//! | Thermal conductivity | 3.2 Btu/hr·ft·°F | 5.538 W/m·K |
//! | Liquidus | 840 °F | 722.0 K |
//!
//! TM-0728 cites its own reference 5 for these values. They are design
//! values, not measurements reported in this source, and have no stated
//! uncertainty.
//!
//! # Range
//!
//! TM-0728 gives each property at one temperature only, so it is held
//! constant, and only over the design fuel band: reactor inlet 1175 °F to
//! outlet 1225 °F (TM-0728 Table 5.2, report p. 78, PDF p. 112). Outside
//! that band TUAS's range check returns an error; the value is never
//! extrapolated. Over the 28 K band, a fluoride salt's thermal expansion
//! (of order 2e-4 /K) moves the density by about 0.6 %. That is not
//! represented here.
//!
//! # Status
//!
//! The maintainer chose this source first (2026-11-09: "From ORNL is best,
//! just take it first"). Temperature-dependent ORNL correlations replace it
//! when found. Its conductivity is 5.5 times the 1.0 W/m·K in the correlation
//! set quoted in `docs/reactor-scoping/vtb-findings.md`. The 1965 design value
//! is believed to predate later, lower measurements; the disagreement is
//! recorded on gh:#1133 and not resolved here.
//!
//! AI-drafted (Claude Opus 5.5 in Claude Code), untrusted until reviewed.
//! Research, education and V&V only.

use uom::si::dynamic_viscosity::pound_per_foot_hour;
use uom::si::f64::*;
use uom::si::mass_density::pound_per_cubic_foot;
use uom::si::specific_heat_capacity::btu_it_per_pound_degree_fahrenheit;
use uom::si::thermal_conductivity::watt_per_meter_kelvin;
use uom::si::thermodynamic_temperature::degree_fahrenheit;

use crate::boussinesq_thermophysical_properties::LiquidMaterial;

/// 1 Btu_IT/(h·ft·°F) in W/(m·K): 1055.05585262 J / (3600 s · 0.3048 m ·
/// 5/9 K). `uom` has no British conductivity unit.
pub const BTU_PER_HOUR_FOOT_FAHRENHEIT_IN_W_PER_M_K: f64 =
    1055.05585262 / (3600.0 * 0.3048 * (5.0 / 9.0));

/// Lower end of the design fuel band: reactor inlet, 1175 °F (TM-0728 Table 5.2).
pub fn msre_fuel_salt_lower_bound() -> ThermodynamicTemperature {
    ThermodynamicTemperature::new::<degree_fahrenheit>(1175.0)
}

/// Upper end of the design fuel band: reactor outlet, 1225 °F (TM-0728 Table 5.2).
pub fn msre_fuel_salt_upper_bound() -> ThermodynamicTemperature {
    ThermodynamicTemperature::new::<degree_fahrenheit>(1225.0)
}

/// Heat capacity, 0.47 Btu/lb·°F, held constant over the band.
pub fn msre_fuel_salt_cp(_temperature: ThermodynamicTemperature) -> SpecificHeatCapacity {
    SpecificHeatCapacity::new::<btu_it_per_pound_degree_fahrenheit>(0.47)
}

/// Thermal conductivity, 3.2 Btu/hr·ft·°F, held constant over the band.
pub fn msre_fuel_salt_k(_temperature: ThermodynamicTemperature) -> ThermalConductivity {
    ThermalConductivity::new::<watt_per_meter_kelvin>(
        3.2 * BTU_PER_HOUR_FOOT_FAHRENHEIT_IN_W_PER_M_K,
    )
}

/// Dynamic viscosity, 20 lb/ft·hr, held constant over the band.
pub fn msre_fuel_salt_mu(_temperature: ThermodynamicTemperature) -> DynamicViscosity {
    DynamicViscosity::new::<pound_per_foot_hour>(20.0)
}

/// Density, 134 lb/ft³, held constant over the band.
pub fn msre_fuel_salt_rho(_temperature: ThermodynamicTemperature) -> MassDensity {
    MassDensity::new::<pound_per_cubic_foot>(134.0)
}

/// The MSRE U-235 fuel salt as a TUAS `CustomLiquid`, valid from 1175 °F to
/// 1225 °F. Use it as `Material::Liquid(msre_fuel_salt_u235_tm0728())`.
pub fn msre_fuel_salt_u235_tm0728() -> LiquidMaterial {
    LiquidMaterial::CustomLiquid(
        (msre_fuel_salt_lower_bound(), msre_fuel_salt_upper_bound()),
        msre_fuel_salt_cp,
        msre_fuel_salt_k,
        msre_fuel_salt_mu,
        msre_fuel_salt_rho,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::boussinesq_thermophysical_properties::density::try_get_rho;
    use crate::boussinesq_thermophysical_properties::dynamic_viscosity::try_get_mu_viscosity;
    use crate::boussinesq_thermophysical_properties::prandtl::try_get_prandtl;
    use crate::boussinesq_thermophysical_properties::specific_heat_capacity::try_get_cp;
    use crate::boussinesq_thermophysical_properties::specific_enthalpy::try_get_h;
    use crate::boussinesq_thermophysical_properties::thermal_conductivity::try_get_kappa_thermal_conductivity;
    use crate::boussinesq_thermophysical_properties::Material;
    use uom::si::dynamic_viscosity::pascal_second;
    use uom::si::mass_density::kilogram_per_cubic_meter;
    use uom::si::pressure::atmosphere;
    use uom::si::specific_heat_capacity::joule_per_kilogram_kelvin;
    use uom::si::thermodynamic_temperature::kelvin;

    fn salt() -> Material {
        Material::Liquid(msre_fuel_salt_u235_tm0728())
    }

    fn at_fahrenheit(t: f64) -> ThermodynamicTemperature {
        ThermodynamicTemperature::new::<degree_fahrenheit>(t)
    }

    /// TH-6 verification: through TUAS's ordinary getters, at 1200 °F, every
    /// property converts back to TM-0728 Table 2.1's printed value at its
    /// printed precision, and the SI values are the ones in the module doc.
    #[test]
    fn the_getters_return_table_2_1_at_its_printed_precision() {
        let p = Pressure::new::<atmosphere>(1.0);
        let t = at_fahrenheit(1200.0);
        let rho = try_get_rho(salt(), t, p).unwrap();
        let mu = try_get_mu_viscosity(salt(), t, p).unwrap();
        let cp = try_get_cp(salt(), t, p).unwrap();
        let k = try_get_kappa_thermal_conductivity(salt(), t, p).unwrap();
        let pr = try_get_prandtl(salt(), t, p).unwrap();
        println!(
            "TH-6 TM-0728 at 1200 F ({:.2} K): rho {:.2} kg/m3, mu {:.4e} Pa s, cp {:.1} J/kg K, k {:.4} W/m K, Pr {:.3}",
            t.get::<kelvin>(),
            rho.get::<kilogram_per_cubic_meter>(),
            mu.get::<pascal_second>(),
            cp.get::<joule_per_kilogram_kelvin>(),
            k.get::<watt_per_meter_kelvin>(),
            pr.value
        );
        assert_eq!(rho.get::<pound_per_cubic_foot>().round(), 134.0);
        assert_eq!(mu.get::<pound_per_foot_hour>().round(), 20.0);
        assert_eq!((cp.get::<btu_it_per_pound_degree_fahrenheit>() * 100.0).round(), 47.0);
        assert_eq!(
            (k.get::<watt_per_meter_kelvin>() / BTU_PER_HOUR_FOOT_FAHRENHEIT_IN_W_PER_M_K * 10.0)
                .round(),
            32.0
        );
        approx::assert_relative_eq!(rho.get::<kilogram_per_cubic_meter>(), 2146.47, max_relative = 1e-4);
        approx::assert_relative_eq!(mu.get::<pascal_second>(), 8.2676e-3, max_relative = 1e-4);
        approx::assert_relative_eq!(cp.get::<joule_per_kilogram_kelvin>(), 1967.8, max_relative = 1e-4);
        approx::assert_relative_eq!(k.get::<watt_per_meter_kelvin>(), 5.5384, max_relative = 1e-4);
    }

    /// Outside the design band (1175-1225 °F) every getter returns an
    /// error: TM-0728's single-temperature values are never extrapolated.
    #[test]
    fn outside_the_design_band_the_getters_refuse() {
        let p = Pressure::new::<atmosphere>(1.0);
        for t in [1170.0, 1230.0] {
            let t = at_fahrenheit(t);
            assert!(try_get_rho(salt(), t, p).is_err());
            assert!(try_get_mu_viscosity(salt(), t, p).is_err());
            assert!(try_get_cp(salt(), t, p).is_err());
            assert!(try_get_kappa_thermal_conductivity(salt(), t, p).is_err());
        }
        for t in [1175.0, 1225.0] {
            assert!(try_get_rho(salt(), at_fahrenheit(t), p).is_ok());
        }
    }

    /// The enthalpy TUAS integrates from cp rises by cp ΔT across the band
    /// (constant cp, so exactly linear), checked to 1e-6.
    #[test]
    fn enthalpy_rises_by_cp_times_the_band() {
        let p = Pressure::new::<atmosphere>(1.0);
        let h_in = try_get_h(salt(), msre_fuel_salt_lower_bound(), p).unwrap();
        let h_out = try_get_h(salt(), msre_fuel_salt_upper_bound(), p).unwrap();
        let dt = msre_fuel_salt_upper_bound().get::<kelvin>() - msre_fuel_salt_lower_bound().get::<kelvin>();
        let expected = msre_fuel_salt_cp(msre_fuel_salt_lower_bound()).get::<joule_per_kilogram_kelvin>() * dt;
        approx::assert_relative_eq!((h_out - h_in).value, expected, max_relative = 1e-6);
    }
}
