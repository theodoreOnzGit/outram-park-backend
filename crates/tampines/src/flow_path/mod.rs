// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of OUTRAM PARK. See `src/lib.rs` for the full licence
// notice.
//! Flow paths between control volumes (gh:#1094, part of the MELCOR-style
//! control-volume epic gh:#1091; MELCOR 2.2 CVH/FL Reference Manual §6,
//! cited, not quoted; no MELCOR source used).
//!
//! > Research, education and V&V only. AI draft, unreviewed.
//!
//! A [`FlowPath`] joins two ends, each at its own elevation in its own
//! volume. It carries **two velocities**, one for the pool (liquid) phase and
//! one for the atmosphere (gas) phase, each with its own momentum equation:
//!
//! ```text
//! ρ_k L dv_k/dt = ΔP_k − ½ K ρ_k |v_k| v_k,
//! ΔP_k = p_from(z_from) − p_to(z_to) − ρ_k g (z_to − z_from)
//! ```
//!
//! with the end pressures at the junction elevations (each volume adds its
//! own hydrostatic head below its level) and `L` the inertial length. The
//! step is semi-implicit in the velocity (the loss linearised about the old
//! velocity, the pressures held): `(ρL/Δt + ½Kρ|v^n|) v^{n+1} = ρL/Δt v^n +
//! ΔP`. A fully implicit network solve, with the volume pressures coupled, is
//! gh:#1095.
//!
//! **Order (gh:#1098 item 1).** With `ΔP` constant over the step this is
//! second order (the linearised quadratic loss matches the exact solution's
//! `Δt²` term; for `v∞ tanh(t/τ)` it is exact on a clock stretched by
//! `artanh(h)/h = 1 + h²/3 + …`, `h = Δt/τ`). With `ΔP` held at its
//! start-of-step value while it changes, it is first order.
//!
//! **Which phase passes** is set by the donor end's level against the
//! opening: the fraction of the opening below the level passes pool liquid,
//! the rest atmosphere (MELCOR's partially submerged junction). The donor is
//! the upstream end by the phase's velocity, or by the pressure difference
//! when the phase is at rest.
//!
//! **Choking.** The gas flow is capped at MELCOR's critical gas flow (§6.3,
//! Eq 6-13), [`critical_moles`], moved verbatim from `sembawang` (which now
//! calls it here); the liquid flow is capped by IF97's HEM break flow from
//! the donor's stagnation state ([`Iapws97Hem`]'s `break_flow`, Moody- and
//! Zaloudek-verified in `tampines-steam-tables`). Both are on by default;
//! `without_choking` is the ablation.
//!
//! **Kinds:** an open path, a check valve (forward only), and a vacuum
//! breaker (forward only, opening above a pressure difference).

use tampines_steam_tables::hem::{HemEquationOfState, Iapws97Hem};
use tampines_steam_tables::gas_mixture::R_GAS;
use uom::si::area::square_meter;
use uom::si::available_energy::joule_per_kilogram;
use uom::si::f64::{
    Area, AvailableEnergy, Length, MassDensity, MassRate, Power, Pressure, ThermodynamicTemperature, Time, Velocity,
};
use uom::si::length::meter;
use uom::si::mass_density::kilogram_per_cubic_meter;
use uom::si::mass_flux::kilogram_per_square_meter_second;
use uom::si::mass_rate::kilogram_per_second;
use uom::si::power::watt;
use uom::si::pressure::pascal;
use uom::si::thermodynamic_temperature::kelvin;
use uom::si::time::second;
use uom::si::velocity::meter_per_second;

use crate::TampinesError;

const G: f64 = 9.80665;

/// The critical (choked) flow of a gas through an orifice, mol/s: MELCOR's
/// sonic flux at the donor's stagnation state (CVH/FL RM §6.3, Eq 6-13),
/// `G = ρ c_s (2/(γ+1))^((γ+1)/(2(γ−1)))` with the multiplier at MELCOR's
/// nominal `γ = 1.4` (0.5787), times the discharge coefficient and area. The
/// sound speed from the donor's own `γ`: diatomic gases at `c_v = 2.5 R`,
/// steam at `3.15 R`, by mole fraction `x_steam`; `molar` the molar mass,
/// kg/mol.
///
/// **Moved verbatim from `sembawang::bwr_mark1::containment`** (2026-11-01,
/// gh:#1094); `sembawang` calls it here, so its results are unchanged.
pub fn critical_moles(cd: f64, area: f64, p: f64, t: f64, molar: f64, x_steam: f64) -> f64 {
    let cv = (1.0 - x_steam) * 2.5 + x_steam * 3.15;
    let gamma = (cv + 1.0) / cv;
    let rho = p * molar / (R_GAS * t);
    let c_s = (gamma * R_GAS * t / molar).sqrt();
    cd * area * rho * c_s * 0.578_7 / molar
}

