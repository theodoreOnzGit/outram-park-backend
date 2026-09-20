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
// PORTED from the GNU Scientific Library (GSL) 2.8, specfunc/sinint.c
// <https://www.gnu.org/software/gsl/>
// Copyright (C) 1996-2000 Gerard Jungman
// GSL is free software under the GNU General Public License, version 3 or
// later.
//
// GSL's own lineage for the two small-argument fits is SLATEC's `si.f` and
// `ci.f` by W. Fullerton. All six Chebyshev tables were extracted from that
// source by script, not retyped, and are audited bit-for-bit by
// tests/gsl_tables_audit.rs.

//! The sine and cosine integrals `Si(x)` and `Ci(x)`.
//!
//! # What these are
//!
//! ```text
//!     Si(x) = integral_0^x sin(t)/t dt
//!     Ci(x) = -integral_x^inf cos(t)/t dt
//!           = gamma + ln x + integral_0^x (cos t - 1)/t dt
//! ```
//!
//! the pair that describes the diffraction of a wave at a straight edge, and
//! the Fourier-domain counterpart of [`crate::specfunc::atanint`]'s `Ti_2`.
//! `Si` rises to `pi/2` with a decaying oscillation; `Ci` starts at
//! `-infinity` with a logarithmic singularity, crosses zero at
//! `x ~ 0.6165`, and oscillates towards zero.
//!
//! # The asymptotic pair `f` and `g`
//!
//! Past `x = 4` both are written in terms of two auxiliary functions,
//!
//! ```text
//!     Si(x) = pi/2 - f(x) cos x - g(x) sin x
//!     Ci(x) =        f(x) sin x - g(x) cos x
//! ```
//!
//! with `f ~ 1/x` and `g ~ 1/x^2`. This is not a convenience: it moves the
//! oscillation entirely into the `sin`/`cos` factors, so the parts that are
//! fitted are smooth and monotone. Four of the six tables here belong to
//! `f` and `g` — two apiece, split at `sqrt(50)` — and only two to the
//! small-argument forms.
//!
//! # Argument range, stated plainly
//!
//! `Si`: **all real `x`**, dimensionless (`f64`), odd, bounded by about
//! 1.8519. `Ci`: **`x > 0`** only — `x <= 0` is upstream's `DOMAIN_ERROR`
//! and returns `NaN`, since `Ci` has a branch cut along the negative axis.
//! Neither overflows. `NaN` propagates.
//!
//! # Accuracy
//!
//! Measured against the **defining integrals** by composite Gauss-Legendre,
//! against the exact limits `Si(inf) = pi/2` and the first zero of `Ci`, and
//! against the derivative identities `Si'(x) = sin(x)/x` and
//! `Ci'(x) = cos(x)/x`. Results are in the tests.

// Under a std-linked build (`cargo test`) f64's inherent abs/ln/sin/cos
// shadow these trait methods, leaving the import formally unused. See
// crate::real.
#[allow(unused_imports)]
use crate::real::Real;

use crate::cheb_slice::eval_gsl as cheb;
use crate::specfunc::{SQRT_DBL_EPSILON, SQRT_DBL_MIN};

/// `sqrt(50)`, where `f` and `g` change fit. Upstream writes the rounded
/// decimal rather than the square root, and it is carried verbatim because
/// it is a branch boundary: rounding it differently moves which fit runs.
const XBND: f64 = 7.071_067_811_87;
/// `1/GSL_SQRT_DBL_EPSILON`, past which `f = 1/x` and `g = 1/x^2` exactly.
const XBIG: f64 = 1.0 / SQRT_DBL_EPSILON;
/// `1/GSL_DBL_MIN`, past which even `1/x` underflows.
const XMAXF: f64 = 1.0 / f64::MIN_POSITIVE;
/// `1/GSL_SQRT_DBL_MIN`, past which `1/x^2` underflows.
const XMAXG: f64 = 1.0 / SQRT_DBL_MIN;

