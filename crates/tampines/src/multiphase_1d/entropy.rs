// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of OUTRAM PARK. See `src/lib.rs` for the full licence
// notice.

//! Interfacial entropy production and the phasic entropy diagnostic
//! (gh:#1064; RELAP-7 Theory Manual §2.1.1 Eqs 18, 23 and §3.1.8
//! Eqs 149–155). AI draft, clean room: the manual is private, so only its
//! equation numbers are cited; every formula below is derived here from the
//! equations [`super::two_fluid::TwoFluid1d`] actually solves.
//!
//! > Research, education and V&V only. AI draft, unreviewed.
//!
//! # What is checked
//!
//! The averaged second law (Eq 155) requires the entropy the interfacial
//! exchange produces to be non-negative. For the six-equation model the
//! interface stores no entropy, so everything the exchange puts into the two
//! phases' entropy equations is production. Take phase `k`'s mass source
//! `Γ_k` (`Γ_g = Γ = −Γ_l`), its static-enthalpy source
//! `Q_k + Γ_k h_k^sat + W_k` (the terms [`super::two_fluid`] adds, with `W_k`
//! the friction work and the `½ (u_int − u_k)²` the transferred mass carries,
//! both from `two_fluid::interface_work`), and Gibbs at constant
//! pressure, `T_k ds_k = dh_k`. Phase `k`'s entropy then gains
//! `Γ_k s_k + (Q_k + Γ_k (h_k^sat − h_k) + W_k) / T_k` per unit volume, and
//! the sum over both phases is
//!
//! `σ = Σ_k Q_k/T_k + Γ (s_g − s_l) + Γ ((h_g^sat − h_g)/T_g − (h_f^sat − h_l)/T_l) + Σ_k W_k/T_k`.
//!
//! Using the interfacial energy jump `Γ h_fg = −(Q_g + Q_l)` that
//! [`super::interfacial`] closes `Γ` with, this splits exactly into
//!
//! | term | expression | sign |
//! |---|---|---|
//! | heat transfer | `Σ_k Q_k (1/T_k − 1/T_sat)` | `≥ 0`: `Q_k = K_k (T_sat − T_k)`, `K_k ≥ 0` |
//! | phase change | `Γ (φ_g − φ_l)`, `φ_k = (s_k − s_k^sat) − (h_k − h_k^sat)/T_k` | not signed; `φ_k` is second order in `T_k − T_sat`, so the term can outweigh the heat term when `Γ` is driven by one phase and the other is far from saturation (measured: see below) |
//! | saturation consistency | `Γ ((s_g^sat − s_f^sat) − h_fg/T_sat)` | zero for an exactly consistent EOS |
//! | drag | `Σ_k F_k (u_int − u_k)/T_k` | `≥ 0` for any `u_int` between `u_l` and `u_g` |
//! | mass-transfer kinetic | `Σ_k Γ_k ½ (u_int − u_k)²/T_k` | **not signed** for the mass-weighted `u_int` |
//!
//! The saturation-consistency term is IF97's, not the closure's: the
//! Region-4 saturation line is a separate fit, so the Region-1 and Region-2
//! Gibbs energies are equal on it only to the fit's accuracy. It is reported
//! separately so that it cannot hide inside the closure terms.
//!
//! The kinetic term: with `w = u_g − u_l` and `x = m_g/(m_g + m_l)`, the
//! mass-weighted `u_int = u_l + x w` gives, at equal phase temperatures,
//! `Γ w² (1 − 2x) / (2T)`, which is **negative for condensation (`Γ < 0`)
//! wherever the vapour holds less than half the mass**. The donor-velocity
//! ablation (`u_int` = the losing phase's velocity) gives `|Γ| w² / (2T) ≥ 0`.
//! Whether Eq 200's `u_int` is the mass-weighted one was not re-checked
//! against the manual (see [`super::two_fluid`]); the sign finding is
//! algebra on the `u_int` this code uses.
//!
//! **Measured (2026-11-01, `tests::every_interfacial_closure_produces_non_negative_entropy_over_a_sweep`):**
//! heat transfer, drag and the donor kinetic term are never negative; the
//! total is negative at 4 of 10 872 swept states with the defaults, all with
//! the vapour 100 K superheated against subcooled liquid at 15 MPa. **The
//! closures as they stand do not satisfy Eq 155 everywhere**; the record is
//! `crates/tampines/verification_and_validation/species_entropy.md`.
//!
//! **What the check does not cover.** [`super::two_fluid::TwoFluid1d`] makes
//! the heat fluxes implicit (`Q_k^imp = (T_sat − T_k)/(1/K_k + Δt/(m_k c_p))`)
//! and may rate-limit `Γ` together with both `Q_k`; both keep
//! `Q_k (T_sat − T_k) ≥ 0` and the energy jump, so the signs above carry over,
//! but the check here evaluates the closures themselves, not the stepped
//! values.

use tampines_steam_tables::region_1_subcooled_liquid::s_tp_1;
use tampines_steam_tables::region_2_vapour::s_tp_2;
use uom::si::f64::{Pressure, ThermodynamicTemperature};
use uom::si::pressure::pascal;
use uom::si::specific_heat_capacity::joule_per_kilogram_kelvin;
use uom::si::thermodynamic_temperature::kelvin;

use super::interfacial::{InterfacialCellState, InterfacialSources};
use super::properties::{PhaseState, SaturatedProperties};
use super::two_fluid::interface_work;
use crate::TampinesError;