/// The geometry of a flow path.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FlowPathGeometry {
    /// Flow area.
    pub area: Area,
    /// Inertial length `L` (the momentum equation's `ρL dv/dt`).
    pub inertial_length: Length,
    /// Total form-loss coefficient `K` (both directions).
    pub form_loss: f64,
    /// Discharge coefficient on the area (1 for a pipe).
    pub discharge_coefficient: f64,
    /// Junction elevation in the `from` volume (its datum).
    pub from_elevation: Length,
    /// Junction elevation in the `to` volume (its datum).
    pub to_elevation: Length,
    /// Height of the opening, over which a level partially submerges it
    /// (0 for a point junction).
    pub opening_height: Length,
}

/// What kind of path.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FlowPathKind {
    /// Flow either way.
    Open,
    /// Forward (`from → to`) only.
    CheckValve,
    /// Forward only, and only while `p_from − p_to` exceeds the opening
    /// difference (a vacuum breaker).
    VacuumBreaker {
        /// Opening pressure difference.
        opening_pressure_difference: Pressure,
    },
}

/// One end of a path, as its volume presents it at the junction elevation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PathEnd {
    /// Pressure at the junction elevation (the atmosphere's pressure plus
    /// the hydrostatic head below the level).
    pub pressure: Pressure,
    /// The volume's (swollen) pool level on the junction's datum.
    pub pool_level: Length,
    /// Pool liquid density and specific enthalpy.
    pub liquid_density: MassDensity,
    /// Pool liquid specific enthalpy (IF97 reference).
    pub liquid_enthalpy: AvailableEnergy,
    /// Atmosphere density.
    pub gas_density: MassDensity,
    /// Atmosphere specific enthalpy (the volume's references).
    pub gas_enthalpy: AvailableEnergy,
    /// Atmosphere temperature.
    pub gas_temperature: ThermodynamicTemperature,
    /// Atmosphere mean molar mass, kg/mol.
    pub gas_molar_mass: f64,
    /// Steam mole fraction of the atmosphere.
    pub gas_steam_mole_fraction: f64,
}

/// What a path moved over a step, positive `from → to`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FlowTransfer {
    /// Pool-phase mass rate.
    pub liquid_mass_rate: MassRate,
    /// Atmosphere-phase mass rate.
    pub gas_mass_rate: MassRate,
    /// Enthalpy carried by the liquid (donor's enthalpy).
    pub liquid_enthalpy_rate: Power,
    /// Enthalpy carried by the gas (donor's enthalpy).
    pub gas_enthalpy_rate: Power,
    /// Whether the liquid flow was choked.
    pub liquid_choked: bool,
    /// Whether the gas flow was choked.
    pub gas_choked: bool,
    /// Whether the gas came from `from` (so its composition is `from`'s).
    pub gas_from_upstream_from: bool,
}

/// Mechanisms (all on by default).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FlowPathMechanisms {
    /// Critical-flow limits on both phases.
    pub choking: bool,
}

impl Default for FlowPathMechanisms {
    fn default() -> Self {
        Self { choking: true }
    }
}

/// A two-velocity flow path.
#[derive(Debug, Clone, PartialEq)]
pub struct FlowPath {
    geometry: FlowPathGeometry,
    kind: FlowPathKind,
    v_liquid: f64,
    v_gas: f64,
    mechanisms: FlowPathMechanisms,
    /// A valve's open fraction of the area, in `[0, 1]` (MELCOR's valve on
    /// a flow path, CVH/FL RM §6, cited): 1 unless set.
    open_fraction: f64,
    /// A vacuum breaker's open state held by a network for one step
    /// (`None`: decided by the pressures each call).
    held_open: Option<bool>,
}

impl FlowPath {
    /// A path at rest.
    ///
    /// # Errors
    ///
    /// [`TampinesError::InvalidInput`] for a non-positive area or inertial
    /// length, a negative loss or opening, or a discharge coefficient
    /// outside `(0, 1]`.
    pub fn new(geometry: FlowPathGeometry, kind: FlowPathKind) -> Result<Self, TampinesError> {
        let g = &geometry;
        let (a, l, d) = (g.area.get::<square_meter>(), g.inertial_length.get::<meter>(), g.opening_height.get::<meter>());
        if !(a > 0.0 && l > 0.0 && g.form_loss >= 0.0 && d >= 0.0 && g.discharge_coefficient > 0.0 && g.discharge_coefficient <= 1.0)
            || ![a, l, d, g.form_loss, g.from_elevation.value, g.to_elevation.value].iter().all(|x| x.is_finite())
        {
            return Err(TampinesError::InvalidInput(format!("flow path {geometry:?}")));
        }
        Ok(Self { geometry, kind, v_liquid: 0.0, v_gas: 0.0, mechanisms: FlowPathMechanisms::default(), open_fraction: 1.0, held_open: None })
    }

