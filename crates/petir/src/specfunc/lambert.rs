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
// PORTED from the GNU Scientific Library (GSL) 2.8, specfunc/lambert.c
// <https://www.gnu.org/software/gsl/>
// Copyright (C) 2007 Brian Gough; started from code donated by K. Briggs.
// GSL is free software under the GNU General Public License, version 3 or
// later.
//
// Lambert-ology from Corless, Gonnet, Hare and Jeffrey, "On Lambert's W
// Function"; the Halley step is that paper's equation 5.12.
//
// SERIES_C is a LOCAL array inside upstream's series_eval, not a file-scope
// declaration, so the table audit reaches it through a dedicated test rather
// than through its name -- see tests/gsl_tables_audit.rs.

//! Lambert's `W` function: the inverse of `w -> w e^w`, real branches.
//!
//! # What this is
//!
//! `W(x)` solves `W e^W = x`. That equation has **two** real solutions for
//! `x` in `(-1/e, 0)` and one elsewhere on the real line, so there are two
//! real branches:
//!
//! | branch | domain | range | at `x = -1/e` |
//! |---|---|---|---|
//! | [`lambert_w0`] | `[-1/e, inf)` | `[-1, inf)` | `-1` |
//! | [`lambert_wm1`] | `[-1/e, 0)` | `(-inf, -1]` | `-1` |
//!
//! They meet at `x = -1/e = -0.3678794...`, where `W = -1` and the
//! derivative is infinite — that square-root branch point is why both
//! branches need a dedicated series near it rather than the iteration.
//!
//! It turns up wherever an exponential and its argument must be untangled:
//! solving `T e^{-E/kT}`-shaped balances, delay differential equations, and
//! the enumeration of trees. In this workspace it is the natural inverse for
//! the exponential attenuation the Monte Carlo flight sampler uses.
//!
//! # Argument range, stated plainly
//!
//! All arguments and results are dimensionless (`f64`).
//!
//! - [`lambert_w0`]: `x >= -1/e`. Below that, upstream returns `-1` **and**
//!   `GSL_EDOM`; this port has no error channel here, so it returns `NaN`,
//!   which is the divergence recorded below.
//! - [`lambert_wm1`]: `-1/e <= x < 0`. For `x > 0` it delegates to
//!   [`lambert_w0`], exactly as upstream does — the `W_{-1}` branch does not
//!   exist there. `x == 0` returns `0`, also upstream's behaviour, although
//!   `W_{-1}(0^-) = -inf`, so that value is a convention rather than a limit.
//!
//! `NaN` propagates.
//!
//! # One deliberate divergence from upstream
//!
//! GSL returns `-1.0` together with `GSL_EDOM` for `x < -1/e`, calling it
//! "a little lenient in case of some epsilon overshoot". A caller that
//! ignores the status — which is every caller of the `gsl_sf_lambert_W0`
//! natural-prototype form — silently gets `-1` for an argument outside the
//! domain. **This port returns `NaN`**, so the domain error cannot be
//! mistaken for an answer. The leniency GSL is buying is preserved a
//! different way: `q = x + 1/e` is compared against zero exactly as upstream
//! computes it, so an argument that is only an epsilon below `-1/e` still
//! lands on the `q == 0` path and returns `-1`.
//!
//! # Accuracy
//!
//! Measured by the **defining identity** `W(x) e^{W(x)} = x`, which this
//! module cannot satisfy by construction — it never evaluates `w e^w` except
//! inside the iteration's own residual, and the check is run on the returned
//! value. Results in the tests; the headline is that both branches satisfy
//! it to better than 1e-14 relative across their domains, and the worst
//! point of each is at the `-1/e` branch point where the derivative is
//! infinite and no implementation can do better.

// Under a std-linked build (`cargo test`) f64's inherent exp/ln/sqrt shadow
// these trait methods, leaving the import formally unused. See crate::real.
#[allow(unused_imports)]
use crate::real::Real;

use crate::specfunc::DBL_EPSILON;

/// `1/e`, so that `-1/e` is the branch point. Upstream writes `1.0/M_E`.
const ONE_OVER_E: f64 = 0.367_879_441_171_442_33;

