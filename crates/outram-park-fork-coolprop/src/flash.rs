//! Single-phase flashes: `(p, T)`, `(p, h)`, `(p, s)` → a full [`FluidState`].
//!
//! The EOS's natural inputs are `(T, ρ)` ([`crate::props::state_trho`], a direct
//! evaluation). Every other input pair needs an **iterative density (and/or
//! temperature) solve**, which is what this module provides:
//!
//! - **`(p, T)`** — Newton on the mass density `ρ` so that `p(T, ρ) = p`, using
//!   the analytic isothermal derivative `(∂p/∂ρ)_T`, with a bisection fallback.
//! - **`(p, h)`** — outer Newton on `T` (step `ΔT = -(h - h_tgt)/c_p`, since
//!   `(∂h/∂T)_p = c_p`), each iteration solving `ρ` from `(p, T)`.
//! - **`(p, s)`** — outer Newton on `T` (step `ΔT = -(s - s_tgt)·T/c_p`, since
//!   `(∂s/∂T)_p = c_p/T`), each iteration solving `ρ` from `(p, T)`.
//!
//! # Scope / limitations
//!
//! These are **single-phase** solvers: they assume the target state is a single
//! stable phase and return the branch reached from an ideal-gas / supercritical
//! initial guess (the vapour / supercritical root). CoolProp's saturation-dome /
//! two-phase quality flash is **not** modelled here (bead op-kbc) — inside the
//! two-phase dome `(∂p/∂ρ)_T` changes sign and these solves will return
//! [`FlashError::NonConvergent`] rather than a wrong number. All quantities are
//! raw SI `f64` (kelvin, pascal, kg/m³, J/kg, J/(kg·K)); the `uom`-typed wrapper
//! is [`crate::single_cv::OPCPFluidSingleCV`].

use crate::fluid::Fluid;
use crate::props::{state_trho, FluidState};

/// Why a flash solve did not produce a state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlashError {
    /// The iteration did not converge within its budget — typically because the
    /// requested state lies inside the two-phase dome (not modelled) or outside
    /// the single-phase region the solver can reach.
    NonConvergent,
    /// A non-physical input (temperature, pressure or density ≤ 0).
    NonPhysicalInput,
}

/// Pressure `p` \[Pa\] and its isothermal derivative `(∂p/∂ρ)_T`
/// \[Pa·m³/kg\] at mass density `rho` \[kg/m³\] and temperature `t` \[K\].
fn pressure_and_dpdrho(fluid: Fluid, t: f64, rho: f64) -> (f64, f64) {
    let eos = fluid.eos();
    let r = eos.gas_constant; // J/(mol·K)
    let m = eos.molar_mass; //   kg/mol
    let rho_molar = rho / m;
    let delta = rho_molar / eos.rho_reducing;
    let tau = eos.t_reducing / t;
    let res = eos.residual_derivs(delta, tau);
    let p = rho_molar * r * t * (1.0 + delta * res.ad);
    // (∂p/∂ρ_molar)_T = R T (1 + 2 δ α_δ + δ² α_δδ); divide by M for mass basis.
    let den = 1.0 + 2.0 * delta * res.ad + delta * delta * res.add;
    let dpdrho = r * t * den / m;
    (p, dpdrho)
}

/// Solve for the single-phase mass density `ρ` \[kg/m³\] at temperature `t`
/// \[K\] and pressure `p` \[Pa\].
///
/// Newton from the ideal-gas guess `ρ₀ = p·M/(R·T)` (which lands on the vapour /
/// supercritical branch), with damping to keep `ρ > 0`, and a bracketing
/// bisection fallback. Returns [`FlashError::NonConvergent`] if no single-phase
/// root with `(∂p/∂ρ)_T > 0` is found (e.g. a two-phase target).
pub fn density_pt(fluid: Fluid, t: f64, p: f64) -> Result<f64, FlashError> {
    let inputs_ok = t.is_finite() && t > 0.0 && p.is_finite() && p > 0.0;
    if !inputs_ok {
        return Err(FlashError::NonPhysicalInput);
    }
    let eos = fluid.eos();
    let rho_ideal = p * eos.molar_mass / (eos.gas_constant * t);
    let tol = p * 1e-11 + 1e-9;

    // Newton with positivity damping.
    let mut rho = rho_ideal;
    for _ in 0..100 {
        let (p_cur, dpdrho) = pressure_and_dpdrho(fluid, t, rho);
        if (p_cur - p).abs() <= tol && dpdrho > 0.0 {
            return Ok(rho);
        }
        if dpdrho <= 0.0 {
            break; // entered the non-monotonic (two-phase) region — bisect instead
        }
        let mut step = (p_cur - p) / dpdrho;
        // Limit the step so density stays positive and does not jump too far.
        while rho - step <= 0.0 {
            step *= 0.5;
        }
        let next = rho - step;
        if (next - rho).abs() <= 1e-14 * rho {
            return if (p_cur - p).abs() <= tol { Ok(next) } else { Err(FlashError::NonConvergent) };
        }
        rho = next;
    }

    // Bisection fallback: scan geometrically for the lowest-density bracket where
    // p(ρ) crosses the target on a branch with (∂p/∂ρ)_T > 0.
    let rho_max = (5.0 * eos.rho_critical * eos.molar_mass).max(2000.0);
    let mut rho_prev = 1e-8_f64;
    let (mut g_prev, _) = {
        let (pp, dd) = pressure_and_dpdrho(fluid, t, rho_prev);
        (pp - p, dd)
    };
    let steps = 400;
    for i in 1..=steps {
        let frac = i as f64 / steps as f64;
        let rho_cur = rho_prev.max(1e-8) * (rho_max / 1e-8).powf(1.0 / steps as f64);
        let _ = frac;
        let (p_cur, dpdrho) = pressure_and_dpdrho(fluid, t, rho_cur);
        let g_cur = p_cur - p;
        if g_prev * g_cur <= 0.0 && dpdrho > 0.0 {
            // Bracketed a rising root in [rho_prev, rho_cur]; bisect.
            let (mut lo, mut hi) = (rho_prev, rho_cur);
            for _ in 0..200 {
                let mid = 0.5 * (lo + hi);
                let (p_mid, _) = pressure_and_dpdrho(fluid, t, mid);
                if (p_mid - p).abs() <= tol {
                    return Ok(mid);
                }
                if (p_mid - p) * g_prev > 0.0 {
                    lo = mid;
                } else {
                    hi = mid;
                }
            }
            return Ok(0.5 * (lo + hi));
        }
        rho_prev = rho_cur;
        g_prev = g_cur;
    }
    Err(FlashError::NonConvergent)
}

