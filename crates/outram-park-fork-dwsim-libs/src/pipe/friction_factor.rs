//! Single-phase Darcy friction factor and pressure drop.
//!
//! Ported from DWSIM `FluidFlowCorrelations/FlowPackageBaseClass.vb`
//! (`FrictionFactor`, `CalculateDeltaPLiquid`/`CalculateDeltaPGas`). DWSIM
//! carries two near-identical explicit Colebrook fits (the other is inlined
//! in `Pipe.vb`'s `CalcOverallHeatTransferCoefficient`); this is the single
//! consolidated Rust version.

use uom::si::f64::{DynamicViscosity, Length, MassDensity, Pressure, Ratio, Velocity};
use uom::si::ratio::ratio;

/// Reynolds number `Re = ρ v D / μ` \[-\] for flow in a pipe of diameter `D`.
pub fn reynolds_number(
    density: MassDensity,
    velocity: Velocity,
    diameter: Length,
    viscosity: DynamicViscosity,
) -> Ratio {
    let re = (density * velocity * diameter / viscosity).value;
    Ratio::new::<ratio>(re)
}

/// Darcy friction factor `f` \[-\] for flow in a pipe of diameter `D` and
/// absolute roughness `k`, from an explicit approximation to the Colebrook
/// equation (turbulent branch) with laminar (`Re < 2100`) and transitional
/// (`2100 <= Re <= 4000`) closures either side.
///
/// Ported from DWSIM `FlowPackageBaseClass.FrictionFactor`:
/// ```text
/// Turbulent (Re>4000):  a1 = log10( (k/D)^1.1096/2.8257 + (5.8506/Re)^0.8961 )
///                       b1 = -2*log10( (k/D)/3.7065 - 5.0452*a1/Re )
///                       f  = (1/b1)^2
/// Laminar (Re<2100):    f = 64/Re
/// Transitional:          f = 8*[ (8/Re)^12 + 1/(b+c)^1.5 ]^(1/12)
///                        b = (2.457*ln(1/((7/Re)^0.9+0.27*k/D)))^16
///                        c = (37530/Re)^16
/// ```
pub fn darcy_friction_factor(reynolds: Ratio, diameter: Length, roughness: Length) -> Ratio {
    let re = reynolds.get::<ratio>();
    let rel_roughness = (roughness / diameter).get::<ratio>();

    let f = if re < 2100.0 {
        64.0 / re
    } else if re > 4000.0 {
        let a1 = ((rel_roughness.powf(1.1096)) / 2.8257 + (5.8506 / re).powf(0.8961)).log10();
        let b1 = -2.0 * (rel_roughness / 3.7065 - 5.0452 * a1 / re).log10();
        (1.0 / b1).powi(2)
    } else {
        // Transitional (2100 <= Re <= 4000): blended explicit fit.
        let b = (2.457 * (1.0 / ((7.0 / re).powf(0.9) + 0.27 * rel_roughness)).ln()).powi(16);
        let c = (37530.0 / re).powi(16);
        8.0 * ((8.0 / re).powi(12) + 1.0 / (b + c).powf(1.5)).powf(1.0 / 12.0)
    };
    Ratio::new::<ratio>(f)
}

/// Frictional pressure drop `ΔP_f = f (L/D) (ρ v^2 / 2)` for single-phase
/// flow through a straight pipe run of length `L`.
pub fn frictional_pressure_drop(
    friction_factor: Ratio,
    length: Length,
    diameter: Length,
    density: MassDensity,
    velocity: Velocity,
) -> Pressure {
    friction_factor.get::<ratio>() * (length / diameter).get::<ratio>() * density * velocity
        * velocity
        / 2.0
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_relative_eq;
    use uom::si::dynamic_viscosity::pascal_second;
    use uom::si::length::meter;
    use uom::si::mass_density::kilogram_per_cubic_meter;
    use uom::si::pressure::pascal;
    use uom::si::velocity::meter_per_second;

    #[test]
    fn laminar_friction_factor_is_64_over_re() {
        let re = Ratio::new::<ratio>(1000.0);
        let d = Length::new::<meter>(0.05);
        let k = Length::new::<meter>(1.5e-5);
        let f = darcy_friction_factor(re, d, k);
        assert_relative_eq!(f.get::<ratio>(), 64.0 / 1000.0, max_relative = 1e-9);
    }

    #[test]
    fn reynolds_number_water_in_pipe() {
        // Water at ~20 C: rho=998 kg/m3, mu=1.0e-3 Pa.s, v=1 m/s, D=0.1 m
        // Re = 998*1*0.1/1e-3 = 99800
        let rho = MassDensity::new::<kilogram_per_cubic_meter>(998.0);
        let v = Velocity::new::<meter_per_second>(1.0);
        let d = Length::new::<meter>(0.1);
        let mu = DynamicViscosity::new::<pascal_second>(1.0e-3);
        let re = reynolds_number(rho, v, d, mu);
        assert_relative_eq!(re.get::<ratio>(), 99800.0, max_relative = 1e-6);
    }

    #[test]
    fn turbulent_pressure_drop_is_positive_and_scales_with_length() {
        let rho = MassDensity::new::<kilogram_per_cubic_meter>(998.0);
        let v = Velocity::new::<meter_per_second>(2.0);
        let d = Length::new::<meter>(0.1);
        let mu = DynamicViscosity::new::<pascal_second>(1.0e-3);
        let k = Length::new::<meter>(4.6e-5); // commercial steel
        let re = reynolds_number(rho, v, d, mu);
        assert!(re.get::<ratio>() > 4000.0);
        let f = darcy_friction_factor(re, d, k);
        let dp10 = frictional_pressure_drop(f, Length::new::<meter>(10.0), d, rho, v);
        let dp20 = frictional_pressure_drop(f, Length::new::<meter>(20.0), d, rho, v);
        assert!(dp10.get::<pascal>() > 0.0);
        assert_relative_eq!(dp20.get::<pascal>(), 2.0 * dp10.get::<pascal>(), max_relative = 1e-9);
    }
}
