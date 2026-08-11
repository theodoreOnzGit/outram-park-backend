// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of OUTRAM PARK. See `src/lib.rs` for the full licence
// notice.

//! The steam/water property layer both 1-D solvers evaluate per cell per step.
//!
//! # What belongs here
//!
//! A thin, *cached* face onto [`tampines_steam_tables`]'s IAPWS-IF97
//! functions, shaped for what a two-phase system-code marching loop actually
//! asks for:
//!
//! - [`SaturatedProperties`] — everything on the saturation line at one
//!   pressure (`T_sat`, `h_f`, `h_g`, `ρ_f`, `ρ_g`, `μ_f`, `μ_g`) plus the
//!   pressure derivatives the pressure equation needs.
//! - [`TwoPhaseState`] — the result of a `(p, h)` flash: quality, void
//!   fraction, mixture density, temperature.
//! - [`SaturatedTransport`] — the *conduction* properties on the saturation
//!   line (`λ_f`, `λ_g`, `c_p,f`, `c_p,g`), which only the six-equation solver
//!   needs and which are kept out of [`SaturatedProperties`] because they cost
//!   four extra IF97 evaluations per call.
//! - [`PhaseState`] — a **single-phase** `(p, h_k)` state for *one* phase,
//!   including the bounded **metastable** extensions (superheated liquid,
//!   subcooled vapour) that a six-equation model needs and a four-equation
//!   equilibrium model does not.
//!
//! # What does not
//!
//! The IAPWS correlations themselves. Those are `tampines-steam-tables`'s job
//! and this module never reimplements one — per the workspace rule that raw
//! property-table equations do not belong in `tampines`.
//!
//! # Why a cache exists at all
//!
//! `sat_temp_4` is a backward correlation and the saturated-property set costs
//! several IF97 evaluations. A two-phase march wants that set at *every* cell
//! at *every* step, and neighbouring cells in a blowdown sit at nearly the same
//! pressure. [`SaturatedProperties::at`] is therefore memo-free but cheap to
//! call, and the solvers hold one instance per cell and refresh it only when
//! the cell pressure has moved by more than a relative tolerance. That is a
//! performance decision with a correctness consequence, so the tolerance is
//! public and documented at [`SaturatedProperties::is_stale_for`].
//!
//! # Units
//!
//! Constructed from `uom` at the boundary; every field is raw `f64` in strict
//! SI — pascal, kelvin, `J/kg`, `kg/m³`, `Pa·s` — because these are read inside
//! the per-cell loop.

use tampines_steam_tables::dynamic_viscosity::mu_tp_eqm_two_phase;
use tampines_steam_tables::region_1_subcooled_liquid::{cp_tp_1, h_tp_1, v_tp_1};
use tampines_steam_tables::region_2_vapour::{cp_tp_2, h_tp_2, v_tp_2};
use tampines_steam_tables::region_4_vap_liq_equilibrium::sat_temp_4;
use tampines_steam_tables::thermal_conductivity::lambda_tp_eqm_two_phase;

use uom::si::available_energy::joule_per_kilogram;
use uom::si::dynamic_viscosity::pascal_second;
use uom::si::f64::{AvailableEnergy, MassDensity, Pressure, ThermodynamicTemperature};
use uom::si::mass_density::kilogram_per_cubic_meter;
use uom::si::pressure::pascal;
use uom::si::specific_heat_capacity::joule_per_kilogram_kelvin;
use uom::si::specific_volume::cubic_meter_per_kilogram;
use uom::si::thermal_conductivity::watt_per_meter_kelvin;
use uom::si::thermodynamic_temperature::kelvin;

use crate::TampinesError;

/// Relative pressure change beyond which a cached [`SaturatedProperties`] is
/// considered stale and must be re-evaluated.
///
/// `1e-4` — a 0.7 kPa move at 7 MPa. Chosen because the saturation temperature
/// varies as roughly `dT_sat/dp ≈ 3e-6 K/Pa` near 7 MPa, so this bounds the
/// staleness of `T_sat` at about `2e-3` K, three orders below any temperature
/// difference a solver resolves.
pub const SATURATION_CACHE_TOLERANCE: f64 = 1.0e-4;

/// The IAPWS-IF97 lower pressure validity limit \[Pa\] — the triple point.
///
/// Below this the flash has no defined answer. Both solvers refuse rather than
/// clamp, for the reason set out in [`crate::multiphase_1d`]: a clamped
/// thermodynamic state produces a plausible number that is wrong.
pub const P_MIN_IF97: f64 = 611.657;

/// The IAPWS-IF97 upper pressure validity limit \[Pa\].
pub const P_MAX_IF97: f64 = 100.0e6;

/// Every saturated property at one pressure, plus the pressure derivatives the
/// semi-implicit pressure equation needs.
///
/// # What each field is
///
/// All on the saturation line at [`pressure`](Self::pressure), so
/// `T_f = T_g = T_sat` by definition — this is the *equilibrium* saturation
/// state, and a solver that wants thermal non-equilibrium (as
/// [`super::two_fluid::TwoFluid1d`] does) carries its own phase temperatures
/// and uses these only as the interface state.
///
/// # Units
///
/// Raw `f64`, strict SI: pascal, kelvin, `J/kg`, `kg/m³`, `Pa·s`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SaturatedProperties {
    /// The pressure these were evaluated at \[Pa\].
    pub pressure: f64,
    /// Saturation temperature `T_sat(p)` \[K\].
    pub t_sat: f64,
    /// Saturated-liquid specific enthalpy `h_f` \[J/kg\].
    pub h_f: f64,
    /// Saturated-vapour specific enthalpy `h_g` \[J/kg\].
    pub h_g: f64,
    /// Saturated-liquid density `ρ_f` \[kg/m³\].
    pub rho_f: f64,
    /// Saturated-vapour density `ρ_g` \[kg/m³\].
    pub rho_g: f64,
    /// Saturated-liquid dynamic viscosity `μ_f` \[Pa·s\].
    pub mu_f: f64,
    /// Saturated-vapour dynamic viscosity `μ_g` \[Pa·s\].
    pub mu_g: f64,
}

