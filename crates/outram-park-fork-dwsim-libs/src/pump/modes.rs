//! Pump duty/pressure-rise calculation modes, and NPSH.
//!
//! Ported from DWSIM `UnitOperations/Pump.vb`'s `Calculate()`. DWSIM follows
//! each mode's algebra with a pressure-enthalpy flash to get outlet
//! temperature -- this pump port is deliberately kept decoupled from the
//! crate's flash kernel ([`crate::thermo`]), so [`PumpResult::outlet_enthalpy`]
//! is as far as this port goes; a caller (e.g. `tampines`, or the crate's own
//! flash) does the final `(p2, h2) -> T2` flash itself. DWSIM's `Curves`
//! calculation mode is not wired into the pump here -- though the underlying
//! Floater-Hormann rational interpolation of head/efficiency/NPSHr/power
//! vs. flow is available in [`crate::interpolation`]; see `op-qo2.9`.

use uom::si::acceleration::meter_per_second_squared;
use uom::si::f64::{
    Acceleration, AvailableEnergy, Length, MassDensity, MassRate, Power, Pressure, Ratio,
};
use uom::si::power::watt;
use uom::si::pressure::pascal;

/// Standard gravitational acceleration.
fn standard_gravity() -> Acceleration {
    Acceleration::new::<meter_per_second_squared>(9.80665)
}

/// The pump's inlet stream state and the one liquid property (density) its
/// hydraulic-power calculation needs.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PumpInlet {
    /// Inlet pressure.
    pub pressure: Pressure,
    /// Inlet specific enthalpy.
    pub enthalpy: AvailableEnergy,
    /// Inlet liquid mass density.
    pub density_liquid: MassDensity,
    /// Mass flow rate through the pump.
    pub mass_flow: MassRate,
}

/// Which quantity specifies the pump's operating point -- exactly one
/// degree of freedom, matching DWSIM's `CalculationMode` enum (`Curves` is
/// not represented, see this module's doc).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PumpSpecification {
    /// Fixed pressure rise `ΔP = p2 - p1`.
    DeltaP(Pressure),
    /// Fixed outlet pressure `p2` (equivalent to `DeltaP(p2 - p1)`).
    OutletPressure(Pressure),
    /// Fixed shaft power.
    Power(Power),
    /// Fixed duty from an external energy stream (DWSIM's `EnergyStream`
    /// mode) -- note this mode's enthalpy-rise relation differs from the
    /// other three (see [`evaluate`]'s source comments): efficiency
    /// *reduces* the usable enthalpy rise here, rather than inflating the
    /// hydraulic power to a larger shaft power.
    EnergyStreamDuty(Power),
}

/// Result of a [`evaluate`] call.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PumpResult {
    /// Outlet pressure.
    pub outlet_pressure: Pressure,
    /// Shaft power (or externally-specified duty, in `EnergyStreamDuty` mode).
    pub power: Power,
    /// Outlet specific enthalpy.
    pub outlet_enthalpy: AvailableEnergy,
    /// Pump head, `(p2 - p1) / (ρ_l g)`.
    pub head: Length,
}

/// Evaluate a pump's outlet state for the given [`PumpSpecification`] and
/// pump efficiency \[0, 1\].
pub fn evaluate(inlet: PumpInlet, spec: PumpSpecification, efficiency: Ratio) -> PumpResult {
    let p1 = inlet.pressure;
    let h1 = inlet.enthalpy;
    let rho_l = inlet.density_liquid;
    let w = inlet.mass_flow;
    let eta = efficiency.get::<uom::si::ratio::ratio>();

    let (p2, power, h2) = match spec {
        PumpSpecification::DeltaP(delta_p) => {
            // Hydraulic power (Q_vol * dP) divided by efficiency = shaft power.
            let power = Power::new::<watt>(delta_p.get::<pascal>() / rho_l.value * w.value / eta);
            let h2 = h1 + power / w;
            (p1 + delta_p, power, h2)
        }
        PumpSpecification::OutletPressure(p2_spec) => {
            let delta_p = p2_spec - p1;
            let power = Power::new::<watt>(delta_p.get::<pascal>() / rho_l.value * w.value / eta);
            let h2 = h1 + power / w;
            (p1 + delta_p, power, h2)
        }
        PumpSpecification::Power(power) => {
            let h2 = h1 + power / w;
            // Inverse of the DeltaP branch's power formula.
            let delta_p =
                Pressure::new::<pascal>(power.get::<watt>() * eta * rho_l.value / w.value);
            (p1 + delta_p, power, h2)
        }
        PumpSpecification::EnergyStreamDuty(duty) => {
            // DWSIM: H2 = H1 + duty*eta/W (efficiency scales the usable
            // enthalpy rise down, since duty here is a raw energy input,
            // not already a hydraulic-power figure); P2 follows from the
            // incompressible-liquid approximation dP = rho_l * dH.
            let h2 = h1 + duty * eta / w;
            let delta_p = Pressure::new::<pascal>((h2 - h1).value * rho_l.value);
            (p1 + delta_p, duty, h2)
        }
    };

    let head = Length::new::<uom::si::length::meter>(
        (p2 - p1).get::<pascal>() / (rho_l.value * standard_gravity().value),
    );

    PumpResult {
        outlet_pressure: p2,
        power,
        outlet_enthalpy: h2,
        head,
    }
}

