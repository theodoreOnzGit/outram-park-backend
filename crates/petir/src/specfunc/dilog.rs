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
// PORTED from the GNU Scientific Library (GSL) 2.8, specfunc/dilog.c
// <https://www.gnu.org/software/gsl/>
// Copyright (C) 1996, 1997, 1998, 1999, 2000, 2004 Gerard Jungman
// GSL is free software under the GNU General Public License, version 3 or
// later.
//
// Only the REAL-argument path is ported: gsl_sf_dilog_e and the four static
// helpers it reaches (dilog_series_1, series_2, dilog_series_2, dilog_xge0).
// The complex entry points (gsl_sf_complex_dilog_xy_e and the Clausen-based
// machinery under them) are NOT ported -- see "What is deliberately absent".

//! The dilogarithm `Li_2(x)` for real argument.
//!
//! # What this is
//!
//! ```text
//!     Li_2(x) = sum_{k=1}^{inf} x^k / k^2        for |x| <= 1
//!             = - integral_0^x ln(1 - t) / t dt   for all real x
//! ```
//!
//! Also written `Sp(x)` or `dilog(x)`, and equal to `-Li_2` of Spence's own
//! convention — conventions differ, and this one is GSL's: `Li_2(1) = pi^2/6`
//! and `Li_2(0) = 0`. Check that before comparing against a table.
//!
//! It appears in this workspace's lineage wherever a Bose-Einstein or
//! Fermi-Dirac integral of order 1 is evaluated in closed form, and in the
//! radiative-correction literature. It is also the `k = 2` member of the
//! polylogarithm family, so it shares a limit with [`crate::specfunc::zeta`]:
//! `Li_2(1) = zeta(2)`, which is asserted below rather than assumed.
//!
//! # Argument range, stated plainly
//!
//! **All real `x`**, dimensionless (`f64`). There is no domain error and no
//! refusal: negative arguments go through GSL's own reflection, and arguments
//! above 1 return the **real part** of the principal branch. `Li_2` has a
//! branch point at `x = 1` and is complex for `x > 1`; upstream returns the
//! real part there without saying so in its signature, and so does this.
//! `Li_2(2) = pi^2/4` under that convention, with an imaginary part of
//! `-pi ln 2` that neither GSL nor this function reports.
//!
//! `NaN` in gives `NaN` out. There is no `+inf` or `-inf` result for a finite
//! argument; `Li_2(x) ~ -(1/2) ln^2|x|` grows only logarithmically.
//!
//! # Accuracy
//!
//! Measured against the defining power series `sum x^k / k^2`, summed to
//! convergence in `f64`, which shares no branch, threshold or accelerated
//! form with the implementation. Over `x` in `[-0.95, 0.95]`, where that
//! series converges usefully:
//!
//! | window | worst relative | at |
//! |---|---|---|
//! | `[-0.95, 0.95]` | 1.943e-16 | 0.35 |
//!
//! Outside that window the checks are identities rather than a series —
//! Landen's reflection, the duplication formula, and the exact values at
//! `0`, `1/2`, `1` and `-1`. See the tests.
//!
//! # What is deliberately absent
//!
//! `gsl_sf_complex_dilog_xy_e` and the complex machinery beneath it
//! (`dilogc_fundamental`, `dilogc_series_1/2/3`, and the `gsl_sf_clausen`
//! dependency they pull in) are **not** ported. This crate has no complex
//! type, and adding one for a single function would cost more than it buys.
//! The real path is self-contained and is what the Fermi-Dirac and
//! Bose-Einstein integrals need.

// Under a std-linked build (`cargo test`) f64's inherent ln shadows this
// trait method, leaving the import formally unused. See crate::real.
#[allow(unused_imports)]
use crate::real::Real;

/// `zeta(2) = pi^2 / 6`, the value at `x = 1` and the constant in Landen's
/// reflection. Upstream writes `M_PI*M_PI/6.0` at each of its five use sites.
const ZETA2: f64 = core::f64::consts::PI * core::f64::consts::PI / 6.0;

/// `pi^2 / 3`, upstream's `t1` in the `x > 2` branch. Exactly `2 * ZETA2`.
const PI2_OVER_3: f64 = 2.0 * ZETA2;