    /// Ablation: no critical-flow limit.
    pub fn without_choking(mut self) -> Self {
        self.mechanisms.choking = false;
        self
    }

    /// Set the valve's open fraction (clamped to `[0, 1]`; a NaN closes
    /// it). It multiplies the flow area, and at 0 the path is closed and its
    /// velocities reset (gh:#1097: a rupture disk, an operator's vent valve,
    /// a flange that unseats).
    pub fn set_open_fraction(&mut self, fraction: f64) {
        self.open_fraction = if fraction.is_nan() { 0.0 } else { fraction.clamp(0.0, 1.0) };
    }

    /// The valve's open fraction.
    pub fn open_fraction(&self) -> f64 {
        self.open_fraction
    }

    /// A vacuum breaker's opening pressure difference (`None` for the
    /// other kinds).
    pub fn opening_pressure_difference(&self) -> Option<f64> {
        match self.kind {
            FlowPathKind::VacuumBreaker { opening_pressure_difference } => Some(opening_pressure_difference.get::<pascal>()),
            _ => None,
        }
    }

    /// Hold a vacuum breaker open or shut whatever the pressures passed to
    /// [`FlowPath::step`] (`None` releases it): a network decides it
    /// ~~once per step from the start-of-step pressures (gh:#1097)~~ on each
    /// Picard iterate, opening at most once a step (gh:#1116; the start-of-step
    /// decision is `CvNetwork::without_iterate_switch_decisions`). Forward
    /// only still holds.
    pub fn hold_open(&mut self, held: Option<bool>) {
        self.held_open = held;
    }

    /// The mechanisms in force.
    pub fn mechanisms(&self) -> FlowPathMechanisms {
        self.mechanisms
    }

    /// The pool-phase and atmosphere-phase velocities, positive `from → to`.
    pub fn velocities(&self) -> (Velocity, Velocity) {
        (Velocity::new::<meter_per_second>(self.v_liquid), Velocity::new::<meter_per_second>(self.v_gas))
    }

    /// Scale the atmosphere-phase velocity by `factor` (a network's
    /// equalising limit, gh:#1095).
    pub fn scale_gas_velocity(&mut self, factor: f64) {
        self.v_gas *= factor;
    }

    /// The geometry.
    pub fn geometry(&self) -> &FlowPathGeometry {
        &self.geometry
    }

    /// The fraction of the opening at `z` (centre) below `level`.
    fn submerged_fraction(&self, z: f64, level: f64) -> f64 {
        let d = self.geometry.opening_height.get::<meter>();
        if d <= 0.0 {
            return if level > z { 1.0 } else { 0.0 };
        }
        ((level - (z - 0.5 * d)) / d).clamp(0.0, 1.0)
    }

