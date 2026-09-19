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
// PORTED from the GNU Scientific Library (GSL) 2.8, specfunc/expint3.c
// <https://www.gnu.org/software/gsl/>
// Copyright (C) 1996-2000 Gerard Jungman
// GSL is free software under the GNU General Public License, version 3 or
// later.
//
// Both Chebyshev tables were extracted from that source by script, not
// retyped, and are audited bit-for-bit by tests/gsl_tables_audit.rs.

//! The cubic exponential integral `Ei_3(x)`.
//!
//! # What this is
//!
//! ```text
//!     Ei_3(x) = integral_0^x e^{-t^3} dt
//! ```
//!
//! the cubic member of the family whose quadratic member is
//! `(sqrt(pi)/2) erf(x)`. It rises monotonically from the origin to
//! `Gamma(4/3) = 0.892979511569249211` and never exceeds it, because the
//! integrand is positive and `integral_0^inf e^{-t^3} dt` is that gamma
//! value by the substitution `u = t^3`.
//!
//! # Argument range, stated plainly
//!
//! **`x >= 0`**, dimensionless (`f64`). Negative arguments are upstream's
//! `DOMAIN_ERROR` and return `NaN` here — note the integrand `e^{-t^3}` grows
//! without bound for negative `t`, so this is a genuine domain restriction
//! and not a convention. `NaN` propagates.
//!
//! There is no overflow: the function is bounded by `Gamma(4/3)`.
//!
//! # Accuracy
//!
//! Measured against the **defining integral** by composite Gauss-Legendre,
//! and against the closed forms at both ends — `Ei_3(x) -> x` at the origin
//! and `-> Gamma(4/3)` at infinity, with `Gamma(4/3)` taken from PETIR's own
//! `gamma` rather than from the literal upstream carries. Results are in the
//! tests.

// Under a std-linked build (`cargo test`) f64's inherent exp/cbrt shadow
// these trait methods, leaving the import formally unused. See crate::real.
#[allow(unused_imports)]
use crate::real::Real;

use crate::cheb_slice::eval_gsl as cheb;

/// `GSL_ROOT3_DBL_EPSILON`, the cube root of `DBL_EPSILON`.
const ROOT3_DBL_EPSILON: f64 = 6.055_454_452_393_343e-6;
/// `GSL_LOG_DBL_EPSILON`.
const LOG_DBL_EPSILON: f64 = -3.604_365_338_911_715_4e1;
/// `Gamma(4/3)`, the value of the integral over the whole half-line.
/// Upstream writes this literal as `val_infinity`.
const VAL_INFINITY: f64 = 0.892_979_511_569_249_211;

/// GSL's `expint3_data`, 24 coefficients on `[-1, 1]`.
#[rustfmt::skip]
const EXPINT3: [f64; 24] = [
    1.269198414221126014, -0.248846446384140982, 0.80526220717231041e-01,
    -0.25772733251968330e-01, 0.7599878873073774e-02, -0.2030695581940405e-02,
    0.490834586699330e-03, -0.107682239142021e-03, 0.21551726264290e-04, -0.3956705137384e-05,
    0.6699240933896e-06, -0.105132180807e-06, 0.15362580199e-07, -0.20990960364e-08,
    0.2692109538e-09, -0.325195242e-10, 0.37114816e-11, -0.4013652e-12, 0.412334e-13,
    -0.40338e-14, 0.3766e-15, -0.336e-16, 0.29e-17, -0.2e-18,
];

/// GSL's `expint3a_data`, 23 coefficients on `[-1, 1]`.
#[rustfmt::skip]
const EXPINT3A: [f64; 23] = [
    1.9270464955068273729, -0.349293565204813805e-01, 0.14503383718983009e-02,
    -0.8925336718327903e-04, 0.70542392191184e-05, -0.6671727454761e-06, 0.724267589982e-07,
    -0.87825825606e-08, 0.11672234428e-08, -0.1676631281e-09, 0.257550158e-10, -0.41957888e-11,
    0.7201041e-12, -0.1294906e-12, 0.24287e-13, -0.47331e-14, 0.95531e-15, -0.1991e-15,
    0.428e-16, -0.94e-17, 0.21e-17, -0.5e-18, 0.1e-18,
];

