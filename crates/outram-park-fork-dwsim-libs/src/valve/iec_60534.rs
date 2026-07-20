//! IEC 60534 / ISA-75.01.01 control-valve sizing equations.
//!
//! Ported from DWSIM `UnitOperations/Valve.vb` (`KvLiquid`/`WLiquid`,
//! `KvGas`/`WGas`, `KvTwoPhase`/`WTwoPhase`, `P2Liquid`/`P2_Gas`/`P2TwoPhase`),
//! which in turn cites SAMSON T00050EN (steam service) and the Masoneilan
//! Control Valve Sizing Handbook (two-phase combination). Piping geometry
//! factor `F_P`, style modifiers `F_L`/`x_T`, and the unit constant `N6` are
//! taken as caller-supplied per IEC 60534-2-1 (defaults of 1.0 are reasonable
//! placeholders when those effects are not being modelled).
//!
//! `Kv` here follows the IEC 60534 metric convention: dimensioned in
//! m^3 h^-1 bar^-0.5, referenced to water density 999.1 kg/m^3 -- this is an
//! engineering unit, not a `uom` SI quantity, so it is wrapped in
//! [`ValveFlowCoefficient`] rather than left as a bare `f64`.

use uom::si::f64::{MassDensity, MassRate, Pressure, Ratio};
use uom::si::mass_density::kilogram_per_cubic_meter;
use uom::si::mass_rate::kilogram_per_hour;
use uom::si::pressure::bar;
use uom::si::ratio::ratio;

/// Reference water density (999.1 kg/m^3) the IEC 60534 metric `Kv`
/// convention is anchored to.
const REFERENCE_WATER_DENSITY_KG_PER_M3: f64 = 999.1;

/// IEC 60534-2-1 unit constant for gas/steam mass-flow sizing with `W` in
/// kg/h, `P` in bar, `ρ` in kg/m^3.
const N6: f64 = 31.6;

/// Valve flow coefficient `Kv` \[m^3 h^-1 bar^-0.5\], IEC 60534 metric
/// convention (referenced to water at 999.1 kg/m^3). Not a `uom` SI
/// quantity -- `Kv`/`Cv` are defined by their sizing equation, not a
/// fundamental physical dimension.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct ValveFlowCoefficient(pub f64);

impl ValveFlowCoefficient {
    /// Convert to the US customary flow coefficient `Cv` \[US gal/min
    /// psi^-0.5\] via the fixed ratio `Cv = 1.156 * Kv` (DWSIM's
    /// `FlowCoefficient` conversion).
    pub fn to_cv(self) -> f64 {
        1.156 * self.0
    }

    /// Construct from a `Cv` \[US gal/min psi^-0.5\] value.
    pub fn from_cv(cv: f64) -> Self {
        Self(cv / 1.156)
    }
}

/// Liquid critical-pressure ratio factor `F_F = 0.96 - 0.28 sqrt(p_v/p_c)`
/// (IEC 60534), used to find the choked-flow pressure-drop limit for liquid
/// service.
pub fn liquid_critical_pressure_ratio_factor(
    vapor_pressure: Pressure,
    critical_pressure: Pressure,
) -> Ratio {
    let pv_over_pc = (vapor_pressure / critical_pressure).get::<ratio>();
    Ratio::new::<ratio>(0.96 - 0.28 * pv_over_pc.sqrt())
}

/// Choked-flow pressure-drop limit for liquid service,
/// `ΔP_choke = F_L^2 (p1 - F_F p_v)`. If the actual pressure drop `p1 - p2`
/// exceeds this, the valve is choked and `p2` should be clamped to
/// `p1 - ΔP_choke` before evaluating [`kv_liquid`]/[`mass_flow_liquid`].
pub fn choked_pressure_drop_liquid(
    p1: Pressure,
    liquid_pressure_recovery_factor: Ratio,
    f_f: Ratio,
    vapor_pressure: Pressure,
) -> Pressure {
    let f_l = liquid_pressure_recovery_factor.get::<ratio>();
    f_l * f_l * (p1 - f_f.get::<ratio>() * vapor_pressure)
}

