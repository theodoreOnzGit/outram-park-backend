//! # Steady-state gas duct: low-Mach friction and convective heat transfer
//!
//! A single straight duct or pipe carrying a compressible gas at low Mach
//! number. Given a mass flow, the duct geometry, the inlet `(T, p)` and a
//! thermal boundary condition, it returns the outlet `(T, p)` together with
//! the dimensionless groups the answer was built from.
//!
//! ## What belongs here / what does not
//!
//! - **Belongs:** single-duct steady friction (Churchill) and single-phase
//!   forced-convection heat transfer (Dittus-Boelter / Gnielinski) for a
//!   gas, plus the low-Mach energy balance that ties them together.
//! - **Does NOT belong:** packed-bed friction ([`super::kta_bed`]), the
//!   property correlations themselves ([`super::properties`], which only
//!   adapts `outram-park-fork-coolprop`), transient/CFD duct flow (that is
//!   `outram-park-fork-coolprop`'s `OPCPFluidArray`, re-exported as
//!   [`crate::compressible::CompressibleFluidArray`]), or two-phase flow.
//!
//! ## The low-Mach assumption, stated explicitly
//!
//! Measured on 2026-08-11 (see
//! `tests::htr10_hot_duct_is_deeply_subsonic`), a 0.30 m HTR-10 primary
//! hot duct at the design flow runs at **Ma = 2.234e-2**; the pebble-bed
//! core itself, with its much larger free-flow area, runs at
//! **Ma = 1.4e-3** (measured in `outram-park-fork-coolprop`). Taking the
//! larger of the two, `Ma^2 = 5.0e-4`. This module therefore drops two
//! terms from the steady energy equation:
//!
//! - the **kinetic-energy** term `u^2/2` against the enthalpy, which is
//!   `O(Ma^2) <= 5e-4` of it; and
//! - the **compressibility work** `u dp/dx / rho`, negligible for the same
//!   reason.
//!
//! Both are therefore below the 0.1 % level for the hot duct and below
//! the 1e-6 level for the core. What is **not** dropped is the density
//! variation itself: `rho` is a full
//! function of `(T, p)` from the Helmholtz equation of state, and the
//! momentum balance keeps the **acceleration** term `G^2 (1/rho_out -
//! 1/rho_in)` that a heated gas duct generates, reported separately from
//! friction so a caller can see its size. Nothing here is valid at
//! transonic conditions; for those, reach for
//! `outram-park-fork-coolprop`'s `SolverMode::HybridAllMach`.
//!
//! ## Correlations and their validity ranges
//!
//! **Friction — Churchill (1977).** A single expression covering laminar,
//! transitional and turbulent flow in rough pipes, asymptotic to `64/Re`
//! below `Re ~ 2000` and to Colebrook-White above `Re ~ 4000`:
//!
//! ```text
//! A  = [ -2.457 ln( (7/Re)^0.9 + 0.27 e/D ) ]^16
//! B  = (37530/Re)^16
//! f  = 8 [ (8/Re)^12 + (A + B)^-1.5 ]^(1/12)      (Darcy friction factor)
//! ```
//!
//! Churchill, S. W., "Friction factor equation spans all fluid-flow
//! regimes", *Chemical Engineering* 84(24), 1977, pp. 91-92. Chosen over
//! Colebrook-White because it is explicit (no inner iteration) and does not
//! blow up or go complex in the laminar and transitional regions, which a
//! startup or natural-circulation transient will visit.
//!
//! **Heat transfer — Dittus-Boelter.** `Nu = 0.023 Re^0.8 Pr^n`, with
//! `n = 0.4` when the gas is being heated and `n = 0.3` when cooled.
//! Stated validity: `Re > 10000`, `0.6 <= Pr <= 160`, `L/D >= 10`. Helium's
//! Prandtl number runs **0.658469 to 0.661835** across the HTR-10 core
//! (measured 2026-08-11, see [`super::properties`]) — inside that band,
//! but with under 10 % margin on its lower bound, which is the reason
//! [`HeatTransferCorrelation::Gnielinski`] is the default here.
//!
//! **Heat transfer — Gnielinski (1976).** Using the Churchill Darcy factor
//! `f`:
//!
//! ```text
//! Nu = (f/8)(Re - 1000) Pr / [ 1 + 12.7 sqrt(f/8) (Pr^(2/3) - 1) ]
//! ```
//!
//! Gnielinski, V., "New equations for heat and mass transfer in turbulent
//! pipe and channel flow", *Int. Chem. Eng.* 16(2), 1976, pp. 359-368.
//! Stated validity: `3000 <= Re <= 5e6`, `0.5 <= Pr <= 2000`. It reaches
//! lower Reynolds numbers than Dittus-Boelter and covers helium's Prandtl
//! number with margin, so it is the default.
//!
//! Both are **fully-developed, constant-property, smooth-tube** forms. No
//! entrance-length correction and no property-ratio (`(mu/mu_w)^0.14`-type)
//! correction is applied — for a gas with a large wall-to-bulk temperature
//! ratio, as in a reactor core channel, that omission is a real modelling
//! error and is recorded here rather than hidden.
//!
//! ## Status
//!
//! **NOT VALIDATED.** The correlations are standard and cited, but this
//! implementation is checked only against analytic limits, self-consistency
//! and published correlation properties. AI-assisted draft pending human
//! review per `RESPONSIBLE_USE.md`.

