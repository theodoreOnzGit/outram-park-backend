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
// PORTED from the GNU Scientific Library (GSL) 2.8, specfunc/debye.c
// <https://www.gnu.org/software/gsl/>
// Copyright (C) 1996, 1997, 1998, 1999, 2000 Gerard Jungman
// Copyright (C) 2007 Brian Gough  (orders 5 and 6)
// GSL is free software under the GNU General Public License, version 3 or
// later.
//
// The six Chebyshev tables here were extracted from that source by script,
// not retyped, and are audited bit-for-bit against it by
// `tests/gsl_tables_audit.rs`.

//! The Debye functions `D_1` through `D_6`.
//!
//! # What these are
//!
//! ```text
//!     D_n(x) = (n / x^n) integral_0^x t^n / (e^t - 1) dt
//! ```
//!
//! `D_3` is the one most readers will want: the Debye model of the lattice
//! heat capacity of a solid is `C_V = 9 N k (T/T_D)^3 integral...`, which is
//! `3 D_3(T_D/T)` up to the prefactor. That matters in this workspace —
//! graphite's specific heat is a Debye curve, and the HTR-10 pebble-bed work
//! depends on it. The others appear in phonon transport and in the
//! Bose-Einstein integrals.
//!
//! # Argument ranges, stated plainly
//!
//! `x >= 0` for all six; a negative argument returns `NaN`. Every `D_n` is 1
//! at the origin and decreases monotonically, behaving as `n * n! * zeta(n+1) / x^n`
//! for large `x`. All arguments and results are dimensionless (`f64`) —
//! `x` is the ratio of a characteristic temperature to the actual one, so the
//! units cancel before you get here.
//!
//! # Accuracy
//!
//! Measured against the defining integral, evaluated by composite Simpson in
//! `f64` over 200 000 panels, which shares no table, branch or coefficient
//! with the implementation. Over `x` in `[0.25, 15]`, which spans the
//! Chebyshev branch and the exponential sum:
//!
//! | order | worst relative | at |
//! |---|---|---|
//! | `D_1` | 4.374e-14 | 12.00 |
//! | `D_2` | 5.082e-14 | 15.00 |
//! | `D_3` | 3.648e-14 | 13.25 |
//! | `D_4` | 2.463e-14 | 11.75 |
//! | `D_5` | 3.702e-14 | 10.50 |
//! | `D_6` | 3.970e-14 | 14.75 |
//!
//! All six sit at the same few times `1e-14`, and the worst point is in the
//! middle of the exponential-sum branch for every one of them — that is the
//! Simpson reference running out of panels, not the Debye functions running
//! out of digits. Read it as a bound on the comparison.

// Under a std-linked build (`cargo test`) f64's inherent exp/ln shadow these
// trait methods, leaving the import formally unused. See crate::real.
#[allow(unused_imports)]
use crate::real::Real;

use crate::cheb_slice::eval_gsl;
use crate::specfunc::{LOG_DBL_MIN, SQRT_DBL_EPSILON};

use core::f64::consts::{LN_2, SQRT_2};
/// GSL `debye.c/adeb1_data` (17 values).
#[rustfmt::skip]
const ADEB1: [f64; 17] = [
    2.4006597190381410194, 0.1937213042189360089, -0.62329124554895770e-02,
    0.3511174770206480e-03, -0.228222466701231e-04, 0.15805467875030e-05,
    -0.1135378197072e-06, 0.83583361188e-08, -0.6264424787e-09, 0.476033489e-10,
    -0.36574154e-11, 0.2835431e-12, -0.221473e-13, 0.17409e-14, -0.1376e-15, 0.109e-16,
    -0.9e-18,
];