/// Clamp `p2` to the choked-flow limit for liquid service, if the requested
/// drop `p1 - p2` would exceed it. Returns the (possibly clamped) outlet
/// pressure to use in the sizing equations.
pub fn clamp_to_choked_liquid(
    p1: Pressure,
    p2: Pressure,
    liquid_pressure_recovery_factor: Ratio,
    f_f: Ratio,
    vapor_pressure: Pressure,
) -> Pressure {
    let dp_choke =
        choked_pressure_drop_liquid(p1, liquid_pressure_recovery_factor, f_f, vapor_pressure);
    let dp_actual = p1 - p2;
    if dp_choke > Pressure::new::<bar>(0.0) && dp_choke < dp_actual {
        p1 - dp_choke
    } else {
        p2
    }
}

/// Required `Kv` for liquid service given mass flow `w`, density `rho`, and
/// pressure drop `p1 - p2` (already clamped to the choked limit if
/// applicable, see [`clamp_to_choked_liquid`]), and piping geometry factor
/// `f_p`.
pub fn kv_liquid(w: MassRate, density: MassDensity, p1: Pressure, p2: Pressure, f_p: Ratio) -> ValveFlowCoefficient {
    let w_kg_h = w.get::<kilogram_per_hour>();
    let rho = density.get::<kilogram_per_cubic_meter>();
    let dp_bar = (p1 - p2).get::<bar>();
    ValveFlowCoefficient(
        w_kg_h / f_p.get::<ratio>() / (rho * REFERENCE_WATER_DENSITY_KG_PER_M3 * dp_bar).sqrt(),
    )
}

/// Mass flow rate for liquid service given `Kv`, density, pressure drop
/// (already clamped to the choked limit if applicable), and piping geometry
/// factor `f_p` -- the inverse of [`kv_liquid`].
pub fn mass_flow_liquid(
    kv: ValveFlowCoefficient,
    density: MassDensity,
    p1: Pressure,
    p2: Pressure,
    f_p: Ratio,
) -> MassRate {
    let rho = density.get::<kilogram_per_cubic_meter>();
    let dp_bar = (p1 - p2).get::<bar>();
    let w_kg_h =
        kv.0 * f_p.get::<ratio>() * (rho * REFERENCE_WATER_DENSITY_KG_PER_M3 * dp_bar).sqrt();
    MassRate::new::<kilogram_per_hour>(w_kg_h)
}

/// Outlet pressure `p2` for a target liquid mass flow `w`, given `Kv` --
/// closed-form inverse of [`kv_liquid`] (liquid is the only service where
/// this back-solve is closed-form; [`solve_p2_gas`] needs bisection instead,
/// since `Y`/choking make it non-invertible in closed form. A two-phase P2
/// back-solve -- DWSIM's `P2TwoPhase`/`P1TwoPhase` -- is not ported here;
/// this pass only covers the forward two-phase sizing in [`kv_two_phase`]/
/// [`mass_flow_two_phase`]).
pub fn solve_p2_liquid(
    w: MassRate,
    kv: ValveFlowCoefficient,
    density: MassDensity,
    p1: Pressure,
    f_p: Ratio,
) -> Pressure {
    let w_kg_h = w.get::<kilogram_per_hour>();
    let rho = density.get::<kilogram_per_cubic_meter>();
    let dp_bar = (w_kg_h / (kv.0 * f_p.get::<ratio>())).powi(2) / (rho * REFERENCE_WATER_DENSITY_KG_PER_M3);
    p1 - Pressure::new::<bar>(dp_bar)
}

