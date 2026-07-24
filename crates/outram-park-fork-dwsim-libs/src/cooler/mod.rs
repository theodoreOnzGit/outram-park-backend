//! Cooler: enthalpy-driven cooling duty and outlet-state relations.
//!
//! Ported from DWSIM `UnitOperations/Cooler.vb` (GPL-3.0), the
//! `Public Overrides Sub Calculate` routine (Cooler.vb:~415-720). The cooler
//! is the near-mirror of the heater ([`crate::heater`]): the same energy
//! balance with the duty sign flipped. As with the heater, this crate has no
//! property-package / flash access, so flash-dependent steps are pushed to the
//! caller — specific enthalpies / mass flow / Cp are taken as inputs, and the
//! rigorous "outlet temperature given" path takes a caller-supplied flash
//! closure (a generic `Fn`, not a trait object, per the workspace no-`dyn`
//! rule).
//!
//! # Sign convention
//!
//! A **cooler removes duty**: the reported heat duty `Q` is positive for heat
//! *removed* from the stream, and removing heat lowers the specific enthalpy,
//! so the outlet enthalpy `h2 <= h1` for `Q >= 0`. This mirrors DWSIM, whose
//! cooler balance is `H2 = -Q*(eta/100)/W + H1` (Cooler.vb:498) — the leading
//! minus is the only structural difference from the heater
//! ([`crate::heater::outlet_enthalpy_heat_added`], Heater.vb:475). Likewise the
//! duty from an outlet spec is `Q = -w*(h2-h1)/eta` (Cooler.vb:561), positive
//! when `h2 < h1`.
//!
//! # Efficiency
//!
//! Identical semantics to the heater: DWSIM's percentage `Eficiencia`
//! (default 100) is taken here as a dimensionless [`Ratio`] (`1.0` = 100%), the
//! fraction of the utility duty coupled to the fluid. `efficiency = 0` is
//! degenerate (no heat removed in the duty-given path; non-finite duty in the
//! spec-given path).
//!
//! # Pressure drop & temperature change
//!
//! `p2 = p1 - dp` (Cooler.vb:436) and `T2 = T1 + dT` (Cooler.vb:587) are
//! identical to the heater's, so they are re-exported from [`crate::heater`]
//! rather than duplicated: see [`outlet_pressure`] and
//! [`outlet_temperature_from_change`]. This is the only cross-file dependency;
//! the duty/enthalpy math below is cooler-specific (opposite sign).
//!
//! # Excluded DWSIM behaviour
//!
//! As with the heater, the GUI editor, XML/JSON serialization, property-grid
//! reflection, report builders, energy-stream flowsheet plumbing, and the
//! dynamic-mode accumulation model are NOT ported — they are application/solver
//! infrastructure, not the thermodynamic kernel. The `EnergyStream` and
//! `OutletVaporFraction` modes require an internal flash with no closed-form
//! duty relation and are left to the caller (`EnergyStream` is identical to
//! `HeatRemoved` once the duty is read from the energy stream — but note it
//! *adds* enthalpy in DWSIM, using the heater sign, Cooler.vb:460).

use uom::si::available_energy::joule_per_kilogram;
use uom::si::f64::{
    AvailableEnergy, MassRate, Power, Ratio, SpecificHeatCapacity, ThermodynamicTemperature,
};
use uom::si::mass_rate::kilogram_per_second;
use uom::si::power::watt;
use uom::si::ratio::ratio;
use uom::si::specific_heat_capacity::joule_per_kilogram_kelvin;
use uom::si::thermodynamic_temperature::kelvin;

// Pressure drop (`p2 = p1 - dp`, Cooler.vb:436) and temperature change
// (`T2 = T1 + dT`, Cooler.vb:587) are byte-identical to the heater's, so they
// are re-exported here instead of being duplicated (documented cross-file
// dependency on `crate::heater`).
pub use crate::heater::{outlet_pressure, outlet_temperature_from_change};

