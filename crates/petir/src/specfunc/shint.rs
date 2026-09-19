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
// PORTED from the GNU Scientific Library (GSL) 2.8, specfunc/shint.c
// <https://www.gnu.org/software/gsl/>
// Copyright (C) 1996-2000 Gerard Jungman
// GSL is free software under the GNU General Public License, version 3 or
// later.
//
// Upstream's own header credits the small-argument series to SLATEC's
// `shi.f` by W. Fullerton, and records its fit: 7 coefficients over
// [0, 1.40625e-01], weighted error 4.67e-20.
//
// ONE CHEBYSHEV TABLE, 7 coefficients -- upstream's `shi_cs`. The rest of
// both functions is `E_1` and `Ei`, which `crate::expint` already ports.

//! The hyperbolic sine and cosine integrals `Shi(x)` and `Chi(x)`.
//!
//! # What these are
//!
//! ```text
//!     Shi(x) = integral_0^x  sinh(t) / t  dt
//!     Chi(x) = gamma + ln|x| + integral_0^x (cosh(t) - 1) / t  dt
//! ```
//!
//! the hyperbolic counterparts of [`crate::specfunc::sinint`]'s `Si` and
//! `Ci`, and — like them — a pair rather than two independent functions:
//! both are combinations of the exponential integrals,
//!
//! ```text
//!     Shi(x) = (Ei(x) + E_1(x)) / 2
//!     Chi(x) = (Ei(x) - E_1(x)) / 2
//! ```
//!
//! **That relation is upstream's entire implementation of `Chi`, and of
//! `Shi` above `|x| = 0.375`.** Only the small-argument branch of `Shi` has
//! a series of its own, because there `Ei` and `E_1` each diverge
//! logarithmically while their sum does not — so forming the difference of
//! two large numbers would lose exactly the digits being asked for.
//!
//! # Why this module is thin, and why that was worth checking
//!
//! `specfunc/shint.c` is 135 lines and carries one 7-coefficient table.
//! Everything else it needs is `gsl_sf_expint_Ei_e` and
//! `gsl_sf_expint_E1_e`. PETIR already had `E_1`; it did **not** have `Ei`,
//! which upstream defines as `-E_1(-x)` and nothing more, so that was added
//! to [`crate::expint`] rather than reimplemented here.
//!
//! Reading the source first is what established that. From the outside these
//! look like two more Chebyshev-fitted special functions; they are almost
//! entirely a re-expression of one that was already present.
//!
//! # Argument range
//!
//! `x` is a dimensionless `f64`.
//!
//! - `Shi` is **odd** and defined for all real `x`. It grows like
//!   `e^x / (2x)`, so it overflows `f64` near `x = 710`.
//! - `Chi` is real only for `x > 0`, where it has a logarithmic singularity
//!   at the origin. **Upstream nonetheless returns a value for `x < 0`** —
//!   it refuses only where `E_1` itself refuses, which is `x = 0` — and that
//!   behaviour is carried rather than tightened. What it returns there is the
//!   real part of the analytic continuation, `Chi(x) = Chi(|x|)` up to the
//!   `i pi` that a real return type cannot express. Treat a negative
//!   argument as a caller error; this module will not catch it for you.
//!
//! `NaN` is returned where upstream signals `DOMAIN_ERROR`, following the
//! convention of the rest of [`crate::specfunc`].
//!
//! # Accuracy
//!
//! Measured against the **defining integrals** by Gauss-Legendre quadrature,
//! against the **series** `Shi(x) = sum x^{2k+1} / ((2k+1) (2k+1)!)`, and
//! against the identity `Shi(x) + Chi(x) = Ei(x)`. Measured 2026-09-20:
//!
//! | check | worst | at |
//! |---|---|---|
//! | `Shi` vs its power series (no quadrature) | 3.948e-16 | 0.28 |
//! | `Shi` vs its defining integral | 1.045e-14 | 7.2 |
//! | `Chi` vs its defining integral | 1.020e-14 | 7.2 |
//! | `Chi`'s zero, module vs integral | 2.220e-16 | — |
//!
//! **The two quadrature figures are upper bounds, not measurements of this
//! module.** Both are worst at the largest argument tried, which is where
//! the integrand has grown by three decades and the 200-panel composite rule
//! is working hardest. The series check, which involves no quadrature at
//! all, is the one that pins `Shi` — and it comes in below one `f64` ulp.

