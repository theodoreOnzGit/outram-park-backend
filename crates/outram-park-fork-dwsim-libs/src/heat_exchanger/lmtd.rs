//! Log-mean temperature difference (LMTD) rating.
//!
//! Ported from DWSIM `UnitOperations/HeatExchanger.vb`'s `Calculate()`
//! preamble (the co/counter-current LMTD forms shared by every calculation
//! mode in that file).

use uom::si::f64::{Area, HeatTransfer, Power, TemperatureInterval, ThermodynamicTemperature};
use uom::si::heat_transfer::watt_per_square_meter_kelvin;
use uom::si::power::watt;
use uom::si::temperature_interval::kelvin as temperature_interval_kelvin;
use uom::si::thermodynamic_temperature::kelvin;

/// Which end of the exchanger the hot and cold streams enter from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlowArrangement {
    /// Hot and cold streams flow in the same direction.
    CoCurrent,
    /// Hot and cold streams flow in opposite directions (the common case,
    /// and always more effective than co-current for the same `U`, `A`).
    CounterCurrent,
}

/// Log-mean temperature difference for a 1-1 (single-pass) exchanger.
///
/// `t_hot_in`/`t_hot_out` are the hot stream's inlet/outlet temperatures;
/// `t_cold_in`/`t_cold_out` the cold stream's. Returns `TemperatureInterval`
/// (a magnitude, not an absolute temperature).
pub fn lmtd(
    arrangement: FlowArrangement,
    t_hot_in: ThermodynamicTemperature,
    t_hot_out: ThermodynamicTemperature,
    t_cold_in: ThermodynamicTemperature,
    t_cold_out: ThermodynamicTemperature,
) -> TemperatureInterval {
    let th_in = t_hot_in.get::<kelvin>();
    let th_out = t_hot_out.get::<kelvin>();
    let tc_in = t_cold_in.get::<kelvin>();
    let tc_out = t_cold_out.get::<kelvin>();
    let (dt1, dt2) = match arrangement {
        FlowArrangement::CoCurrent => (th_in - tc_in, th_out - tc_out),
        FlowArrangement::CounterCurrent => (th_in - tc_out, th_out - tc_in),
    };
    TemperatureInterval::new::<temperature_interval_kelvin>((dt1 - dt2) / (dt1 / dt2).ln())
}

/// Heat duty `Q = U A (LMTD) F`, where `F` is an optional multi-pass
/// correction factor (see [`crate::heat_exchanger::f_correction`]; pass
/// `Ratio::new::<ratio>(1.0)` for a true 1-1 exchanger).
pub fn duty(
    overall_coefficient: HeatTransfer,
    area: Area,
    lmtd: TemperatureInterval,
    f_correction: uom::si::f64::Ratio,
) -> Power {
    Power::new::<watt>(
        overall_coefficient.get::<watt_per_square_meter_kelvin>()
            * area.get::<uom::si::area::square_meter>()
            * lmtd.get::<temperature_interval_kelvin>()
            * f_correction.get::<uom::si::ratio::ratio>(),
    )
}

/// Overall heat-transfer coefficient `U = Q / (A * LMTD * F)` -- the inverse
/// of [`duty`], used when `Q` and `A` are known and `U` is the sought output
/// (DWSIM's `CalcTempHotOut`/`CalcTempColdOut`/`CalcBothTemp` modes).
pub fn overall_coefficient_from_duty(
    duty: Power,
    area: Area,
    lmtd: TemperatureInterval,
    f_correction: uom::si::f64::Ratio,
) -> HeatTransfer {
    HeatTransfer::new::<watt_per_square_meter_kelvin>(
        duty.get::<watt>()
            / (area.get::<uom::si::area::square_meter>()
                * lmtd.get::<temperature_interval_kelvin>()
                * f_correction.get::<uom::si::ratio::ratio>()),
    )
}

/// Heat-transfer area `A = Q / (U * LMTD * F)` -- used when `Q` and `U` are
/// known and `A` is the sought output (DWSIM's `CalcArea`/`ThermalEfficiency`
/// modes).
pub fn area_from_duty(
    duty: Power,
    overall_coefficient: HeatTransfer,
    lmtd: TemperatureInterval,
    f_correction: uom::si::f64::Ratio,
) -> Area {
    Area::new::<uom::si::area::square_meter>(
        duty.get::<watt>()
            / (overall_coefficient.get::<watt_per_square_meter_kelvin>()
                * lmtd.get::<temperature_interval_kelvin>()
                * f_correction.get::<uom::si::ratio::ratio>()),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use uom::si::area::square_meter;
    use uom::si::f64::Ratio;
    use uom::si::ratio::ratio;

    #[test]
    fn counter_current_lmtd_matches_hand_calc() {
        // Hot: 373.15 -> 333.15 K, Cold: 293.15 -> 313.15 K (counter-current)
        let t_hot_in = ThermodynamicTemperature::new::<kelvin>(373.15);
        let t_hot_out = ThermodynamicTemperature::new::<kelvin>(333.15);
        let t_cold_in = ThermodynamicTemperature::new::<kelvin>(293.15);
        let t_cold_out = ThermodynamicTemperature::new::<kelvin>(313.15);
        let result = lmtd(
            FlowArrangement::CounterCurrent,
            t_hot_in,
            t_hot_out,
            t_cold_in,
            t_cold_out,
        );
        // dt1 = 373.15-313.15=60, dt2=333.15-293.15=40 -> (60-40)/ln(60/40) = 49.33
        assert!((result.get::<temperature_interval_kelvin>() - 49.326).abs() < 0.01);
    }

    #[test]
    fn co_current_lmtd_is_smaller_than_counter_current_for_same_temps() {
        let t_hot_in = ThermodynamicTemperature::new::<kelvin>(373.15);
        let t_hot_out = ThermodynamicTemperature::new::<kelvin>(333.15);
        let t_cold_in = ThermodynamicTemperature::new::<kelvin>(293.15);
        let t_cold_out = ThermodynamicTemperature::new::<kelvin>(313.15);
        let cc = lmtd(
            FlowArrangement::CounterCurrent,
            t_hot_in,
            t_hot_out,
            t_cold_in,
            t_cold_out,
        );
        let co = lmtd(
            FlowArrangement::CoCurrent,
            t_hot_in,
            t_hot_out,
            t_cold_in,
            t_cold_out,
        );
        assert!(co.get::<temperature_interval_kelvin>() < cc.get::<temperature_interval_kelvin>());
    }

    #[test]
    fn duty_and_area_from_duty_are_inverses() {
        let u = HeatTransfer::new::<watt_per_square_meter_kelvin>(500.0);
        let a = Area::new::<square_meter>(10.0);
        let dt = TemperatureInterval::new::<temperature_interval_kelvin>(20.0);
        let f = Ratio::new::<ratio>(1.0);
        let q = duty(u, a, dt, f);
        let a_back = area_from_duty(q, u, dt, f);
        assert!((a_back.get::<square_meter>() - 10.0).abs() < 1e-9);
    }
}
