//! Heater: enthalpy-driven heating duty and outlet-state relations.
//!
//! Ported from DWSIM `UnitOperations/Heater.vb` (GPL-3.0), the
//! `Public Overrides Sub Calculate` routine (Heater.vb:421-722). A DWSIM
//! heater performs a stream-heating energy balance: the outlet enthalpy is set
//! by the duty and the outlet temperature comes from a pressure-enthalpy (PH)
//! flash, or vice-versa. This equipment port is deliberately kept decoupled
//! from the crate's flash kernel ([`crate::thermo`]), so every
//! flash-dependent step is pushed to the caller: the functions below take
//! already-known specific enthalpies / mass flow / Cp as inputs, and the
//! rigorous "outlet temperature given" path takes a caller-supplied closure
//! (a generic `Fn`, not a trait object, per the workspace no-`dyn` rule) that
//! evaluates the outlet enthalpy at the specified outlet temperature via the
//! caller's own PH/PT flash.
//!
//! # Sign convention
//!
//! A **heater adds duty**: a positive heat duty `Q` raises the specific
//! enthalpy, so the outlet enthalpy `h2 >= h1` for `Q >= 0`. (The mirror
//! [`crate::cooler`] removes duty: positive `Q` there *lowers* `h2`.) This
//! matches DWSIM's own accounting, where `DeltaQ` is reported positive for a
//! heater and the enthalpy balance is `H2 = +Q*(eta/100)/W + H1`
//! (Heater.vb:475).
//!
//! # Efficiency
//!
//! DWSIM stores efficiency as a percentage in `[0, 100]` (`Eficiencia`,
//! default 100). This port takes it as a dimensionless [`Ratio`] where `1.0`
//! means 100% — the fraction of the electrical/utility duty that actually
//! reaches the process fluid. In the "duty given" direction the fluid sees
//! `Q * eta`; in the "outlet spec given" direction the required duty is
//! `w*(h2-h1)/eta`, i.e. *more* duty than the enthalpy rise because part is
//! lost. Efficiency `0` is degenerate: the "duty given" path then delivers no
//! heat (`h2 = h1`) and the "spec given" path divides by zero (returns a
//! non-finite duty) — callers must guard against it.
//!
//! # Pressure drop
//!
//! Pressure drop is a simple specified value: `p2 = p1 - dp` (Heater.vb:458),
//! exposed as [`outlet_pressure`]. It carries no flash coupling and is shared
//! with [`crate::cooler`], which re-exports it through this module.
//!
//! # Excluded DWSIM behaviour
//!
//! The GUI editor (`EditingForm_HeaterCooler`), XML/JSON serialization
//! (`CloneXML`/`CloneJSON`/`SaveData`), the property-grid reflection
//! (`GetPropertyValue`/`SetPropertyValue`/`GetProperties`), the report
//! builders (`GetReport`/`GetStructuredReport`), the flowsheet energy-stream
//! plumbing (`GetInletEnergyStream`/`EnergyFlow`), and the dynamic-mode
//! accumulation model (`RunDynamicModel`, Heater.vb:244-419) are deliberately
//! NOT ported — they are DWSIM application/solver infrastructure, not the
//! thermodynamic kernel. The `EnergyStream` and `OutletVaporFraction`
//! calculation modes both require an internal flash with no closed-form
//! duty/enthalpy relation of their own, so they are left to the caller (the
//! `EnergyStream` mode is arithmetically identical to `HeatAdded` once the
//! duty is read from the energy stream — use [`outlet_enthalpy_heat_added`]).

use uom::si::available_energy::joule_per_kilogram;
use uom::si::f64::{
    AvailableEnergy, MassRate, Power, Pressure, Ratio, SpecificHeatCapacity,
    ThermodynamicTemperature,
};
use uom::si::mass_rate::kilogram_per_second;
use uom::si::power::watt;
use uom::si::ratio::ratio;
use uom::si::specific_heat_capacity::joule_per_kilogram_kelvin;
use uom::si::thermodynamic_temperature::kelvin;

/// Outlet pressure after a specified pressure drop: `p2 = p1 - dp`.
///
/// Ported from Heater.vb:458 (`P2 = Pi - Me.DeltaP`). This is a plain
/// subtraction with no flash coupling; [`crate::cooler`] re-uses it (Cooler.vb
/// applies the identical `P2 = Pi - Me.DeltaP`, Cooler.vb:436). `pressure_drop`
/// is the specified drop (>= 0 for a real device); no check is applied, so a
/// negative drop yields `p2 > p1`.
pub fn outlet_pressure(p1: Pressure, pressure_drop: Pressure) -> Pressure {
    p1 - pressure_drop
}