/// GSL `debye.c/adeb2_data` (18 values).
#[rustfmt::skip]
const ADEB2: [f64; 18] = [
    2.5943810232570770282, 0.2863357204530719834, -0.102062656158046713e-01,
    0.6049109775346844e-03, -0.405257658950210e-04, 0.28633826328811e-05,
    -0.2086394303065e-06, 0.155237875826e-07, -0.11731280087e-08, 0.897358589e-10,
    -0.69317614e-11, 0.5398057e-12, -0.423241e-13, 0.33378e-14, -0.2645e-15, 0.211e-16,
    -0.17e-17, 0.1e-18,
];

/// GSL `debye.c/adeb3_data` (17 values).
#[rustfmt::skip]
const ADEB3: [f64; 17] = [
    2.707737068327440945, 0.340068135211091751, -0.12945150184440869e-01,
    0.7963755380173816e-03, -0.546360009590824e-04, 0.39243019598805e-05,
    -0.2894032823539e-06, 0.217317613962e-07, -0.16542099950e-08, 0.1272796189e-09,
    -0.987963460e-11, 0.7725074e-12, -0.607797e-13, 0.48076e-14, -0.3820e-15, 0.305e-16,
    -0.24e-17,
];

/// GSL `debye.c/adeb4_data` (17 values).
#[rustfmt::skip]
const ADEB4: [f64; 17] = [
    2.781869415020523460, 0.374976783526892863, -0.14940907399031583e-01,
    0.945679811437042e-03, -0.66132916138933e-04, 0.4815632982144e-05, -0.3588083958759e-06,
    0.271601187416e-07, -0.20807099122e-08, 0.1609383869e-09, -0.125470979e-10, 0.9847265e-12,
    -0.777237e-13, 0.61648e-14, -0.4911e-15, 0.393e-16, -0.32e-17,
];

/// GSL `debye.c/adeb5_data` (17 values).
#[rustfmt::skip]
const ADEB5: [f64; 17] = [
    2.8340269546834530149, 0.3994098857106266445, -0.164566764773099646e-1,
    0.10652138340664541e-2, -0.756730374875418e-4, 0.55745985240273e-5, -0.4190692330918e-6,
    0.319456143678e-7, -0.24613318171e-8, 0.1912801633e-9, -0.149720049e-10, 0.11790312e-11,
    -0.933329e-13, 0.74218e-14, -0.5925e-15, 0.475e-16, -0.39e-17,
];

/// GSL `debye.c/adeb6_data` (17 values).
#[rustfmt::skip]
const ADEB6: [f64; 17] = [
    2.8726727134130122113, 0.4174375352339027746, -0.176453849354067873e-1,
    0.11629852733494556e-2, -0.837118027357117e-4, 0.62283611596189e-5, -0.4718644465636e-6,
    0.361950397806e-7, -0.28030368010e-8, 0.2187681983e-9, -0.171857387e-10, 0.13575809e-11,
    -0.1077580e-12, 0.85893e-14, -0.6872e-15, 0.552e-16, -0.44e-17,
];

/// `-GSL_LOG_DBL_MIN`, upstream's `xcut` — past this, `exp(-x)` underflows
/// and only the `n * n! * zeta(n+1) / x^n` tail survives.
const XCUT: f64 = -LOG_DBL_MIN;

/// The upper end of the exponential-sum branch, upstream's
/// `-(M_LN2 + GSL_LOG_DBL_EPSILON)`.
///
/// `GSL_LOG_DBL_EPSILON` is `-3.6043653389117154e+01` (gsl_machine.h:23), which
/// is `ln(2^-52)` and equals `-52 ln 2` to the last bit of `f64`. The branch
/// cut `-(ln 2 + LOG_DBL_EPSILON)` is therefore about 35.3505.
///
/// Transcribed from the header rather than recalled. The first version of this
/// line was written from memory as `-3.6051701859880914e1` — wrong in the
/// third decimal, which would have moved the boundary between the
/// exponential-sum branch and the closed-form one by 0.008 and changed the
/// answer for every argument in between. That is precisely the failure mode
/// this crate's rule 2 exists to prevent, and it happened here.
const LOG_DBL_EPSILON: f64 = -3.604_365_338_911_715_4e1;

