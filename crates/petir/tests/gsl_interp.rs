// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 Theodore Ong and the outram-park contributors.
//
// Test data from the GNU Scientific Library (GSL) 2.8, interpolation/test.c,
// commit cf180cd7fbd06039a577f9c9ff0b428784765ac1.
// Copyright (C) 1996-2000 Gerard Jungman, Brian Gough. GPL-3.0-or-later.

//! Verifies PETIR's interpolants against GSL's own interpolation test data,
//! including the hard-coded cubic-spline reference values from upstream's
//! `test_cspline` case.

use petir::interp::{Interp, InterpMethod};
use petir::PetirError;

/// The defining property of ANY interpolant: it passes through every knot.
#[test]
fn every_method_reproduces_the_knots_exactly() {
    let x = [0.0, 1.0, 2.0, 3.0, 4.0];
    let y = [1.0, 0.5, 2.0, -1.0, 3.0];

    for m in [InterpMethod::Linear, InterpMethod::CubicSpline] {
        let s = Interp::new(m, &x, &y).unwrap();
        for (&xi, &yi) in x.iter().zip(y.iter()) {
            let got = s.eval(xi).unwrap();
            assert!(
                (got - yi).abs() < 1e-12,
                "{m:?}: at knot {xi} got {got}, want {yi}"
            );
        }
    }
}

/// GSL's `test_cspline` case: three points on y = x^2, with upstream's own
/// hard-coded expected values at the test abscissae.
#[test]
fn cubic_spline_matches_gsl_test_cspline() {
    // GSL interpolation/test.c, test_cspline -- read from the vendored source.
    // An earlier version of this test carried values I had recalled rather than
    // read, and they were wrong: I had the spline running through x^2 with
    // derivatives 0.75/1.5/2.25, where upstream's case is a straight line. The
    // port was right and the "reference" was invented. Everything below now
    // comes out of the file.
    let data_x = [0.0, 1.0, 2.0];
    let data_y = [0.0, 1.0, 2.0];
    let test_x = [0.0, 0.5, 1.0, 2.0];
    let test_y = [0.0, 0.5, 1.0, 2.0];
    let test_dy = [1.0, 1.0, 1.0, 1.0];

    let s = Interp::new(InterpMethod::CubicSpline, &data_x, &data_y).unwrap();
    for i in 0..test_x.len() {
        let y = s.eval(test_x[i]).unwrap();
        assert!(
            (y - test_y[i]).abs() < 1e-10,
            "eval at {}: got {y}, GSL expects {}",
            test_x[i],
            test_y[i]
        );
        let dy = s.eval_deriv(test_x[i]).unwrap();
        assert!(
            (dy - test_dy[i]).abs() < 1e-10,
            "deriv at {}: got {dy}, GSL expects {}",
            test_x[i],
            test_dy[i]
        );
    }
}

/// GSL `test_linear` (interpolation/test.c), values read from the source.
#[test]
fn linear_matches_gsl_test_linear() {
    let data_x = [0.0, 1.0, 2.0, 3.0];
    let data_y = [0.0, 1.0, 2.0, 3.0];
    let test_x = [0.0, 0.5, 1.0, 1.5, 2.5, 3.0];
    let test_y = [0.0, 0.5, 1.0, 1.5, 2.5, 3.0];
    let test_dy = [1.0, 1.0, 1.0, 1.0, 1.0, 1.0];

    let s = Interp::new(InterpMethod::Linear, &data_x, &data_y).unwrap();
    for i in 0..test_x.len() {
        assert!((s.eval(test_x[i]).unwrap() - test_y[i]).abs() < 1e-12);
        assert!((s.eval_deriv(test_x[i]).unwrap() - test_dy[i]).abs() < 1e-12);
    }
}

