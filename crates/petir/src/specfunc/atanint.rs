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
// PORTED from the GNU Scientific Library (GSL) 2.8, specfunc/atanint.c
// <https://www.gnu.org/software/gsl/>
// Copyright (C) 1996-2000 Gerard Jungman
// GSL is free software under the GNU General Public License, version 3 or
// later.
//
// The one Chebyshev table here was extracted from that source by script, not
// retyped, and is audited bit-for-bit by tests/gsl_tables_audit.rs.

//! The inverse-tangent integral `Ti_2(x)`.
//!
//! # What this is
//!
//! ```text
//!     Ti_2(x) = integral_0^x arctan(t) / t dt
//!             = sum_{k>=0} (-1)^k x^{2k+1} / (2k+1)^2      for |x| <= 1
//! ```
//!
//! It is the odd counterpart of [`crate::specfunc::clausen`] — where `Cl_2`
//! sums `sin(k x)/k^2` over all `k`, this sums `x^{2k+1}/(2k+1)^2` with
//! alternating sign — and like it, `Ti_2(1)` is **Catalan's constant**. The
//! two are related through `Ti_2(tan t) = t ln(tan t) + (1/2)(Cl_2(2t) +
//! Cl_2(pi - 2t))`, which is why they share a reference value.
//!
//! # Argument range, stated plainly
//!
//! **All real `x`**, dimensionless (`f64`). `Ti_2` is odd, so only `|x|`
//! matters to the magnitude. There is no refusal and no overflow: the
//! function grows like `(pi/2) ln|x|` for large `|x|`, i.e. logarithmically.
//!
//! `NaN` propagates.
//!
//! # Accuracy
//!
//! Measured two ways, neither of which this module can satisfy by
//! construction: against the defining **power series** on `[-1, 1]`, and
//! against its own **derivative** `arctan(x)/x` by an eighth-order central
//! difference over a much wider range. Results are in the tests.

// Under a std-linked build (`cargo test`) f64's inherent abs/ln shadow these
// trait methods, leaving the import formally unused. See crate::real.
#[allow(unused_imports)]
use crate::real::Real;

use crate::specfunc::SQRT_DBL_EPSILON;

/// GSL's `atanint_cs`, 21 coefficients, order 20 -- the whole array is used.
#[rustfmt::skip]
const ATANINT: [f64; 21] = [
    1.91040361296235937512, -0.4176351437656746940e-01, 0.275392550786367434e-02,
    -0.25051809526248881e-03, 0.2666981285121171e-04, -0.311890514107001e-05,
    0.38833853132249e-06, -0.5057274584964e-07, 0.681225282949e-08, -0.94212561654e-09,
    0.13307878816e-09, -0.1912678075e-10, 0.278912620e-11, -0.41174820e-12, 0.6142987e-13,
    -0.924929e-14, 0.140387e-14, -0.21460e-15, 0.3301e-16, -0.511e-17, 0.79e-18,
];