/// Expansion factor `Y = 1 - x / (3 x_choked)` for gas/vapour service, where
/// `x = (p1-p2)/p1` is clamped to the choked-flow limit `x_choked =
/// (k/1.4) x_T` beforehand (see [`choked_pressure_drop_ratio_gas`]).
pub fn expansion_factor_gas(pressure_drop_ratio: Ratio, choked_pressure_drop_ratio: Ratio) -> Ratio {
    let x = pressure_drop_ratio.get::<ratio>();
    let x_choked = choked_pressure_drop_ratio.get::<ratio>();
    Ratio::new::<ratio>(1.0 - x / (3.0 * x_choked))
}

/// Choked-flow pressure-drop ratio limit for gas/vapour service,
/// `x_choked = (k/1.4) x_T`, where `k = Cp/Cv` (specific heat ratio) and
/// `x_T` is the valve's terminal pressure-drop ratio style modifier.
pub fn choked_pressure_drop_ratio_gas(specific_heat_ratio: Ratio, x_t: Ratio) -> Ratio {
    Ratio::new::<ratio>(specific_heat_ratio.get::<ratio>() / 1.4 * x_t.get::<ratio>())
}

/// Clamp the pressure-drop ratio `x = (p1-p2)/p1` to the gas choked-flow
/// limit, returning `(x_clamped, Y)`.
pub fn clamp_and_expand_gas(
    p1: Pressure,
    p2: Pressure,
    specific_heat_ratio: Ratio,
    x_t: Ratio,
) -> (Ratio, Ratio) {
    let x_choked = choked_pressure_drop_ratio_gas(specific_heat_ratio, x_t);
    let x_raw = ((p1 - p2) / p1).get::<ratio>();
    let x = Ratio::new::<ratio>(x_raw.min(x_choked.get::<ratio>()));
    let y = expansion_factor_gas(x, x_choked);
    (x, y)
}

/// Required `Kv` for gas/vapour service given mass flow `w`, upstream
/// pressure `p1`, upstream density `rho1`, the (already-clamped) pressure
/// drop ratio `x` and expansion factor `y` from [`clamp_and_expand_gas`],
/// and piping geometry factor `f_p`.
pub fn kv_gas(
    w: MassRate,
    p1: Pressure,
    density_upstream: MassDensity,
    x: Ratio,
    y: Ratio,
    f_p: Ratio,
) -> ValveFlowCoefficient {
    let w_kg_h = w.get::<kilogram_per_hour>();
    let p1_bar = p1.get::<bar>();
    let rho1 = density_upstream.get::<kilogram_per_cubic_meter>();
    ValveFlowCoefficient(
        w_kg_h
            / (N6 * f_p.get::<ratio>() * y.get::<ratio>() * (x.get::<ratio>() * p1_bar * rho1).sqrt()),
    )
}

/// Mass flow rate for gas/vapour service -- the inverse of [`kv_gas`].
pub fn mass_flow_gas(
    kv: ValveFlowCoefficient,
    p1: Pressure,
    density_upstream: MassDensity,
    x: Ratio,
    y: Ratio,
    f_p: Ratio,
) -> MassRate {
    let p1_bar = p1.get::<bar>();
    let rho1 = density_upstream.get::<kilogram_per_cubic_meter>();
    let w_kg_h = kv.0
        * N6
        * f_p.get::<ratio>()
        * y.get::<ratio>()
        * (x.get::<ratio>() * p1_bar * rho1).sqrt();
    MassRate::new::<kilogram_per_hour>(w_kg_h)
}

