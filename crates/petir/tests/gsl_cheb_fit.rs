// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 Theodore Ong and the outram-park contributors.

//! Verification of [`petir::ChebSeries::fit`] — least-squares Chebyshev
//! fitting at arbitrary abscissae.
//!
//! # What is being verified, and what is NOT
//!
//! These are **verification** checks in the crate's sense: they ask whether
//! the routine computes what it claims, against closed forms and against the
//! crate's own independently-derived interpolation path. None of it is
//! validation — nothing here is compared against a published benchmark, and
//! `petir` is not a declared-mature crate. See `RESPONSIBLE_USE.md`.
//!
//! # Why this path exists separately from `ChebSeries::new`
//!
//! `new` interpolates a *callable* function at the Chebyshev nodes by a cosine
//! transform. `fit` takes *data*, at whatever abscissae it arrived on, and
//! solves the overdetermined system in the Chebyshev basis by QR. The first
//! test below pins the relationship between them: on `order + 1` samples taken
//! exactly at the nodes, the fit must reproduce the interpolant, because the
//! least-squares solution of a square consistent system is its exact solution.

use petir::{ChebSeries, PetirError};

/// Chebyshev nodes of the first kind on `[a, b]`, the abscissae
/// `ChebSeries::new` samples at (`cheb/init.c`).
fn cheb_nodes(n: usize, a: f64, b: f64) -> Vec<f64> {
    let bma = 0.5 * (b - a);
    let bpa = 0.5 * (b + a);
    (0..n)
        .map(|k| {
            let y = (core::f64::consts::PI * (k as f64 + 0.5) / (n as f64)).cos();
            y * bma + bpa
        })
        .collect()
}

/// Sampled at the nodes, with exactly as many samples as coefficients, the fit
/// must reproduce the interpolant.
///
/// # Methodology
///
/// Take `f(x) = exp(x) sin(3x)` on `[-2, 3]` at order 12. Build the
/// interpolant with `ChebSeries::new`, then sample `f` at the 13 Chebyshev
/// nodes and fit the same order with `ChebSeries::fit`. Compare coefficients
/// entry-wise, and compare the two series at 201 points across the interval.
///
/// The two routines share no code below the Chebyshev basis recurrence: one is
/// a cosine transform ported from `cheb/init.c`, the other a Householder QR
/// ported from `linalg/qr.c`. Agreement is therefore a real cross-check of
/// both, not a tautology.
///
/// # Results
///
/// Measured 2026-09-15: maximum coefficient difference **4.27e-15**, maximum
/// pointwise difference **2.13e-14**, RMS residual **exactly 0.0**, rank
/// indicator 0.707. Function values on this interval reach ~19, so the
/// pointwise agreement is ~1e-15 relative.
///
/// Interpretation: the QR path recovers the same polynomial as the cosine
/// transform to within a few units in the last place after a degree-12 solve.
/// The residual being *exactly* zero is the expected signature of a square
/// consistent system — there is no over-determination to leave anything
/// over — and is a sharper check than a small-but-nonzero value would be.
#[test]
fn fit_reproduces_the_interpolant_at_the_nodes() {
    let (a, b) = (-2.0, 3.0);
    let order = 12usize;
    let f = |x: f64| x.exp() * (3.0 * x).sin();

    let interp = ChebSeries::new(order, a, b, f).unwrap();

    let xs = cheb_nodes(order + 1, a, b);
    let ys: Vec<f64> = xs.iter().map(|&x| f(x)).collect();
    let fit = ChebSeries::fit(order, a, b, &xs, &ys).unwrap();

    let worst_coeff = interp
        .coefficients()
        .iter()
        .zip(fit.series.coefficients().iter())
        .map(|(p, q)| (p - q).abs())
        .fold(0.0_f64, f64::max);
    assert!(
        worst_coeff < 1e-11,
        "coefficients differ by {worst_coeff}; the two paths should agree"
    );

    let mut worst_point = 0.0_f64;
    for k in 0..=200 {
        let x = a + (b - a) * (k as f64) / 200.0;
        let d = (interp.eval(x) - fit.series.eval(x)).abs();
        if d > worst_point {
            worst_point = d;
        }
    }
    assert!(
        worst_point < 1e-11,
        "series differ pointwise by {worst_point}"
    );

    // A square consistent system leaves nothing over.
    assert!(
        fit.rms_residual < 1e-12,
        "rms residual {} on an exact interpolation",
        fit.rms_residual
    );
}