/// Outlet specific enthalpy for the **heat-removed** mode (duty given):
/// `h2 = h1 - Q * eta / w`.
///
/// Ported from Cooler.vb:498
/// (`H2 = -Me.DeltaQ * (Me.Eficiencia / 100) / Wi + Hi`). This is the exact
/// mirror of [`crate::heater::outlet_enthalpy_heat_added`] with the duty sign
/// flipped: a cooler *removes* duty, so `h2 <= h1` for `q >= 0`.
///
/// - `h1` — inlet specific enthalpy \[J/kg\].
/// - `q` — heat duty \[W\]; positive **removes** heat.
/// - `w` — mass flow rate \[kg/s\]; must be `> 0`.
/// - `efficiency` — fraction of duty coupled to the fluid, `1.0` = 100%.
pub fn outlet_enthalpy_heat_removed(
    h1: AvailableEnergy,
    q: Power,
    w: MassRate,
    efficiency: Ratio,
) -> AvailableEnergy {
    let h2 = h1.get::<joule_per_kilogram>()
        - q.get::<watt>() * efficiency.get::<ratio>() / w.get::<kilogram_per_second>();
    AvailableEnergy::new::<joule_per_kilogram>(h2)
}

/// Heat duty from a known outlet specific enthalpy (rigorous "outlet spec"
/// direction): `Q = -w * (h2 - h1) / eta`.
///
/// Ported from Cooler.vb:561
/// (`Me.DeltaQ = -(H2 - Hi) / (Me.Eficiencia / 100) * Wi`). The outlet enthalpy
/// `h2` comes from the caller's flash at the specified outlet temperature (or
/// vapour fraction); the same relation serves the `OutletTemperature` and
/// `TemperatureChange` modes (Cooler.vb:561 and 606). The leading minus makes
/// the reported duty positive when the stream is cooled (`h2 < h1`).
///
/// `efficiency = 0` gives a non-finite duty.
pub fn duty_from_outlet_enthalpy(
    h1: AvailableEnergy,
    h2: AvailableEnergy,
    w: MassRate,
    efficiency: Ratio,
) -> Power {
    let q = -w.get::<kilogram_per_second>() * (h2 - h1).get::<joule_per_kilogram>()
        / efficiency.get::<ratio>();
    Power::new::<watt>(q)
}

/// Heat duty for the "outlet temperature given" mode under a **constant-`Cp`
/// approximation**: `Q = -w * Cp * (T2 - T1) / eta`.
///
/// Port-added closed-form stand-in for the rigorous PT-flash path
/// (Cooler.vb:557-561); DWSIM has no constant-`Cp` branch. Valid only where
/// `Cp` is sensibly constant over `[T1, T2]` (no phase change, mild range). For
/// the rigorous path use [`duty_from_outlet_temperature`]. Positive duty for
/// cooling (`T2 < T1`), matching the cooler sign convention.
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
    let q = -w.get::<kilogram_per_second>() * cp.get::<joule_per_kilogram_kelvin>() * dt
        / efficiency.get::<ratio>();
    Power::new::<watt>(q)
}