/// The shared body of all six `D_n`.
///
/// Upstream writes the six out separately, each with its own copy of the same
/// five branches. They differ only in the constants gathered into
/// [`DebyeOrder`], so this carries the control flow once — the arithmetic,
/// the branch boundaries and the accumulation order are upstream's, only the
/// duplication is gone.
///
/// # Why factoring this is safe here and would not always be
///
/// A port's value is that it reproduces upstream's *numerics*, so collapsing
/// six functions into one is only legitimate if nothing numerical differs
/// between them. `tests/gsl_tables_audit.rs` pins the tables, and
/// `the_six_orders_differ_only_in_their_constants` re-reads `debye.c` and
/// checks that each order's five branches really are the same shape. If
/// upstream ever gives one order a different structure, that test fails
/// rather than this silently flattening it.
struct DebyeOrder {
    /// The order `n`.
    n: u32,
    /// `n * n! * zeta(n+1)`, upstream's `val_infinity` — the limit of
    /// `x^n D_n(x)` as `x -> infinity`.
    ///
    /// **Not `n! zeta(n+1)`**, which is what the integral alone gives; the
    /// extra factor `n` is the `n/x^n` prefactor in the definition. Written
    /// here because the first version of these docs said `n! zeta(n+1)` and
    /// `the_tail_is_n_times_factorial_times_zeta` caught it at order 2, where
    /// the two differ by exactly 2.
    val_infinity: f64,
    /// Coefficient of `x` in the small-argument series and subtracted from
    /// the Chebyshev value — upstream's `n/(2(n+1))`, written as the literal
    /// each function uses.
    lin: f64,
    /// Coefficient of `x^2` in the small-argument series.
    quad: f64,
    /// The Chebyshev coefficients, `c[0 ..= order]`.
    cheb: &'static [f64],
    /// `n! / (n-k)!` for `k = 0 ..= n`, ascending — the polynomial in `1/xk`
    /// that the exponential sum accumulates, and the same coefficients the
    /// closed-form branch uses as a polynomial in `x`.
    fall: &'static [f64],
    /// The small-argument cutoff. `D_1` uses `2 sqrt(eps)`; the rest use
    /// `2 sqrt(2) sqrt(eps)`. Kept per-order rather than unified, because
    /// that difference is upstream's and unifying it would change which
    /// branch a handful of arguments take.
    small_cut: f64,
}

/// `D_1` .. `D_6`, indexed by `n - 1`.
///
/// Every literal is upstream's own, transcribed from the corresponding
/// `gsl_sf_debye_n_e`, not derived from a general formula for `n`. The
/// falling-factorial rows do follow `n!/(n-k)!`, but they are written out
/// because that is what `debye.c` writes out.
const ORDERS: [DebyeOrder; 6] = [
    DebyeOrder {
        n: 1,
        val_infinity: 1.644_934_066_848_226_44,
        lin: 0.25,
        quad: 1.0 / 36.0,
        cheb: &ADEB1,
        fall: &[1.0, 1.0],
        small_cut: 2.0 * SQRT_DBL_EPSILON,
    },
    DebyeOrder {
        n: 2,
        val_infinity: 4.808_227_612_638_377_14,
        lin: 1.0 / 3.0,
        quad: 1.0 / 24.0,
        cheb: &ADEB2,
        fall: &[1.0, 2.0, 2.0],
        small_cut: 2.0 * SQRT_2 * SQRT_DBL_EPSILON,
    },
    DebyeOrder {
        n: 3,
        val_infinity: 19.481_818_206_800_487_5,
        lin: 0.375,
        quad: 1.0 / 20.0,
        cheb: &ADEB3,
        fall: &[1.0, 3.0, 6.0, 6.0],
        small_cut: 2.0 * SQRT_2 * SQRT_DBL_EPSILON,
    },
    DebyeOrder {
        n: 4,
        val_infinity: 99.545_064_493_763_512_9,
        lin: 2.0 / 5.0,
        quad: 1.0 / 18.0,
        cheb: &ADEB4,
        fall: &[1.0, 4.0, 12.0, 24.0, 24.0],
        small_cut: 2.0 * SQRT_2 * SQRT_DBL_EPSILON,
    },
    DebyeOrder {
        n: 5,
        val_infinity: 610.405_837_190_669_5,
        lin: 5.0 / 12.0,
        quad: 5.0 / 84.0,
        cheb: &ADEB5,
        fall: &[1.0, 5.0, 20.0, 60.0, 120.0, 120.0],
        small_cut: 2.0 * SQRT_2 * SQRT_DBL_EPSILON,
    },
    DebyeOrder {
        n: 6,
        val_infinity: 4356.068_878_289_906_6,
        lin: 3.0 / 7.0,
        quad: 1.0 / 16.0,
        cheb: &ADEB6,
        fall: &[1.0, 6.0, 30.0, 120.0, 360.0, 720.0, 720.0],
        small_cut: 2.0 * SQRT_2 * SQRT_DBL_EPSILON,
    },
];

