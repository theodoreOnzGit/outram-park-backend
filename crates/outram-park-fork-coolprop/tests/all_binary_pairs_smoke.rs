//! Smoke test — every wired binary pair evaluates to a physically sane
//! mixture state.
//!
//! ## Methodology
//!
//! 840 of CoolProp's 888 binary pairs are wired into
//! [`binary_pairs::all`] (see `dev/regen_mixture_all.py`; the other 48 touch
//! a component outside this crate's 137 ported pure fluids). This test
//! iterates every pair and, at an equal-mole-fraction (`x = 0.5/0.5`)
//! composition, checks:
//!
//! - `reducing_temperature`/`reducing_density` are finite and positive,
//! - `residual_derivs` at `(δ=1, τ=1)` is finite (a representative interior
//!   point, not a singularity for any of these forms), and
//! - `state_trho_molar` at a **dilute supercritical** state (`1.5·T_r`,
//!   `0.1·ρ_r`) gives a finite, positive pressure and `c_p > c_v`. That state
//!   is deliberately chosen far from any phase boundary: this crate's direct
//!   `(T, ρ)` evaluation has no phase-stability check (documented on
//!   `props::state_trho`), so a point near/inside the two-phase dome can
//!   legitimately give a negative `c_p` on the unstable branch — real EOS
//!   behaviour, not a porting bug, but the wrong target for a coverage check.
//!
//! This is a coverage/regression smoke test, not an accuracy check — accuracy
//! for the two pairs verified against real references (N₂–O₂ reproducing
//! air's speed of sound; R32–R125 exercising a real departure function) is
//! `tests/mixture_nitrogen_oxygen.rs` / `tests/mixture_departure_function.rs`.
//! The point here is to catch a pair whose generated data fails to evaluate
//! at all (NaN/inf/≤0), which would flag a departure-function type or
//! reducing-parameter mishandled for that specific pair.
//!
//! ## Results (2026-07-10)
//!
//! All 840 wired pairs evaluate to a finite, positive reducing state and a
//! finite, positive pressure/`c_p` at the dilute supercritical state, with
//! `c_p > c_v` throughout.

use outram_park_fork_coolprop::mixtures::{binary_pairs, Mixture};

#[test]
fn every_binary_pair_evaluates_at_a_representative_state() {
    let mut checked = 0;
    for pair in binary_pairs::all() {
        let mix = Mixture::new(vec![pair.a, pair.b], vec![0.5, 0.5]).unwrap();
        let label = format!("{}-{}", pair.a.name(), pair.b.name());

        let t_r = mix.reducing_temperature();
        let rho_r = mix.reducing_density();
        assert!(
            t_r.is_finite() && t_r > 0.0,
            "{label}: non-physical T_r {t_r}"
        );
        assert!(
            rho_r.is_finite() && rho_r > 0.0,
            "{label}: non-physical rho_r {rho_r}"
        );

        let res = mix.residual_derivs(1.0, 1.0);
        assert!(
            res.a.is_finite(),
            "{label}: non-finite alpha_r at (delta=1,tau=1): {}",
            res.a
        );

        // Dilute supercritical state -- see the module doc for why.
        let state = mix
            .state_trho_molar(1.5 * t_r, 0.1 * rho_r)
            .unwrap_or_else(|e| panic!("{label}: state_trho_molar failed: {e:?}"));
        assert!(
            state.pressure.is_finite() && state.pressure > 0.0,
            "{label}: non-physical p {}",
            state.pressure
        );
        assert!(
            state.cp.is_finite() && state.cp > 0.0,
            "{label}: non-physical cp {}",
            state.cp
        );
        assert!(
            state.cp > state.cv,
            "{label}: cp {} should exceed cv {}",
            state.cp,
            state.cv
        );

        checked += 1;
    }
    assert!(
        checked >= 840,
        "expected at least 840 binary pairs wired, only {checked} checked"
    );
    println!("{checked} binary pairs checked");
}
