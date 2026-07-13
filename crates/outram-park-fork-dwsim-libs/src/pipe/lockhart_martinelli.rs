//! Lockhart-Martinelli (1949) two-phase gas-liquid pipe flow correlation.
//!
//! Ported from DWSIM `FluidFlowCorrelations/LockhartMartinelli.vb`. DWSIM's
//! implementation is a separated-flow / two-phase-multiplier model (each
//! phase's single-phase pressure drop, corrected by a multiplier derived
//! from the Martinelli parameter `X`), not the classic void-fraction chart.

use super::friction_factor::{darcy_friction_factor, frictional_pressure_drop, reynolds_number};
use uom::si::acceleration::meter_per_second_squared;
use uom::si::angle::radian;
use uom::si::f64::{
    Acceleration, Angle, DynamicViscosity, Length, MassDensity, Pressure, Ratio, Velocity,
    VolumeRate,
};
use uom::si::length::meter;
use uom::si::pressure::pascal;
use uom::si::ratio::ratio;
use uom::si::velocity::meter_per_second;
use uom::si::volume_rate::cubic_meter_per_second;

/// Standard gravitational acceleration, m/s^2.
const G: f64 = 9.80665;

/// Result of a Lockhart-Martinelli two-phase pressure-drop evaluation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LockhartMartinelliResult {
    /// Martinelli parameter `X = sqrt(dP_SL / dP_SG)` \[-\].
    pub martinelli_parameter: Ratio,
    /// Liquid holdup estimate `1 / sqrt(1 + 20/X + 1/X^2)` \[-\].
    pub liquid_holdup: Ratio,
    /// Frictional pressure drop: `max(phi_L^2 dP_SL, phi_G^2 dP_SG)`.
    pub friction_pressure_drop: Pressure,
    /// Elevation (hydrostatic) pressure drop.
    pub elevation_pressure_drop: Pressure,
}

impl LockhartMartinelliResult {
    /// Total pressure drop, friction plus elevation.
    pub fn total_pressure_drop(&self) -> Pressure {
        self.friction_pressure_drop + self.elevation_pressure_drop
    }
}

/// Evaluate the Lockhart-Martinelli (1949) two-phase pressure drop over a
/// pipe segment, given liquid and gas volumetric flow rates, densities,
/// viscosities, and the pipe's inclination angle from horizontal (positive =
/// uphill).
#[allow(clippy::too_many_arguments)]
pub fn lockhart_martinelli_pressure_drop(
    length: Length,
    diameter: Length,
    roughness: Length,
    inclination: Angle,
    q_liquid: VolumeRate,
    q_gas: VolumeRate,
    density_liquid: MassDensity,
    density_gas: MassDensity,
    viscosity_liquid: DynamicViscosity,
    viscosity_gas: DynamicViscosity,
) -> LockhartMartinelliResult {
    let d = diameter.get::<meter>();
    let area = std::f64::consts::PI / 4.0 * d * d;
    let v_sl = Velocity::new::<meter_per_second>(q_liquid.get::<cubic_meter_per_second>() / area);
    let v_sg = Velocity::new::<meter_per_second>(q_gas.get::<cubic_meter_per_second>() / area);

    let re_sl = reynolds_number(density_liquid, v_sl, diameter, viscosity_liquid);
    let re_sg = reynolds_number(density_gas, v_sg, diameter, viscosity_gas);
    let f_sl = darcy_friction_factor(re_sl, diameter, roughness);
    let f_sg = darcy_friction_factor(re_sg, diameter, roughness);

    let dp_sl = frictional_pressure_drop(f_sl, length, diameter, density_liquid, v_sl);
    let dp_sg = frictional_pressure_drop(f_sg, length, diameter, density_gas, v_sg);

    let x = (dp_sl.get::<pascal>() / dp_sg.get::<pascal>())
        .sqrt();
    let phi_l_sq = 1.0 + 20.0 / x + 1.0 / (x * x);
    let phi_g_sq = 1.0 + 20.0 * x + x * x;

    let friction_pressure_drop = Pressure::new::<pascal>(
        (phi_l_sq * dp_sl.get::<pascal>())
            .max(phi_g_sq * dp_sg.get::<pascal>()),
    );

    let holdup = 1.0 / (1.0 + 20.0 / x + 1.0 / (x * x)).sqrt();
    let theta = inclination.get::<radian>();
    let c_l = q_liquid.get::<cubic_meter_per_second>()
        / (q_liquid.get::<cubic_meter_per_second>() + q_gas.get::<cubic_meter_per_second>());
    let elevation_pressure_drop = (density_gas * (1.0 - c_l) + density_liquid * c_l)
        * Acceleration::new::<meter_per_second_squared>(G)
        * theta.sin()
        * length;

    LockhartMartinelliResult {
        martinelli_parameter: Ratio::new::<ratio>(x),
        liquid_holdup: Ratio::new::<ratio>(holdup),
        friction_pressure_drop,
        elevation_pressure_drop,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uom::si::angle::radian;
    use uom::si::dynamic_viscosity::pascal_second;
    use uom::si::length::meter;
    use uom::si::mass_density::kilogram_per_cubic_meter;
    use uom::si::volume_rate::cubic_meter_per_second;

    #[test]
    fn martinelli_parameter_large_for_liquid_dominated_flow() {
        let result = lockhart_martinelli_pressure_drop(
            Length::new::<meter>(100.0),
            Length::new::<meter>(0.1),
            Length::new::<meter>(4.6e-5),
            Angle::new::<radian>(0.0),
            VolumeRate::new::<cubic_meter_per_second>(0.005),
            VolumeRate::new::<cubic_meter_per_second>(0.0005),
            MassDensity::new::<kilogram_per_cubic_meter>(998.0),
            MassDensity::new::<kilogram_per_cubic_meter>(1.2),
            DynamicViscosity::new::<pascal_second>(1.0e-3),
            DynamicViscosity::new::<pascal_second>(1.8e-5),
        );
        assert!(result.martinelli_parameter.get::<ratio>() > 1.0);
        assert!(result.liquid_holdup.get::<ratio>() > 0.0 && result.liquid_holdup.get::<ratio>() < 1.0);
        assert!(result.total_pressure_drop().get::<pascal>() > 0.0);
    }

    #[test]
    fn horizontal_pipe_has_zero_elevation_drop() {
        let result = lockhart_martinelli_pressure_drop(
            Length::new::<meter>(100.0),
            Length::new::<meter>(0.1),
            Length::new::<meter>(4.6e-5),
            Angle::new::<radian>(0.0),
            VolumeRate::new::<cubic_meter_per_second>(0.005),
            VolumeRate::new::<cubic_meter_per_second>(0.0005),
            MassDensity::new::<kilogram_per_cubic_meter>(998.0),
            MassDensity::new::<kilogram_per_cubic_meter>(1.2),
            DynamicViscosity::new::<pascal_second>(1.0e-3),
            DynamicViscosity::new::<pascal_second>(1.8e-5),
        );
        assert!(result.elevation_pressure_drop.get::<pascal>().abs() < 1e-6);
    }
}