impl SaturatedProperties {
    /// Evaluate the whole saturated set at pressure `p` \[Pa\].
    ///
    /// # Errors
    ///
    /// [`TampinesError::Unphysical`] if `p` is outside
    /// `[P_MIN_IF97, P_MAX_IF97]` or is not finite. Refused, not clamped.
    pub fn at(p: f64) -> Result<Self, TampinesError> {
        if !p.is_finite() || !(P_MIN_IF97..=P_MAX_IF97).contains(&p) {
            return Err(TampinesError::Unphysical(format!(
                "pressure {p} Pa is outside the IAPWS-IF97 validity range \
                 [{P_MIN_IF97}, {P_MAX_IF97}] Pa; refused rather than clamped"
            )));
        }
        let p_q = Pressure::new::<pascal>(p);
        let t_sat_q = sat_temp_4(p_q);
        let t_sat = t_sat_q.get::<kelvin>();

        let h_f = h_tp_1(t_sat_q, p_q).get::<joule_per_kilogram>();
        let h_g = h_tp_2(t_sat_q, p_q).get::<joule_per_kilogram>();
        let v_f = v_tp_1(t_sat_q, p_q).get::<cubic_meter_per_kilogram>();
        let v_g = v_tp_2(t_sat_q, p_q).get::<cubic_meter_per_kilogram>();

        if !(v_f > 0.0) || !(v_g > 0.0) {
            return Err(TampinesError::Unphysical(format!(
                "IF97 returned a non-positive specific volume at p = {p} Pa \
                 (v_f = {v_f}, v_g = {v_g})"
            )));
        }

        // Viscosity on each side of the dome: the two-phase call at x = 0 and
        // x = 1 is exactly the single-phase liquid and vapour viscosity, and
        // taking both from one entry point keeps the two consistent.
        let mu_f = mu_tp_eqm_two_phase(t_sat_q, p_q, 0.0).get::<pascal_second>();
        let mu_g = mu_tp_eqm_two_phase(t_sat_q, p_q, 1.0).get::<pascal_second>();

        Ok(Self {
            pressure: p,
            t_sat,
            h_f,
            h_g,
            rho_f: 1.0 / v_f,
            rho_g: 1.0 / v_g,
            mu_f,
            mu_g,
        })
    }

    /// Latent heat of vaporisation `h_fg = h_g − h_f` \[J/kg\].
    #[must_use]
    pub fn h_fg(self) -> f64 {
        self.h_g - self.h_f
    }

    /// Whether this cached set is too stale to use at pressure `p` \[Pa\].
    ///
    /// True when `|p − p_cached| / p_cached` exceeds
    /// [`SATURATION_CACHE_TOLERANCE`].
    #[must_use]
    pub fn is_stale_for(self, p: f64) -> bool {
        (p - self.pressure).abs() > SATURATION_CACHE_TOLERANCE * self.pressure
    }

    /// Saturation temperature as a `uom` quantity, for callers outside the
    /// marching loop.
    #[must_use]
    pub fn saturation_temperature(self) -> ThermodynamicTemperature {
        ThermodynamicTemperature::new::<kelvin>(self.t_sat)
    }

    /// Saturated-liquid density as a `uom` quantity.
    #[must_use]
    pub fn liquid_density(self) -> MassDensity {
        MassDensity::new::<kilogram_per_cubic_meter>(self.rho_f)
    }

    /// Saturated-vapour density as a `uom` quantity.
    #[must_use]
    pub fn vapour_density(self) -> MassDensity {
        MassDensity::new::<kilogram_per_cubic_meter>(self.rho_g)
    }

    /// Latent heat as a `uom` quantity.
    #[must_use]
    pub fn latent_heat(self) -> AvailableEnergy {
        AvailableEnergy::new::<joule_per_kilogram>(self.h_fg())
    }
}

/// The thermodynamic state of one cell, obtained from a `(p, h)` flash.
///
/// # Sign and range conventions
///
/// - [`quality`](Self::quality) `x ∈ [0, 1]`: the **equilibrium** vapour mass
///   fraction. A subcooled cell returns `0`, a superheated cell `1` — clipped
///   deliberately, because the *thermodynamic* quality outside the dome is not
///   a mass fraction and feeding a negative one into a void-fraction formula
///   produces nonsense.
/// - [`void_fraction`](Self::void_fraction) `α ∈ [0, 1]`: the **volume**
///   fraction of vapour. Related to quality by
///   `α = x ρ_f / (x ρ_f + (1−x) ρ_g)` for a homogeneous mixture — note this
///   is the *no-slip* relation, so it is the correct initial value but a
///   drift-flux or two-fluid solver transports `α` independently thereafter
///   and the two stop agreeing. That divergence is the physics, not an error.
/// - [`density`](Self::density) `ρ_m` \[kg/m³\]: the mixture density
///   `α ρ_g + (1−α) ρ_f`.
///
/// # Units
///
/// Raw `f64` in strict SI.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TwoPhaseState {
    /// Pressure \[Pa\].
    pub pressure: f64,
    /// Mixture specific enthalpy \[J/kg\].
    pub enthalpy: f64,
    /// Equilibrium vapour mass fraction `x ∈ [0, 1]` \[-\].
    pub quality: f64,
    /// Vapour volume fraction `α ∈ [0, 1]` \[-\], at no slip.
    pub void_fraction: f64,
    /// Mixture density `ρ_m` \[kg/m³\].
    pub density: f64,
    /// Temperature \[K\].
    pub temperature: f64,
}