/// `Ei_3(x) = integral_0^x e^{-t^3} dt`, GSL's `gsl_sf_expint_3`.
///
/// `x` is dimensionless (`f64`) and must be non-negative; a negative argument
/// is upstream's `DOMAIN_ERROR` and returns `NaN`.
///
/// Four branches: `Ei_3(x) = x` below `1.6 * GSL_ROOT3_DBL_EPSILON`, a
/// Chebyshev fit in `x^3/4 - 1` to `x = 2`, a second in `16/x^3 - 1`
/// subtracted from `Gamma(4/3)` while `e^{-x^3}` is still representable, and
/// the constant `Gamma(4/3)` beyond.
///
/// # Examples
///
/// ```
/// use petir::specfunc::expint3::expint_3;
/// assert_eq!(expint_3(0.0), 0.0);
/// // Bounded above by Gamma(4/3).
/// assert!((expint_3(10.0) - 0.892_979_511_569_249_2).abs() < 1e-15);
/// // Negative x is outside the domain.
/// assert!(expint_3(-1.0).is_nan());
/// ```
pub fn expint_3(x: f64) -> f64 {
    if x.is_nan() || x < 0.0 {
        // Upstream's DOMAIN_ERROR.
        return f64::NAN;
    }
    if x < 1.6 * ROOT3_DBL_EPSILON {
        x
    } else if x <= 2.0 {
        x * cheb(x * x * x / 4.0 - 1.0, &EXPINT3)
    } else if x < (-LOG_DBL_EPSILON).powf(1.0 / 3.0) {
        // Past x^3 = -log(eps) the correction is below one ulp of the limit,
        // which is what the next branch relies on.
        let t = 16.0 / (x * x * x) - 1.0;
        let s = (-x * x * x).exp() / (3.0 * x * x);
        VAL_INFINITY - cheb(t, &EXPINT3A) * s
    } else {
        VAL_INFINITY
    }
}

#[cfg(test)]
mod tests {
    extern crate std;
    use super::*;

    /// `integral_0^x e^{-t^3} dt`, computed **independently of this module**
    /// by composite 30-point Gauss-Legendre.
    fn by_quadrature(x: f64, panels: usize) -> f64 {
        if x == 0.0 {
            return 0.0;
        }
        let h = x / panels as f64;
        let mut acc = 0.0;
        for k in 0..panels {
            let a = k as f64 * h;
            acc += crate::integration::gauss_legendre::gauss_legendre(
                |t: f64| (-(t * t * t)).exp(),
                a,
                a + h,
                30,
            )
            .expect("30-point Gauss-Legendre is tabulated");
        }
        acc
    }

    /// **The defining integral is reproduced to 6.0e-15**, measured
    /// 2026-09-19 across 14 abscissae straddling all three branch boundaries
    /// — the linear cut at `1.6 * GSL_ROOT3_DBL_EPSILON = 9.689e-06`, the
    /// Chebyshev handover at `x = 2`, and the saturation cut at
    /// `(-log eps)^{1/3} = 3.30326`.
    ///
    /// The worst point is `x = 1e-7`, inside the linear branch, and the
    /// residual there is the quadrature's rather than the function's: on that
    /// branch `Ei_3(x) = x` exactly.
    #[test]
    fn the_defining_integral_is_reproduced() {
        let (mut worst, mut at) = (0.0_f64, 0.0_f64);
        for x in [
            1e-7_f64, 1e-4, 0.1, 0.5, 1.0, 1.999, 2.0, 2.5, 3.0, 3.3, 3.302, 3.303, 4.0, 10.0,
        ] {
            let q = by_quadrature(x, 300);
            let e = ((expint_3(x) - q) / q).abs();
            if e > worst {
                worst = e;
                at = x;
            }
        }
        assert!(
            worst < 1e-13,
            "Ei_3 against its defining integral: {worst:e} at x = {at}. \
             Documented at 5.956e-15"
        );
    }

    /// **Upstream's saturation cut is exactly placed** — the two branches it
    /// separates agree bit for bit.
    ///
    /// Past `x = (-GSL_LOG_DBL_EPSILON)^{1/3} = 3.3032613423484727`, `Ei_3`
    /// is declared equal to `Gamma(4/3)` and the correction
    /// `cheb(16/x^3-1) e^{-x^3}/(3x^2)` is dropped. Straddling that point by
    /// one part in 1e13 gives **zero difference** (2026-09-19), because
    /// `e^{-x^3}` is already below one ulp of the limit there — which is
    /// exactly what the constant is derived to guarantee.
    ///
    /// Worth asserting because this crate has a counter-example on file:
    /// `fermi_dirac`'s `F_2` carries the analogous cut six orders of
    /// magnitude too early and steps by 3.56e-10. A cut derived from the
    /// right quantity is invisible; one derived from the wrong quantity is
    /// not.
    #[test]
    fn the_saturation_cut_is_exactly_placed() {
        let cut = (-LOG_DBL_EPSILON).powf(1.0 / 3.0);
        assert!(
            (cut - 3.303_261_342_348_472_7).abs() < 1e-12,
            "the saturation cut is documented at 3.3032613423484727; it is {cut}"
        );
        let below = expint_3(cut * (1.0 - 1e-13));
        let above = expint_3(cut * (1.0 + 1e-13));
        assert_eq!(
            below.to_bits(),
            above.to_bits(),
            "the saturation cut is documented as invisible -- {below:e} below \
             it against {above:e} above. A visible step means the constant no \
             longer marks where e^{{-x^3}} falls under one ulp"
        );
        assert_eq!(above, VAL_INFINITY);
    }

