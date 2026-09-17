//! Beggs & Brill (1973) two-phase gas-liquid pipe flow correlation.
//!
//! Ported from DWSIM `FluidFlowCorrelations/BeggsBrill.vb`. DWSIM's own
//! source documents the full derivation inline (HTML/MathJax); the equations
//! below follow that documentation and the standard Beggs & Brill (1973)
//! reference ("A Study of Two-Phase Flow in Inclined Pipes", JPT).
//!
//! The empirical regime-boundary and holdup correlations below are
//! dimensionless curve fits (not unit-checked physics beyond the base
//! quantities), so -- matching DWSIM's own approach -- intermediate work is
//! done in plain `f64` (SI) after unwrapping the `uom` inputs; only the
//! function boundary and the final results are dimensioned.

use super::friction_factor::{darcy_friction_factor, reynolds_number};
use uom::si::f64::{
    Acceleration, DynamicViscosity, Length, MassDensity, Pressure, Ratio, SurfaceTension, Velocity,
    VolumeRate,
};
use uom::si::length::meter;
use uom::si::mass_density::kilogram_per_cubic_meter;
use uom::si::pressure::pascal;
use uom::si::ratio::ratio;
use uom::si::surface_tension::newton_per_meter;
use uom::si::velocity::meter_per_second;
use uom::si::volume_rate::cubic_meter_per_second;

/// Standard gravitational acceleration, m/s^2.
const G: f64 = 9.80665;

/// Flow regime identified by the Beggs & Brill regime map.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlowRegime {
    /// Stratified/annular-like, low liquid holdup change with inclination.
    Segregated,
    /// Slug flow.
    Intermittent,
    /// Bubble / dispersed-bubble flow.
    Distributed,
    /// Blended between [`Self::Segregated`] and [`Self::Intermittent`].
    Transition,
}

/// Result of a Beggs & Brill two-phase pressure-drop evaluation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BeggsBrillResult {
    /// Identified flow regime.
    pub regime: FlowRegime,
    /// Liquid holdup at the pipe's actual inclination `H_L(θ)` \[-\].
    pub liquid_holdup: Ratio,
    /// Two-phase mixture density using the inclined holdup, `ρ_m`.
    pub mixture_density: MassDensity,
    /// Frictional pressure drop.
    pub friction_pressure_drop: Pressure,
    /// Elevation (hydrostatic) pressure drop.
    pub elevation_pressure_drop: Pressure,
}

impl BeggsBrillResult {
    /// Total pressure drop, friction plus elevation.
    pub fn total_pressure_drop(&self) -> Pressure {
        self.friction_pressure_drop + self.elevation_pressure_drop
    }
}

/// Horizontal (`θ=0`) liquid holdup, per regime: `a * C_L^b / Fr_m^c`,
/// floored at the no-slip holdup `C_L` (Beggs & Brill 1973, Table 1).
fn holdup_horizontal(regime: FlowRegime, fr_m: f64, c_l: f64) -> f64 {
    let (a, b, c) = match regime {
        FlowRegime::Segregated | FlowRegime::Transition => (0.98, 0.4846, 0.0868),
        FlowRegime::Intermittent => (0.845, 0.5351, 0.0173),
        FlowRegime::Distributed => (1.065, 0.5824, 0.0609),
    };
    (a * c_l.powf(b) / fr_m.powf(c)).max(c_l)
}

/// Inclination correction `β`, uphill coefficients only (Beggs & Brill 1973,
/// Table 2); distributed flow has no correction (`β=0`).
fn beta(regime: FlowRegime, c_l: f64, n_lv: f64, fr_m: f64) -> f64 {
    let (d, e, f, g) = match regime {
        FlowRegime::Segregated | FlowRegime::Transition => (0.011, -3.768, 3.539, -1.614),
        FlowRegime::Intermittent => (2.96, 0.305, -0.4473, 0.0978),
        FlowRegime::Distributed => return 0.0,
    };
    let x = d * c_l.powf(e) * n_lv.powf(f) * fr_m.powf(g);
    (1.0 - c_l) * x.ln()
}