impl TwoPhaseState {
    /// Flash `(p, h)` to a full state, given the saturated set at that
    /// pressure.
    ///
    /// Passing `saturated` rather than re-evaluating it is deliberate: the
    /// caller usually has it already, and the two must be at the *same*
    /// pressure or the state is inconsistent — which the function checks.
    ///
    /// # Subcooled and superheated branches
    ///
    /// Outside the dome the mixture density is not `α ρ_g + (1−α) ρ_f` — with
    /// `x` clipped to `0` that formula would return `ρ_f` at *saturation*, not
    /// the density of the actual subcooled liquid, which is denser. So the
    /// single-phase branches evaluate IF97 directly at the cell temperature:
    ///
    /// - Subcooled (`h < h_f`): temperature from the Region-1 enthalpy
    ///   inverted by a secant iteration, then `ρ = 1/v(T, p)`.
    /// - Superheated (`h > h_g`): the same in Region 2.
    ///
    /// This matters for a blowdown, where the pipe starts subcooled and the
    /// initial mass inventory is set by exactly this density.
    ///
    /// # Errors
    ///
    /// [`TampinesError::Unphysical`] if `saturated` is at a different pressure
    /// than `p`, if `h` is not finite, or if the single-phase temperature
    /// inversion fails to converge.
    pub fn flash(p: f64, h: f64, saturated: SaturatedProperties) -> Result<Self, TampinesError> {
        if (saturated.pressure - p).abs() > 1.0e-9 * p.abs().max(1.0) {
            return Err(TampinesError::Unphysical(format!(
                "saturated set is at {} Pa but the flash is at {p} Pa",
                saturated.pressure
            )));
        }
        if !h.is_finite() {
            return Err(TampinesError::Unphysical(format!(
                "specific enthalpy {h} J/kg is not finite"
            )));
        }

        if h <= saturated.h_f {
            // Subcooled liquid: invert h_tp_1 for T at fixed p.
            let t = invert_region1_temperature(p, h, saturated)?;
            let v = v_tp_1(
                ThermodynamicTemperature::new::<kelvin>(t),
                Pressure::new::<pascal>(p),
            )
            .get::<cubic_meter_per_kilogram>();
            return Ok(Self {
                pressure: p,
                enthalpy: h,
                quality: 0.0,
                void_fraction: 0.0,
                density: 1.0 / v,
                temperature: t,
            });
        }

        if h >= saturated.h_g {
            // Superheated vapour: invert h_tp_2 for T at fixed p.
            let t = invert_region2_temperature(p, h, saturated)?;
            let v = v_tp_2(
                ThermodynamicTemperature::new::<kelvin>(t),
                Pressure::new::<pascal>(p),
            )
            .get::<cubic_meter_per_kilogram>();
            return Ok(Self {
                pressure: p,
                enthalpy: h,
                quality: 1.0,
                void_fraction: 1.0,
                density: 1.0 / v,
                temperature: t,
            });
        }

        // Inside the dome.
        let x = (h - saturated.h_f) / saturated.h_fg();
        let v_mix = x / saturated.rho_g + (1.0 - x) / saturated.rho_f;
        let alpha = (x / saturated.rho_g) / v_mix;
        Ok(Self {
            pressure: p,
            enthalpy: h,
            quality: x,
            void_fraction: alpha,
            density: 1.0 / v_mix,
            temperature: saturated.t_sat,
        })
    }

    /// Compressibility `ψ = ∂ρ/∂p|_h` \[s²/m²\] by central finite difference
    /// of the real flash at **fixed enthalpy**.
    ///
    /// # Why fixed enthalpy, and why this is the term that matters
    ///
    /// A segregated pressure solve freezes `h` while it corrects `p`, so
    /// `∂ρ/∂p|_h` is the correct linearisation — not the isothermal
    /// `ρ κ_T`. The difference is enormous inside the dome: the isothermal
    /// compressibility misses the flashing term `(v_g − v_f) dx/dp` entirely
    /// and comes out around two orders of magnitude too small, which lets the
    /// pressure fall straight through the saturation plateau instead of being
    /// pinned on it. The same correction was needed for the HEM array (bead
    /// `op-21g.14`); it is repeated here because the failure mode is identical.
    ///
    /// # Arguments
    ///
    /// `relative_step` — the finite-difference step as a fraction of `p`.
    /// `1e-4` is a reasonable default: large enough that the difference is not
    /// swamped by IF97's own round-off, small enough to stay local.
    ///
    /// # Errors
    ///
    /// [`TampinesError::Unphysical`] if either perturbed pressure leaves the
    /// IF97 range.
    pub fn compressibility(self, relative_step: f64) -> Result<f64, TampinesError> {
        let dp = (relative_step * self.pressure).max(1.0);
        let p_lo = (self.pressure - dp).max(P_MIN_IF97 * 1.000_001);
        let p_hi = (self.pressure + dp).min(P_MAX_IF97 * 0.999_999);
        let sat_lo = SaturatedProperties::at(p_lo)?;
        let sat_hi = SaturatedProperties::at(p_hi)?;
        let rho_lo = Self::flash(p_lo, self.enthalpy, sat_lo)?.density;
        let rho_hi = Self::flash(p_hi, self.enthalpy, sat_hi)?.density;
        Ok((rho_hi - rho_lo) / (p_hi - p_lo))
    }
}

