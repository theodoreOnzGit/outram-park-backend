// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 Theodore Ong and the outram-park contributors.
//
// Test functions taken from the GNU Scientific Library (GSL) 2.8,
// roots/test_funcs.c, commit cf180cd7fbd06039a577f9c9ff0b428784765ac1.
// Copyright (C) 1996-2000 Reid Priedhorsky, Brian Gough. GPL-3.0-or-later.

//! Verifies PETIR's root finders against **GSL's own test functions**.
//!
//! # Why upstream's test set rather than one of my own
//!
//! Because a port should inherit its upstream's V&V, and because GSL's set was
//! chosen adversarially by people who knew where these algorithms break. The
//! functions below are not arbitrary: `x^20 - 1` is brutally flat then brutally
//! steep, `sqrt(|x|) * sign(x)` has an infinite derivative at its root,
//! `x exp(-x)` has a root where the function is very nearly flat, and
//! `(x - 1)^7` is a root of multiplicity seven where both `f` and `f'` vanish.
//! A test suite I invented would have been kinder, and would have proved less.

use petir::roots::{
    test_delta, test_interval, BracketingMethod, BracketingSolver, Convergence, PolishingMethod,
    PolishingSolver,
};

/// GSL `func1`: `x^20 - 1`, root at 1. Flat near zero, near-vertical at the root.
fn func1(x: f64) -> f64 {
    x.powf(20.0) - 1.0
}
fn func1_df(x: f64) -> f64 {
    20.0 * x.powf(19.0)
}

/// GSL `func2`: `sqrt(|x|) * sign(x)`, root at 0. Derivative is infinite there.
fn func2(x: f64) -> f64 {
    let delta = if x > 0.0 {
        1.0
    } else if x < 0.0 {
        -1.0
    } else {
        0.0
    };
    x.abs().sqrt() * delta
}

/// GSL `func3`: `x^2 - 1e-8`, root at 1e-4. Tests relative tolerance handling.
fn func3(x: f64) -> f64 {
    x.powf(2.0) - 1e-8
}

/// GSL `func4`: `x exp(-x)`, root at 0, very flat approaching it from above.
fn func4(x: f64) -> f64 {
    x * (-x).exp()
}
fn func4_df(x: f64) -> f64 {
    (-x).exp() * (1.0 - x)
}

/// GSL `func6`: `(x - 1)^7`, a root of multiplicity SEVEN at 1.
fn func6(x: f64) -> f64 {
    (x - 1.0).powf(7.0)
}

const METHODS: [BracketingMethod; 3] = [
    BracketingMethod::Bisection,
    BracketingMethod::FalsePosition,
    BracketingMethod::Brent,
];

/// Drive a bracketing solver to convergence on the interval width.
fn solve_bracketed<F: Fn(f64) -> f64>(
    method: BracketingMethod,
    f: F,
    lo: f64,
    hi: f64,
    epsabs: f64,
    epsrel: f64,
    max_iter: usize,
) -> Option<f64> {
    let mut s = BracketingSolver::new(method, f, lo, hi).ok()?;
    for _ in 0..max_iter {
        s.iterate().ok()?;
        if test_interval(s.x_lower(), s.x_upper(), epsabs, epsrel).ok()? == Convergence::Converged {
            return Some(s.root());
        }
    }
    None
}

#[test]
fn all_bracketing_methods_find_the_root_of_x20_minus_1() {
    for m in METHODS {
        let r = solve_bracketed(m, func1, 0.1, 2.0, 0.0, 1e-10, 1000)
            .unwrap_or_else(|| panic!("{m:?} failed to converge on x^20 - 1"));
        assert!((r - 1.0).abs() < 1e-8, "{m:?}: root {r}, want 1.0");
    }
}

/// The infinite-derivative case. Bracketing methods should still cope, because
/// they never touch the derivative.
#[test]
fn all_bracketing_methods_handle_an_infinite_derivative_at_the_root() {
    for m in METHODS {
        let r = solve_bracketed(m, func2, -1.0 / 3.0, 1.0, 1e-12, 0.0, 1000)
            .unwrap_or_else(|| panic!("{m:?} failed on sqrt(|x|) sign(x)"));
        assert!(r.abs() < 1e-10, "{m:?}: root {r}, want 0.0");
    }
}

/// A small root, where an absolute-only tolerance would be meaningless and a
/// relative one is the point.
#[test]
fn all_bracketing_methods_find_a_small_root_with_relative_tolerance() {
    for m in METHODS {
        let r = solve_bracketed(m, func3, 0.0, 1.0, 0.0, 1e-10, 1000)
            .unwrap_or_else(|| panic!("{m:?} failed on x^2 - 1e-8"));
        assert!(
            (r - 1e-4).abs() / 1e-4 < 1e-6,
            "{m:?}: root {r}, want 1e-4"
        );
    }
}

