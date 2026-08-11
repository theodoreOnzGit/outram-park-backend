//! # Idealised helium circulator
//!
//! The pressure-raising machine that drives a gas-cooled reactor primary
//! circuit, modelled as an **idealisation** rather than from a
//! characteristic map.
//!
//! ## What belongs here / what does not
//!
//! - **Belongs:** the thermodynamics of a single-stage compression at a
//!   prescribed duty — the isentropic temperature rise, the efficiency
//!   correction to it, and the shaft power that follows.
//! - **Does NOT belong:** a real machine **characteristic map**
//!   (pressure rise vs. volumetric flow vs. shaft speed, with surge and
//!   choke limits), variable-speed control, or rotor dynamics. Those are
//!   **future work** — see the "Limitations" section below. Nor the loop
//!   resistance the circulator works against, which is
//!   [`super::pipe`] and [`super::kta_bed`].
//!
//! ## Formulation
//!
//! Given an inlet state `(T_1, p_1)` and an outlet pressure `p_2`, the
//! **isentropic** outlet state is the one at `p_2` with the same specific
//! entropy as the inlet, so the ideal specific work is
//! `w_s = h_2s - h_1`. The **actual** work follows from the isentropic
//! efficiency `eta_s`:
//!
//! ```text
//! w_actual = (h_2s - h_1) / eta_s
//! h_2      = h_1 + w_actual
//! T_2      = T(p_2, h_2)          (real-gas (p,h) flash)
//! P_shaft  = mdot * w_actual
//! ```
//!
//! All of it is done on **specific enthalpy** with real-gas `(p, s)` and
//! `(p, h)` flashes from the Helmholtz equation of state, not on an
//! ideal-gas `(p_2/p_1)^((gamma-1)/gamma)` shortcut. For helium at HTGR
//! conditions the two are close, but only because helium is nearly ideal
//! there — the shortcut is not assumed.
//!
//! The efficiency loss appears entirely as **extra enthalpy in the gas**
//! (an adiabatic machine); no heat is lost to the surroundings.
//!
//! ## Limitations (read before using a result)
//!
//! - **No characteristic map.** The duty is whatever the caller
//!   prescribes; the circulator cannot tell you whether a real machine
//!   could deliver that pressure rise at that flow, and it has no surge or
//!   choke limit. A real map, and the flow-vs-resistance intersection it
//!   would let you solve for, is **future work** and is not implemented.
//! - **No off-design efficiency.** `eta_s` is a constant the caller
//!   supplies, not a function of flow or speed.
//! - **Single stage, adiabatic.** No intercooling, no leakage, no bearing
//!   or windage losses beyond whatever the caller folds into `eta_s`.
//!
//! ## Status
//!
//! **NOT VALIDATED.** Checked against thermodynamic limits (the
//! `eta_s = 1` case must be exactly isentropic; the temperature rise must
//! grow as efficiency falls) and self-consistency only. AI-assisted draft
//! pending human review per `RESPONSIBLE_USE.md`.

use super::properties::{helium_state, helium_state_ph, HeliumState, SpecificEnthalpy};
use crate::TampinesError;
use outram_park_fork_coolprop::{state_ps, Fluid};
use uom::si::available_energy::joule_per_kilogram;
use uom::si::f64::{MassRate, Power, Pressure, Ratio, ThermodynamicTemperature};
use uom::si::mass_rate::kilogram_per_second;
use uom::si::power::watt;
use uom::si::pressure::pascal;
use uom::si::ratio::ratio;

/// What the circulator is asked to deliver.
///
/// Enum dispatch, not a trait object, per the workspace's mandatory "no
/// trait objects" Rust design rule.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CirculatorDuty {
    /// A prescribed **pressure rise** `p_2 - p_1`, Pa. Must be positive.
    PressureRise(Pressure),
    /// A prescribed **pressure ratio** `p_2 / p_1`, dimensionless. Must be
    /// greater than 1.
    PressureRatio(Ratio),
}