// ─────────────────────────────────────────────────────────────────────────────
//  Six-equation additions (2026-08-11): per-phase metastable states and the
//  saturated conduction properties an interfacial heat-transfer closure needs.
//  Everything above this line is unchanged and is what `super::drift_flux`
//  uses; nothing below is on the drift-flux path, so the four extra IF97
//  evaluations `SaturatedTransport` costs are not paid by that solver.
// ─────────────────────────────────────────────────────────────────────────────

/// The IAPWS-IF97 Region-1 upper temperature limit \[K\].
///
/// Region 1 (subcooled liquid) is defined for `273.15 K ≤ T ≤ 623.15 K`. The
/// metastable-liquid extension in [`PhaseState::liquid_at`] never brackets
/// above this.
pub const T_MAX_REGION_1: f64 = 623.15;

/// The IAPWS-IF97 Region-2 upper temperature limit \[K\].
pub const T_MAX_REGION_2: f64 = 1073.15;

/// The IAPWS-IF97 lower temperature limit \[K\] — the triple point.
pub const T_MIN_IF97: f64 = 273.16;

/// How far above `T_sat(p)` the **metastable superheated liquid** branch of
/// [`PhaseState::liquid_at`] will go before refusing \[K\].
///
/// # Why this exists, and why it is bounded
///
/// A flashing blowdown does not put the liquid on the saturation line. The
/// pressure falls faster than the liquid can boil, so the liquid is left
/// **metastable — superheated relative to the local `T_sat(p)`** — and the
/// resulting superheat is precisely what drives the interfacial mass transfer
/// in [`super::two_fluid`]. A property layer that refuses `T_l > T_sat(p)`
/// cannot represent that state at all, which is why the equilibrium flash
/// [`TwoPhaseState::flash`] is not enough for a six-equation model.
///
/// `30.0 K`. IAPWS-IF97 publishes **no** metastable-liquid equation, so this
/// branch is an *extrapolation* of the Region-1 Gibbs equation past its stated
/// validity boundary — the same extrapolation the system codes make, and
/// smooth because the Region-1 equation is analytic. It is bounded rather than
/// unbounded because an extrapolation is only credible near the boundary:
/// measured flashing superheats in depressurisation transients are a few K to
/// a few tens of K, so 30 K covers the physics with margin while still
/// refusing a state that has clearly gone wrong. **Beyond this the state is
/// refused, not extrapolated** — see [`super::two_fluid`]'s
/// refusal-not-clamping rule.
pub const MAX_METASTABLE_LIQUID_SUPERHEAT: f64 = 30.0;

/// How far below `T_sat(p)` the **metastable subcooled vapour** branch of
/// [`PhaseState::vapour_at`] will go before refusing \[K\].
///
/// `30.0 K`, the mirror of [`MAX_METASTABLE_LIQUID_SUPERHEAT`] and bounded for
/// the same reason.
///
/// # Why the stable Region-2 equation is extrapolated rather than IAPWS's own
/// metastable-vapour equation
///
/// `tampines-steam-tables` does carry
/// [`tampines_steam_tables::region_2_vapour::metastable_region_2`], the IF97
/// metastable-vapour (supersaturated-steam) equation. It is **deliberately not
/// used here**, for two reasons:
///
/// 1. **It is a different equation, so it does not meet the stable one at the
///    saturation line.** [`SaturatedProperties`] takes `h_g` and `ρ_g` from the
///    *stable* Region-2 equation, and [`super::two_fluid`]'s interfacial energy
///    balance is built on the interface sitting exactly at those saturated
///    values. Switching formulation at `T_sat` would put a small step in
///    `h_g(T)` precisely at the state where every flashing term is evaluated,
///    and the exact cancellation the conservation test pins would be lost.
/// 2. **Its validity stops at 10 MPa** (IF97 restricts the metastable-vapour
///    equation to `p ≤ 10 MPa` and to the region above the 5 % equilibrium
///    moisture line), whereas this module's stated pressure range is the full
///    IF97 span. Using it would mean a *third* branch keyed on pressure.
///
/// Both choices are extrapolations of comparable size a few K from the dome;
/// the one taken here is the one that stays continuous with everything else in
/// this module. If a future case needs genuine supersaturated-steam accuracy —
/// nozzle condensation shocks, say — the metastable equation is the right tool
/// and this constant is where that decision would be revisited.
pub const MAX_METASTABLE_VAPOUR_SUBCOOLING: f64 = 30.0;

/// Relative pressure change beyond which a cached [`SaturatedTransport`] is
/// considered stale.
///
/// `1e-2`, a hundred times looser than [`SATURATION_CACHE_TOLERANCE`], because
/// the quantities it holds enter the answer far more weakly. `λ` and `c_p` set
/// the Nusselt/Prandtl scaling of the interfacial heat transfer, and both vary
/// by well under a percent over a 1 % pressure change away from the critical
/// point — whereas `T_sat` sets the *driving temperature difference* of the
/// same closure and therefore has to be tight. The looser tolerance matters:
/// [`SaturatedTransport::at`] costs four IF97 evaluations, one of them the
/// R15-11 conductivity with its critical-enhancement term, which is the most
/// expensive property call in this module.
pub const TRANSPORT_CACHE_TOLERANCE: f64 = 1.0e-2;