/// Specific entropies `(s_g, s_l)` \[J/(kg·K)\] of vapour at `(p, t_g)` and
/// liquid at `(p, t_l)`, from IAPWS-IF97 Region 2 and Region 1 (the liquid's
/// metastable continuation where `t_l > T_sat`), the same equations
/// [`PhaseState`] inverts. Raw SI: `p` \[Pa\], temperatures \[K\].
#[must_use]
pub fn phase_entropies(p: f64, t_g: f64, t_l: f64) -> (f64, f64) {
    let p_q = Pressure::new::<pascal>(p);
    let s_g = s_tp_2(ThermodynamicTemperature::new::<kelvin>(t_g), p_q)
        .get::<joule_per_kilogram_kelvin>();
    let s_l = s_tp_1(ThermodynamicTemperature::new::<kelvin>(t_l), p_q)
        .get::<joule_per_kilogram_kelvin>();
    (s_g, s_l)
}

/// Which interface-work terms the solver applies, mirroring
/// [`super::two_fluid::TwoFluid1d`]'s gh:#1060 defaults and ablations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InterfaceWorkTerms {
    /// Mass-weighted `u_int` with friction work (default `true`; `false` is
    /// [`super::two_fluid::TwoFluid1d::without_interface_velocity`]).
    pub interface_velocity: bool,
    /// Transferred mass carries `H_int` (default `true`; `false` is
    /// [`super::two_fluid::TwoFluid1d::without_total_enthalpy_mass_transfer`]).
    pub total_enthalpy_mass_transfer: bool,
}

impl Default for InterfaceWorkTerms {
    fn default() -> Self {
        Self {
            interface_velocity: true,
            total_enthalpy_mass_transfer: true,
        }
    }
}

/// The interfacial entropy production of one cell \[W/(m³·K)\], term by term
/// (see the module docs for each expression and its expected sign).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct InterfacialEntropyProduction {
    /// `Σ_k Q_k (1/T_k − 1/T_sat)`.
    pub heat_transfer: f64,
    /// `Γ (φ_g − φ_l)`.
    pub phase_change: f64,
    /// `Γ ((s_g^sat − s_f^sat) − h_fg/T_sat)`: the equation of state's own
    /// saturation inconsistency, not the closure's.
    pub saturation_consistency: f64,
    /// Friction work at `u_int`, `Σ_k F_k (u_int − u_k)/T_k`.
    pub drag: f64,
    /// `Σ_k Γ_k ½ (u_int − u_k)²/T_k`.
    pub mass_transfer_kinetic: f64,
}

impl InterfacialEntropyProduction {
    /// The sum of every term: the entropy production Eq 155 requires to be
    /// non-negative.
    #[must_use]
    pub fn total(self) -> f64 {
        self.heat_transfer
            + self.phase_change
            + self.saturation_consistency
            + self.drag
            + self.mass_transfer_kinetic
    }
}

/// Evaluate the interfacial entropy production for the closure output
/// `sources` at the cell state `cell` (gh:#1064, RELAP-7 Eq 155).
///
/// `saturated` must be at `cell.pressure`. Raw SI throughout; the result is
/// in W/(m³·K).
///
/// # Errors
///
/// As [`PhaseState::vapour_at`] / [`PhaseState::liquid_at`] for a phase
/// enthalpy outside the metastable brackets.
pub fn interfacial_entropy_production(
    cell: InterfacialCellState,
    sources: &InterfacialSources,
    saturated: SaturatedProperties,
    terms: InterfaceWorkTerms,
) -> Result<InterfacialEntropyProduction, TampinesError> {
    let p = cell.pressure;
    let vapour = PhaseState::vapour_at(p, cell.vapour_enthalpy, saturated)?;
    let liquid = PhaseState::liquid_at(p, cell.liquid_enthalpy, saturated)?;
    let (t_g, t_l, t_i) = (vapour.temperature, liquid.temperature, saturated.t_sat);
    let (q_g, q_l, gamma) = (sources.vapour_heat, sources.liquid_heat, sources.mass_transfer);

    let (s_g, s_l) = phase_entropies(p, t_g, t_l);
    let (s_g_sat, s_f_sat) = phase_entropies(p, t_i, t_i);
    let phi_g = (s_g - s_g_sat) - (cell.vapour_enthalpy - saturated.h_g) / t_g;
    let phi_l = (s_l - s_f_sat) - (cell.liquid_enthalpy - saturated.h_f) / t_l;

    let m_g = cell.void_fraction * vapour.density;
    let m_l = (1.0 - cell.void_fraction) * liquid.density;
    let (u_g, u_l) = (cell.vapour_velocity, cell.liquid_velocity);
    let (drag_g, drag_l) = interface_work(
        terms.interface_velocity,
        false,
        0.0,
        sources.volumetric_drag_coefficient,
        m_g,
        m_l,
        u_g,
        u_l,
    );
    let (kin_g, kin_l) = interface_work(
        terms.interface_velocity,
        terms.total_enthalpy_mass_transfer,
        gamma,
        0.0,
        m_g,
        m_l,
        u_g,
        u_l,
    );

    Ok(InterfacialEntropyProduction {
        heat_transfer: q_g * (1.0 / t_g - 1.0 / t_i) + q_l * (1.0 / t_l - 1.0 / t_i),
        phase_change: gamma * (phi_g - phi_l),
        saturation_consistency: gamma * ((s_g_sat - s_f_sat) - saturated.h_fg() / t_i),
        drag: drag_g / t_g + drag_l / t_l,
        mass_transfer_kinetic: kin_g / t_g + kin_l / t_l,
    })
}

#[cfg(test)]
mod tests;
