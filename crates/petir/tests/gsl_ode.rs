// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 Theodore Ong and the outram-park contributors.
//
// Test systems from the GNU Scientific Library (GSL) 2.8, ode-initval2/test.c,
// commit cf180cd7fbd06039a577f9c9ff0b428784765ac1.
// Copyright (C) 1996-2000 Gerard Jungman, Brian Gough. GPL-3.0-or-later.

//! Verifies PETIR's ODE integrators against systems with closed-form solutions.

use petir::ode::{rk4_step, rkf45_step, solve_rkf45};
use petir::PetirError;

/// `dy/dt = y`, `y(0) = 1`, so `y(t) = e^t`. GSL's `rhs_exp`.
fn rhs_exp(_t: f64, y: &[f64], dy: &mut [f64]) {
    dy[0] = y[0];
}

/// The harmonic oscillator `y'' = -y` as a first-order pair. GSL's `rhs_sin`.
/// `y(0) = (0, 1)` gives `y = (sin t, cos t)`.
fn rhs_sin(_t: f64, y: &[f64], dy: &mut [f64]) {
    dy[0] = y[1];
    dy[1] = -y[0];
}

/// A SINGLE step measures LOCAL truncation error, which is one order HIGHER
/// than the method's name.
///
/// A "fourth-order" method means its GLOBAL error over a fixed interval falls
/// as `h^4`. Taking `1/h` steps to cross that interval, each contributing
/// `O(h^5)`, is what produces `O(h^4)` overall. So halving `h` cuts a single
/// step's error by 32, not 16 — and by 64, not 32, for RKF45.
///
/// Measured here: 32.3 for RK4 and 64.7 for RKF45, both within a few percent
/// of the theoretical `2^5` and `2^6`. An earlier version of these tests
/// expected 16 and 32 and flagged the correct behaviour as a failure; the
/// distinction between local and global order is the whole content of the fix,
/// so it is written down rather than just patched.
#[test]
fn a_single_rkf45_step_shows_sixth_order_local_error() {
    let err_at = |h: f64| {
        let s = rkf45_step(rhs_exp, 0.0, &[1.0], h).unwrap();
        (s.y[0] - h.exp()).abs()
    };
    let ratio = err_at(0.1) / err_at(0.05);
    assert!(
        (45.0..=85.0).contains(&ratio),
        "RKF45 local error should scale as h^6 (ratio ~64), got {ratio}"
    );
}

#[test]
fn a_single_rk4_step_shows_fifth_order_local_error() {
    let err_at = |h: f64| {
        let y = rk4_step(rhs_exp, 0.0, &[1.0], h).unwrap();
        (y[0] - h.exp()).abs()
    };
    let ratio = err_at(0.1) / err_at(0.05);
    assert!(
        (24.0..=42.0).contains(&ratio),
        "RK4 local error should scale as h^5 (ratio ~32), got {ratio}"
    );
}

/// And the GLOBAL order is what the method is named for: integrating a fixed
/// interval with a fixed step, RK4's error falls as `h^4`.
#[test]
fn rk4_shows_fourth_order_global_error_over_a_fixed_interval() {
    let global_err = |steps: usize| {
        let h = 1.0 / steps as f64;
        let mut y = vec![1.0_f64];
        let mut t = 0.0_f64;
        for _ in 0..steps {
            y = rk4_step(rhs_exp, t, &y, h).unwrap();
            t += h;
        }
        (y[0] - core::f64::consts::E).abs()
    };
    let ratio = global_err(20) / global_err(40);
    assert!(
        (12.0..=20.0).contains(&ratio),
        "RK4 global error should scale as h^4 (ratio ~16), got {ratio}"
    );
}


/// The embedded error estimate has to actually bound the error, or step
/// control is built on sand.
#[test]
fn the_embedded_error_estimate_bounds_the_true_error() {
    for &h in &[0.5_f64, 0.2, 0.1, 0.01] {
        let s = rkf45_step(rhs_exp, 0.0, &[1.0], h).unwrap();
        let true_err = (s.y[0] - h.exp()).abs();
        assert!(
            true_err <= s.yerr[0].abs() * 10.0 + 1e-15,
            "h = {h}: true error {true_err:e} against estimate {:e}",
            s.yerr[0]
        );
    }
}

#[test]
fn adaptive_integration_reproduces_the_exponential() {
    let sol = solve_rkf45(rhs_exp, 0.0, 1.0, &[1.0], 1e-3, 1e-12, 1e-12, 100_000).unwrap();
    let exact = core::f64::consts::E;
    assert!(
        (sol.final_state()[0] - exact).abs() < 1e-9,
        "got {}, want {exact}",
        sol.final_state()[0]
    );
    assert!((sol.final_t() - 1.0).abs() < 1e-14, "must land exactly on t1");
    assert!(sol.accepted > 0);
}

#[test]
fn adaptive_integration_reproduces_the_harmonic_oscillator() {
    let t_end = 10.0_f64;
    let sol = solve_rkf45(rhs_sin, 0.0, t_end, &[0.0, 1.0], 1e-3, 1e-12, 1e-12, 100_000).unwrap();
    let y = sol.final_state();
    assert!(
        (y[0] - t_end.sin()).abs() < 1e-8,
        "sin: got {}, want {}",
        y[0],
        t_end.sin()
    );
    assert!(
        (y[1] - t_end.cos()).abs() < 1e-8,
        "cos: got {}, want {}",
        y[1],
        t_end.cos()
    );
}