/// Full single-phase state from temperature `t` \[K\] and pressure `p` \[Pa\].
pub fn state_pt(fluid: Fluid, t: f64, p: f64) -> Result<FluidState, FlashError> {
    let rho = density_pt(fluid, t, p)?;
    Ok(state_trho(fluid, t, rho))
}

/// Full single-phase state from pressure `p` \[Pa\] and specific enthalpy `h`
/// \[J/kg\]. Outer Newton on `T` (`ΔT = -(h - h_tgt)/c_p`).
pub fn state_ph(fluid: Fluid, p: f64, h: f64) -> Result<FluidState, FlashError> {
    solve_pt_outer(fluid, p, |s| (s.enthalpy - h, s.cp))
}

/// Full single-phase state from pressure `p` \[Pa\] and specific entropy `s`
/// \[J/(kg·K)\]. Outer Newton on `T` (`ΔT = -(s - s_tgt)·T/c_p`).
pub fn state_ps(fluid: Fluid, p: f64, s: f64) -> Result<FluidState, FlashError> {
    solve_pt_outer(fluid, p, |st| ((st.entropy - s) * st.temperature, st.cp))
}

/// Shared outer Newton for `(p, h)` / `(p, s)`: iterate `T` at fixed `p`.
///
/// `residual_and_slope(state)` returns `(f, c_p)` where the Newton step is
/// `ΔT = -f / c_p` — for `(p, h)`, `f = h - h_tgt`; for `(p, s)`,
/// `f = (s - s_tgt)·T` (so dividing by `c_p` gives the correct step, since
/// `(∂s/∂T)_p = c_p/T`).
fn solve_pt_outer(
    fluid: Fluid,
    p: f64,
    residual_and_slope: impl Fn(&FluidState) -> (f64, f64),
) -> Result<FluidState, FlashError> {
    if !(p.is_finite() && p > 0.0) {
        return Err(FlashError::NonPhysicalInput);
    }
    let eos = fluid.eos();
    // Start supercritical so the first (p, T) density solve is well-posed.
    let mut t = 1.1 * eos.t_critical;
    let t_min = eos.t_triple.max(1.0);
    for _ in 0..100 {
        let st = state_pt(fluid, t, p)?;
        let (f, cp) = residual_and_slope(&st);
        if cp <= 0.0 {
            return Err(FlashError::NonConvergent);
        }
        // Converge on the Newton temperature step: `ΔT = -f/c_p`. This is robust
        // near the critical point, where c_p (hence the true `∂h/∂T|_p`) is huge,
        // so a tiny ΔT already nulls the residual — an `f`-magnitude tolerance
        // would stop too early and leave `ρ` (hypersensitive to `T` there) off.
        let dt_newton = -f / cp;
        if dt_newton.abs() <= 1e-11 * t {
            return Ok(st);
        }
        let mut dt = dt_newton;
        // Damp so T stays positive and does not leap more than 30% per step.
        let max_step = 0.3 * t;
        if dt > max_step {
            dt = max_step;
        } else if dt < -max_step {
            dt = -max_step;
        }
        let mut next = t + dt;
        if next < t_min {
            next = 0.5 * (t + t_min);
        }
        if (next - t).abs() <= 1e-10 * t {
            return Ok(st);
        }
        t = next;
    }
    Err(FlashError::NonConvergent)
}