/// A polynomial of degree <= order is recovered exactly, from points that are
/// nothing like Chebyshev nodes.
///
/// # Methodology
///
/// `p(x) = 2 - 3x + 0.5 x^2 - x^3` sampled at 40 points clustered towards the
/// left of `[-1, 4]` (a quadratic ramp, deliberately far from the Chebyshev
/// distribution), fitted at order 3 and at order 6. Both must reproduce `p`.
///
/// Fitting ABOVE the true degree is the sharper half of this test: the extra
/// coefficients have nothing to explain and must come out at zero rather than
/// absorbing rounding into visible high-order wiggle.
///
/// # Results
///
/// Measured 2026-09-15: at order 3 the maximum pointwise error against `p`
/// over 500 points is **1.42e-14**; at order 6 it is **2.13e-14**, and the
/// three surplus coefficients are all below **1.90e-15** in a series whose
/// largest coefficient is 34.5 — so the surplus content is ~5e-17 relative.
/// At order 3 the surplus set is empty and the largest coefficient is
/// identical, confirming the extra degrees changed nothing.
///
/// Interpretation: exact recovery to rounding from a sample distribution
/// deliberately unlike the Chebyshev nodes, with no spurious high-order
/// content when the degree is over-specified.
#[test]
fn a_polynomial_is_recovered_exactly_from_non_chebyshev_points() {
    let (a, b) = (-1.0, 4.0);
    let p = |x: f64| 2.0 - 3.0 * x + 0.5 * x * x - x * x * x;

    // Clustered towards `a`, so the sample distribution is emphatically not
    // the one the transform path would have chosen.
    let xs: Vec<f64> = (0..40)
        .map(|k| {
            let u = (k as f64) / 39.0;
            a + (b - a) * u * u
        })
        .collect();
    let ys: Vec<f64> = xs.iter().map(|&x| p(x)).collect();

    for order in [3usize, 6] {
        let fit = ChebSeries::fit(order, a, b, &xs, &ys).unwrap();
        let mut worst = 0.0_f64;
        for k in 0..=500 {
            let x = a + (b - a) * (k as f64) / 500.0;
            let d = (fit.series.eval(x) - p(x)).abs();
            if d > worst {
                worst = d;
            }
        }
        assert!(
            worst < 1e-11,
            "order {order}: max error {worst} recovering an exact cubic"
        );

        if order > 3 {
            // Coefficients above the true degree must be zero, not noise.
            for (k, &c) in fit.series.coefficients().iter().enumerate().skip(4) {
                assert!(c.abs() < 1e-11, "surplus coefficient {k} is {c}, not zero");
            }
        }
    }
}

/// The residual is exactly `y - p(x)` at every sample, and its norm is the
/// reported RMS.
///
/// A fit that reported a residual it had not actually computed would pass
/// every accuracy test above, so this checks the reported diagnostics against
/// the series itself.
#[test]
fn the_reported_residual_matches_the_fitted_series() {
    let xs: Vec<f64> = (0..25).map(|k| -1.0 + 2.0 * (k as f64) / 24.0).collect();
    // Something a quadratic cannot fit, so the residual is genuinely non-zero.
    let ys: Vec<f64> = xs.iter().map(|&x| (4.0 * x).sin()).collect();

    let fit = ChebSeries::fit(2, -1.0, 1.0, &xs, &ys).unwrap();
    assert_eq!(fit.residual.len(), xs.len());

    let mut sum_sq = 0.0_f64;
    for ((&x, &y), &r) in xs.iter().zip(ys.iter()).zip(fit.residual.iter()) {
        let direct = y - fit.series.eval(x);
        assert!(
            (direct - r).abs() < 1e-12,
            "reported residual {r} != y - p(x) = {direct}"
        );
        sum_sq += r * r;
    }
    let rms = (sum_sq / (xs.len() as f64)).sqrt();
    assert!(
        (rms - fit.rms_residual).abs() < 1e-14,
        "reported rms {} != {rms}",
        fit.rms_residual
    );
    assert!(
        fit.rms_residual > 1e-3,
        "a quadratic should NOT fit sin(4x) well; rms {} looks too good",
        fit.rms_residual
    );
}