/// The saturation-line **conduction** properties an interfacial heat-transfer
/// closure needs, at one pressure.
///
/// # What each field is, and why they are here rather than in [`SaturatedProperties`]
///
/// [`super::two_fluid`]'s two-resistance closure
/// ([`outram_foam_multiphase::heat_transfer::InterfacialHeatTransfer`]) wants a
/// thermal conductivity per phase and a Prandtl number per phase; the Prandtl
/// number needs `c_p`. Those four numbers cost four extra IF97 evaluations, and
/// [`super::drift_flux`] — which evaluates [`SaturatedProperties`] several
/// times per cell per step and does not have an energy equation per phase —
/// needs none of them. Keeping them in a separate type means the four-equation
/// solver does not pay for the six-equation solver's closures.
///
/// # Where they are evaluated, and the approximation that implies
///
/// **On the saturation line at `pressure`**, via the two-phase entry points at
/// quality `0` and `1`, exactly as [`SaturatedProperties`] does for viscosity
/// and for the same reason: taking both phases from one entry point keeps them
/// mutually consistent.
///
/// The approximation: [`super::two_fluid`] evaluates these at `T_sat(p)` even
/// when the phase itself is metastable at `T_k ≠ T_sat(p)`. That is deliberate.
/// The single-phase IF97 conductivity entry point flashes its own density from
/// `(T, p)` and would route a metastable-liquid state — which sits at
/// `p < p_sat(T_l)` — into the *vapour* region, returning a conductivity an
/// order of magnitude wrong, silently. Over the bounded metastable departure
/// this module admits ([`MAX_METASTABLE_LIQUID_SUPERHEAT`]) the real variation
/// in `λ` and `c_p` is under a percent, so the saturation-line value is both
/// more accurate and vastly safer than the branch it avoids.
///
/// # Units
///
/// Raw `f64`, strict SI: pascal, `W/(m·K)`, `J/(kg·K)`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SaturatedTransport {
    /// The pressure these were evaluated at \[Pa\].
    pub pressure: f64,
    /// Saturated-liquid thermal conductivity `λ_f` \[W/(m·K)\].
    pub lambda_f: f64,
    /// Saturated-vapour thermal conductivity `λ_g` \[W/(m·K)\].
    pub lambda_g: f64,
    /// Saturated-liquid isobaric specific heat `c_p,f` \[J/(kg·K)\].
    pub cp_f: f64,
    /// Saturated-vapour isobaric specific heat `c_p,g` \[J/(kg·K)\].
    pub cp_g: f64,
}

impl SaturatedTransport {
    /// Evaluate the conduction set at pressure `p` \[Pa\].
    ///
    /// # Errors
    ///
    /// [`TampinesError::Unphysical`] if `p` is outside
    /// `[P_MIN_IF97, P_MAX_IF97]`, or if IF97 returns a non-positive
    /// conductivity or heat capacity — either would make a Prandtl number
    /// meaningless.
    pub fn at(p: f64) -> Result<Self, TampinesError> {
        if !p.is_finite() || !(P_MIN_IF97..=P_MAX_IF97).contains(&p) {
            return Err(TampinesError::Unphysical(format!(
                "pressure {p} Pa is outside the IAPWS-IF97 validity range \
                 [{P_MIN_IF97}, {P_MAX_IF97}] Pa; refused rather than clamped"
            )));
        }
        let p_q = Pressure::new::<pascal>(p);
        let t_sat_q = sat_temp_4(p_q);

        let lambda_f = lambda_tp_eqm_two_phase(t_sat_q, p_q, 0.0).get::<watt_per_meter_kelvin>();
        let lambda_g = lambda_tp_eqm_two_phase(t_sat_q, p_q, 1.0).get::<watt_per_meter_kelvin>();
        let cp_f = cp_tp_1(t_sat_q, p_q).get::<joule_per_kilogram_kelvin>();
        let cp_g = cp_tp_2(t_sat_q, p_q).get::<joule_per_kilogram_kelvin>();

        if !(lambda_f > 0.0) || !(lambda_g > 0.0) || !(cp_f > 0.0) || !(cp_g > 0.0) {
            return Err(TampinesError::Unphysical(format!(
                "IF97 returned a non-positive conduction property at p = {p} Pa \
                 (lambda_f = {lambda_f}, lambda_g = {lambda_g}, cp_f = {cp_f}, cp_g = {cp_g})"
            )));
        }

        Ok(Self {
            pressure: p,
            lambda_f,
            lambda_g,
            cp_f,
            cp_g,
        })
    }

    /// Whether this cached set is too stale to use at pressure `p` \[Pa\].
    ///
    /// True when `|p − p_cached| / p_cached` exceeds
    /// [`TRANSPORT_CACHE_TOLERANCE`].
    #[must_use]
    pub fn is_stale_for(self, p: f64) -> bool {
        (p - self.pressure).abs() > TRANSPORT_CACHE_TOLERANCE * self.pressure
    }

    /// Liquid Prandtl number `Pr_f = c_p,f μ_f / λ_f` \[-\].
    ///
    /// Takes the viscosity from the matching [`SaturatedProperties`] rather
    /// than holding its own copy, so the two cannot drift to different
    /// pressures unnoticed — which is also why this method takes `saturated`
    /// instead of a bare `μ`.
    ///
    /// # Errors
    ///
    /// [`TampinesError::Unphysical`] if `saturated` is at a different pressure.
    pub fn liquid_prandtl(self, saturated: SaturatedProperties) -> Result<f64, TampinesError> {
        self.check_same_pressure(saturated)?;
        Ok(self.cp_f * saturated.mu_f / self.lambda_f)
    }

    /// Vapour Prandtl number `Pr_g = c_p,g μ_g / λ_g` \[-\].
    ///
    /// # Errors
    ///
    /// [`TampinesError::Unphysical`] if `saturated` is at a different pressure.
    pub fn vapour_prandtl(self, saturated: SaturatedProperties) -> Result<f64, TampinesError> {
        self.check_same_pressure(saturated)?;
        Ok(self.cp_g * saturated.mu_g / self.lambda_g)
    }

    fn check_same_pressure(self, saturated: SaturatedProperties) -> Result<(), TampinesError> {
        if (saturated.pressure - self.pressure).abs()
            > TRANSPORT_CACHE_TOLERANCE * self.pressure.abs().max(1.0)
        {
            return Err(TampinesError::Unphysical(format!(
                "saturated conduction set is at {} Pa but the saturated property \
                 set is at {} Pa",
                self.pressure, saturated.pressure
            )));
        }
        Ok(())
    }

