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
// PORTED from the GNU Scientific Library (GSL) 2.8, specfunc/clausen.c
// <https://www.gnu.org/software/gsl/>
// Copyright (C) 1996-2000 Gerard Jungman
// GSL is free software under the GNU General Public License, version 3 or
// later.
//
// The one Chebyshev table here was extracted from that source by script, not
// retyped, and is audited bit-for-bit by tests/gsl_tables_audit.rs.

//! The Clausen function `Cl_2(x)`.
//!
//! # What this is
//!
//! ```text
//!     Cl_2(x) = - integral_0^x ln|2 sin(t/2)| dt
//!             = sum_{k=1}^{inf} sin(k x) / k^2
//! ```
//!
//! It is the imaginary part of `Li_2(e^{i x})`, which makes it the
//! trigonometric sibling of [`crate::specfunc::dilog`] — and the reason the
//! two are cross-checked against each other in the tests rather than each
//! being trusted alone.
//!
//! It appears in the volumes of hyperbolic tetrahedra, in lattice Green's
//! functions, and wherever a log-sine integral has to be evaluated in closed
//! form.
//!
//! # Argument range, stated plainly
//!
//! **All real `x`**, dimensionless (`f64`), interpreted as an angle in
//! radians. `Cl_2` is odd and `2 pi`-periodic, with `Cl_2(0) = Cl_2(pi) = 0`
//! and a maximum of `1.0149416...` at `x = pi/3`.
//!
//! The one refusal is inherited from the argument reduction: `|x|` above
//! `0.0625 / DBL_EPSILON`, about 2.815e+14, returns `NaN`, because the
//! reduced angle would carry no information. See
//! [`crate::specfunc::trig`], and note its stronger practical warning —
//! the reduction is only worth full precision below about `1e7`, so `Cl_2`
//! at a very large argument is periodic in name rather than in value.
//!
//! `NaN` propagates.
//!
//! # Accuracy
//!
//! Measured against the **Fourier series** `sum sin(k x) / k^2`, summed
//! directly, which shares no coefficient, branch or reduction with the
//! implementation. Results are in the tests.

// Under a std-linked build (`cargo test`) f64's inherent ln/sin shadow these
// trait methods, leaving the import formally unused. See crate::real.
#[allow(unused_imports)]
use crate::real::Real;

use crate::specfunc::{trig::angle_restrict_pos, SQRT_DBL_EPSILON};

/// GSL's `aclaus_cs`, 15 coefficients, order 14 — the whole array is used.
///
/// Upstream's `order_sp` field carries the comment
/// *"FIXME: this is a guess, correct value needed here BJG"*. That only
/// affects `GSL_PREC_SINGLE`, which this port does not carry, so the guess is
/// not inherited — but it is recorded here because a reader comparing
/// against upstream will meet it.
#[rustfmt::skip]
const ACLAUS: [f64; 15] = [
    2.142694363766688447e+00, 0.723324281221257925e-01, 0.101642475021151164e-02,
    0.3245250328531645e-04, 0.133315187571472e-05, 0.6213240591653e-07,
    0.313004135337e-08, 0.16635723056e-09, 0.919659293e-11, 0.52400462e-12,
    0.3058040e-13, 0.18197e-14, 0.1100e-15, 0.68e-17, 0.4e-18,
];