/// GSL's `f1_data`, 20 coefficients on `[-1, 1]`.
#[rustfmt::skip]
const F1: [f64; 20] = [
    -0.1191081969051363610, -0.0247823144996236248, 0.0011910281453357821,
    -0.0000927027714388562, 0.0000093373141568271, -0.0000011058287820557,
    0.0000001464772071460, -0.0000000210694496288, 0.0000000032293492367,
    -0.0000000005206529618, 0.0000000000874878885, -0.0000000000152176187,
    0.0000000000027257192, -0.0000000000005007053, 0.0000000000000940241,
    -0.0000000000000180014, 0.0000000000000035063, -0.0000000000000006935,
    0.0000000000000001391, -0.0000000000000000282,
];

/// GSL's `f2_data`, 29 coefficients on `[-1, 1]`.
#[rustfmt::skip]
const F2: [f64; 29] = [
    -0.0348409253897013234, -0.0166842205677959686, 0.0006752901241237738,
    -0.0000535066622544701, 0.0000062693421779007, -0.0000009526638801991,
    0.0000001745629224251, -0.0000000368795403065, 0.0000000087202677705,
    -0.0000000022601970392, 0.0000000006324624977, -0.0000000001888911889,
    0.0000000000596774674, -0.0000000000198044313, 0.0000000000068641396,
    -0.0000000000024731020, 0.0000000000009226360, -0.0000000000003552364,
    0.0000000000001407606, -0.0000000000000572623, 0.0000000000000238654,
    -0.0000000000000101714, 0.0000000000000044259, -0.0000000000000019634,
    0.0000000000000008868, -0.0000000000000004074, 0.0000000000000001901,
    -0.0000000000000000900, 0.0000000000000000432,
];

/// GSL's `g1_data`, 21 coefficients on `[-1, 1]`.
#[rustfmt::skip]
const G1: [f64; 21] = [
    -0.3040578798253495954, -0.0566890984597120588, 0.0039046158173275644,
    -0.0003746075959202261, 0.0000435431556559844, -0.0000057417294453025,
    0.0000008282552104503, -0.0000001278245892595, 0.0000000207978352949,
    -0.0000000035313205922, 0.0000000006210824236, -0.0000000001125215474,
    0.0000000000209088918, -0.0000000000039715832, 0.0000000000007690431,
    -0.0000000000001514697, 0.0000000000000302892, -0.0000000000000061400,
    0.0000000000000012601, -0.0000000000000002615, 0.0000000000000000548,
];

/// GSL's `g2_data`, 34 coefficients on `[-1, 1]`.
#[rustfmt::skip]
const G2: [f64; 34] = [
    -0.0967329367532432218, -0.0452077907957459871, 0.0028190005352706523,
    -0.0002899167740759160, 0.0000407444664601121, -0.0000071056382192354,
    0.0000014534723163019, -0.0000003364116512503, 0.0000000859774367886,
    -0.0000000238437656302, 0.0000000070831906340, -0.0000000022318068154,
    0.0000000007401087359, -0.0000000002567171162, 0.0000000000926707021,
    -0.0000000000346693311, 0.0000000000133950573, -0.0000000000053290754,
    0.0000000000021775312, -0.0000000000009118621, 0.0000000000003905864,
    -0.0000000000001708459, 0.0000000000000762015, -0.0000000000000346151,
    0.0000000000000159996, -0.0000000000000075213, 0.0000000000000035970,
    -0.0000000000000017530, 0.0000000000000008738, -0.0000000000000004487,
    0.0000000000000002397, -0.0000000000000001347, 0.0000000000000000801,
    -0.0000000000000000501,
];

/// GSL's `si_data`, 12 coefficients on `[-1, 1]`.
#[rustfmt::skip]
const SI: [f64; 12] = [
    -0.1315646598184841929, -0.2776578526973601892, 0.0354414054866659180,
    -0.0025631631447933978, 0.0001162365390497009, -0.0000035904327241606,
    0.0000000802342123706, -0.0000000013562997693, 0.0000000000179440722,
    -0.0000000000001908387, 0.0000000000000016670, -0.0000000000000000122,
];