    /// Saturated-liquid thermal conductivity as a `uom` quantity.
    #[must_use]
    pub fn liquid_conductivity(self) -> uom::si::f64::ThermalConductivity {
        uom::si::f64::ThermalConductivity::new::<watt_per_meter_kelvin>(self.lambda_f)
    }

    /// Saturated-vapour thermal conductivity as a `uom` quantity.
    #[must_use]
    pub fn vapour_conductivity(self) -> uom::si::f64::ThermalConductivity {
        uom::si::f64::ThermalConductivity::new::<watt_per_meter_kelvin>(self.lambda_g)
    }
}

/// The state of **one** phase at `(p, h_k)`, including the bounded metastable
/// extensions a six-equation model needs.
///
/// # How this differs from [`TwoPhaseState`], and why both exist
///
/// [`TwoPhaseState::flash`] answers *"what mixture does this `(p, h)` describe
/// at equilibrium?"* — it puts the state on the saturation line whenever `h`
/// lands inside the dome. That is exactly right for HEM and for drift flux,
/// which carry one energy equation and therefore one temperature.
///
/// A six-equation model carries `h_g` and `h_l` **separately**, and the whole
/// point is that neither is required to sit on the saturation line. `h_l` below
/// `h_f` is ordinary subcooled liquid; `h_l` *above* `h_f` is metastable
/// superheated liquid, which the equilibrium flash would call a two-phase
/// mixture and which is instead the state that drives flashing. So this type
/// answers a different question: *"given that this is the liquid (or the
/// vapour), what temperature and density does `(p, h_k)` imply?"*
///
/// # Sign convention of [`metastable_departure`](Self::metastable_departure)
///
/// Positive means "further into the metastable region": for a liquid,
/// `T − T_sat` (superheat); for a vapour, `T_sat − T` (subcooling). Zero or
/// negative means the state is thermodynamically stable as that phase. A caller
/// that wants to know whether it is relying on the extrapolated branch reads
/// this field rather than re-deriving it.
///
/// # Units
///
/// Raw `f64`, strict SI: pascal, `J/kg`, kelvin, `kg/m³`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PhaseState {
    /// Pressure \[Pa\].
    pub pressure: f64,
    /// This phase's specific enthalpy \[J/kg\].
    pub enthalpy: f64,
    /// This phase's temperature \[K\].
    pub temperature: f64,
    /// This phase's density \[kg/m³\].
    pub density: f64,
    /// How far into the metastable region the state sits \[K\]; see the
    /// type-level docs for the sign convention. Zero or negative for a stable
    /// state.
    pub metastable_departure: f64,
}

impl PhaseState {
    /// The **liquid** state at `(p, h)`, allowing bounded metastable superheat.
    ///
    /// # Branches
    ///
    /// - `h ≤ h_f`: ordinary subcooled liquid, `T ∈ [273.16 K, T_sat]`, the
    ///   same Region-1 inversion [`TwoPhaseState::flash`] uses.
    /// - `h > h_f`: **metastable superheated liquid**, `T ∈ (T_sat, T_sat + Δ]`
    ///   with `Δ = ` [`MAX_METASTABLE_LIQUID_SUPERHEAT`] (capped at
    ///   [`T_MAX_REGION_1`]). The Region-1 Gibbs equation is evaluated past its
    ///   stated validity boundary — see [`MAX_METASTABLE_LIQUID_SUPERHEAT`] for
    ///   why that is the defensible choice and what its limits are.
    ///
    /// # Errors
    ///
    /// - [`TampinesError::Unphysical`] if `saturated` is at a different
    ///   pressure, if `h` is not finite, or if `h` exceeds the enthalpy at the
    ///   top of the metastable bracket. **The last case is a refusal, not a
    ///   clamp**: a liquid more than
    ///   [`MAX_METASTABLE_LIQUID_SUPERHEAT`] superheated is outside anything
    ///   this property layer can honestly evaluate, and returning the bracket
    ///   endpoint would present an extrapolation as an answer.
    /// - [`TampinesError::Numerical`] if the inversion cannot bracket the root.
    pub fn liquid_at(p: f64, h: f64, saturated: SaturatedProperties) -> Result<Self, TampinesError> {
        check_flash_inputs(p, h, saturated)?;
        let p_q = Pressure::new::<pascal>(p);
        let residual =
            |t: f64| h_tp_1(ThermodynamicTemperature::new::<kelvin>(t), p_q).get::<joule_per_kilogram>() - h;

        let (t, departure) = if h <= saturated.h_f {
            let t = invert_monotone_fast(residual, T_MIN_IF97, saturated.t_sat, "Region 1 (subcooled liquid)")?;
            (t, t - saturated.t_sat)
        } else {
            // Metastable superheated liquid: extend the bracket above T_sat by
            // the documented margin, and refuse beyond it rather than
            // extrapolating further.
            let hi = (saturated.t_sat + MAX_METASTABLE_LIQUID_SUPERHEAT).min(T_MAX_REGION_1.max(saturated.t_sat));
            let h_hi = residual(hi) + h;
            if h > h_hi {
                return Err(TampinesError::Unphysical(format!(
                    "liquid enthalpy {h} J/kg at p = {p} Pa implies a superheat beyond the \
                     {MAX_METASTABLE_LIQUID_SUPERHEAT} K metastable bound (h at T_sat + bound = \
                     {h_hi} J/kg, T_sat = {} K); refused rather than extrapolated",
                    saturated.t_sat
                )));
            }
            let t = invert_monotone_fast(residual, saturated.t_sat, hi, "Region 1 (metastable superheated liquid)")?;
            (t, t - saturated.t_sat)
        };

        let v = v_tp_1(
            ThermodynamicTemperature::new::<kelvin>(t),
            p_q,
        )
        .get::<cubic_meter_per_kilogram>();
        if !(v > 0.0) || !v.is_finite() {
            return Err(TampinesError::Unphysical(format!(
                "IF97 returned a non-positive liquid specific volume {v} m^3/kg at \
                 p = {p} Pa, T = {t} K"
            )));
        }
        Ok(Self {
            pressure: p,
            enthalpy: h,
            temperature: t,
            density: 1.0 / v,
            metastable_departure: departure,
        })
    }