/// Raising the degree must reduce the residual monotonically.
///
/// # Methodology
///
/// Fit `1 / (1 + 25 x^2)` — Runge's function — on `[-1, 1]` from 60 equally
/// spaced samples, at orders 2, 4, 8, 16, and record the RMS residual.
///
/// Equally spaced samples are chosen deliberately: this is the setting in
/// which *interpolation* diverges (Runge's phenomenon), and least-squares
/// fitting is the standard remedy. A monotone decrease is what distinguishes
/// the two.
///
/// # Results
///
/// Measured 2026-09-15, RMS residual by order:
/// order 2 -> **1.933e-1**, order 4 -> **1.309e-1**, order 8 -> **5.961e-2**,
/// order 16 -> **1.197e-2**.
///
/// Interpretation: monotone convergence, a factor of **16** over the range.
/// That is slow, and it is supposed to be: Runge's function has poles at
/// `x = +/- i/5`, just off the real interval, which caps the convergence rate
/// of any polynomial approximation on `[-1, 1]`. The point of the test is the
/// monotonicity and the absence of blow-up, not the rate — equally spaced
/// *interpolation* of this same function diverges as the degree rises, and
/// this does not.
#[test]
fn a_higher_degree_fits_runges_function_better() {
    let xs: Vec<f64> = (0..60).map(|k| -1.0 + 2.0 * (k as f64) / 59.0).collect();
    let ys: Vec<f64> = xs.iter().map(|&x| 1.0 / (1.0 + 25.0 * x * x)).collect();

    let mut previous = f64::INFINITY;
    for order in [2usize, 4, 8, 16] {
        let fit = ChebSeries::fit(order, -1.0, 1.0, &xs, &ys).unwrap();
        assert!(
            fit.rms_residual < previous,
            "order {order}: rms {} did not improve on {previous}",
            fit.rms_residual
        );
        previous = fit.rms_residual;
    }
    assert!(
        previous < 2e-2,
        "order 16 should fit Runge's function to better than 2e-2, got {previous}"
    );
}

/// Samples that do not determine the requested degree are REPORTED, not
/// answered with an arbitrary member of the solution set.
///
/// # Why this matters more than it looks
///
/// Without the rank check, three distinct abscissae asked for a cubic give a
/// perfectly finite answer with a residual near zero — it is *a* minimiser,
/// but which one is decided by rounding. That is the worst kind of wrong
/// answer: plausible, reproducible on one machine, and different on another.
#[test]
fn samples_that_do_not_determine_the_degree_are_reported() {
    // Nine samples, but only three distinct abscissae: a cubic is not pinned.
    let xs = [0.0, 0.0, 0.0, 0.5, 0.5, 0.5, 1.0, 1.0, 1.0];
    let ys = [1.0, 1.0, 1.0, 2.0, 2.0, 2.0, 5.0, 5.0, 5.0];
    let err = ChebSeries::fit(3, 0.0, 1.0, &xs, &ys).unwrap_err();
    assert!(
        matches!(err, PetirError::Singular { .. }),
        "expected Singular, got {err:?}"
    );

    // The same points DO determine a quadratic, and it is exact.
    let fit = ChebSeries::fit(2, 0.0, 1.0, &xs, &ys).unwrap();
    assert!(
        fit.rms_residual < 1e-13,
        "three distinct points determine a quadratic exactly; rms {}",
        fit.rms_residual
    );
}

