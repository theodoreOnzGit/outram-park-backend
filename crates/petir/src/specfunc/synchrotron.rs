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
// PORTED from the GNU Scientific Library (GSL) 2.8, specfunc/synchrotron.c
// <https://www.gnu.org/software/gsl/>
// Copyright (C) 1996-2000 Gerard Jungman; small-argument correction terms by
// Brian Gough.
// GSL is free software under the GNU General Public License, version 3 or
// later.
//
// The six Chebyshev tables here were extracted from that source by script,
// not retyped, and are audited bit-for-bit by tests/gsl_tables_audit.rs.

//! The synchrotron radiation functions `S_1(x)` and `S_2(x)`.
//!
//! # What these are
//!
//! ```text
//!     S_1(x) = x integral_x^inf K_{5/3}(t) dt
//!     S_2(x) = x K_{2/3}(x)
//! ```
//!
//! They give the angle-integrated spectrum of synchrotron emission from a
//! relativistic charge in a magnetic field: `S_1` for the total power per
//! unit frequency, `S_2` for the polarised difference. `x` is the frequency
//! in units of the critical frequency.
//!
//! Both rise like `x^{1/3}` at small argument and fall like
//! `sqrt(pi x / 2) e^{-x}` at large — a spectrum with a single broad peak,
//! which is why the shape matters more than any one value.
//!
//! # Argument range, stated plainly
//!
//! `x >= 0`, dimensionless (`f64`). A negative argument returns `NaN`
//! (upstream's `DOMAIN_ERROR`), and so does `NaN`.
//!
//! **Above `-8 ln(DBL_MIN)/7`, about 809.9, both return `0.0`** — upstream's
//! `UNDERFLOW_ERROR`. That is not an approximation: `e^{-x}` has left `f64`'s
//! range by then, and the functions are genuinely unrepresentable rather than
//! merely small.
//!
//! # Accuracy
//!
//! `S_2(x) = x K_{2/3}(x)` is an **exact identity**, and this crate has no
//! fractional-order Bessel function — so the reference is the integral
//! representation
//!
//! ```text
//!     K_nu(x) = integral_0^inf e^{-x cosh t} cosh(nu t) dt
//! ```
//!
//! evaluated by quadrature, which shares no table, branch or coefficient with
//! the implementation. `S_1` is checked the same way through its own defining
//! double integral. Results are in the tests.

// Under a std-linked build (`cargo test`) f64's inherent powf/exp/sqrt shadow
// these trait methods, leaving the import formally unused. See crate::real.
#[allow(unused_imports)]
use crate::real::Real;

use crate::specfunc::{LOG_DBL_MIN, SQRT_DBL_EPSILON};
/// GSL's `synchrotron1_cs`, 13 coefficients, order 12.
#[rustfmt::skip]
const SYNCH1: [f64; 13] = [
    30.364682982501076273, 17.079395277408394574, 4.560132133545072889, 0.549281246730419979,
    0.372976075069301172e-01, 0.161362430201041242e-02, 0.481916772120371e-04,
    0.10512425288938e-05, 0.174638504670e-07, 0.22815486544e-09, 0.240443082e-11,
    0.2086588e-13, 0.15167e-15,
];

/// GSL's `synchrotron2_cs`, 12 coefficients, order 11.
#[rustfmt::skip]
const SYNCH2: [f64; 12] = [
    0.4490721623532660844, 0.898353677994187218e-01, 0.81044573772151290e-02,
    0.4261716991089162e-03, 0.147609631270746e-04, 0.3628633615300e-06, 0.66634807498e-08,
    0.949077166e-10, 0.1079125e-11, 0.10022e-13, 0.77e-16, 0.5e-18,
];