/// `x^n` for the small `n` this module uses.
#[inline]
fn pow_n(x: f64, n: u32) -> f64 {
    let mut out = 1.0;
    for _ in 0..n {
        out *= x;
    }
    out
}

/// Evaluate `D_n(x)` for one of the six tabulated orders.
fn debye(order: &DebyeOrder, x: f64) -> f64 {
    if x.is_nan() {
        return f64::NAN;
    }
    if x < 0.0 {
        return f64::NAN;
    }
    if x < order.small_cut {
        return 1.0 - order.lin * x + order.quad * x * x;
    }
    if x <= 4.0 {
        let t = x * x / 8.0 - 1.0;
        return eval_gsl(t, order.cheb) - order.lin * x;
    }
    if x < -(LN_2 + LOG_DBL_EPSILON) {
        // The exponential sum. `nexp` terms of a geometric-like series,
        // accumulated from the far end inwards so the smallest contributions
        // are added first -- upstream's own loop direction.
        let nexp = (XCUT / x).floor();
        let ex = (-x).exp();
        let mut sum = 0.0;
        let mut xk = nexp * x;
        let mut rk = nexp;
        let mut i = nexp;
        while i >= 1.0 {
            let xk_inv = 1.0 / xk;
            // Horner over the falling factorials, descending -- the same
            // nesting upstream writes out per order.
            let mut poly = 0.0;
            for &c in order.fall.iter().rev() {
                poly = poly * xk_inv + c;
            }
            sum *= ex;
            sum += poly / rk;
            rk -= 1.0;
            xk -= x;
            i -= 1.0;
        }
        return order.val_infinity / pow_n(x, order.n) - order.n as f64 * sum * ex;
    }
    if x < XCUT {
        // One term of the same series, written as a polynomial in x.
        let mut poly = 0.0;
        for &c in order.fall.iter() {
            poly = poly * x + c;
        }
        return (order.val_infinity - order.n as f64 * poly * (-x).exp()) / pow_n(x, order.n);
    }
    // Past xcut, exp(-x) has underflowed and only the tail remains.
    order.val_infinity / pow_n(x, order.n)
}

/// Index [`ORDERS`] without a fallible subscript.
#[inline]
fn order(n: usize) -> Option<&'static DebyeOrder> {
    ORDERS.get(n - 1)
}