/// GSL's `ci_data`, 13 coefficients on `[-1, 1]`.
#[rustfmt::skip]
const CI: [f64; 13] = [
    -0.34004281856055363156, -1.03302166401177456807, 0.19388222659917082877,
    -0.01918260436019865894, 0.00110789252584784967, -0.00004157234558247209,
    0.00000109278524300229, -0.00000002123285954183, 0.00000000031733482164,
    -0.00000000000376141548, 0.00000000000003622653, -0.00000000000000028912,
    0.00000000000000000194,
];

/// The asymptotic pair `(f, g)` for `x >= 4`, GSL's `fg_asymp`.
///
/// `f ~ 1/x` and `g ~ 1/x^2`, both smooth and monotone — the oscillation of
/// `Si` and `Ci` lives entirely in the `sin`/`cos` factors that multiply
/// them, which is why these are fittable at all.
///
/// Three regimes: the `f1`/`g1` fits in `(1/x^2 - 0.04125)/0.02125` up to
/// `sqrt(50)`, the `f2`/`g2` fits in `100/x^2 - 1` up to [`XBIG`], and the
/// bare asymptotes beyond, each guarded against its own underflow.
fn fg_asymp(x: f64) -> (f64, f64) {
    let x2 = x * x;
    if x <= XBND {
        let t = (1.0 / x2 - 0.04125) / 0.02125;
        ((1.0 + cheb(t, &F1)) / x, (1.0 + cheb(t, &G1)) / x2)
    } else if x <= XBIG {
        let t = 100.0 / x2 - 1.0;
        ((1.0 + cheb(t, &F2)) / x, (1.0 + cheb(t, &G2)) / x2)
    } else {
        // Each asymptote is guarded separately: 1/x survives much further
        // than 1/x^2, so upstream uses two different bounds rather than one.
        (
            if x < XMAXF { 1.0 / x } else { 0.0 },
            if x < XMAXG { 1.0 / x2 } else { 0.0 },
        )
    }
}

/// `Si(x) = integral_0^x sin(t)/t dt`, GSL's `gsl_sf_Si`.
///
/// `x` is dimensionless (`f64`), any real value. `Si` is odd and bounded by
/// about 1.8519 (its maximum, at `x = pi`), rising to `pi/2` as
/// `x -> infinity`. `NaN` propagates.
///
/// # Examples
///
/// ```
/// use petir::specfunc::sinint::si;
/// assert_eq!(si(0.0), 0.0);
/// assert_eq!(si(-2.0), -si(2.0));
/// // The Gibbs overshoot: Si peaks above pi/2 at x = pi.
/// assert!(si(core::f64::consts::PI) > core::f64::consts::FRAC_PI_2);
/// // And settles on pi/2.
/// assert!((si(1e6) - core::f64::consts::FRAC_PI_2).abs() < 1e-6);
/// ```
pub fn si(x: f64) -> f64 {
    if x.is_nan() {
        return f64::NAN;
    }
    let ax = x.abs();
    if ax < SQRT_DBL_EPSILON {
        x
    } else if ax <= 4.0 {
        // The 0.75 is carried outside the fit, as in GSL.
        x * (0.75 + cheb((x * x - 8.0) * 0.125, &SI))
    } else {
        let (f, g) = fg_asymp(ax);
        // No loss of precision here despite the subtraction: the leading
        // pi/2 dominates, and f, g are both O(1/x).
        let v = core::f64::consts::FRAC_PI_2 - f * ax.cos() - g * ax.sin();
        if x < 0.0 {
            -v
        } else {
            v
        }
    }
}