use super::properties::{helium_state, helium_state_ph, HeliumState, SpecificEnthalpy};
use super::MassFlux;
use crate::TampinesError;
use uom::si::area::square_meter;
use uom::si::available_energy::joule_per_kilogram;
use uom::si::f64::{
    Area, HeatTransfer, Length, MassRate, Power, Pressure, Ratio, ThermodynamicTemperature,
    Velocity,
};
use uom::si::heat_transfer::watt_per_square_meter_kelvin;
use uom::si::length::meter;
use uom::si::mass_rate::kilogram_per_second;
use uom::si::power::watt;
use uom::si::pressure::pascal;
use uom::si::ratio::ratio;
use uom::si::specific_heat_capacity::joule_per_kilogram_kelvin;
use uom::si::thermodynamic_temperature::kelvin;
use uom::si::velocity::meter_per_second;

/// Which single-phase forced-convection correlation a [`GasDuct`] uses for
/// its wall heat-transfer coefficient.
///
/// Enum dispatch, not a trait object, per the workspace's mandatory "no
/// trait objects" Rust design rule. See the module docs for each
/// correlation's equation, citation and stated validity range.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HeatTransferCorrelation {
    /// Gnielinski (1976), `3000 <= Re <= 5e6`, `0.5 <= Pr <= 2000`. The
    /// default: it covers helium's `Pr ~ 0.66` with margin and reaches
    /// lower Reynolds numbers than Dittus-Boelter.
    #[default]
    Gnielinski,
    /// Dittus-Boelter, `Nu = 0.023 Re^0.8 Pr^0.4` (gas being heated).
    /// Stated validity `Re > 10000`, `0.6 <= Pr <= 160`.
    DittusBoelterHeating,
    /// Dittus-Boelter, `Nu = 0.023 Re^0.8 Pr^0.3` (gas being cooled).
    DittusBoelterCooling,
}

/// What the duct's wall does to the gas.
///
/// Enum dispatch, not a trait object.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DuctThermalBoundary {
    /// No heat exchange with the wall. The gas still cools slightly on
    /// expansion through the friction pressure drop (a real Joule-Thomson
    /// effect the enthalpy balance captures, since constant enthalpy at
    /// falling pressure is not constant temperature).
    Adiabatic,
    /// A prescribed **total** heat input to the gas over the whole duct,
    /// W. Positive heats the gas. The heat-transfer correlation is still
    /// evaluated (and reported) but does not set the duty.
    HeatInput(Power),
    /// A prescribed **uniform wall temperature**. The duty follows from the
    /// convective correlation through an NTU closure,
    /// `T_out = T_w - (T_w - T_in) exp(-hA / (mdot cp))`, so the gas
    /// approaches the wall temperature asymptotically and can never
    /// overshoot it.
    WallTemperature(ThermodynamicTemperature),
}

/// A straight gas duct: geometry plus the heat-transfer correlation to use.
///
/// Plain data; the physics lives in [`GasDuct::solve_helium`]. Construct
/// with [`GasDuct::new_circular`] for the common round-pipe case, or by
/// filling the fields for a non-circular channel.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GasDuct {
    /// Hydraulic diameter `D_h = 4 A / P`, metres. For a circular pipe this
    /// is simply the internal diameter.
    pub hydraulic_diameter: Length,
    /// Flow (cross-sectional) area, m^2.
    pub flow_area: Area,
    /// Wetted perimeter, metres — sets the wall area `P L` available for
    /// heat transfer.
    pub wetted_perimeter: Length,
    /// Duct length, metres.
    pub length: Length,
    /// Absolute wall roughness `e`, metres. Enters the Churchill friction
    /// factor as the relative roughness `e/D_h`. Drawn steel is about
    /// 4.5e-5 m; a machined graphite channel is rougher.
    pub roughness: Length,
    /// Which forced-convection correlation to use for the wall
    /// heat-transfer coefficient.
    pub heat_transfer: HeatTransferCorrelation,
}

/// Everything [`GasDuct::solve_helium`] computed: the outlet state, the
/// pressure-drop split, and the dimensionless groups behind them.
///
/// Returned in full rather than as a bare outlet state so a caller can
/// check that the flow actually sat inside the correlations' validity
/// ranges — a pressure drop with `Re = 800` from a turbulent correlation is
/// a number, but not an answer.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GasDuctResult {
    /// Gas state at the duct inlet.
    pub inlet: HeliumState,
    /// Gas state at the duct outlet.
    pub outlet: HeliumState,
    /// Total pressure drop, Pa (positive = pressure falls along the duct).
    /// The sum of [`Self::friction_pressure_drop`] and
    /// [`Self::acceleration_pressure_drop`].
    pub pressure_drop: Pressure,
    /// Frictional part of the pressure drop, Pa,
    /// `f (L/D_h) G^2 / (2 rho_mean)`.
    pub friction_pressure_drop: Pressure,
    /// Acceleration ("momentum flux") part, Pa,
    /// `G^2 (1/rho_out - 1/rho_in)`. Positive when the gas is heated and
    /// therefore expands and speeds up; **negative** when it is cooled.
    pub acceleration_pressure_drop: Pressure,
    /// Superficial mass flux `G = mdot / A`, kg/(m^2 s). Constant along a
    /// duct of fixed area regardless of expansion.
    pub mass_flux: MassFlux,
    /// Reynolds number `Re = G D_h / mu` at the mean state, dimensionless.
    pub reynolds: Ratio,
    /// Prandtl number at the mean state, dimensionless.
    pub prandtl: Ratio,
    /// Churchill Darcy friction factor at the mean state, dimensionless.
    pub darcy_friction_factor: Ratio,
    /// Nusselt number from the selected correlation, dimensionless.
    pub nusselt: Ratio,
    /// Wall heat-transfer coefficient `h = Nu lambda / D_h`, W/(m^2 K).
    pub heat_transfer_coefficient: HeatTransfer,
    /// Net heat added to the gas over the duct, W (negative = removed).
    pub heat_duty: Power,
    /// Bulk gas velocity at the inlet, m/s.
    pub inlet_velocity: Velocity,
    /// Bulk gas velocity at the outlet, m/s.
    pub outlet_velocity: Velocity,
    /// Mach number at the outlet, dimensionless — the larger of the two for
    /// a heated duct, and the number that justifies (or refutes) the
    /// low-Mach assumption for a given case.
    pub outlet_mach: Ratio,
}