/// An idealised single-stage helium circulator: a duty and an isentropic
/// efficiency.
///
/// Plain data; the physics lives in [`Circulator::compress_helium`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Circulator {
    /// What the machine is asked to deliver.
    pub duty: CirculatorDuty,
    /// Isentropic (adiabatic) efficiency, dimensionless, in `(0, 1]`.
    /// A large helium circulator sits around 0.80-0.90; `1.0` gives the
    /// exactly-isentropic ideal machine.
    pub isentropic_efficiency: Ratio,
}

/// Everything [`Circulator::compress_helium`] computed.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CirculatorResult {
    /// Gas state at the circulator inlet.
    pub inlet: HeliumState,
    /// Gas state at the circulator outlet (real, efficiency-corrected).
    pub outlet: HeliumState,
    /// Outlet temperature the machine would reach if it were exactly
    /// isentropic, K — the lower bound on [`Self::outlet`]'s temperature.
    pub isentropic_outlet_temperature: ThermodynamicTemperature,
    /// Actual specific work put into the gas, J/kg.
    pub specific_work: SpecificEnthalpy,
    /// Ideal (isentropic) specific work, J/kg. Equals
    /// [`Self::specific_work`] times the efficiency.
    pub isentropic_specific_work: SpecificEnthalpy,
    /// Shaft power, W: `mdot` times [`Self::specific_work`].
    pub shaft_power: Power,
    /// Temperature rise across the machine, K.
    pub temperature_rise_kelvin: f64,
}

impl Circulator {
    /// A circulator delivering a fixed pressure rise at the given
    /// isentropic efficiency (dimensionless, in `(0, 1]`).
    pub fn new_fixed_pressure_rise(rise: Pressure, isentropic_efficiency: Ratio) -> Self {
        Self {
            duty: CirculatorDuty::PressureRise(rise),
            isentropic_efficiency,
        }
    }

    /// A circulator delivering a fixed pressure ratio at the given
    /// isentropic efficiency (dimensionless, in `(0, 1]`).
    pub fn new_fixed_pressure_ratio(ratio_pp: Ratio, isentropic_efficiency: Ratio) -> Self {
        Self {
            duty: CirculatorDuty::PressureRatio(ratio_pp),
            isentropic_efficiency,
        }
    }

    /// The outlet pressure this duty implies for the given inlet pressure.
    ///
    /// Errors with [`TampinesError::InvalidInput`] for a non-positive
    /// pressure rise or a pressure ratio at or below 1 (a circulator raises
    /// pressure; a machine that lowers it is a turbine and belongs in
    /// [`crate::components::turbine`]).
    pub fn outlet_pressure(&self, inlet_pressure: Pressure) -> Result<Pressure, TampinesError> {
        let p1 = inlet_pressure.get::<pascal>();
        match self.duty {
            CirculatorDuty::PressureRise(dp) => {
                let d = dp.get::<pascal>();
                if !d.is_finite() || d <= 0.0 {
                    return Err(TampinesError::InvalidInput(format!(
                        "circulator: pressure rise {d} Pa must be finite and positive"
                    )));
                }
                Ok(Pressure::new::<pascal>(p1 + d))
            }
            CirculatorDuty::PressureRatio(r) => {
                let rr = r.get::<ratio>();
                if !rr.is_finite() || rr <= 1.0 {
                    return Err(TampinesError::InvalidInput(format!(
                        "circulator: pressure ratio {rr} must be finite and greater than 1"
                    )));
                }
                Ok(Pressure::new::<pascal>(p1 * rr))
            }
        }
    }

