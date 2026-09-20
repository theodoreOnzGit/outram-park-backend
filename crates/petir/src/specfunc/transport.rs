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
// PORTED from the GNU Scientific Library (GSL) 2.8, specfunc/transport.c
// <https://www.gnu.org/software/gsl/>
// Copyright (C) 1996-2000 Gerard Jungman
// GSL is free software under the GNU General Public License, version 3 or
// later.
//
// The four Chebyshev tables here were extracted from that source by script,
// not retyped, and are audited bit-for-bit by tests/gsl_tables_audit.rs.

//! The transport integrals `J(n, x)` for `n = 2, 3, 4, 5`.
//!
//! # What these are
//!
//! ```text
//!     J(n, x) = integral_0^x t^n e^t / (e^t - 1)^2 dt
//! ```
//!
//! They are the Debye-model transport integrals: `J(2, x)` appears in the
//! electrical and thermal conductivity of a metal under the Bloch-Grüneisen
//! model, and `J(4, x)` and `J(5, x)` in phonon-limited resistivity. The
//! integrand is the derivative of the Bose-Einstein occupation times `t^n`,
//! which is why they sit beside [`crate::specfunc::debye`] rather than
//! anywhere else — same physics, one derivative apart.
//!
//! # Argument range, stated plainly
//!
//! `x >= 0`, dimensionless (`f64`); `x` is a characteristic temperature
//! ratio, so the units cancel before you get here. A negative argument
//! returns `NaN` — upstream's `DOMAIN_ERROR`. Each `J(n, .)` increases
//! monotonically from `0` to a finite limit:
//!
//! | `n` | `J(n, inf)` |
//! |---|---|
//! | 2 | 3.289868133696452873 |
//! | 3 | 7.212341418957565712 |
//! | 4 | 25.97575760906731660 |
//! | 5 | 124.4313306172043912 |
//!
//! Those are `n! zeta(n)` for `n >= 2` — `J(2, inf) = 2 zeta(2) = pi^2/3`,
//! which is checked against [`crate::specfunc::zeta`] rather than asserted.
//!
//! `NaN` propagates. An untabulated order returns `NaN`.
//!
//! # Accuracy
//!
//! Measured against the defining integral by composite Simpson in `f64`,
//! which shares no table, branch or coefficient with the implementation.
//! Results are in the tests.

// Under a std-linked build (`cargo test`) f64's inherent exp/ln shadow these
// trait methods, leaving the import formally unused. See crate::real.
#[allow(unused_imports)]
use crate::real::Real;

use crate::specfunc::{DBL_EPSILON, SQRT_DBL_EPSILON};

/// `ln(DBL_EPSILON)`, upstream's `GSL_LOG_DBL_EPSILON`.
///
/// Written out rather than computed: the same constant was once taken from
/// memory in `debye.rs` and was wrong in its fourth digit, which moved a
/// branch boundary. This is `gsl_machine.h`'s value.
const LOG_DBL_EPSILON: f64 = -3.604_365_338_911_715_4e1;
/// GSL's `transport2_cs`, 18 coefficients, order 17.
#[rustfmt::skip]
const TRANSPORT2: [f64; 18] = [
    1.671760446434538503, -0.147735359946794490, 0.148213819946936338e-01,
    -0.14195330326305613e-02, 0.1306541324415708e-03, -0.117155795867579e-04,
    0.10333498445756e-05, -0.901911304223e-07, 0.78177169833e-08, -0.6744565684e-09,
    0.579946394e-10, -0.49747619e-11, 0.425961e-12, -0.36422e-13, 0.3111e-14, -0.265e-15,
    0.23e-16, -0.19e-17,
];