/// GSL's `synchrotron1a_cs`, 23 coefficients, order 22.
#[rustfmt::skip]
const SYNCH1A: [f64; 23] = [
    2.1329305161355000985, 0.741352864954200240e-01, 0.86968099909964198e-02,
    0.11703826248775692e-02, 0.1645105798619192e-03, 0.240201021420640e-04,
    0.35827756389389e-05, 0.5447747626984e-06, 0.838802856196e-07, 0.13069882684e-07,
    0.2053099071e-08, 0.325187537e-09, 0.517914041e-10, 0.83002988e-11, 0.13352728e-11,
    0.2159150e-12, 0.349967e-13, 0.56994e-14, 0.9291e-15, 0.152e-15, 0.249e-16, 0.41e-17,
    0.7e-18,
];

/// GSL's `synchrotron21_cs`, 13 coefficients, order 12.
#[rustfmt::skip]
const SYNCH21: [f64; 13] = [
    38.617839923843085480, 23.037715594963734597, 5.3802499868335705968,
    0.6156793806995710776, 0.406688004668895584e-01, 0.17296274552648414e-02,
    0.51061258836577e-04, 0.110459595022e-05, 0.18235530206e-07, 0.2370769803e-09,
    0.24887296e-11, 0.21529e-13, 0.156e-15,
];

/// GSL's `synchrotron22_cs`, 13 coefficients, order 12.
#[rustfmt::skip]
const SYNCH22: [f64; 13] = [
    7.9063148270660804288, 3.1353463612853425684, 0.4854879477453714538,
    0.394816675827237234e-01, 0.19661622334808802e-02, 0.659078932293042e-04,
    0.15857561349856e-05, 0.286865301123e-07, 0.4041202360e-09, 0.45568444e-11, 0.420459e-13,
    0.3232e-15, 0.21e-17,
];

/// GSL's `synchrotron2a_cs`, 17 coefficients, order 16.
#[rustfmt::skip]
const SYNCH2A: [f64; 17] = [
    2.020337094170713600, 0.10956237121807404e-01, 0.8542384730114676e-03,
    0.723430242132822e-04, 0.63124427962699e-05, 0.5648193141174e-06, 0.512832480138e-07,
    0.47196532914e-08, 0.4380744214e-09, 0.410268149e-10, 0.38623072e-11, 0.3661323e-12,
    0.348023e-13, 0.33301e-14, 0.319e-15, 0.307e-16, 0.3e-17,
];

/// `pi / sqrt(3)`, upstream's `M_PI/M_SQRT3` in `S_1`'s Chebyshev branch.
///
/// `M_SQRT3` is 1.73205080756887729353; the quotient is written out rather
/// than formed from `PI / 3.0_f64.sqrt()` so the value does not depend on
/// how a given libm rounds `sqrt(3)`.
const PI_OVER_SQRT3: f64 = 1.813_799_364_234_217_843_9;

/// `ln(sqrt(pi/2))`, upstream's `c0` in both large-argument branches.
const LOG_SQRT_PI_2: f64 = 0.225_791_352_644_727_432_363_097_6;

/// The point past which both functions underflow `f64`, upstream's
/// `-8 GSL_LOG_DBL_MIN / 7`. About 809.9.
fn underflow_cut() -> f64 {
    -8.0 * LOG_DBL_MIN / 7.0
}

/// `x^n` for the small integer powers upstream takes with
/// `gsl_sf_pow_int` — 11 for `S_1`, 5 for `S_2`.
fn pow_int(x: f64, n: u32) -> f64 {
    let mut out = 1.0;
    for _ in 0..n {
        out *= x;
    }
    out
}