/// GSL `test_cspline2` -- Young & Gregory, *A Survey of Numerical Mathematics*
/// Vol 1 ch. 6.8, as vendored in `interpolation/test.c`.
///
/// This is the case that actually exercises a CURVED spline: 50 reference
/// values against six knots of `1/(1 + x^2)`-like data. The arrays were
/// extracted mechanically from the vendored file, not retyped.
#[test]
fn cubic_spline_matches_gsl_test_cspline2() {
    let data_x = [0.0, 0.2, 0.4, 0.6, 0.8, 1.0];
    let data_y = [
        1.0,
        0.961538461538461,
        0.862068965517241,
        0.735294117647059,
        0.609756097560976,
        0.500000000000000,
    ];

    let s = Interp::new(InterpMethod::CubicSpline, &data_x, &data_y).unwrap();

    let mut worst = 0.0_f64;
    for i in 0..CSPLINE2_TEST_X.len() {
        let got = s.eval(CSPLINE2_TEST_X[i]).unwrap();
        let want = CSPLINE2_TEST_Y[i];
        let err = (got - want).abs();
        if err > worst {
            worst = err;
        }
        assert!(
            err < 1e-12,
            "at x = {}: got {got}, GSL expects {want} (error {err:e})",
            CSPLINE2_TEST_X[i]
        );
    }
    // Recorded so a regression shows up as a number, not just a pass/fail.
    assert!(worst < 1e-12, "worst deviation from GSL was {worst:e}");
}

/// The natural boundary condition: zero curvature at both ends.
///
/// Checked by CONTRAST rather than against an absolute epsilon. The second
/// derivative is exactly zero AT each endpoint, but a finite difference has to
/// sample just inside, where the cubic's own third derivative makes it
/// genuinely non-zero -- an earlier version of this test measured 1.7e-4 there
/// and read it as a failure when it was the correct value at the sampled point.
/// Comparing end curvature against interior curvature is the honest form: the
/// ratio is what the boundary condition actually constrains.
#[test]
fn the_natural_boundary_condition_holds() {
    let x = [0.0, 1.0, 2.0, 3.0];
    let y = [0.0, 1.0, 8.0, 27.0];
    let s = Interp::new(InterpMethod::CubicSpline, &x, &y).unwrap();

    let h = 1e-5;
    let second_deriv_at = |t: f64| (s.eval_deriv(t + h).unwrap() - s.eval_deriv(t).unwrap()) / h;

    let lo = second_deriv_at(x[0]).abs();
    let hi = second_deriv_at(x[x.len() - 1] - 2.0 * h).abs();
    // Somewhere in the middle, where the spline is genuinely curved.
    let mid = second_deriv_at(1.5).abs();

    assert!(mid > 1.0, "the test data should be curved in the interior, got {mid}");
    assert!(
        lo < mid * 1e-3,
        "curvature at the lower end ({lo}) should be negligible against the \
         interior ({mid})"
    );
    assert!(
        hi < mid * 1e-3,
        "curvature at the upper end ({hi}) should be negligible against the \
         interior ({mid})"
    );
}

/// Linear interpolation is exact on linear data; the spline must be too.
#[test]
fn both_methods_are_exact_on_a_straight_line() {
    let x = [0.0, 1.0, 2.0, 3.0, 4.0];
    let y: Vec<f64> = x.iter().map(|xi| 3.0 * xi - 1.0).collect();

    for m in [InterpMethod::Linear, InterpMethod::CubicSpline] {
        let s = Interp::new(m, &x, &y).unwrap();
        let mut t = 0.0_f64;
        while t <= 4.0 {
            let got = s.eval(t).unwrap();
            let exact = 3.0 * t - 1.0;
            assert!((got - exact).abs() < 1e-10, "{m:?} at {t}: {got} vs {exact}");
            // ...and the slope is 3 everywhere.
            assert!((s.eval_deriv(t).unwrap() - 3.0).abs() < 1e-9, "{m:?} slope at {t}");
            t += 0.137;
        }
    }
}

