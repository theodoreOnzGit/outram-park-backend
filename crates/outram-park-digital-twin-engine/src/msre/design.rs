//! # MSRE published plant constants
//!
//! The MSRE design point and the coefficient sets the stepping model
//! ([`super::model`]) runs on. Every value carries its citation. Where a value
//! already lives in another workspace crate it is **re-exported or read from
//! there**, not copied, so the two cannot drift.
//!
//! | Quantity | Value | Source | Where it lives |
//! |---|---|---|---|
//! | Design thermal power | 10 MW | ORNL-TM-0728 Part I §2.4.1 | here |
//! | Fuel salt, reactor inlet / outlet | 1175 / 1225 °F | TM-0728 §2.4.1 (also Table 5.2) | here |
//! | Coolant salt, heat-exchanger inlet / outlet | 1025 / 1100 °F | TM-0728 §2.4.3 | here |
//! | Radiator air inlet / outlet | 100 / 300 °F | TM-0728 Table 8.2 | here |
//! | Radiator air flow | 200 000 cfm | TM-0728 Table 8.2 (§2.4.5) | here |
//! | Fuel pump flow | 1200 gpm | TM-0728 §2.4.2 | here |
//! | Coolant pump flow | 850 gpm | TM-0728 §2.4.4 (also §2.4.3) | here |
//! | λ_i, β'_i (six groups, β = 0.00666) | | ORNL-TM-1070 App. B p. 69 | [`LAMBDA`], [`BETA`] (`nee_soon`) |
//! | Λ, core transit τ_c, loop transit τ_L, piping lags | 2.4e-4 s, 8.46 s, 16.73 s, 5.77/8.67/4.71/8.24 s | TM-1070 Table 1, App. B p. 70 | [`MsreLoopModel::tm1070`] (`nee_soon`) |
//! | Core lumps (τ_F, K_F, K_G, MC_F, MC_G, hA) | | TM-1070 App. B pp. 67-68 | [`TAU_F`] … [`HA`] (`nee_soon`) |
//! | Delayed-gamma share and lag | 0.0564, 188 s | TM-1070 Table 1 | [`GAMMA_SHARE`], [`GAMMA_LAG`] (`nee_soon`) |
//! | Heat exchanger and radiator lumps | | TM-1070 App. B p. 69 | [`HX_1965`], [`RADIATOR_1965_10MW`] (`nee_soon`) |
//! | Temperature coefficients (1965 prediction) | fuel −4.84, graphite −3.7 × 10⁻⁵ /°F | TM-1070 Table 1 | [`ORNL_1965_FUEL_PER_F`], [`ORNL_1965_GRAPHITE_PER_F`] (`nee_soon`) |
//! | Measured natural periods (7 powers) | | ORNL-TM-1647 Fig. 29, digitised | [`MEASURED_PERIODS_TM1647`] (`nee_soon`) |
//! | Measured circulation loss | 0.212 ± 0.004 % δk/k | ORNL-TM-1626 (the maintainer's choice of source for N-4, 2026-11-03) | here; also `teh-o-prke/tests/msre_n4_circulation_loss_tm1626.rs` |
//! | Salt properties | | TM-0728 Table 2.1 (fuel); ORNL-TM-2316 (coolant) | [`super::salts`] (`tuas_boussinesq_solver`) |
//!
//! TM-0728 values were read on 2026-11-10 from this workspace's OCR of the
//! open report (`crates/kovan-literature/generated/markdown/open/msre-design-and-operation.md`,
//! §2.4 and Table 8.2), not from the scan. **Not re-checked against the scan.**
//! TM-0728's Table 8.2 gives the radiator salt outlet as 1023 °F where §2.4.3
//! and §2.4.5 say 1025 °F; the section text is used here.
//!
//! **Design values are not measurements.** The MSRE ran at about 8 MW, not
//! the 10 MW design, and TM-0728's temperatures are the design heat balance.
//! The model's uncalibrated steady state is compared with them in
//! [`super::model`]'s tests, and the disagreement is recorded there.
//!
//! AI-drafted (Claude Opus 5.5 in Claude Code), untrusted until reviewed.
//! Research, education and V&V only.