/// `Ci(x) = -integral_x^inf cos(t)/t dt`, GSL's `gsl_sf_Ci`.
///
/// `x` is dimensionless (`f64`) and must be **strictly positive**: `Ci` has a
/// logarithmic singularity at the origin and a branch cut along the negative
/// axis, so `x <= 0` is upstream's `DOMAIN_ERROR` and returns `NaN`.
///
/// `Ci` crosses zero once on `(0, 4]`, near `x = 0.616505`, and thereafter
/// oscillates towards zero with amplitude `~1/x`.
///
/// # Examples
///
/// ```
/// use petir::specfunc::sinint::ci;
/// assert!(ci(0.0).is_nan());
/// assert!(ci(-1.0).is_nan());
/// // Negative just below its first zero, positive just above.
/// assert!(ci(0.6) < 0.0 && ci(0.7) > 0.0);
/// // And decays.
/// assert!(ci(1e6).abs() < 1e-6);
/// ```
pub fn ci(x: f64) -> f64 {
    if x.is_nan() || x <= 0.0 {
        // Upstream's DOMAIN_ERROR.
        return f64::NAN;
    }
    if x <= 4.0 {
        let lx = x.ln();
        // The ln x carries the singularity; the fit carries the rest, so
        // neither has to represent the other's behaviour.
        lx - 0.5 + cheb((x * x - 8.0) * 0.125, &CI)
    } else {
        let (f, g) = fg_asymp(x);
        f * x.sin() - g * x.cos()
    }
}

#[cfg(test)]
mod tests {
    extern crate std;
    use super::*;

    const EULER: f64 = 0.577_215_664_901_532_860_6;

    /// Composite 30-point Gauss-Legendre, shared by both references below.
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

    /// **Both defining integrals are reproduced** — `Si` to 1.0e-14 and `Ci`
    /// to 1.4e-12, measured 2026-09-19 across 12 abscissae that straddle the
    /// `x = 4` handover into the asymptotic form and the `sqrt(50)` split
    /// between the `f1`/`g1` and `f2`/`g2` fits.
    ///
    /// # Methodology
    ///
    /// `Si` is compared against `integral_0^x sin(t)/t dt` directly. `Ci`
    /// cannot be, because `integral_x^inf cos(t)/t dt` is improper and
    /// oscillatory; the equivalent convergent form
    /// `gamma + ln x + integral_0^x (cos t - 1)/t dt` is used instead, which
    /// shares no table with this module.
    ///
    /// `Ci`'s residual is two orders looser than `Si`'s and it is the
    /// **reference** that is looser: with 400 fixed panels over `[0, 100]`
    /// the oscillating integrand gets about four panels per period by the end.
    /// It degrades smoothly with `x` — 6.6e-15 at 0.5, 5.3e-14 at 10,
    /// 1.4e-12 at 100 — which is the signature of the quadrature, not of a
    /// branch.
    #[test]
    fn both_defining_integrals_are_reproduced() {
        let xs = [
            0.1_f64, 0.5, 1.0, 2.0, 3.999, 4.0, 5.0, 7.07, 7.08, 10.0, 30.0, 100.0,
        ];

        let (mut worst, mut at) = (0.0_f64, 0.0_f64);
        for x in xs {
            let q = quad(
                |t: f64| if t == 0.0 { 1.0 } else { t.sin() / t },
                0.0,
                x,
                400,
            );
            let e = ((si(x) - q) / q).abs();
            if e > worst {
                worst = e;
                at = x;
            }
        }
        assert!(
            worst < 1e-13,
            "Si against its defining integral: {worst:e} at x = {at}. \
             Documented at 1.047e-14"
        );

        let (mut worst, mut at) = (0.0_f64, 0.0_f64);
        for x in xs {
            let q = EULER
                + x.ln()
                + quad(
                    |t: f64| if t == 0.0 { 0.0 } else { (t.cos() - 1.0) / t },
                    0.0,
                    x,
                    400,
                );
            let e = ((ci(x) - q) / q).abs();
            if e > worst {
                worst = e;
                at = x;
            }
        }
        assert!(
            worst < 1e-11,
            "Ci against gamma + ln x + integral_0^x (cos t - 1)/t: {worst:e} \
             at x = {at}. Documented at 1.400e-12, which is the reference's \
             own limit at x = 100"
        );
    }

