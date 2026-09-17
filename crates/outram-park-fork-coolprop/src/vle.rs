//! Vapour–liquid equilibrium: the thermodynamically-consistent saturation state
//! from the EOS itself (not just the ancillary correlations), plus the
//! two-phase `(p, h)` quality flash.
//!
//! # Maxwell construction
//!
//! At a temperature `T < T_c`, the coexisting liquid (`ρ'`) and vapour (`ρ''`)
//! densities satisfy **equal pressure** and **equal Gibbs energy**. In reduced
//! form (`δ = ρ_molar/ρ_r`, `τ = T_r/T`, `α^r` the residual reduced Helmholtz
//! energy) these are
//!
//! $$ \delta'(1 + \delta'\alpha^r_\delta) = \delta''(1 + \delta''\alpha^r_\delta) \quad (\text{equal } p) $$
//! $$ \ln(\delta'/\delta'') + (\alpha^r{}' - \alpha^r{}'') + (\delta'\alpha^r_\delta{}' - \delta''\alpha^r_\delta{}'') = 0 \quad (\text{equal } g) $$
//!
//! solved by Newton in `(δ', δ'')` from the ancillary densities
//! ([`crate::ancillaries`]) as the initial guess. This is the standard
//! saturated-density solve (e.g. Span; Akasaka 2008).
//!
//! # Scope
//!
//! Pure-fluid VLE only, and only where the fluid ships saturation ancillaries
//! (for the initial guess). Near the critical point the two densities merge and
//! the Newton system stiffens; callers get [`None`] rather than a wrong number.

use crate::ancillaries::FluidAncillaries;
use crate::fluid::Fluid;
use crate::props::state_trho;

/// A converged saturation state at one temperature.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SaturationState {
    /// Saturation temperature \[K\].
    pub temperature: f64,
    /// Saturation (vapour) pressure \[Pa\].
    pub pressure: f64,
    /// Saturated-liquid mass density \[kg/m³\].
    pub rho_liquid: f64,
    /// Saturated-vapour mass density \[kg/m³\].
    pub rho_vapour: f64,
}

/// Reduced residual `(α^r, α^r_δ, α^r_δδ)` at `(δ, τ)`.
fn residual_at(fluid: Fluid, delta: f64, tau: f64) -> (f64, f64, f64) {
    let d = fluid.eos().residual_derivs(delta, tau);
    (d.a, d.ad, d.add)
}

/// Solve the Maxwell VLE at temperature `t` \[K\]. Returns the saturation
/// pressure and the two coexisting densities, or [`None`] if the fluid has no
/// ancillaries, `t` is not sub-critical, or the Newton solve does not converge
/// (e.g. too close to `T_c`).
pub fn saturation_at_temperature(fluid: Fluid, t: f64) -> Option<SaturationState> {
    let eos = fluid.eos();
    let anc: &FluidAncillaries = fluid.ancillaries()?;
    if !(t.is_finite() && t > 0.0) || t >= eos.t_critical {
        return None;
    }
    let r = eos.gas_constant;
    let m = eos.molar_mass;
    let rho_r = eos.rho_reducing;
    let tau = eos.t_reducing / t;

    // Initial guess from the ancillary densities (molar → reduced).
    let mut delta_l = anc.rho_l.evaluate(t) / rho_r;
    let mut delta_v = anc.rho_v.evaluate(t) / rho_r;
    if !(delta_l.is_finite() && delta_v.is_finite() && delta_l > 0.0 && delta_v > 0.0) {
        return None;
    }

    for _ in 0..80 {
        let (a_l, ad_l, add_l) = residual_at(fluid, delta_l, tau);
        let (a_v, ad_v, add_v) = residual_at(fluid, delta_v, tau);

        // Residuals: F1 = equal pressure, F2 = equal Gibbs (both divided out).
        let big_a_l = delta_l * (1.0 + delta_l * ad_l);
        let big_a_v = delta_v * (1.0 + delta_v * ad_v);
        let f1 = big_a_l - big_a_v;
        let f2 = (delta_l / delta_v).ln() + (a_l - a_v) + (delta_l * ad_l - delta_v * ad_v);

        // dA_i/dδ_i = 1 + 2 δ_i α_δ + δ_i² α_δδ  (the isothermal p-slope factor).
        let da_l = 1.0 + 2.0 * delta_l * ad_l + delta_l * delta_l * add_l;
        let da_v = 1.0 + 2.0 * delta_v * ad_v + delta_v * delta_v * add_v;
        // Jacobian rows: [dF1/dδ', dF1/dδ''; dF2/dδ', dF2/dδ''].
        let (j11, j12) = (da_l, -da_v);
        let (j21, j22) = (da_l / delta_l, -da_v / delta_v);
        let det = j11 * j22 - j12 * j21;
        if det.abs() < 1e-300 {
            return None;
        }
        // Newton step: Δ = -J⁻¹ F.
        let dl = -(j22 * f1 - j12 * f2) / det;
        let dv = -(-j21 * f1 + j11 * f2) / det;
        delta_l += dl;
        delta_v += dv;
        if !(delta_l > 0.0 && delta_v > 0.0) {
            return None;
        }
        if dl.abs() <= 1e-12 * delta_l && dv.abs() <= 1e-12 * delta_v {
            let (_, ad_l2, _) = residual_at(fluid, delta_l, tau);
            let p = delta_l * (1.0 + delta_l * ad_l2) * rho_r * r * t;
            if !(p.is_finite() && p > 0.0) {
                return None;
            }
            return Some(SaturationState {
                temperature: t,
                pressure: p,
                rho_liquid: delta_l * rho_r * m,
                rho_vapour: delta_v * rho_r * m,
            });
        }
    }
    None
}