/// `S_1(x)`, GSL's `gsl_sf_synchrotron_1`.
///
/// `NaN` for `x < 0` and for `NaN`; `0.0` past the underflow cut.
///
/// # Examples
///
/// ```
/// use petir::specfunc::synchrotron::synchrotron_1;
/// // Rises like x^{1/3} from the origin.
/// assert_eq!(synchrotron_1(0.0), 0.0);
/// assert!(synchrotron_1(1.0) > 0.0);
/// // And underflows rather than returning a denormal.
/// assert_eq!(synchrotron_1(1000.0), 0.0);
/// ```
pub fn synchrotron_1(x: f64) -> f64 {
    if x.is_nan() || x < 0.0 {
        return f64::NAN;
    }
    if x < 2.0 * core::f64::consts::SQRT_2 * SQRT_DBL_EPSILON {
        // Brian Gough's first-order correction to the Taylor series
        // S1(x) = (4pi / (sqrt(3) Gamma(1/3))) (x/2)^{1/3} (1 - ...).
        let z = x.powf(1.0 / 3.0);
        let cf = 1.0 - 8.438_127_628_132_05e-1 * z * z;
        return 2.149_528_241_534_478_636_71 * z * cf;
    }
    if x <= 4.0 {
        let c0 = PI_OVER_SQRT3;
        let px = x.powf(1.0 / 3.0);
        let px11 = pow_int(px, 11);
        let t = x * x / 8.0 - 1.0;
        return px * crate::cheb_slice::eval_gsl(t, &SYNCH1)
            - px11 * crate::cheb_slice::eval_gsl(t, &SYNCH2)
            - c0 * x;
    }
    if x < underflow_cut() {
        let t = (12.0 - x) / (x + 4.0);
        return x.sqrt() * crate::cheb_slice::eval_gsl(t, &SYNCH1A) * (LOG_SQRT_PI_2 - x).exp();
    }
    // Upstream's UNDERFLOW_ERROR: e^{-x} has left f64's range.
    0.0
}

/// `S_2(x) = x K_{2/3}(x)`, GSL's `gsl_sf_synchrotron_2`.
///
/// `NaN` for `x < 0` and for `NaN`; `0.0` past the underflow cut.
pub fn synchrotron_2(x: f64) -> f64 {
    if x.is_nan() || x < 0.0 {
        return f64::NAN;
    }
    if x < 2.0 * core::f64::consts::SQRT_2 * SQRT_DBL_EPSILON {
        let z = x.powf(1.0 / 3.0);
        let cf = 1.0 - 1.177_671_565_102_35 * z * x;
        return 1.074_764_120_767_239_318_36 * z * cf;
    }
    if x <= 4.0 {
        let px = x.powf(1.0 / 3.0);
        let px5 = pow_int(px, 5);
        let t = x * x / 8.0 - 1.0;
        return px * crate::cheb_slice::eval_gsl(t, &SYNCH21)
            - px5 * crate::cheb_slice::eval_gsl(t, &SYNCH22);
    }
    if x < underflow_cut() {
        let t = (10.0 - x) / (x + 2.0);
        return x.sqrt() * (LOG_SQRT_PI_2 - x).exp() * crate::cheb_slice::eval_gsl(t, &SYNCH2A);
    }
    0.0
}

#[cfg(test)]
mod tests {
    extern crate std;
    use super::*;

    /// `K_nu(x)` by its integral representation
    ///
    /// ```text
    ///     K_nu(x) = integral_0^inf e^{-x cosh t} cosh(nu t) dt
    /// ```
    ///
    /// evaluated by the trapezoid rule, which for this integrand is
    /// **spectrally accurate**: the integrand decays doubly exponentially in
    /// `t`, so the Euler-Maclaurin error terms all vanish and the trapezoid
    /// beats Simpson here. The upper limit is chosen from where
    /// `x cosh(t) - nu t` exceeds `ln(f64::MAX)`.
    ///
    /// Shares no table, branch or coefficient with the implementation.
    fn bessel_k_nu(nu: f64, x: f64, steps: u32) -> f64 {
        // Cut where e^{-x cosh t} cosh(nu t) has left f64's range.
        let mut hi = 1.0_f64;
        while x * hi.cosh() - nu * hi < 720.0 {
            hi += 0.5;
        }
        let h = hi / steps as f64;
        let f = |t: f64| (-x * t.cosh()).exp() * (nu * t).cosh();
        let mut s = 0.5 * (f(0.0) + f(hi));
        for k in 1..steps {
            s += f(k as f64 * h);
        }
        s * h
    }