/// Outlet temperature for the "temperature change" mode: `T2 = T1 + dT`.
///
/// Ported from Heater.vb:561 (`T2 = Ti + Me.DeltaT`). `delta_t` is a signed
/// temperature *interval* (positive to heat, negative to cool). Shared with
/// [`crate::cooler`], whose temperature-change mode uses the identical
/// `T2 = Ti + Me.DeltaT` (Cooler.vb:587).
pub fn outlet_temperature_from_change(
    t1: ThermodynamicTemperature,
    delta_t: uom::si::f64::TemperatureInterval,
) -> ThermodynamicTemperature {
    use uom::si::temperature_interval::kelvin as ti_kelvin;
    ThermodynamicTemperature::new::<kelvin>(t1.get::<kelvin>() + delta_t.get::<ti_kelvin>())
}

/// Outlet specific enthalpy for the **heat-added** mode (duty given):
/// `h2 = h1 + Q * eta / w`.
///
/// Ported from Heater.vb:475
/// (`H2 = Me.DeltaQ * (Me.Eficiencia / 100) / Wi + Hi`). This is the
/// `HeatAdded` / `HeatAddedRemoved` mode, and is also the closed-form part of
/// the `EnergyStream` mode (Heater.vb:608), which is identical once the duty
/// is taken from the energy stream.
///
/// Sign: a heater *adds* duty, so `h2 >= h1` for `q >= 0`.
///
/// - `h1` — inlet specific enthalpy \[J/kg\].
/// - `q` — heat duty \[W\]; positive adds heat.
/// - `w` — mass flow rate \[kg/s\]; must be `> 0`.
/// - `efficiency` — fraction of duty reaching the fluid, `1.0` = 100%.
pub fn outlet_enthalpy_heat_added(
    h1: AvailableEnergy,
    q: Power,
    w: MassRate,
    efficiency: Ratio,
) -> AvailableEnergy {
    let h2 = h1.get::<joule_per_kilogram>()
        + q.get::<watt>() * efficiency.get::<ratio>() / w.get::<kilogram_per_second>();
    AvailableEnergy::new::<joule_per_kilogram>(h2)
}

/// Heat duty from a known outlet specific enthalpy (rigorous "outlet spec"
/// direction): `Q = w * (h2 - h1) / eta`.
///
/// Ported from Heater.vb:536
/// (`Me.DeltaQ = (H2 - Hi) / (Me.Eficiencia / 100) * Wi`). The outlet enthalpy
/// `h2` comes from the caller's flash at the specified outlet temperature (or
/// vapour fraction); the same relation serves both the `OutletTemperature` and
/// `TemperatureChange` modes (Heater.vb:536 and 580).
///
/// Division by `efficiency` inflates the duty above the raw enthalpy rise
/// `w*(h2-h1)` to account for losses; `efficiency = 0` gives a non-finite duty.
pub fn duty_from_outlet_enthalpy(
    h1: AvailableEnergy,
    h2: AvailableEnergy,
    w: MassRate,
    efficiency: Ratio,
) -> Power {
    let q = w.get::<kilogram_per_second>() * (h2 - h1).get::<joule_per_kilogram>()
        / efficiency.get::<ratio>();
    Power::new::<watt>(q)
}

/// Heat duty for the "outlet temperature given" mode under a **constant-`Cp`
/// approximation**: `Q = w * Cp * (T2 - T1) / eta`.
///
/// This is the closed-form stand-in for the rigorous PT-flash path
/// (Heater.vb:532-536). DWSIM itself always does a rigorous flash — there is
/// no constant-`Cp` branch in `Heater.vb` — so this is a *port-added*
/// approximation valid only where `Cp` is sensibly constant over `[T1, T2]`
/// (no phase change, mild `T` range). For the rigorous enthalpy path use
/// [`duty_from_outlet_temperature`] with a flash closure.
///
/// - `w` — mass flow rate \[kg/s\].
/// - `cp` — mean specific heat capacity \[J/(kg·K)\] over the interval.
/// - `t1`, `t2` — inlet / outlet temperatures \[K\].
/// - `efficiency` — utility-to-fluid fraction, `1.0` = 100%.
pub fn duty_constant_cp(
    w: MassRate,
    cp: SpecificHeatCapacity,
    t1: ThermodynamicTemperature,
    t2: ThermodynamicTemperature,
    efficiency: Ratio,
) -> Power {
    let dt = t2.get::<kelvin>() - t1.get::<kelvin>();
    let q = w.get::<kilogram_per_second>() * cp.get::<joule_per_kilogram_kelvin>() * dt
        / efficiency.get::<ratio>();
    Power::new::<watt>(q)
}