    /// The **vapour** state at `(p, h)`, allowing bounded metastable
    /// subcooling.
    ///
    /// # Branches
    ///
    /// - `h ≥ h_g`: ordinary superheated vapour, `T ∈ [T_sat, 1073.15 K]`, the
    ///   same Region-2 inversion [`TwoPhaseState::flash`] uses.
    /// - `h < h_g`: **metastable subcooled vapour**, `T ∈ [T_sat − Δ, T_sat)`
    ///   with `Δ = ` [`MAX_METASTABLE_VAPOUR_SUBCOOLING`]. The *stable*
    ///   Region-2 equation is extrapolated below `T_sat`; see
    ///   [`MAX_METASTABLE_VAPOUR_SUBCOOLING`] for why IAPWS's own
    ///   metastable-vapour equation is deliberately not used.
    ///
    /// # Errors
    ///
    /// As [`liquid_at`](Self::liquid_at), with the bound applied on the
    /// subcooled side.
    pub fn vapour_at(p: f64, h: f64, saturated: SaturatedProperties) -> Result<Self, TampinesError> {
        check_flash_inputs(p, h, saturated)?;
        let p_q = Pressure::new::<pascal>(p);
        let residual =
            |t: f64| h_tp_2(ThermodynamicTemperature::new::<kelvin>(t), p_q).get::<joule_per_kilogram>() - h;

        let (t, departure) = if h >= saturated.h_g {
            let t = invert_monotone_fast(residual, saturated.t_sat, T_MAX_REGION_2, "Region 2 (superheated vapour)")?;
            (t, saturated.t_sat - t)
        } else {
            let lo = (saturated.t_sat - MAX_METASTABLE_VAPOUR_SUBCOOLING).max(T_MIN_IF97.min(saturated.t_sat));
            let h_lo = residual(lo) + h;
            if h < h_lo {
                return Err(TampinesError::Unphysical(format!(
                    "vapour enthalpy {h} J/kg at p = {p} Pa implies a subcooling beyond the \
                     {MAX_METASTABLE_VAPOUR_SUBCOOLING} K metastable bound (h at T_sat - bound = \
                     {h_lo} J/kg, T_sat = {} K); refused rather than extrapolated",
                    saturated.t_sat
                )));
            }
            let t = invert_monotone_fast(residual, lo, saturated.t_sat, "Region 2 (metastable subcooled vapour)")?;
            (t, saturated.t_sat - t)
        };

        let v = v_tp_2(
            ThermodynamicTemperature::new::<kelvin>(t),
            p_q,
        )
        .get::<cubic_meter_per_kilogram>();
        if !(v > 0.0) || !v.is_finite() {
            return Err(TampinesError::Unphysical(format!(
                "IF97 returned a non-positive vapour specific volume {v} m^3/kg at \
                 p = {p} Pa, T = {t} K"
            )));
        }
        Ok(Self {
            pressure: p,
            enthalpy: h,
            temperature: t,
            density: 1.0 / v,
            metastable_departure: departure,
        })
    }

    /// Temperature as a `uom` quantity, for callers outside the marching loop.
    #[must_use]
    pub fn thermodynamic_temperature(self) -> ThermodynamicTemperature {
        ThermodynamicTemperature::new::<kelvin>(self.temperature)
    }

    /// Density as a `uom` quantity.
    #[must_use]
    pub fn mass_density(self) -> MassDensity {
        MassDensity::new::<kilogram_per_cubic_meter>(self.density)
    }
}

/// Shared precondition check for the `(p, h, saturated)` triple.
fn check_flash_inputs(
    p: f64,
    h: f64,
    saturated: SaturatedProperties,
) -> Result<(), TampinesError> {
    if (saturated.pressure - p).abs() > 1.0e-9 * p.abs().max(1.0) {
        return Err(TampinesError::Unphysical(format!(
            "saturated set is at {} Pa but the phase state is at {p} Pa",
            saturated.pressure
        )));
    }
    if !h.is_finite() {
        return Err(TampinesError::Unphysical(format!(
            "specific enthalpy {h} J/kg is not finite"
        )));
    }
    Ok(())
}

/// Invert `h_tp_1(T, p) = h` for `T`, by a bracketed secant with bisection
/// fallback.
///
/// The bracket is `[273.16 K, T_sat(p)]` — Region 1 spans exactly that at a
/// given pressure, and `h_tp_1` is strictly increasing in `T` there, so a root
/// exists whenever `h ≤ h_f` and lies inside.
fn invert_region1_temperature(
    p: f64,
    h: f64,
    saturated: SaturatedProperties,
) -> Result<f64, TampinesError> {
    let p_q = Pressure::new::<pascal>(p);
    let f = |t: f64| {
        h_tp_1(ThermodynamicTemperature::new::<kelvin>(t), p_q).get::<joule_per_kilogram>() - h
    };
    invert_monotone(f, 273.16, saturated.t_sat, "Region 1 (subcooled liquid)")
}