/// A cubic spline converges at `O(h^4)` in the INTERIOR, against linear's
/// `O(h^2)` everywhere -- but a NATURAL spline drops back to `O(h^2)` in a
/// boundary layer, and this test pins both halves.
///
/// The boundary degradation is not a defect in the port; it is what the natural
/// boundary condition costs. Forcing the second derivative to zero at the ends
/// is simply wrong for a function whose curvature there is not zero (`sin` at
/// `x = 4` has curvature `-sin(4) ~ -0.757`), and that error does not shrink
/// any faster than `h^2`.
///
/// Two earlier versions of this test got this wrong in different ways: first by
/// comparing the two methods at one coarse spacing, then by expecting the
/// order-4 ratio to show up in the MAX-norm over the whole interval, where the
/// boundary layer dominates and the ratio is ~4. Measuring interior and
/// boundary separately is what makes the claim both true and useful.
#[test]
fn the_spline_converges_at_order_four_inside_and_order_two_at_the_boundary() {
    let f = |t: f64| t.sin();

    // `margin` excludes a boundary layer of that width from the error scan.
    let worst_error = |m: InterpMethod, n: usize, margin: f64| -> f64 {
        let x: Vec<f64> = (0..=n).map(|i| 4.0 * i as f64 / n as f64).collect();
        let y: Vec<f64> = x.iter().map(|&t| f(t)).collect();
        let s = Interp::new(m, &x, &y).unwrap();
        let mut worst = 0.0_f64;
        let mut t = margin;
        while t <= 4.0 - margin {
            worst = worst.max((s.eval(t).unwrap() - f(t)).abs());
            t += 0.0013;
        }
        worst
    };

    // Linear is order 2 everywhere, boundary or not.
    let lin_ratio = worst_error(InterpMethod::Linear, 8, 0.0)
        / worst_error(InterpMethod::Linear, 16, 0.0);
    assert!(
        (3.0..=5.5).contains(&lin_ratio),
        "linear should converge at order 2 (ratio ~4), got {lin_ratio}"
    );

    // The spline, measured away from the ends, is order 4.
    let spl_interior = worst_error(InterpMethod::CubicSpline, 8, 1.0)
        / worst_error(InterpMethod::CubicSpline, 16, 1.0);
    assert!(
        spl_interior > 10.0,
        "spline should converge at order 4 in the interior (ratio ~16), got {spl_interior}"
    );

    // Across the WHOLE interval the natural boundary condition drags it back
    // toward order 2. This is the documented cost, so it is asserted, not
    // worked around.
    let spl_all = worst_error(InterpMethod::CubicSpline, 8, 0.0)
        / worst_error(InterpMethod::CubicSpline, 16, 0.0);
    assert!(
        spl_all < 8.0,
        "the natural spline's boundary layer should hold the whole-interval \
         ratio near 4; got {spl_all}, which would mean the boundary condition \
         is better than documented"
    );

    // And it must still beat linear comfortably where it counts.
    assert!(
        worst_error(InterpMethod::CubicSpline, 16, 1.0)
            < worst_error(InterpMethod::Linear, 16, 1.0) * 0.05,
        "spline should be far more accurate than linear in the interior"
    );
}

/// Linear interpolation CANNOT overshoot: every value lies between its
/// neighbours. This is the property that makes it the safe choice for a
/// physical lookup table.
#[test]
fn linear_never_overshoots() {
    let x = [0.0, 1.0, 2.0, 3.0];
    let y = [0.0, 0.0, 1.0, 1.0]; // a step
    let s = Interp::new(InterpMethod::Linear, &x, &y).unwrap();

    let mut t = 0.0_f64;
    while t <= 3.0 {
        let v = s.eval(t).unwrap();
        assert!(
            (0.0..=1.0).contains(&v),
            "linear produced {v} at {t}, outside the data range [0, 1]"
        );
        t += 0.01;
    }
}