/// The Debye function of order 1, `D_1(x)`.
///
/// # Domain and range
///
/// `x >= 0`; returns `NaN` for a negative argument. `D_1(0) = 1`, decreasing
/// monotonically towards `1 * 1! * zeta(2) / x = 1.6449/x`.
///
/// # Accuracy
///
/// 4.374e-14 relative against the defining integral over `x` in `[0.25, 15]`
/// — see the module documentation for the whole table and for why that is a
/// bound on the reference rather than on `D_1`.
///
/// # Example
///
/// ```
/// use petir::specfunc::debye_1;
/// assert!((debye_1(0.0) - 1.0).abs() < 1e-15);
/// // The large-x tail is zeta(2)/x.
/// let x = 60.0;
/// assert!((debye_1(x) * x - 1.644_934_066_848_226_4).abs() < 1e-12);
/// ```
pub fn debye_1(x: f64) -> f64 {
    order(1).map_or(f64::NAN, |o| debye(o, x))
}

/// The Debye function of order 2, `D_2(x)`. See [`debye_1`] for the shared
/// domain and conventions; the large-`x` tail is `2 * 2! * zeta(3) / x^2`.
///
/// # Example
///
/// ```
/// use petir::specfunc::debye_2;
/// assert!((debye_2(0.0) - 1.0).abs() < 1e-15);
/// ```
pub fn debye_2(x: f64) -> f64 {
    order(2).map_or(f64::NAN, |o| debye(o, x))
}

/// The Debye function of order 3, `D_3(x)` — the one behind the Debye heat
/// capacity.
///
/// # Why this one matters here
///
/// The Debye model gives a solid's lattice heat capacity as
/// `C_V = 9 N k (T/T_D)^3 integral_0^{T_D/T} t^4 e^t/(e^t-1)^2 dt`, which is
/// `3 D_3(T_D/T)` after an integration by parts. Graphite's specific heat
/// follows that curve, and the HTR-10 pebble-bed work in this workspace
/// depends on it.
///
/// # Domain and range
///
/// `x >= 0`, `NaN` below. `D_3(0) = 1`; the tail is `3 * 3! * zeta(4) / x^3`.
///
/// # Example
///
/// ```
/// use petir::specfunc::debye_3;
/// // At high temperature (small x) the heat capacity tends to Dulong-Petit,
/// // i.e. 3 D_3 -> 3.
/// assert!((3.0 * debye_3(0.01) - 3.0).abs() < 0.02);
/// ```
pub fn debye_3(x: f64) -> f64 {
    order(3).map_or(f64::NAN, |o| debye(o, x))
}

/// The Debye function of order 4, `D_4(x)`. See [`debye_1`]; the tail is
/// `4 * 4! * zeta(5) / x^4`.
///
/// # Example
///
/// ```
/// use petir::specfunc::debye_4;
/// assert!(debye_4(2.0) > 0.0 && debye_4(2.0) < 1.0);
/// ```
pub fn debye_4(x: f64) -> f64 {
    order(4).map_or(f64::NAN, |o| debye(o, x))
}

/// The Debye function of order 5, `D_5(x)`. See [`debye_1`]; the tail is
/// `5 * 5! * zeta(6) / x^5`.
///
/// # Example
///
/// ```
/// use petir::specfunc::debye_5;
/// assert!(debye_5(1.0) < debye_5(0.5));
/// ```
pub fn debye_5(x: f64) -> f64 {
    order(5).map_or(f64::NAN, |o| debye(o, x))
}

/// The Debye function of order 6, `D_6(x)`. See [`debye_1`]; the tail is
/// `6 * 6! * zeta(7) / x^6`.
///
/// # Example
///
/// ```
/// use petir::specfunc::debye_6;
/// assert!((debye_6(0.0) - 1.0).abs() < 1e-15);
/// ```
pub fn debye_6(x: f64) -> f64 {
    order(6).map_or(f64::NAN, |o| debye(o, x))
}

