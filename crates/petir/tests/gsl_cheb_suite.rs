//! **V&V — GSL's own `cheb/test.c`, ported.**
//!
//! Ported from the GNU Scientific Library `cheb/test.c`, GSL 2.8 at commit
//! `cf180cd7fbd06039a577f9c9ff0b428784765ac1`, read 2026-09-14.
//! Copyright (C) 1996-2000 Gerard Jungman (upstream), GPL-3.0-or-later.
//!
//! # Why this file is the port's verification, rather than tests written here
//!
//! `bn:op-chyp` decision 2: *numerics are ported from a library with real V&V,
//! never written from scratch* — and the point of that rule is that the port
//! **inherits the upstream suite**. Tests invented alongside a port only
//! check the port against its own author's understanding. These check it
//! against the assertions GSL's maintainers commit to.
//!
//! Everything is kept as upstream has it: the same test functions, the same
//! sweeps (`x` from `a` to `b` in 100 steps), the same tolerances
//! (`tol = 100 * DBL_EPSILON`, and upstream's looser `1600 * tol` for the
//! derivative and `100 * tol` for `eval_n_err`), and — the strongest part —
//! the same **hard-coded reference coefficients** for `sin(x)` on `[-π, π]`,
//! which are upstream's own published numbers and cannot be reproduced by
//! accident.
//!
//! Upstream guards its derivative- and integral-error checks behind
//! `#ifdef TEST_DERIVATIVE_ERR` / `TEST_INTEGRAL_ERR`, which are not defined in
//! its build. Those blocks are ported but marked `#[ignore]`, so they are
//! present and runnable without asserting something upstream itself does not.

use petir::ChebSeries;

/// `tol` in `cheb/test.c:74` — `100.0 * GSL_DBL_EPSILON`.
const TOL: f64 = 100.0 * f64::EPSILON;
/// `ftol` in `cheb/test.c:112` — the factor bound on the error estimate.
const FTOL: f64 = 20.0;

/// `gsl_test_abs`: |observed - expected| <= tol.
#[track_caller]
fn test_abs(observed: f64, expected: f64, tol: f64, what: &str) {
    let d = (observed - expected).abs();
    assert!(
        d <= tol,
        "{what}: |{observed:e} - {expected:e}| = {d:e} > {tol:e}"
    );
}

/// `gsl_test_factor`: the two agree to within a multiplicative `factor`.
#[track_caller]
fn test_factor(observed: f64, expected: f64, factor: f64, what: &str) {
    let ok = if observed == expected {
        true
    } else if expected == 0.0 {
        observed.abs() <= factor
    } else {
        let u = observed / expected;
        u <= factor && u >= 1.0 / factor
    };
    assert!(ok, "{what}: {observed:e} vs {expected:e}, factor > {factor}");
}

// The test functions of cheb/test.c:28-58.
fn f_t0(_x: f64) -> f64 { 1.0 }
fn f_t1(x: f64) -> f64 { x }
fn f_t2(x: f64) -> f64 { 2.0 * x * x - 1.0 }
fn f_dp(_x: f64) -> f64 { 2.0 }
fn f_p(x: f64) -> f64 { 2.0 * x + 3.0 }
/// First-order approximation to the integral over [-5, 5] (upstream's comment).
fn f_ip1(x: f64) -> f64 { 30.0 * (x + 5.0) / 10.0 }
fn f_ip2(x: f64) -> f64 { x * x + 3.0 * x - 10.0 }

/// `test_dim` (`cheb/test.c:70-106`): build, then check the series, its
/// derivative and its integral against closed forms over the interval.
fn test_dim(n: usize, a: f64, b: f64, f: fn(f64) -> f64, df: fn(f64) -> f64, ifn: fn(f64) -> f64) {
    let cs = ChebSeries::new(n, a, b, f).expect("init");
    let step = (b - a) / 100.0;

    let mut x = a;
    while x < b {
        test_abs(cs.eval(x), f(x), TOL, &format!("cheb_eval, F({x:.3})"));
        x += step;
    }

    let csd = cs.deriv();
    let mut x = a;
    while x < b {
        test_abs(csd.eval(x), df(x), TOL, &format!("cheb_eval, deriv F({x:.3})"));
        x += step;
    }

    let csi = cs.integ();
    let mut x = a;
    while x < b {
        test_abs(csi.eval(x), ifn(x), TOL, &format!("cheb_eval, integ F({x:.3})"));
        x += step;
    }
}