use tuas_boussinesq_solver::boussinesq_thermophysical_properties::liquid_database::msre_coolant_salt_tm2316::{
    msre_coolant_salt_cp, msre_coolant_salt_rho,
};
use tuas_boussinesq_solver::boussinesq_thermophysical_properties::liquid_database::msre_fuel_salt_tm0728::{
    msre_fuel_salt_cp, msre_fuel_salt_rho,
};
use uom::si::f64::{Power, Ratio, ThermalConductance, ThermodynamicTemperature, VolumeRate};
use uom::si::power::megawatt;
use uom::si::ratio::percent;
use uom::si::thermal_conductance::watt_per_kelvin;
use uom::si::thermodynamic_temperature::degree_fahrenheit;
use uom::si::volume_rate::{cubic_foot_per_minute, gallon_per_minute};

pub use nee_soon::msre_frequency_response::{
    per_f, HxSection, MsreLoopModel, BETA, GAMMA_LAG, GAMMA_SHARE, HA, HX_1965, K_F, K_G, LAMBDA,
    MC_F, MC_G, MEASURED_PERIODS_TM1647, ORNL_1965_FUEL_PER_F, ORNL_1965_GRAPHITE_PER_F,
    RADIATOR_1965_10MW, TAU_F,
};

/// One MW/°F in W/K: 10⁶ W per 5/9 K.
pub const MW_PER_F_IN_W_PER_K: f64 = 1.0e6 * 1.8;

/// The MSRE design point, ORNL-TM-0728 Part I (Robertson, 1965). Plain data:
/// it holds no state and does no physics. Construct with
/// [`MsreDesignPoint::tm0728`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MsreDesignPoint {
    /// Design thermal power, 10 MW (§2.4.1).
    pub thermal_power: Power,
    /// Fuel salt entering the reactor vessel, 1175 °F (§2.4.1).
    pub fuel_reactor_inlet: ThermodynamicTemperature,
    /// Fuel salt leaving the reactor vessel, 1225 °F (§2.4.1).
    pub fuel_reactor_outlet: ThermodynamicTemperature,
    /// Coolant salt entering the heat exchanger tubes, 1025 °F (§2.4.3).
    pub coolant_hx_inlet: ThermodynamicTemperature,
    /// Coolant salt leaving the heat exchanger, 1100 °F (§2.4.3).
    pub coolant_hx_outlet: ThermodynamicTemperature,
    /// Radiator cooling air in, 100 °F (Table 8.2).
    pub air_inlet: ThermodynamicTemperature,
    /// Radiator cooling air out, 300 °F (Table 8.2; §2.4.5 gives the 200 °F rise).
    pub air_outlet: ThermodynamicTemperature,
    /// Radiator air flow at design power, 200 000 cfm (Table 8.2; two blowers, §2.4.5).
    pub air_flow: VolumeRate,
    /// Fuel pump flow, 1200 gpm (§2.4.2).
    pub fuel_flow: VolumeRate,
    /// Coolant pump flow, 850 gpm (§2.4.4).
    pub coolant_flow: VolumeRate,
}

impl MsreDesignPoint {
    /// The TM-0728 design point; see each field for its source.
    pub fn tm0728() -> Self {
        let f = ThermodynamicTemperature::new::<degree_fahrenheit>;
        Self {
            thermal_power: Power::new::<megawatt>(10.0),
            fuel_reactor_inlet: f(1175.0),
            fuel_reactor_outlet: f(1225.0),
            coolant_hx_inlet: f(1025.0),
            coolant_hx_outlet: f(1100.0),
            air_inlet: f(100.0),
            air_outlet: f(300.0),
            air_flow: VolumeRate::new::<cubic_foot_per_minute>(200_000.0),
            fuel_flow: VolumeRate::new::<gallon_per_minute>(1200.0),
            coolant_flow: VolumeRate::new::<gallon_per_minute>(850.0),
        }
    }

    /// The fuel stream's heat-capacity rate `ρ c_p Q` at the design flow, with
    /// the TUAS fuel salt (TM-0728 Table 2.1, 1200 °F; [`super::salts`]).
    /// Evaluated at the mean of the reactor inlet and outlet, inside the
    /// band TUAS accepts.
    pub fn fuel_heat_capacity_rate(&self) -> ThermalConductance {
        let t = mean(self.fuel_reactor_inlet, self.fuel_reactor_outlet);
        msre_fuel_salt_rho(t) * msre_fuel_salt_cp(t) * self.fuel_flow
    }

    /// The coolant stream's heat-capacity rate `ρ c_p Q` at the design flow,
    /// with the TUAS coolant salt (ORNL-TM-2316 correlations), at the mean of
    /// the heat-exchanger inlet and outlet.
    pub fn coolant_heat_capacity_rate(&self) -> ThermalConductance {
        let t = mean(self.coolant_hx_inlet, self.coolant_hx_outlet);
        msre_coolant_salt_rho(t) * msre_coolant_salt_cp(t) * self.coolant_flow
    }
}