/// Heat duty for the "outlet temperature given" mode, **rigorous** path: the
/// caller supplies the outlet enthalpy at `t2` via a flash closure.
///
/// This mirrors Heater.vb:532-536, where DWSIM does a PT flash at `(P2, T2)`
/// to get `H2` and then `Me.DeltaQ = (H2 - Hi) / (eta/100) * Wi`. Since this
/// crate cannot flash, `outlet_enthalpy_at` is the caller's PT-flash step:
/// given the specified outlet temperature it returns the outlet specific
/// enthalpy `h2`. It is a generic `Fn` (no `dyn`), so it monomorphises with
/// zero dispatch overhead.
///
/// The returned duty equals [`duty_from_outlet_enthalpy`] evaluated at the
/// flashed `h2`.
pub fn duty_from_outlet_temperature<F>(
    h1: AvailableEnergy,
    w: MassRate,
    efficiency: Ratio,
    t2: ThermodynamicTemperature,
    outlet_enthalpy_at: F,
) -> Power
where
    F: Fn(ThermodynamicTemperature) -> AvailableEnergy,
{
    let h2 = outlet_enthalpy_at(t2);
    duty_from_outlet_enthalpy(h1, h2, w, efficiency)
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_relative_eq;
    use uom::si::pressure::pascal;
    use uom::si::temperature_interval::kelvin as ti_kelvin;

    // Convenience constructors keeping the tests readable.
    fn jkg(v: f64) -> AvailableEnergy {
        AvailableEnergy::new::<joule_per_kilogram>(v)
    }
    fn w_(v: f64) -> Power {
        Power::new::<watt>(v)
    }
    fn kgs(v: f64) -> MassRate {
        MassRate::new::<kilogram_per_second>(v)
    }
    fn eta(v: f64) -> Ratio {
        Ratio::new::<ratio>(v)
    }
    fn t_k(v: f64) -> ThermodynamicTemperature {
        ThermodynamicTemperature::new::<kelvin>(v)
    }

    /// V&V — Heater, heat-added mode, outlet enthalpy.
    ///
    /// **Methodology.** Port of Heater.vb:475
    /// `H2 = Q*(eta/100)/W + H1`. Hand-computed reference with
    /// `h1 = 100000 J/kg`, `Q = 50000 W`, `w = 10 kg/s`:
    /// - at `eta = 1.0`: `h2 = 100000 + 50000*1.0/10 = 105000 J/kg`.
    /// - at `eta = 0.5`: `h2 = 100000 + 50000*0.5/10 = 102500 J/kg`.
    ///
    /// **Results (2026-07-24).** Both computed values equal the hand
    /// references to `< 1e-9` relative. Confirms sign (heater raises `h2`) and
    /// the efficiency scaling of delivered heat.
    #[test]
    fn heat_added_outlet_enthalpy() {
        let h2 = outlet_enthalpy_heat_added(jkg(100_000.0), w_(50_000.0), kgs(10.0), eta(1.0));
        assert_relative_eq!(h2.get::<joule_per_kilogram>(), 105_000.0, epsilon = 1e-9);

        let h2_half = outlet_enthalpy_heat_added(jkg(100_000.0), w_(50_000.0), kgs(10.0), eta(0.5));
        assert_relative_eq!(
            h2_half.get::<joule_per_kilogram>(),
            102_500.0,
            epsilon = 1e-9
        );
        assert!(
            h2.get::<joule_per_kilogram>() > 100_000.0,
            "heater must raise enthalpy"
        );
    }

    /// V&V — Heater, outlet-temperature mode, duty via constant-Cp.
    ///
    /// **Methodology.** Port-added constant-`Cp` closed form
    /// `Q = w*Cp*(T2-T1)/eta` (stand-in for the PT-flash of Heater.vb:532-536).
    /// Hand value with `w = 2 kg/s`, `Cp = 4180 J/(kg·K)`, `T1 = 300 K`,
    /// `T2 = 310 K` (liquid-water-like), `eta = 1.0`:
    /// `Q = 2*4180*10 = 83600 W`.
    ///
    /// **Results (2026-07-24).** Computed `Q = 83600 W`, matching to `< 1e-9`
    /// relative. Positive duty for heating (`T2 > T1`), consistent with the
    /// heater sign convention.
    #[test]
    fn outlet_temperature_duty_constant_cp() {
        let cp = SpecificHeatCapacity::new::<joule_per_kilogram_kelvin>(4180.0);
        let q = duty_constant_cp(kgs(2.0), cp, t_k(300.0), t_k(310.0), eta(1.0));
        assert_relative_eq!(q.get::<watt>(), 83_600.0, epsilon = 1e-9);
        assert!(
            q.get::<watt>() > 0.0,
            "heating a stream needs positive duty"
        );
    }

    /// V&V — Heater, rigorous outlet-temperature path agrees with the direct
    /// enthalpy-balance duty.
    ///
    /// **Methodology.** [`duty_from_outlet_temperature`] with a toy flash
    /// closure `h2(T) = h1 + 4180*(T - 300)` (constant-Cp stand-in for a real
    /// PT flash). With `h1 = 1.0e6 J/kg`, `w = 3 kg/s`, `T2 = 305 K`,
    /// `eta = 0.8`: `h2 = 1.0e6 + 4180*5 = 1_020_900 J/kg`, so
    /// `Q = 3*(1_020_900-1_000_000)/0.8 = 78_375 W`.
    ///
    /// **Results (2026-07-24).** Closure path and [`duty_from_outlet_enthalpy`]
    /// both give `78375 W` to `< 1e-9` relative.
    #[test]
    fn outlet_temperature_duty_rigorous_closure() {
        let h1 = jkg(1_000_000.0);
        let flash =
            |t: ThermodynamicTemperature| jkg(1_000_000.0 + 4180.0 * (t.get::<kelvin>() - 300.0));
        let q = duty_from_outlet_temperature(h1, kgs(3.0), eta(0.8), t_k(305.0), flash);
        assert_relative_eq!(q.get::<watt>(), 78_375.0, epsilon = 1e-9);

        let q_direct = duty_from_outlet_enthalpy(h1, flash(t_k(305.0)), kgs(3.0), eta(0.8));
        assert_relative_eq!(q.get::<watt>(), q_direct.get::<watt>(), epsilon = 1e-9);
    }

    /// V&V — Heater, degenerate zero-duty cases.
    ///
    /// **Methodology.** With `Q = 0` the heat-added enthalpy must be unchanged
    /// (`h2 = h1`); with `h2 = h1` the duty must be exactly zero; with
    /// `T2 = T1` the constant-Cp duty must be zero.
    ///
    /// **Results (2026-07-24).** All three return `0` (enthalpy unchanged /
    /// duty `0 W`) to `< 1e-12` absolute.
    #[test]
    fn zero_duty_degenerate() {
        let h1 = jkg(250_000.0);
        let h2 = outlet_enthalpy_heat_added(h1, w_(0.0), kgs(5.0), eta(1.0));
        assert_relative_eq!(h2.get::<joule_per_kilogram>(), 250_000.0, epsilon = 1e-12);

        let q = duty_from_outlet_enthalpy(h1, h1, kgs(5.0), eta(1.0));
        assert!(q.get::<watt>().abs() < 1e-12);

        let cp = SpecificHeatCapacity::new::<joule_per_kilogram_kelvin>(2000.0);
        let q_cp = duty_constant_cp(kgs(5.0), cp, t_k(400.0), t_k(400.0), eta(1.0));
        assert!(q_cp.get::<watt>().abs() < 1e-12);
    }

    /// V&V — pressure drop and temperature-change helpers.
    ///
    /// **Methodology.** `p2 = p1 - dp` (Heater.vb:458) with
    /// `p1 = 200000 Pa`, `dp = 50000 Pa` → `150000 Pa`; and `T2 = T1 + dT`
    /// (Heater.vb:561) with `T1 = 300 K`, `dT = +25 K` → `325 K`.
    ///
    /// **Results (2026-07-24).** `p2 = 150000 Pa`, `T2 = 325 K`, both exact to
    /// `< 1e-9` relative.
    #[test]
    fn pressure_drop_and_temperature_change() {
        let p2 = outlet_pressure(
            Pressure::new::<pascal>(200_000.0),
            Pressure::new::<pascal>(50_000.0),
        );
        assert_relative_eq!(p2.get::<pascal>(), 150_000.0, epsilon = 1e-9);

        let t2 = outlet_temperature_from_change(
            t_k(300.0),
            uom::si::f64::TemperatureInterval::new::<ti_kelvin>(25.0),
        );
        assert_relative_eq!(t2.get::<kelvin>(), 325.0, epsilon = 1e-9);
    }
}