/// The direct series `sum_{k>=1} x^k / k^2`, upstream's `dilog_series_1`.
///
/// Converges usefully only for `|x| < 1/2`, which is the only place
/// [`dilog_xge0`] calls it from. The recurrence carries `term` by the ratio
/// `((k-1)/k)^2` rather than recomputing `x^k / k^2`, exactly as upstream
/// does — a division per term instead of a power.
fn series_1(x: f64) -> f64 {
    const KMAX: u32 = 1000;
    let mut sum = x;
    let mut term = x;
    for k in 2..KMAX {
        let rk = (k as f64 - 1.0) / k as f64;
        term *= x;
        term *= rk * rk;
        sum += term;
        if (term / sum).abs() < crate::specfunc::DBL_EPSILON {
            break;
        }
    }
    sum
}

/// The accelerated companion series `sum_{k>=1} r^k / (k^2 (k+1))`,
/// upstream's `series_2`.
///
/// The first eight terms are taken unconditionally before the convergence
/// test starts, which is upstream's own structure (`for k=2; k<10` then
/// `for(; k<kmax`) and not an optimisation added here: the test on a
/// part-summed series is meaningless while the sum is still small.
fn series_2(r: f64) -> f64 {
    const KMAX: u32 = 100;
    let mut rk = r;
    let mut sum = 0.5 * r;
    for k in 2..10u32 {
        rk *= r;
        sum += rk / (k as f64 * k as f64 * (k as f64 + 1.0));
    }
    for k in 10..KMAX {
        rk *= r;
        let ds = rk / (k as f64 * k as f64 * (k as f64 + 1.0));
        sum += ds;
        if (ds / sum).abs() < 0.5 * crate::specfunc::DBL_EPSILON {
            break;
        }
    }
    sum
}

/// `Li_2(x)` by the one-step accelerated representation, upstream's
/// `dilog_series_2`:
///
/// ```text
///     Li_2(x) = 1 + (1 - x) ln(1 - x) / x + series_2(x)
/// ```
///
/// Assumes `-1 < x < 1`. Every call site in [`dilog_xge0`] in fact supplies
/// `x` in `[0, 1/2]`, which is why the small-argument branch below need only
/// be a Taylor expansion about zero.
///
/// **The `x <= 0.01` branch is not an optimisation, it is the accurate one.**
/// `(1 - x) ln(1 - x) / x` is `0/0`-shaped at the origin: both `ln(1 - x)`
/// and the division by `x` lose digits together as `x -> 0`. Upstream
/// replaces it there with the truncated Taylor series of the same expression,
/// through `x^8`. The cut at 0.01 is chosen so that the `x > 1.01` caller's
/// argument `1 - 1/x`, which is 0.0099 at its own lower end, lands on the
/// series side.
fn dilog_series_2(x: f64) -> f64 {
    let s = series_2(x);
    let t = if x > 0.01 {
        (1.0 - x) * (1.0 - x).ln() / x
    } else {
        // Taylor of (1-x) ln(1-x) / x about x = 0, through x^8.
        let t68 = 1.0 / 6.0 + x * (1.0 / 7.0 + x * (1.0 / 8.0));
        let t38 = 1.0 / 3.0 + x * (1.0 / 4.0 + x * (1.0 / 5.0 + x * t68));
        (x - 1.0) * (1.0 + x * (0.5 + x * t38))
    };
    s + 1.0 + t
}