    /// Advance the two velocities by `dt` and return the transfer.
    ///
    /// # Errors
    ///
    /// [`TampinesError::InvalidInput`] for a non-positive `dt`; the liquid
    /// break-flow closure's errors.
    pub fn step(&mut self, dt: Time, from: &PathEnd, to: &PathEnd) -> Result<FlowTransfer, TampinesError> {
        let dt = dt.get::<second>();
        if !(dt.is_finite() && dt > 0.0) {
            return Err(TampinesError::InvalidInput(format!("dt {dt} s")));
        }
        let g = self.geometry;
        let (z_f, z_t) = (g.from_elevation.get::<meter>(), g.to_elevation.get::<meter>());
        let (p_f, p_t) = (from.pressure.get::<pascal>(), to.pressure.get::<pascal>());
        let a_eff = g.discharge_coefficient * g.area.get::<square_meter>() * self.open_fraction;
        let l = g.inertial_length.get::<meter>();
        let dp = p_f - p_t;
        // Closed kinds.
        let open = self.open_fraction > 0.0
            && match self.kind {
            FlowPathKind::Open | FlowPathKind::CheckValve => true,
            FlowPathKind::VacuumBreaker { opening_pressure_difference } => self.held_open.unwrap_or(dp > opening_pressure_difference.get::<pascal>()),
        };
        if !open {
            self.v_liquid = 0.0;
            self.v_gas = 0.0;
            return Ok(FlowTransfer::zero());
        }
        let forward_only = !matches!(self.kind, FlowPathKind::Open);
        // One phase's velocity update; `rho_f`, `rho_t` its densities at the
        // two ends (the donor's is used).
        let advance = |v: f64, rho_f: f64, rho_t: f64| -> f64 {
            let forward = if v != 0.0 { v > 0.0 } else { dp >= 0.0 };
            let rho = if forward { rho_f } else { rho_t };
            let dp_k = dp - rho * G * (z_t - z_f);
            let a = rho * l / dt + 0.5 * g.form_loss * rho * v.abs();
            let v_new = (rho * l / dt * v + dp_k) / a;
            if forward_only { v_new.max(0.0) } else { v_new }
        };
        let (rl_f, rl_t) = (from.liquid_density.get::<kilogram_per_cubic_meter>(), to.liquid_density.get::<kilogram_per_cubic_meter>());
        let (rg_f, rg_t) = (from.gas_density.get::<kilogram_per_cubic_meter>(), to.gas_density.get::<kilogram_per_cubic_meter>());
        self.v_liquid = advance(self.v_liquid, rl_f, rl_t);
        self.v_gas = advance(self.v_gas, rg_f, rg_t);
        // Donors and the phase fractions of the opening at the donor.
        let (liq_fwd, gas_fwd) = (self.v_liquid >= 0.0, self.v_gas >= 0.0);
        let f_liq = if liq_fwd {
            self.submerged_fraction(z_f, from.pool_level.get::<meter>())
        } else {
            self.submerged_fraction(z_t, to.pool_level.get::<meter>())
        };
        let f_gas = if gas_fwd {
            1.0 - self.submerged_fraction(z_f, from.pool_level.get::<meter>())
        } else {
            1.0 - self.submerged_fraction(z_t, to.pool_level.get::<meter>())
        };
        let (dl, dg) = (if liq_fwd { from } else { to }, if gas_fwd { from } else { to });
        let mut m_liq = dl.liquid_density.get::<kilogram_per_cubic_meter>() * f_liq * a_eff * self.v_liquid;
        let mut m_gas = dg.gas_density.get::<kilogram_per_cubic_meter>() * f_gas * a_eff * self.v_gas;
        let (mut liq_choked, mut gas_choked) = (false, false);
        if self.mechanisms.choking {
            if m_gas != 0.0 && f_gas > 0.0 {
                let p_up = if gas_fwd { p_f } else { p_t };
                let n_crit = critical_moles(
                    g.discharge_coefficient,
                    f_gas * g.area.get::<square_meter>() * self.open_fraction,
                    p_up,
                    dg.gas_temperature.get::<kelvin>(),
                    dg.gas_molar_mass,
                    dg.gas_steam_mole_fraction,
                );
                let m_crit = n_crit * dg.gas_molar_mass;
                if m_gas.abs() > m_crit {
                    m_gas = m_crit.copysign(m_gas);
                    self.v_gas = m_gas / (dg.gas_density.get::<kilogram_per_cubic_meter>() * f_gas * a_eff);
                    gas_choked = true;
                }
            }
            if m_liq != 0.0 && f_liq > 0.0 {
                let (p_up, p_dn) = if liq_fwd { (p_f, p_t) } else { (p_t, p_f) };
                if p_up > p_dn {
                    let rho = dl.liquid_density.get::<kilogram_per_cubic_meter>();
                    // The donor is a volume at rest: its stagnation enthalpy is
                    // its own (a first version added ½v² of the path's
                    // pre-limit velocity, which ran to 4.5e7 J/kg).
                    let h0 = dl.liquid_enthalpy.get::<joule_per_kilogram>();
                    let bf = Iapws97Hem.break_flow(
                        Pressure::new::<pascal>(p_up),
                        AvailableEnergy::new::<joule_per_kilogram>(h0),
                        Pressure::new::<pascal>(p_dn),
                    )?;
                    let m_crit = bf.mass_flux.get::<kilogram_per_square_meter_second>() * f_liq * a_eff;
                    if bf.choked && m_liq.abs() > m_crit {
                        m_liq = m_crit.copysign(m_liq);
                        self.v_liquid = m_liq / (rho * f_liq * a_eff);
                        liq_choked = true;
                    }
                }
            }
        }
        Ok(FlowTransfer {
            liquid_mass_rate: MassRate::new::<kilogram_per_second>(m_liq),
            gas_mass_rate: MassRate::new::<kilogram_per_second>(m_gas),
            liquid_enthalpy_rate: Power::new::<watt>(m_liq * dl.liquid_enthalpy.get::<joule_per_kilogram>()),
            gas_enthalpy_rate: Power::new::<watt>(m_gas * dg.gas_enthalpy.get::<joule_per_kilogram>()),
            liquid_choked: liq_choked,
            gas_choked,
            gas_from_upstream_from: gas_fwd,
        })
    }
}

impl FlowTransfer {
    fn zero() -> Self {
        Self {
            liquid_mass_rate: MassRate::new::<kilogram_per_second>(0.0),
            gas_mass_rate: MassRate::new::<kilogram_per_second>(0.0),
            liquid_enthalpy_rate: Power::new::<watt>(0.0),
            gas_enthalpy_rate: Power::new::<watt>(0.0),
            liquid_choked: false,
            gas_choked: false,
            gas_from_upstream_from: true,
        }
    }
}

#[cfg(test)]
mod tests;