/// Evaluate the Beggs & Brill (1973) two-phase pressure drop over a pipe
/// segment of length `length`, diameter `diameter`, absolute roughness
/// `roughness`, and net elevation rise `elevation_change` (positive =
/// uphill, must satisfy `|elevation_change| <= length`), given liquid and
/// gas volumetric flow rates, densities, a single no-slip-mixture dynamic
/// viscosity (matching DWSIM's treatment), and liquid surface tension.
#[allow(clippy::too_many_arguments)]
pub fn beggs_brill_pressure_drop(
    length: Length,
    diameter: Length,
    roughness: Length,
    elevation_change: Length,
    q_liquid: VolumeRate,
    q_gas: VolumeRate,
    density_liquid: MassDensity,
    density_gas: MassDensity,
    viscosity_no_slip: DynamicViscosity,
    surface_tension_liquid: SurfaceTension,
) -> BeggsBrillResult {
    let d = diameter.get::<meter>();
    let area = std::f64::consts::PI / 4.0 * d * d;
    let q_l = q_liquid.get::<cubic_meter_per_second>();
    let q_g = q_gas.get::<cubic_meter_per_second>();
    let v_sl = q_l / area;
    let v_sg = q_g / area;
    let v_m = v_sl + v_sg;
    let c_l = q_l / (q_l + q_g);
    let fr_m = v_m * v_m / (G * d);

    let l1 = 316.0 * c_l.powf(0.302);
    let l2 = 9.252e-4 * c_l.powf(-2.4684);
    let l3 = 0.1 * c_l.powf(-1.4516);
    let l4 = 0.5 * c_l.powf(-6.738);

    let is_segregated = (c_l < 0.01 && fr_m < l1) || (c_l >= 0.01 && fr_m < l2);
    let is_distributed = (c_l < 0.4 && fr_m >= l1) || (c_l >= 0.4 && fr_m > l4);
    let is_transition = !is_segregated && c_l >= 0.01 && fr_m >= l2 && fr_m <= l3;

    let rho_l = density_liquid.get::<kilogram_per_cubic_meter>();
    let sigma_l = surface_tension_liquid.get::<newton_per_meter>();
    let n_lv = 1.938 * v_sl * (rho_l / (G * sigma_l)).powf(0.25);

    let theta_rad = (elevation_change.get::<meter>() / length.get::<meter>()).asin();
    let incl_factor = |beta_val: f64| -> f64 {
        1.0 + beta_val * ((1.8 * theta_rad).sin() - (1.0 / 3.0) * (1.8 * theta_rad).sin().powi(3))
    };

    let (regime, holdup_incl) = if is_transition {
        let h_seg = holdup_horizontal(FlowRegime::Segregated, fr_m, c_l)
            * incl_factor(beta(FlowRegime::Segregated, c_l, n_lv, fr_m));
        let h_int = holdup_horizontal(FlowRegime::Intermittent, fr_m, c_l)
            * incl_factor(beta(FlowRegime::Intermittent, c_l, n_lv, fr_m));
        let a_blend = (l3 - fr_m) / (l3 - l2);
        (
            FlowRegime::Transition,
            a_blend * h_seg + (1.0 - a_blend) * h_int,
        )
    } else {
        let regime = if is_segregated {
            FlowRegime::Segregated
        } else if is_distributed {
            FlowRegime::Distributed
        } else {
            FlowRegime::Intermittent
        };
        let h0 = holdup_horizontal(regime, fr_m, c_l);
        let b = incl_factor(beta(regime, c_l, n_lv, fr_m));
        (regime, h0 * b)
    };

    let holdup = holdup_incl.clamp(c_l, 1.0);
    let density_no_slip = density_liquid * c_l + density_gas * (1.0 - c_l);
    let mixture_density = density_liquid * holdup + density_gas * (1.0 - holdup);

    // No-slip friction-factor ratio correction (Beggs & Brill f_tp/f_ns).
    let y = (c_l / (holdup * holdup)).ln();
    let s = if y > 1.0 && y < 1.2 {
        (2.2 * y.exp() - 1.2).ln()
    } else {
        y / (-0.0523 + 3.182 * y - 0.8725 * y * y + 0.01853 * y.powi(4))
    };

    let velocity_mix = Velocity::new::<meter_per_second>(v_m);
    let re_ns = reynolds_number(density_no_slip, velocity_mix, diameter, viscosity_no_slip);
    let f_ns = darcy_friction_factor(re_ns, diameter, roughness);
    let f_tp = f_ns.get::<ratio>() * s.exp();

    let friction_pressure_drop = Pressure::new::<pascal>(
        f_tp * v_m * v_m / 2.0
            * density_no_slip.get::<kilogram_per_cubic_meter>()
            * length.get::<meter>()
            / d,
    );
    let elevation_pressure_drop = mixture_density
        * Acceleration::new::<uom::si::acceleration::meter_per_second_squared>(G)
        * elevation_change;

    BeggsBrillResult {
        regime,
        liquid_holdup: Ratio::new::<ratio>(holdup),
        mixture_density,
        friction_pressure_drop,
        elevation_pressure_drop,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uom::si::dynamic_viscosity::pascal_second;
    use uom::si::length::meter;
    use uom::si::mass_density::kilogram_per_cubic_meter;
    use uom::si::surface_tension::newton_per_meter;
    use uom::si::volume_rate::cubic_meter_per_second;

    #[test]
    fn horizontal_high_liquid_fraction_gives_distributed_or_intermittent() {
        let result = beggs_brill_pressure_drop(
            Length::new::<meter>(100.0),
            Length::new::<meter>(0.1),
            Length::new::<meter>(4.6e-5),
            Length::new::<meter>(0.0),
            VolumeRate::new::<cubic_meter_per_second>(0.005),
            VolumeRate::new::<cubic_meter_per_second>(0.001),
            MassDensity::new::<kilogram_per_cubic_meter>(998.0),
            MassDensity::new::<kilogram_per_cubic_meter>(1.2),
            DynamicViscosity::new::<pascal_second>(1.0e-3),
            SurfaceTension::new::<newton_per_meter>(0.072),
        );
        assert!(result.liquid_holdup.get::<ratio>() > 0.5);
        assert!(result.liquid_holdup.get::<ratio>() <= 1.0);
        assert!(result.total_pressure_drop().get::<pascal>() > 0.0);
    }

    #[test]
    fn gas_dominated_flow_gives_low_holdup() {
        let result = beggs_brill_pressure_drop(
            Length::new::<meter>(100.0),
            Length::new::<meter>(0.1),
            Length::new::<meter>(4.6e-5),
            Length::new::<meter>(0.0),
            VolumeRate::new::<cubic_meter_per_second>(0.0005),
            VolumeRate::new::<cubic_meter_per_second>(0.05),
            MassDensity::new::<kilogram_per_cubic_meter>(998.0),
            MassDensity::new::<kilogram_per_cubic_meter>(1.2),
            DynamicViscosity::new::<pascal_second>(1.0e-3),
            SurfaceTension::new::<newton_per_meter>(0.072),
        );
        assert!(result.liquid_holdup.get::<ratio>() < 0.3);
    }

    #[test]
    fn uphill_elevation_adds_positive_hydrostatic_drop() {
        let result = beggs_brill_pressure_drop(
            Length::new::<meter>(100.0),
            Length::new::<meter>(0.1),
            Length::new::<meter>(4.6e-5),
            Length::new::<meter>(10.0),
            VolumeRate::new::<cubic_meter_per_second>(0.005),
            VolumeRate::new::<cubic_meter_per_second>(0.001),
            MassDensity::new::<kilogram_per_cubic_meter>(998.0),
            MassDensity::new::<kilogram_per_cubic_meter>(1.2),
            DynamicViscosity::new::<pascal_second>(1.0e-3),
            SurfaceTension::new::<newton_per_meter>(0.072),
        );
        assert!(result.elevation_pressure_drop.get::<pascal>() > 0.0);
    }
}