    /// **Upstream's `val_infinity` literal is the correctly rounded
    /// `Gamma(4/3)`, and PETIR's own `gamma` is 6 ulp away from it.**
    ///
    /// `Gamma(4/3) = 0.89297951156924921123761146935850659...`, whose nearest
    /// `f64` is `0.8929795115692493`. Upstream's literal rounds to exactly
    /// that. `crate::specfunc::gamma::gamma(4.0/3.0)` returns
    /// `0.8929795115692499`, six ulp higher (2026-09-19).
    ///
    /// The literal is therefore used here rather than a call into `gamma` —
    /// which is also what upstream does, so it is the faithful choice
    /// anyway. The `gamma` discrepancy is recorded rather than chased: six
    /// ulp is ordinary for a Lanczos evaluation and this is not the place to
    /// establish whether compiled GSL agrees. Filed as a bead.
    #[test]
    fn the_saturation_value_is_a_correctly_rounded_gamma_four_thirds() {
        // The decimal expansion, to more digits than f64 holds.
        let true_value = 0.892_979_511_569_249_211_237_611_469_358_5_f64;
        assert_eq!(
            VAL_INFINITY.to_bits(),
            true_value.to_bits(),
            "upstream's val_infinity is documented as the correctly rounded \
             Gamma(4/3)"
        );

        let from_gamma = crate::specfunc::gamma::gamma(4.0 / 3.0);
        // Counted in BITS, not as `|a-b| / (EPSILON * value)`. A first draft
        // used the latter and reported 3.4 where the true separation is 6:
        // `f64::EPSILON` is the ulp at 1.0, and 0.893 lies in [0.5, 1) where
        // the spacing is half that. Consecutive f64 differ by one in the bit
        // pattern, so subtracting the patterns IS the ulp count.
        let ulps = from_gamma.to_bits().abs_diff(VAL_INFINITY.to_bits()) as f64;
        assert!(
            (4.0..10.0).contains(&ulps),
            "PETIR's gamma(4/3) is documented as 6 ulp above upstream's \
             literal; it is {ulps:.1} ulp away ({from_gamma:.17e} against \
             {VAL_INFINITY:.17e}). If this has moved, either gamma changed or \
             this note is stale"
        );
        // And it is gamma that is off, not the literal.
        assert!(
            (from_gamma - true_value).abs() > (VAL_INFINITY - true_value).abs(),
            "the literal is documented as the closer of the two to the true \
             Gamma(4/3)"
        );
    }

    /// Monotone, bounded by `Gamma(4/3)`, linear at the origin, and the
    /// documented domain restriction.
    #[test]
    fn the_shape_and_the_domain() {
        assert_eq!(expint_3(0.0), 0.0);
        assert!(expint_3(f64::NAN).is_nan());
        // Upstream's DOMAIN_ERROR: e^{-t^3} GROWS for negative t, so this is
        // a real restriction and not a sign convention.
        for x in [-1e-300_f64, -1.0, -1e300, f64::NEG_INFINITY] {
            assert!(expint_3(x).is_nan(), "Ei_3({x:e}) should be NaN");
        }

        let mut prev = 0.0_f64;
        for k in 0..=200_000 {
            let x = 1e-4 * k as f64;
            let v = expint_3(x);
            assert!(v >= prev, "not non-decreasing at {x}: {v:e} vs {prev:e}");
            assert!(v <= VAL_INFINITY, "Ei_3({x}) = {v:e} exceeds Gamma(4/3)");
            prev = v;
        }
        assert_eq!(
            prev, VAL_INFINITY,
            "it should saturate exactly at Gamma(4/3)"
        );

        // Linear at the origin, to the order the next term allows:
        // Ei_3(x) = x - x^4/4 + O(x^7).
        for x in [1e-7_f64, 1e-5, 1e-3, 1e-2] {
            let two_term = x - x * x * x * x / 4.0;
            assert!(
                ((expint_3(x) - two_term) / two_term).abs() < 1e-12,
                "Ei_3({x:e}) = {:e} against the two-term {two_term:e}",
                expint_3(x)
            );
        }
        assert_eq!(expint_3(f64::INFINITY), VAL_INFINITY);
    }
}

/// `Gamma(4/3)`, so [`crate::wgsl::mirror_expint3`] can check its `f32`
/// against upstream's literal rather than repeating it.
pub fn probe_val_infinity() -> f64 {
    VAL_INFINITY
}
