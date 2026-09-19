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
// PORTED from the GNU Scientific Library (GSL) 2.8, specfunc/trig.c
// <https://www.gnu.org/software/gsl/>
// Copyright (C) 1996-2000 Gerard Jungman
// GSL is free software under the GNU General Public License, version 3 or
// later.
//
// Only the two ANGLE REDUCTION entry points are ported -- see "What is
// deliberately absent".

//! Extended-precision reduction of an angle to one period.
//!
//! # What this is
//!
//! Reducing `theta` modulo `2 pi` is the step that decides how accurate
//! `sin` and `cos` are at a large argument, and doing it with a single
//! `f64` value of `2 pi` throws away digits: `2 pi` is irrational, its `f64`
//! representation is wrong by about 2.4e-16, and that error is multiplied by
//! however many periods are subtracted.
//!
//! Upstream's answer, reproduced here, is to split `2 pi` into **three**
//! `f64` pieces `P1 + P2 + P3` whose sum carries far more than 53 bits, and
//! to subtract them one at a time so each product `y * Pk` is exact or nearly
//! so. At `theta = 1e6` that is the difference between roughly nine correct
//! digits and fifteen.
//!
//! # Argument range, stated plainly
//!
//! All arguments and results are angles in **radians** (`f64`). Both
//! functions return `NaN` for `|theta| > 0.0625 / DBL_EPSILON`, about
//! 2.815e+14, which is upstream's `GSL_ELOSS`: past that point `theta`'s own
//! `f64` spacing exceeds a useful fraction of a period, so the reduced angle
//! would be meaningless rather than merely inaccurate. **That is a refusal,
//! not an overflow** — the answer does not exist, rather than being too large
//! to hold.
//!
//! `NaN` propagates.
//!
//! # What is deliberately absent
//!
//! `trig.c`'s other entry points — the complex sine and cosine, `sinc`,
//! `lnsinh`, `lncosh`, and the `*_err` forms that propagate an input
//! uncertainty — are **not** ported. This crate has no complex type, and the
//! error-propagating forms need the `gsl_sf_result` error channel that PETIR
//! deliberately does not carry. The two reductions are here because
//! [`crate::specfunc::clausen`] needs one and because they are useful on
//! their own.

// Under a std-linked build (`cargo test`) f64's inherent floor/abs shadow
// these trait methods, leaving the import formally unused. See crate::real.
#[allow(unused_imports)]
use crate::real::Real;

use crate::specfunc::DBL_EPSILON;

/// The three-way split of `pi/2`, scaled by 4 to give `2 pi = 2 (P1+P2+P3)`.
///
/// Each is exactly representable in `f64`, so `y * Pk` is exact whenever `y`
/// is a modest integer — which is the whole point of the split. Upstream
/// writes these inline in both functions; they are identical apart from
/// trailing digits that round to the same `f64`, which
/// `the_two_reductions_use_the_same_split` checks rather than assumes.
const P1: f64 = 4.0 * 7.853_981_256_484_985_351_562_5e-1;
const P2: f64 = 4.0 * 3.774_894_707_930_798_176_676_0e-8;
const P3: f64 = 4.0 * 2.695_151_429_079_059_484_055_2e-15;
/// `2 pi` as the sum of the three pieces, which is what the reductions
/// actually subtract. Deliberately **not** `core::f64::consts::TAU`.
const TWO_PI: f64 = 2.0 * (P1 + P2 + P3);

/// The largest `|theta|` for which a reduction is meaningful, upstream's
/// `0.0625 / GSL_DBL_EPSILON`. About 2.815e+14.
const LOSS_CUT: f64 = 0.0625 / DBL_EPSILON;