/// Invert `h_tp_2(T, p) = h` for `T`, bracketed on `[T_sat(p), 1073.15 K]` —
/// the Region-2 span at a given pressure, on which `h_tp_2` increases
/// monotonically.
fn invert_region2_temperature(
    p: f64,
    h: f64,
    saturated: SaturatedProperties,
) -> Result<f64, TampinesError> {
    let p_q = Pressure::new::<pascal>(p);
    let f = |t: f64| {
        h_tp_2(ThermodynamicTemperature::new::<kelvin>(t), p_q).get::<joule_per_kilogram>() - h
    };
    invert_monotone(f, saturated.t_sat, 1073.15, "Region 2 (superheated vapour)")
}

/// Bracketed **Illinois** (modified regula-falsi) inversion of a monotone
/// increasing residual over `[lo, hi]`, for the per-phase states.
///
/// # Why a second inverter exists next to [`invert_monotone`]
///
/// Same contract, different cost. [`invert_monotone`] takes 60 unconditional
/// bisections; that is 60 IF97 evaluations, which [`super::drift_flux`] pays
/// only in a *single-phase* cell. [`super::two_fluid`] needs a temperature
/// inversion for **both** phases in **every** cell on **every** outer
/// corrector, so 60 evaluations per inversion dominates its runtime.
///
/// Illinois is the right replacement rather than a plain secant: it keeps the
/// root bracketed at all times (so it cannot run away on the extrapolated
/// metastable branch, where the residual is smooth but no longer guaranteed to
/// behave like the in-region one), while converging superlinearly on a
/// near-linear residual — and `h(T)` at fixed `p` is very nearly linear over
/// these brackets. The safeguard clamps any false-position step that crowds
/// within 1 % of an endpoint back to the midpoint, which bounds the worst case
/// at bisection speed.
///
/// [`invert_monotone`] is deliberately left in place and unchanged: the
/// drift-flux results recorded in `edwards_tests.rs` were measured against it,
/// and swapping the inverter under a gated V&V benchmark would invalidate those
/// numbers for no physical gain.
///
/// Terminates when the bracket is narrower than `1e-9` K — six orders below any
/// temperature difference either solver resolves.
///
/// Same out-of-bracket convention as [`invert_monotone`]: if the requested
/// value lies outside `[f(lo), f(hi)]` the endpoint is returned, because that
/// happens when a cell sits a hair outside a region boundary through round-off.
/// Callers that must *refuse* an out-of-range request (the metastable bounds)
/// check before calling.
fn invert_monotone_fast(
    f: impl Fn(f64) -> f64,
    lo: f64,
    hi: f64,
    region: &'static str,
) -> Result<f64, TampinesError> {
    if hi == lo {
        return Ok(lo);
    }
    if !(hi > lo) {
        return Err(TampinesError::Numerical(format!(
            "{region}: degenerate temperature bracket [{lo}, {hi}] K"
        )));
    }
    let (mut a, mut b) = (lo, hi);
    let (mut fa, mut fb) = (f(a), f(b));
    if !fa.is_finite() || !fb.is_finite() {
        return Err(TampinesError::Numerical(format!(
            "{region}: IF97 returned a non-finite enthalpy on the bracket [{lo}, {hi}] K"
        )));
    }
    if fa >= 0.0 {
        return Ok(a);
    }
    if fb <= 0.0 {
        return Ok(b);
    }

    for _ in 0..80 {
        if b - a <= 1.0e-9 {
            break;
        }
        let width = b - a;
        let mut m = (a * fb - b * fa) / (fb - fa);
        if !m.is_finite() || m < a + 0.01 * width || m > b - 0.01 * width {
            m = 0.5 * (a + b);
        }
        let fm = f(m);
        if !fm.is_finite() {
            return Err(TampinesError::Numerical(format!(
                "{region}: IF97 returned a non-finite enthalpy at T = {m} K"
            )));
        }
        if fm < 0.0 {
            a = m;
            fa = fm;
            fb *= 0.5;
        } else if fm > 0.0 {
            b = m;
            fb = fm;
            fa *= 0.5;
        } else {
            return Ok(m);
        }
    }
    Ok(0.5 * (a + b))
}

/// Bisection on a monotone increasing residual over `[lo, hi]`.
///
/// Bisection rather than a secant or Newton: the IF97 backward enthalpy is
/// smooth but its derivative is not cheap, and 60 bisections on a 500 K
/// bracket reach `4e-16` K — far below anything the solve resolves — for 60
/// forward evaluations. Robustness is worth more than iteration count here,
/// because this runs inside a transient that must not fall over.
///
/// If the requested value lies outside `[f(lo), f(hi)]` the endpoint is
/// returned rather than an error: that happens when a cell sits a hair outside
/// the region boundary through round-off, and the endpoint is the right answer
/// there.
fn invert_monotone(
    f: impl Fn(f64) -> f64,
    lo: f64,
    hi: f64,
    region: &'static str,
) -> Result<f64, TampinesError> {
    if !(hi > lo) {
        return Err(TampinesError::Numerical(format!(
            "{region}: degenerate temperature bracket [{lo}, {hi}] K"
        )));
    }
    let f_lo = f(lo);
    let f_hi = f(hi);
    if !f_lo.is_finite() || !f_hi.is_finite() {
        return Err(TampinesError::Numerical(format!(
            "{region}: IF97 returned a non-finite enthalpy on the bracket \
             [{lo}, {hi}] K"
        )));
    }
    if f_lo >= 0.0 {
        return Ok(lo);
    }
    if f_hi <= 0.0 {
        return Ok(hi);
    }

    let (mut a, mut b) = (lo, hi);
    for _ in 0..60 {
        let m = 0.5 * (a + b);
        if f(m) < 0.0 {
            a = m;
        } else {
            b = m;
        }
    }
    Ok(0.5 * (a + b))
}