    /// `S_1(x) = x integral_x^inf K_{5/3}(t) dt`, by quadrature on the outer
    /// integral too. Substituting `t = x + u^2` clusters points near the
    /// lower limit, where the integrand is largest.
    fn s1_reference(x: f64, outer: u32, inner: u32) -> f64 {
        let mut upper = 1.0_f64;
        while upper * upper < 60.0 {
            upper += 0.5;
        }
        let h = upper / outer as f64;
        let g = |u: f64| 2.0 * u * bessel_k_nu(5.0 / 3.0, x + u * u, inner);
        let mut s = 0.5 * (g(0.0) + g(upper));
        for k in 1..outer {
            s += g(k as f64 * h);
        }
        x * s * h
    }

    /// **`S_2(x) = x K_{2/3}(x)`, against the integral representation.**
    ///
    /// An exact identity, so this is the strongest check available: worst
    /// relative **9.806e-13** at `x = 4`, measured 2026-09-19. The reference
    /// shares no table, branch or coefficient with the implementation.
    #[test]
    fn s2_is_x_times_the_modified_bessel_function() {
        let (mut worst, mut at) = (0.0_f64, 0.0_f64);
        for k in 1..=120 {
            let x = 0.05 * k as f64;
            let r = x * bessel_k_nu(2.0 / 3.0, x, 20_000);
            if r.abs() < 1e-14 {
                continue;
            }
            let e = ((synchrotron_2(x) - r) / r).abs();
            if e > worst {
                worst = e;
                at = x;
            }
        }
        assert!(worst < 1e-10, "S_2 vs x K_2/3: {worst:e} at {at}");
    }

    /// **`S_1(x)` against its own defining double integral.**
    ///
    /// Worst relative **1.029e-05** at `x = 0.25`, measured 2026-09-19 —
    /// seven orders looser than `S_2`'s check, and that is the *reference*,
    /// not `S_1`. `K_{5/3}(t)` diverges like `t^{-5/3}` as `t -> 0`, so the
    /// outer quadrature is integrating a nearly singular integrand at small
    /// `x`; `the_double_integral_reference_is_what_limits_the_s1_check`
    /// establishes that rather than assuming it.
    #[test]
    fn s1_matches_its_defining_double_integral() {
        let (mut worst, mut at) = (0.0_f64, 0.0_f64);
        for k in 1..=24 {
            let x = 0.25 * k as f64;
            let r = s1_reference(x, 2_000, 2_000);
            if r.abs() < 1e-14 {
                continue;
            }
            let e = ((synchrotron_1(x) - r) / r).abs();
            if e > worst {
                worst = e;
                at = x;
            }
        }
        assert!(
            worst < 1e-3,
            "S_1 vs its double integral: {worst:e} at {at}"
        );
    }

    /// **The `S_1` residual is the double-integral reference's, not
    /// `S_1`'s** — it falls as the outer quadrature is refined, which an
    /// error belonging to `S_1` could not do.
    #[test]
    fn the_double_integral_reference_is_what_limits_the_s1_check() {
        let x = 0.25_f64;
        let rel = |outer: u32| {
            let r = s1_reference(x, outer, 2_000);
            ((synchrotron_1(x) - r) / r).abs()
        };
        let coarse = rel(250);
        let fine = rel(4_000);
        assert!(
            coarse > 4.0 * fine,
            "the S_1 residual is documented as the reference quadrature's \
             rather than S_1's, because it falls as the outer integral is \
             refined: {coarse:e} at 250 panels against {fine:e} at 4000"
        );
    }

