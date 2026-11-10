//! Liquid sodium as a TUAS `CustomLiquid`, evaluated with CoolProp's `LiqNa`
//! incompressible fit (gh:#1320; EBR-II groundwork, epic gh:#1312).
//!
//! # Source
//!
//! No new correlation is written here. Every property is CoolProp's `LiqNa`
//! fit, through this workspace's port
//! [`outram_park_fork_coolprop::incompressibles::Incompressible::LiqNa`]
//! (generated from CoolProp's `dev/incompressible_liquids/json/LiqNa.json`,
//! CoolProp commit `0e67fe74b30a`, MIT):
//!
//! | Property | Form | Fitted to |
//! |---|---|---|
//! | density | cubic in `T − 1450 K` | 22 points, 400-2500 K |
//! | heat capacity | cubic in `T − 1450 K` | **7 points, 400-1000 K only** |
//! | thermal conductivity | cubic in `T − 1450 K` | 22 points, 400-2500 K |
//! | viscosity | `exp(c0/(T + c1) − c2)` | 21 points, 400-2400 K |
//!
//! The fitted data are CoolProp's own table, class `LiquidSodium` in
//! `dev/incompressible_liquids/CPIncomp/PureFluids.py` (same commit). Upstream
//! records no literature source for that table (`reference = "LiqNa"`); the
//! primary source is therefore **Not re-checked**.
//!
//! # Range: 400-1000 K, and why not CoolProp's 400-2500 K
//!
//! CoolProp declares `LiqNa` valid over 400-2500 K, but its heat-capacity
//! table stops at 1000 K (the entries above are `NaN`). Above 1000 K the
//! heat capacity is a cubic **extrapolated** 1500 K past its last data point
//! (it reaches 2658 J/(kg·K) at 2500 K, more than double the 1000 K value).
//! TUAS integrates the heat capacity for enthalpy, so the enthalpy and every
//! temperature recovered from it would rest on that extrapolation. The range
//! here is therefore the span on which all four properties have source data:
//! **400 K to 1000 K**. 400 K is also CoolProp's own lower bound (sodium
//! melts near 371 K); 1000 K is below sodium's normal boiling point (about
//! 1156 K), so a single-phase liquid at about atmospheric pressure is inside
//! it. Outside the range TUAS's getters return an error.
//!
//! # Enthalpy reference
//!
//! TUAS's `CustomLiquid` enthalpy is `∫ c_p dT` from the lower bound, so
//! `h(400 K) = 0`. CoolProp's port references `h = 0` at `T_base = 1450 K`.
//! Differences agree (both integrate the same cubic; TUAS's 20-point
//! Gauss-Legendre rule is exact for it), absolute values do not.
//!
//! # V&V (2026-11-10)
//!
//! Methodology, results and the measured fit residuals are in the test
//! module below and in
//! `crates/tuas_boussinesq_solver/docs/liquid_sodium_coolprop_vv.md`.
//!
//! AI-drafted (Claude Opus 5.5 in Claude Code), untrusted until reviewed.
//! Research, education and V&V only.

use outram_park_fork_coolprop::incompressibles::Incompressible;
use uom::si::dynamic_viscosity::pascal_second;
use uom::si::f64::*;
use uom::si::mass_density::kilogram_per_cubic_meter;
use uom::si::specific_heat_capacity::joule_per_kilogram_kelvin;
use uom::si::thermal_conductivity::watt_per_meter_kelvin;
use uom::si::thermodynamic_temperature::kelvin;

use crate::boussinesq_thermophysical_properties::LiquidMaterial;

/// Lower bound, 400 K: CoolProp `LiqNa`'s own lower bound.
pub fn liquid_sodium_lower_bound() -> ThermodynamicTemperature {
    ThermodynamicTemperature::new::<kelvin>(400.0)
}

/// Upper bound, 1000 K: the last temperature with heat-capacity data in
/// CoolProp's source table (see the module doc).
pub fn liquid_sodium_upper_bound() -> ThermodynamicTemperature {
    ThermodynamicTemperature::new::<kelvin>(1000.0)
}

/// Liquid sodium as a TUAS liquid, 400-1000 K, properties from CoolProp's
/// `LiqNa`. Use it as `Material::Liquid(liquid_sodium())`.
pub fn liquid_sodium() -> LiquidMaterial {
    LiquidMaterial::CustomLiquid(
        (liquid_sodium_lower_bound(), liquid_sodium_upper_bound()),
        liquid_sodium_cp,
        liquid_sodium_k,
        liquid_sodium_mu,
        liquid_sodium_rho,
    )
}

// The four callbacks below are reached only through `liquid_sodium()`'s
// `CustomLiquid`, whose getters range-check against 400-1000 K first; that
// window lies inside CoolProp's 400-2500 K, so `LiqNa` cannot return an
// error there. A caller who copies these function pointers into a
// `CustomLiquid` with wider bounds gets NaN outside 400-2500 K rather than a
// panic, which TUAS's own NaN checks then surface.

