//! epsilon-NTU (number of transfer units) effectiveness method for a 1-1
//! (single-pass) co- or counter-current heat exchanger.
//!
//! Ported from DWSIM `UnitOperations/HeatExchanger.vb`'s `CalcBothTemp_UA`
//! mode, which iterates because it derives heat-capacity rates from
//! flash-derived enthalpy slopes (non-constant, real-fluid `Cp`). This crate
//! has no flash/property-package access of its own, so this port takes the
//! heat-capacity rates `C_hot`/`C_cold` \[W/K\] as given (the standard
//! textbook epsilon-NTU form -- Kays & London / Incropera -- which DWSIM's
//! own per-stream effectiveness formulas reduce to when `C` is constant); a
//! caller with flash access (e.g. `tampines`) can supply a locally
//! linearised `C = W (H2-H1)/(T2-T1)` the same way DWSIM does, or a plain
//! `W * Cp` for ideal/near-ideal fluids.

use super::lmtd::FlowArrangement;
use uom::si::f64::{Power, Ratio, TemperatureInterval, ThermalConductance};
use uom::si::ratio::ratio;
use uom::si::temperature_interval::kelvin as temperature_interval_kelvin;
use uom::si::thermal_conductance::watt_per_kelvin;

/// Result of an epsilon-NTU evaluation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NtuResult {
    /// Number of transfer units, `NTU = UA / C_min`.
    pub ntu: Ratio,
    /// Heat-exchanger effectiveness `ε = Q / Q_max`, \[0, 1\].
    pub effectiveness: Ratio,
    /// Actual heat duty `Q = ε C_min ΔT_max`.
    pub duty: Power,
}

/// Evaluate epsilon-NTU effectiveness and duty for a 1-1 exchanger, given
/// the overall conductance `ua = U * A`, both streams' heat-capacity rates,
/// and the maximum possible temperature difference `delta_t_max = T_hot_in -
/// T_cold_in` (always positive by definition of "hot"/"cold").
pub fn evaluate(
    arrangement: FlowArrangement,
    ua: ThermalConductance,
    c_hot: ThermalConductance,
    c_cold: ThermalConductance,
    delta_t_max: TemperatureInterval,
) -> NtuResult {
    let c_hot = c_hot.get::<watt_per_kelvin>();
    let c_cold = c_cold.get::<watt_per_kelvin>();
    let c_min = c_hot.min(c_cold);
    let c_max = c_hot.max(c_cold);
    let c_r = c_min / c_max;
    let ntu = ua.get::<watt_per_kelvin>() / c_min;

    let effectiveness = match arrangement {
        FlowArrangement::CoCurrent => (1.0 - (-ntu * (1.0 + c_r)).exp()) / (1.0 + c_r),
        FlowArrangement::CounterCurrent => {
            if (c_r - 1.0).abs() < 1e-9 {
                ntu / (1.0 + ntu)
            } else {
                let e = (-ntu * (1.0 - c_r)).exp();
                (1.0 - e) / (1.0 - c_r * e)
            }
        }
    };

    let q = effectiveness * c_min * delta_t_max.get::<temperature_interval_kelvin>();

    NtuResult {
        ntu: Ratio::new::<ratio>(ntu),
        effectiveness: Ratio::new::<ratio>(effectiveness),
        duty: Power::new::<uom::si::power::watt>(q),
    }
}

/// Outlet temperature changes implied by an [`NtuResult`]:
/// `ΔT_hot = Q/C_hot`, `ΔT_cold = Q/C_cold` (hot stream cools by
/// `ΔT_hot`, cold stream warms by `ΔT_cold`).
pub fn outlet_temperature_changes(
    result: &NtuResult,
    c_hot: ThermalConductance,
    c_cold: ThermalConductance,
) -> (TemperatureInterval, TemperatureInterval) {
    let q = result.duty.get::<uom::si::power::watt>();
    (
        TemperatureInterval::new::<temperature_interval_kelvin>(
            q / c_hot.get::<watt_per_kelvin>(),
        ),
        TemperatureInterval::new::<temperature_interval_kelvin>(
            q / c_cold.get::<watt_per_kelvin>(),
        ),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counter_current_effectiveness_bounded_0_to_1() {
        let ua = ThermalConductance::new::<watt_per_kelvin>(500.0);
        let c_hot = ThermalConductance::new::<watt_per_kelvin>(2000.0);
        let c_cold = ThermalConductance::new::<watt_per_kelvin>(3000.0);
        let dt = TemperatureInterval::new::<temperature_interval_kelvin>(80.0);
        let result = evaluate(FlowArrangement::CounterCurrent, ua, c_hot, c_cold, dt);
        assert!(result.effectiveness.get::<ratio>() > 0.0 && result.effectiveness.get::<ratio>() < 1.0);
        assert!(result.duty.get::<uom::si::power::watt>() > 0.0);
    }

    #[test]
    fn counter_current_more_effective_than_co_current() {
        let ua = ThermalConductance::new::<watt_per_kelvin>(500.0);
        let c_hot = ThermalConductance::new::<watt_per_kelvin>(2000.0);
        let c_cold = ThermalConductance::new::<watt_per_kelvin>(1000.0);
        let dt = TemperatureInterval::new::<temperature_interval_kelvin>(80.0);
        let cc = evaluate(FlowArrangement::CounterCurrent, ua, c_hot, c_cold, dt);
        let co = evaluate(FlowArrangement::CoCurrent, ua, c_hot, c_cold, dt);
        assert!(cc.effectiveness.get::<ratio>() >= co.effectiveness.get::<ratio>());
    }

    #[test]
    fn balanced_counter_current_limit_case_c_r_equals_1() {
        // C_hot == C_cold triggers the NTU/(1+NTU) limiting-case branch.
        let ua = ThermalConductance::new::<watt_per_kelvin>(1000.0);
        let c = ThermalConductance::new::<watt_per_kelvin>(1000.0);
        let dt = TemperatureInterval::new::<temperature_interval_kelvin>(50.0);
        let result = evaluate(FlowArrangement::CounterCurrent, ua, c, c, dt);
        let ntu = 1.0;
        let expected_eff = ntu / (1.0 + ntu);
        assert!((result.effectiveness.get::<ratio>() - expected_eff).abs() < 1e-9);
    }

    #[test]
    fn outlet_temperature_changes_are_consistent_with_duty() {
        let ua = ThermalConductance::new::<watt_per_kelvin>(500.0);
        let c_hot = ThermalConductance::new::<watt_per_kelvin>(2000.0);
        let c_cold = ThermalConductance::new::<watt_per_kelvin>(3000.0);
        let dt = TemperatureInterval::new::<temperature_interval_kelvin>(80.0);
        let result = evaluate(FlowArrangement::CounterCurrent, ua, c_hot, c_cold, dt);
        let (dt_hot, dt_cold) = outlet_temperature_changes(&result, c_hot, c_cold);
        let q = result.duty.get::<uom::si::power::watt>();
        assert!((dt_hot.get::<temperature_interval_kelvin>() * c_hot.get::<watt_per_kelvin>() - q).abs() < 1e-6);
        assert!((dt_cold.get::<temperature_interval_kelvin>() * c_cold.get::<watt_per_kelvin>() - q).abs() < 1e-6);
    }
}