/// Heat duty for the "outlet temperature given" mode, **rigorous** path: the
/// caller supplies the outlet enthalpy at `t2` via a flash closure.
///
/// Mirrors Cooler.vb:557-561, where DWSIM does a PT flash at `(P2, T2)` to get
/// `H2` and then `Me.DeltaQ = -(H2 - Hi) / (eta/100) * Wi`. `outlet_enthalpy_at`
/// is the caller's PT-flash step (a generic `Fn`, no `dyn`): given the
/// specified outlet temperature it returns the outlet specific enthalpy `h2`.
/// The returned duty equals [`duty_from_outlet_enthalpy`] at the flashed `h2`.
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

    /// V&V — Cooler, heat-removed mode, outlet enthalpy (opposite sign to the
    /// heater).
    ///
    /// **Methodology.** Port of Cooler.vb:498 `H2 = -Q*(eta/100)/W + H1`. Hand
    /// value with `h1 = 100000 J/kg`, `Q = 50000 W`, `w = 10 kg/s`,
    /// `eta = 1.0`: `h2 = 100000 - 50000/10 = 95000 J/kg`. This is the exact
    /// mirror of the heater's `105000 J/kg` for the same inputs.
    ///
    /// **Results (2026-07-24).** Computed `h2 = 95000 J/kg` to `< 1e-9`
    /// relative, and `h2 < h1` — confirms the cooler removes enthalpy, opposite
    /// to the heater.
    #[test]
    fn heat_removed_outlet_enthalpy() {
        let h2 = outlet_enthalpy_heat_removed(jkg(100_000.0), w_(50_000.0), kgs(10.0), eta(1.0));
        assert_relative_eq!(h2.get::<joule_per_kilogram>(), 95_000.0, epsilon = 1e-9);
        assert!(h2.get::<joule_per_kilogram>() < 100_000.0, "cooler must lower enthalpy");

        // Explicit opposite-sign check versus the heater on identical inputs.
        let h2_heater = crate::heater::outlet_enthalpy_heat_added(
            jkg(100_000.0),
            w_(50_000.0),
            kgs(10.0),
            eta(1.0),
        );
        assert_relative_eq!(
            h2.get::<joule_per_kilogram>() - 100_000.0,
            -(h2_heater.get::<joule_per_kilogram>() - 100_000.0),
            epsilon = 1e-9
        );
    }

    /// V&V — Cooler, outlet-temperature mode, duty via constant-Cp.
    ///
    /// **Methodology.** Port-added constant-`Cp` form `Q = -w*Cp*(T2-T1)/eta`
    /// (stand-in for the PT-flash of Cooler.vb:557-561). Hand value with
    /// `w = 2 kg/s`, `Cp = 4180 J/(kg·K)`, `T1 = 310 K`, `T2 = 300 K`
    /// (cooling), `eta = 1.0`: `Q = -2*4180*(300-310) = +83600 W`.
    ///
    /// **Results (2026-07-24).** Computed `Q = 83600 W` to `< 1e-9` relative,
    /// **positive** because the duty is reported positive for heat removed.
    #[test]
    fn outlet_temperature_duty_constant_cp() {
        let cp = SpecificHeatCapacity::new::<joule_per_kilogram_kelvin>(4180.0);
        let q = duty_constant_cp(kgs(2.0), cp, t_k(310.0), t_k(300.0), eta(1.0));
        assert_relative_eq!(q.get::<watt>(), 83_600.0, epsilon = 1e-9);
        assert!(q.get::<watt>() > 0.0, "cooling reports positive removed duty");
    }

    /// V&V — Cooler, rigorous outlet-temperature path agrees with the direct
    /// enthalpy-balance duty.
    ///
    /// **Methodology.** [`duty_from_outlet_temperature`] with a toy flash
    /// closure `h2(T) = h1 + 4180*(T - 310)`. With `h1 = 1.0e6 J/kg`,
    /// `w = 3 kg/s`, `T2 = 305 K`, `eta = 0.8`:
    /// `h2 = 1.0e6 + 4180*(-5) = 979_100 J/kg`, so
    /// `Q = -3*(979_100-1_000_000)/0.8 = +78_375 W`.
    ///
    /// **Results (2026-07-24).** Closure path and [`duty_from_outlet_enthalpy`]
    /// both give `+78375 W` to `< 1e-9` relative.
    #[test]
    fn outlet_temperature_duty_rigorous_closure() {
        let h1 = jkg(1_000_000.0);
        let flash =
            |t: ThermodynamicTemperature| jkg(1_000_000.0 + 4180.0 * (t.get::<kelvin>() - 310.0));
        let q = duty_from_outlet_temperature(h1, kgs(3.0), eta(0.8), t_k(305.0), flash);
        assert_relative_eq!(q.get::<watt>(), 78_375.0, epsilon = 1e-9);

        let q_direct = duty_from_outlet_enthalpy(h1, flash(t_k(305.0)), kgs(3.0), eta(0.8));
        assert_relative_eq!(q.get::<watt>(), q_direct.get::<watt>(), epsilon = 1e-9);
    }

    /// V&V — Cooler, degenerate zero-duty cases.
    ///
    /// **Methodology.** `Q = 0` leaves `h2 = h1`; `h2 = h1` gives duty `0`;
    /// `T2 = T1` gives constant-Cp duty `0`.
    ///
    /// **Results (2026-07-24).** All three return `0` (enthalpy unchanged /
    /// duty `0 W`) to `< 1e-12` absolute.
    #[test]
    fn zero_duty_degenerate() {
        let h1 = jkg(250_000.0);
        let h2 = outlet_enthalpy_heat_removed(h1, w_(0.0), kgs(5.0), eta(1.0));
        assert_relative_eq!(h2.get::<joule_per_kilogram>(), 250_000.0, epsilon = 1e-12);

        let q = duty_from_outlet_enthalpy(h1, h1, kgs(5.0), eta(1.0));
        assert!(q.get::<watt>().abs() < 1e-12);

        let cp = SpecificHeatCapacity::new::<joule_per_kilogram_kelvin>(2000.0);
        let q_cp = duty_constant_cp(kgs(5.0), cp, t_k(400.0), t_k(400.0), eta(1.0));
        assert!(q_cp.get::<watt>().abs() < 1e-12);
    }
}