/// `cheb/test.c:150-183` — a Chebyshev polynomial's own expansion must be a
/// single non-zero coefficient. `T_0` gives `c[0] = 2` (not 1: upstream's
/// convention carries the factor of two that `eval` undoes with its
/// `0.5 * c[0]`).
#[test]
fn chebyshev_polynomials_have_unit_expansions() {
    let cs = ChebSeries::new(40, -1.0, 1.0, f_t0).unwrap();
    assert_eq!(cs.order(), 40, "cheb_order");
    assert_eq!(cs.size(), 41, "cheb_size");
    for i in 0..cs.order() {
        let expected = if i == 0 { 2.0 } else { 0.0 };
        test_abs(cs.coefficients()[i], expected, TOL, &format!("c[{i}] for T_0(x)"));
    }

    let cs = ChebSeries::new(40, -1.0, 1.0, f_t1).unwrap();
    for i in 0..cs.order() {
        let expected = if i == 1 { 1.0 } else { 0.0 };
        test_abs(cs.coefficients()[i], expected, TOL, &format!("c[{i}] for T_1(x)"));
    }

    let cs = ChebSeries::new(40, -1.0, 1.0, f_t2).unwrap();
    for i in 0..cs.order() {
        let expected = if i == 2 { 1.0 } else { 0.0 };
        test_abs(cs.coefficients()[i], expected, TOL, &format!("c[{i}] for T_2(x)"));
    }
}

/// **The strongest check in the suite** (`cheb/test.c:185-192`): the first six
/// coefficients of `sin(x)` on `[-π, π]`, as hard-coded by GSL's maintainers.
/// These are upstream's own published numbers — reproducing them to
/// `100 * DBL_EPSILON` is not something a wrong transcription does by luck.
#[test]
fn sin_coefficients_match_gsls_published_values() {
    let cs = ChebSeries::new(40, -core::f64::consts::PI, core::f64::consts::PI, f64::sin).unwrap();
    let c = cs.coefficients();
    test_abs(c[0], 0.0, TOL, "c[0] for F_sin(x)");
    test_abs(c[1], 5.69230686359506e-01, TOL, "c[1] for F_sin(x)");
    test_abs(c[2], 0.0, TOL, "c[2] for F_sin(x)");
    test_abs(c[3], -6.66916672405979e-01, TOL, "c[3] for F_sin(x)");
    test_abs(c[4], 0.0, TOL, "c[4] for F_sin(x)");
    test_abs(c[5], 1.04282368734237e-01, TOL, "c[5] for F_sin(x)");
}

/// `cheb/test.c:194-218` — evaluation, error estimate, and truncated
/// evaluation of the `sin` series.
#[test]
fn sin_series_evaluates_and_bounds_its_own_error() {
    let pi = core::f64::consts::PI;
    let cs = ChebSeries::new(40, -pi, pi, f64::sin).unwrap();
    let step = pi / 100.0;

    let mut x = -pi;
    while x < pi {
        test_abs(cs.eval(x), x.sin(), TOL, &format!("cheb_eval, sin({x:.3})"));
        x += step;
    }

    let mut x = -pi;
    while x < pi {
        let (r, e) = cs.eval_err(x);
        test_abs(r, x.sin(), TOL, &format!("cheb_eval_err, sin({x:.3})"));
        test_factor(
            (r - x.sin()).abs() + f64::EPSILON,
            e,
            FTOL,
            &format!("cheb_eval_err, error sin({x:.3})"),
        );
        x += step;
    }

    let mut x = -pi;
    while x < pi {
        test_abs(cs.eval_n(25, x), x.sin(), TOL, &format!("cheb_eval_n, sin({x:.3})"));
        x += step;
    }

    let mut x = -pi;
    while x < pi {
        let (r, e) = cs.eval_n_err(25, x);
        test_abs(r, x.sin(), 100.0 * TOL, &format!("cheb_eval_n_err, sin({x:.3})"));
        test_factor(
            (r - x.sin()).abs() + f64::EPSILON,
            e,
            FTOL,
            &format!("cheb_eval_n_err, error sin({x:.3})"),
        );
        x += step;
    }
}