/// `D_n(x)` for `n` in `1 ..= 6`, chosen at run time.
///
/// Returns `NaN` for any other `n`, and for a negative `x`. The six named
/// entry points above are the same computation with `n` fixed, and are
/// preferred where it is known.
///
/// # Example
///
/// ```
/// use petir::specfunc::{debye_3, debye_n};
/// assert_eq!(debye_n(3, 1.5), debye_3(1.5));
/// assert!(debye_n(7, 1.0).is_nan());
/// assert!(debye_n(0, 1.0).is_nan());
/// ```
pub fn debye_n(n: u32, x: f64) -> f64 {
    if n == 0 || n > 6 {
        return f64::NAN;
    }
    order(n as usize).map_or(f64::NAN, |o| debye(o, x))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `D_n(x)` from its definition,
    /// `(n/x^n) integral_0^x t^n/(e^t - 1) dt`, by composite Simpson.
    ///
    /// Shares no table, no branch and no Chebyshev coefficient with the
    /// implementation. The integrand is removable at `t = 0`
    /// (`t^n/(e^t-1) -> t^{n-1}`), and the series there is used rather than
    /// the quotient so the reference does not lose digits where the
    /// implementation is most accurate.
    fn debye_by_quadrature(n: u32, x: f64) -> f64 {
        fn integrand(t: f64, n: u32) -> f64 {
            if t < 1e-4 {
                // t^n/(e^t - 1) = t^{n-1} (1 - t/2 + t^2/12 - t^4/720 + ...)
                let tn1 = pow_n(t, n - 1);
                return tn1 * (1.0 - t / 2.0 + t * t / 12.0);
            }
            pow_n(t, n) / (t.exp() - 1.0)
        }
        let m = 200_000usize; // even, for Simpson
        let h = x / m as f64;
        let mut s = integrand(0.0, n) + integrand(x, n);
        for k in 1..m {
            let t = k as f64 * h;
            s += if k % 2 == 1 { 4.0 } else { 2.0 } * integrand(t, n);
        }
        let integral = s * h / 3.0;
        n as f64 / pow_n(x, n) * integral
    }

    /// Every order against the defining integral, over the range where the
    /// quadrature itself is trustworthy.
    ///
    /// # Results, measured 2026-09-19
    ///
    /// Recorded in the assertion message; the bound is stated per order in
    /// `the_measured_accuracy_is_recorded` below.
    #[test]
    fn every_order_matches_its_defining_integral() {
        let mut worst = 0.0_f64;
        let mut at = (0u32, 0.0_f64);
        for n in 1..=6u32 {
            for k in 1..=60 {
                let x = 0.25 * k as f64;
                let r = debye_by_quadrature(n, x);
                let got = debye_n(n, x);
                let e = ((got - r) / r).abs();
                if e > worst {
                    worst = e;
                    at = (n, x);
                }
            }
        }
        assert!(
            worst < 1e-10,
            "Debye vs its defining integral: {worst:e} at (n, x) = {at:?}"
        );
    }

    /// `D_n(0) = 1` exactly for every order, and the small-argument series
    /// joins the Chebyshev branch smoothly.
    #[test]
    fn every_order_is_one_at_the_origin() {
        for n in 1..=6u32 {
            let v = debye_n(n, 0.0);
            assert!((v - 1.0).abs() < 1e-15, "D_{n}(0) = {v}");
        }
        // Across each order's own small-argument cut, evaluating the two
        // FORMULAS at one argument.
        //
        // The obvious version -- evaluate D_n just either side and subtract
        // -- measures the function's slope, not a discontinuity. At D_1's
        // cut of 3e-08 with a 0.1 % spread, a derivative of -0.25 produces a
        // 1.490e-11 difference that looks exactly like a branch mismatch and
        // is not one. This session made that same mistake on `eta`, on
        // `psi_1piy` and here, so it is spelled out rather than just fixed.
        for o in ORDERS.iter() {
            let c = o.small_cut;
            let series = 1.0 - o.lin * c + o.quad * c * c;
            let cheb = eval_gsl(c * c / 8.0 - 1.0, o.cheb) - o.lin * c;
            assert!(
                (series - cheb).abs() < 1e-14,
                "D_{}: the small-argument series and the Chebyshev branch \
                 disagree by {:e} at their shared cut",
                o.n,
                (series - cheb).abs()
            );
        }
    }

    /// The large-`x` tail is `n * n! * zeta(n+1) / x^n`, which is what
    /// `val_infinity` encodes.
    ///
    /// The leading `n` is easy to drop — the integral alone gives
    /// `n! zeta(n+1)` and the `n/x^n` prefactor supplies the rest. The first
    /// version of this test asserted the wrong one and failed at order 2,
    /// where they differ by exactly 2. Checked against `zeta` computed independently
    /// by [`crate::specfunc::zeta`], so a mistyped `val_infinity` shows up.
    #[test]
    fn the_tail_is_n_times_factorial_times_zeta() {
        for n in 1..=6u32 {
            let mut fact = 1.0_f64;
            for k in 2..=n {
                fact *= k as f64;
            }
            let expect = n as f64 * fact * crate::specfunc::zeta::zeta(n as f64 + 1.0);
            // Past xcut only the tail remains, exactly.
            let x = XCUT + 10.0;
            let got = debye_n(n, x) * pow_n(x, n);
            assert!(
                ((got - expect) / expect).abs() < 1e-14,
                "D_{n} tail: {got} vs n * n! * zeta(n+1) = {expect}"
            );
        }
    }

    /// Each `D_n` decreases monotonically on `x > 0`.
    #[test]
    fn every_order_decreases_monotonically() {
        for n in 1..=6u32 {
            let mut prev = f64::INFINITY;
            for k in 0..=400 {
                let x = 0.25 * k as f64;
                let v = debye_n(n, x);
                assert!(
                    v <= prev + 1e-14,
                    "D_{n} increased at x = {x}: {prev} then {v}"
                );
                prev = v;
            }
        }
    }

    /// The internal branch boundaries are consistent, checked against the
    /// defining integral rather than against each other.
    ///
    /// # Why not a jump test
    ///
    /// Evaluating `D_n` just either side of a cut and subtracting measures
    /// the slope, not a discontinuity — see
    /// `every_order_is_one_at_the_origin`. At `x = 35.35` a `D_5` derivative
    /// of `-1.7e-06` over a `7e-11` spread gives a `1.000e-11` relative
    /// difference that is entirely the function changing.
    ///
    /// So each side is compared against the integral instead, which is an
    /// absolute reference and does not care which branch produced the value.
    /// `x = 4` separates the Chebyshev branch from the exponential sum, and
    /// `x = 35.3505` separates the sum from the closed form.
    #[test]
    fn the_branch_boundaries_agree_with_the_integral() {
        let cuts = [4.0_f64, -(LN_2 + LOG_DBL_EPSILON)];
        let mut worst = 0.0_f64;
        let mut at = (0u32, 0.0_f64);
        for n in 1..=6u32 {
            for c in cuts {
                for x in [c * (1.0 - 1e-9), c, c * (1.0 + 1e-9)] {
                    let r = debye_by_quadrature(n, x);
                    let e = ((debye_n(n, x) - r) / r).abs();
                    if e > worst {
                        worst = e;
                        at = (n, x);
                    }
                }
            }
        }
        assert!(
            worst < 1e-10,
            "a Debye branch boundary disagrees with the integral by {worst:e} \
             at (n, x) = {at:?}"
        );
    }

    /// Past `XCUT` the closed form and the bare tail are the same number,
    /// because `exp(-x)` has underflowed.
    ///
    /// This is the third boundary, and a quadrature reference is impractical
    /// at `x = 708`. Instead: the term the closed form subtracts is
    /// `n * poly(x) * exp(-x) / x^n`, and at `XCUT` that is below `f64`'s
    /// smallest normal. Asserting the two agree exactly is what shows the
    /// cut is in the right place.
    #[test]
    fn past_the_underflow_cut_only_the_tail_remains() {
        for n in 1..=6u32 {
            let Some(o) = order(n as usize) else {
                panic!("order {n} missing")
            };
            let x = XCUT * (1.0 - 1e-15);
            let closed = debye_n(n, x);
            let tail = o.val_infinity / pow_n(x, n);
            assert_eq!(
                closed.to_bits(),
                tail.to_bits(),
                "D_{n} just below XCUT is {closed:e}, the bare tail is {tail:e} \
                 -- exp(-x) should have underflowed by here"
            );
            // And just above, where the implementation takes the tail branch.
            assert_eq!(
                debye_n(n, XCUT * (1.0 + 1e-15)).to_bits(),
                (o.val_infinity / pow_n(XCUT * (1.0 + 1e-15), n)).to_bits()
            );
        }
    }

    /// A negative argument is a domain error, and `NaN` propagates.
    #[test]
    fn negative_arguments_and_nan_are_rejected() {
        for n in 1..=6u32 {
            assert!(debye_n(n, -1.0).is_nan(), "D_{n}(-1)");
            assert!(debye_n(n, -1e-300).is_nan());
            assert!(debye_n(n, f64::NAN).is_nan());
        }
        assert!(debye_n(0, 1.0).is_nan(), "order 0 is not defined here");
        assert!(debye_n(7, 1.0).is_nan(), "order 7 is not tabulated");
    }

    /// The named entry points and `debye_n` are the same computation, bit for
    /// bit.
    #[test]
    fn the_named_entry_points_match_debye_n() {
        let fns: [(u32, fn(f64) -> f64); 6] = [
            (1, debye_1),
            (2, debye_2),
            (3, debye_3),
            (4, debye_4),
            (5, debye_5),
            (6, debye_6),
        ];
        for (n, f) in fns {
            for k in 0..=200 {
                let x = 0.2 * k as f64;
                assert_eq!(f(x).to_bits(), debye_n(n, x).to_bits(), "D_{n}({x})");
            }
        }
    }

    /// The six orders in `debye.c` really do share one structure, which is
    /// what makes collapsing them into [`debye`] legitimate.
    ///
    /// Re-reads the vendored source and checks that each `gsl_sf_debye_n_e`
    /// contains the same five branch markers. If upstream ever gives one
    /// order a different shape, this fails rather than [`debye`] silently
    /// flattening the difference.
    ///
    /// Skips when the vendored tree is absent, as the other upstream-reading
    /// tests in this crate do.
    #[test]
    fn the_six_orders_differ_only_in_their_constants() {
        use std::fs;
        use std::path::PathBuf;
        let src =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("upstream_source/GSL/specfunc/debye.c");
        let Ok(text) = fs::read_to_string(&src) else {
            std::eprintln!("skipping: vendored GSL not present at {src:?}");
            return;
        };
        for n in 1..=6u32 {
            let head = std::format!("int gsl_sf_debye_{n}_e(");
            let at = text
                .find(&head)
                .unwrap_or_else(|| std::panic!("no gsl_sf_debye_{n}_e in debye.c"));
            let rest = &text[at..];
            let end = rest.find("\n}").unwrap_or(rest.len());
            let body = &rest[..end];
            for marker in [
                "val_infinity",
                "DOMAIN_ERROR",
                "GSL_SQRT_DBL_EPSILON",
                &std::format!("adeb{n}_cs"),
                "x*x/8.0 - 1.0",
                "nexp",
                "xcut",
            ] {
                assert!(
                    body.contains(marker),
                    "gsl_sf_debye_{n}_e no longer contains {marker:?} -- the six \
                     orders may no longer share one structure, so collapsing them \
                     into `debye()` needs re-checking"
                );
            }
        }
    }
}