/// The oscillator conserves `y0^2 + y1^2`. A conserved quantity is the
/// sharpest check available on a long integration, because it is independent
/// of the closed form.
#[test]
fn the_oscillator_conserves_its_invariant() {
    let sol = solve_rkf45(rhs_sin, 0.0, 50.0, &[0.0, 1.0], 1e-3, 1e-12, 1e-12, 100_000).unwrap();
    for p in &sol.points {
        let energy = p.y[0] * p.y[0] + p.y[1] * p.y[1];
        assert!(
            (energy - 1.0).abs() < 1e-7,
            "at t = {}: invariant drifted to {energy}",
            p.t
        );
    }
}

/// A tighter tolerance must actually buy accuracy -- otherwise the controller
/// is not doing anything.
#[test]
fn a_tighter_tolerance_gives_a_more_accurate_answer() {
    let err_at = |tol: f64| {
        let sol = solve_rkf45(rhs_exp, 0.0, 1.0, &[1.0], 1e-2, tol, tol, 100_000).unwrap();
        (sol.final_state()[0] - core::f64::consts::E).abs()
    };
    let loose = err_at(1e-4);
    let tight = err_at(1e-12);
    assert!(
        tight < loose,
        "tightening the tolerance should improve accuracy: {loose:e} -> {tight:e}"
    );
}

/// Integrating backwards must work and must undo the forward pass.
#[test]
fn integrating_backwards_returns_to_the_initial_condition() {
    let fwd = solve_rkf45(rhs_sin, 0.0, 3.0, &[0.0, 1.0], 1e-3, 1e-12, 1e-12, 100_000).unwrap();
    let end = fwd.final_state().to_vec();

    let back = solve_rkf45(rhs_sin, 3.0, 0.0, &end, 1e-3, 1e-12, 1e-12, 100_000).unwrap();
    let y = back.final_state();

    assert!((back.final_t() - 0.0).abs() < 1e-12, "should land on t = 0");
    assert!(y[0].abs() < 1e-8, "y0 returned to {}, want 0", y[0]);
    assert!((y[1] - 1.0).abs() < 1e-8, "y1 returned to {}, want 1", y[1]);
}

/// The controller must REJECT steps when the tolerance demands it -- if it
/// never rejects, it is not controlling anything.
#[test]
fn the_controller_rejects_steps_when_the_initial_guess_is_too_large() {
    // A wildly optimistic first step on a rapidly varying system.
    let sol = solve_rkf45(rhs_sin, 0.0, 20.0, &[0.0, 1.0], 10.0, 1e-12, 1e-12, 100_000).unwrap();
    assert!(
        sol.rejected > 0,
        "an absurd initial step should have been rejected at least once"
    );
    // ...and it still gets the right answer.
    assert!((sol.final_state()[0] - 20.0_f64.sin()).abs() < 1e-7);
}

#[test]
fn malformed_input_is_reported() {
    assert!(matches!(
        solve_rkf45(rhs_exp, 0.0, 1.0, &[], 1e-3, 1e-9, 1e-9, 100),
        Err(PetirError::Invalid)
    ));
    // Non-positive initial step.
    assert!(matches!(
        solve_rkf45(rhs_exp, 0.0, 1.0, &[1.0], 0.0, 1e-9, 1e-9, 100),
        Err(PetirError::Invalid)
    ));
    // Zero-length interval.
    assert!(matches!(
        solve_rkf45(rhs_exp, 1.0, 1.0, &[1.0], 1e-3, 1e-9, 1e-9, 100),
        Err(PetirError::Invalid)
    ));
    // Empty system for the single-step routines.
    assert!(matches!(rk4_step(rhs_exp, 0.0, &[], 0.1), Err(PetirError::Invalid)));
    assert!(matches!(rkf45_step(rhs_exp, 0.0, &[], 0.1), Err(PetirError::Invalid)));
}

/// A blow-up is reported, not returned as a plausible number.
#[test]
fn a_diverging_system_is_reported() {
    // dy/dt = y^2, y(0) = 1 blows up at t = 1.
    let rhs = |_t: f64, y: &[f64], dy: &mut [f64]| dy[0] = y[0] * y[0];
    let outcome = solve_rkf45(rhs, 0.0, 2.0, &[1.0], 1e-3, 1e-10, 1e-10, 10_000);
    assert!(
        outcome.is_err(),
        "integrating through a finite-time blow-up must fail, got {outcome:?}"
    );
}

/// The documented stiffness limitation, demonstrated rather than merely
/// asserted: an explicit method forced to resolve a fast decayed mode.
#[test]
fn a_stiff_system_exhausts_the_step_budget_as_documented() {
    // The classic stiff pair: one mode with rate -1, one with -1000.
    let rhs = |_t: f64, y: &[f64], dy: &mut [f64]| {
        dy[0] = -y[0];
        dy[1] = -1000.0 * y[1];
    };
    // Far too few steps for an explicit method to cross this span, because the
    // stable step is set by the -1000 mode even after it has decayed away.
    let outcome = solve_rkf45(rhs, 0.0, 100.0, &[1.0, 1.0], 1e-3, 1e-10, 1e-10, 200);
    assert!(
        matches!(outcome, Err(PetirError::MaxIterations)),
        "a stiff system should exhaust the budget, not silently succeed: {outcome:?}"
    );
}

/// Every accepted point must lie on the trajectory in order, and the first
/// must be the initial condition.
#[test]
fn the_trajectory_is_ordered_and_starts_at_the_initial_condition() {
    let sol = solve_rkf45(rhs_exp, 0.0, 2.0, &[1.0], 1e-3, 1e-9, 1e-9, 100_000).unwrap();
    assert_eq!(sol.points[0].t, 0.0);
    assert_eq!(sol.points[0].y, vec![1.0]);
    for w in sol.points.windows(2) {
        assert!(w[1].t > w[0].t, "trajectory went backwards: {} -> {}", w[0].t, w[1].t);
    }
    assert_eq!(sol.points.len(), sol.accepted + 1);
}