    /// The two derivative identities, `Si'(x) = sin(x)/x` and
    /// `Ci'(x) = cos(x)/x`, which hold across every branch at once and share
    /// no table with either function.
    #[test]
    fn the_derivative_identities_hold() {
        for (name, f, d) in [
            (
                "Si",
                si as fn(f64) -> f64,
                (|x: f64| x.sin() / x) as fn(f64) -> f64,
            ),
            ("Ci", ci, |x: f64| x.cos() / x),
        ] {
            let (mut worst, mut at) = (0.0_f64, 0.0_f64);
            for k in 1..=2000 {
                let x = 0.02 * k as f64;
                // Skip the two branch boundaries, where a stencil straddling
                // the cut measures the cut rather than the derivative.
                if (x - 4.0).abs() < 1e-6 || (x - XBND).abs() < 0.02 {
                    continue;
                }
                let h = 1e-4;
                let g = ((1.0 / 280.0) * (f(x - 4.0 * h) - f(x + 4.0 * h))
                    + (4.0 / 105.0) * (f(x + 3.0 * h) - f(x - 3.0 * h))
                    + 0.2 * (f(x - 2.0 * h) - f(x + 2.0 * h))
                    + 0.8 * (f(x + h) - f(x - h)))
                    / h;
                let e = (g - d(x)).abs();
                if e > worst {
                    worst = e;
                    at = x;
                }
            }
            assert!(
                worst < 1e-9,
                "{name}' residual: {worst:e} at x = {at}, which should be the \
                 difference stencil's floor rather than the function's"
            );
        }
    }

    /// The landmarks: `Si`'s Gibbs maximum, its `pi/2` limit, and `Ci`'s
    /// first zero — all against published values.
    ///
    /// Measured 2026-09-19:
    ///
    /// | | measured | published |
    /// |---|---|---|
    /// | `Si(pi)` | 1.851937051982 | 1.8519370519824661 |
    /// | first zero of `Ci` | 0.616505485621 | 0.6165054856 |
    ///
    /// `Si(pi)` is the Gibbs constant — the 17.9 % overshoot of a truncated
    /// Fourier series at a jump is `2 Si(pi)/pi - 1`, so this number is the
    /// one physical constant in the module.
    #[test]
    fn the_landmarks_match_the_literature() {
        let pi = core::f64::consts::PI;
        assert!(
            (si(pi) - 1.851_937_051_982_466_1).abs() < 1e-12,
            "Si(pi), the Gibbs constant, is documented as 1.8519370519824661; \
             it is {:.15}",
            si(pi)
        );
        // The Gibbs overshoot itself, 17.898 %.
        let overshoot = 2.0 * si(pi) / pi - 1.0;
        assert!(
            (overshoot - 0.178_979_744_472).abs() < 1e-10,
            "the Gibbs overshoot is documented as 17.898 %; it is {:.9}",
            overshoot
        );

        // Si -> pi/2, with the leading correction -cos(x)/x.
        for x in [1e3_f64, 1e5, 1e6] {
            let want = core::f64::consts::FRAC_PI_2 - x.cos() / x;
            assert!(
                (si(x) - want).abs() < 3.0 / (x * x),
                "Si({x:e}) = {:e} against pi/2 - cos(x)/x = {want:e}",
                si(x)
            );
        }

        // Ci's first zero, by bisection rather than a grid scan.
        let (mut lo, mut hi) = (0.1_f64, 1.5_f64);
        assert!(ci(lo) < 0.0 && ci(hi) > 0.0, "the bracket must straddle it");
        for _ in 0..200 {
            let m = 0.5 * (lo + hi);
            if ci(m) < 0.0 {
                lo = m;
            } else {
                hi = m;
            }
        }
        let z = 0.5 * (lo + hi);
        assert!(
            (z - 0.616_505_485_6).abs() < 1e-9,
            "Ci's first zero is documented at 0.6165054856; it is {z:.12}"
        );
    }

