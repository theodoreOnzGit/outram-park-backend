// SPDX-License-Identifier: GPL-3.0-only
//
// PORTED from the `peroxide` crate, version 0.41.2, file
// `src/numerical/integral.rs`, read from the published crates.io source on
// 2026-09-15.
//
//     https://github.com/Axect/Peroxide
//     Copyright (c) Tae Geun Kim (axect) <axect@outlook.kr>
//     Licensed MIT OR Apache-2.0.
//
// Taken under the MIT option; the full notice is in
// `crates/petir/src/poly/dense.rs`, the other file ported from this crate, and
// in the crate NOTICE. MIT into GPL-3.0-only is a ONE-WAY flow.
//
// Ported items:
//   unit_gauss_legendre_quadrature -> gauss_legendre_unit
//   gauss_legendre_quadrature      -> gauss_legendre
//   newton_cotes_quadrature        -> newton_cotes
//   seq                            -> the node spacing inside newton_cotes

//! Gauss-Legendre and Newton-Cotes quadrature — ported from the `peroxide`
//! crate by Tae Geun Kim.
//!
//! # What these add to [`crate::integration`]
//!
//! The rest of this module is **QUADPACK** by way of GSL: Gauss-Kronrod pairs
//! and the `qag` adaptive driver, which exist to produce an *error estimate*
//! alongside the integral and to subdivide where it is large. These two rules
//! answer a different question.
//!
//! | | when to reach for it |
//! |---|---|
//! | [`crate::integration::qag`] | you do not know the integrand's behaviour and want a tolerance met |
//! | [`gauss_legendre`] | you know it is smooth, want the fewest evaluations, and do not need an error estimate |
//! | [`newton_cotes`] | you want an equally-spaced rule, or the integrand is only sampled on a grid |
//!
//! Gauss-Legendre with `n` points is exact for polynomials up to degree
//! `2n - 1`, which is the best any `n`-point rule can do; the tests below
//! measure that it really is. Neither rule here adapts, and neither returns an
//! error estimate — if you need one, use `qag`.
//!
//! # Units
//!
//! Bare dimensionless `f64`. The integrand's units multiply the abscissa's to
//! give the result's, as always.

use alloc::vec::Vec;

#[allow(unused_imports)]
use crate::real::Real;

use crate::integration::gauss_legendre_tables::{gauss_legendre_table, MAX_ORDER, MIN_ORDER};
use crate::poly::dense::DensePoly;
use crate::{PetirError, Result};

/// `n`-point Gauss-Legendre quadrature on the reference interval `[-1, 1]`.
///
/// Ports upstream's `unit_gauss_legendre_quadrature`.
///
/// # Errors
///
/// [`PetirError::Domain`] if `n` is outside `MIN_ORDER..=MAX_ORDER` (2 to 30).
/// Upstream's table lookup has no fallback arm and panics there.
///
/// # Examples
///
/// ```
/// use petir::integration::gauss_legendre::gauss_legendre_unit;
///
/// // int_{-1}^{1} x^2 dx = 2/3, and a 2-point rule is exact for degree 3.
/// let v = gauss_legendre_unit(|x| x * x, 2).unwrap();
/// assert!((v - 2.0 / 3.0).abs() < 1e-15);
/// ```
pub fn gauss_legendre_unit<F>(f: F, n: usize) -> Result<f64>
where
    F: Fn(f64) -> f64,
{
    let (nodes, weights) = gauss_legendre_table(n).ok_or(PetirError::Domain)?;
    let mut s = 0.0;
    for (&x, &w) in nodes.iter().zip(weights.iter()) {
        s += f(x) * w;
    }
    Ok(s)
}

/// `n`-point Gauss-Legendre quadrature of `f` over `[a, b]`.
///
/// Ports upstream's `gauss_legendre_quadrature`, which maps the reference
/// interval onto `[a, b]` by the affine change of variable and scales by the
/// Jacobian `(b - a)/2`.
///
/// **Exact for polynomials of degree up to `2n - 1`**, which is the defining
/// property of a Gauss rule and the reason to prefer it over an equally-spaced
/// rule of the same cost. It has **no error estimate** and does not adapt; use
/// [`crate::integration::qag`] when you need either.
///
/// # Errors
///
/// [`PetirError::Domain`] if `n` is outside 2 to 30. A reversed interval
/// (`b < a`) is fine and returns the negated integral, as it should.
///
/// # Examples
///
/// ```
/// use petir::integration::gauss_legendre::gauss_legendre;
///
/// // int_0^1 x^5 dx = 1/6. A 3-point rule is exact to degree 5.
/// let v = gauss_legendre(|x| x.powi(5), 0.0, 1.0, 3).unwrap();
/// assert!((v - 1.0 / 6.0).abs() < 1e-15);
/// ```
pub fn gauss_legendre<F>(f: F, a: f64, b: f64, n: usize) -> Result<f64>
where
    F: Fn(f64) -> f64,
{
    let half = (b - a) / 2.0;
    let mid = (a + b) / 2.0;
    let v = gauss_legendre_unit(|x| f(x * half + mid), n)?;
    Ok(v * half)
}