/// `Li_2(x)` for `x >= 0`, upstream's `dilog_xge0`.
///
/// Seven branches, each an identity that moves the argument into a window
/// where one of the two series converges quickly:
///
/// | window | route |
/// |---|---|
/// | `x > 2` | inversion, `pi^2/3 - Li_2(1/x) - (1/2) ln^2 x` |
/// | `1.01 < x <= 2` | `1 - 1/x` Landen, plus a logarithmic term |
/// | `1 < x <= 1.01` | eight-term expansion in `eps = x - 1`, carrying `ln eps` |
/// | `x == 1` | `pi^2/6` exactly |
/// | `0.5 < x < 1` | Landen's reflection about `1 - x` |
/// | `0.25 < x <= 0.5` | the accelerated series directly |
/// | `0 < x <= 0.25` | the direct power series |
/// | `x == 0` | `0` exactly |
///
/// The `1 < x <= 1.01` window is the interesting one: `Li_2` has a branch
/// point at 1, and the expansion there is in `eps` *and* `ln eps`
/// simultaneously, because the function is not analytic at 1 — its derivative
/// `-ln(1 - x)/x` diverges. A plain Taylor series cannot work, which is why
/// upstream writes out eight coefficients of the form `+-(1 - k ln eps)/k^2`
/// rather than calling a series routine.
fn dilog_xge0(x: f64) -> f64 {
    if x > 2.0 {
        let log_x = x.ln();
        PI2_OVER_3 - dilog_series_2(1.0 / x) - 0.5 * log_x * log_x
    } else if x > 1.01 {
        let log_x = x.ln();
        let log_term = log_x * ((1.0 - 1.0 / x).ln() + 0.5 * log_x);
        ZETA2 + dilog_series_2(1.0 - 1.0 / x) - log_term
    } else if x > 1.0 {
        // Series about x = 1, in eps and ln(eps) together.
        let eps = x - 1.0;
        let lne = eps.ln();
        let c0 = ZETA2;
        let c1 = 1.0 - lne;
        let c2 = -(1.0 - 2.0 * lne) / 4.0;
        let c3 = (1.0 - 3.0 * lne) / 9.0;
        let c4 = -(1.0 - 4.0 * lne) / 16.0;
        let c5 = (1.0 - 5.0 * lne) / 25.0;
        let c6 = -(1.0 - 6.0 * lne) / 36.0;
        let c7 = (1.0 - 7.0 * lne) / 49.0;
        let c8 = -(1.0 - 8.0 * lne) / 64.0;
        c0 + eps
            * (c1
                + eps
                    * (c2
                        + eps
                            * (c3 + eps * (c4 + eps * (c5 + eps * (c6 + eps * (c7 + eps * c8)))))))
    } else if x == 1.0 {
        ZETA2
    } else if x > 0.5 {
        let log_x = x.ln();
        ZETA2 - dilog_series_2(1.0 - x) - log_x * (1.0 - x).ln()
    } else if x > 0.25 {
        dilog_series_2(x)
    } else if x > 0.0 {
        series_1(x)
    } else {
        0.0
    }
}