/// Reduce `theta` to `[0, 2 pi)`, GSL's `gsl_sf_angle_restrict_pos`.
///
/// Returns `NaN` for `|theta| > 0.0625 / DBL_EPSILON` — see the module
/// documentation on why that is a refusal rather than a failure.
///
/// # Examples
///
/// ```
/// use petir::specfunc::trig::angle_restrict_pos;
/// use core::f64::consts::PI;
/// let r = angle_restrict_pos(-PI / 2.0);
/// assert!((r - 3.0 * PI / 2.0).abs() < 1e-15);
/// // Far outside one period, and still accurate.
/// assert!(angle_restrict_pos(1.0e6).is_finite());
/// // Past the loss cut it refuses rather than guessing.
/// assert!(angle_restrict_pos(1.0e15).is_nan());
/// ```
pub fn angle_restrict_pos(theta: f64) -> f64 {
    if theta.is_nan() || theta.abs() > LOSS_CUT {
        return f64::NAN;
    }
    let y = 2.0 * (theta / TWO_PI).floor();
    let mut r = ((theta - y * P1) - y * P2) - y * P3;
    if r > TWO_PI {
        r = ((r - 2.0 * P1) - 2.0 * P2) - 2.0 * P3;
    } else if r < 0.0 {
        // Can happen through floating-point rounding; upstream says so.
        r = ((r + 2.0 * P1) + 2.0 * P2) + 2.0 * P3;
    }
    r
}

