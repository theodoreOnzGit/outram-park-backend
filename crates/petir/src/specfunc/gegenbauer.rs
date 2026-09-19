// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of PETIR, a component of OUTRAM PARK.
//
// PETIR is free software: you can redistribute it and/or modify it under the
// terms of the GNU General Public License as published by the Free Software
// Foundation, version 3 of the License.
//
// PETIR is distributed in the hope that it will be useful, but WITHOUT ANY
// WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
// FOR A PARTICULAR PURPOSE.  See the GNU General Public License for more
// details.
//
// You should have received a copy of the GNU General Public License along
// with PETIR.  If not, see <https://www.gnu.org/licenses/>.
//
// PORTED from the GNU Scientific Library (GSL) 2.8, specfunc/gegenbauer.c
// <https://www.gnu.org/software/gsl/>
// Copyright (C) 1996-2000 Gerard Jungman
// GSL is free software under the GNU General Public License, version 3 or
// later.
//
// NO CHEBYSHEV TABLES. Three closed forms and a three-term recurrence.

//! The Gegenbauer (ultraspherical) polynomials `C_n^lambda(x)`.
//!
//! # What these are
//!
//! The orthogonal polynomials on `[-1, 1]` with weight
//! `(1 - x^2)^{lambda - 1/2}`, and the family that contains most of the
//! other classical ones as special cases:
//!
//! | `lambda` | reduces to |
//! |---|---|
//! | `1/2` | the Legendre polynomials `P_n(x)` |
//! | `1` | the Chebyshev polynomials of the second kind `U_n(x)` |
//! | `0` | `2 T_n(x) / n`, first kind, up to normalisation |
//!
//! All three of those are checked in the tests, and the first is checked
//! against [`crate::poly::dense::legendre`] — which builds `P_n` as an
//! explicit coefficient vector by polynomial arithmetic, not by a
//! recurrence in the value — so it is a cross-check between two modules by
//! two different routes, and not a restatement.
//!
//! # How they are computed
//!
//! By the standard three-term recurrence in `n`, upstream's own:
//!
//! ```text
//!     k C_k = 2 (k + lambda - 1) x C_{k-1} - (k + 2 lambda - 2) C_{k-2}
//! ```
//!
//! seeded from closed forms at `n = 2` and `n = 3`. `n = 0 ..= 3` are closed
//! forms in their own right. Upstream carries a separate branch for
//! `lambda = 0` on `[-1, 1]`, where the recurrence's `1/k` normalisation
//! degenerates and `2 cos(n arccos x) / n` is used instead; that branch is
//! carried.
//!
//! **A recurrence is not a series**: there is no truncation and no
//! convergence question, only accumulated rounding, which grows like `n`.
//! That is why the error estimate upstream attaches is proportional to `n`
//! and why the tests below measure against `n` rather than against `x`.
//!
//! # Argument range
//!
//! `n` is a non-negative order, `lambda > -1/2` the ultraspherical
//! parameter, and `x` the argument — dimensionless `f64` throughout.
//! Outside `lambda > -1/2` upstream signals `DOMAIN_ERROR` and this returns
//! `NaN`.
//!
//! `x` is **not** restricted to `[-1, 1]`. The polynomials are defined for
//! every real `x`; they merely stop being bounded outside it, and grow like
//! `x^n`. The `lambda = 0` special case *is* restricted, because
//! `arccos` is, and upstream falls back to the recurrence outside — which is
//! carried.
//!
//! # Accuracy
//!
//! Measured against the three closed-form special cases above, against the
//! **generating function** `(1 - 2xt + t^2)^{-lambda}` recovered by a Cauchy
//! integral, and against [`crate::poly::dense::legendre`].
//!
//! One result is worth reading before using either module: at `|x| = 1` the
//! recurrence here is **bit-exact** where the explicit-coefficient form in
//! `poly::dense` is off by `4.470e-07` at `n = 29`. `P_n` at `+/-1` is
//! `(+/-1)^n` exactly, so that is not a matter of interpretation. See
//! `lambda_one_half_gives_the_legendre_polynomials`.

// Under a std-linked build (`cargo test`) f64's inherent acos/cos shadow
// these trait methods, leaving the import formally unused. See crate::real.
#[allow(unused_imports)]
use crate::real::Real;

/// `C_1^lambda(x) = 2 lambda x`, GSL's `gsl_sf_gegenpoly_1_e`.
///
/// At `lambda = 0` upstream returns `2x` rather than `0`, which is the
/// limit of `C_1^lambda / lambda` and not of `C_1^lambda` itself. That is
/// deliberate upstream and is carried — see
/// `the_lambda_zero_closed_forms_are_upstreams_normalisation`.
pub fn gegenpoly_1(lambda: f64, x: f64) -> f64 {
    if lambda == 0.0 {
        2.0 * x
    } else {
        2.0 * lambda * x
    }
}

