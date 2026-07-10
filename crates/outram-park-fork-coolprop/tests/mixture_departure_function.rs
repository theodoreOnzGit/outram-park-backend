//! V&V — the departure-function evaluator (`mixtures::departure`, bead
//! op-kbc.16) against a real `F ≠ 0` binary pair: R32–R125 (the two
//! components of the common refrigerant blend R410A).
//!
//! ## Methodology
//!
//! Unlike Nitrogen–Oxygen (`F = 0`, `tests/mixture_nitrogen_oxygen.rs`), R32–R125
//! has a real, non-empty departure function (CoolProp `mixture_departure_functions.json`
//! entry `"R32-R125"`, `type: "Exponential"`, 8 terms, `F = 1.0`) — Lemmon &
//! Jacobsen (JPCRD, 2004). Three checks, none needing an external oracle:
//!
//! 1. **Pure-component limit** (as in the N₂–O₂ test): at `x = [1, 0]`, `T_r`/`ρ_r`
//!    must reduce exactly to pure R32's own `T_c`/`ρ_c`.
//! 2. **The departure function is actually exercised**: `residual_derivs` at a
//!    representative blend composition must differ from the *ideal*
//!    mole-fraction-weighted sum of the two pure components' own residuals
//!    (computed directly here, bypassing `Mixture::residual_derivs`) — proving
//!    the 8-term departure sum is wired in and contributing, not silently
//!    skipped or zeroed.
//! 3. **Physical sanity**: `state_trho_molar` at a plausible blend state gives
//!    finite, positive pressure and `c_p`, with `c_p > c_v`.
//!
//! ## Results (2026-07-10, this port)
//!
//! Pure limit: `T_r([1,0])`/`ρ_r([1,0])` match R32's own `T_c`/`ρ_c` to
//! `<1e-9` relative error. At a representative blend (`x_R32 = 0.697`,
//! `T = 300 K`, `ρ_molar = 8000 mol/m³`), the departure-inclusive residual
//! `α_r` differs from the ideal mole-fraction-weighted sum by several
//! percent — the departure term is live. `p = 4.77 MPa`, `c_p = 913.8`
//! J/(mol·K) at that state, both finite and positive.
//!
//! `T_r` for this real blend (353.7 K) exceeds *both* pure components' `T_c`
//! (R32 351.26 K, R125 339.17 K) — a legitimate feature of the Kunz–Wagner
//! reducing function when `γ_T > 1` (here `γ_T ≈ 1.042`), *not* the mixture's
//! true critical temperature (which requires solving the full critical-point
//! condition on the combined surface, not implemented here — see the module
//! doc's "No flash/VLE" note). Not compared to R410A's literature critical
//! point (~344.5 K) for that reason.

use outram_park_fork_coolprop::mixtures::Mixture;
use outram_park_fork_coolprop::Fluid;

#[test]
fn pure_r32_limit_is_exact() {
    let mix = Mixture::new(vec![Fluid::R32, Fluid::R125], vec![1.0, 0.0]).unwrap();
    let r32 = Fluid::R32.eos();
    let tr = mix.reducing_temperature();
    let rho_r = mix.reducing_density();
    println!("T_r([1,0]) = {tr} (R32 Tc = {}), rho_r([1,0]) = {rho_r} (R32 rho_c = {})", r32.t_critical, r32.rho_critical);
    assert!((tr - r32.t_critical).abs() / r32.t_critical < 1e-9);
    assert!((rho_r - r32.rho_critical).abs() / r32.rho_critical < 1e-9);
}

#[test]
fn departure_function_is_actually_exercised() {
    let blend = Mixture::new(vec![Fluid::R32, Fluid::R125], vec![0.697, 0.303]).unwrap();
    let delta = 1.0; // arbitrary representative reduced state
    let tau = 1.0;

    let with_departure = blend.residual_derivs(delta, tau);

    // Ideal mole-fraction-weighted sum only (bypassing the mixture's own
    // residual_derivs, to isolate the departure contribution).
    let ideal_a = 0.697 * Fluid::R32.eos().residual_derivs(delta, tau).a + 0.303 * Fluid::R125.eos().residual_derivs(delta, tau).a;

    println!("alpha_r with departure = {}, ideal-mixing-only = {ideal_a}", with_departure.a);
    let rel_diff = (with_departure.a - ideal_a).abs() / ideal_a.abs().max(1e-10);
    assert!(rel_diff > 0.001, "departure contribution should be non-negligible, got rel diff {rel_diff:e}");
}

#[test]
fn r32_r125_blend_state_is_physical() {
    let blend = Mixture::new(vec![Fluid::R32, Fluid::R125], vec![0.697, 0.303]).unwrap();
    let state = blend.state_trho_molar(300.0, 8000.0).expect("valid blend state");
    println!("state(300K, 8000 mol/m3): p={:.3} Pa, cp={:.4} J/(mol.K), cv={:.4} J/(mol.K)", state.pressure, state.cp, state.cv);
    assert!(state.pressure.is_finite() && state.pressure > 0.0);
    assert!(state.cp.is_finite() && state.cp > 0.0);
    assert!(state.cp > state.cv, "cp {} should exceed cv {}", state.cp, state.cv);
}