/// `c_p`, CoolProp `LiqNa`.
fn liquid_sodium_cp(temperature: ThermodynamicTemperature) -> SpecificHeatCapacity {
    let value = Incompressible::LiqNa
        .heat_capacity(temperature.get::<kelvin>(), 0.0, 0.0)
        .unwrap_or(f64::NAN);
    SpecificHeatCapacity::new::<joule_per_kilogram_kelvin>(value)
}

/// Thermal conductivity, CoolProp `LiqNa`.
fn liquid_sodium_k(temperature: ThermodynamicTemperature) -> ThermalConductivity {
    let value = Incompressible::LiqNa
        .conductivity(temperature.get::<kelvin>(), 0.0, 0.0)
        .unwrap_or(f64::NAN);
    ThermalConductivity::new::<watt_per_meter_kelvin>(value)
}

/// Dynamic viscosity, CoolProp `LiqNa`.
fn liquid_sodium_mu(temperature: ThermodynamicTemperature) -> DynamicViscosity {
    let value = Incompressible::LiqNa
        .viscosity(temperature.get::<kelvin>(), 0.0, 0.0)
        .unwrap_or(f64::NAN);
    DynamicViscosity::new::<pascal_second>(value)
}

/// Density, CoolProp `LiqNa`.
fn liquid_sodium_rho(temperature: ThermodynamicTemperature) -> MassDensity {
    let value = Incompressible::LiqNa
        .density(temperature.get::<kelvin>(), 0.0, 0.0)
        .unwrap_or(f64::NAN);
    MassDensity::new::<kilogram_per_cubic_meter>(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::boussinesq_thermophysical_properties::density::try_get_rho;
    use crate::boussinesq_thermophysical_properties::dynamic_viscosity::try_get_mu_viscosity;
    use crate::boussinesq_thermophysical_properties::prandtl::try_get_prandtl;
    use crate::boussinesq_thermophysical_properties::specific_enthalpy::try_get_h;
    use crate::boussinesq_thermophysical_properties::specific_heat_capacity::try_get_cp;
    use crate::boussinesq_thermophysical_properties::thermal_conductivity::try_get_kappa_thermal_conductivity;
    use crate::boussinesq_thermophysical_properties::Material;
    use uom::si::available_energy::joule_per_kilogram;
    use uom::si::pressure::atmosphere;
    use uom::si::ratio::ratio;

    fn sodium() -> Material {
        Material::Liquid(liquid_sodium())
    }

    fn kelvin_t(t: f64) -> ThermodynamicTemperature {
        ThermodynamicTemperature::new::<kelvin>(t)
    }

    fn one_atm() -> Pressure {
        Pressure::new::<atmosphere>(1.0)
    }

    /// Verification: through TUAS's getters, each property is CoolProp
    /// `LiqNa` evaluated directly, bit for bit (the same function is called;
    /// this pins the wiring and the units), at 400, 550, 700, 850 and 1000 K.
    #[test]
    fn tuas_getters_return_coolprop_liqna_bit_for_bit() {
        let p = one_atm();
        for t_k in [400.0, 550.0, 700.0, 850.0, 1000.0] {
            let t = kelvin_t(t_k);
            let na = Incompressible::LiqNa;
            assert_eq!(
                try_get_rho(sodium(), t, p).unwrap().get::<kilogram_per_cubic_meter>(),
                na.density(t_k, 0.0, 0.0).unwrap()
            );
            assert_eq!(
                try_get_cp(sodium(), t, p).unwrap().get::<joule_per_kilogram_kelvin>(),
                na.heat_capacity(t_k, 0.0, 0.0).unwrap()
            );
            assert_eq!(
                try_get_kappa_thermal_conductivity(sodium(), t, p)
                    .unwrap()
                    .get::<watt_per_meter_kelvin>(),
                na.conductivity(t_k, 0.0, 0.0).unwrap()
            );
            assert_eq!(
                try_get_mu_viscosity(sodium(), t, p).unwrap().get::<pascal_second>(),
                na.viscosity(t_k, 0.0, 0.0).unwrap()
            );
        }
    }

    /// The range is 400-1000 K: just outside either end every getter errors.
    #[test]
    fn sodium_outside_400_to_1000_kelvin_is_an_error() {
        let p = one_atm();
        for t_k in [399.0, 1001.0] {
            let t = kelvin_t(t_k);
            assert!(try_get_rho(sodium(), t, p).is_err());
            assert!(try_get_cp(sodium(), t, p).is_err());
            assert!(try_get_kappa_thermal_conductivity(sodium(), t, p).is_err());
            assert!(try_get_mu_viscosity(sodium(), t, p).is_err());
        }
    }

    /// Verification of the enthalpy route: TUAS integrates `c_p` from 400 K;
    /// the CoolProp port integrates the same cubic analytically from 1450 K.
    /// Differences must agree to round-off (20-point Gauss-Legendre is exact
    /// for a cubic). Prediction: relative difference below 1e-12.
    /// Result 2026-11-10: below 1e-12 at every point (asserted).
    #[test]
    fn tuas_enthalpy_differences_match_coolprop_integral() {
        let p = one_atm();
        let h_tuas_400 = try_get_h(sodium(), kelvin_t(400.0), p).unwrap();
        approx::assert_abs_diff_eq!(h_tuas_400.get::<joule_per_kilogram>(), 0.0, epsilon = 1e-9);
        let h_cp_400 = Incompressible::LiqNa.enthalpy(400.0, 0.0, 0.0).unwrap();
        for t_k in [500.0, 644.0, 800.0, 1000.0] {
            let dh_tuas = try_get_h(sodium(), kelvin_t(t_k), p).unwrap().get::<joule_per_kilogram>();
            let dh_coolprop = Incompressible::LiqNa.enthalpy(t_k, 0.0, 0.0).unwrap() - h_cp_400;
            approx::assert_relative_eq!(dh_tuas, dh_coolprop, max_relative = 1e-12);
        }
    }

    /// Validation of the fit against CoolProp's own source table (class
    /// `LiquidSodium`, `CPIncomp/PureFluids.py`, commit `0e67fe74b30a`),
    /// 400-1000 K, the range used here. The table values are copied below.
    ///
    /// Measured 2026-11-10 (fit/table − 1): density +0.874 % at 400 K, worst
    /// −0.675 % elsewhere; heat capacity within ±0.043 %; conductivity within
    /// ±0.006 %; viscosity −2.003 % at 400 K, +1.756 % at 600 K. These are
    /// CoolProp's own fit residuals, not a defect of the port.
    ///
    /// **The bounds asserted (1 %, 0.1 %, 0.01 %, 2.1 %) were set after that
    /// measurement**, as a regression pin on the residuals, not as an
    /// acceptance criterion chosen in advance: no acceptance band has been
    /// set for sodium properties.
    #[test]
    fn coolprop_liqna_fit_against_its_source_table() {
        let p = one_atm();
        let table: [(f64, f64, f64, f64, f64); 7] = [
            // T [K], rho [kg/m3], cp [J/(kg K)], k [W/(m K)], mu [Pa s]
            (400.0, 919.0, 1372.0, 87.22, 5.99e-4),
            (500.0, 897.0, 1334.0, 80.09, 4.15e-4),
            (600.0, 874.0, 1301.0, 73.70, 3.21e-4),
            (700.0, 852.0, 1277.0, 68.00, 2.64e-4),
            (800.0, 828.0, 1260.0, 62.90, 2.27e-4),
            (900.0, 805.0, 1252.0, 58.34, 2.01e-4),
            (1000.0, 781.0, 1252.0, 54.24, 1.81e-4),
        ];
        let mut worst = [0.0_f64; 4];
        for (t_k, rho, cp, k, mu) in table {
            let t = kelvin_t(t_k);
            let r = [
                try_get_rho(sodium(), t, p).unwrap().get::<kilogram_per_cubic_meter>() / rho,
                try_get_cp(sodium(), t, p).unwrap().get::<joule_per_kilogram_kelvin>() / cp,
                try_get_kappa_thermal_conductivity(sodium(), t, p)
                    .unwrap()
                    .get::<watt_per_meter_kelvin>()
                    / k,
                try_get_mu_viscosity(sodium(), t, p).unwrap().get::<pascal_second>() / mu,
            ]
            .map(|x| x - 1.0);
            println!(
                "LiqNa fit/table - 1 [%] at {t_k:6.1} K: rho {:+.3} cp {:+.3} k {:+.4} mu {:+.3}",
                100.0 * r[0],
                100.0 * r[1],
                100.0 * r[2],
                100.0 * r[3]
            );
            for j in 0..4 {
                if r[j].abs() > worst[j].abs() {
                    worst[j] = r[j];
                }
            }
        }
        assert!(worst[0].abs() <= 0.01, "rho {worst:?}");
        assert!(worst[1].abs() <= 0.001, "cp {worst:?}");
        assert!(worst[2].abs() <= 0.0001, "k {worst:?}");
        assert!(worst[3].abs() <= 0.021, "mu {worst:?}");
    }

    /// Sanity, not a gate on a reference: the Prandtl number of sodium is of
    /// order 5e-3 (the liquid-metal regime the correlations in
    /// `nusselt_number_correlations::liquid_metal` are for). At 700 K,
    /// `c_p mu / k` from the table above is 1277 * 2.64e-4 / 68.0 = 4.96e-3.
    #[test]
    fn sodium_prandtl_number_is_of_order_five_thousandths() {
        let pr = try_get_prandtl(sodium(), kelvin_t(700.0), one_atm()).unwrap();
        let pr = pr.get::<ratio>();
        println!("sodium Pr(700 K) = {pr:.5e}");
        assert!(pr > 4.0e-3 && pr < 6.0e-3, "Pr = {pr}");
    }
}