/// GSL's `transport3_cs`, 18 coefficients, order 17.
#[rustfmt::skip]
const TRANSPORT3: [f64; 18] = [
    0.762012543243872007, -0.105674387705058533, 0.119778084819657810e-01,
    -0.12144015203698307e-02, 0.1155099769392855e-03, -0.105815992124423e-04,
    0.9474663385302e-06, -0.836221212858e-07, 0.73109099278e-08, -0.6350594779e-09,
    0.549118282e-10, -0.47321395e-11, 0.4067695e-12, -0.348971e-13, 0.29892e-14, -0.256e-15,
    0.219e-16, -0.19e-17,
];

/// GSL's `transport4_cs`, 18 coefficients, order 17.
#[rustfmt::skip]
const TRANSPORT4: [f64; 18] = [
    0.4807570994615110579, -0.8175378810321083956e-01, 0.1002700665975162973e-01,
    -0.10599339359820151e-02, 0.1034506245030405e-03, -0.96442705485899e-05,
    0.8745544408515e-06, -0.779321207981e-07, 0.68649886141e-08, -0.5999571076e-09,
    0.521366241e-10, -0.45118382e-11, 0.3892159e-12, -0.334936e-13, 0.28767e-14, -0.2467e-15,
    0.211e-16, -0.18e-17,
];

/// GSL's `transport5_cs`, 18 coefficients, order 17.
#[rustfmt::skip]
const TRANSPORT5: [f64; 18] = [
    0.347777777133910789, -0.66456988976050428e-01, 0.8611072656883309e-02,
    -0.9396682223755538e-03, 0.936324806081513e-04, -0.88571319340833e-05, 0.811914989145e-06,
    -0.72957654233e-07, 0.646971455e-08, -0.568490283e-09, 0.49625598e-10, -0.4310940e-11,
    0.373100e-12, -0.32198e-13, 0.2772e-14, -0.238e-15, 0.21e-16, -0.18e-17,
];

/// Per-order constants. The four entry points differ only in these.
struct Order {
    n: u32,
    val_infinity: f64,
    cheb: &'static [f64],
}

const ORDERS: [Order; 4] = [
    Order {
        n: 2,
        val_infinity: 3.289_868_133_696_452_873,
        cheb: &TRANSPORT2,
    },
    Order {
        n: 3,
        val_infinity: 7.212_341_418_957_565_712,
        cheb: &TRANSPORT3,
    },
    Order {
        n: 4,
        val_infinity: 25.975_757_609_067_316_60,
        cheb: &TRANSPORT4,
    },
    Order {
        n: 5,
        val_infinity: 124.431_330_617_204_391_2,
        cheb: &TRANSPORT5,
    },
];

/// Upstream's `transport_sumexp`: the sum over `numexp` exponential terms
/// that carries the large-`x` tail.
///
/// The inner loop builds `sum2 = 1 + 1/(rk x) (1 + 2/(rk x) (1 + ...))` to
/// `order` terms — an asymptotic series in `1/(rk x)` — and the outer loop
/// accumulates them with a factor `t = e^{-x}` per step, so the whole thing
/// is Horner in `e^{-x}` over the `numexp` images.
fn sumexp(numexp: u32, order: u32, t: f64, x: f64) -> f64 {
    let mut rk = numexp as f64;
    let mut out = 0.0_f64;
    for _ in 1..=numexp {
        let mut sum2 = 1.0_f64;
        let xk = 1.0 / (rk * x);
        let mut xk1 = 1.0_f64;
        for _ in 1..=order {
            sum2 = sum2 * xk1 * xk + 1.0;
            xk1 += 1.0;
        }
        out *= t;
        out += sum2;
        rk -= 1.0;
    }
    out
}

/// Index [`ORDERS`] without a fallible subscript.
fn order_of(n: u32) -> Option<&'static Order> {
    match n {
        2 => ORDERS.first(),
        3 => ORDERS.get(1),
        4 => ORDERS.get(2),
        5 => ORDERS.get(3),
        _ => None,
    }
}