/// Reduce `theta` to `(-pi, pi]`, GSL's `gsl_sf_angle_restrict_symm`.
///
/// Returns `NaN` past the same loss cut as [`angle_restrict_pos`].
///
/// # Examples
///
/// ```
/// use petir::specfunc::trig::angle_restrict_symm;
/// use core::f64::consts::PI;
/// let r = angle_restrict_symm(3.0 * PI);
/// assert!((r.abs() - PI).abs() < 1e-14);
/// assert!(angle_restrict_symm(0.5).abs() < 0.5 + 1e-15);
/// ```
pub fn angle_restrict_symm(theta: f64) -> f64 {
    if theta.is_nan() || theta.abs() > LOSS_CUT {
        return f64::NAN;
    }
    // GSL_SIGN is +1 for zero, which matters: floor(|theta|/2pi) is then 0
    // and the whole correction vanishes either way.
    let sign = if theta < 0.0 { -1.0 } else { 1.0 };
    let y = sign * 2.0 * (theta.abs() / TWO_PI).floor();
    let mut r = ((theta - y * P1) - y * P2) - y * P3;
    let pi = core::f64::consts::PI;
    if r > pi {
        r = ((r - 2.0 * P1) - 2.0 * P2) - 2.0 * P3;
    } else if r < -pi {
        r = ((r + 2.0 * P1) + 2.0 * P2) + 2.0 * P3;
    }
    r
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::f64::consts::PI;

    /// Both reductions land in their stated interval, for arguments spanning
    /// fourteen decades either side of zero.
    #[test]
    fn the_results_are_in_the_stated_intervals() {
        // A fixed sweep rather than a Vec: this crate is no_std.
        let mut probes = [0.0_f64; 140];
        let mut i = 0;
        for e in 0..14i32 {
            for m in [1.0_f64, 1.7, 3.3, 6.28318, 9.4] {
                probes[i] = m * 10f64.powi(e);
                probes[i + 1] = -m * 10f64.powi(e);
                i += 2;
            }
        }
        for &t in probes.iter() {
            let p = angle_restrict_pos(t);
            assert!(
                (0.0..TWO_PI).contains(&p),
                "angle_restrict_pos({t:e}) = {p} is outside [0, 2pi)"
            );
            let s = angle_restrict_symm(t);
            assert!(
                s > -PI - 1e-12 && s <= PI + 1e-12,
                "angle_restrict_symm({t:e}) = {s} is outside (-pi, pi]"
            );
        }
    }

    /// **The reduction is congruent to the original modulo `2 pi`**, checked
    /// through `sin` and `cos` rather than by re-deriving the arithmetic.
    ///
    /// This is the property that matters and the one a split-constant bug
    /// breaks: `sin(reduce(theta))` must equal `sin(theta)`. It is not a
    /// tautology, because the two are computed by completely different
    /// routes — one reduces then calls `libm`, the other hands `libm` the
    /// raw argument to reduce itself.
    #[test]
    fn the_reduction_is_congruent_modulo_two_pi() {
        let mut worst_pos: f64 = 0.0;
        let mut worst_symm: f64 = 0.0;
        for k in 1..=4000 {
            let t = k as f64 * 137.035_999_08; // nothing commensurate with 2pi
            worst_pos = worst_pos.max((angle_restrict_pos(t).sin() - t.sin()).abs());
            worst_symm = worst_symm.max((angle_restrict_symm(t).cos() - t.cos()).abs());
        }
        assert!(worst_pos < 1e-9, "pos congruence: {worst_pos:e}");
        assert!(worst_symm < 1e-9, "symm congruence: {worst_symm:e}");
    }

    /// **The three-way split is what buys the accuracy — and it stops
    /// buying it at about `theta = 1e8`.**
    ///
    /// `theta - 2 pi floor(theta / 2 pi)` with a single `f64` `2 pi` is the
    /// obvious implementation, and it is wrong by roughly `n * 2.4e-16` after
    /// `n` periods. Worst `|sin(reduce(theta)) - sin(theta)|` over 500 probes
    /// per decade, measured 2026-09-19:
    ///
    /// | `theta` | split | naive | ratio |
    /// |---|---|---|---|
    /// | 1e1 | 4.441e-16 | 1.263e-13 | 285 |
    /// | 1e3 | 4.302e-16 | 1.816e-11 | 4.2e+04 |
    /// | 1e5 | 4.302e-16 | 1.407e-09 | 3.3e+06 |
    /// | 1e7 | 4.337e-16 | 1.600e-07 | **3.7e+08** |
    /// | 1e8 | 9.536e-07 | 1.407e-06 | 1.5 |
    /// | 1e10 | 1.194e-04 | 1.547e-04 | 1.3 |
    /// | 1e13 | 1.504e-02 | 2.483e-02 | 1.7 |
    ///
    /// Two regimes, and **both** are asserted below because a first draft of
    /// this test claimed only the first and probed only the second, so it
    /// failed:
    ///
    /// 1. Up to about `1e7` the split is pinned at one `f64` ulp — it does
    ///    not degrade at all — while the naive form degrades linearly, so the
    ///    advantage grows to eight orders of magnitude.
    /// 2. From about `1e8` the advantage collapses to a factor of 1.5,
    ///    because the split's premise fails: each `y * Pk` must be **exact**,
    ///    and `y` there carries about 25 bits against `P1`'s 30, so the
    ///    product needs 55 and `f64` has 53.
    ///
    /// GSL's own "reduced accuracy" marker, `0.0625 / sqrt(DBL_EPSILON)`,
    /// is 4.2e+06 — just below where that happens, which corroborates the
    /// explanation from upstream's side. **Do not reduce an angle above
    /// ~1e7 and expect it to be worth anything**; the `NaN` cut at 2.8e+14
    /// is far beyond the point where the answer stops meaning much, and is a
    /// last-resort guard rather than the usable range.
    #[test]
    fn the_three_way_split_beats_the_naive_reduction_until_1e8() {
        let naive = |t: f64| t - core::f64::consts::TAU * (t / core::f64::consts::TAU).floor();
        let per_decade = |e: i32| {
            let scale = 10f64.powi(e);
            let (mut ws, mut wn) = (0.0f64, 0.0f64);
            for k in 1..=500 {
                let t = k as f64 * scale / 500.0 * 137.035_999_08;
                if t.abs() > LOSS_CUT {
                    continue;
                }
                ws = ws.max((angle_restrict_pos(t).sin() - t.sin()).abs());
                wn = wn.max((naive(t).sin() - t.sin()).abs());
            }
            (ws, wn)
        };

        // 1. Below 1e8 the split does not degrade, and the gap grows.
        for e in 1..=7i32 {
            let (ws, wn) = per_decade(e);
            assert!(
                ws < 1e-14,
                "the split is documented as holding at one f64 ulp up to                  1e7, and \
                 at 1e{e} it measured {ws:e}"
            );
            assert!(
                wn > 100.0 * ws,
                "the naive reduction is documented as far worse below 1e8: at 1e{e}, split {ws:e}, \
                 naive {wn:e}"
            );
        }
        // And the gap really does grow with the magnitude.
        let (s1, n1) = per_decade(1);
        let (s7, n7) = per_decade(7);
        assert!(
            (n7 / s7) > 1000.0 * (n1 / s1),
            "the advantage is documented as growing with theta: {:.1e} at              1e1 against \
             {:.1e} at 1e7",
            n1 / s1,
            n7 / s7
        );

        // 2. From 1e8 the split's own premise fails and the advantage goes.
        for e in 8..=13i32 {
            let (ws, wn) = per_decade(e);
            assert!(
                ws > 1e-8,
                "the split is documented as LOSING its exactness above 1e8, because y * P1 no \
                 longer fits in 53 bits. At 1e{e} it measured {ws:e}, which would mean it still \
                 holds and this explanation is wrong"
            );
            assert!(
                wn < 10.0 * ws,
                "above 1e8 the two are documented as comparable (ratio ~1.5): at 1e{e}, split \
                 {ws:e}, naive {wn:e}"
            );
        }

        // GSL's own reduced-accuracy marker sits just below the collapse.
        let gsl_marker = 0.0625 / crate::specfunc::SQRT_DBL_EPSILON;
        assert!(
            (4.0e6..5.0e6).contains(&gsl_marker),
            "GSL's 0.0625/sqrt(eps) marker is documented at 4.2e+06, and is              \
             {gsl_marker:e}"
        );
    }

    /// Past the loss cut both refuse, and just inside it both answer.
    ///
    /// The cut is about 2.815e+14, where `theta`'s own `f64` spacing (0.0625)
    /// is already a measurable fraction of a radian.
    #[test]
    fn past_the_loss_cut_both_refuse() {
        assert!(
            (2.8e14..2.9e14).contains(&LOSS_CUT),
            "cut moved: {LOSS_CUT:e}"
        );
        for t in [LOSS_CUT * 1.001, 1e15, -1e15, f64::INFINITY] {
            assert!(angle_restrict_pos(t).is_nan(), "pos({t:e})");
            assert!(angle_restrict_symm(t).is_nan(), "symm({t:e})");
        }
        for t in [LOSS_CUT * 0.999, -LOSS_CUT * 0.999] {
            assert!(angle_restrict_pos(t).is_finite(), "pos({t:e})");
            assert!(angle_restrict_symm(t).is_finite(), "symm({t:e})");
        }
        assert!(angle_restrict_pos(f64::NAN).is_nan());
        assert!(angle_restrict_symm(f64::NAN).is_nan());
        // The spacing claim, stated so it can fail.
        let spacing = (LOSS_CUT + 1.0) - LOSS_CUT;
        assert!(
            spacing >= 0.0625,
            "the loss cut is documented as the point where theta's own f64 \
             spacing reaches 0.0625 rad, and it measured {spacing}"
        );
    }

    /// The two functions agree where their intervals overlap, and the split
    /// constants are the same `f64` in both despite upstream writing them
    /// with different numbers of digits.
    #[test]
    fn the_two_reductions_use_the_same_split() {
        // Upstream's two spellings of P1, as written in trig.c.
        assert_eq!(4.0 * 7.853_981_256_484_985_351_56e-1, P1);
        assert_eq!(4.0 * 7.853_981_256_484_985_351_562_5e-1, P1);
        // TWO_PI is close to, but NOT, the library constant -- that is the
        // point of the split.
        assert!((TWO_PI - core::f64::consts::TAU).abs() < 1e-15);

        for k in -500..=500i32 {
            let t = k as f64 * 0.37;
            let (p, s) = (angle_restrict_pos(t), angle_restrict_symm(t));
            let d = (p - s).abs();
            assert!(
                d < 1e-12 || (d - TWO_PI).abs() < 1e-12,
                "at {t}: pos {p}, symm {s} differ by {d}, neither 0 nor 2pi"
            );
        }
    }
}