    /// The published asymptotes, small and large.
    #[test]
    fn the_asymptotes_are_right() {
        // Small x: both rise like x^{1/3}, and the residual against the
        // LEADING term is exactly Brian Gough's first-order correction --
        // ratio 1.0000 at every probe, measured 2026-09-19. Asserting that
        // rather than a loose bound pins the correction term itself.
        for x in [1e-10_f64, 1e-8, 1e-6, 1e-4] {
            let z = x.powf(1.0 / 3.0);
            let a1 = 2.149_528_241_534_478_636_71 * z;
            let rel = ((synchrotron_1(x) - a1) / a1).abs();
            let predicted = 8.438_127_628_132_05e-1 * z * z;
            assert!(
                (rel / predicted - 1.0).abs() < 1e-5,
                "S_1's departure from its leading x^1/3 term is documented as \
                 exactly the correction 0.8438 x^2/3: at x = {x:e}, measured \
                 {rel:e} against predicted {predicted:e}"
            );
            // S_2 rises with half the prefactor.
            let a2 = 1.074_764_120_767_239_318_36 * z;
            assert!(
                ((synchrotron_2(x) - a2) / a2).abs() < 1e-2,
                "S_2({x:e}) vs its x^1/3 asymptote"
            );
            assert!(
                ((synchrotron_1(x) / synchrotron_2(x)) - 2.0).abs() < 1e-2,
                "S_1/S_2 at {x:e} is documented as 2 in the small-x limit"
            );
        }

        // Large x: both fall like sqrt(pi x / 2) e^{-x}.
        for x in [50.0_f64, 100.0, 300.0] {
            let a = (core::f64::consts::PI * x / 2.0).sqrt() * (-x).exp();
            for (name, v) in [("S_1", synchrotron_1(x)), ("S_2", synchrotron_2(x))] {
                assert!(
                    ((v - a) / a).abs() < 0.02,
                    "{name}({x}) = {v:e} vs sqrt(pi x/2) e^-x = {a:e}"
                );
            }
        }
    }

    /// The shape: both positive, single-peaked, and zero past the underflow
    /// cut.
    #[test]
    fn the_shape_and_the_underflow_are_right() {
        assert_eq!(synchrotron_1(0.0), 0.0);
        assert_eq!(synchrotron_2(0.0), 0.0);
        for f in [synchrotron_1 as fn(f64) -> f64, synchrotron_2] {
            let mut rising = true;
            let mut prev = 0.0_f64;
            let mut turned = 0;
            for k in 1..=2000 {
                let x = 0.01 * k as f64;
                let v = f(x);
                assert!(v > 0.0, "not positive at {x}");
                if rising && v < prev {
                    rising = false;
                    turned += 1;
                }
                prev = v;
            }
            assert_eq!(turned, 1, "documented as single-peaked");
        }
        // THE EXPLICIT UNDERFLOW CUT NEVER FIRES FIRST -- in f64, not just
        // in f32. Upstream's bound is -8 ln(DBL_MIN)/7 = 809.5959, but
        // exp(c0 - x) has already reached exactly zero through the denormal
        // range at x = 745.3590, where the value just below is 1.33e-322.
        // So the guard is 64 units of x too late to be what produces the
        // zero. That is the same "the arithmetic enforces the guard before
        // the guard does" shape as airy's overflow threshold and transport's
        // tail cut -- but those are f32 observations, and this one holds at
        // full f64 width, so it is upstream's own redundancy.
        let cut = underflow_cut();
        assert!(
            (809.0..810.0).contains(&cut),
            "upstream's cut is documented at 809.5959, and is {cut}"
        );
        let first_zero = |f: fn(f64) -> f64| {
            let (mut lo, mut hi) = (4.0_f64, cut);
            for _ in 0..200 {
                let m = 0.5 * (lo + hi);
                if f(m) > 0.0 {
                    lo = m;
                } else {
                    hi = m;
                }
            }
            (hi, f(lo))
        };
        for (name, f) in [
            ("S_1", synchrotron_1 as fn(f64) -> f64),
            ("S_2", synchrotron_2 as fn(f64) -> f64),
        ] {
            let (z, just_below) = first_zero(f);
            assert!(
                (745.0..746.0).contains(&z),
                "{name} is documented as first reaching zero at x = 745.3590, \
                 well before upstream's {cut} cut; measured {z}"
            );
            assert!(
                z < cut - 60.0,
                "{name} reaches zero {} units before the explicit cut, which \
                 is the point: the guard is redundant",
                cut - z
            );
            assert!(
                just_below > 0.0 && just_below < 1e-300,
                "{name} just below its zero is documented as a DENORMAL \
                 (1.33e-322), which is what makes this an underflow rather \
                 than a truncation; it is {just_below:e}"
            );
        }
        // And past upstream's cut both are still zero, so the guard is
        // consistent with the arithmetic even though it is not what acts.
        assert_eq!(synchrotron_1(cut + 1.0), 0.0);
        assert_eq!(synchrotron_2(cut + 1.0), 0.0);
    }