/// `J(n, x)` for `n` in `2 ..= 5`, GSL's `gsl_sf_transport_n`.
///
/// `NaN` for `x < 0`, for `NaN`, and for an untabulated order.
///
/// # The large-`x` branches subtract from the limit, and that is deliberate
///
/// Past `x = 4` the value is formed as `J(n, inf) - e^t` with
/// `t = n ln x - x + ln(sumexp)`, i.e. as the **limit minus the tail**
/// rather than by continuing to integrate. The tail decays like
/// `x^n e^{-x}`, so once `t` falls below `ln(DBL_EPSILON)` it cannot change
/// the answer and upstream returns the limit exactly — which is why
/// `J(n, 40)` is bit-identical to `J(n, inf)` rather than merely close.
///
/// # Examples
///
/// ```
/// use petir::specfunc::transport::transport;
/// // J(2, inf) = 2 zeta(2) = pi^2/3.
/// # use core::f64::consts::PI;
/// assert!((transport(2, 1e3) - PI * PI / 3.0).abs() < 1e-14);
/// // Monotone, and zero at the origin.
/// assert_eq!(transport(2, 0.0), 0.0);
/// assert!(transport(4, 1.0) < transport(4, 2.0));
/// ```
pub fn transport(n: u32, x: f64) -> f64 {
    let Some(o) = order_of(n) else {
        return f64::NAN;
    };
    if x.is_nan() || x < 0.0 {
        return f64::NAN;
    }
    let nf = o.n as f64;

    if x < 3.0 * SQRT_DBL_EPSILON {
        // J(n, x) ~ x^{n-1} / (n-1) as x -> 0.
        let mut p = 1.0;
        for _ in 1..o.n {
            p *= x;
        }
        return p / (nf - 1.0);
    }

    if x <= 4.0 {
        // Upstream writes the argument as (x^2/8 - 0.5) - 0.5, not
        // x^2/8 - 1.0: the two differ in the last bit near x = 0, and the
        // grouping is reproduced rather than simplified.
        let t = (x * x / 8.0 - 0.5) - 0.5;
        let mut p = 1.0;
        for _ in 1..o.n {
            p *= x;
        }
        return p * crate::cheb_slice::eval_gsl(t, o.cheb);
    }

    // Past 4, the limit minus the tail.
    let t = if x < -LOG_DBL_EPSILON {
        let numexp = ((-LOG_DBL_EPSILON) / x) as u32 + 1;
        let s = sumexp(numexp, o.n, (-x).exp(), x);
        nf * x.ln() - x + s.ln()
    } else if x < 2.0 / DBL_EPSILON {
        let s = sumexp(1, o.n, 1.0, x);
        nf * x.ln() - x + s.ln()
    } else {
        nf * x.ln() - x
    };

    if t < LOG_DBL_EPSILON {
        o.val_infinity
    } else {
        o.val_infinity - t.exp()
    }
}

/// `J(2, x)`, the Bloch-Grüneisen transport integral.
pub fn transport_2(x: f64) -> f64 {
    transport(2, x)
}

/// `J(3, x)`.
pub fn transport_3(x: f64) -> f64 {
    transport(3, x)
}

/// `J(4, x)`, which appears in phonon-limited resistivity.
pub fn transport_4(x: f64) -> f64 {
    transport(4, x)
}