/// Saturation temperature \[K\] at pressure `p` \[Pa\] — the inverse of
/// [`saturation_at_temperature`], by bisection over `(T_triple, T_c)` on the
/// EOS saturation pressure (monotone in `T`). [`None`] if `p` is outside the
/// vapour-pressure range or the fluid has no ancillaries.
pub fn saturation_temperature(fluid: Fluid, p: f64) -> Option<f64> {
    let eos = fluid.eos();
    fluid.ancillaries()?; // require ancillaries
    if !(p.is_finite() && p > 0.0) || p >= eos.p_critical {
        return None;
    }
    let mut lo = eos.t_triple.max(1.0);
    let mut hi = eos.t_critical * (1.0 - 1e-4);
    // Require p above the low-T end (else it's below the vapour-pressure range).
    // The VLE solve fails right at Tc, so the bisection below (None ⇒ shrink hi)
    // handles the top of the bracket without an upfront probe there.
    if let Some(s_lo) = saturation_at_temperature(fluid, lo) {
        if p <= s_lo.pressure {
            return None;
        }
    }
    for _ in 0..100 {
        let mid = 0.5 * (lo + hi);
        match saturation_at_temperature(fluid, mid) {
            Some(s) => {
                if (s.pressure - p).abs() <= 1e-8 * p {
                    return Some(mid);
                }
                if s.pressure < p {
                    lo = mid;
                } else {
                    hi = mid;
                }
            }
            // VLE failed at mid (too near Tc): shrink the top of the bracket.
            None => hi = mid,
        }
    }
    Some(0.5 * (lo + hi))
}

/// Where a `(p, h)` state sits relative to the saturation dome.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PhaseAtPh {
    /// Single-phase (subcooled/compressed liquid or superheated/supercritical
    /// vapour) — use the single-phase [`crate::flash`].
    SinglePhase,
    /// Inside the dome: two-phase at `temperature` \[K\], vapour quality
    /// `quality` \[-\], and mixture mass density `density` \[kg/m³\].
    TwoPhase {
        temperature: f64,
        quality: f64,
        density: f64,
    },
}

/// Classify a `(p, h)` state (pressure \[Pa\], specific enthalpy \[J/kg\]) as
/// single- or two-phase, returning the quality and temperature when two-phase.
///
/// Sub-critical only: finds `T_sat(p)`, then the saturated-phase enthalpies
/// `h'`,`h''`; `h ∈ [h', h'']` ⇒ two-phase with `x = (h−h')/(h''−h')`. Above the
/// critical pressure (or when the fluid has no ancillaries) it returns
/// [`PhaseAtPh::SinglePhase`] — the two-phase region does not apply.
pub fn phase_at_ph(fluid: Fluid, p: f64, h: f64) -> PhaseAtPh {
    let Some(t_sat) = saturation_temperature(fluid, p) else {
        return PhaseAtPh::SinglePhase;
    };
    let Some(sat) = saturation_at_temperature(fluid, t_sat) else {
        return PhaseAtPh::SinglePhase;
    };
    let h_l = state_trho(fluid, t_sat, sat.rho_liquid).enthalpy;
    let h_v = state_trho(fluid, t_sat, sat.rho_vapour).enthalpy;
    if h >= h_l && h <= h_v && h_v > h_l {
        let x = (h - h_l) / (h_v - h_l);
        // Mixture density from specific volumes: 1/ρ = (1−x)/ρ' + x/ρ''.
        let v = (1.0 - x) / sat.rho_liquid + x / sat.rho_vapour;
        PhaseAtPh::TwoPhase {
            temperature: t_sat,
            quality: x,
            density: 1.0 / v,
        }
    } else {
        PhaseAtPh::SinglePhase
    }
}
