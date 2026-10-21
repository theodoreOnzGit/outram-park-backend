// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 Theodore Ong and the outram-park contributors.
//
// Test functions from the GNU Scientific Library (GSL) 2.8, min/test_funcs.c,
// commit cf180cd7fbd06039a577f9c9ff0b428784765ac1.
// Copyright (C) 1996-2000 Reid Priedhorsky, Brian Gough. GPL-3.0-or-later.

//! Verifies PETIR's minimisers against GSL's own minimisation test functions.

use petir::min::{MinMethod, Minimizer};
use petir::PetirError;

const METHODS: [MinMethod; 2] = [MinMethod::GoldenSection, MinMethod::Brent];

/// The accuracy floor for a minimiser -- see the module docs. A minimum's
/// LOCATION cannot be pinned better than about sqrt(eps) relative, because `f`
/// is locally quadratic there.
const FLOOR: f64 = 1e-5;

#[test]
fn both_methods_find_the_minimum_of_cos() {
    let pi = core::f64::consts::PI;
    for m in METHODS {
        let mut s = Minimizer::new(m, |x: f64| x.cos(), pi - 0.5, 0.0, 2.0 * pi).unwrap();
        let x = s
            .solve(1e-6, 0.0, 2000)
            .unwrap_or_else(|e| panic!("{m:?} failed: {e}"));
        assert!((x - pi).abs() < FLOOR, "{m:?}: minimum at {x}, want {pi}");
        assert!((s.f_minimum() + 1.0).abs() < 1e-14, "{m:?}: f = {}", s.f_minimum());
    }
}

/// GSL's `func1`: `x^4 - 1`, minimum at 0.
///
/// A quartic minimum is far flatter than a quadratic one -- `f` changes by
/// `d^4`, not `d^2` -- so the location can only be pinned to about
/// `eps^(1/4)`, around `1e-4`. Golden section additionally STALLS here, and
/// that is upstream behaviour faithfully ported rather than a defect: when the
/// probe lands where `f_new == f_min` to the last bit, none of GSL's three
/// update branches applies and `goldensection_iterate` returns `GSL_FAILURE`.
/// PETIR reports that as [`PetirError::NoProgress`].
///
/// So the test asserts what is actually true of each method: both must get
/// close, and golden section is permitted to stop early saying so.
#[test]
fn both_methods_find_the_minimum_of_x_to_the_fourth() {
    for m in METHODS {
        let mut s = Minimizer::new(m, |x: f64| x.powf(4.0) - 1.0, -0.1, -2.0, 3.0).unwrap();
        match s.solve(1e-6, 0.0, 2000) {
            Ok(x) => assert!(x.abs() < 1e-3, "{m:?}: minimum at {x}, want 0"),
            Err(PetirError::NoProgress) => {
                assert_eq!(
                    m,
                    MinMethod::GoldenSection,
                    "only golden section is expected to stall on a flat minimum"
                );
                // It must still have got close before stalling -- stalling is
                // acceptable, stalling in the wrong place is not.
                assert!(
                    s.x_minimum().abs() < 1e-3,
                    "golden section stalled at {}, want near 0",
                    s.x_minimum()
                );
            }
            Err(e) => panic!("{m:?} failed unexpectedly: {e}"),
        }
    }
}

/// GSL's `func2`: sqrt(|x|), minimum at 0 with an infinite derivative.
#[test]
fn both_methods_handle_an_infinite_derivative_at_the_minimum() {
    for m in METHODS {
        let mut s = Minimizer::new(m, |x: f64| x.abs().sqrt(), -0.1, -2.0, 1.5).unwrap();
        let x = s
            .solve(1e-6, 0.0, 2000)
            .unwrap_or_else(|e| panic!("{m:?} failed: {e}"));
        assert!(x.abs() < 1e-6, "{m:?}: minimum at {x}, want 0");
    }
}

/// A minimum away from the origin, to catch a relative-tolerance mistake that
/// a minimum at zero would hide.
#[test]
fn both_methods_find_an_off_origin_minimum() {
    // (x - 3)^2 + 1, minimum at 3.
    let f = |x: f64| (x - 3.0) * (x - 3.0) + 1.0;
    for m in METHODS {
        let mut s = Minimizer::new(m, f, 2.5, 0.0, 10.0).unwrap();
        let x = s
            .solve(1e-6, 0.0, 2000)
            .unwrap_or_else(|e| panic!("{m:?} failed: {e}"));
        assert!((x - 3.0).abs() < FLOOR, "{m:?}: minimum at {x}, want 3");
        assert!((s.f_minimum() - 1.0).abs() < 1e-12);
    }
}