/// `J(5, x)`.
pub fn transport_5(x: f64) -> f64 {
    transport(5, x)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::specfunc::zeta;

    /// The defining integral by composite Simpson.
    ///
    /// `t^n e^t / (e^t - 1)^2` is `t^{n-2}` near the origin, so the
    /// integrand is finite there for `n >= 2` but its higher derivatives are
    /// not smooth in a way Simpson likes. The panel count is chosen for that,
    /// not for elegance; the residual it leaves is the reference's, and
    /// `the_residual_is_the_quadrature_not_the_integrals` establishes that
    /// rather than assuming it.
    fn simpson(n: u32, x: f64, panels: u32) -> f64 {
        let f = |t: f64| {
            if t == 0.0 {
                // The limit: t^n e^t/(e^t-1)^2 -> t^{n-2}, which is 0 for
                // n > 2 and 1 for n = 2.
                return if n == 2 { 1.0 } else { 0.0 };
            }
            let e = t.exp();
            let d = e - 1.0;
            let mut p = 1.0;
            for _ in 0..n {
                p *= t;
            }
            p * e / (d * d)
        };
        let h = x / panels as f64;
        let mut s = f(0.0) + f(x);
        for k in 1..panels {
            let t = k as f64 * h;
            s += if k % 2 == 1 { 4.0 * f(t) } else { 2.0 * f(t) };
        }
        s * h / 3.0
    }

    /// **Against the defining integral**, composite Simpson at a fixed
    /// **step** of 1.5e-03 with a floor of 2000 panels.
    ///
    /// The step, not the panel count, is what Simpson's `O(h^4)` error
    /// depends on — and the floor is what keeps small `x` from being
    /// integrated on a handful of panels. Two earlier versions got this
    /// wrong and the test caught both: 200 000 panels everywhere is *past*
    /// the roundoff optimum (see
    /// [`the_residual_is_the_quadrature_not_the_integrals`]), and 2000
    /// everywhere is that optimum at `x = 3` but far too coarse at `x = 15`.
    ///
    /// # Results, measured 2026-09-19
    ///
    /// Worst relative over `x` in `(0, 15]`:
    ///
    /// | order | worst | at |
    /// |---|---|---|
    /// | `J(2)` | 7.290e-15 | 13.5 |
    /// | `J(3)` | 7.190e-15 | 7.25 |
    /// | `J(4)` | 6.713e-15 | 14.0 |
    /// | `J(5)` | 2.323e-14 | 3.0 |
    ///
    /// Thirty to a hundred `f64` ulp, and within about an order of the
    /// Simpson reference's own floor — so a good part of what is measured
    /// here is the reference rather than `transport`.
    #[test]
    fn every_order_matches_its_defining_integral() {
        for n in 2..=5u32 {
            let (mut worst, mut at) = (0.0_f64, 0.0_f64);
            for k in 1..=60 {
                let x = 0.25 * k as f64;
                // A fixed STEP, not a fixed panel count: Simpson's error is
                // O(h^4) per unit length, so the optimum count scales with x.
                // A first version used 2000 panels everywhere, which is the
                // measured optimum at x = 3 and far too coarse at x = 15.
                let panels = ((((x / 1.5e-3).ceil() as u32).max(2_000)) + 1) & !1u32;
                let r = simpson(n, x, panels);
                if r.abs() < 1e-12 {
                    continue;
                }
                let e = ((transport(n, x) - r) / r).abs();
                if e > worst {
                    worst = e;
                    at = x;
                }
            }
            assert!(
                worst < 1e-12,
                "J({n}, .) vs the integral: {worst:e} at {at}"
            );
        }
    }

    /// **The limits are `n! zeta(n)`**, checked against
    /// [`crate::specfunc::zeta`], which shares nothing with this module.
    ///
    /// `J(2, inf) = 2 zeta(2) = pi^2/3` is the one most readers will
    /// recognise.
    #[test]
    fn the_limits_are_n_factorial_times_zeta() {
        let mut factorial = 1.0_f64;
        for n in 2..=5u32 {
            factorial *= n as f64;
            let Some(o) = order_of(n) else { unreachable!() };
            let expect = factorial * zeta::zeta(n as f64);
            assert!(
                ((o.val_infinity - expect) / expect).abs() < 1e-14,
                "J({n}, inf) = {} vs {n}! zeta({n}) = {expect}",
                o.val_infinity
            );
            // And the function reaches it, exactly.
            assert_eq!(transport(n, 100.0), o.val_infinity);
        }
        // The recognisable one.
        assert!(
            (transport(2, 1e3) - core::f64::consts::PI * core::f64::consts::PI / 3.0).abs() < 1e-14
        );
    }

    /// **The residual above is the quadrature's, not the integrals'**, and
    /// 2000 panels is that quadrature's measured optimum.
    ///
    /// Composite Simpson at `x = 3`, `n = 4`, worst relative against
    /// `transport`, measured 2026-09-19:
    ///
    /// | panels | residual |
    /// |---|---|
    /// | 20 | 8.924e-08 |
    /// | 100 | 1.439e-10 |
    /// | 200 | 8.995e-12 |
    /// | 500 | 2.308e-13 |
    /// | **2 000** | **8.934e-16** |
    /// | 200 000 | 9.381e-15 |
    ///
    /// Sixteen-fold per doubling is Simpson's fourth order, and the rise at
    /// 200 000 is `f64` roundoff over a hundred times as many terms. A first
    /// version of the accuracy test used 200 000 panels — past the optimum,
    /// and so measuring the reference's own noise. Three claims are asserted:
    /// the steep fall, the fourth-order rate, and the turnaround.
    #[test]
    fn the_residual_is_the_quadrature_not_the_integrals() {
        let x = 3.0_f64;
        let resid = |p: u32| ((transport(4, x) - simpson(4, x, p)) / transport(4, x)).abs();
        let coarse = resid(20);
        let fine = resid(500);
        assert!(
            coarse > 1000.0 * fine,
            "the residual is documented as the Simpson reference's rather \
             than J(4, .)'s, because it falls steeply with the panel count: \
             {coarse:e} at 20 panels against {fine:e} at 500"
        );
        // Simpson is fourth order, so each doubling should buy about 16x.
        assert!(
            resid(100) > 8.0 * resid(200),
            "the reference is documented as converging at Simpson's fourth \
             order (~16x per doubling): {:e} at 100 panels against {:e} at \
             200",
            resid(100),
            resid(200)
        );
        // And past about 2000 panels it RISES -- roundoff, not truncation,
        // which is why the comparison above uses 2000 and not more.
        assert!(
            resid(200_000) > resid(2_000),
            "the reference is documented as bottoming out near 2000 panels \
             and rising after: {:e} at 2e3 against {:e} at 2e5. If it is \
             still falling, the optimum has moved and the panel count the \
             accuracy test uses should move with it",
            resid(2_000),
            resid(200_000)
        );
    }

    /// Every branch boundary joins, compared as two **formulas** at one
    /// argument rather than as the function either side of the cut.
    #[test]
    fn the_branch_boundaries_join() {
        for n in 2..=5u32 {
            let Some(o) = order_of(n) else { unreachable!() };
            let nf = n as f64;

            // The small-argument power against the Chebyshev branch.
            let x = 3.0 * SQRT_DBL_EPSILON;
            let mut p = 1.0;
            for _ in 1..n {
                p *= x;
            }
            let small = p / (nf - 1.0);
            let t = (x * x / 8.0 - 0.5) - 0.5;
            let cheb = p * crate::cheb_slice::eval_gsl(t, o.cheb);
            assert!(
                ((small - cheb) / cheb).abs() < 1e-8,
                "J({n}) small/Chebyshev join at {x:e}: {small:e} vs {cheb:e}"
            );

            // The Chebyshev branch against the limit-minus-tail form, at 4.
            let x = 4.0_f64;
            let mut p = 1.0;
            for _ in 1..n {
                p *= x;
            }
            let t = (x * x / 8.0 - 0.5) - 0.5;
            let a = p * crate::cheb_slice::eval_gsl(t, o.cheb);
            let numexp = ((-LOG_DBL_EPSILON) / x) as u32 + 1;
            let s = sumexp(numexp, n, (-x).exp(), x);
            let b = o.val_infinity - (nf * x.ln() - x + s.ln()).exp();
            assert!(
                ((a - b) / b).abs() < 1e-10,
                "J({n}) Chebyshev/tail join at 4: {a} vs {b}"
            );

            // The two tail forms, at x = -LOG_DBL_EPSILON.
            let x = -LOG_DBL_EPSILON;
            let numexp = ((-LOG_DBL_EPSILON) / x) as u32 + 1;
            let a =
                o.val_infinity - (nf * x.ln() - x + sumexp(numexp, n, (-x).exp(), x).ln()).exp();
            let b = o.val_infinity - (nf * x.ln() - x + sumexp(1, n, 1.0, x).ln()).exp();
            assert!(
                ((a - b) / b).abs() < 1e-14,
                "J({n}) tail-form join at {x}: {a} vs {b}"
            );
        }
    }

    /// Each `J(n, .)` starts at zero, increases, and stays below its limit.
    #[test]
    fn every_order_is_monotone_below_its_limit() {
        // The measured points at which each J(n, .) reaches its limit
        // EXACTLY, because the tail has fallen below ln(DBL_EPSILON).
        const SATURATES_AT: [(u32, f64); 4] = [(2, 43.65), (3, 47.0), (4, 49.7), (5, 52.5)];

        for n in 2..=5u32 {
            let Some(o) = order_of(n) else { unreachable!() };
            assert_eq!(transport(n, 0.0), 0.0, "J({n}, 0)");
            let mut prev = 0.0_f64;
            for k in 1..=1200 {
                let x = 0.05 * k as f64;
                let v = transport(n, x);
                // NON-decreasing, not strictly increasing: once the tail
                // e^{n ln x - x} drops below one ulp of the limit, two
                // consecutive samples are the same f64. A first version of
                // this test asserted strict increase and failed at x = 39.85
                // on J(2) -- correctly.
                assert!(v >= prev, "J({n}) decreased at {x}: {v} < {prev}");
                assert!(v <= o.val_infinity, "J({n}, {x}) = {v} exceeds its limit");
                prev = v;
            }
            // It is strictly increasing while the tail is still resolvable.
            for k in 1..=600 {
                let x = 0.05 * k as f64;
                assert!(
                    transport(n, x + 0.05) > transport(n, x),
                    "J({n}) is documented as strictly increasing below x = 30, \
                     and is not at {x}"
                );
            }
        }

        // And each reaches its limit EXACTLY, at the measured point.
        for (n, sat) in SATURATES_AT {
            let Some(o) = order_of(n) else { unreachable!() };
            assert_eq!(
                transport(n, sat),
                o.val_infinity,
                "J({n}) is documented as reaching its limit exactly by x = {sat}"
            );
            assert!(
                transport(n, sat - 1.0) < o.val_infinity,
                "J({n}) is documented as still below its limit at x = {}, so \
                 the saturation point is a real boundary rather than an \
                 arbitrary large number",
                sat - 1.0
            );
        }
    }

    /// The refusals: a negative argument, `NaN`, and an untabulated order.
    #[test]
    fn the_refusals_are_right() {
        for n in 2..=5u32 {
            assert!(transport(n, -1.0).is_nan(), "J({n}, -1)");
            assert!(transport(n, -1e-9).is_nan(), "J({n}, -1e-9)");
            assert!(transport(n, f64::NAN).is_nan(), "J({n}, NaN)");
        }
        for n in [0_u32, 1, 6, 100] {
            assert!(transport(n, 1.0).is_nan(), "order {n}");
        }
        // The named entry points agree with the dispatcher.
        for k in 1..=40 {
            let x = 0.25 * k as f64;
            assert_eq!(transport_2(x), transport(2, x));
            assert_eq!(transport_3(x), transport(3, x));
            assert_eq!(transport_4(x), transport(4, x));
            assert_eq!(transport_5(x), transport(5, x));
        }
    }
}