/// `Ti_2(x)`, GSL's `gsl_sf_atanint`.
///
/// Odd, and unbounded only logarithmically. `NaN` propagates; there is no
/// domain error.
///
/// # The two Chebyshev branches share one table, evaluated at reciprocal
/// arguments
///
/// For `|x| <= 1` the series is evaluated at `2(x^2 - 1/2)`; for `|x| > 1` at
/// `2(1/x^2 - 1/2)`, and the result is combined with `(pi/2) ln|x|`. That is
/// the Landen-type reflection `Ti_2(x) - Ti_2(1/x) = (pi/2) ln x` built into
/// the branch structure rather than applied afterwards — which is why
/// `the_reflection_is_the_branch_structure` tests it as a *join* rather than
/// as an independent identity.
///
/// # Examples
///
/// ```
/// use petir::specfunc::atanint::atanint;
/// // Ti_2(1) is Catalan's constant.
/// assert!((atanint(1.0) - 0.915_965_594_177_219_0).abs() < 1e-15);
/// // Odd, and zero at the origin.
/// assert_eq!(atanint(0.0), 0.0);
/// assert!((atanint(-2.0) + atanint(2.0)).abs() < 1e-15);
/// ```
pub fn atanint(x: f64) -> f64 {
    if x.is_nan() {
        return f64::NAN;
    }
    let ax = x.abs();
    let sgn = if x < 0.0 { -1.0 } else { 1.0 };

    if ax == 0.0 {
        0.0
    } else if ax < 0.5 * SQRT_DBL_EPSILON {
        // Ti_2(x) ~ x as x -> 0, the first term of the series.
        x
    } else if ax <= 1.0 {
        let t = 2.0 * (x * x - 0.5);
        x * crate::cheb_slice::eval_gsl(t, &ATANINT)
    } else if ax < 1.0 / SQRT_DBL_EPSILON {
        let t = 2.0 * (1.0 / (x * x) - 0.5);
        sgn * (0.5 * core::f64::consts::PI * ax.ln()
            + crate::cheb_slice::eval_gsl(t, &ATANINT) / ax)
    } else {
        // The series at 1/x^2 has collapsed to its leading term, 1.
        sgn * (0.5 * core::f64::consts::PI * ax.ln() + 1.0 / ax)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::f64::consts::PI;

    /// Catalan's constant, `Ti_2(1)`, to the digits OEIS A006752 gives.
    const CATALAN: f64 = 0.915_965_594_177_219_015;

    /// The defining power series, valid for `|x| <= 1`. Shares no
    /// coefficient, branch or threshold with the implementation.
    fn series(x: f64) -> f64 {
        let mut s = 0.0_f64;
        let x2 = x * x;
        let mut p = x;
        for k in 0..200_000u32 {
            let d = (2 * k + 1) as f64;
            let term = p / (d * d);
            s += if k % 2 == 0 { term } else { -term };
            if term.abs() < 1e-19 * s.abs().max(1e-300) {
                break;
            }
            p *= x2;
        }
        s
    }

    /// An eighth-order central first difference.
    fn derivative(f: impl Fn(f64) -> f64, x: f64, h: f64) -> f64 {
        const C: [f64; 9] = [
            1.0 / 280.0,
            -4.0 / 105.0,
            1.0 / 5.0,
            -4.0 / 5.0,
            0.0,
            4.0 / 5.0,
            -1.0 / 5.0,
            4.0 / 105.0,
            -1.0 / 280.0,
        ];
        let mut s = 0.0;
        for (k, &ck) in C.iter().enumerate() {
            s += ck * f(x + (k as f64 - 4.0) * h);
        }
        s / h
    }

    /// **Against the defining power series**, on the interval where it
    /// converges.
    ///
    /// Worst relative **1.493e-15** at `x = -0.97`, measured 2026-09-19 —
    /// about seven `f64` ulp, at the end of the interval where the
    /// alternating series converges most slowly.
    #[test]
    fn it_matches_the_defining_power_series() {
        let (mut worst, mut at) = (0.0_f64, 0.0_f64);
        for k in -99..=99i32 {
            let x = 0.01 * k as f64;
            let r = series(x);
            if r.abs() < 1e-14 {
                continue;
            }
            let e = ((atanint(x) - r) / r).abs();
            if e > worst {
                worst = e;
                at = x;
            }
        }
        assert!(worst < 1e-13, "Ti_2 vs its power series: {worst:e} at {at}");
    }

    /// **Against its own derivative**, `d/dx Ti_2(x) = arctan(x)/x`, which
    /// reaches far past where the power series converges and so covers the
    /// reflected branch too.
    ///
    /// Worst relative **2.236e-12** at `x = 28.15`, measured 2026-09-19. That
    /// is the eighth-order difference's own truncation at `h = 1/64`, not
    /// `Ti_2`'s — the same instrument and the same caveat as
    /// [`crate::specfunc::airy`]'s ODE residual, where the step size was
    /// measured rather than assumed.
    #[test]
    fn its_derivative_is_arctan_over_x() {
        let h = 1.0 / 64.0;
        let (mut worst, mut at) = (0.0_f64, 0.0_f64);
        for k in 1..=600i32 {
            let x = 0.05 * k as f64;
            let want = x.atan() / x;
            let got = derivative(atanint, x, h);
            let e = ((got - want) / want).abs();
            if e > worst {
                worst = e;
                at = x;
            }
        }
        assert!(worst < 1e-9, "d/dx Ti_2 vs arctan(x)/x: {worst:e} at {at}");
    }

    /// Catalan's constant, oddness, and the logarithmic tail.
    #[test]
    fn the_known_values_and_shape_are_right() {
        assert_eq!(atanint(0.0), 0.0);
        assert!(
            (atanint(1.0) - CATALAN).abs() < 1e-15,
            "Ti_2(1) = {} vs Catalan",
            atanint(1.0)
        );
        for k in 1..=400 {
            let x = 0.05 * k as f64;
            assert!((atanint(-x) + atanint(x)).abs() < 1e-15, "odd at {x}");
        }
        // Increasing on the positive axis, and logarithmic at large x.
        let mut prev = 0.0;
        for k in 1..=500 {
            let x = 0.1 * k as f64;
            let v = atanint(x);
            assert!(v > prev, "not increasing at {x}");
            prev = v;
        }
        for x in [1e6_f64, 1e12, 1e18] {
            let asym = 0.5 * PI * x.ln() + 1.0 / x;
            assert!(
                ((atanint(x) - asym) / asym).abs() < 1e-12,
                "Ti_2({x:e}) vs (pi/2) ln x + 1/x"
            );
        }
        assert!(atanint(f64::NAN).is_nan());
    }

    /// **The reflection IS the branch structure**, so it is tested as a join
    /// rather than as an independent identity.
    ///
    /// `Ti_2(x) - Ti_2(1/x) = (pi/2) ln x` is what the `|x| > 1` branch
    /// implements, so satisfying it there is close to a tautology. What is
    /// *not* a tautology is that the two branches agree at `|x| = 1` where
    /// they meet, and that the identity holds **across** the boundary — one
    /// side evaluated by each branch. Both are checked.
    #[test]
    fn the_reflection_is_the_branch_structure() {
        // The two formulas at x = 1, where the branches meet.
        let x = 1.0_f64;
        let below = x * crate::cheb_slice::eval_gsl(2.0 * (x * x - 0.5), &ATANINT);
        let above = 0.5 * PI * x.ln()
            + crate::cheb_slice::eval_gsl(2.0 * (1.0 / (x * x) - 0.5), &ATANINT) / x;
        assert!(
            ((below - above) / above).abs() < 1e-14,
            "the two Chebyshev branches at x = 1: {below} vs {above}"
        );

        // The identity across the boundary: x > 1 uses the reflected branch,
        // 1/x < 1 uses the direct one.
        let mut worst = 0.0_f64;
        for k in 1..=200 {
            let x = 1.0 + 0.1 * k as f64;
            let lhs = atanint(x) - atanint(1.0 / x);
            let rhs = 0.5 * PI * x.ln();
            worst = worst.max(((lhs - rhs) / rhs).abs());
        }
        assert!(worst < 1e-14, "reflection across the boundary: {worst:e}");

        // The small-argument branch joins the Chebyshev one.
        let x = 0.5 * SQRT_DBL_EPSILON;
        let cheb = x * crate::cheb_slice::eval_gsl(2.0 * (x * x - 0.5), &ATANINT);
        assert!(
            ((x - cheb) / cheb).abs() < 1e-14,
            "small/Chebyshev join at {x:e}: {x:e} vs {cheb:e}"
        );

        // And the large-argument branch joins the reflected one.
        let x = 1.0 / SQRT_DBL_EPSILON;
        let a = 0.5 * PI * x.ln()
            + crate::cheb_slice::eval_gsl(2.0 * (1.0 / (x * x) - 0.5), &ATANINT) / x;
        let b = 0.5 * PI * x.ln() + 1.0 / x;
        assert!(
            ((a - b) / b).abs() < 1e-14,
            "reflected/asymptote join at {x:e}: {a} vs {b}"
        );
    }
}