/// `C_2^lambda(x)`, GSL's `gsl_sf_gegenpoly_2_e`.
pub fn gegenpoly_2(lambda: f64, x: f64) -> f64 {
    if lambda == 0.0 {
        -1.0 + 2.0 * x * x
    } else {
        lambda * (-1.0 + 2.0 * (1.0 + lambda) * x * x)
    }
}

/// `C_3^lambda(x)`, GSL's `gsl_sf_gegenpoly_3_e`.
pub fn gegenpoly_3(lambda: f64, x: f64) -> f64 {
    if lambda == 0.0 {
        x * (-2.0 + 4.0 / 3.0 * x * x)
    } else {
        let c = 4.0 + lambda * (6.0 + 2.0 * lambda);
        2.0 * lambda * x * (-1.0 - lambda + c * x * x / 3.0)
    }
}

/// `C_n^lambda(x)` for any order, GSL's `gsl_sf_gegenpoly_n_e`
/// (`specfunc/gegenbauer.c:88`).
///
/// `lambda > -1/2`; `NaN` otherwise, which is upstream's `DOMAIN_ERROR`.
/// `n` is a `u32` here where upstream takes a signed `int` and rejects
/// negatives — the type makes that branch unreachable rather than checked.
///
/// # Examples
///
/// ```
/// use petir::specfunc::gegenbauer::gegenpoly_n;
/// // lambda = 1/2 gives the Legendre polynomials.
/// let p4 = gegenpoly_n(4, 0.5, 0.3);
/// assert!((p4 - petir::poly::dense::legendre(4).eval(0.3)).abs() < 1e-14);
/// ```
pub fn gegenpoly_n(n: u32, lambda: f64, x: f64) -> f64 {
    if lambda.is_nan() || x.is_nan() || lambda <= -0.5 {
        return f64::NAN;
    }
    match n {
        0 => return 1.0,
        1 => return gegenpoly_1(lambda, x),
        2 => return gegenpoly_2(lambda, x),
        3 => return gegenpoly_3(lambda, x),
        _ => {}
    }
    // Upstream's Chebyshev branch: at lambda = 0 the recurrence's 1/k
    // normalisation degenerates, and C_n^0 is 2 T_n(x)/n.
    if lambda == 0.0 && (-1.0..=1.0).contains(&x) {
        let z = n as f64 * x.acos();
        return 2.0 * z.cos() / n as f64;
    }
    let mut gkm2 = gegenpoly_2(lambda, x);
    let mut gkm1 = gegenpoly_3(lambda, x);
    let mut gk = 0.0;
    for k in 4..=n {
        let kf = k as f64;
        gk = (2.0 * (kf + lambda - 1.0) * x * gkm1 - (kf + 2.0 * lambda - 2.0) * gkm2) / kf;
        gkm2 = gkm1;
        gkm1 = gk;
    }
    gk
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **`lambda = 1/2` gives the Legendre polynomials** — and the
    /// comparison turned out to measure the *reference*, not this module.
    ///
    /// [`crate::poly::dense::legendre`] builds `P_n` as an explicit
    /// coefficient vector through polynomial arithmetic and evaluates it by
    /// Horner. That shares nothing with the value-recurrence here, which is
    /// what makes it a real cross-check. What it also shows is that the two
    /// routes are not equally good, and the gap between them grows
    /// exponentially in `n` — measured 2026-09-20, worst over 37 arguments
    /// in `[-0.9, 0.9]`:
    ///
    /// | `n` | gap | `P_n(-1)` from the coefficients |
    /// |---|---|---|
    /// | 8 | 2.220e-15 | exact |
    /// | 12 | 3.453e-14 | exact |
    /// | 16 | 1.509e-12 | exact |
    /// | 20 | 2.534e-11 | exact |
    /// | 24 | 2.767e-10 | exact |
    /// | 29 | 8.010e-08 | **4.470e-07 off** |
    /// | 32 | 2.270e-07 | **5.444e-07 off** |
    ///
    /// About **one digit lost every three orders**. `P_n` at `+/-1` is
    /// `(+/-1)^n` exactly, so the last column settles which side is wrong:
    /// `P_29`'s coefficients reach `~1e7` and alternate in sign, and summing
    /// them cancels. The recurrence never forms a large intermediate and is
    /// **bit-exact at both endpoints for every `n` tried**.
    ///
    /// This is the standard reason value-recurrences are preferred to
    /// coefficient representations for orthogonal polynomials. It is
    /// measured here rather than asserted, and it is a property of the
    /// reference — so the cross-check is run only where the reference is
    /// sound, and the endpoints are checked against the exact value
    /// instead.
    #[test]
    fn lambda_one_half_gives_the_legendre_polynomials() {
        let (mut worst, mut at) = (0.0_f64, (0u32, 0.0_f64));
        for n in 0..=20u32 {
            for i in 0..=36 {
                let x = -0.9 + 1.8 * i as f64 / 36.0;
                let a = gegenpoly_n(n, 0.5, x);
                let b = crate::poly::dense::legendre(n as usize).eval(x);
                let e = (a - b).abs();
                if e > worst {
                    worst = e;
                    at = (n, x);
                }
            }
        }
        assert!(
            worst < 1e-10,
            "C_n^{{1/2}} against Legendre P_n for n <= 20: {worst:e} at \
             n = {}, x = {}",
            at.0,
            at.1
        );

        // And the growth is the REFERENCE's, not this module's: the
        // recurrence is bit-exact at the endpoints where the exact value is
        // known, for orders far past where the coefficient form has lost
        // seven digits.
        for n in 0..=40u32 {
            let even = n % 2 == 0;
            assert_eq!(gegenpoly_n(n, 0.5, 1.0), 1.0, "P_{n}(1) must be exactly 1");
            assert_eq!(
                gegenpoly_n(n, 0.5, -1.0),
                if even { 1.0 } else { -1.0 },
                "P_{n}(-1) must be exactly (-1)^n"
            );
        }
        // The claim about the reference, so it cannot quietly stop being
        // true: at n = 29 the coefficient form really does miss the exact
        // endpoint value by far more than the recurrence's zero.
        let dense_29 = crate::poly::dense::legendre(29).eval(-1.0);
        assert!(
            (dense_29 + 1.0).abs() > 1e-9,
            "the coefficient form is documented as losing ~7 digits at \
             n = 29; it gave {dense_29:.17}, which would mean the growth \
             table above is stale"
        );
    }

    /// **`lambda = 1` gives the Chebyshev polynomials of the second kind**,
    /// `U_n(x) = sin((n+1) theta) / sin(theta)` with `x = cos theta`.
    ///
    /// The reference is the trigonometric closed form, which shares nothing
    /// with the recurrence. `x = +/-1` is excluded, where the closed form is
    /// `0/0`.
    #[test]
    fn lambda_one_gives_chebyshev_of_the_second_kind() {
        let (mut worst, mut at) = (0.0_f64, (0u32, 0.0_f64));
        for n in 0..=25u32 {
            for i in 1..40 {
                let x = -1.0 + 2.0 * i as f64 / 40.0;
                let theta = x.acos();
                let want = ((n as f64 + 1.0) * theta).sin() / theta.sin();
                let e = (gegenpoly_n(n, 1.0, x) - want).abs();
                if e > worst {
                    worst = e;
                    at = (n, x);
                }
            }
        }
        assert!(
            worst < 1e-11,
            "C_n^1 against U_n: {worst:e} at n = {}, x = {}",
            at.0,
            at.1
        );
    }

    /// **`lambda = 0` is `2 T_n(x) / n`** — upstream's own normalisation,
    /// and the reason a separate branch exists at all.
    ///
    /// `C_n^lambda` vanishes identically at `lambda = 0` for `n >= 1`; what
    /// upstream returns is the limit of `C_n^lambda / lambda`, which is the
    /// useful quantity and is what the `2 T_n / n` branch computes. This
    /// test pins that interpretation, because a reader who assumed the
    /// former would think the module wrong.
    #[test]
    fn the_lambda_zero_closed_forms_are_upstreams_normalisation() {
        for n in 4..=20u32 {
            for i in 0..=40 {
                let x = -1.0 + 2.0 * i as f64 / 40.0;
                // 2 T_n(x) / n, with T_n by its own trigonometric form.
                let want = 2.0 * (n as f64 * x.acos()).cos() / n as f64;
                assert!(
                    (gegenpoly_n(n, 0.0, x) - want).abs() < 1e-13,
                    "C_{n}^0 at x = {x}: {} against 2 T_n/n = {want}",
                    gegenpoly_n(n, 0.0, x)
                );
            }
        }
        // And the low orders follow the same normalisation, which is why
        // gegenpoly_1(0, x) is 2x rather than 0.
        assert_eq!(gegenpoly_1(0.0, 0.7), 1.4);
        assert!((gegenpoly_2(0.0, 0.7) - (2.0 * 0.49 - 1.0)).abs() < 1e-15);
    }

    /// **The recurrence agrees with the closed forms where they overlap.**
    ///
    /// `n = 2` and `n = 3` are both a closed form *and* the recurrence's
    /// seed, so this cannot fail; `n = 4` and `n = 5` are the first orders
    /// the recurrence actually produces, and they are checked against the
    /// explicit polynomials
    ///
    /// ```text
    ///   C_4 = (2/3) l (1+l) (-3 + 2(2+l)(1 + ... )) ...
    /// ```
    ///
    /// written instead as the Rodrigues-free generating identity
    /// `sum_n C_n^l(x) t^n = (1 - 2xt + t^2)^{-l}`, evaluated by
    /// differentiating the generating function numerically. That is an
    /// independent reference and covers every `n` at once.
    #[test]
    fn the_generating_function_reproduces_the_recurrence() {
        // (1 - 2 x t + t^2)^{-lambda} = sum_n C_n^lambda(x) t^n.
        // Recover the coefficients by evaluating the generating function on
        // a circle and doing a discrete Fourier transform -- a Cauchy
        // integral, which needs only the generating function itself.
        let n_fft = 256usize;
        for &lambda in &[0.25_f64, 0.5, 1.0, 2.5] {
            for &x in &[-0.8_f64, -0.3, 0.0, 0.45, 0.9] {
                let r = 0.5_f64; // inside the nearest singularity
                for n in 4..=12u32 {
                    // c_n = (1/2 pi) integral_0^{2pi} f(r e^{i th}) e^{-i n th} dth / r^n
                    let (mut re, mut im) = (0.0_f64, 0.0_f64);
                    for j in 0..n_fft {
                        let th = 2.0 * core::f64::consts::PI * j as f64 / n_fft as f64;
                        let (tr, ti) = (r * th.cos(), r * th.sin());
                        // w = 1 - 2 x t + t^2, complex
                        let wr = 1.0 - 2.0 * x * tr + (tr * tr - ti * ti);
                        let wi = -2.0 * x * ti + 2.0 * tr * ti;
                        // w^{-lambda} = exp(-lambda log w)
                        let modw = (wr * wr + wi * wi).sqrt();
                        let argw = ti.atan2(1.0) * 0.0 + wi.atan2(wr);
                        let mag = (-lambda * modw.ln()).exp();
                        let (fr, fi) = (mag * (-lambda * argw).cos(), mag * (-lambda * argw).sin());
                        let (er, ei) = ((-(n as f64) * th).cos(), (-(n as f64) * th).sin());
                        re += fr * er - fi * ei;
                        im += fr * ei + fi * er;
                    }
                    let _ = im;
                    let coeff = re / n_fft as f64 / r.powi(n as i32);
                    let got = gegenpoly_n(n, lambda, x);
                    assert!(
                        (got - coeff).abs() < 1e-9 * (1.0 + coeff.abs()),
                        "C_{n}^{lambda}({x}) = {got:e}, generating function gives {coeff:e}"
                    );
                }
            }
        }
    }

    /// The domain refusal, and the shape of the recurrence at large `n`.
    #[test]
    fn the_refusals_and_the_growth_are_right() {
        for lambda in [-0.5_f64, -0.6, -1.0] {
            assert!(gegenpoly_n(5, lambda, 0.3).is_nan(), "lambda = {lambda}");
        }
        assert!(gegenpoly_n(5, f64::NAN, 0.3).is_nan());
        assert!(gegenpoly_n(5, 0.5, f64::NAN).is_nan());
        // C_0 is 1 for every lambda and x.
        for lambda in [0.0_f64, 0.5, 3.0] {
            assert_eq!(gegenpoly_n(0, lambda, 0.37), 1.0);
        }
        // On [-1, 1] with lambda = 1/2 the polynomials are bounded by 1,
        // which is Legendre's classical bound and a real check that the
        // recurrence is not drifting.
        for n in 0..=60u32 {
            for i in 0..=80 {
                let x = -1.0 + 2.0 * i as f64 / 80.0;
                let v = gegenpoly_n(n, 0.5, x);
                assert!(
                    v.abs() <= 1.0 + 1e-12,
                    "|P_{n}({x})| = {v} exceeds 1; the recurrence is drifting"
                );
            }
        }
        // Outside [-1, 1] they grow, which is the other half of the shape.
        assert!(gegenpoly_n(10, 0.5, 1.5).abs() > 100.0);
    }
}