/// The documented spline hazard, demonstrated rather than merely warned about:
/// through a sharp step, a cubic spline goes OUTSIDE the range of its own data.
#[test]
fn the_cubic_spline_does_overshoot_as_documented() {
    let x = [0.0, 1.0, 2.0, 3.0];
    let y = [0.0, 0.0, 1.0, 1.0];
    let s = Interp::new(InterpMethod::CubicSpline, &x, &y).unwrap();

    let mut min = f64::INFINITY;
    let mut max = f64::NEG_INFINITY;
    let mut t = 0.0_f64;
    while t <= 3.0 {
        let v = s.eval(t).unwrap();
        min = min.min(v);
        max = max.max(v);
        t += 0.005;
    }
    assert!(
        min < -1e-6 || max > 1.0 + 1e-6,
        "the spline stayed within [{min}, {max}]; the overshoot warning in the \
         module docs would then be wrong and should be corrected"
    );
}

#[test]
fn extrapolation_is_refused() {
    let s = Interp::new(InterpMethod::CubicSpline, &[0.0, 1.0, 2.0], &[0.0, 1.0, 4.0]).unwrap();
    assert!(matches!(s.eval(-0.001), Err(PetirError::Domain)));
    assert!(matches!(s.eval(2.001), Err(PetirError::Domain)));
    assert!(matches!(s.eval(f64::NAN), Err(PetirError::Domain)));
    assert!(matches!(s.eval_deriv(-1.0), Err(PetirError::Domain)));
    // The endpoints themselves are IN range.
    assert!(s.eval(0.0).is_ok());
    assert!(s.eval(2.0).is_ok());
}

#[test]
fn malformed_input_is_reported() {
    // Mismatched lengths.
    assert!(matches!(
        Interp::new(InterpMethod::Linear, &[0.0, 1.0], &[0.0]),
        Err(PetirError::LengthMismatch { .. })
    ));
    // Too few points for the method.
    assert!(matches!(
        Interp::new(InterpMethod::CubicSpline, &[0.0, 1.0], &[0.0, 1.0]),
        Err(PetirError::Invalid)
    ));
    assert!(matches!(
        Interp::new(InterpMethod::Linear, &[0.0], &[0.0]),
        Err(PetirError::Invalid)
    ));
    // Non-increasing abscissae -- rejected, not silently sorted.
    assert!(matches!(
        Interp::new(InterpMethod::Linear, &[0.0, 2.0, 1.0], &[0.0, 1.0, 2.0]),
        Err(PetirError::Domain)
    ));
    // A repeated abscissa has no single y.
    assert!(matches!(
        Interp::new(InterpMethod::Linear, &[0.0, 1.0, 1.0], &[0.0, 1.0, 2.0]),
        Err(PetirError::Domain)
    ));
}

/// The minimum sizes are part of the contract.
#[test]
fn minimum_sizes_are_what_the_methods_need() {
    assert_eq!(InterpMethod::Linear.min_size(), 2);
    assert_eq!(InterpMethod::CubicSpline.min_size(), 3);
    // Exactly at the minimum must work.
    assert!(Interp::new(InterpMethod::Linear, &[0.0, 1.0], &[0.0, 1.0]).is_ok());
    assert!(Interp::new(InterpMethod::CubicSpline, &[0.0, 1.0, 2.0], &[0.0, 1.0, 4.0]).is_ok());
}

/// The bracket search must land on the right interval everywhere, including
/// exactly on interior knots -- an off-by-one there is the classic
/// interpolation bug and shows up as a small discontinuity.
#[test]
fn the_interpolant_is_continuous_across_every_knot() {
    let x = [0.0, 0.3, 1.1, 2.7, 4.0, 5.5];
    let y = [1.0, -2.0, 0.5, 3.0, -1.0, 2.0];

    for m in [InterpMethod::Linear, InterpMethod::CubicSpline] {
        let s = Interp::new(m, &x, &y).unwrap();
        for &knot in &x[1..x.len() - 1] {
            let eps = 1e-9;
            let left = s.eval(knot - eps).unwrap();
            let right = s.eval(knot + eps).unwrap();
            assert!(
                (left - right).abs() < 1e-6,
                "{m:?}: discontinuity at knot {knot}: {left} vs {right}"
            );
        }
    }
}