/// `cheb/test.c:220-243` — differentiating the `sin` series must give `cos`.
/// Upstream's tolerance here is `1600 * tol`, not `tol`: differentiation
/// amplifies coefficient error by `n^2`, and it does not pretend otherwise.
#[test]
fn derivative_of_the_sin_series_is_cos() {
    let pi = core::f64::consts::PI;
    let csd = ChebSeries::new(40, -pi, pi, f64::sin).unwrap().deriv();
    let step = pi / 100.0;

    let mut x = -pi;
    while x < pi {
        test_abs(csd.eval(x), x.cos(), 1600.0 * TOL, &format!("cheb_eval, deriv sin({x:.3})"));
        x += step;
    }

    let mut x = -pi;
    while x < pi {
        test_abs(
            csd.eval_n(25, x),
            x.cos(),
            1600.0 * TOL,
            &format!("cheb_eval_n, deriv sin({x:.3})"),
        );
        x += step;
    }
}

/// `cheb/test.c:255-278` — integrating the `sin` series gives `-(1 + cos x)`,
/// which fixes upstream's constant of integration: the integral vanishes at
/// `x = -π`.
#[test]
fn integral_of_the_sin_series_is_minus_one_plus_cos() {
    let pi = core::f64::consts::PI;
    let csi = ChebSeries::new(40, -pi, pi, f64::sin).unwrap().integ();
    let step = pi / 100.0;

    let mut x = -pi;
    while x < pi {
        test_abs(csi.eval(x), -(1.0 + x.cos()), TOL, &format!("cheb_eval, integ sin({x:.3})"));
        x += step;
    }

    let mut x = -pi;
    while x < pi {
        test_abs(
            csi.eval_n(25, x),
            -(1.0 + x.cos()),
            TOL,
            &format!("cheb_eval_n, integ sin({x:.3})"),
        );
        x += step;
    }
}

/// `cheb/test.c:291-292` — the low-order cases, where the `n == 1` and
/// `n == 2` special branches of `integ` are the code under test.
#[test]
fn low_order_cases() {
    test_dim(2, -5.0, 5.0, f_p, f_dp, f_ip2);
    test_dim(1, -5.0, 5.0, f_p, f_dp, f_ip1);
}

/// Ported but `#[ignore]`d, because upstream does not define
/// `TEST_DERIVATIVE_ERR` / `TEST_INTEGRAL_ERR` in its build
/// (`cheb/test.c:229, 245, 262, 279`). Kept so the blocks are not silently
/// lost, and so anyone who wants to know how the error estimate behaves on a
/// differentiated series can run it deliberately.
#[test]
#[ignore = "upstream leaves TEST_DERIVATIVE_ERR / TEST_INTEGRAL_ERR undefined"]
fn derivative_and_integral_error_estimates() {
    let pi = core::f64::consts::PI;
    let cs = ChebSeries::new(40, -pi, pi, f64::sin).unwrap();
    let (csd, csi) = (cs.deriv(), cs.integ());
    let step = pi / 100.0;

    let mut x = -pi;
    while x < pi {
        let (r, e) = csd.eval_err(x);
        test_abs(r, x.cos(), TOL, &format!("cheb_eval_err, deriv sin({x:.3})"));
        test_factor((r - x.cos()).abs() + f64::EPSILON, e, FTOL, "deriv error");
        let (r, e) = csi.eval_err(x);
        test_abs(r, -(1.0 + x.cos()), TOL, &format!("cheb_eval_err, integ sin({x:.3})"));
        test_factor((r + 1.0 + x.cos()).abs() + f64::EPSILON, e, FTOL, "integ error");
        x += step;
    }
}
