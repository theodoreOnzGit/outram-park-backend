// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 Theodore Ong and the outram-park contributors.
//
// Test integrands from the GNU Scientific Library (GSL) 2.8,
// integration/tests.c, commit cf180cd7fbd06039a577f9c9ff0b428784765ac1.
// Copyright (C) 1996-2000 Brian Gough. GPL-3.0-or-later.

//! Verifies PETIR's quadrature against integrands with known closed forms,
//! including several from GSL's own QUADPACK test set.

use petir::integration::{kronrod, qag, QkRule};
use petir::PetirError;

const ALL_RULES: [QkRule; 6] = [
    QkRule::Qk15,
    QkRule::Qk21,
    QkRule::Qk31,
    QkRule::Qk41,
    QkRule::Qk51,
    QkRule::Qk61,
];

/// A Gauss-Kronrod rule with `n` Kronrod points wraps a Gauss rule of degree
/// `2g - 1`; the combined rule is exact to degree `3g + 1` at least. Every one
/// of them must therefore integrate a low-degree polynomial EXACTLY, which is
/// the sharpest possible check on the node and weight tables: a single mistyped
/// digit breaks it immediately.
#[test]
fn every_rule_integrates_low_degree_polynomials_exactly() {
    for rule in ALL_RULES {
        // integral of 1 from 0 to 1 = 1
        let r = kronrod(rule, |_x: f64| 1.0, 0.0, 1.0);
        assert!((r.result - 1.0).abs() < 1e-14, "{rule:?} on a constant: {}", r.result);

        // integral of x from 0 to 2 = 2
        let r = kronrod(rule, |x: f64| x, 0.0, 2.0);
        assert!((r.result - 2.0).abs() < 1e-13, "{rule:?} on x: {}", r.result);

        // integral of x^3 from 0 to 2 = 4
        let r = kronrod(rule, |x: f64| x * x * x, 0.0, 2.0);
        assert!((r.result - 4.0).abs() < 1e-12, "{rule:?} on x^3: {}", r.result);

        // integral of x^5 from -1 to 1 = 0 (odd)
        let r = kronrod(rule, |x: f64| x.powi(5), -1.0, 1.0);
        assert!(r.result.abs() < 1e-14, "{rule:?} on x^5 over [-1,1]: {}", r.result);
    }
}

/// The weights of any quadrature rule must sum to the interval length --
/// equivalently, it integrates 1 exactly. Checked on a non-unit interval so a
/// scaling error in the half-length cannot hide.
#[test]
fn every_rule_has_weights_summing_to_the_interval() {
    for rule in ALL_RULES {
        let r = kronrod(rule, |_x: f64| 1.0, -3.0, 7.0);
        assert!(
            (r.result - 10.0).abs() < 1e-12,
            "{rule:?}: integral of 1 over [-3, 7] = {}, want 10",
            r.result
        );
    }
}

/// GSL's `f1` from integration/tests.c: `x^alpha * ln(1/x)` with alpha = 2.6,
/// whose integral over [0, 1] is `1 / (alpha + 1)^2`.
#[test]
fn adaptive_quadrature_matches_the_closed_form_for_gsl_f1() {
    let alpha = 2.6_f64;
    let f = |x: f64| x.powf(alpha) * (1.0 / x).ln();
    let exact = 1.0 / ((alpha + 1.0) * (alpha + 1.0));

    let r = qag(QkRule::Qk21, f, 0.0, 1.0, 0.0, 1e-10, 200).unwrap();
    assert!(
        (r.value - exact).abs() < 1e-10,
        "got {}, exact {exact}, abserr {}",
        r.value,
        r.abserr
    );
    assert!(r.abserr >= (r.value - exact).abs(), "error estimate must bound the error");
}

/// A set of integrands with elementary closed forms, run through every rule.
#[test]
fn adaptive_quadrature_matches_elementary_closed_forms() {
    let pi = core::f64::consts::PI;
    #[allow(clippy::type_complexity)]
    let cases: [(&str, fn(f64) -> f64, f64, f64, f64); 5] = [
        ("exp", |x| x.exp(), 0.0, 1.0, core::f64::consts::E - 1.0),
        ("sin over [0, pi]", |x| x.sin(), 0.0, core::f64::consts::PI, 2.0),
        ("1/x over [1, e]", |x| 1.0 / x, 1.0, core::f64::consts::E, 1.0),
        ("1/(1+x^2) over [0,1]", |x| 1.0 / (1.0 + x * x), 0.0, 1.0, core::f64::consts::PI / 4.0),
        ("x^2 over [0,3]", |x| x * x, 0.0, 3.0, 9.0),
    ];

    for rule in ALL_RULES {
        for &(name, f, a, b, exact) in &cases {
            let r = qag(rule, f, a, b, 0.0, 1e-11, 200)
                .unwrap_or_else(|e| panic!("{rule:?} on {name}: {e}"));
            assert!(
                (r.value - exact).abs() < 1e-9,
                "{rule:?} on {name}: got {}, want {exact}",
                r.value
            );
        }
    }
    let _ = pi;
}