/// Outlet pressure `p2` for a target gas/vapour mass flow `w`, given `Kv` --
/// bisection inverse of [`kv_gas`] (non-invertible in closed form because of
/// the `Y`/choking non-linearity, matching DWSIM's own `P2_Gas`).
#[allow(clippy::too_many_arguments)]
pub fn solve_p2_gas(
    w: MassRate,
    kv: ValveFlowCoefficient,
    p1: Pressure,
    density_upstream: MassDensity,
    specific_heat_ratio: Ratio,
    x_t: Ratio,
    f_p: Ratio,
    tolerance: Pressure,
) -> Pressure {
    let mut p2_low = Pressure::new::<bar>(1e-6);
    let mut p2_high = p1;
    for _ in 0..200 {
        let p2_mid = (p2_low + p2_high) / 2.0;
        let (x, y) = clamp_and_expand_gas(p1, p2_mid, specific_heat_ratio, x_t);
        let w_at_mid = mass_flow_gas(kv, p1, density_upstream, x, y, f_p);
        if w_at_mid > w {
            // Too much flow at this p2 -> need a smaller pressure drop -> raise p2.
            p2_low = p2_mid;
        } else {
            p2_high = p2_mid;
        }
        if (p2_high - p2_low).abs() < tolerance {
            break;
        }
    }
    (p2_low + p2_high) / 2.0
}

/// Two-phase `Kv`, combining independently-evaluated liquid and gas `Kv` at
/// the same `(p1, p2)` by mass-fraction-weighted quadrature (Masoneilan-style):
/// `Kv = sqrt(mass_frac_gas * Kv_gas^2 + mass_frac_liquid * Kv_liquid^2)`.
pub fn kv_two_phase(
    kv_liquid: ValveFlowCoefficient,
    kv_gas: ValveFlowCoefficient,
    mass_fraction_liquid: Ratio,
    mass_fraction_gas: Ratio,
) -> ValveFlowCoefficient {
    ValveFlowCoefficient(
        (mass_fraction_gas.get::<ratio>() * kv_gas.0 * kv_gas.0
            + mass_fraction_liquid.get::<ratio>() * kv_liquid.0 * kv_liquid.0)
            .sqrt(),
    )
}

/// Two-phase mass flow rate, combining independently-evaluated liquid and
/// gas mass flow at the same `(Kv, p1, p2)`:
/// `1/W^2 = mass_frac_liquid/W_liquid^2 + mass_frac_gas/W_gas^2`.
pub fn mass_flow_two_phase(
    mass_flow_liquid: MassRate,
    mass_flow_gas: MassRate,
    mass_fraction_liquid: Ratio,
    mass_fraction_gas: Ratio,
) -> MassRate {
    let w_l = mass_flow_liquid.get::<kilogram_per_hour>();
    let w_g = mass_flow_gas.get::<kilogram_per_hour>();
    let inv_w_sq = mass_fraction_liquid.get::<ratio>() / (w_l * w_l)
        + mass_fraction_gas.get::<ratio>() / (w_g * w_g);
    MassRate::new::<kilogram_per_hour>((1.0 / inv_w_sq).sqrt())
}

/// Which opening-characteristic curve relates a valve's opening percentage
/// `OP` \[0, 100\] to its flow coefficient. DWSIM's `UserDefined` (a
/// user-entered expression) and `DataTable` (an arbitrary lookup table) are
/// not represented here -- those need a runtime expression evaluator /
/// interpolation table respectively, out of scope for this port.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum OpeningCharacteristic {
    /// `Kv(OP) = Kv_max * OP/100`.
    Linear,
    /// `Kv(OP) = Kv_max * sqrt(OP/100)`.
    QuickOpening,
    /// `Kv(OP) = Kv_max * R^(OP/100 - 1)`, `R` = rangeability parameter.
    EqualPercentage {
        /// Rangeability parameter `R` (typically 20-50).
        rangeability: f64,
    },
}