    /// Compress helium from the given inlet state at the given mass flow.
    ///
    /// `mass_flow` must be positive. `inlet_temperature` and
    /// `inlet_pressure` must lie inside [`helium_state`]'s accepted
    /// envelope, as must the resulting outlet state.
    ///
    /// See the module docs for the formulation and, importantly, its
    /// limitations — in particular that this machine has **no
    /// characteristic map**, so it will happily report a duty no real
    /// circulator could deliver.
    ///
    /// Errors with [`TampinesError::InvalidInput`] for a non-positive mass
    /// flow or an efficiency outside `(0, 1]`, with
    /// [`TampinesError::Numerical`] if the isentropic `(p, s)` flash fails,
    /// and propagates [`helium_state`]'s errors.
    pub fn compress_helium(
        &self,
        mass_flow: MassRate,
        inlet_temperature: ThermodynamicTemperature,
        inlet_pressure: Pressure,
    ) -> Result<CirculatorResult, TampinesError> {
        use uom::si::thermodynamic_temperature::kelvin;

        let mdot = mass_flow.get::<kilogram_per_second>();
        if !mdot.is_finite() || mdot <= 0.0 {
            return Err(TampinesError::InvalidInput(format!(
                "circulator: mass flow {mdot} kg/s must be finite and positive"
            )));
        }
        let eta = self.isentropic_efficiency.get::<ratio>();
        if !eta.is_finite() || eta <= 0.0 || eta > 1.0 {
            return Err(TampinesError::InvalidInput(format!(
                "circulator: isentropic efficiency {eta} must lie in (0, 1]"
            )));
        }

        let inlet = helium_state(inlet_temperature, inlet_pressure)?;
        let p2 = self.outlet_pressure(inlet_pressure)?;

        // Entropy of the inlet, then the isentropic state at p2.
        let s1 = outram_park_fork_coolprop::state_pt(
            Fluid::Helium,
            inlet_temperature.get::<kelvin>(),
            inlet_pressure.get::<pascal>(),
        )
        .map_err(|e| {
            TampinesError::Numerical(format!("circulator: inlet (p,T) flash failed: {e:?}"))
        })?
        .entropy;

        let ideal = state_ps(Fluid::Helium, p2.get::<pascal>(), s1).map_err(|e| {
            TampinesError::Numerical(format!(
                "circulator: isentropic (p,s) flash failed at p2 = {} Pa, s = {s1} J/(kg K): {e:?}",
                p2.get::<pascal>()
            ))
        })?;

        let h1 = inlet.specific_enthalpy.get::<joule_per_kilogram>();
        let w_ideal = ideal.enthalpy - h1;
        let w_actual = w_ideal / eta;
        let h2 = h1 + w_actual;

        let outlet = helium_state_ph(p2, SpecificEnthalpy::new::<joule_per_kilogram>(h2))?;

        Ok(CirculatorResult {
            inlet,
            outlet,
            isentropic_outlet_temperature: ThermodynamicTemperature::new::<kelvin>(
                ideal.temperature,
            ),
            specific_work: SpecificEnthalpy::new::<joule_per_kilogram>(w_actual),
            isentropic_specific_work: SpecificEnthalpy::new::<joule_per_kilogram>(w_ideal),
            shaft_power: Power::new::<watt>(mdot * w_actual),
            temperature_rise_kelvin: outlet.temperature.get::<kelvin>()
                - inlet.temperature.get::<kelvin>(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gas_phase::properties::htr10_design_point;
    use uom::si::thermodynamic_temperature::kelvin;

    /// Print the circulator solution for an HTR-10-scale duty.
    /// Measurement harness for the V&V numbers recorded below
    /// (2026-08-11).
    #[test]
    fn measure_htr10_circulator() {
        for eta in [1.0_f64, 0.9, 0.8, 0.7] {
            let c = Circulator::new_fixed_pressure_rise(
                Pressure::new::<pascal>(6.0e4), // 60 kPa, an illustrative loop resistance
                Ratio::new::<ratio>(eta),
            );
            let r = c
                .compress_helium(
                    htr10_design_point::mass_flow_rate(),
                    htr10_design_point::core_inlet_temperature(),
                    htr10_design_point::pressure(),
                )
                .unwrap();
            println!(
                "eta = {eta:.2}: dT = {:.6} K (isentropic {:.6} K)  w = {:.3} J/kg  \
                 P_shaft = {:.3} kW  T_out = {:.4} K",
                r.temperature_rise_kelvin,
                r.isentropic_outlet_temperature.get::<kelvin>()
                    - r.inlet.temperature.get::<kelvin>(),
                r.specific_work.get::<joule_per_kilogram>(),
                r.shaft_power.get::<watt>() / 1000.0,
                r.outlet.temperature.get::<kelvin>(),
            );
        }
    }

    /// V&V — a perfectly efficient circulator must be exactly isentropic.
    ///
    /// **Methodology.** With `eta_s = 1` the actual outlet enthalpy equals
    /// the isentropic one by construction, so the actual and isentropic
    /// outlet temperatures must coincide. Pass criterion: the two agree to
    /// better than `1e-6 K`. Case: 4.3 kg/s helium, 3.0 MPa inlet at
    /// 523.15 K, 60 kPa rise.
    ///
    /// **Result (2026-08-11).** Residual `-1.137e-13 K`. PASSES —
    /// the machine-precision residual confirms the `(p,s)` and `(p,h)`
    /// flashes are mutually consistent.
    #[test]
    fn unit_efficiency_is_exactly_isentropic() {
        let c = Circulator::new_fixed_pressure_rise(
            Pressure::new::<pascal>(6.0e4),
            Ratio::new::<ratio>(1.0),
        );
        let r = c
            .compress_helium(
                htr10_design_point::mass_flow_rate(),
                htr10_design_point::core_inlet_temperature(),
                htr10_design_point::pressure(),
            )
            .unwrap();
        let residual =
            r.outlet.temperature.get::<kelvin>() - r.isentropic_outlet_temperature.get::<kelvin>();
        println!("eta = 1: T_out - T_out,s = {residual:.3e} K");
        assert!(
            residual.abs() < 1e-6,
            "eta = 1 is not isentropic: residual {residual} K"
        );
    }

    /// V&V — thermodynamic monotonicity. A less efficient machine must put
    /// more work into the gas for the same pressure rise, and therefore
    /// reach a higher outlet temperature; and the real outlet temperature
    /// can never fall below the isentropic one.
    ///
    /// **Methodology.** Sweep `eta_s` over 1.0, 0.9, 0.8, 0.7 at the same
    /// duty (4.3 kg/s, 3.0 MPa, 523.15 K, 60 kPa rise) and check strict
    /// monotonicity of the temperature rise and the shaft power, plus
    /// `T_out >= T_out,isentropic` throughout.
    ///
    /// **Results (2026-08-11).** The isentropic rise is `4.156139 K`
    /// throughout. Actual temperature rise and shaft power:
    /// `eta = 1.00` -> `4.156139 K`, `21768.662 J/kg`, `93.605 kW`;
    /// `eta = 0.90` -> `4.622039 K`, `24187.402 J/kg`, `104.006 kW`;
    /// `eta = 0.80` -> `5.204415 K`, `27210.828 J/kg`, `117.007 kW`;
    /// `eta = 0.70` -> `5.953184 K`, `31098.089 J/kg`, `133.722 kW`.
    /// Strictly monotone in both, and `T_out >= T_out,s` throughout.
    /// PASSES.
    ///
    /// **Interpretation.** A 60 kPa rise on 4.3 kg/s of helium costs about
    /// 100 kW of shaft power and heats the gas by only ~5 K, which is
    /// under 1 % of the 450 K core rise — the circulator is a small
    /// thermal perturbation on the HTR-10 primary circuit, though not a
    /// negligible parasitic load against 10 MW thermal (~1 %).
    #[test]
    fn lower_efficiency_means_more_work_and_more_heating() {
        let mut previous: Option<(f64, f64)> = None;
        for eta in [1.0_f64, 0.9, 0.8, 0.7] {
            let c = Circulator::new_fixed_pressure_rise(
                Pressure::new::<pascal>(6.0e4),
                Ratio::new::<ratio>(eta),
            );
            let r = c
                .compress_helium(
                    htr10_design_point::mass_flow_rate(),
                    htr10_design_point::core_inlet_temperature(),
                    htr10_design_point::pressure(),
                )
                .unwrap();
            assert!(
                r.outlet.temperature.get::<kelvin>()
                    >= r.isentropic_outlet_temperature.get::<kelvin>() - 1e-9,
                "eta = {eta}: real outlet is colder than isentropic"
            );
            assert!(r.temperature_rise_kelvin > 0.0);
            assert!(r.shaft_power.get::<watt>() > 0.0);
            if let Some((prev_dt, prev_p)) = previous {
                assert!(
                    r.temperature_rise_kelvin > prev_dt,
                    "eta = {eta}: dT {} not greater than previous {prev_dt}",
                    r.temperature_rise_kelvin
                );
                assert!(r.shaft_power.get::<watt>() > prev_p);
            }
            previous = Some((r.temperature_rise_kelvin, r.shaft_power.get::<watt>()));
        }
    }

    /// The two duty forms must agree when they describe the same outlet
    /// pressure.
    #[test]
    fn pressure_rise_and_pressure_ratio_duties_agree() {
        let p1 = htr10_design_point::pressure();
        let dp = 6.0e4_f64;
        let by_rise = Circulator::new_fixed_pressure_rise(
            Pressure::new::<pascal>(dp),
            Ratio::new::<ratio>(0.85),
        );
        let by_ratio = Circulator::new_fixed_pressure_ratio(
            Ratio::new::<ratio>((p1.get::<pascal>() + dp) / p1.get::<pascal>()),
            Ratio::new::<ratio>(0.85),
        );
        let a = by_rise
            .compress_helium(
                htr10_design_point::mass_flow_rate(),
                htr10_design_point::core_inlet_temperature(),
                p1,
            )
            .unwrap();
        let b = by_ratio
            .compress_helium(
                htr10_design_point::mass_flow_rate(),
                htr10_design_point::core_inlet_temperature(),
                p1,
            )
            .unwrap();
        let d = (a.temperature_rise_kelvin - b.temperature_rise_kelvin).abs();
        println!(
            "rise-duty dT = {:.9} K, ratio-duty dT = {:.9} K, diff = {d:.3e}",
            a.temperature_rise_kelvin, b.temperature_rise_kelvin
        );
        assert!(d < 1e-9, "duty forms disagree by {d} K");
    }

    /// Guard behaviour: bad efficiency, bad flow and a non-raising duty are
    /// rejected.
    #[test]
    fn invalid_inputs_are_rejected() {
        let t = htr10_design_point::core_inlet_temperature();
        let p = htr10_design_point::pressure();
        let mdot = htr10_design_point::mass_flow_rate();

        let bad_eta = Circulator::new_fixed_pressure_rise(
            Pressure::new::<pascal>(6.0e4),
            Ratio::new::<ratio>(1.5),
        );
        assert!(bad_eta.compress_helium(mdot, t, p).is_err());

        let zero_eta = Circulator::new_fixed_pressure_rise(
            Pressure::new::<pascal>(6.0e4),
            Ratio::new::<ratio>(0.0),
        );
        assert!(zero_eta.compress_helium(mdot, t, p).is_err());

        let good = Circulator::new_fixed_pressure_rise(
            Pressure::new::<pascal>(6.0e4),
            Ratio::new::<ratio>(0.85),
        );
        assert!(good
            .compress_helium(MassRate::new::<kilogram_per_second>(0.0), t, p)
            .is_err());

        let falling = Circulator::new_fixed_pressure_ratio(
            Ratio::new::<ratio>(0.9),
            Ratio::new::<ratio>(0.85),
        );
        assert!(falling.compress_helium(mdot, t, p).is_err());
    }
}
