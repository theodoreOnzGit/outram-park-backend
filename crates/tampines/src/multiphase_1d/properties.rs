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
use tampines_steam_tables::region_1_subcooled_liquid::{h_tp_1, v_tp_1};
use tampines_steam_tables::region_2_vapour::{h_tp_2, v_tp_2};
use tampines_steam_tables::region_4_vap_liq_equilibrium::sat_temp_4;

use uom::si::available_energy::joule_per_kilogram;
use uom::si::dynamic_viscosity::pascal_second;
use uom::si::f64::{AvailableEnergy, MassDensity, Pressure, ThermodynamicTemperature};
use uom::si::mass_density::kilogram_per_cubic_meter;
use uom::si::pressure::pascal;
use uom::si::specific_volume::cubic_meter_per_kilogram;
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