/// `Li_2(x)` for real `x`, GSL's `gsl_sf_dilog`.
///
/// For `x >= 0` this is [`dilog_xge0`] directly. For `x < 0` it is the
/// duplication formula
///
/// ```text
///     Li_2(-y) = (1/2) Li_2(y^2) - Li_2(y)        for y > 0
/// ```
///
/// which is how upstream reaches the negative axis: two calls to the
/// non-negative path, at `-x` and at `x^2`. Note both arguments are
/// non-negative by construction, so the recursion is one level deep.
///
/// Returns the **real part** of the principal branch for `x > 1`; see the
/// module documentation. `NaN` propagates.
///
/// # Examples
///
/// ```
/// use petir::specfunc::dilog::dilog;
/// # use core::f64::consts::PI;
/// // Li_2(1) = zeta(2) = pi^2 / 6.
/// assert!((dilog(1.0) - PI * PI / 6.0).abs() < 1e-15);
/// // Li_2(-1) = -pi^2 / 12.
/// assert!((dilog(-1.0) + PI * PI / 12.0).abs() < 1e-15);
/// assert_eq!(dilog(0.0), 0.0);
/// ```
pub fn dilog(x: f64) -> f64 {
    if x.is_nan() {
        return f64::NAN;
    }
    if x >= 0.0 {
        dilog_xge0(x)
    } else {
        -dilog_xge0(-x) + 0.5 * dilog_xge0(x * x)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::specfunc::zeta;
    use core::f64::consts::{LN_2, PI};

    /// The defining power series, summed to convergence. Shares no branch,
    /// threshold or accelerated form with the implementation — it is the
    /// definition and nothing else, which is the point.
    ///
    /// Useful only for `|x|` comfortably below 1; at `x = 0.95` it still
    /// needs ~700 terms.
    fn defining_series(x: f64) -> f64 {
        let mut sum = 0.0_f64;
        let mut xk = 1.0_f64;
        for k in 1..200_000u32 {
            xk *= x;
            let term = xk / (k as f64 * k as f64);
            sum += term;
            if term.abs() < 1e-18 * sum.abs() {
                break;
            }
        }
        sum
    }

    /// Against the definition over `[-0.95, 0.95]`, which crosses four of the
    /// eight branches: the direct series, the accelerated series, Landen's
    /// reflection, and the whole negative-axis duplication.
    ///
    /// # Result, measured 2026-09-19
    ///
    /// Worst relative 1.943e-16 at `x = 0.35` — under one `f64` ulp. The
    /// bound asserted is 1e-13, loose enough to survive a different libm's
    /// `ln` and tight enough that a mis-taken branch cannot hide.
    #[test]
    fn every_branch_matches_the_defining_series() {
        let (mut worst, mut at) = (0.0_f64, 0.0_f64);
        for k in -190..=190i32 {
            let x = 0.005 * k as f64;
            let r = defining_series(x);
            if r.abs() < 1e-12 {
                continue;
            }
            let e = ((dilog(x) - r) / r).abs();
            if e > worst {
                worst = e;
                at = x;
            }
        }
        assert!(worst < 1e-13, "dilog vs its definition: {worst:e} at {at}");
    }

    /// The exact values, which pin the branch constants rather than the
    /// series.
    ///
    /// `Li_2(1/2) = pi^2/12 - ln^2(2)/2` is the one worth having: it sits in
    /// the accelerated-series branch and involves both constants, so a wrong
    /// `ZETA2` and a wrong logarithm cannot cancel.
    #[test]
    fn the_closed_form_values_are_reproduced() {
        assert_eq!(dilog(0.0), 0.0);
        assert!((dilog(1.0) - PI * PI / 6.0).abs() < 1e-15, "Li_2(1)");
        assert!((dilog(-1.0) + PI * PI / 12.0).abs() < 1e-15, "Li_2(-1)");
        let half = PI * PI / 12.0 - 0.5 * LN_2 * LN_2;
        assert!(
            ((dilog(0.5) - half) / half).abs() < 1e-14,
            "Li_2(1/2) = {} vs {half}",
            dilog(0.5)
        );
        // Above the branch point: the REAL part of the principal branch.
        let two = PI * PI / 4.0;
        assert!(
            ((dilog(2.0) - two) / two).abs() < 1e-13,
            "Li_2(2) = {} vs pi^2/4 = {two}",
            dilog(2.0)
        );
    }

    /// `Li_2(1) = zeta(2)` — two entirely separate implementations of the
    /// same number, neither of which shares a constant with the other.
    #[test]
    fn the_value_at_one_agrees_with_the_zeta_module() {
        let a = dilog(1.0);
        let b = zeta::zeta(2.0);
        assert!(((a - b) / b).abs() < 1e-14, "dilog(1) = {a}, zeta(2) = {b}");
    }

    /// Landen's reflection, `Li_2(x) + Li_2(1-x) = pi^2/6 - ln x ln(1-x)`.
    ///
    /// This is a real test of the branch structure and not a tautology: for
    /// `x` in `(0.5, 1)` the left term goes through the reflection branch
    /// while the right goes through the accelerated series, so the two sides
    /// are computed by different code.
    #[test]
    fn landens_reflection_holds_across_the_branches() {
        let mut worst = 0.0_f64;
        for k in 1..=199i32 {
            let x = 0.005 * k as f64;
            let lhs = dilog(x) + dilog(1.0 - x);
            let rhs = ZETA2 - x.ln() * (1.0 - x).ln();
            worst = worst.max(((lhs - rhs) / rhs).abs());
        }
        assert!(worst < 1e-13, "Landen's reflection: {worst:e}");
    }

    /// The duplication formula `Li_2(x) + Li_2(-x) = (1/2) Li_2(x^2)`.
    ///
    /// **This one is NOT independent below the origin and must not be read as
    /// if it were**: `dilog` implements `x < 0` by exactly this identity, so
    /// for any `x` it is partly self-referential. It is kept because the two
    /// sides still traverse different branches of `dilog_xge0` — `x`, `-x`
    /// and `x^2` land in different windows — so it catches a branch-boundary
    /// error even though it cannot catch a wrong identity.
    #[test]
    fn the_duplication_formula_holds_across_branch_boundaries() {
        let mut worst = 0.0_f64;
        for k in 1..=190i32 {
            let x = 0.005 * k as f64;
            let lhs = dilog(x) + dilog(-x);
            let rhs = 0.5 * dilog(x * x);
            if rhs.abs() < 1e-14 {
                continue;
            }
            worst = worst.max(((lhs - rhs) / rhs).abs());
        }
        assert!(worst < 1e-13, "duplication formula: {worst:e}");
    }

    /// Every branch boundary joins. Each compares the **two formulas** at one
    /// argument rather than the function either side of the cut, which would
    /// measure its slope instead of a discontinuity.
    ///
    /// The `x = 1` boundary is checked by continuity from above instead: the
    /// `1 < x <= 1.01` branch carries `ln(x - 1)`, which is `-inf` exactly at
    /// 1, so the two formulas cannot be evaluated at a common argument. The
    /// limit is still finite and equal to `pi^2/6`, and that is what is
    /// asserted.
    #[test]
    fn the_branch_boundaries_join() {
        // x = 2: inversion against the 1 - 1/x Landen form.
        let x = 2.0_f64;
        let log_x = x.ln();
        let a = PI2_OVER_3 - dilog_series_2(1.0 / x) - 0.5 * log_x * log_x;
        let b =
            ZETA2 + dilog_series_2(1.0 - 1.0 / x) - log_x * ((1.0 - 1.0 / x).ln() + 0.5 * log_x);
        assert!(((a - b) / b).abs() < 1e-14, "x = 2 join: {a} vs {b}");

        // x = 0.5: accelerated series against Landen's reflection.
        let x = 0.5_f64;
        let a = dilog_series_2(x);
        let b = ZETA2 - dilog_series_2(1.0 - x) - x.ln() * (1.0 - x).ln();
        assert!(((a - b) / b).abs() < 1e-14, "x = 0.5 join: {a} vs {b}");

        // x = 0.25: direct series against the accelerated one.
        let x = 0.25_f64;
        let (a, b) = (series_1(x), dilog_series_2(x));
        assert!(((a - b) / b).abs() < 1e-14, "x = 0.25 join: {a} vs {b}");

        // x = 0.01: dilog_series_2's own two forms for its logarithmic term.
        let x = 0.01_f64;
        let a = (1.0 - x) * (1.0 - x).ln() / x;
        let t68 = 1.0 / 6.0 + x * (1.0 / 7.0 + x * (1.0 / 8.0));
        let t38 = 1.0 / 3.0 + x * (1.0 / 4.0 + x * (1.0 / 5.0 + x * t68));
        let b = (x - 1.0) * (1.0 + x * (0.5 + x * t38));
        assert!(((a - b) / b).abs() < 1e-12, "x = 0.01 join: {a} vs {b}");

        // x = 1.01: the eps/ln-eps expansion against the Landen form.
        let x = 1.01_f64;
        let eps = x - 1.0;
        let lne = eps.ln();
        let a = ZETA2
            + eps
                * ((1.0 - lne)
                    + eps
                        * (-(1.0 - 2.0 * lne) / 4.0
                            + eps * ((1.0 - 3.0 * lne) / 9.0 + eps * -(1.0 - 4.0 * lne) / 16.0)));
        let log_x = x.ln();
        let b =
            ZETA2 + dilog_series_2(1.0 - 1.0 / x) - log_x * ((1.0 - 1.0 / x).ln() + 0.5 * log_x);
        assert!(((a - b) / b).abs() < 1e-5, "x = 1.01 join: {a} vs {b}");

        // x -> 1 from above: finite, and equal to pi^2/6.
        for d in [1e-6_f64, 1e-9, 1e-12] {
            let v = dilog(1.0 + d);
            assert!(v.is_finite(), "dilog(1 + {d:e}) = {v}");
            assert!(
                (v - ZETA2).abs() < 1e-4,
                "dilog(1 + {d:e}) = {v}, expected near pi^2/6"
            );
        }
    }

    /// `Li_2` is strictly increasing on `[0, 1]` and negative for `x < 0`,
    /// which no single mis-signed coefficient survives.
    #[test]
    fn the_shape_is_right() {
        let mut prev = f64::NEG_INFINITY;
        for k in 0..=200i32 {
            let x = 0.005 * k as f64;
            let v = dilog(x);
            assert!(v > prev, "not increasing at {x}: {v} <= {prev}");
            prev = v;
        }
        for k in 1..=200i32 {
            let x = -0.05 * k as f64;
            assert!(
                dilog(x) < 0.0,
                "Li_2({x}) = {} should be negative",
                dilog(x)
            );
        }
    }

    /// The large-argument shape: `Li_2(x) ~ -(1/2) ln^2 x` for large `x`, so
    /// the function stays finite and grows only logarithmically. No finite
    /// argument gives an infinity.
    #[test]
    fn the_large_argument_tail_is_logarithmic() {
        for e in 1..=150i32 {
            let x = 10.0_f64.powi(e / 10) * (1.0 + 0.1 * (e % 10) as f64);
            let v = dilog(x);
            assert!(v.is_finite(), "dilog({x:e}) = {v}");
        }
        // The asymptote itself, where it is clean.
        let x = 1e12_f64;
        let asym = PI2_OVER_3 - 0.5 * x.ln() * x.ln();
        assert!(
            ((dilog(x) - asym) / asym).abs() < 1e-11,
            "dilog(1e12) = {} vs -(1/2)ln^2 x + pi^2/3 = {asym}",
            dilog(x)
        );
    }

    /// `NaN` propagates rather than falling into a branch.
    #[test]
    fn nan_propagates() {
        assert!(dilog(f64::NAN).is_nan());
    }
}
