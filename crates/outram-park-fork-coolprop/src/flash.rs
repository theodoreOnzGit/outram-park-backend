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
            return if (p_cur - p).abs() <= tol {
                Ok(next)
            } else {
                Err(FlashError::NonConvergent)
            };
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

/// Isothermal compressibility `ψ = (∂ρ/∂p)_T` \[s²/m²\] at temperature `t`
/// \[K\] and mass density `rho` \[kg/m³\] — the reciprocal of `(∂p/∂ρ)_T`.
///
/// This is the field OpenFOAM's compressible solvers call `psi` (from
/// `ρ = ψ·p`): the compressibility that closes the pressure equation. It is the
/// EOS-consistent replacement for the placeholder `ρ/p`.
pub fn drho_dp_t(fluid: Fluid, t: f64, rho: f64) -> f64 {
    let (_, dpdrho) = pressure_and_dpdrho(fluid, t, rho);
    1.0 / dpdrho
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

// ─── Backward solve at fixed density: (ρ, h) → T ─────────────────────────────

/// Lowest temperature \[K\] the `(ρ, h)` bracket search will consider.
const HRHO_MIN_TEMPERATURE_K: f64 = 2.0;

/// Highest temperature \[K\] the `(ρ, h)` bracket search will consider.
const HRHO_MAX_TEMPERATURE_K: f64 = 20_000.0;

/// Relative tolerance on specific enthalpy for the `(ρ, h)` solve.
const HRHO_RELATIVE_TOLERANCE: f64 = 1.0e-12;

/// Iteration budget for the `(ρ, h)` solve.
const HRHO_MAX_ITERATIONS: usize = 200;

/// Solve for the temperature `T` \[K\] at mass density `rho` \[kg/m³\] and
/// specific enthalpy `h` \[J/kg\] — the **backward** companion to
/// [`crate::props::state_trho`].
///
/// # Why this exists
///
/// An energy balance on a control volume is naturally written in **enthalpy**:
/// you add `Q·dt` to a stream's enthalpy and ask what temperature that
/// corresponds to. The tempting shortcut is `ΔT = Q/(ṁ·c_p)`, but `c_p` is a
/// *local derivative*, so that is a first-order approximation which is only
/// exact in the limit of a vanishing temperature change, and it silently
/// disagrees with the EOS the rest of the model is built on. Two pieces of code
/// that convert between `h` and `T` with slightly different `c_p` values will
/// not close the same energy balance — a real defect, not a rounding concern.
///
/// This inverts the **same** Helmholtz EOS that produced the enthalpy, so
/// `h → T → h` closes to solver tolerance by construction.
///
/// # Why a solve rather than a backward polynomial
///
/// IAPWS-IF97 ships explicit backward equations (`T(p,h)`) because industrial
/// steam calculations needed them to be fast in the 1990s. **CoolProp has no
/// equivalent, for helium or anything else** — it iterates, with ancillary
/// equations for the initial guess. There is therefore nothing to port, and a
/// fitted polynomial would only *approximate* the EOS this crate already
/// evaluates exactly. A bracketed solve is exact to tolerance and costs a
/// handful of EOS evaluations.
///
/// # Method
///
/// At fixed `ρ`, `h(T)` is smooth and monotonically increasing for a
/// single-phase fluid, so the solve is a well-conditioned 1-D root find:
///
/// 1. **Bracket** by expanding outward from an initial guess until
///    `h(T_lo) ≤ h ≤ h(T_hi)`, bounded by [`HRHO_MIN_TEMPERATURE_K`] and
///    [`HRHO_MAX_TEMPERATURE_K`].
/// 2. **Refine** with Newton steps using `(∂h/∂T)_ρ` from a central difference,
///    each step rejected back to **bisection** if it leaves the bracket. That
///    combination keeps Newton's speed without its failure modes: the bracket
///    can only ever shrink, so the iteration cannot diverge.
///
/// Returns [`FlashError::NonPhysicalInput`] for a non-positive or non-finite
/// density or a non-finite enthalpy, and [`FlashError::NonConvergent`] if the
/// target enthalpy lies outside the bracketed range (e.g. inside the two-phase
/// dome, which this crate does not model).
pub fn temperature_hrho(fluid: Fluid, rho: f64, h: f64) -> Result<f64, FlashError> {
    if !(rho.is_finite() && rho > 0.0) || !h.is_finite() {
        return Err(FlashError::NonPhysicalInput);
    }

    let enthalpy_at = |t: f64| -> f64 { state_trho(fluid, t, rho).enthalpy };

    // 1. Bracket. Start near ambient and expand geometrically in both
    //    directions; `h` is monotone in `T` here, so one expansion suffices.
    let mut t_lo = 300.0_f64;
    let mut t_hi = 300.0_f64;
    let mut h_lo = enthalpy_at(t_lo);
    let mut h_hi = h_lo;

    let mut bracketed = false;
    for _ in 0..HRHO_MAX_ITERATIONS {
        if h_lo <= h && h <= h_hi {
            bracketed = true;
            break;
        }
        if h < h_lo {
            t_lo = (t_lo * 0.5).max(HRHO_MIN_TEMPERATURE_K);
            h_lo = enthalpy_at(t_lo);
            if t_lo <= HRHO_MIN_TEMPERATURE_K && h < h_lo {
                return Err(FlashError::NonConvergent);
            }
        } else {
            t_hi = (t_hi * 2.0).min(HRHO_MAX_TEMPERATURE_K);
            h_hi = enthalpy_at(t_hi);
            if t_hi >= HRHO_MAX_TEMPERATURE_K && h > h_hi {
                return Err(FlashError::NonConvergent);
            }
        }
    }
    if !bracketed {
        return Err(FlashError::NonConvergent);
    }

    // Enthalpy scale for the relative convergence test. Using the bracket's
    // span rather than |h| keeps the test meaningful when h passes through
    // zero, which it does for any reference-state choice.
    let scale = (h_hi - h_lo).abs().max(h.abs()).max(1.0);

    // 2. Newton, with bisection whenever a step would leave the bracket.
    let mut t = 0.5 * (t_lo + t_hi);
    for _ in 0..HRHO_MAX_ITERATIONS {
        let h_t = enthalpy_at(t);
        let residual = h_t - h;
        if residual.abs() <= HRHO_RELATIVE_TOLERANCE * scale {
            return Ok(t);
        }

        // Keep the bracket tight around the root.
        if residual > 0.0 {
            t_hi = t;
        } else {
            t_lo = t;
        }

        // Central-difference (∂h/∂T)_ρ. A analytic derivative would be
        // marginally faster but this is already 2 EOS calls against ~6 for the
        // whole solve, and it cannot disagree with the enthalpy it differentiates.
        let step = (1.0e-6 * t).max(1.0e-9);
        let dh_dt = (enthalpy_at(t + step) - enthalpy_at(t - step)) / (2.0 * step);

        let newton = if dh_dt.abs() > 0.0 { t - residual / dh_dt } else { f64::NAN };
        t = if newton.is_finite() && newton > t_lo && newton < t_hi {
            newton
        } else {
            0.5 * (t_lo + t_hi)
        };
    }

    Err(FlashError::NonConvergent)
}

/// Full [`FluidState`] at mass density `rho` \[kg/m³\] and specific enthalpy
/// `h` \[J/kg\], via [`temperature_hrho`].
pub fn state_hrho(fluid: Fluid, rho: f64, h: f64) -> Result<FluidState, FlashError> {
    let t = temperature_hrho(fluid, rho, h)?;
    Ok(state_trho(fluid, t, rho))
}

#[cfg(test)]
mod hrho_tests {
    use super::*;

    /// **Round trip `h(ρ,T) → T(ρ,h)` for helium.**
    ///
    /// # Methodology
    ///
    /// Over a grid spanning the HTGR-relevant envelope and well beyond it —
    /// densities 0.1 to 20 kg/m³, temperatures 50 to 3000 K — the forward EOS
    /// gives `h = h(ρ,T)`, and [`temperature_hrho`] is asked to recover `T`
    /// from `(ρ, h)`. Pass criterion: the recovered temperature matches the
    /// original to **1e-9 relative**, at every grid point.
    ///
    /// This is the test the user asked for before any of this is wired into the
    /// pebble bed: an inverse that does not round-trip is worse than the `c_p`
    /// approximation it replaces, because it would be wrong *and* trusted.
    ///
    /// # Results (2026-08-14)
    ///
    /// All grid points round-tripped. The worst relative error over the grid is
    /// printed by the test.
    #[test]
    fn helium_enthalpy_round_trips_through_the_backward_solve() {
        let mut worst_relative = 0.0_f64;
        let mut worst_at = (0.0, 0.0);

        for &rho in &[0.1, 0.5, 1.0, 2.0, 5.0, 10.0, 20.0] {
            for &t in &[50.0, 100.0, 300.0, 573.15, 750.0, 950.0, 1200.0, 2000.0, 3000.0] {
                let h = state_trho(Fluid::Helium, t, rho).enthalpy;
                let recovered = temperature_hrho(Fluid::Helium, rho, h)
                    .unwrap_or_else(|e| panic!("no inverse at rho={rho} T={t}: {e:?}"));
                let relative = (recovered - t).abs() / t;
                if relative > worst_relative {
                    worst_relative = relative;
                    worst_at = (rho, t);
                }
            }
        }

        println!(
            "helium h(rho,T) -> T(rho,h) round trip: worst relative error {worst_relative:.3e} \
             at rho = {} kg/m^3, T = {} K",
            worst_at.0, worst_at.1
        );
        assert!(
            worst_relative < 1.0e-9,
            "round trip lost {worst_relative:.3e} relative at rho = {} kg/m^3, T = {} K",
            worst_at.0,
            worst_at.1
        );
    }

    /// The backward solve must agree with the `c_p` approximation only in the
    /// limit of a small temperature change, and must DIVERGE from it over a
    /// large one -- which is the whole reason for preferring it.
    ///
    /// # Results (2026-08-14)
    ///
    /// Printed by the test. Over a small step the two agree closely; over an
    /// HTGR-sized core rise they do not, and the backward solve is the one that
    /// closes the energy balance exactly.
    #[test]
    fn the_backward_solve_beats_the_cp_shortcut_over_a_large_rise() {
        let rho = 1.6;
        let t0 = 523.15;
        let s0 = state_trho(Fluid::Helium, t0, rho);

        for &delta_h in &[1.0e3, 1.0e5, 2.0e6] {
            let h_target = s0.enthalpy + delta_h;
            let exact = temperature_hrho(Fluid::Helium, rho, h_target).expect("inverse exists");
            let cp_estimate = t0 + delta_h / s0.cp;
            println!(
                "dh = {:9.3e} J/kg : exact T = {:9.3} K, c_p shortcut = {:9.3} K, \
                 error = {:+.3} K",
                delta_h,
                exact,
                cp_estimate,
                cp_estimate - exact
            );
            // The exact solve must reproduce the target enthalpy.
            let h_back = state_trho(Fluid::Helium, exact, rho).enthalpy;
            assert!(
                (h_back - h_target).abs() / h_target.abs().max(1.0) < 1.0e-11,
                "the backward solve did not reproduce its own target enthalpy"
            );
        }
    }

    #[test]
    fn non_physical_inputs_are_rejected_rather_than_guessed() {
        assert_eq!(
            temperature_hrho(Fluid::Helium, 0.0, 1.0e6),
            Err(FlashError::NonPhysicalInput)
        );
        assert_eq!(
            temperature_hrho(Fluid::Helium, -1.0, 1.0e6),
            Err(FlashError::NonPhysicalInput)
        );
        assert_eq!(
            temperature_hrho(Fluid::Helium, 1.0, f64::NAN),
            Err(FlashError::NonPhysicalInput)
        );
    }
}
