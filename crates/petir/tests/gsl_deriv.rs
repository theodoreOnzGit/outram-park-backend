// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 Theodore Ong and the outram-park contributors.
//
// Test functions from the GNU Scientific Library (GSL) 2.8, deriv/test.c,
// commit cf180cd7fbd06039a577f9c9ff0b428784765ac1.
// Copyright (C) 2004, 2007 Brian Gough. GPL-3.0-or-later.

//! Verifies PETIR's numerical derivatives against GSL's own test set.
//!
//! Each case pairs a function with its ANALYTIC derivative, so the reference is
//! exact and the test measures the quadrature error rather than comparing two
//! approximations to each other.

use petir::deriv::{backward, central, forward};

/// GSL's deriv test cases: (name, f, f', x).
type Case = (&'static str, fn(f64) -> f64, fn(f64) -> f64, f64);

const CASES: &[Case] = &[
    ("x^1.5", |x| x.powf(1.5), |x| 1.5 * x.sqrt(), 1.0),
    ("sqrt(x)", |x| x.sqrt(), |x| 0.5 / x.sqrt(), 1.0),
    ("1/x", |x| 1.0 / x, |x| -1.0 / (x * x), 1.0),
    ("exp(x)", |x| x.exp(), |x| x.exp(), 1.0),
    ("sin(x)", |x| x.sin(), |x| x.cos(), 1.0),
    ("cos(x)", |x| x.cos(), |x| -x.sin(), 1.0),
    ("ln(x)", |x| x.ln(), |x| 1.0 / x, 2.0),
    ("x^2 away from origin", |x| x * x, |x| 2.0 * x, 7.0),
];

/// The central rule's realistic accuracy floor: about eps^(2/3).
const CENTRAL_TOL: f64 = 1e-9;
/// The one-sided rules are an order worse: about eps^(1/2).
const ONE_SIDED_TOL: f64 = 1e-6;

#[test]
fn central_differences_match_the_analytic_derivative() {
    for &(name, f, df, x) in CASES {
        let d = central(f, x, 1e-4);
        let exact = df(x);
        let err = (d.value - exact).abs();
        assert!(
            err < CENTRAL_TOL * exact.abs().max(1.0),
            "{name} at {x}: got {}, want {exact}, error {err:e}",
            d.value
        );
    }
}

#[test]
fn forward_differences_match_the_analytic_derivative() {
    for &(name, f, df, x) in CASES {
        let d = forward(f, x, 1e-4);
        let exact = df(x);
        let err = (d.value - exact).abs();
        assert!(
            err < ONE_SIDED_TOL * exact.abs().max(1.0),
            "{name} at {x}: got {}, want {exact}, error {err:e}",
            d.value
        );
    }
}

#[test]
fn backward_differences_match_the_analytic_derivative() {
    for &(name, f, df, x) in CASES {
        let d = backward(f, x, 1e-4);
        let exact = df(x);
        let err = (d.value - exact).abs();
        assert!(
            err < ONE_SIDED_TOL * exact.abs().max(1.0),
            "{name} at {x}: got {}, want {exact}, error {err:e}",
            d.value
        );
    }
}

/// The error estimate is the load-bearing half of the result, so it must
/// actually bound the error rather than being decorative.
#[test]
fn the_reported_error_bounds_the_true_error() {
    for &(name, f, df, x) in CASES {
        for (label, d) in [
            ("central", central(f, x, 1e-4)),
            ("forward", forward(f, x, 1e-4)),
            ("backward", backward(f, x, 1e-4)),
        ] {
            let true_err = (d.value - df(x)).abs();
            assert!(
                true_err <= d.abserr * 10.0,
                "{label} {name}: true error {true_err:e} far exceeds the estimate {:e}",
                d.abserr
            );
        }
    }
}

/// Central should beat the one-sided rules -- an order of accuracy is the
/// whole reason to prefer it.
#[test]
fn central_is_more_accurate_than_one_sided() {
    let f = |x: f64| x.exp();
    let df = |x: f64| x.exp();
    let x = 1.0;

    let c = (central(f, x, 1e-4).value - df(x)).abs();
    let fwd = (forward(f, x, 1e-4).value - df(x)).abs();
    assert!(
        c < fwd,
        "central error {c:e} should beat forward error {fwd:e}"
    );
}

/// The central rule never evaluates `f` AT `x`, which is occasionally the only
/// point where `f` is undefined. Verified by handing it a function that panics
/// there -- if the rule touched `x`, this test would not survive.
#[test]
fn the_central_rule_never_evaluates_the_centre_point() {
    let f = |x: f64| {
        assert!(x != 1.0, "the central rule must not evaluate f at x itself");
        x * x
    };
    let d = central(f, 1.0, 1e-4);
    assert!((d.value - 2.0).abs() < 1e-9);
}

/// The step refinement is ONE-DIRECTIONAL, and this pins that asymmetry.
///
/// Upstream guards the retry with `round < trunc`, so:
///   - too LARGE an `h` (truncation dominates) IS refined downward;
///   - too SMALL an `h` (rounding dominates) is NOT rescued.
///
/// An earlier version of this test assumed both directions were handled and
/// failed at `h = 1e-13`. The behaviour was correct and the assumption was
/// wrong -- which is also why the module docs now say which direction it works
/// in. What protects a caller from a too-small `h` is the error estimate, so
/// that is asserted here too.
#[test]
fn the_step_is_refined_downward_but_not_upward() {
    let f = |x: f64| x.sin();
    let exact = 1.0_f64.cos();

    // Too large: refinement shrinks h and the answer is good.
    let big = central(f, 1.0, 0.5);
    assert!(
        (big.value - exact).abs() < 1e-3,
        "h = 0.5 gave {}, want {exact}",
        big.value
    );

    // Too small: NOT rescued -- several digits are simply gone.
    let tiny = central(f, 1.0, 1e-13);
    let tiny_err = (tiny.value - exact).abs();
    assert!(
        tiny_err > 1e-6,
        "h = 1e-13 was unexpectedly accurate ({tiny_err:e}); if upstream gained \
         an upward refinement this test and the module docs both need updating"
    );

    // ...but the routine SAYS so, which is the actual protection.
    assert!(
        tiny.abserr > tiny_err,
        "the error estimate {:e} must cover the true error {tiny_err:e}",
        tiny.abserr
    );
    assert!(
        tiny.abserr > 1e-6,
        "a too-small h must produce a visibly large error estimate, got {:e}",
        tiny.abserr
    );
}

/// Forward and backward must agree on a smooth function, since they approach
/// the same limit from opposite sides.
#[test]
fn forward_and_backward_agree_on_a_smooth_function() {
    for &(name, f, _, x) in CASES {
        let fwd = forward(f, x, 1e-4).value;
        let bwd = backward(f, x, 1e-4).value;
        assert!(
            (fwd - bwd).abs() < 1e-6 * fwd.abs().max(1.0),
            "{name}: forward {fwd} vs backward {bwd}"
        );
    }
}

/// The documented accuracy ceiling, demonstrated rather than asserted: no
/// choice of `h` reaches machine precision, because that is a property of
/// finite differencing and not of this implementation.
#[test]
fn no_step_size_reaches_machine_precision() {
    let f = |x: f64| x.exp();
    let exact = 1.0_f64.exp();
    let mut best = f64::INFINITY;
    for k in 1..=16 {
        let h = 10.0_f64.powi(-k);
        let err = (central(f, 1.0, h).value - exact).abs() / exact;
        if err < best {
            best = err;
        }
    }
    // Comfortably better than 1e-9...
    assert!(best < 1e-10, "best relative error over all h was {best:e}");
    // ...but nowhere near machine precision, as the module docs say.
    assert!(
        best > 1e-17,
        "best relative error {best:e} claims better than eps, which cannot be right"
    );
}