/// Net positive suction head available,
/// `NPSH = (p1 - p_bubble) / (ρ_l g)`, from the inlet pressure `p1`, the
/// liquid's bubble-point pressure `p_bubble` at the inlet temperature (a
/// `(T, VF=0)` flash, supplied by the caller), and inlet liquid density.
/// Returns `None` if `p1 <= p_bubble` (already at or below the bubble point
/// -- DWSIM returns `+infinity` here via a caught flash failure; `None` is
/// this port's equivalent "not meaningful" signal instead of an
/// infinite/NaN `Length`).
pub fn npsh(p1: Pressure, p_bubble: Pressure, density_liquid: MassDensity) -> Option<Length> {
    if p1 <= p_bubble {
        return None;
    }
    Some(Length::new::<uom::si::length::meter>(
        (p1 - p_bubble).get::<pascal>() / (density_liquid.value * standard_gravity().value),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use uom::si::available_energy::joule_per_kilogram;
    use uom::si::mass_density::kilogram_per_cubic_meter;
    use uom::si::mass_rate::kilogram_per_second;
    use uom::si::ratio::ratio;

    fn sample_inlet() -> PumpInlet {
        PumpInlet {
            pressure: Pressure::new::<pascal>(101_325.0),
            enthalpy: AvailableEnergy::new::<joule_per_kilogram>(100_000.0),
            density_liquid: MassDensity::new::<kilogram_per_cubic_meter>(998.0),
            mass_flow: MassRate::new::<kilogram_per_second>(5.0),
        }
    }

    #[test]
    fn delta_p_and_outlet_pressure_modes_agree() {
        let inlet = sample_inlet();
        let eta = Ratio::new::<ratio>(0.75);
        let delta_p = Pressure::new::<pascal>(500_000.0);
        let r1 = evaluate(inlet, PumpSpecification::DeltaP(delta_p), eta);
        let r2 = evaluate(
            inlet,
            PumpSpecification::OutletPressure(inlet.pressure + delta_p),
            eta,
        );
        assert!(
            (r1.outlet_pressure.get::<pascal>() - r2.outlet_pressure.get::<pascal>()).abs() < 1e-6
        );
        assert!((r1.power.get::<watt>() - r2.power.get::<watt>()).abs() < 1e-6);
    }

    #[test]
    fn power_mode_is_inverse_of_delta_p_mode() {
        let inlet = sample_inlet();
        let eta = Ratio::new::<ratio>(0.75);
        let delta_p = Pressure::new::<pascal>(500_000.0);
        let from_dp = evaluate(inlet, PumpSpecification::DeltaP(delta_p), eta);
        let from_power = evaluate(inlet, PumpSpecification::Power(from_dp.power), eta);
        assert!(
            (from_dp.outlet_pressure.get::<pascal>() - from_power.outlet_pressure.get::<pascal>())
                .abs()
                < 1e-3
        );
    }

    #[test]
    fn pump_increases_outlet_pressure_and_enthalpy() {
        let inlet = sample_inlet();
        let eta = Ratio::new::<ratio>(0.75);
        let delta_p = Pressure::new::<pascal>(500_000.0);
        let result = evaluate(inlet, PumpSpecification::DeltaP(delta_p), eta);
        assert!(result.outlet_pressure.get::<pascal>() > inlet.pressure.get::<pascal>());
        assert!(
            result.outlet_enthalpy.get::<joule_per_kilogram>()
                > inlet.enthalpy.get::<joule_per_kilogram>()
        );
        assert!(result.head.get::<uom::si::length::meter>() > 0.0);
    }

    #[test]
    fn npsh_positive_when_above_bubble_point() {
        let p1 = Pressure::new::<pascal>(200_000.0);
        let p_bubble = Pressure::new::<pascal>(50_000.0);
        let density = MassDensity::new::<kilogram_per_cubic_meter>(998.0);
        let result = npsh(p1, p_bubble, density);
        assert!(result.is_some());
        assert!(result.unwrap().get::<uom::si::length::meter>() > 0.0);
    }

    #[test]
    fn npsh_none_when_at_or_below_bubble_point() {
        let p1 = Pressure::new::<pascal>(50_000.0);
        let p_bubble = Pressure::new::<pascal>(50_000.0);
        let density = MassDensity::new::<kilogram_per_cubic_meter>(998.0);
        assert!(npsh(p1, p_bubble, density).is_none());
    }
}