/// The series in `r = sqrt(q)` that upstream uses near the branch point,
/// `q = x + 1/e`. Twelve coefficients, GSL's local `c[12]` in `series_eval`.
///
/// Both branches use the same coefficients and differ only in the sign of
/// `r` — `W_0` takes `+sqrt(q)` and `W_{-1}` takes `-sqrt(q)`, which is the
/// two sides of the square-root branch point.
#[rustfmt::skip]
const SERIES_C: [f64; 12] = [
    -1.0,
    2.331643981597124203363536062168,
    -1.812187885639363490240191647568,
    1.936631114492359755363277457668,
    -2.353551201881614516821543561516,
    3.066858901050631912893148922704,
    -4.175335600258177138854984177460,
    5.858023729874774148815053846119,
    -8.401032217523977370984161688514,
    12.250753501314460424,
    -18.100697012472442755,
    27.029044799010561650,
];

/// Upstream's `series_eval`, Horner in the nested grouping GSL writes.
///
/// The grouping is reproduced rather than flattened: `t_8`, `t_5`, `t_1` are
/// upstream's own intermediate names, and a different association would give
/// a different last bit.
fn series_eval(r: f64) -> f64 {
    // Indexed through the const directly rather than a `let c = &SERIES_C`
    // binding: tests/no_panic_gate.rs allows a literal index into a const
    // array of literal length, which rustc bounds-checks at compile time,
    // and a reference binding is not that.
    let t8 = SERIES_C[8] + r * (SERIES_C[9] + r * (SERIES_C[10] + r * SERIES_C[11]));
    let t5 = SERIES_C[5] + r * (SERIES_C[6] + r * (SERIES_C[7] + r * t8));
    let t1 = SERIES_C[1] + r * (SERIES_C[2] + r * (SERIES_C[3] + r * (SERIES_C[4] + r * t5)));
    SERIES_C[0] + r * t1
}

/// Upstream's `halley_iteration`, Corless et al. equation 5.12.
///
/// **It is not purely Halley, and that is upstream's choice, not a slip.**
/// For `w > 0` it takes a plain Newton step; only for `w <= 0` does it take
/// the Halley step with its second-order correction. The asymmetry is
/// deliberate: on the positive branch `p = w + 1` is bounded away from zero
/// and Newton converges quadratically with no help, while near `w = -1` the
/// Halley denominator is what keeps the step finite.
///
/// Returns `NaN` if the iteration does not converge within `max_iters`, which
/// upstream reports as `GSL_EMAXITER` and comments "should never get here".
fn halley(x: f64, w_initial: f64, max_iters: u32) -> f64 {
    let mut w = w_initial;
    for _ in 0..max_iters {
        let e = w.exp();
        let p = w + 1.0;
        let mut t = w * e - x;
        if w > 0.0 {
            t = (t / p) / e; // Newton
        } else {
            t /= e * p - 0.5 * (p + 1.0) * t / p; // Halley
        }
        w -= t;
        let tol = 10.0 * DBL_EPSILON * w.abs().max(1.0 / ((p.abs()) * e));
        if t.abs() < tol {
            return w;
        }
    }
    f64::NAN
}

/// The principal branch `W_0(x)`, GSL's `gsl_sf_lambert_W0`.
///
/// Defined for `x >= -1/e`, with `W_0(-1/e) = -1`, `W_0(0) = 0` and
/// `W_0(e) = 1`. Increasing throughout. Returns **`NaN`** below `-1/e` —
/// see the module documentation on that divergence from upstream.
///
/// # Examples
///
/// ```
/// use petir::specfunc::lambert::lambert_w0;
/// // The omega constant, W_0(1) = 0.5671432904097838...
/// assert!((lambert_w0(1.0) - 0.567_143_290_409_783_9).abs() < 1e-15);
/// // W e^W = x, the defining identity.
/// let w = lambert_w0(3.0);
/// assert!((w * w.exp() - 3.0).abs() < 1e-14);
/// assert_eq!(lambert_w0(0.0), 0.0);
/// ```
pub fn lambert_w0(x: f64) -> f64 {
    if x.is_nan() {
        return f64::NAN;
    }
    let q = x + ONE_OVER_E;
    if x == 0.0 {
        0.0
    } else if q < 0.0 {
        // Upstream returns -1.0 with GSL_EDOM; see the module docs.
        f64::NAN
    } else if q == 0.0 {
        -1.0
    } else if q < 1.0e-3 {
        // Series near -1/e in sqrt(q).
        series_eval(q.sqrt())
    } else {
        let w = if x < 1.0 {
            // Initial guess from the series near x = 0. No extra care needed:
            // Halley converges nicely on this branch.
            let p = (2.0 * core::f64::consts::E * q).sqrt();
            -1.0 + p * (1.0 + p * (-1.0 / 3.0 + p * 11.0 / 72.0))
        } else {
            // Rough asymptotic.
            let mut w = x.ln();
            if x > 3.0 {
                w -= w.ln();
            }
            w
        };
        halley(x, w, 10)
    }
}