impl OpeningCharacteristic {
    /// Evaluate `Kv` at a given opening percentage `opening_pct` \[0, 100\].
    pub fn kv_at_opening(&self, kv_max: ValveFlowCoefficient, opening_pct: Ratio) -> ValveFlowCoefficient {
        let op = opening_pct.get::<ratio>() / 100.0;
        let factor = match self {
            Self::Linear => op,
            Self::QuickOpening => op.sqrt(),
            Self::EqualPercentage { rangeability } => rangeability.powf(op - 1.0),
        };
        ValveFlowCoefficient(kv_max.0 * factor)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uom::si::mass_density::kilogram_per_cubic_meter;
    use uom::si::mass_rate::kilogram_per_hour;
    use uom::si::pressure::bar;

    #[test]
    fn kv_liquid_and_mass_flow_liquid_are_inverses() {
        let density = MassDensity::new::<kilogram_per_cubic_meter>(998.0);
        let p1 = Pressure::new::<bar>(10.0);
        let p2 = Pressure::new::<bar>(6.0);
        let f_p = Ratio::new::<ratio>(1.0);
        let w = MassRate::new::<kilogram_per_hour>(5000.0);

        let kv = kv_liquid(w, density, p1, p2, f_p);
        let w_back = mass_flow_liquid(kv, density, p1, p2, f_p);
        assert!((w_back.get::<kilogram_per_hour>() - 5000.0).abs() < 1e-6);
    }

    #[test]
    fn solve_p2_liquid_round_trips_kv_liquid() {
        let density = MassDensity::new::<kilogram_per_cubic_meter>(998.0);
        let p1 = Pressure::new::<bar>(10.0);
        let p2 = Pressure::new::<bar>(6.0);
        let f_p = Ratio::new::<ratio>(1.0);
        let w = MassRate::new::<kilogram_per_hour>(5000.0);

        let kv = kv_liquid(w, density, p1, p2, f_p);
        let p2_back = solve_p2_liquid(w, kv, density, p1, f_p);
        assert!((p2_back.get::<bar>() - 6.0).abs() < 1e-6);
    }

    #[test]
    fn solve_p2_gas_converges_to_within_tolerance() {
        let p1 = Pressure::new::<bar>(10.0);
        let density = MassDensity::new::<kilogram_per_cubic_meter>(5.0);
        let k = Ratio::new::<ratio>(1.3);
        let x_t = Ratio::new::<ratio>(0.7);
        let f_p = Ratio::new::<ratio>(1.0);
        let p2_true = Pressure::new::<bar>(8.0);

        let (x, y) = clamp_and_expand_gas(p1, p2_true, k, x_t);
        let kv = kv_gas(MassRate::new::<kilogram_per_hour>(1000.0), p1, density, x, y, f_p);
        let w = mass_flow_gas(kv, p1, density, x, y, f_p);

        let p2_solved = solve_p2_gas(w, kv, p1, density, k, x_t, f_p, Pressure::new::<bar>(1e-4));
        assert!((p2_solved.get::<bar>() - 8.0).abs() < 1e-2);
    }

    #[test]
    fn opening_characteristics_agree_at_full_open() {
        let kv_max = ValveFlowCoefficient(100.0);
        let full_open = Ratio::new::<ratio>(100.0);
        assert!((OpeningCharacteristic::Linear.kv_at_opening(kv_max, full_open).0 - 100.0).abs() < 1e-9);
        assert!(
            (OpeningCharacteristic::QuickOpening.kv_at_opening(kv_max, full_open).0 - 100.0).abs()
                < 1e-9
        );
        assert!(
            (OpeningCharacteristic::EqualPercentage { rangeability: 30.0 }
                .kv_at_opening(kv_max, full_open)
                .0
                - 100.0)
                .abs()
                < 1e-9
        );
    }

    #[test]
    fn opening_characteristics_are_zero_at_closed() {
        let kv_max = ValveFlowCoefficient(100.0);
        let closed = Ratio::new::<ratio>(0.0);
        assert!(OpeningCharacteristic::Linear.kv_at_opening(kv_max, closed).0.abs() < 1e-9);
        assert!(OpeningCharacteristic::QuickOpening.kv_at_opening(kv_max, closed).0.abs() < 1e-9);
    }

    #[test]
    fn kv_to_cv_round_trip() {
        let kv = ValveFlowCoefficient(50.0);
        let cv = kv.to_cv();
        let kv_back = ValveFlowCoefficient::from_cv(cv);
        assert!((kv_back.0 - 50.0).abs() < 1e-9);
    }
}