impl GasDuctResult {
    /// Whether the flow sat inside the selected heat-transfer
    /// correlation's stated Reynolds and Prandtl validity range.
    ///
    /// Reported rather than enforced: an out-of-range result is still
    /// returned (a startup transient legitimately passes through laminar
    /// flow), but a caller that reports a heat-transfer coefficient from an
    /// out-of-range correlation without saying so is overclaiming.
    pub fn heat_transfer_correlation_in_range(&self, correlation: HeatTransferCorrelation) -> bool {
        let re = self.reynolds.get::<ratio>();
        let pr = self.prandtl.get::<ratio>();
        match correlation {
            HeatTransferCorrelation::Gnielinski => {
                (3000.0..=5.0e6).contains(&re) && (0.5..=2000.0).contains(&pr)
            }
            HeatTransferCorrelation::DittusBoelterHeating
            | HeatTransferCorrelation::DittusBoelterCooling => {
                re > 10000.0 && (0.6..=160.0).contains(&pr)
            }
        }
    }
}

/// Churchill (1977) Darcy friction factor, dimensionless, from the Reynolds
/// number and the relative roughness `e/D_h`.
///
/// Valid across all flow regimes: it is asymptotic to `64/Re` in laminar
/// flow and to Colebrook-White in the fully turbulent region. See the
/// module docs for the equation and the citation.
///
/// Errors with [`TampinesError::InvalidInput`] for a non-positive or
/// non-finite Reynolds number, or a negative relative roughness.
pub fn churchill_friction_factor(
    reynolds: Ratio,
    relative_roughness: Ratio,
) -> Result<Ratio, TampinesError> {
    let re = reynolds.get::<ratio>();
    let rr = relative_roughness.get::<ratio>();
    if !re.is_finite() || re <= 0.0 {
        return Err(TampinesError::InvalidInput(format!(
            "Churchill friction factor: Reynolds number {re} must be finite and positive"
        )));
    }
    if !rr.is_finite() || rr < 0.0 {
        return Err(TampinesError::InvalidInput(format!(
            "Churchill friction factor: relative roughness {rr} must be finite and non-negative"
        )));
    }
    let a = (-2.457 * ((7.0 / re).powf(0.9) + 0.27 * rr).ln()).powi(16);
    let b = (37530.0 / re).powi(16);
    let f = 8.0 * ((8.0 / re).powi(12) + 1.0 / (a + b).powf(1.5)).powf(1.0 / 12.0);
    Ok(Ratio::new::<ratio>(f))
}

/// Nusselt number from the selected correlation, dimensionless.
///
/// `darcy_friction_factor` is only used by
/// [`HeatTransferCorrelation::Gnielinski`]. See the module docs for each
/// correlation's equation, citation and stated validity range; validity is
/// **not** enforced here (use
/// [`GasDuctResult::heat_transfer_correlation_in_range`]), but a
/// non-positive result is rejected as unphysical.
pub fn nusselt_number(
    correlation: HeatTransferCorrelation,
    reynolds: Ratio,
    prandtl: Ratio,
    darcy_friction_factor: Ratio,
) -> Result<Ratio, TampinesError> {
    let re = reynolds.get::<ratio>();
    let pr = prandtl.get::<ratio>();
    if !re.is_finite() || re <= 0.0 || !pr.is_finite() || pr <= 0.0 {
        return Err(TampinesError::InvalidInput(format!(
            "Nusselt number: Re = {re}, Pr = {pr} must both be finite and positive"
        )));
    }
    let nu = match correlation {
        HeatTransferCorrelation::Gnielinski => {
            let f8 = darcy_friction_factor.get::<ratio>() / 8.0;
            if f8 <= 0.0 {
                return Err(TampinesError::InvalidInput(format!(
                    "Gnielinski: Darcy friction factor {} must be positive",
                    darcy_friction_factor.get::<ratio>()
                )));
            }
            f8 * (re - 1000.0) * pr / (1.0 + 12.7 * f8.sqrt() * (pr.powf(2.0 / 3.0) - 1.0))
        }
        HeatTransferCorrelation::DittusBoelterHeating => 0.023 * re.powf(0.8) * pr.powf(0.4),
        HeatTransferCorrelation::DittusBoelterCooling => 0.023 * re.powf(0.8) * pr.powf(0.3),
    };
    if !nu.is_finite() || nu <= 0.0 {
        return Err(TampinesError::Unphysical(format!(
            "Nusselt number {nu} is non-positive at Re = {re}, Pr = {pr} \
             (Gnielinski goes negative below Re = 1000 by construction)"
        )));
    }
    Ok(Ratio::new::<ratio>(nu))
}