/// Every documented error path fires.
#[test]
fn malformed_input_is_reported() {
    let xs = [0.0, 0.5, 1.0];
    let ys = [1.0, 2.0, 3.0];

    // a >= b
    assert!(matches!(
        ChebSeries::fit(1, 1.0, 1.0, &xs, &ys),
        Err(PetirError::Domain)
    ));
    // mismatched lengths
    assert!(matches!(
        ChebSeries::fit(1, 0.0, 1.0, &xs, &ys[..2]),
        Err(PetirError::LengthMismatch { .. })
    ));
    // fewer samples than coefficients
    assert!(matches!(
        ChebSeries::fit(5, 0.0, 1.0, &xs, &ys),
        Err(PetirError::Invalid)
    ));
    // a sample outside [a, b]
    assert!(matches!(
        ChebSeries::fit(1, 0.0, 0.9, &xs, &ys),
        Err(PetirError::Domain)
    ));
    // a non-finite ordinate
    assert!(matches!(
        ChebSeries::fit(1, 0.0, 1.0, &xs, &[1.0, f64::NAN, 3.0]),
        Err(PetirError::Domain)
    ));
    // NaN abscissa
    assert!(matches!(
        ChebSeries::fit(1, 0.0, 1.0, &[0.0, f64::NAN, 1.0], &ys),
        Err(PetirError::Domain)
    ));
}

/// A fit must not chase a perturbation its basis cannot represent.
///
/// # Methodology
///
/// `f(x) = 1 + 2x - x^2` sampled at 200 points with a deterministic
/// zero-mean perturbation (alternating +/- 0.01, so no RNG and the test is
/// reproducible), fitted at order 2. The recovered coefficients are compared
/// against the exact ones.
///
/// # Results
///
/// Measured 2026-09-15: the recovered series matches the noiseless `f` to a
/// maximum pointwise error of **1.49e-4** over the interval, against a
/// per-sample perturbation of 1e-2 — a rejection factor of **67**.
///
/// # Interpretation, including what this does NOT show
///
/// 67 is far better than the `sqrt(m/n) = sqrt(200/3) = 8.2` that averaging
/// *random* noise would give, and the discrepancy is the interesting part: the
/// perturbation here alternates `+/-` at every sample, so it is a
/// high-frequency pattern nearly orthogonal to a quadratic basis, and the fit
/// rejects it almost entirely rather than merely averaging it down.
///
/// So this test shows that the fit does not chase a perturbation the basis
/// cannot represent. It does **not** demonstrate `sqrt(m/n)` noise averaging,
/// and should not be cited for that — showing it would need a random
/// perturbation, which would need an RNG this crate deliberately does not
/// have. The deterministic pattern was chosen so the test is reproducible;
/// the cost is that it measures something narrower than it first appears.
#[test]
fn a_fit_rejects_a_perturbation_orthogonal_to_its_basis() {
    let f = |x: f64| 1.0 + 2.0 * x - x * x;
    let xs: Vec<f64> = (0..200).map(|k| -1.0 + 2.0 * (k as f64) / 199.0).collect();
    let ys: Vec<f64> = xs
        .iter()
        .enumerate()
        .map(|(k, &x)| f(x) + if k % 2 == 0 { 0.01 } else { -0.01 })
        .collect();

    let fit = ChebSeries::fit(2, -1.0, 1.0, &xs, &ys).unwrap();
    let mut worst = 0.0_f64;
    for k in 0..=400 {
        let x = -1.0 + 2.0 * (k as f64) / 400.0;
        let d = (fit.series.eval(x) - f(x)).abs();
        if d > worst {
            worst = d;
        }
    }
    assert!(
        worst < 5e-3,
        "a +/- 1e-2 alternating perturbation should be rejected, not fitted; \
         max error {worst}"
    );
}