/// The bracketing precondition is what makes the method sound, so it must be
/// enforced rather than assumed.
#[test]
fn a_triple_that_does_not_enclose_a_minimum_is_rejected() {
    let f = |x: f64| x * x;
    // Middle point is not below both ends: f(1) = 1 > f(0) = 0.
    // `unwrap_err` would need the Minimizer (and so the closure) to be Debug,
    // which a closure is not; match on the error instead.
    assert!(matches!(
        Minimizer::new(MinMethod::Brent, f, 1.0, 0.0, 3.0),
        Err(PetirError::Invalid)
    ));
    // Unordered points.
    assert!(matches!(
        Minimizer::new(MinMethod::Brent, f, 5.0, 0.0, 3.0),
        Err(PetirError::Invalid)
    ));
    // A monotonic function has no interior minimum to bracket.
    // NOTE: the closure must be bound first. Written inline, the `|` of
    // `|x: f64|` is parsed as a pattern alternation by `matches!`.
    let monotonic = Minimizer::new(MinMethod::Brent, |x: f64| x, 1.0, 0.0, 3.0);
    assert!(matches!(monotonic, Err(PetirError::Invalid)));
}

/// A valid bracket must be accepted.
#[test]
fn a_valid_bracketing_triple_is_accepted() {
    let f = |x: f64| x * x;
    let s = Minimizer::new(MinMethod::Brent, f, 0.5, -2.0, 3.0).unwrap();
    assert_eq!(s.x_lower(), -2.0);
    assert_eq!(s.x_upper(), 3.0);
    assert_eq!(s.x_minimum(), 0.5);
    assert!((s.f_minimum() - 0.25).abs() < 1e-15);
}

/// The bracket must shrink monotonically and keep enclosing the best point.
/// This is the invariant the whole method rests on.
#[test]
fn the_bracket_shrinks_and_keeps_enclosing_the_minimum() {
    for m in METHODS {
        let mut s = Minimizer::new(m, |x: f64| x.cos(), 3.0, 0.0, 6.0).unwrap();
        let mut width = s.x_upper() - s.x_lower();
        for i in 0..60 {
            if s.iterate().is_err() {
                break;
            }
            let new_width = s.x_upper() - s.x_lower();
            assert!(
                new_width <= width + 1e-15,
                "{m:?} step {i}: bracket grew, {width} -> {new_width}"
            );
            assert!(
                s.x_lower() <= s.x_minimum() && s.x_minimum() <= s.x_upper(),
                "{m:?} step {i}: best point {} escaped [{}, {}]",
                s.x_minimum(),
                s.x_lower(),
                s.x_upper()
            );
            width = new_width;
        }
        assert!(width < 1e-6, "{m:?}: bracket only reached {width}");
    }
}

/// Brent should beat golden section on a smooth function -- that is why it is
/// the recommended default.
#[test]
fn brent_converges_in_fewer_iterations_than_golden_section() {
    fn count(method: MinMethod) -> usize {
        let f = |x: f64| (x - 3.0) * (x - 3.0) + 1.0;
        let mut s = Minimizer::new(method, f, 2.5, 0.0, 10.0).unwrap();
        for i in 1..=2000 {
            if s.iterate().is_err() {
                return usize::MAX;
            }
            if s.x_upper() - s.x_lower() < 1e-6 {
                return i;
            }
        }
        usize::MAX
    }
    let golden = count(MinMethod::GoldenSection);
    let brent = count(MinMethod::Brent);
    assert!(
        brent < golden,
        "Brent took {brent} iterations, golden section {golden}"
    );
}

/// Golden section shrinks the bracket by a FIXED ratio every step -- its
/// defining property, and the reason it is predictable.
#[test]
fn golden_section_shrinks_the_bracket_at_a_steady_rate() {
    let f = |x: f64| (x - 3.0) * (x - 3.0) + 1.0;
    let mut s = Minimizer::new(MinMethod::GoldenSection, f, 2.5, 0.0, 10.0).unwrap();
    let start = s.x_upper() - s.x_lower();
    for _ in 0..40 {
        s.iterate().unwrap();
    }
    let end = s.x_upper() - s.x_lower();
    // 40 steps at a ratio strictly below 1 must shrink it by orders of
    // magnitude, but never to zero.
    assert!(end < start * 1e-3, "{start} -> {end}");
    assert!(end > 0.0);
}

/// The documented accuracy floor, demonstrated: asking for better than
/// sqrt(eps) on the LOCATION cannot succeed, and the failure is reported
/// rather than faked.
#[test]
fn asking_for_better_than_the_sqrt_eps_floor_reports_max_iterations() {
    let f = |x: f64| (x - 3.0) * (x - 3.0) + 1.0;
    let mut s = Minimizer::new(MinMethod::Brent, f, 2.5, 0.0, 10.0).unwrap();
    let outcome = s.solve(1e-17, 0.0, 300);
    assert!(
        matches!(outcome, Err(PetirError::MaxIterations)),
        "an unattainable tolerance must be reported, not silently accepted"
    );
    // The estimate is still good to the floor, which is the useful part.
    assert!((s.x_minimum() - 3.0).abs() < 1e-7);
}