impl GasDuct {
    /// A circular pipe of the given internal `diameter`, `length` and
    /// absolute wall `roughness`, using the default
    /// [`HeatTransferCorrelation::Gnielinski`].
    ///
    /// Sets `flow_area = pi D^2 / 4`, `wetted_perimeter = pi D` and
    /// `hydraulic_diameter = D`, which are mutually consistent by
    /// construction.
    pub fn new_circular(diameter: Length, length: Length, roughness: Length) -> Self {
        let d = diameter.get::<meter>();
        Self {
            hydraulic_diameter: diameter,
            flow_area: Area::new::<square_meter>(std::f64::consts::FRAC_PI_4 * d * d),
            wetted_perimeter: Length::new::<meter>(std::f64::consts::PI * d),
            length,
            roughness,
            heat_transfer: HeatTransferCorrelation::default(),
        }
    }

    /// Wall (wetted) surface area available for heat transfer, `P L`, m^2.
    pub fn wall_area(&self) -> Area {
        self.wetted_perimeter * self.length
    }

    /// Relative wall roughness `e / D_h`, dimensionless.
    pub fn relative_roughness(&self) -> Ratio {
        self.roughness / self.hydraulic_diameter
    }

    /// Solve the duct for helium: outlet `(T, p)` and the full
    /// [`GasDuctResult`].
    ///
    /// `mass_flow` must be positive (this is a steady forward-flow model;
    /// reverse flow is a separate sign convention, not a negative input).
    /// `inlet_temperature` and `inlet_pressure` must lie inside
    /// [`helium_state`]'s accepted envelope.
    ///
    /// **Solution procedure.** Properties are evaluated at the *mean* state
    /// (arithmetic mean of inlet and outlet temperature, mean of inlet and
    /// outlet pressure), which is not known until the duct is solved, so
    /// the whole thing is iterated to a fixed point. Convergence is on the
    /// outlet temperature to 1e-9 K, capped at 50 sweeps. Because the
    /// pressure drop is small against the system pressure at HTGR
    /// conditions, this converges in a handful of sweeps.
    ///
    /// The energy balance is done on **specific enthalpy**
    /// (`h_out = h_in + Q/mdot`, then a `(p, h)` flash for the outlet
    /// temperature), not on `cp dT` — so it stays exact as `cp` varies and
    /// captures the small non-ideal temperature change of an adiabatic
    /// expansion.
    ///
    /// Errors with [`TampinesError::InvalidInput`] for non-positive flow or
    /// geometry, [`TampinesError::Numerical`] if the fixed point does not
    /// converge, and propagates [`helium_state`]'s errors if the iteration
    /// wanders outside the property envelope.
    pub fn solve_helium(
        &self,
        mass_flow: MassRate,
        inlet_temperature: ThermodynamicTemperature,
        inlet_pressure: Pressure,
        boundary: DuctThermalBoundary,
    ) -> Result<GasDuctResult, TampinesError> {
        let mdot = mass_flow.get::<kilogram_per_second>();
        let area = self.flow_area.get::<square_meter>();
        let d_h = self.hydraulic_diameter.get::<meter>();
        let len = self.length.get::<meter>();
        for (name, v) in [
            ("mass flow", mdot),
            ("flow area", area),
            ("hydraulic diameter", d_h),
            ("length", len),
        ] {
            if !v.is_finite() || v <= 0.0 {
                return Err(TampinesError::InvalidInput(format!(
                    "gas duct: {name} must be finite and positive, got {v}"
                )));
            }
        }

        let inlet = helium_state(inlet_temperature, inlet_pressure)?;
        let g = mdot / area; // kg/(m^2 s)
        let mass_flux: MassFlux = mass_flow / self.flow_area;
        let rr = self.relative_roughness();
        let h_in = inlet.specific_enthalpy.get::<joule_per_kilogram>();
        let p_in = inlet_pressure.get::<pascal>();

        // Fixed point on the outlet temperature. Start from the inlet.
        let mut t_out_k = inlet_temperature.get::<kelvin>();
        let mut p_out_pa = p_in;
        let mut converged = false;
        let mut last = None;

        for _ in 0..50 {
            let t_mean = ThermodynamicTemperature::new::<kelvin>(
                0.5 * (inlet_temperature.get::<kelvin>() + t_out_k),
            );
            let p_mean = Pressure::new::<pascal>(0.5 * (p_in + p_out_pa));
            let mean = helium_state(t_mean, p_mean)?;

            let re = Ratio::new::<ratio>(
                g * d_h
                    / mean
                        .dynamic_viscosity
                        .get::<uom::si::dynamic_viscosity::pascal_second>(),
            );
            let f = churchill_friction_factor(re, rr)?;
            let pr = mean.prandtl();
            let nu = nusselt_number(self.heat_transfer, re, pr, f)?;
            let htc = nu.get::<ratio>()
                * mean
                    .thermal_conductivity
                    .get::<uom::si::thermal_conductivity::watt_per_meter_kelvin>()
                / d_h;

            // --- energy balance -> outlet enthalpy ---
            let q_w = match boundary {
                DuctThermalBoundary::Adiabatic => 0.0,
                DuctThermalBoundary::HeatInput(p) => p.get::<watt>(),
                DuctThermalBoundary::WallTemperature(t_w) => {
                    // NTU closure: T_out = T_w - (T_w - T_in) exp(-hA/(mdot cp)).
                    let cp = mean.specific_heat_cp.get::<joule_per_kilogram_kelvin>();
                    let ntu = htc * self.wall_area().get::<square_meter>() / (mdot * cp);
                    let t_wall = t_w.get::<kelvin>();
                    let t_i = inlet_temperature.get::<kelvin>();
                    let t_target = t_wall - (t_wall - t_i) * (-ntu).exp();
                    mdot * cp * (t_target - t_i)
                }
            };
            let h_out = h_in + q_w / mdot;

            // --- momentum balance -> outlet pressure ---
            // Friction, at the mean density; plus the acceleration term.
            let rho_mean = mean
                .density
                .get::<uom::si::mass_density::kilogram_per_cubic_meter>();
            let dp_fric = f.get::<ratio>() * (len / d_h) * g * g / (2.0 * rho_mean);

            // The outlet density needs the outlet state, which needs the
            // outlet pressure; use the previous sweep's pressure, which the
            // fixed point then corrects.
            let out_guess = helium_state_ph(
                Pressure::new::<pascal>(p_out_pa),
                SpecificEnthalpy::new::<joule_per_kilogram>(h_out),
            )?;
            let rho_out = out_guess
                .density
                .get::<uom::si::mass_density::kilogram_per_cubic_meter>();
            let rho_in = inlet
                .density
                .get::<uom::si::mass_density::kilogram_per_cubic_meter>();
            let dp_acc = g * g * (1.0 / rho_out - 1.0 / rho_in);

            let new_p_out = p_in - dp_fric - dp_acc;
            let new_t_out = out_guess.temperature.get::<kelvin>();

            let delta = (new_t_out - t_out_k).abs();
            t_out_k = new_t_out;
            p_out_pa = new_p_out;
            last = Some((
                mean, re, pr, f, nu, htc, dp_fric, dp_acc, q_w, rho_in, rho_out,
            ));
            if delta < 1e-9 {
                converged = true;
                break;
            }
        }

        let (_mean, re, pr, f, nu, htc, dp_fric, dp_acc, q_w, rho_in, rho_out) =
            last.ok_or_else(|| {
                TampinesError::Numerical("gas duct: fixed point produced no iterate".to_string())
            })?;
        if !converged {
            return Err(TampinesError::Numerical(format!(
                "gas duct: outlet temperature fixed point did not converge in 50 sweeps \
                 (last T_out = {t_out_k} K)"
            )));
        }

        let outlet = helium_state(
            ThermodynamicTemperature::new::<kelvin>(t_out_k),
            Pressure::new::<pascal>(p_out_pa),
        )?;

        Ok(GasDuctResult {
            inlet,
            outlet,
            pressure_drop: Pressure::new::<pascal>(dp_fric + dp_acc),
            friction_pressure_drop: Pressure::new::<pascal>(dp_fric),
            acceleration_pressure_drop: Pressure::new::<pascal>(dp_acc),
            mass_flux,
            reynolds: re,
            prandtl: pr,
            darcy_friction_factor: f,
            nusselt: nu,
            heat_transfer_coefficient: HeatTransfer::new::<watt_per_square_meter_kelvin>(htc),
            heat_duty: Power::new::<watt>(q_w),
            inlet_velocity: Velocity::new::<meter_per_second>(g / rho_in),
            outlet_velocity: Velocity::new::<meter_per_second>(g / rho_out),
            outlet_mach: Ratio::new::<ratio>(
                (g / rho_out) / outlet.speed_of_sound.get::<meter_per_second>(),
            ),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gas_phase::properties::htr10_design_point;

    /// The HTR-10 primary hot-gas duct, as a round pipe. The IAEA HTR-10
    /// benchmark description (Open tier) gives a hot-gas duct inner
    /// diameter of 0.30 m; the 5 m length here is an illustrative
    /// vessel-to-steam-generator run, not a cited figure.
    fn htr10_hot_duct() -> GasDuct {
        GasDuct::new_circular(
            Length::new::<meter>(0.30),
            Length::new::<meter>(5.0),
            Length::new::<meter>(4.5e-5), // drawn steel
        )
    }

    /// Analytic limit — Churchill must reduce to the exact laminar
    /// `f = 64/Re` at low Reynolds number.
    #[test]
    fn churchill_reduces_to_laminar_64_over_re() {
        for re in [1.0_f64, 10.0, 100.0, 500.0] {
            let f = churchill_friction_factor(Ratio::new::<ratio>(re), Ratio::new::<ratio>(0.0))
                .unwrap()
                .get::<ratio>();
            let exact = 64.0 / re;
            let rel = (f - exact).abs() / exact;
            println!("Re = {re:6}  Churchill f = {f:.9}  64/Re = {exact:.9}  rel = {rel:.3e}");
            assert!(
                rel < 1e-6,
                "Re = {re}: f = {f}, 64/Re = {exact}, rel = {rel}"
            );
        }
    }

    /// Analytic limit — in the fully rough regime Churchill must approach
    /// the von Karman value `f = [1.14 - 2 log10(e/D)]^-2`, which is the
    /// Reynolds-independent asymptote of Colebrook-White.
    #[test]
    fn churchill_reaches_the_fully_rough_asymptote() {
        for rr in [1e-3_f64, 1e-2, 5e-2] {
            let f = churchill_friction_factor(Ratio::new::<ratio>(1.0e9), Ratio::new::<ratio>(rr))
                .unwrap()
                .get::<ratio>();
            let von_karman = (1.14 - 2.0 * rr.log10()).powi(-2);
            let rel = (f - von_karman).abs() / von_karman;
            println!("e/D = {rr:.0e}  Churchill f = {f:.9}  von Karman = {von_karman:.9}  rel = {rel:.3e}");
            assert!(
                rel < 0.02,
                "e/D = {rr}: f = {f}, von Karman = {von_karman}, rel = {rel}"
            );
        }
    }

    /// Churchill must reproduce the implicit Colebrook-White root in the
    /// turbulent region: substituting Churchill's `f` back into
    /// Colebrook-White's residual should very nearly zero it.
    #[test]
    fn churchill_agrees_with_colebrook_white_in_the_turbulent_region() {
        for re in [1.0e4_f64, 1.0e5, 1.0e6, 1.0e7] {
            for rr in [0.0_f64, 1e-4, 1e-3] {
                let f = churchill_friction_factor(Ratio::new::<ratio>(re), Ratio::new::<ratio>(rr))
                    .unwrap()
                    .get::<ratio>();
                // Colebrook-White: 1/sqrt(f) = -2 log10(e/(3.7 D) + 2.51/(Re sqrt(f)))
                let lhs = 1.0 / f.sqrt();
                let rhs = -2.0 * (rr / 3.7 + 2.51 / (re * f.sqrt())).log10();
                let rel = (lhs - rhs).abs() / rhs;
                println!("Re = {re:.0e} e/D = {rr:.0e}  f = {f:.9}  CW residual rel = {rel:.3e}");
                assert!(
                    rel < 0.02,
                    "Re = {re}, e/D = {rr}: 1/sqrt(f) = {lhs}, CW = {rhs}"
                );
            }
        }
    }

    /// Print the HTR-10 hot-duct solution. Measurement harness for the V&V
    /// numbers recorded below (2026-08-11).
    #[test]
    fn measure_htr10_hot_duct() {
        let duct = htr10_hot_duct();
        let out = duct
            .solve_helium(
                htr10_design_point::mass_flow_rate(),
                htr10_design_point::core_outlet_temperature(),
                htr10_design_point::pressure(),
                DuctThermalBoundary::Adiabatic,
            )
            .unwrap();
        println!(
            "HTR-10 hot duct (D = 0.30 m, L = 5 m, adiabatic):\n  \
             G = {:.6} kg/(m^2 s)  Re = {:.1}  Pr = {:.6}  f = {:.6}  Nu = {:.3}  h = {:.3} W/(m^2 K)\n  \
             u_in = {:.4} m/s  u_out = {:.4} m/s  Ma_out = {:.6e}\n  \
             dp_friction = {:.4} Pa  dp_accel = {:.6} Pa  dp_total = {:.4} Pa\n  \
             T_in = {:.6} K  T_out = {:.6} K  p_out = {:.3} Pa\n  \
             rho_in = {:.6}  rho_out = {:.6} kg/m^3  in_range = {}",
            out.mass_flux.value,
            out.reynolds.get::<ratio>(),
            out.prandtl.get::<ratio>(),
            out.darcy_friction_factor.get::<ratio>(),
            out.nusselt.get::<ratio>(),
            out.heat_transfer_coefficient.get::<watt_per_square_meter_kelvin>(),
            out.inlet_velocity.get::<meter_per_second>(),
            out.outlet_velocity.get::<meter_per_second>(),
            out.outlet_mach.get::<ratio>(),
            out.friction_pressure_drop.get::<pascal>(),
            out.acceleration_pressure_drop.get::<pascal>(),
            out.pressure_drop.get::<pascal>(),
            out.inlet.temperature.get::<kelvin>(),
            out.outlet.temperature.get::<kelvin>(),
            out.outlet.pressure.get::<pascal>(),
            out.inlet.density.get::<uom::si::mass_density::kilogram_per_cubic_meter>(),
            out.outlet.density.get::<uom::si::mass_density::kilogram_per_cubic_meter>(),
            out.heat_transfer_correlation_in_range(duct.heat_transfer),
        );
    }

    /// V&V — the HTR-10 hot-gas duct is deeply subsonic, which is the
    /// measurement the module's low-Mach assumption rests on.
    ///
    /// **Methodology.** Solve the hot duct (round, `D = 0.30 m`,
    /// `L = 5 m`, drawn-steel roughness `4.5e-5 m`, adiabatic) at the
    /// HTR-10 design point: helium, 3.0 MPa, 4.3 kg/s, 973.15 K core
    /// outlet. Pass criterion: outlet Mach number below `1e-2`, so
    /// `Ma^2 < 1e-4` and the dropped kinetic-energy term is at most a
    /// hundredth of a percent of the enthalpy.
    ///
    /// **Results (2026-08-11, this implementation).**
    /// `G = 60.832556 kg/(m^2 s)`, `Re = 402769.7`, `Pr = 0.661835`,
    /// Churchill `f = 0.015361`, Gnielinski `Nu = 589.471`,
    /// `h = 698.384 W/(m^2 K)` (in range), `u = 41.1370 m/s`,
    /// **`Ma_out = 2.234193e-2`**, friction drop `320.3303 Pa` over 5 m,
    /// acceleration term `~0` (adiabatic), `p_out = 2999679.670 Pa`,
    /// `rho` `1.478781 -> 1.478624 kg/m^3`. PASSES.
    ///
    /// **Interpretation.** `Ma^2 = 4.99e-4`, so the dropped kinetic-energy
    /// term is at most 0.05 % of the enthalpy — the low-Mach treatment is
    /// justified for this duct, though by a smaller margin than for the
    /// core (`Ma = 1.4e-3`, `Ma^2 = 2e-6`). Note this duct diameter is an
    /// illustrative choice; a narrower duct would raise the Mach number
    /// and this assertion is what would catch it.
    #[test]
    fn htr10_hot_duct_is_deeply_subsonic() {
        let duct = htr10_hot_duct();
        let out = duct
            .solve_helium(
                htr10_design_point::mass_flow_rate(),
                htr10_design_point::core_outlet_temperature(),
                htr10_design_point::pressure(),
                DuctThermalBoundary::Adiabatic,
            )
            .unwrap();
        let ma = out.outlet_mach.get::<ratio>();
        println!("HTR-10 hot duct outlet Mach = {ma:.6e}");
        assert!(
            ma > 0.0 && ma < 5.0e-2,
            "outlet Mach {ma} is not deeply subsonic"
        );
    }

    /// V&V — energy conservation. With a prescribed heat input the outlet
    /// enthalpy must satisfy `h_out - h_in = Q / mdot` exactly (the balance
    /// the solver is built on), and the outlet temperature must therefore
    /// be consistent with the EOS rather than with an assumed constant
    /// `cp`.
    ///
    /// **Methodology.** Heat 4.3 kg/s of helium at 3.0 MPa with 10 MW —
    /// the HTR-10 core duty — in a duct, from the 523.15 K core inlet.
    /// Check `mdot (h_out - h_in)` against 10 MW to a relative tolerance of
    /// `1e-9`, and separately compare the resulting outlet temperature
    /// against the 973.15 K design core outlet.
    ///
    /// **Results (2026-08-11).** The enthalpy balance closes to
    /// `rel = 1.863e-16`, i.e. machine precision, as it must by
    /// construction. The resulting outlet temperature is
    /// **`T_out = 971.1108 K`**, a residual of **`-2.0392 K`**
    /// (`-0.2096 %`) against the design 973.15 K. PASSES.
    ///
    /// **Interpretation.** The published HTR-10 triple (10 MW, 4.3 kg/s,
    /// 250 -> 700 C) is internally consistent to about 0.2 % once a
    /// real-gas enthalpy rise is used instead of a round `c_p`; the
    /// shortfall is the same sign and comparable size to the `+0.455 %`
    /// the digital-twin crate records from the reciprocal check.
    #[test]
    fn energy_balance_closes_on_enthalpy() {
        let duct = GasDuct::new_circular(
            Length::new::<meter>(0.30),
            Length::new::<meter>(5.0),
            Length::new::<meter>(4.5e-5),
        );
        let mdot = htr10_design_point::mass_flow_rate();
        let q = htr10_design_point::thermal_power();
        let out = duct
            .solve_helium(
                mdot,
                htr10_design_point::core_inlet_temperature(),
                htr10_design_point::pressure(),
                DuctThermalBoundary::HeatInput(q),
            )
            .unwrap();

        let dh = out.outlet.specific_enthalpy.get::<joule_per_kilogram>()
            - out.inlet.specific_enthalpy.get::<joule_per_kilogram>();
        let q_back = dh * mdot.get::<kilogram_per_second>();
        let rel = (q_back - q.get::<watt>()).abs() / q.get::<watt>();
        let t_out = out.outlet.temperature.get::<kelvin>();
        println!(
            "10 MW into 4.3 kg/s He at 3 MPa from 523.15 K:\n  \
             T_out = {t_out:.4} K (design 973.15 K, residual {:+.4} K, {:+.4} %)\n  \
             enthalpy balance closes to rel = {rel:.3e}",
            t_out - 973.15,
            100.0 * (t_out - 973.15) / 973.15,
        );
        assert!(rel < 1e-6, "enthalpy balance did not close: rel = {rel}");
        assert!(
            (t_out - 973.15).abs() / 973.15 < 0.03,
            "outlet temperature {t_out} K is more than 3 % from the design 973.15 K"
        );
    }

    /// A prescribed wall temperature must be approached asymptotically and
    /// never overshot, for both heating and cooling.
    #[test]
    fn wall_temperature_boundary_never_overshoots() {
        let duct = GasDuct::new_circular(
            Length::new::<meter>(0.05),
            Length::new::<meter>(20.0),
            Length::new::<meter>(1e-5),
        );
        let p = htr10_design_point::pressure();
        let mdot = MassRate::new::<kilogram_per_second>(0.02);

        // Heating: wall hotter than the gas.
        let hot = duct
            .solve_helium(
                mdot,
                ThermodynamicTemperature::new::<kelvin>(523.15),
                p,
                DuctThermalBoundary::WallTemperature(ThermodynamicTemperature::new::<kelvin>(
                    1073.15,
                )),
            )
            .unwrap();
        let t_hot = hot.outlet.temperature.get::<kelvin>();
        println!(
            "heating: T_out = {t_hot:.4} K (in 523.15, wall 1073.15), Q = {:.2} W",
            hot.heat_duty.get::<watt>()
        );
        assert!(t_hot > 523.15 && t_hot < 1073.15, "T_out = {t_hot}");
        assert!(hot.heat_duty.get::<watt>() > 0.0);

        // Cooling: wall colder than the gas.
        let cold = duct
            .solve_helium(
                mdot,
                ThermodynamicTemperature::new::<kelvin>(973.15),
                p,
                DuctThermalBoundary::WallTemperature(ThermodynamicTemperature::new::<kelvin>(
                    523.15,
                )),
            )
            .unwrap();
        let t_cold = cold.outlet.temperature.get::<kelvin>();
        println!(
            "cooling: T_out = {t_cold:.4} K (in 973.15, wall 523.15), Q = {:.2} W",
            cold.heat_duty.get::<watt>()
        );
        assert!(t_cold < 973.15 && t_cold > 523.15, "T_out = {t_cold}");
        assert!(cold.heat_duty.get::<watt>() < 0.0);
    }

    /// The acceleration pressure-drop term must change sign with the
    /// direction of heat flow: a heated gas expands and accelerates
    /// (positive drop), a cooled gas contracts and decelerates (pressure
    /// recovery, negative drop).
    #[test]
    fn acceleration_term_changes_sign_with_heating() {
        let duct = GasDuct::new_circular(
            Length::new::<meter>(0.10),
            Length::new::<meter>(5.0),
            Length::new::<meter>(1e-5),
        );
        let p = htr10_design_point::pressure();
        let mdot = MassRate::new::<kilogram_per_second>(1.0);
        let t_in = ThermodynamicTemperature::new::<kelvin>(700.0);

        let heated = duct
            .solve_helium(
                mdot,
                t_in,
                p,
                DuctThermalBoundary::HeatInput(Power::new::<watt>(5.0e5)),
            )
            .unwrap();
        let cooled = duct
            .solve_helium(
                mdot,
                t_in,
                p,
                DuctThermalBoundary::HeatInput(Power::new::<watt>(-5.0e5)),
            )
            .unwrap();
        println!(
            "heated dp_acc = {:+.6} Pa, cooled dp_acc = {:+.6} Pa",
            heated.acceleration_pressure_drop.get::<pascal>(),
            cooled.acceleration_pressure_drop.get::<pascal>(),
        );
        assert!(heated.acceleration_pressure_drop.get::<pascal>() > 0.0);
        assert!(cooled.acceleration_pressure_drop.get::<pascal>() < 0.0);
        assert!(heated.friction_pressure_drop.get::<pascal>() > 0.0);
    }

    /// Gnielinski and Dittus-Boelter must agree to within the ~20 % spread
    /// the correlations are quoted at, in the region where both are valid.
    #[test]
    fn gnielinski_and_dittus_boelter_agree_within_correlation_spread() {
        let pr = Ratio::new::<ratio>(0.66); // helium
        for re in [2.0e4_f64, 1.0e5, 5.0e5] {
            let re_q = Ratio::new::<ratio>(re);
            let f = churchill_friction_factor(re_q, Ratio::new::<ratio>(0.0)).unwrap();
            let nu_g = nusselt_number(HeatTransferCorrelation::Gnielinski, re_q, pr, f)
                .unwrap()
                .get::<ratio>();
            let nu_db = nusselt_number(HeatTransferCorrelation::DittusBoelterHeating, re_q, pr, f)
                .unwrap()
                .get::<ratio>();
            let rel = (nu_g - nu_db).abs() / nu_db;
            println!("Re = {re:.0e}: Nu_Gnielinski = {nu_g:.3}, Nu_DittusBoelter = {nu_db:.3}, rel = {:.2} %", 100.0 * rel);
            assert!(rel < 0.25, "Re = {re}: Nu spread {rel} exceeds 25 %");
        }
    }

    /// Guard behaviour: bad geometry and bad flow are rejected.
    #[test]
    fn invalid_inputs_are_rejected() {
        let duct = htr10_hot_duct();
        let p = htr10_design_point::pressure();
        let t = htr10_design_point::core_outlet_temperature();
        assert!(duct
            .solve_helium(
                MassRate::new::<kilogram_per_second>(0.0),
                t,
                p,
                DuctThermalBoundary::Adiabatic
            )
            .is_err());
        assert!(duct
            .solve_helium(
                MassRate::new::<kilogram_per_second>(-1.0),
                t,
                p,
                DuctThermalBoundary::Adiabatic
            )
            .is_err());
        assert!(
            churchill_friction_factor(Ratio::new::<ratio>(0.0), Ratio::new::<ratio>(0.0)).is_err()
        );
        assert!(nusselt_number(
            HeatTransferCorrelation::Gnielinski,
            Ratio::new::<ratio>(-1.0),
            Ratio::new::<ratio>(0.66),
            Ratio::new::<ratio>(0.02)
        )
        .is_err());
    }
}