    /// Shape, symmetry and the two documented refusals.
    #[test]
    fn the_shape_and_the_domain() {
        assert_eq!(si(0.0), 0.0);
        assert!(si(f64::NAN).is_nan() && ci(f64::NAN).is_nan());

        // Si is odd, exactly: the sign is applied to the value rather than
        // to the argument in every branch.
        for k in 1..=4000 {
            let x = 0.01 * k as f64;
            assert_eq!(si(-x), -si(x), "Si not odd at {x}");
        }
        // And bounded by its maximum at pi.
        let cap = si(core::f64::consts::PI);
        for k in 0..=200_000 {
            let x = 0.001 * k as f64;
            assert!(si(x) <= cap, "Si({x}) exceeds Si(pi)");
        }

        // Ci's domain: upstream's DOMAIN_ERROR at and below zero, where the
        // function has a logarithmic singularity and a branch cut.
        for x in [0.0_f64, -1e-300, -1.0, -1e300, f64::NEG_INFINITY] {
            assert!(ci(x).is_nan(), "Ci({x:e}) should be NaN");
        }
        // Just above zero it is large and negative, not infinite.
        assert!(ci(1e-300) < -600.0 && ci(1e-300).is_finite());

        // Ci decays like 1/x and never overflows.
        for x in [1e3_f64, 1e6, 1e12, 1e100, 1e300] {
            assert!(ci(x).abs() < 2.0 / x, "Ci({x:e}) = {:e}", ci(x));
        }
    }

    /// **The far-field guards discard representable denormals, and that is
    /// upstream's choice, carried.**
    ///
    /// `fg_asymp` zeroes `f` past `1/GSL_DBL_MIN = 4.494e+307` and `g` past
    /// `1/GSL_SQRT_DBL_MIN = 6.704e+153`, but `1/x` and `1/x^2` are still
    /// representable there as denormals — `1/1e308 = 1e-308` is a perfectly
    /// good `f64`. So `Ci(1e308)` returns exactly `0` where the arithmetic
    /// would have given about `-1e-308` (2026-09-19).
    ///
    /// This is the same category as `synchrotron`'s underflow guard, seen
    /// from the other side: there the bound is so far out that it never
    /// fires, here it fires early and costs answers. It is kept because the
    /// bar for this module is agreement with GSL, and the measurement is
    /// asserted so the cost is on the record rather than assumed to be zero.
    #[test]
    fn the_far_field_guards_fire_early_and_that_is_upstreams() {
        assert_eq!(XMAXF, 1.0 / f64::MIN_POSITIVE);
        assert_eq!(XMAXG, 1.0 / SQRT_DBL_MIN);
        assert!(
            XMAXG < XMAXF,
            "g underflows first, which is why there are two"
        );

        // Both guards fire before the arithmetic would.
        let (f, g) = fg_asymp(1e308);
        assert_eq!((f, g), (0.0, 0.0), "both guards should have fired at 1e308");
        assert!(
            1.0 / 1e308_f64 > 0.0,
            "and yet 1/x is representable there -- which is what the guard \
             is discarding"
        );
        assert_eq!(ci(1e308), 0.0);
        assert_eq!(si(1e308), core::f64::consts::FRAC_PI_2);

        // Just inside, the answers are real and small.
        let (f, g) = fg_asymp(XMAXG * 0.999);
        assert!(f > 0.0 && g > 0.0, "inside XMAXG both should be non-zero");
        assert!(ci(1e300) != 0.0, "Ci(1e300) is still a real number");
    }

    /// The four machine constants are the expressions upstream declares, and
    /// `XBND` is carried as upstream's rounded decimal rather than as
    /// `sqrt(50)`.
    #[test]
    fn the_bounds_are_what_upstream_declares() {
        assert_eq!(XBIG, 1.0 / SQRT_DBL_EPSILON);
        assert_eq!(XMAXF, 1.0 / f64::MIN_POSITIVE);
        assert_eq!(XMAXG, 1.0 / SQRT_DBL_MIN);
        // XBND is a BRANCH BOUNDARY, so upstream's rounded 7.07106781187 is
        // carried verbatim; sqrt(50) differs in the twelfth digit and would
        // move which fit runs on a thin sliver of the domain.
        assert_ne!(XBND, 50.0_f64.sqrt());
        assert!((XBND - 50.0_f64.sqrt()).abs() < 1e-11);
        assert!(4.0 < XBND && XBND < XBIG && XBIG < XMAXG && XMAXG < XMAXF);
    }
}