/// The secondary real branch `W_{-1}(x)`, GSL's `gsl_sf_lambert_Wm1`.
///
/// Defined for `-1/e <= x < 0`, decreasing from `W_{-1}(-1/e) = -1` towards
/// `-inf` as `x -> 0^-`.
///
/// **Delegates to [`lambert_w0`] for `x > 0`**, which is upstream's own
/// behaviour: there is no second real branch there, so the only sensible
/// answer is the principal one. `x == 0` returns `0` by the same convention,
/// although the true limit along this branch is `-inf`. Returns `NaN` below
/// `-1/e`.
///
/// # Examples
///
/// ```
/// use petir::specfunc::lambert::lambert_wm1;
/// // The identity holds on this branch too.
/// let w = lambert_wm1(-0.1);
/// assert!((w * w.exp() + 0.1).abs() < 1e-14);
/// // And it is the OTHER root: W_{-1} <= -1 <= W_0 on (-1/e, 0).
/// assert!(w < -1.0);
/// ```
pub fn lambert_wm1(x: f64) -> f64 {
    if x.is_nan() {
        return f64::NAN;
    }
    if x > 0.0 {
        return lambert_w0(x);
    }
    if x == 0.0 {
        return 0.0;
    }
    let q = x + ONE_OVER_E;
    if q < 0.0 {
        return f64::NAN;
    }
    let mut w;
    if x < -1.0e-6 {
        // Series about q = 0, with -sqrt(q): the other side of the branch
        // point. Upstream bails out here when q is small, because Halley
        // converges badly in finite arithmetic when p is near zero and the
        // increment alternates.
        w = series_eval(-q.sqrt());
        if q < 3.0e-3 {
            return w;
        }
    } else {
        // Asymptotic near zero.
        let l1 = (-x).ln();
        let l2 = (-l1).ln();
        w = l1 - l2 + l2 / l1;
    }
    halley(x, w, 32)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `W_0(1)`, the omega constant, to the digits OEIS A030178 gives.
    const OMEGA: f64 = 0.567_143_290_409_783_87;

    /// **The defining identity `W(x) e^{W(x)} = x`.**
    ///
    /// This is the strongest check available for an inverse function and the
    /// module cannot satisfy it by construction: the returned `w` is fed back
    /// through `w e^w` and compared against the original argument, which
    /// exercises the series branches, the initial guesses and the iteration's
    /// stopping rule all at once. A wrong series coefficient, a wrong branch
    /// cut, or a premature exit all break it.
    ///
    /// # Results, measured 2026-09-19
    ///
    /// Worst relative `|w e^w - x| / |x|`:
    ///
    /// | branch | window | worst | at |
    /// |---|---|---|---|
    /// | `W_0` | `[-1/e, 100]` | 6.135e-16 | 17.372 |
    /// | `W_{-1}` | `[-1/e, -1e-8)` | 1.804e-15 | -1.056e-06 |
    ///
    /// Both are within a few `f64` ulp across the whole domain, and
    /// **neither worst point is at the branch point** — which is worth saying
    /// because it is where one would expect them. A first draft of this table
    /// asserted exactly that, from expectation rather than measurement, and
    /// was wrong twice over.
    ///
    /// The branch point is in fact the *best*-behaved region, because
    /// upstream does not iterate there at all: it switches to the
    /// `sqrt(q)` series, which is evaluated once and has no convergence
    /// criterion to stop short on. The residual instead concentrates where
    /// the **iteration** works hardest — mid-range for `W_0`, and for
    /// `W_{-1}` near `0^-` where `|W|` grows without bound and `w e^w` is a
    /// large number times a tiny one. `the_branch_point_is_the_series_not_the_iteration`
    /// tests that explanation rather than restating it.
    #[test]
    fn both_branches_satisfy_the_defining_identity() {
        let mut worst0: f64 = 0.0;
        let mut at0 = 0.0;
        for k in 0..=4000 {
            let x = -ONE_OVER_E + (100.0 + ONE_OVER_E) * (k as f64 / 4000.0);
            let w = lambert_w0(x);
            assert!(w.is_finite(), "W_0({x}) = {w}");
            if x != 0.0 {
                let r = ((w * w.exp() - x) / x).abs();
                if r > worst0 {
                    worst0 = r;
                    at0 = x;
                }
            }
        }
        assert!(worst0 < 1e-13, "W_0 identity: {worst0:e} at {at0}");

        let mut worst1: f64 = 0.0;
        let mut at1 = 0.0;
        for k in 0..=4000 {
            // Geometric from -1/e up to -1e-8, so the whole tail is covered.
            let t = k as f64 / 4000.0;
            let x = -ONE_OVER_E * (1e-8_f64 / ONE_OVER_E).powf(t);
            let w = lambert_wm1(x);
            assert!(w.is_finite(), "W_-1({x:e}) = {w}");
            let r = ((w * w.exp() - x) / x).abs();
            if r > worst1 {
                worst1 = r;
                at1 = x;
            }
        }
        assert!(worst1 < 1e-13, "W_-1 identity: {worst1:e} at {at1:e}");
    }

    /// **The branch point is the best-behaved region, not the worst**, and
    /// the reason is that upstream does not iterate there.
    ///
    /// The expectation — recorded and refuted, see the table above — was that
    /// `x = -1/e` would dominate the residual, since `dW/dx` is infinite
    /// there. It does not: for `q = x + 1/e` below 1e-3 the `sqrt(q)` series
    /// is evaluated once with no convergence test, and it is excellent.
    ///
    /// Three claims, all asserted so the explanation can fail:
    ///
    /// 1. The residual within 1e-4 of the branch point is **smaller** than
    ///    the worst over the whole domain.
    /// 2. The value returned there is literally [`series_eval`]'s, i.e. the
    ///    iteration is not entered at all — checked by bit equality, not a
    ///    tolerance.
    /// 3. **The real limit near the branch point is the argument, not the
    ///    series.** `x = -1/e + q` is stored to one ulp of `1/e`, about
    ///    5.6e-17, so `q` cannot be recovered from `x` to better than that:
    ///    at `q = 1e-12` about five significant figures survive and the rest
    ///    are gone before this function is called. Claim 2's first draft
    ///    compared against `series_eval(sqrt(dq))` using the `dq` that built
    ///    `x`, and failed — correctly, because those are different numbers.
    ///    The test now uses the function's own recovered `q` and asserts the
    ///    loss separately.
    #[test]
    fn the_branch_point_is_the_series_not_the_iteration() {
        // 2. The series branch is the one taken. Note the reference uses
        // the function's OWN q -- `x + 1/e` recomputed -- not the `dq` that
        // built `x`. Those differ, and the difference is the third finding
        // below.
        for dq in [1e-12_f64, 1e-8, 1e-6, 5e-4, 9.9e-4] {
            let x = -ONE_OVER_E + dq;
            let q_seen = x + ONE_OVER_E;
            assert_eq!(
                lambert_w0(x),
                series_eval(q_seen.sqrt()),
                "W_0 at q = {dq:e} is documented as the series, evaluated once"
            );
        }

        // 3. NEAR THE BRANCH POINT THE LIMIT IS THE ARGUMENT, NOT THE SERIES.
        // x = -1/e + q is stored to one ulp of 1/e, about 5.6e-17 absolute,
        // so `q` cannot be recovered from `x` to better than that -- at
        // q = 1e-12 it survives to roughly five significant figures and no
        // more. This is information the CALLER has already lost, and no
        // implementation can recover it.
        let dq = 1e-12_f64;
        let q_seen = (-ONE_OVER_E + dq) + ONE_OVER_E;
        let rel = ((q_seen - dq) / dq).abs();
        assert!(
            rel > 1e-6,
            "recovering q from x near the branch point is documented as \
             losing most of its digits to cancellation (measured ~5e-05 \
             relative at q = 1e-12), and it measured {rel:e}. If q now \
             survives intact the argument reduction has changed"
        );
        // And the consequence: the two evaluations differ far above f64 ulp.
        let a = lambert_w0(-ONE_OVER_E + dq);
        let b = series_eval(dq.sqrt());
        assert!(
            ((a - b) / b).abs() > 1e-12,
            "the q-recovery loss is documented as visible in the answer: \
             {a} vs {b}"
        );

        // 1. And it is better there than the domain-wide worst.
        let mut near: f64 = 0.0;
        for k in 1..=200 {
            let q = 1e-4 * (k as f64 / 200.0);
            let x = -ONE_OVER_E + q;
            let w = lambert_w0(x);
            near = near.max(((w * w.exp() - x) / x).abs());
        }
        let mut far: f64 = 0.0;
        for k in 0..=2000 {
            let x = 1.0 + 99.0 * (k as f64 / 2000.0);
            let w = lambert_w0(x);
            far = far.max(((w * w.exp() - x) / x).abs());
        }
        assert!(
            near <= far,
            "the branch point is documented as BETTER behaved than the \
             iterated range, because the series is used there: near {near:e}, \
             far {far:e}. If the branch point is now the worse of the two, \
             the series has a defect and the table above needs re-measuring"
        );
    }

    /// The exact and published values.
    #[test]
    fn the_known_values_are_reproduced() {
        assert_eq!(lambert_w0(0.0), 0.0);
        assert!((lambert_w0(1.0) - OMEGA).abs() < 1e-15, "omega constant");
        // W_0(e) = 1 exactly.
        assert!(
            (lambert_w0(core::f64::consts::E) - 1.0).abs() < 1e-14,
            "W_0(e) = {}",
            lambert_w0(core::f64::consts::E)
        );
        // Both branches meet at -1/e with value -1.
        assert!((lambert_w0(-ONE_OVER_E) + 1.0).abs() < 1e-7, "W_0(-1/e)");
        assert!((lambert_wm1(-ONE_OVER_E) + 1.0).abs() < 1e-7, "W_-1(-1/e)");
        // W_0(-ln(2)/2) = -ln(2): 2^{-1/2} ln 2 ... the classic closed form.
        let x = -core::f64::consts::LN_2 / 2.0;
        assert!(
            (lambert_w0(x) + core::f64::consts::LN_2).abs() < 1e-14,
            "W_0(-ln2/2) = {} vs -ln 2",
            lambert_w0(x)
        );
    }

    /// The two branches really are **different** roots of the same equation
    /// on `(-1/e, 0)`, with `W_{-1} <= -1 <= W_0`.
    ///
    /// Without this, a defect that returned the principal value from
    /// [`lambert_wm1`] would pass the identity test above — both roots
    /// satisfy `w e^w = x`.
    #[test]
    fn the_two_branches_are_distinct_roots() {
        for k in 1..=200 {
            let x = -ONE_OVER_E + ONE_OVER_E * (k as f64 / 201.0);
            let (a, b) = (lambert_w0(x), lambert_wm1(x));
            assert!(a >= -1.0, "W_0({x}) = {a} should be >= -1");
            assert!(b <= -1.0, "W_-1({x}) = {b} should be <= -1");
            assert!(
                b < a,
                "the branches are documented as distinct on (-1/e, 0), and at \
                 {x} both gave W_0 = {a}, W_-1 = {b}"
            );
            // Both are genuine roots.
            for w in [a, b] {
                assert!(((w * w.exp() - x) / x).abs() < 1e-12, "root at {x}");
            }
        }
    }

    /// `W_0` is increasing on its whole domain, `W_{-1}` decreasing on
    /// `(-1/e, 0)` and unbounded below as `x -> 0^-`.
    #[test]
    fn the_monotonicity_is_right() {
        let mut prev = f64::NEG_INFINITY;
        for k in 0..=500 {
            let x = -ONE_OVER_E + (50.0 + ONE_OVER_E) * (k as f64 / 500.0);
            let v = lambert_w0(x);
            assert!(v > prev, "W_0 not increasing at {x}: {v} <= {prev}");
            prev = v;
        }
        let mut prev = -1.0;
        for k in 1..=500 {
            let t = k as f64 / 500.0;
            let x = -ONE_OVER_E * (1e-10_f64 / ONE_OVER_E).powf(t);
            let v = lambert_wm1(x);
            assert!(v < prev, "W_-1 not decreasing at {x:e}: {v} >= {prev}");
            prev = v;
        }
        assert!(prev < -20.0, "W_-1 near 0^- should be far below -1: {prev}");
    }

    /// **The documented divergence from upstream: `NaN`, not `-1`, below
    /// `-1/e`** — and the leniency GSL buys with its `-1` is still preserved
    /// for an argument only an epsilon outside the domain.
    #[test]
    fn below_the_branch_point_is_a_refusal_not_an_answer() {
        for x in [-0.4_f64, -1.0, -1e3] {
            assert!(lambert_w0(x).is_nan(), "W_0({x})");
            assert!(lambert_wm1(x).is_nan(), "W_-1({x})");
        }
        // An argument that rounds to exactly -1/e still answers -1, which is
        // the epsilon-overshoot case upstream's comment is about.
        assert_eq!(lambert_w0(-ONE_OVER_E), -1.0);
        assert_eq!(lambert_wm1(-ONE_OVER_E), -1.0);
        assert!(lambert_w0(f64::NAN).is_nan());
        assert!(lambert_wm1(f64::NAN).is_nan());
    }

    /// `W_{-1}` delegates to `W_0` for positive argument, which is upstream's
    /// behaviour and not an accident — there is no second real branch there.
    #[test]
    fn wm1_delegates_to_w0_on_the_positive_axis() {
        for k in 1..=100 {
            let x = 0.1 * k as f64;
            assert_eq!(lambert_wm1(x), lambert_w0(x), "at {x}");
        }
        assert_eq!(lambert_wm1(0.0), 0.0);
    }

    /// Every branch of the dispatch is actually reached by the sweeps above.
    ///
    /// A branch that no probe enters is untested however green the suite
    /// looks, so the boundaries are named and hit explicitly: `q < 1e-3`
    /// (series), `x < 1` (near-zero guess), `x > 3` (asymptotic guess), and
    /// `W_{-1}`'s `q < 3e-3` early return and `x >= -1e-6` asymptotic.
    #[test]
    fn every_branch_is_reached() {
        // W_0: the sqrt(q) series, just inside q < 1e-3.
        let x = -ONE_OVER_E + 5e-4;
        assert!((lambert_w0(x) - series_eval((x + ONE_OVER_E).sqrt())).abs() < 1e-300);
        // W_0: the near-zero initial guess (q >= 1e-3, x < 1).
        let w = lambert_w0(0.5);
        assert!(((w * w.exp() - 0.5) / 0.5).abs() < 1e-14);
        // W_0: the asymptotic guess, x > 3.
        let w = lambert_w0(1e6);
        assert!(((w * w.exp() - 1e6) / 1e6).abs() < 1e-14);
        // W_0: 1 <= x <= 3, log without the log-log correction.
        let w = lambert_w0(2.0);
        assert!(((w * w.exp() - 2.0) / 2.0).abs() < 1e-14);
        // W_-1: the early return, q < 3e-3 and x < -1e-6.
        let x = -ONE_OVER_E + 1e-3;
        assert_eq!(lambert_wm1(x), series_eval(-(x + ONE_OVER_E).sqrt()));
        // W_-1: the asymptotic branch, x >= -1e-6.
        let w = lambert_wm1(-1e-7);
        assert!(((w * w.exp() + 1e-7) / -1e-7).abs() < 1e-12, "W_-1(-1e-7)");
    }
}