/// The arithmetic mean of two temperatures.
fn mean(a: ThermodynamicTemperature, b: ThermodynamicTemperature) -> ThermodynamicTemperature {
    let k = |t: ThermodynamicTemperature| t.get::<degree_fahrenheit>();
    ThermodynamicTemperature::new::<degree_fahrenheit>(0.5 * (k(a) + k(b)))
}

/// The fuel heat-capacity rate implied by TM-1070's core lumps,
/// `MC_F / τ_F` (App. B pp. 67-68): each fuel lump holds `MC_F` and
/// is swept in `τ_F`.
pub fn tm1070_fuel_heat_capacity_rate() -> ThermalConductance {
    ThermalConductance::new::<watt_per_kelvin>(MC_F / TAU_F * MW_PER_F_IN_W_PER_K)
}

/// The **measured** reactivity lost to fuel circulation, 0.212 % δk/k
/// (U-235 loading, zero power), ORNL-TM-1626 (Prince, 1966); the maintainer's
/// choice of source for MSRE N-4 (gh:#1130, 2026-11-03).
pub fn measured_circulation_loss_tm1626() -> Ratio {
    Ratio::new::<percent>(0.212)
}

/// The stated uncertainty of [`measured_circulation_loss_tm1626`],
/// ±0.004 % δk/k (TM-1626).
pub fn measured_circulation_loss_uncertainty_tm1626() -> Ratio {
    Ratio::new::<percent>(0.004)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **Cross-check of two independent sources, TH-6 against T-1's lumps.**
    ///
    /// Methodology: the fuel heat-capacity rate `ρ c_p Q` from the TUAS fuel
    /// salt (TM-0728 Table 2.1: 134 lb/ft³, 0.47 Btu/lb·°F) at TM-0728's
    /// 1200 gpm, against TM-1070's `MC_F/τ_F` = 0.750/4.23 MW/°F.
    /// Prediction (gh:#1330, before running): within 2 %.
    ///
    /// Result (2026-11-10): TUAS 3.1978e5 W/K, TM-1070 3.1915e5 W/K,
    /// **+0.20 %**. The two agree; the criterion is the 2 % set before
    /// running. The coolant rate is printed for the record only (no TM-1070
    /// counterpart is in the lumps): TUAS's TM-2316 coolant gives 2.5240e5
    /// W/K at 850 gpm, a 71.3 °F rise at 10 MW against TM-0728's 75 °F
    /// (−4.9 %).
    #[test]
    fn the_tuas_fuel_salt_and_tm1070_lumps_agree_on_the_heat_capacity_rate() {
        let d = MsreDesignPoint::tm0728();
        let tuas = d.fuel_heat_capacity_rate().get::<watt_per_kelvin>();
        let tm1070 = tm1070_fuel_heat_capacity_rate().get::<watt_per_kelvin>();
        let coolant = d.coolant_heat_capacity_rate().get::<watt_per_kelvin>();
        let rise_f = 10.0e6 / coolant * 1.8;
        println!(
            "fuel W cp: TUAS {tuas:.4e} W/K, TM-1070 {tm1070:.4e} W/K ({:+.3} %); coolant {coolant:.4e} W/K, rise at 10 MW {rise_f:.2} F",
            100.0 * (tuas / tm1070 - 1.0)
        );
        assert!((tuas / tm1070 - 1.0).abs() < 0.02);
        assert!(coolant > 0.0);
    }

    /// The design point is transcribed as cited, and the measured loss and
    /// its uncertainty are the TM-1626 values.
    #[test]
    fn the_design_point_is_the_cited_one() {
        let d = MsreDesignPoint::tm0728();
        assert_eq!(d.thermal_power.get::<megawatt>(), 10.0);
        assert!((d.fuel_reactor_outlet.get::<degree_fahrenheit>() - 1225.0).abs() <= 1e-12 * 1225.0);
        assert!((d.air_inlet.get::<degree_fahrenheit>() - 100.0).abs() <= 1e-12 * 100.0);
        assert!((d.fuel_flow.get::<gallon_per_minute>() - 1200.0).abs() <= 1e-12 * 1200.0);
        assert!((measured_circulation_loss_tm1626().get::<percent>() - 0.212).abs() <= 1e-12 * 0.212);
        assert!((measured_circulation_loss_uncertainty_tm1626().get::<percent>() - 0.004).abs() <= 1e-12 * 0.004);
    }
}