// Under a std-linked build (`cargo test`) f64's inherent ln/abs shadow these
// trait methods, leaving the import formally unused. See crate::real.
#[allow(unused_imports)]
use crate::real::Real;

use crate::cheb_slice::eval_gsl as cheb;

/// Upstream's `shi_cs`, 7 coefficients on `[-1, 1]`, order 6.
///
/// From SLATEC's `shi.f` (W. Fullerton) by way of `specfunc/shint.c:42`.
/// Upstream records the fit as covering `[0, 1.40625e-01]` in the mapped
/// variable with a weighted error of `4.67e-20`.
const SHI: [f64; 7] = [
    0.0078372685688900950695,
    0.0039227664934234563973,
    0.0000041346787887617267,
    0.0000000024707480372883,
    0.0000000000009379295591,
    0.0000000000000002451817,
    0.0000000000000000000467,
];

/// GSL's `order_sp` for [`SHI`] — **6, the same as its `f64` order.**
///
/// Every other Chebyshev series PETIR has ported carries a strictly smaller
/// single-precision order; this one does not, because the series is already
/// only seven terms and its last coefficient is `4.67e-22`. There is nothing
/// to cut. Recorded rather than omitted so the shader's table is visibly a
/// decision and not an oversight.
pub const SHI_ORDER_SP: usize = 6;

/// `Shi(x) = integral_0^x sinh(t)/t dt`, GSL's `gsl_sf_Shi`
/// (`specfunc/shint.c:60`).
///
/// Odd in `x`, dimensionless, defined for every real argument and finite up
/// to about `x = 710` where `e^x / (2x)` leaves `f64`'s range.
///
/// # Examples
///
/// ```
/// use petir::specfunc::shint::shi;
/// // Shi is odd, and Shi(x) -> x as x -> 0.
/// assert!((shi(1e-9) - 1e-9).abs() < 1e-24);
/// assert!((shi(-2.0) + shi(2.0)).abs() < 1e-14);
/// ```
pub fn shi(x: f64) -> f64 {
    if x.is_nan() {
        return f64::NAN;
    }
    let ax = x.abs();
    // Upstream's `xsml`, GSL_SQRT_DBL_EPSILON: below it the series is x and
    // nothing measurable more.
    if ax < crate::specfunc::SQRT_DBL_EPSILON {
        return x;
    }
    if ax <= 0.375 {
        return x * (1.0 + cheb(128.0 * x * x / 9.0 - 1.0, &SHI));
    }
    // (Ei + E_1) / 2. Upstream reports underflow where BOTH underflow and
    // overflow where either overflows; with a bare-f64 return the arithmetic
    // says the same thing, so the branch is not reproduced as a separate
    // refusal.
    let ei = crate::expint::expint_ei(x);
    let e1 = crate::expint::expint_e1(x);
    match (ei, e1) {
        (Ok((a, _)), Ok((b, _))) => 0.5 * (a + b),
        _ => f64::NAN,
    }
}