#[test]
fn all_bracketing_methods_find_the_flat_root_of_x_exp_minus_x() {
    for m in METHODS {
        let r = solve_bracketed(m, func4, -1.0 / 3.0, 2.0, 1e-12, 0.0, 1000)
            .unwrap_or_else(|| panic!("{m:?} failed on x exp(-x)"));
        assert!(r.abs() < 1e-10, "{m:?}: root {r}, want 0.0");
    }
}

/// Multiplicity seven. `f` does change sign here (odd multiplicity), so
/// bracketing still works -- slowly.
#[test]
fn bracketing_survives_a_root_of_multiplicity_seven() {
    for m in METHODS {
        let r = solve_bracketed(m, func6, 0.0, 3.0, 1e-10, 0.0, 2000)
            .unwrap_or_else(|| panic!("{m:?} failed on (x-1)^7"));
        assert!((r - 1.0).abs() < 1e-4, "{m:?}: root {r}, want 1.0");
    }
}

/// Brent should reach a given accuracy in far fewer steps than bisection.
/// This is the reason it is the recommended default, so it is checked rather
/// than asserted in prose.
#[test]
fn brent_converges_in_fewer_iterations_than_bisection() {
    fn count_iters(method: BracketingMethod) -> usize {
        let mut s = BracketingSolver::new(method, func1, 0.1, 2.0).unwrap();
        for i in 1..=1000 {
            s.iterate().unwrap();
            if test_interval(s.x_lower(), s.x_upper(), 0.0, 1e-12).unwrap()
                == Convergence::Converged
            {
                return i;
            }
        }
        usize::MAX
    }
    let bisect = count_iters(BracketingMethod::Bisection);
    let brent = count_iters(BracketingMethod::Brent);
    assert!(
        brent < bisect,
        "Brent took {brent} iterations, bisection {bisect}"
    );
}

/// Bisection's defining property: the interval halves exactly each step, so
/// the iteration count to a given width is predictable.
#[test]
fn bisection_halves_the_interval_every_step() {
    let mut s = BracketingSolver::new(BracketingMethod::Bisection, func1, 0.1, 2.0).unwrap();
    let mut width = s.x_upper() - s.x_lower();
    for _ in 0..20 {
        s.iterate().unwrap();
        let new_width = s.x_upper() - s.x_lower();
        // "Exact" halving is exact in arithmetic, not in floating point: the
        // midpoint (a + b) / 2 rounds, and that rounding is an ulp of the
        // ENDPOINTS (order 1 here), not of the width (order 1e-5 after 20
        // steps). Judging the halving against the width's own magnitude is the
        // wrong scale and fails at the 1e-10 level for no real reason.
        let ratio = new_width / width;
        assert!(
            (ratio - 0.5).abs() < 1e-9,
            "width went {width} -> {new_width} (ratio {ratio}), expected a halving"
        );
        width = new_width;
    }
}

#[test]
fn a_bracket_that_does_not_straddle_zero_is_rejected() {
    // Both endpoints positive.
    assert!(BracketingSolver::new(BracketingMethod::Brent, func1, 1.5, 2.0).is_err());
    // Reversed interval.
    assert!(BracketingSolver::new(BracketingMethod::Brent, func1, 2.0, 0.1).is_err());
}

/// A root sitting exactly on an endpoint must be accepted and returned.
#[test]
fn a_root_exactly_at_an_endpoint_is_found() {
    for m in METHODS {
        let mut s = BracketingSolver::new(m, func1, 1.0, 2.0).unwrap();
        s.iterate().unwrap();
        assert!((s.root() - 1.0).abs() < 1e-12, "{m:?}: root {}", s.root());
    }
}

const POLISHERS: [PolishingMethod; 3] = [
    PolishingMethod::Newton,
    PolishingMethod::Secant,
    PolishingMethod::Steffenson,
];

#[test]
fn all_polishing_methods_find_the_root_of_x20_minus_1() {
    for m in POLISHERS {
        let mut s = PolishingSolver::new(m, func1, func1_df, 1.5).unwrap();
        let mut root = f64::NAN;
        for _ in 0..500 {
            let prev = s.root();
            if s.iterate().is_err() {
                break;
            }
            if test_delta(s.root(), prev, 0.0, 1e-12).unwrap() == Convergence::Converged {
                root = s.root();
                break;
            }
        }
        assert!((root - 1.0).abs() < 1e-8, "{m:?}: root {root}, want 1.0");
    }
}

#[test]
fn all_polishing_methods_find_the_root_of_x_exp_minus_x() {
    for m in POLISHERS {
        let mut s = PolishingSolver::new(m, func4, func4_df, 0.5).unwrap();
        let r = s.solve(0.0, 1e-12, 500);
        let root = r.unwrap_or_else(|e| panic!("{m:?} failed: {e}"));
        assert!(root.abs() < 1e-8, "{m:?}: root {root}, want 0.0");
    }
}