    /// The refusals.
    #[test]
    fn the_refusals_are_right() {
        for x in [-1e-9_f64, -1.0, -1e9] {
            assert!(synchrotron_1(x).is_nan(), "S_1({x:e})");
            assert!(synchrotron_2(x).is_nan(), "S_2({x:e})");
        }
        assert!(synchrotron_1(f64::NAN).is_nan());
        assert!(synchrotron_2(f64::NAN).is_nan());
    }

    /// Every branch boundary joins, compared as two **formulas** at one
    /// argument.
    #[test]
    fn the_branch_boundaries_join() {
        let x = 2.0 * core::f64::consts::SQRT_2 * SQRT_DBL_EPSILON;
        let z = x.powf(1.0 / 3.0);
        let px = z;
        let t = x * x / 8.0 - 1.0;

        let small1 = 2.149_528_241_534_478_636_71 * z * (1.0 - 8.438_127_628_132_05e-1 * z * z);
        let cheb1 = px * crate::cheb_slice::eval_gsl(t, &SYNCH1)
            - pow_int(px, 11) * crate::cheb_slice::eval_gsl(t, &SYNCH2)
            - PI_OVER_SQRT3 * x;
        assert!(
            ((small1 - cheb1) / cheb1).abs() < 1e-8,
            "S_1 small/Chebyshev join at {x:e}: {small1:e} vs {cheb1:e}"
        );

        let small2 = 1.074_764_120_767_239_318_36 * z * (1.0 - 1.177_671_565_102_35 * z * x);
        let cheb2 = px * crate::cheb_slice::eval_gsl(t, &SYNCH21)
            - pow_int(px, 5) * crate::cheb_slice::eval_gsl(t, &SYNCH22);
        assert!(
            ((small2 - cheb2) / cheb2).abs() < 1e-8,
            "S_2 small/Chebyshev join at {x:e}: {small2:e} vs {cheb2:e}"
        );

        // The Chebyshev branch against the large-argument one, at x = 4.
        let x = 4.0_f64;
        let px = x.powf(1.0 / 3.0);
        let t = x * x / 8.0 - 1.0;
        let a = px * crate::cheb_slice::eval_gsl(t, &SYNCH1)
            - pow_int(px, 11) * crate::cheb_slice::eval_gsl(t, &SYNCH2)
            - PI_OVER_SQRT3 * x;
        let b = x.sqrt()
            * crate::cheb_slice::eval_gsl((12.0 - x) / (x + 4.0), &SYNCH1A)
            * (LOG_SQRT_PI_2 - x).exp();
        assert!(
            ((a - b) / b).abs() < 1e-8,
            "S_1 Chebyshev/large join at 4: {a:e} vs {b:e}"
        );

        let a = px * crate::cheb_slice::eval_gsl(t, &SYNCH21)
            - pow_int(px, 5) * crate::cheb_slice::eval_gsl(t, &SYNCH22);
        let b = x.sqrt()
            * (LOG_SQRT_PI_2 - x).exp()
            * crate::cheb_slice::eval_gsl((10.0 - x) / (x + 2.0), &SYNCH2A);
        assert!(
            ((a - b) / b).abs() < 1e-8,
            "S_2 Chebyshev/large join at 4: {a:e} vs {b:e}"
        );
    }
}