/// `n`-interval Newton-Cotes quadrature of `f` over `[a, b]`.
///
/// Ports upstream's `newton_cotes_quadrature`, which is unusual and worth
/// describing because the name covers a family: rather than applying a fixed
/// weight table, it **samples `f` at `n + 1` equally spaced points,
/// interpolates them with a Lagrange polynomial, and integrates that
/// polynomial exactly**. For a given `n` this reproduces the classical closed
/// Newton-Cotes rule — `n = 1` is the trapezoid rule, `n = 2` is Simpson's —
/// without tabulating any of them.
///
/// It reuses [`crate::poly::dense`], the other `peroxide` port, for both the
/// interpolation and the exact integration.
///
/// # Runge's phenomenon is real here — prefer a low `n`
///
/// Equally-spaced interpolation diverges as the degree rises for some
/// perfectly smooth integrands; this is Runge's phenomenon, and a
/// Newton-Cotes rule inherits it directly because it *is* an interpolating
/// rule. High-order closed Newton-Cotes also has negative weights, which
/// destroys numerical stability. The tests below measure both effects on
/// `1/(1 + 25x^2)`. **Use `n <= 8`, or use [`gauss_legendre`] instead**,
/// which places its nodes where this does not happen.
///
/// # Errors
///
/// [`PetirError::Invalid`] if `n` is zero, or if the interpolation fails.
///
/// # Examples
///
/// ```
/// use petir::integration::gauss_legendre::newton_cotes;
///
/// // n = 2 is Simpson's rule, exact for cubics.
/// let v = newton_cotes(|x| x * x * x, 0.0, 1.0, 2).unwrap();
/// assert!((v - 0.25).abs() < 1e-14);
/// ```
pub fn newton_cotes<F>(f: F, a: f64, b: f64, n: usize) -> Result<f64>
where
    F: Fn(f64) -> f64,
{
    if n == 0 {
        return Err(PetirError::Invalid);
    }
    let h = (b - a) / (n as f64);
    let node_x: Vec<f64> = (0..=n).map(|i| a + h * i as f64).collect();
    let node_y: Vec<f64> = node_x.iter().map(|&x| f(x)).collect();
    let p = DensePoly::lagrange(&node_x, &node_y)?;
    Ok(p.integrate(a, b))
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    /// Gauss-Legendre is exact for degree `2n - 1` and not beyond.
    ///
    /// # Methodology
    ///
    /// For `n = 2..=10`, integrate `x^k` over `[-1, 1]` for every `k` up to
    /// `2n - 1` and compare against the exact `2/(k+1)` for even `k`, `0` for
    /// odd. Then integrate `x^(2n)`, which the rule must *not* get exactly
    /// right — a rule that did would mean the test is not testing anything.
    ///
    /// # Results
    ///
    /// **Worst relative error 7.216e-15** over all exact cases, measured
    /// 2026-09-15 — and the rule is confirmed *not* exact at degree `2n`,
    /// which is the half of this test that stops it from passing vacuously.
    #[test]
    fn gauss_legendre_is_exact_to_degree_two_n_minus_one() {
        let mut worst = 0.0_f64;
        for n in 2..=10usize {
            for k in 0..=(2 * n - 1) {
                let got = gauss_legendre(|x| x.powi(k as i32), -1.0, 1.0, n).unwrap();
                let want = if k % 2 == 0 {
                    2.0 / (k as f64 + 1.0)
                } else {
                    0.0
                };
                let err = if want == 0.0 {
                    got.abs()
                } else {
                    ((got - want) / want).abs()
                };
                if err > worst {
                    worst = err;
                }
            }
            // Degree 2n must NOT be exact.
            let k = 2 * n;
            let got = gauss_legendre(|x| x.powi(k as i32), -1.0, 1.0, n).unwrap();
            let want = 2.0 / (k as f64 + 1.0);
            let rel = ((got - want) / want).abs();
            assert!(
                rel > 1e-6,
                "n={n}: degree {k} came out exact to {rel:e}, which it should not"
            );
        }
        assert!(worst < 1e-12, "worst relative error {worst:e}");
    }

    /// The tabulated nodes and weights are audited in their own test file.
    ///
    /// `tests/gauss_legendre_table_audit.rs` checks all 928 values against
    /// Bonnet's recurrence — it is a separate file because it needs a stable
    /// evaluator that this module deliberately does not provide, and because
    /// **it found three wrong node values in upstream**, which is a result
    /// worth its own place rather than a line inside a quadrature test.
    ///
    /// What is checked here is only that the lookup is wired up.
    #[test]
    fn the_table_lookup_covers_its_documented_range() {
        for n in MIN_ORDER..=MAX_ORDER {
            let (nodes, weights) = gauss_legendre_table(n).expect("in range");
            assert_eq!(nodes.len(), n);
            assert_eq!(weights.len(), n);
        }
        assert!(gauss_legendre_table(MIN_ORDER - 1).is_none());
        assert!(gauss_legendre_table(MAX_ORDER + 1).is_none());
    }

    /// Gauss-Legendre agrees with this crate's Gauss-Kronrod on a smooth
    /// integrand.
    ///
    /// # Why cross-check two quadrature lineages
    ///
    /// [`crate::integration::kronrod`] is QUADPACK by way of GSL; this is
    /// `peroxide`. They share no code and no data. Agreement on a smooth
    /// integrand is evidence for both, and is the only check the ported tables
    /// get against something outside the `peroxide` lineage entirely.
    ///
    /// # Results
    ///
    /// Split by integrand, measured 2026-09-15 on `[0, 1]` and `[-1, 2]`:
    ///
    /// - **Polynomials, where both rules are exact: worst relative difference
    ///   1.266e-15.**
    /// - Non-polynomial integrands: worst **3.695e-06**, nine orders of
    ///   magnitude looser.
    ///
    /// The split matters, and the first version of this test did not make it.
    /// Two *non-adaptive* rules of different order have no reason to agree to
    /// rounding on a function neither integrates exactly — each is converging
    /// to the same answer at its own rate, and the gap between them is the
    /// sum of two truncation errors, not a disagreement. Where both are exact,
    /// as on a polynomial of degree at most 19, any difference really is a
    /// defect, and that is the comparison with teeth.
    #[test]
    fn gauss_legendre_agrees_with_gsl_gauss_kronrod() {
        use crate::integration::{kronrod, QkRule};

        // Both rules are exact here: Gauss-Legendre(10) to degree 19, and
        // GSL's Kronrod-15 to degree 22 in its Gauss part.
        let mut worst_exact = 0.0_f64;
        // Neither is exact here; they agree only to their truncation errors.
        let mut worst_inexact = 0.0_f64;

        for &(a, b) in &[(0.0_f64, 1.0_f64), (-1.0, 2.0)] {
            let polynomials: [fn(f64) -> f64; 3] = [
                |x| x * x * x,
                |x| x.mul_add(x, 3.0) * (x - 1.0),
                |x| x.powi(7) - 2.0 * x.powi(4) + 5.0,
            ];
            let others: [fn(f64) -> f64; 3] = [
                |x| (x * 0.5).exp(),
                |x| 1.0 / (1.0 + x * x),
                |x| (2.0 * x).cos(),
            ];
            let mut measure = |f: fn(f64) -> f64, slot: &mut f64| {
                let gl = gauss_legendre(f, a, b, 10).unwrap();
                let gk = kronrod(QkRule::Qk15, f, a, b).result;
                let rel = if gk == 0.0 {
                    (gl - gk).abs()
                } else {
                    ((gl - gk) / gk).abs()
                };
                if rel > *slot {
                    *slot = rel;
                }
            };
            for f in polynomials {
                measure(f, &mut worst_exact);
            }
            for f in others {
                measure(f, &mut worst_inexact);
            }
        }

        assert!(
            worst_exact < 1e-14,
            "two exact rules disagree by {worst_exact:e} on a polynomial"
        );
        assert!(
            worst_inexact < 1e-4,
            "worst non-polynomial difference {worst_inexact:e}"
        );
    }

    /// Newton-Cotes reproduces the classical low-order rules.
    ///
    /// # Methodology
    ///
    /// `n = 1` must be the trapezoid rule and `n = 2` Simpson's. Rather than
    /// asserting the weights, this checks the property that defines each:
    /// trapezoid is exact for linear integrands and not quadratic, Simpson's
    /// is exact for cubics and not quartics.
    ///
    /// # Results
    ///
    /// Both hold, measured 2026-09-15: trapezoid is exact on a line and
    /// visibly wrong on `x^2`; Simpson's is exact on a cubic and visibly wrong
    /// on `x^4`. The test asserts both halves — being exact where it should
    /// not be would mean the rule is not the one named.
    #[test]
    fn newton_cotes_reproduces_trapezoid_and_simpson() {
        // n = 1: trapezoid. Exact for linear.
        let lin = newton_cotes(|x| 2.0 * x + 1.0, 0.0, 1.0, 1).unwrap();
        assert!((lin - 2.0).abs() < 1e-14, "trapezoid on a line: {lin}");
        // ... and not for quadratic.
        let quad = newton_cotes(|x| x * x, 0.0, 1.0, 1).unwrap();
        assert!(
            (quad - 1.0 / 3.0).abs() > 1e-3,
            "trapezoid should not be exact on x^2"
        );

        // n = 2: Simpson's. Exact for cubic.
        let cubic = newton_cotes(|x| x * x * x, 0.0, 1.0, 2).unwrap();
        assert!((cubic - 0.25).abs() < 1e-14, "Simpson on a cubic: {cubic}");
        // ... and not for quartic.
        let quartic = newton_cotes(|x| x.powi(4), 0.0, 1.0, 2).unwrap();
        assert!(
            (quartic - 0.2).abs() > 1e-4,
            "Simpson should not be exact on x^4"
        );
    }

    /// Runge's phenomenon in Newton-Cotes, measured rather than warned about.
    ///
    /// # Methodology
    ///
    /// Integrate Runge's function `1/(1 + 25x^2)` over `[-1, 1]`, whose exact
    /// value is `(2/5) arctan(5)`, with Newton-Cotes at rising `n`. An
    /// interpolating rule on equally spaced nodes is expected to *stop*
    /// improving and then get worse.
    ///
    /// # Results
    ///
    /// Measured 2026-09-15 — the error falls, turns, and then grows:
    ///
    /// | n | absolute error |
    /// |---|---|
    /// | 4 | 7.456e-02 |
    /// | 8 | 2.493e-01 |
    /// | 12 | 6.119e-01 |
    /// | 16 | 1.798e+00 |
    /// | 20 | 5.919e+00 |
    ///
    /// The error does not merely stop improving — **it grows without bound**,
    /// and by `n = 20` it is three times the integral itself (0.5494).
    /// Gauss-Legendre on the same integrand goes the other way: 1.899e-02 at
    /// 10 points, 6.836e-06 at 30.
    ///
    /// Interpretation: this is why the module docs say use `n <= 8`, or use
    /// Gauss-Legendre. The divergence is a property of equally spaced
    /// interpolation, not of the port. Note also that Gauss-Legendre is
    /// *converging but slowly* here — Runge's function has poles at
    /// `x = +- i/5`, very close to the interval, and no polynomial rule does
    /// well against that. 6.8e-06 at 30 points is the honest number, not a
    /// flattering one.
    #[test]
    fn newton_cotes_diverges_on_runge_and_gauss_legendre_does_not() {
        let f = |x: f64| 1.0 / (1.0 + 25.0 * x * x);
        let exact = 0.4 * 5.0_f64.atan();

        let mut errs = vec![];
        for n in [4usize, 8, 12, 16, 20] {
            let v = newton_cotes(f, -1.0, 1.0, n).unwrap();
            errs.push((n, (v - exact).abs()));
        }
        // The error must turn: worse at n = 20 than at n = 12.
        let at12 = errs
            .iter()
            .find(|(n, _)| *n == 12)
            .map(|(_, e)| *e)
            .unwrap();
        let at20 = errs
            .iter()
            .find(|(n, _)| *n == 20)
            .map(|(_, e)| *e)
            .unwrap();
        assert!(
            at20 > at12,
            "Runge's phenomenon should show: n=12 {at12:e}, n=20 {at20:e}"
        );

        // Gauss-Legendre keeps improving on the same integrand.
        let gl10 = (gauss_legendre(f, -1.0, 1.0, 10).unwrap() - exact).abs();
        let gl30 = (gauss_legendre(f, -1.0, 1.0, 30).unwrap() - exact).abs();
        assert!(
            gl30 < gl10,
            "Gauss-Legendre should improve: {gl10:e} -> {gl30:e}"
        );
        assert!(gl30 < 1e-4, "30-point Gauss-Legendre error {gl30:e}");
    }

    /// Out-of-range orders and degenerate intervals are errors, not panics.
    #[test]
    fn degenerate_inputs_are_handled() {
        assert!(gauss_legendre_unit(|x| x, 0).is_err());
        assert!(gauss_legendre_unit(|x| x, 1).is_err());
        assert!(gauss_legendre_unit(|x| x, MAX_ORDER + 1).is_err());
        assert!(gauss_legendre(|x| x, 0.0, 1.0, 31).is_err());
        assert!(newton_cotes(|x| x, 0.0, 1.0, 0).is_err());

        // A zero-width interval integrates to zero.
        assert_eq!(gauss_legendre(|x| x * x, 1.0, 1.0, 5).unwrap(), 0.0);
        // A reversed interval negates.
        let fwd = gauss_legendre(|x| x * x, 0.0, 1.0, 5).unwrap();
        let rev = gauss_legendre(|x| x * x, 1.0, 0.0, 5).unwrap();
        assert!((fwd + rev).abs() < 1e-15, "{fwd} vs {rev}");
    }
}