/// Newton's quadratic convergence, demonstrated rather than claimed: from a
/// good guess it should need only a handful of steps.
#[test]
fn newton_converges_quadratically_from_a_good_guess() {
    let f = |x: f64| x * x - 2.0;
    let df = |x: f64| 2.0 * x;
    let mut s = PolishingSolver::new(PolishingMethod::Newton, f, df, 1.5).unwrap();
    let mut errors = Vec::new();
    for _ in 0..5 {
        s.iterate().unwrap();
        errors.push((s.root() - 2.0_f64.sqrt()).abs());
    }
    // Each error should be roughly the square of the previous one, so after
    // five steps from 1.5 we are at machine precision.
    assert!(
        errors[4] < 1e-15,
        "after 5 Newton steps the error is {:e}",
        errors[4]
    );
    // The error must fall monotonically WHILE there is still error to remove.
    // Once Newton lands on the exactly-rounded root the error is 0, and the
    // next step can move it by one ulp -- which is convergence, not
    // divergence, so the monotonicity claim only applies above that floor.
    for w in errors.windows(2) {
        if w[0] > 1e-15 {
            assert!(w[1] <= w[0], "error rose: {:e} -> {:e}", w[0], w[1]);
        }
    }
}

/// A zero derivative is reported, not divided by.
#[test]
fn a_zero_derivative_is_reported() {
    // f(x) = x^2 has f'(0) = 0; starting exactly at 0 is the degenerate case.
    let f = |x: f64| x * x;
    let df = |x: f64| 2.0 * x;
    let mut s = PolishingSolver::new(PolishingMethod::Newton, f, df, 0.0).unwrap();
    assert_eq!(s.iterate().unwrap_err(), petir::PetirError::ZeroDivide);
}

/// The documented failure mode: without a bracket, a bad guess diverges and
/// nothing warns you. Pinned so the docs are demonstrably true.
#[test]
fn newton_diverges_on_a_hostile_function_without_complaint() {
    // f(x) = 1/(1 + exp(x)) (GSL's func5) has NO root; Newton runs away.
    let f = |x: f64| 1.0 / (1.0 + x.exp());
    let df = |x: f64| {
        let e = x.exp();
        -e / ((1.0 + e) * (1.0 + e))
    };
    // The Newton step here is x - f/df = x + 1 + exp(-x), so the iterate walks
    // off toward +infinity at roughly ONE PER STEP -- steadily, not explosively.
    // An earlier version of this test looked for a magnitude above 1e12 within
    // 100 steps and failed at x = 101: the divergence was real, the threshold
    // was wrong. What actually characterises the failure is that the iteration
    // never converges and never stops moving.
    let mut s = PolishingSolver::new(PolishingMethod::Newton, f, df, 0.0).unwrap();
    let start = s.root();

    // It must not report success.
    let outcome = s.solve(1e-12, 0.0, 200);
    assert_eq!(
        outcome.unwrap_err(),
        petir::PetirError::MaxIterations,
        "a function with no root must not yield a converged answer"
    );

    // And it must have walked a long way from the start, still moving.
    let travelled = (s.root() - start).abs();
    assert!(
        travelled > 50.0,
        "expected the iterate to run away; it moved only {travelled} from {start}"
    );
    let before = s.root();
    s.iterate().unwrap();
    assert!(
        (s.root() - before).abs() > 0.5,
        "the iterate should still be moving by about 1 per step"
    );
}

/// Secant must call the true derivative exactly once, at construction.
/// That is its entire selling point, so it is counted.
#[test]
fn secant_evaluates_the_true_derivative_only_once() {
    use core::cell::Cell;
    let df_calls = Cell::new(0usize);
    let f = |x: f64| x * x - 2.0;
    let df = |x: f64| {
        df_calls.set(df_calls.get() + 1);
        2.0 * x
    };
    let mut s = PolishingSolver::new(PolishingMethod::Secant, f, df, 1.5).unwrap();
    for _ in 0..20 {
        if s.iterate().is_err() {
            break;
        }
    }
    assert_eq!(
        df_calls.get(),
        1,
        "secant called the true derivative {} times; it should call it once",
        df_calls.get()
    );
    assert!((s.root() - 2.0_f64.sqrt()).abs() < 1e-12);
}

/// The two families must agree on the same root, which is a cross-check
/// neither family can perform on itself.
#[test]
fn bracketing_and_polishing_agree_on_the_same_root() {
    let f = |x: f64| x.cos() - x;
    let df = |x: f64| -x.sin() - 1.0;

    let bracketed =
        solve_bracketed(BracketingMethod::Brent, f, 0.0, 1.0, 0.0, 1e-14, 200).unwrap();
    let mut p = PolishingSolver::new(PolishingMethod::Newton, f, df, 0.5).unwrap();
    let polished = p.solve(0.0, 1e-14, 200).unwrap();

    assert!(
        (bracketed - polished).abs() < 1e-12,
        "Brent {bracketed} vs Newton {polished}"
    );
    // The Dottie number, to the digits f64 carries.
    assert!((bracketed - 0.739_085_133_215_160_6).abs() < 1e-12);
}