/// The reported error must actually bound the true error -- an estimate that
/// under-reports is worse than none, because adaptivity stops on it.
#[test]
fn the_reported_error_bounds_the_true_error() {
    let f = |x: f64| (x * 7.0).sin() * x.exp();
    // integral of e^x sin(7x) = e^x (sin 7x - 7 cos 7x) / 50
    let anti = |x: f64| x.exp() * ((7.0 * x).sin() - 7.0 * (7.0 * x).cos()) / 50.0;
    let exact = anti(1.0) - anti(0.0);

    for rule in ALL_RULES {
        let r = qag(rule, f, 0.0, 1.0, 0.0, 1e-10, 300).unwrap();
        let true_err = (r.value - exact).abs();
        assert!(
            true_err <= r.abserr.max(1e-13),
            "{rule:?}: true error {true_err:e} exceeds estimate {:e}",
            r.abserr
        );
    }
}

/// Adaptivity must pay off: a function that is nasty in one small region
/// should need far fewer evaluations adaptively than a single rule can manage.
#[test]
fn adaptivity_concentrates_subdivision_where_the_error_is() {
    // A narrow spike at x = 0.3 on an otherwise flat interval.
    let f = |x: f64| 1.0 / (1.0e-4 + (x - 0.3) * (x - 0.3));
    // integral = (1/c) [atan((x-0.3)/c)] with c = 1e-2
    let c = 1.0e-2_f64;
    let anti = |x: f64| ((x - 0.3) / c).atan() / c;
    let exact = anti(1.0) - anti(0.0);

    let r = qag(QkRule::Qk21, f, 0.0, 1.0, 0.0, 1e-8, 500).unwrap();
    assert!(
        (r.value - exact).abs() / exact < 1e-7,
        "got {}, exact {exact}",
        r.value
    );
    // A single application of the same rule should be far worse, which is what
    // makes the subdivision worth its cost.
    let single = kronrod(QkRule::Qk21, f, 0.0, 1.0);
    assert!(
        (single.result - exact).abs() > (r.value - exact).abs() * 100.0,
        "one rule gave {}, adaptive gave {}, exact {exact}",
        single.result,
        r.value
    );
}

/// A high-order rule should beat a low-order one on an OSCILLATORY integrand,
/// which is the documented reason to choose Qk61.
#[test]
fn a_high_order_rule_wins_on_an_oscillatory_integrand() {
    let f = |x: f64| (40.0 * x).sin();
    let exact = (1.0 - (40.0_f64).cos()) / 40.0;

    let e15 = (kronrod(QkRule::Qk15, f, 0.0, 1.0).result - exact).abs();
    let e61 = (kronrod(QkRule::Qk61, f, 0.0, 1.0).result - exact).abs();
    assert!(
        e61 < e15,
        "Qk61 error {e61:e} should beat Qk15 error {e15:e} on sin(40x)"
    );
}

#[test]
fn malformed_input_is_reported() {
    let f = |x: f64| x;
    // Infinite range needs QAGI, which is not ported.
    assert!(matches!(
        qag(QkRule::Qk21, f, 0.0, f64::INFINITY, 0.0, 1e-8, 50),
        Err(PetirError::Domain)
    ));
    // An unattainable tolerance is refused up front.
    assert!(matches!(
        qag(QkRule::Qk21, f, 0.0, 1.0, 0.0, 0.0, 50),
        Err(PetirError::Tolerance)
    ));
    // A zero subdivision limit is meaningless.
    assert!(matches!(
        qag(QkRule::Qk21, f, 0.0, 1.0, 1e-6, 0.0, 0),
        Err(PetirError::Invalid)
    ));
}

/// The documented limitation, pinned: an endpoint singularity is what QAGS
/// exists for, and `qag` must FAIL LOUDLY rather than return a plausible wrong
/// number.
#[test]
fn an_endpoint_singularity_is_reported_not_silently_wrong() {
    // integral of 1/sqrt(x) from 0 to 1 = 2, but the integrand is unbounded
    // at 0. Guard the exact endpoint so the rule does not evaluate infinity.
    let f = |x: f64| if x > 0.0 { 1.0 / x.sqrt() } else { 0.0 };
    let outcome = qag(QkRule::Qk21, f, 0.0, 1.0, 0.0, 1e-12, 60);
    assert!(
        matches!(
            outcome,
            Err(PetirError::MaxIterations) | Err(PetirError::RoundOff)
        ),
        "expected a reported failure on a singular integrand, got {outcome:?}"
    );
}

/// Reversing the limits negates the integral, as it must.
#[test]
fn reversing_the_limits_negates_the_result() {
    let f = |x: f64| x * x;
    let fwd = qag(QkRule::Qk21, f, 0.0, 3.0, 0.0, 1e-10, 100).unwrap();
    let rev = qag(QkRule::Qk21, f, 3.0, 0.0, 0.0, 1e-10, 100).unwrap();
    assert!(
        (fwd.value + rev.value).abs() < 1e-10,
        "forward {} and reversed {} should sum to zero",
        fwd.value,
        rev.value
    );
}

#[test]
fn rule_point_counts_are_what_they_claim() {
    assert_eq!(QkRule::Qk15.points(), 15);
    assert_eq!(QkRule::Qk21.points(), 21);
    assert_eq!(QkRule::Qk31.points(), 31);
    assert_eq!(QkRule::Qk41.points(), 41);
    assert_eq!(QkRule::Qk51.points(), 51);
    assert_eq!(QkRule::Qk61.points(), 61);
}