/// `Cl_2(x)`, GSL's `gsl_sf_clausen`.
///
/// Odd and `2 pi`-periodic. Returns `NaN` for `|x|` past the argument
/// reduction's loss cut — see the module documentation.
///
/// # Examples
///
/// ```
/// use petir::specfunc::clausen::clausen;
/// use core::f64::consts::PI;
/// // Cl_2(pi/2) is Catalan's constant.
/// assert!((clausen(PI / 2.0) - 0.915_965_594_177_219_0).abs() < 1e-14);
/// // Odd, and zero at 0 and pi.
/// assert_eq!(clausen(0.0), 0.0);
/// assert!(clausen(PI).abs() < 1e-15);
/// assert!((clausen(-1.0) + clausen(1.0)).abs() < 1e-15);
/// ```
pub fn clausen(x: f64) -> f64 {
    if x.is_nan() {
        return f64::NAN;
    }
    let x_cut = core::f64::consts::PI * SQRT_DBL_EPSILON;

    let mut x = x;
    let mut sgn = 1.0_f64;
    if x < 0.0 {
        x = -x;
        sgn = -1.0;
    }

    // Reduce to [0, 2pi), then to [0, pi).
    x = angle_restrict_pos(x);
    if x.is_nan() {
        return f64::NAN;
    }
    if x > core::f64::consts::PI {
        // Simulated extra precision: 2 pi = p0 + p1, so that pi - x is formed
        // without first rounding 2 pi. Upstream's own constants.
        const P0: f64 = 6.28125;
        const P1: f64 = 0.193_530_717_958_647_692_53e-2;
        x = (P0 - x) + P1;
        sgn = -sgn;
    }

    let val = if x == 0.0 {
        0.0
    } else if x < x_cut {
        // Cl_2(x) ~ x(1 - ln x) as x -> 0. Not an optimisation: the
        // Chebyshev branch below would compute ln(x) - c(t) as a difference
        // of two large quantities there.
        x * (1.0 - x.ln())
    } else {
        let t = 2.0 * (x * x / (core::f64::consts::PI * core::f64::consts::PI) - 0.5);
        x * (crate::cheb_slice::eval_gsl(t, &ACLAUS) - x.ln())
    };

    sgn * val
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::f64::consts::PI;

    /// Catalan's constant, `Cl_2(pi/2)`, to the digits OEIS A006752 gives.
    const CATALAN: f64 = 0.915_965_594_177_219_015;

    /// The Fourier series `sum_{k>=1} sin(k x) / k^2`, summed directly.
    ///
    /// Shares no coefficient, branch or argument reduction with the
    /// implementation — it is the definition. Convergence is only `1/k^2`,
    /// so the tail after `N` terms is `O(1/(N |sin(x/2)|))`; the tolerance
    /// below is set from that, not from what `Cl_2` can do.
    fn fourier(x: f64, n: u32) -> f64 {
        let mut s = 0.0_f64;
        for k in (1..=n).rev() {
            s += (k as f64 * x).sin() / (k as f64 * k as f64);
        }
        s
    }

    /// **Against the defining Fourier series `sum sin(k x) / k^2`.**
    ///
    /// # Results, measured 2026-09-19
    ///
    /// Worst absolute difference over 120 points in `(0, pi)` with
    /// `N = 2 000 000` terms: **9.396e-12**, at `x = 0.026`.
    ///
    /// The worst point is at small `x`, which is where the series converges
    /// most slowly relative to the value — and the residual is the
    /// **series'** truncation, not `Cl_2`'s error. That is asserted below
    /// rather than claimed: at `x = 1.3` the difference falls from 6.104e-09
    /// at `N = 1e4` to 1.986e-13 at `N = 2e6`, which an error belonging to
    /// `Cl_2` could not do.
    ///
    /// Absolute rather than relative, because `Cl_2` has zeros at `0` and
    /// `pi` and the sweep runs between them.
    #[test]
    fn it_matches_the_defining_fourier_series() {
        let (mut worst, mut at) = (0.0_f64, 0.0_f64);
        for k in 1..=120 {
            let x = PI * k as f64 / 121.0;
            let r = fourier(x, 2_000_000);
            let e = (clausen(x) - r).abs();
            if e > worst {
                worst = e;
                at = x;
            }
        }
        assert!(worst < 1e-9, "Cl_2 vs Fourier: {worst:e} at {at}");

        // AND THE RESIDUAL IS THE SERIES' TRUNCATION, NOT Cl_2's ERROR.
        // If it were Cl_2's it would not move with N. Measured at x = 1.3:
        // 6.104e-09 (N = 1e4), 2.050e-12 (1e5), 8.120e-13 (1e6),
        // 1.986e-13 (2e6) -- three and a half orders across the sweep.
        let x = 1.3_f64;
        let coarse = (clausen(x) - fourier(x, 10_000)).abs();
        let fine = (clausen(x) - fourier(x, 1_000_000)).abs();
        assert!(
            coarse > 100.0 * fine,
            "the residual is documented as the Fourier series' truncation \
             rather than Cl_2's error, because it falls with N: {coarse:e} at \
             N = 1e4 against {fine:e} at N = 1e6. If it has stopped falling, \
             the residual now has a component Cl_2 owns"
        );
    }

    /// The exact and published values, which pin the branch constants.
    #[test]
    fn the_known_values_are_reproduced() {
        assert_eq!(clausen(0.0), 0.0);
        assert!(clausen(PI).abs() < 1e-15, "Cl_2(pi) = {}", clausen(PI));
        assert!(
            (clausen(PI / 2.0) - CATALAN).abs() < 1e-14,
            "Cl_2(pi/2) = {} vs Catalan {CATALAN}",
            clausen(PI / 2.0)
        );
        // The maximum, at x = pi/3.
        let m = clausen(PI / 3.0);
        assert!(
            (m - 1.014_941_606_409_653_6).abs() < 1e-14,
            "Cl_2(pi/3) = {m}"
        );
        // And it really is the maximum.
        for k in 0..=400 {
            let x = 2.0 * PI * k as f64 / 400.0;
            assert!(
                clausen(x) <= m + 1e-15,
                "Cl_2({x}) = {} exceeds the max",
                clausen(x)
            );
        }
    }

    /// Odd, `2 pi`-periodic, and antisymmetric about `pi` — the three
    /// structural identities, each of which crosses the sign flip and the
    /// argument reduction.
    #[test]
    fn the_symmetries_hold() {
        for k in 1..=200 {
            let x = 0.03 * k as f64;
            assert!(
                (clausen(-x) + clausen(x)).abs() < 1e-15,
                "odd at {x}: {} vs {}",
                clausen(-x),
                clausen(x)
            );
            let p = clausen(x + 2.0 * PI);
            assert!(
                (p - clausen(x)).abs() < 1e-14,
                "periodic at {x}: {p} vs {}",
                clausen(x)
            );
            // Cl_2(2pi - x) = -Cl_2(x).
            assert!(
                (clausen(2.0 * PI - x) + clausen(x)).abs() < 1e-14,
                "reflection at {x}"
            );
        }
    }

    /// **The duplication formula `Cl_2(2x) = 2 Cl_2(x) - 2 Cl_2(pi - x)`.**
    ///
    /// A genuine test of the branch structure rather than a tautology: the
    /// three arguments land in different windows of the reduction, and
    /// `Cl_2(2x)` crosses `pi` for `x > pi/2` where the sign flips.
    #[test]
    fn the_duplication_formula_holds() {
        let (mut worst, mut at) = (0.0_f64, 0.0_f64);
        for k in 1..=300 {
            let x = PI * k as f64 / 301.0;
            let lhs = clausen(2.0 * x);
            let rhs = 2.0 * clausen(x) - 2.0 * clausen(PI - x);
            let e = (lhs - rhs).abs();
            if e > worst {
                worst = e;
                at = x;
            }
        }
        assert!(worst < 1e-13, "duplication: {worst:e} at {at}");
    }

    /// The small-argument branch joins the Chebyshev one, compared as two
    /// **formulas** at one argument rather than as the function either side.
    #[test]
    fn the_small_argument_branch_joins() {
        let x = PI * SQRT_DBL_EPSILON;
        let small = x * (1.0 - x.ln());
        let t = 2.0 * (x * x / (PI * PI) - 0.5);
        let cheb = x * (crate::cheb_slice::eval_gsl(t, &ACLAUS) - x.ln());
        assert!(
            ((small - cheb) / cheb).abs() < 1e-12,
            "join at x_cut = {x:e}: {small:e} vs {cheb:e}"
        );
        // And the asymptote itself, well inside the small branch.
        for x in [1e-12_f64, 1e-9, 1e-6] {
            let want = x * (1.0 - x.ln());
            assert!(
                ((clausen(x) - want) / want).abs() < 1e-15,
                "Cl_2({x:e}) vs x(1 - ln x)"
            );
        }
    }

    /// The refusal inherited from the argument reduction, and `NaN`.
    #[test]
    fn the_refusal_is_inherited_from_the_reduction() {
        for x in [1e15_f64, -1e15, f64::INFINITY, f64::NEG_INFINITY] {
            assert!(clausen(x).is_nan(), "Cl_2({x:e})");
        }
        assert!(clausen(f64::NAN).is_nan());
        // Just inside the cut it still answers.
        let inside = 0.0625 / crate::specfunc::DBL_EPSILON * 0.999;
        assert!(clausen(inside).is_finite(), "Cl_2({inside:e})");
    }
}