/// `Chi(x) = gamma + ln|x| + integral_0^x (cosh t - 1)/t dt`, GSL's
/// `gsl_sf_Chi` (`specfunc/shint.c:98`).
///
/// Dimensionless. Real for `x > 0`; see the module documentation for what
/// upstream does with a negative argument, which is carried here.
///
/// # Examples
///
/// ```
/// use petir::specfunc::shint::{chi, shi};
/// use petir::expint::expint_ei;
/// // Shi + Chi = Ei, which is the identity both are built from.
/// let (ei, _) = expint_ei(1.5).unwrap();
/// assert!((shi(1.5) + chi(1.5) - ei).abs() < 1e-13);
/// ```
pub fn chi(x: f64) -> f64 {
    if x.is_nan() {
        return f64::NAN;
    }
    let ei = crate::expint::expint_ei(x);
    let e1 = crate::expint::expint_e1(x);
    match (ei, e1) {
        (Ok((a, _)), Ok((b, _))) => 0.5 * (a - b),
        _ => f64::NAN,
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    /// Composite 30-point Gauss-Legendre, the independent reference.
    fn quad<F: Fn(f64) -> f64>(f: F, a: f64, b: f64, panels: usize) -> f64 {
        let h = (b - a) / panels as f64;
        let mut acc = 0.0;
        for k in 0..panels {
            let lo = a + k as f64 * h;
            acc += crate::integration::gauss_legendre::gauss_legendre(&f, lo, lo + h, 30)
                .expect("30-point Gauss-Legendre is tabulated");
        }
        acc
    }

    /// **`Shi` reproduces its defining integral**, which shares nothing with
    /// the implementation: the quadrature never forms a Chebyshev series and
    /// never touches `E_1`.
    ///
    /// ```text
    ///     Shi(x) = integral_0^x sinh(t)/t dt
    /// ```
    ///
    /// Measured 2026-09-20 over 40 points in `(0, 8]`: worst relative
    /// **1.045e-14**, at `x = 7.2`. That is the quadrature's own error as
    /// much as the module's — 200 panels of a 30-point rule over a range
    /// where `sinh` has grown by three decades — so read it as an upper
    /// bound on the module, not a measurement of it. The power-series check
    /// below, which has no quadrature in it, comes in at 3.9e-16.
    /// The integrand is removable at `t = 0` (`sinh t / t -> 1`),
    /// so the panel nearest the origin is evaluated with that limit
    /// substituted rather than by dividing by zero.
    #[test]
    fn shi_reproduces_its_defining_integral() {
        let (mut worst, mut at) = (0.0_f64, 0.0_f64);
        for i in 1..=40 {
            let x = 0.2 * i as f64;
            let q = quad(
                |t: f64| {
                    if t.abs() < 1e-12 {
                        1.0
                    } else {
                        t.sinh() / t
                    }
                },
                0.0,
                x,
                200,
            );
            let e = ((shi(x) - q) / q).abs();
            if e > worst {
                worst = e;
                at = x;
            }
        }
        assert!(worst < 1e-12, "Shi against its integral: {worst:e} at {at}");
    }

    /// **`Shi` reproduces its power series**, which is a second independent
    /// reference and the one that covers the small-argument Chebyshev branch
    /// the integral check exercises least:
    ///
    /// ```text
    ///     Shi(x) = sum_{k>=0} x^{2k+1} / ((2k+1) (2k+1)!)
    /// ```
    ///
    /// Summed to machine precision, which is practical only for small `x` —
    /// so this is deliberately run across the `0.375` branch cut, where the
    /// series must agree with *both* the Chebyshev branch below it and the
    /// `(Ei + E_1)/2` branch above.
    #[test]
    fn shi_reproduces_its_power_series_across_the_branch_cut() {
        // Term by term, exactly as written: x^{2k+1} / ((2k+1) * (2k+1)!).
        let direct = |x: f64| -> f64 {
            let mut sum = 0.0_f64;
            let mut fact = 1.0_f64; // (2k+1)!
            let mut pow = x; // x^{2k+1}
            for k in 0..40u32 {
                let n = 2 * k + 1;
                if k > 0 {
                    fact *= (n as f64 - 1.0) * n as f64;
                    pow *= x * x;
                }
                let t = pow / (n as f64 * fact);
                sum += t;
                if t.abs() < 1e-18 * sum.abs() {
                    break;
                }
            }
            sum
        };
        // Sanity: the series itself against the quadrature, so a wrong series
        // cannot make a wrong module look right.
        let q = quad(
            |t: f64| if t.abs() < 1e-12 { 1.0 } else { t.sinh() / t },
            0.0,
            0.5,
            200,
        );
        assert!(
            ((direct(0.5) - q) / q).abs() < 1e-13,
            "the reference series is wrong: {} against {q}",
            direct(0.5)
        );

        let (mut worst, mut at) = (0.0_f64, 0.0_f64);
        for i in 1..=60 {
            let x = 0.01 * i as f64; // 0.01 .. 0.60, crossing 0.375
            let r = direct(x);
            let e = ((shi(x) - r) / r).abs();
            if e > worst {
                worst = e;
                at = x;
            }
        }
        assert!(
            worst < 1e-13,
            "Shi against its power series across the 0.375 cut: {worst:e} at {at}"
        );
    }

    /// **`Shi + Chi = Ei`** — the identity both functions are built from,
    /// checked against [`crate::expint::expint_ei`] directly.
    ///
    /// This is weaker than it looks above `0.375`, where both sides are the
    /// same two calls rearranged, and is genuinely informative **below** it,
    /// where `Shi` comes from its own Chebyshev series and `Chi` does not.
    /// The sweep is chosen to sit on both sides of that cut for exactly that
    /// reason.
    #[test]
    fn shi_plus_chi_is_ei() {
        for i in 1..=50 {
            let x = 0.05 * i as f64;
            let (ei, _) = crate::expint::expint_ei(x).expect("Ei is finite here");
            let s = shi(x) + chi(x);
            assert!(
                ((s - ei) / ei).abs() < 1e-12,
                "Shi + Chi = Ei at x = {x}: {s:e} against {ei:e}"
            );
        }
    }

    /// `Chi - Shi = -E_1`, the other combination, which isolates `Chi`'s own
    /// branch from `Shi`'s series.
    #[test]
    fn chi_minus_shi_is_minus_e1() {
        for i in 1..=50 {
            let x = 0.05 * i as f64;
            let (e1, _) = crate::expint::expint_e1(x).expect("E_1 is finite here");
            let d = chi(x) - shi(x);
            assert!(
                ((d + e1) / e1).abs() < 1e-12,
                "Chi - Shi = -E_1 at x = {x}: {d:e} against {:e}",
                -e1
            );
        }
    }

    /// **`Chi` reproduces its own defining integral, and its zero is where
    /// that integral puts it.**
    ///
    /// ```text
    ///     Chi(x) = gamma + ln x + integral_0^x (cosh t - 1) / t  dt
    /// ```
    ///
    /// The quadrature shares nothing with the module: no Chebyshev series,
    /// no `E_1`, no `Ei`. The integrand is removable at the origin
    /// (`(cosh t - 1)/t -> t/2`), so the first panel uses that limit.
    ///
    /// **The zero is located by bisecting the QUADRATURE, not quoted.** An
    /// earlier draft of this test asserted `Chi(0.523822571389...) = 0` from
    /// a recalled digit string and failed at `-1.07e-12` — which could have
    /// been read as the module being slightly wrong, when what was wrong was
    /// the constant. Deriving it here makes the test say what it means: the
    /// module and the integral agree about where `Chi` crosses zero, to
    /// within the bisection's own resolution. Measured 2026-09-20: they
    /// agree to **2.220e-16** in position, which is one `f64` ulp at 0.5,
    /// and the function itself matches the integral to 1.020e-14 (again
    /// quadrature-limited, worst at `x = 7.2`).
    ///
    /// For the record, since it is the reason this test is shaped this way:
    /// the zero is at **0.52382257138986432**, and the recalled string that
    /// failed was `0.523822571389372` — wrong from the twelfth digit.
    ///
    /// A zero is the sharpest available check on a function like this,
    /// because no scale error can produce one in the right place.
    #[test]
    fn chi_matches_its_defining_integral_and_its_zero() {
        let chi_def = |x: f64| -> f64 {
            crate::specfunc::EULER
                + x.ln()
                + quad(
                    |t: f64| {
                        if t.abs() < 1e-12 {
                            0.5 * t
                        } else {
                            (t.cosh() - 1.0) / t
                        }
                    },
                    0.0,
                    x,
                    200,
                )
        };

        // The function itself, across the 0.375 cut and out to 8.
        let (mut worst, mut at) = (0.0_f64, 0.0_f64);
        for i in 1..=40 {
            let x = 0.2 * i as f64;
            let r = chi_def(x);
            // Relative where Chi is not near its zero; absolute where it is.
            let e = if r.abs() > 1e-3 {
                ((chi(x) - r) / r).abs()
            } else {
                (chi(x) - r).abs()
            };
            if e > worst {
                worst = e;
                at = x;
            }
        }
        assert!(
            worst < 1e-12,
            "Chi against its defining integral: {worst:e} at {at}"
        );

        // Bisect each for its zero, independently.
        let bisect = |f: &dyn Fn(f64) -> f64| -> f64 {
            let (mut lo, mut hi) = (0.3_f64, 0.9_f64);
            assert!(f(lo) < 0.0 && f(hi) > 0.0, "the zero is not bracketed");
            for _ in 0..200 {
                let mid = 0.5 * (lo + hi);
                if f(mid) < 0.0 {
                    lo = mid;
                } else {
                    hi = mid;
                }
            }
            0.5 * (lo + hi)
        };
        let z_mod = bisect(&|x| chi(x));
        let z_ref = bisect(&chi_def);
        assert!(
            (z_mod - z_ref).abs() < 1e-12,
            "Chi's zero: the module puts it at {z_mod:.17}, the defining \
             integral at {z_ref:.17}"
        );
        // And it is a crossing, not a touch.
        assert!(chi(z_mod - 1e-3) < 0.0 && chi(z_mod + 1e-3) > 0.0);
        // Sanity on the bracket, so a bisection that converged to an endpoint
        // could not pass.
        assert!(z_mod > 0.4 && z_mod < 0.7, "Chi's zero at {z_mod}");
    }

    /// `Shi` is odd and `Chi` is not, and the refusals are upstream's.
    #[test]
    fn the_symmetry_and_the_refusals_are_right() {
        for i in 1..=30 {
            let x = 0.3 * i as f64;
            let s = shi(x);
            assert!(
                ((shi(-x) + s) / s).abs() < 1e-14,
                "Shi is odd; at {x} it gave {} and {}",
                shi(-x),
                s
            );
        }
        // Shi is strictly increasing on the positive axis.
        let mut prev = f64::NEG_INFINITY;
        for i in 0..=60 {
            let x = 0.1 * i as f64;
            let v = shi(x);
            assert!(v > prev, "Shi not increasing at {x}: {v} <= {prev}");
            prev = v;
        }
        // x = 0 is E_1's singularity, so Chi refuses there; Shi does not,
        // because its own series covers the origin.
        assert!(chi(0.0).is_nan(), "Chi(0) = {}", chi(0.0));
        assert_eq!(shi(0.0), 0.0);
        assert!(shi(f64::NAN).is_nan());
        assert!(chi(f64::NAN).is_nan());
    }

    /// The single-precision order is recorded and is genuinely 6.
    ///
    /// Every other series PETIR ships carries an `order_sp` strictly below
    /// its `f64` order. This one does not, and the constant exists so the
    /// shader's full-length table reads as a decision rather than a missed
    /// truncation.
    #[test]
    fn the_single_precision_order_is_the_full_order_and_that_is_upstreams() {
        assert_eq!(SHI_ORDER_SP, SHI.len() - 1);
        // Because the series has nothing left to cut: the last coefficient is
        // already below f32's smallest normal contribution to a value of
        // order 1.
        assert!(
            SHI[6].abs() < f64::from(f32::EPSILON) * 1e-10,
            "SHI's last coefficient is {:e}",
            SHI[6]
        );
    }
}