/// The spline's DERIVATIVE is continuous too; linear's is not. Both are
/// documented, so both are pinned.
#[test]
fn the_spline_derivative_is_continuous_and_the_linear_one_is_not() {
    let x = [0.0, 1.0, 2.0, 3.0];
    let y = [0.0, 1.0, 4.0, 9.0];

    let spl = Interp::new(InterpMethod::CubicSpline, &x, &y).unwrap();
    let lin = Interp::new(InterpMethod::Linear, &x, &y).unwrap();
    let eps = 1e-9;

    let spl_jump = (spl.eval_deriv(2.0 - eps).unwrap() - spl.eval_deriv(2.0 + eps).unwrap()).abs();
    assert!(spl_jump < 1e-6, "spline slope jumped by {spl_jump} at a knot");

    let lin_jump = (lin.eval_deriv(2.0 - eps).unwrap() - lin.eval_deriv(2.0 + eps).unwrap()).abs();
    assert!(
        lin_jump > 1.0,
        "linear slope should jump at a knot, but changed by only {lin_jump}"
    );
}

#[test]
fn accessors_report_the_construction_data() {
    let x = [0.0, 1.0, 2.0];
    let y = [3.0, 4.0, 5.0];
    let s = Interp::new(InterpMethod::CubicSpline, &x, &y).unwrap();
    assert_eq!(s.method(), InterpMethod::CubicSpline);
    assert_eq!(s.xa(), &x);
    assert_eq!(s.ya(), &y);
    assert_eq!(s.x_min(), 0.0);
    assert_eq!(s.x_max(), 2.0);
}
/// GSL `test_cspline2` abscissae (interpolation/test.c), extracted
/// mechanically from the vendored source rather than retyped.
const CSPLINE2_TEST_X: [f64; 50] = [
    0.00,
    0.02,
    0.04,
    0.06,
    0.08,
    0.10,
    0.12,
    0.14,
    0.16,
    0.18,
    0.20,
    0.22,
    0.24,
    0.26,
    0.28,
    0.30,
    0.32,
    0.34,
    0.36,
    0.38,
    0.40,
    0.42,
    0.44,
    0.46,
    0.48,
    0.50,
    0.52,
    0.54,
    0.56,
    0.58,
    0.60,
    0.62,
    0.64,
    0.66,
    0.68,
    0.70,
    0.72,
    0.74,
    0.76,
    0.78,
    0.80,
    0.82,
    0.84,
    0.86,
    0.88,
    0.90,
    0.92,
    0.94,
    0.96,
    0.98,
];

/// GSL `test_cspline2` expected values -- the Young \& Gregory reference.
const CSPLINE2_TEST_Y: [f64; 50] = [
    1.000000000000000,
    0.997583282975581,
    0.995079933416512,
    0.992403318788142,
    0.989466806555819,
    0.986183764184894,
    0.982467559140716,
    0.978231558888635,
    0.973389130893999,
    0.967853642622158,
    0.961538461538461,
    0.954382579685350,
    0.946427487413627,
    0.937740299651188,
    0.928388131325928,
    0.918438097365742,
    0.907957312698524,
    0.897012892252170,
    0.885671950954575,
    0.874001603733634,
    0.862068965517241,
    0.849933363488199,
    0.837622973848936,
    0.825158185056786,
    0.812559385569085,
    0.799846963843167,
    0.787041308336369,
    0.774162807506023,
    0.761231849809467,
    0.748268823704033,
    0.735294117647059,
    0.722328486073082,
    0.709394147325463,
    0.696513685724764,
    0.683709685591549,
    0.671004731246381,
    0.658421407009825,
    0.645982297202442,
    0.633709986144797,
    0.621627058157454,
    0.609756097560976,
    0.598112015427308,
    0.586679029833925,
    0.575433685609685,
    0.564352527583445,
    0.553412100584061,
    0.542588949440392,
    0.531859618981294,
    0.521200654035625,
    0.510588599432241,
];
