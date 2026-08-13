// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// PORTED CODE — the one file in this crate that carries upstream source.
// Ported from Blender blenlib `BLI_math_boolean.hh` / `intern/math_boolean.cc`,
// github.com/blender/blender @ 96294be75080bbf687fa7f108e344a1063713586.
//     SPDX-FileCopyrightText: 2023 Blender Authors
//     SPDX-License-Identifier: GPL-2.0-or-later
// GPL-2.0-or-later is GPL-3.0-compatible, so the port ships as GPL-3.0-only here.
// Underlying method: J. R. Shewchuk, "Adaptive Precision Floating-Point Arithmetic
// and Fast Robust Geometric Predicates", Discrete & Computational Geometry 18(3),
// 1997, pp. 305-363.
// See the module documentation below for the full provenance statement — do not
// remove or weaken it.
//
// This file is part of OUTRAM PARK.
//
// OUTRAM PARK is free software: you can redistribute it and/or modify it
// under the terms of the GNU General Public License as published by the
// Free Software Foundation, either version 3 of the License, or (at your
// option) any later version.
//
// OUTRAM PARK is distributed in the hope that it will be useful, but
// WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the GNU
// General Public License for more details.
//
// You should have received a copy of the GNU General Public License along
// with OUTRAM PARK.  If not, see <https://www.gnu.org/licenses/>.

//! Robust geometric predicates — orientation and in-circle/in-sphere tests.
//!
//! Blender analogue / provenance: ported from `blender/blenlib`
//! `BLI_math_boolean.hh` / `intern/math_boolean.cc`, upstream repo
//! `github.com/blender/blender`, commit
//! `96294be75080bbf687fa7f108e344a1063713586`.
//!
//! ```text
//! SPDX-FileCopyrightText: 2023 Blender Authors
//! SPDX-License-Identifier: GPL-2.0-or-later
//! ```
//!
//! Adapted to pure Rust for `outram-blender` (GPL-3.0-only); GPL-2.0-or-later
//! is GPL-3.0-compatible per the workspace provenance rule. Only the `double`
//! (floating-point) predicate API is ported here — Blender's `mpq_class`
//! (GMP rational) overloads are intentionally **not** ported: this crate must
//! stay Android-buildable with **no C dependencies**, and GMP is a C library.
//!
//! Blender's own `double` predicates are, in turn, a C++ adaptation of
//! Jonathan Shewchuk's `predicates.c` — "Routines for Arbitrary Precision
//! Floating-point Arithmetic and Fast Robust Geometric Predicates", placed in
//! the **public domain** by Jonathan Richard Shewchuk (Carnegie Mellon
//! University, May 1996). The error-bound coefficients used below
//! (`CCW_ERR_BOUND_A`, `O3D_ERR_BOUND_A`, `ICC_ERR_BOUND_A`) are taken
//! directly from Shewchuk's `exactinit()` derivation as reproduced in
//! Blender's `math_boolean.cc`.
//!
//! ## Robustness contract — what is actually implemented here
//!
//! This is **not** a line-for-line port of Shewchuk's multi-stage adaptive
//! expansion arithmetic (`orient2dadapt`/`orient3dadapt`/`incircleadapt`,
//! which build growing exact "expansions" out of `double[]` arrays and only
//! spend as much precision as each case needs). That algorithm is correct
//! and fast, but reproducing its ~150-1000 lines of index-juggling C macros
//! by hand for `orient3d`/`incircle` carries real risk of a transcription bug
//! that silently produces a *wrong* sign — worse than an honest, simpler
//! scheme. Instead, each predicate here uses a **two-stage filter**:
//!
//! 1. **Fast path** — compute the determinant in plain `f64` (the `_fast`
//!    variant's formula) together with Shewchuk's rigorous `permanent`-based
//!    error bound (`errbound = C * permanent`, using the *exact* coefficients
//!    `C` from his error analysis). If `|det| > errbound`, the `f64` result's
//!    **sign is provably correct** — this stage is the real Shewchuk
//!    algorithm, not a simplification.
//! 2. **Refine path** — if the fast filter is inconclusive (the near-
//!    degenerate case), recompute the *same* determinant formula using
//!    **double-double (`Dd`) arithmetic**: each `f64` is carried as an exact
//!    `(hi, lo)` pair via error-free transformations (Knuth's `two_sum`,
//!    FMA-based `two_prod`), giving roughly 106 bits of mantissa instead of
//!    53. The sign of the double-double result is returned.
//!
//! Stage 2 is the **simplified/partial** part of this port, relative to
//! Shewchuk/Blender's full adaptive-precision expansion arithmetic:
//! double-double arithmetic is *not* arbitrary precision. It resolves the
//! sign correctly for the vast majority of practical near-degenerate
//! configurations (anything not degenerate below roughly the 106th
//! significant bit), and the `tests` module below demonstrates a concrete
//! case where the `_fast` plain-`f64` path returns the wrong sign and the
//! double-double-refined path returns the correct one. But it is not a
//! mathematical guarantee of exactness for *arbitrarily* degenerate
//! adversarial inputs (e.g. points constructed via exact expansion
//! arithmetic to be non-zero only at the 107th+ bit) — a full Shewchuk/CGAL-
//! style expansion port would be needed for that. This limitation is
//! intentional and documented rather than hidden.
//!
//! [`insphere`] goes one step further in caution: deriving Shewchuk's precise
//! `isperrboundA` permanent formula correctly (it involves six 2x2
//! sub-determinants folded into four 3x3 cofactors) is easy to get subtly
//! wrong from a partial reading of the reference source, and a wrong
//! permanent bound could make the fast-path filter *unsoundly* trust an
//! incorrect `f64` sign. So [`insphere`] skips the fast-path filter entirely
//! and **always** evaluates via double-double arithmetic — slower, but never
//! unsoundly fast. This is called out again on the function itself.
//!
//! ## Sign convention
//!
//! All predicates return a plain `i32` in `{-1, 0, +1}` (matching Blender's
//! `double` predicate signatures exactly), rather than an enum, so callers
//! can use ordinary integer comparisons/arithmetic on the result the same way
//! Blender's own callers do.
//!
//! - [`orient2d`]/[`orient2d_fast`]: `+1` if `a, b, c` occur counter-clockwise,
//!   `-1` clockwise, `0` collinear.
//! - [`orient3d`]/[`orient3d_fast`]: `+1` if `d` lies *below* the plane through
//!   `a, b, c` (with `a, b, c` counter-clockwise when viewed from above that
//!   plane), `-1` above, `0` coplanar.
//! - [`incircle`]/[`incircle_fast`]: `+1` if `d` lies inside the circle
//!   through `a, b, c` (which must be given counter-clockwise, or the sign
//!   reverses), `-1` outside, `0` co-circular.
//! - [`insphere`]/[`insphere_fast`]: `+1` if `e` lies inside the sphere
//!   through `a, b, c, d` (which must be positively oriented per
//!   [`orient3d`], or the sign reverses), `-1` outside, `0` co-spherical.
//!
//! > **Untrusted AI-generated draft** until a human reviews it, per the
//! > workspace `RESPONSIBLE_USE.md`. Not for nuclear facility operation,
//! > reactor control, safety-critical, or licensing decisions.

use crate::math::Vec3;

/// A 2-component vector, `f64` `x`/`y`.
///
/// Local to this module rather than added to [`crate::math`] — the boolean
/// predicates are the only 2D consumer in the crate today (see the task that
/// created this file). Dimensionless model-space coordinates, exactly like
/// [`crate::math::Vec3`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Vec2 {
    /// X component.
    pub x: f64,
    /// Y component.
    pub y: f64,
}

impl Vec2 {
    /// Construct a vector from explicit components.
    pub const fn new(x: f64, y: f64) -> Self {
        Vec2 { x, y }
    }
}

// ============================================================================
// Error-free transformations (Knuth / Dekker) and double-double arithmetic.
//
// These are the primitive building blocks of Shewchuk's algorithm: a "two_sum"
// or "two_product" computes a `f64` result *and* the exact rounding error as a
// second `f64`, so the pair `(hi, lo)` represents `a op b` with no information
// lost. Stacking these gives extended-precision ("double-double", ~106-bit
// mantissa) arithmetic using only `f64` operations — no big-integer / rational
// library, no `unsafe`.
// ============================================================================

/// Machine epsilon for `f64`: the largest power of two such that
/// `1.0 + EPSILON` would round to something other than `1.0` — i.e. half a
/// unit in the last place of `1.0`. Matches Shewchuk's `exactinit()`
/// derivation (`epsilon = 2^-53` for IEEE-754 binary64), used only to derive
/// the static error-bound coefficients below.
const EPSILON: f64 = f64::EPSILON / 2.0;

/// Shewchuk's `ccwerrboundA`: static error bound coefficient for
/// [`orient2d`]'s fast-path filter (`errbound = CCW_ERR_BOUND_A * detsum`).
const CCW_ERR_BOUND_A: f64 = (3.0 + 16.0 * EPSILON) * EPSILON;

/// Shewchuk's `o3derrboundA`: static error bound coefficient for
/// [`orient3d`]'s fast-path filter.
const O3D_ERR_BOUND_A: f64 = (7.0 + 56.0 * EPSILON) * EPSILON;

/// Shewchuk's `iccerrboundA`: static error bound coefficient for
/// [`incircle`]'s fast-path filter.
const ICC_ERR_BOUND_A: f64 = (10.0 + 96.0 * EPSILON) * EPSILON;

/// Knuth's exact sum: returns `(s, e)` such that `s = fl(a + b)` (the ordinary
/// rounded `f64` sum) and `a + b == s + e` **exactly** (in infinite
/// precision) — `e` is the rounding error `two_sum` would otherwise discard.
/// Valid for any `a`, `b` (no magnitude ordering required), 6 flops.
#[inline]
fn two_sum(a: f64, b: f64) -> (f64, f64) {
    let s = a + b;
    let bb = s - a;
    let err = (a - (s - bb)) + (b - bb);
    (s, err)
}

/// Like [`two_sum`], but only valid when `|a| >= |b|` (3 flops instead of 6).
/// Used for the second step of double-double add/mul, where that ordering is
/// guaranteed by construction (the running "hi" accumulator dominates the
/// small correction term being folded in).
#[inline]
fn quick_two_sum(a: f64, b: f64) -> (f64, f64) {
    let s = a + b;
    let err = b - (s - a);
    (s, err)
}

/// Exact difference: `(s, e)` with `a - b == s + e` exactly. Implemented as
/// `two_sum(a, -b)`, which is exact because negation of a finite `f64` is
/// exact.
#[inline]
fn two_diff(a: f64, b: f64) -> (f64, f64) {
    two_sum(a, -b)
}

/// Exact product: `(p, e)` such that `p = fl(a * b)` and `a * b == p + e`
/// exactly. Shewchuk's original used `Split`/Dekker's algorithm to emulate
/// this without hardware FMA; this port instead uses `f64::mul_add`, which
/// Rust guarantees is a correctly-rounded fused multiply-add (using the
/// hardware instruction where available, a correctly-rounded software
/// fallback otherwise) — `a.mul_add(b, -p)` is exactly the rounding error of
/// `a * b` with no `Split`/`splitter` machinery needed.
#[inline]
fn two_prod(a: f64, b: f64) -> (f64, f64) {
    let p = a * b;
    let e = a.mul_add(b, -p);
    (p, e)
}

/// A double-double: an unevaluated sum `hi + lo` representing one value with
/// roughly twice `f64`'s mantissa (≈106 bits vs. 53), where `lo` holds the
/// rounding error `hi` alone would have lost. See the module docs for how
/// this fits into the robustness contract (the "refine" stage, standing in
/// for Shewchuk's full arbitrary-length exact expansions).
///
/// The add/mul algorithms below are the standard "sloppy" double-double
/// routines (Bailey, Hida & Li, *Library for Double-Double and Quad-Double
/// Arithmetic*) built on the error-free transformations above; "sloppy" here
/// is the established name for this well-known, widely used variant (as
/// opposed to a slower "accurate" variant with an extra renormalization
/// pass) — not a caveat about this specific implementation.
#[derive(Debug, Clone, Copy)]
struct Dd {
    hi: f64,
    lo: f64,
}

impl Dd {
    /// The exact double-double representation of `a - b` (a single `f64`
    /// subtraction, widened losslessly via [`two_diff`]).
    #[inline]
    fn from_diff(a: f64, b: f64) -> Dd {
        let (hi, lo) = two_diff(a, b);
        Dd { hi, lo }
    }

    /// Negation (exact).
    #[inline]
    fn neg(self) -> Dd {
        Dd {
            hi: -self.hi,
            lo: -self.lo,
        }
    }

    /// Double-double sum, accurate to double-double precision (~106 bits).
    #[inline]
    fn add(self, other: Dd) -> Dd {
        let (s, e) = two_sum(self.hi, other.hi);
        let e = e + self.lo + other.lo;
        let (hi, lo) = quick_two_sum(s, e);
        Dd { hi, lo }
    }

    /// Double-double difference — `self.add(other.neg())`.
    #[inline]
    fn sub(self, other: Dd) -> Dd {
        self.add(other.neg())
    }

    /// Double-double product, accurate to double-double precision.
    #[inline]
    fn mul(self, other: Dd) -> Dd {
        let (p, e) = two_prod(self.hi, other.hi);
        // The cross terms `hi*lo` are already second-order-small relative to
        // `hi*hi`, so they are folded in as plain `f64` products (no need to
        // widen them further — this is exactly what makes it "double-double"
        // rather than "quad-double").
        let e = e + (self.hi * other.lo + self.lo * other.hi);
        let (hi, lo) = quick_two_sum(p, e);
        Dd { hi, lo }
    }

    /// Exact sign in `{-1, 0, +1}`. Since `lo` is by construction much
    /// smaller in magnitude than `hi` (or `hi` is exactly zero and `lo` holds
    /// the whole value), the sign of `hi` decides it unless `hi` is exactly
    /// `0.0`, in which case `lo`'s sign decides.
    #[inline]
    fn sign(self) -> i32 {
        if self.hi > 0.0 {
            1
        } else if self.hi < 0.0 {
            -1
        } else {
            sign_f64(self.lo)
        }
    }
}

/// Sign of a plain `f64` in `{-1, 0, +1}` (`NaN` and `-0.0`/`0.0` both map to
/// `0`, matching how the geometric predicates below use it — a computed
/// determinant is never expected to be `NaN` for finite input coordinates).
#[inline]
fn sign_f64(x: f64) -> i32 {
    if x > 0.0 {
        1
    } else if x < 0.0 {
        -1
    } else {
        0
    }
}

// ============================================================================
// orient2d
// ============================================================================

/// Plain-`f64` 2D orientation test. **Not robust** — near-collinear inputs
/// can silently return the wrong sign (or `0`) due to floating-point
/// cancellation. Use [`orient2d`] unless the caller has already established
/// the points are well away from collinear.
///
/// Returns `+1` if `a, b, c` occur counter-clockwise, `-1` if clockwise, `0`
/// if (numerically) collinear. The magnitude of the underlying determinant is
/// twice the signed area of triangle `abc`.
pub fn orient2d_fast(a: Vec2, b: Vec2, c: Vec2) -> i32 {
    sign_f64(orient2d_fast_det(a, b, c))
}

#[inline]
fn orient2d_fast_det(a: Vec2, b: Vec2, c: Vec2) -> f64 {
    let acx = a.x - c.x;
    let bcx = b.x - c.x;
    let acy = a.y - c.y;
    let bcy = b.y - c.y;
    acx * bcy - acy * bcx
}

/// Robust 2D orientation test — see the module-level robustness contract.
///
/// Returns `+1` if `a, b, c` occur counter-clockwise, `-1` if clockwise, `0`
/// if *exactly* (to double-double precision) collinear. Correct even when
/// [`orient2d_fast`] would flip sign or misreport `0` due to cancellation —
/// see `tests::orient2d_fast_gets_near_collinear_case_wrong` /
/// `tests::orient2d_robust_resolves_the_same_case_correctly` below for a
/// concrete demonstration.
pub fn orient2d(a: Vec2, b: Vec2, c: Vec2) -> i32 {
    let acx = a.x - c.x;
    let bcx = b.x - c.x;
    let acy = a.y - c.y;
    let bcy = b.y - c.y;

    let detleft = acx * bcy;
    let detright = acy * bcx;
    let det = detleft - detright;

    // Shewchuk's cheap opposite-sign short-circuit: if detleft and detright
    // have strictly opposite signs (or either is exactly zero), their
    // difference cannot have cancelled and `det`'s sign is already exact.
    let detsum = if detleft > 0.0 {
        if detright <= 0.0 {
            return sign_f64(det);
        }
        detleft + detright
    } else if detleft < 0.0 {
        if detright >= 0.0 {
            return sign_f64(det);
        }
        -detleft - detright
    } else {
        return sign_f64(det);
    };

    let errbound = CCW_ERR_BOUND_A * detsum;
    if det >= errbound || -det >= errbound {
        return sign_f64(det);
    }

    // Fast path inconclusive — refine in double-double precision.
    let acx_dd = Dd::from_diff(a.x, c.x);
    let bcx_dd = Dd::from_diff(b.x, c.x);
    let acy_dd = Dd::from_diff(a.y, c.y);
    let bcy_dd = Dd::from_diff(b.y, c.y);
    let det_dd = acx_dd.mul(bcy_dd).sub(acy_dd.mul(bcx_dd));
    det_dd.sign()
}

// ============================================================================
// orient3d
// ============================================================================

/// Plain-`f64` 3D orientation test. **Not robust** — see [`orient2d_fast`]'s
/// caveat; the same cancellation risk applies in 3D. Use [`orient3d`] unless
/// the caller has already established the points are well away from
/// coplanar.
///
/// Returns `+1` if `d` lies below the plane through `a, b, c` (with `a, b, c`
/// counter-clockwise viewed from above that plane), `-1` if above, `0` if
/// (numerically) coplanar. The magnitude of the underlying determinant is six
/// times the signed volume of tetrahedron `abcd`.
pub fn orient3d_fast(a: Vec3, b: Vec3, c: Vec3, d: Vec3) -> i32 {
    sign_f64(orient3d_fast_det(a, b, c, d))
}

#[inline]
fn orient3d_fast_det(a: Vec3, b: Vec3, c: Vec3, d: Vec3) -> f64 {
    let adx = a.x - d.x;
    let bdx = b.x - d.x;
    let cdx = c.x - d.x;
    let ady = a.y - d.y;
    let bdy = b.y - d.y;
    let cdy = c.y - d.y;
    let adz = a.z - d.z;
    let bdz = b.z - d.z;
    let cdz = c.z - d.z;
    adx * (bdy * cdz - bdz * cdy) + bdx * (cdy * adz - cdz * ady) + cdx * (ady * bdz - adz * bdy)
}

/// Robust 3D orientation test — see the module-level robustness contract.
///
/// Returns `+1` if `d` lies below the plane through `a, b, c` (with `a, b, c`
/// counter-clockwise viewed from above), `-1` if above, `0` if *exactly* (to
/// double-double precision) coplanar.
pub fn orient3d(a: Vec3, b: Vec3, c: Vec3, d: Vec3) -> i32 {
    let adx = a.x - d.x;
    let bdx = b.x - d.x;
    let cdx = c.x - d.x;
    let ady = a.y - d.y;
    let bdy = b.y - d.y;
    let cdy = c.y - d.y;
    let adz = a.z - d.z;
    let bdz = b.z - d.z;
    let cdz = c.z - d.z;

    let bdxcdy = bdx * cdy;
    let cdxbdy = cdx * bdy;
    let cdxady = cdx * ady;
    let adxcdy = adx * cdy;
    let adxbdy = adx * bdy;
    let bdxady = bdx * ady;

    let det = adz * (bdxcdy - cdxbdy) + bdz * (cdxady - adxcdy) + cdz * (adxbdy - bdxady);

    // Shewchuk's rigorous `permanent`-based error bound: sums the absolute
    // value of every term that contributed to `det`, so `errbound` bounds the
    // worst-case rounding error of the computation above regardless of which
    // terms happened to cancel.
    let permanent = (bdxcdy.abs() + cdxbdy.abs()) * adz.abs()
        + (cdxady.abs() + adxcdy.abs()) * bdz.abs()
        + (adxbdy.abs() + bdxady.abs()) * cdz.abs();
    let errbound = O3D_ERR_BOUND_A * permanent;
    if det > errbound || -det > errbound {
        return sign_f64(det);
    }

    // Fast path inconclusive — refine in double-double precision, replicating
    // the exact same formula structure as above.
    let adx_dd = Dd::from_diff(a.x, d.x);
    let bdx_dd = Dd::from_diff(b.x, d.x);
    let cdx_dd = Dd::from_diff(c.x, d.x);
    let ady_dd = Dd::from_diff(a.y, d.y);
    let bdy_dd = Dd::from_diff(b.y, d.y);
    let cdy_dd = Dd::from_diff(c.y, d.y);
    let adz_dd = Dd::from_diff(a.z, d.z);
    let bdz_dd = Dd::from_diff(b.z, d.z);
    let cdz_dd = Dd::from_diff(c.z, d.z);

    let bdxcdy_dd = bdx_dd.mul(cdy_dd);
    let cdxbdy_dd = cdx_dd.mul(bdy_dd);
    let cdxady_dd = cdx_dd.mul(ady_dd);
    let adxcdy_dd = adx_dd.mul(cdy_dd);
    let adxbdy_dd = adx_dd.mul(bdy_dd);
    let bdxady_dd = bdx_dd.mul(ady_dd);

    let det_dd = adz_dd
        .mul(bdxcdy_dd.sub(cdxbdy_dd))
        .add(bdz_dd.mul(cdxady_dd.sub(adxcdy_dd)))
        .add(cdz_dd.mul(adxbdy_dd.sub(bdxady_dd)));
    det_dd.sign()
}

// ============================================================================
// incircle
// ============================================================================

/// Plain-`f64` 2D in-circle test. **Not robust** — see [`orient2d_fast`]'s
/// caveat; the same cancellation risk applies here. Use [`incircle`] unless
/// the caller has already established `d` is well away from the circle
/// through `a, b, c`.
///
/// Returns `+1` if `d` lies inside the circle through `a, b, c` (which must
/// be given counter-clockwise, or the sign reverses), `-1` if outside, `0` if
/// (numerically) co-circular.
pub fn incircle_fast(a: Vec2, b: Vec2, c: Vec2, d: Vec2) -> i32 {
    sign_f64(incircle_fast_det(a, b, c, d))
}

#[inline]
fn incircle_fast_det(a: Vec2, b: Vec2, c: Vec2, d: Vec2) -> f64 {
    let adx = a.x - d.x;
    let ady = a.y - d.y;
    let bdx = b.x - d.x;
    let bdy = b.y - d.y;
    let cdx = c.x - d.x;
    let cdy = c.y - d.y;

    let abdet = adx * bdy - bdx * ady;
    let bcdet = bdx * cdy - cdx * bdy;
    let cadet = cdx * ady - adx * cdy;
    let alift = adx * adx + ady * ady;
    let blift = bdx * bdx + bdy * bdy;
    let clift = cdx * cdx + cdy * cdy;

    alift * bcdet + blift * cadet + clift * abdet
}

/// Robust 2D in-circle test — see the module-level robustness contract.
///
/// Returns `+1` if `d` lies inside the circle through `a, b, c` (which must
/// be given counter-clockwise, or the sign reverses), `-1` if outside, `0` if
/// *exactly* (to double-double precision) co-circular.
pub fn incircle(a: Vec2, b: Vec2, c: Vec2, d: Vec2) -> i32 {
    let adx = a.x - d.x;
    let bdx = b.x - d.x;
    let cdx = c.x - d.x;
    let ady = a.y - d.y;
    let bdy = b.y - d.y;
    let cdy = c.y - d.y;

    let bdxcdy = bdx * cdy;
    let cdxbdy = cdx * bdy;
    let alift = adx * adx + ady * ady;

    let cdxady = cdx * ady;
    let adxcdy = adx * cdy;
    let blift = bdx * bdx + bdy * bdy;

    let adxbdy = adx * bdy;
    let bdxady = bdx * ady;
    let clift = cdx * cdx + cdy * cdy;

    let det = alift * (bdxcdy - cdxbdy) + blift * (cdxady - adxcdy) + clift * (adxbdy - bdxady);

    let permanent = (bdxcdy.abs() + cdxbdy.abs()) * alift
        + (cdxady.abs() + adxcdy.abs()) * blift
        + (adxbdy.abs() + bdxady.abs()) * clift;
    let errbound = ICC_ERR_BOUND_A * permanent;
    if det > errbound || -det > errbound {
        return sign_f64(det);
    }

    // Fast path inconclusive — refine in double-double precision.
    let adx_dd = Dd::from_diff(a.x, d.x);
    let bdx_dd = Dd::from_diff(b.x, d.x);
    let cdx_dd = Dd::from_diff(c.x, d.x);
    let ady_dd = Dd::from_diff(a.y, d.y);
    let bdy_dd = Dd::from_diff(b.y, d.y);
    let cdy_dd = Dd::from_diff(c.y, d.y);

    let bdxcdy_dd = bdx_dd.mul(cdy_dd);
    let cdxbdy_dd = cdx_dd.mul(bdy_dd);
    let alift_dd = adx_dd.mul(adx_dd).add(ady_dd.mul(ady_dd));

    let cdxady_dd = cdx_dd.mul(ady_dd);
    let adxcdy_dd = adx_dd.mul(cdy_dd);
    let blift_dd = bdx_dd.mul(bdx_dd).add(bdy_dd.mul(bdy_dd));

    let adxbdy_dd = adx_dd.mul(bdy_dd);
    let bdxady_dd = bdx_dd.mul(ady_dd);
    let clift_dd = cdx_dd.mul(cdx_dd).add(cdy_dd.mul(cdy_dd));

    let det_dd = alift_dd
        .mul(bdxcdy_dd.sub(cdxbdy_dd))
        .add(blift_dd.mul(cdxady_dd.sub(adxcdy_dd)))
        .add(clift_dd.mul(adxbdy_dd.sub(bdxady_dd)));
    det_dd.sign()
}

// ============================================================================
// insphere (optional — implemented, but see the module docs and the note on
// `insphere` itself for why it skips the fast-path filter).
// ============================================================================

/// Plain-`f64` 3D in-sphere test. **Not robust** — see [`orient2d_fast`]'s
/// caveat. Use [`insphere`] unless the caller has already established `e` is
/// well away from the sphere through `a, b, c, d`.
///
/// Returns `+1` if `e` lies inside the sphere through `a, b, c, d` (which
/// must be positively oriented per [`orient3d`], or the sign reverses), `-1`
/// if outside, `0` if (numerically) co-spherical.
pub fn insphere_fast(a: Vec3, b: Vec3, c: Vec3, d: Vec3, e: Vec3) -> i32 {
    sign_f64(insphere_det(
        a.x - e.x,
        a.y - e.y,
        a.z - e.z,
        b.x - e.x,
        b.y - e.y,
        b.z - e.z,
        c.x - e.x,
        c.y - e.y,
        c.z - e.z,
        d.x - e.x,
        d.y - e.y,
        d.z - e.z,
    ))
}

/// Shared `f64` in-sphere determinant, factored out of [`insphere_fast`] so
/// [`insphere`]'s double-double refinement (below) can be a line-by-line
/// mirror of exactly this formula (Shewchuk's `inspherefast`), just with
/// [`Dd`] operations in place of `f64` ones.
#[allow(clippy::too_many_arguments)]
#[inline]
fn insphere_det(
    aex: f64,
    aey: f64,
    aez: f64,
    bex: f64,
    bey: f64,
    bez: f64,
    cex: f64,
    cey: f64,
    cez: f64,
    dex: f64,
    dey: f64,
    dez: f64,
) -> f64 {
    let ab = aex * bey - bex * aey;
    let bc = bex * cey - cex * bey;
    let cd = cex * dey - dex * cey;
    let da = dex * aey - aex * dey;
    let ac = aex * cey - cex * aey;
    let bd = bex * dey - dex * bey;

    let abc = aez * bc - bez * ac + cez * ab;
    let bcd = bez * cd - cez * bd + dez * bc;
    let cda = cez * da + dez * ac + aez * cd;
    let dab = dez * ab + aez * bd + bez * da;

    let alift = aex * aex + aey * aey + aez * aez;
    let blift = bex * bex + bey * bey + bez * bez;
    let clift = cex * cex + cey * cey + cez * cez;
    let dlift = dex * dex + dey * dey + dez * dez;

    (dlift * abc - clift * dab) + (blift * cda - alift * bcd)
}

/// Robust 3D in-sphere test.
///
/// Returns `+1` if `e` lies inside the sphere through `a, b, c, d` (which
/// must be positively oriented per [`orient3d`], or the sign reverses), `-1`
/// if outside, `0` if *exactly* (to double-double precision) co-spherical.
///
/// **Unlike [`orient2d`]/[`orient3d`]/[`incircle`] above, this always
/// evaluates in double-double precision — there is no `f64` fast-path
/// filter.** Shewchuk's real `isperrboundA`/permanent formula for insphere
/// folds together six 2x2 sub-determinants (`ab`, `bc`, `cd`, `da`, `ac`,
/// `bd`) into four 3x3 cofactors before the final 4-term combination; getting
/// that error-bound derivation subtly wrong from a partial read of the
/// reference source is a real risk, and a *too-tight* bound would make the
/// fast path unsoundly trust a wrong `f64` sign — silently worse than doing
/// no filtering at all. So this implementation is deliberately conservative:
/// always pay the double-double cost, never risk an unsound fast path. A
/// future contributor who re-derives and carefully verifies the exact
/// `isperrboundA` permanent formula against Shewchuk's source can add the
/// fast path the same way [`orient3d`]/[`incircle`] do.
pub fn insphere(a: Vec3, b: Vec3, c: Vec3, d: Vec3, e: Vec3) -> i32 {
    let aex = Dd::from_diff(a.x, e.x);
    let aey = Dd::from_diff(a.y, e.y);
    let aez = Dd::from_diff(a.z, e.z);
    let bex = Dd::from_diff(b.x, e.x);
    let bey = Dd::from_diff(b.y, e.y);
    let bez = Dd::from_diff(b.z, e.z);
    let cex = Dd::from_diff(c.x, e.x);
    let cey = Dd::from_diff(c.y, e.y);
    let cez = Dd::from_diff(c.z, e.z);
    let dex = Dd::from_diff(d.x, e.x);
    let dey = Dd::from_diff(d.y, e.y);
    let dez = Dd::from_diff(d.z, e.z);

    let ab = aex.mul(bey).sub(bex.mul(aey));
    let bc = bex.mul(cey).sub(cex.mul(bey));
    let cd = cex.mul(dey).sub(dex.mul(cey));
    let da = dex.mul(aey).sub(aex.mul(dey));
    let ac = aex.mul(cey).sub(cex.mul(aey));
    let bd = bex.mul(dey).sub(dex.mul(bey));

    let abc = aez.mul(bc).sub(bez.mul(ac)).add(cez.mul(ab));
    let bcd = bez.mul(cd).sub(cez.mul(bd)).add(dez.mul(bc));
    let cda = cez.mul(da).add(dez.mul(ac)).add(aez.mul(cd));
    let dab = dez.mul(ab).add(aez.mul(bd)).add(bez.mul(da));

    let alift = aex.mul(aex).add(aey.mul(aey)).add(aez.mul(aez));
    let blift = bex.mul(bex).add(bey.mul(bey)).add(bez.mul(bez));
    let clift = cex.mul(cex).add(cey.mul(cey)).add(cez.mul(cez));
    let dlift = dex.mul(dex).add(dey.mul(dey)).add(dez.mul(dez));

    let det_dd = dlift
        .mul(abc)
        .sub(clift.mul(dab))
        .add(blift.mul(cda))
        .sub(alift.mul(bcd));
    det_dd.sign()
}

#[cfg(test)]
mod tests {
    use super::*;

    // -- orient2d: clear-sign cases -----------------------------------------

    #[test]
    fn orient2d_ccw_triangle_is_positive() {
        let a = Vec2::new(0.0, 0.0);
        let b = Vec2::new(1.0, 0.0);
        let c = Vec2::new(0.0, 1.0);
        assert_eq!(orient2d(a, b, c), 1);
        assert_eq!(orient2d_fast(a, b, c), 1);
    }

    #[test]
    fn orient2d_cw_triangle_is_negative() {
        let a = Vec2::new(0.0, 0.0);
        let b = Vec2::new(0.0, 1.0);
        let c = Vec2::new(1.0, 0.0);
        assert_eq!(orient2d(a, b, c), -1);
        assert_eq!(orient2d_fast(a, b, c), -1);
    }

    #[test]
    fn orient2d_collinear_is_zero() {
        let a = Vec2::new(0.0, 0.0);
        let b = Vec2::new(1.0, 1.0);
        let c = Vec2::new(2.0, 2.0);
        assert_eq!(orient2d(a, b, c), 0);
        assert_eq!(orient2d_fast(a, b, c), 0);
    }

    // -- orient3d: clear-sign cases ------------------------------------------

    #[test]
    fn orient3d_point_below_xy_plane_is_positive() {
        // a, b, c CCW in the z=0 plane viewed from +z; d below (z<0) should be +1
        // per Blender's convention ("below" the CCW-from-above plane).
        let a = Vec3::new(0.0, 0.0, 0.0);
        let b = Vec3::new(1.0, 0.0, 0.0);
        let c = Vec3::new(0.0, 1.0, 0.0);
        let d = Vec3::new(0.0, 0.0, -1.0);
        assert_eq!(orient3d(a, b, c, d), 1);
        assert_eq!(orient3d_fast(a, b, c, d), 1);
    }

    #[test]
    fn orient3d_point_above_xy_plane_is_negative() {
        let a = Vec3::new(0.0, 0.0, 0.0);
        let b = Vec3::new(1.0, 0.0, 0.0);
        let c = Vec3::new(0.0, 1.0, 0.0);
        let d = Vec3::new(0.0, 0.0, 1.0);
        assert_eq!(orient3d(a, b, c, d), -1);
        assert_eq!(orient3d_fast(a, b, c, d), -1);
    }

    #[test]
    fn orient3d_coplanar_is_zero() {
        let a = Vec3::new(0.0, 0.0, 0.0);
        let b = Vec3::new(1.0, 0.0, 0.0);
        let c = Vec3::new(0.0, 1.0, 0.0);
        let d = Vec3::new(1.0, 1.0, 0.0); // also in z=0
        assert_eq!(orient3d(a, b, c, d), 0);
        assert_eq!(orient3d_fast(a, b, c, d), 0);
    }

    // -- incircle -------------------------------------------------------------

    #[test]
    fn incircle_point_clearly_inside_unit_circle() {
        // a, b, c on the unit circle, CCW.
        let a = Vec2::new(1.0, 0.0);
        let b = Vec2::new(0.0, 1.0);
        let c = Vec2::new(-1.0, 0.0);
        let d = Vec2::new(0.0, 0.0); // center: clearly inside
        assert_eq!(incircle(a, b, c, d), 1);
        assert_eq!(incircle_fast(a, b, c, d), 1);
    }

    #[test]
    fn incircle_point_clearly_outside_unit_circle() {
        let a = Vec2::new(1.0, 0.0);
        let b = Vec2::new(0.0, 1.0);
        let c = Vec2::new(-1.0, 0.0);
        let d = Vec2::new(0.0, -5.0); // far outside
        assert_eq!(incircle(a, b, c, d), -1);
        assert_eq!(incircle_fast(a, b, c, d), -1);
    }

    #[test]
    fn incircle_point_exactly_on_unit_circle() {
        let a = Vec2::new(1.0, 0.0);
        let b = Vec2::new(0.0, 1.0);
        let c = Vec2::new(-1.0, 0.0);
        let d = Vec2::new(0.0, -1.0); // also on the unit circle
        assert_eq!(incircle(a, b, c, d), 0);
        assert_eq!(incircle_fast(a, b, c, d), 0);
    }

    // -- insphere ---------------------------------------------------------------

    #[test]
    fn insphere_point_clearly_inside_and_outside_unit_sphere() {
        // a,b,c,d: four points on the unit sphere, positively oriented
        // (orient3d(a,b,c,d) > 0 — verified by the assertion below rather than
        // asserted a priori, since the sign convention is easy to get backwards
        // by hand).
        let a = Vec3::new(1.0, 0.0, 0.0);
        let b = Vec3::new(0.0, 1.0, 0.0);
        let c = Vec3::new(0.0, 0.0, 1.0);
        let d = Vec3::new(-1.0, 0.0, 0.0);
        assert_eq!(
            orient3d(a, b, c, d),
            1,
            "test fixture must be positively oriented"
        );

        let inside = Vec3::new(0.0, 0.0, 0.0); // center
        let outside = Vec3::new(10.0, 10.0, 10.0);
        assert_eq!(insphere(a, b, c, d, inside), 1);
        assert_eq!(insphere_fast(a, b, c, d, inside), 1);
        assert_eq!(insphere(a, b, c, d, outside), -1);
        assert_eq!(insphere_fast(a, b, c, d, outside), -1);
    }

    // -- robustness case --------------------------------------------------------
    //
    // Construction: three points on the line y = x, offset by a large common
    // `base` (so the coordinate differences `a-c`, `b-c` are computed exactly by
    // Sterbenz's lemma — no precision is lost there), with `c` perturbed off the
    // line by a `tiny` amount far below the ULP of the ~`base^2`-magnitude cross
    // products that get subtracted to test orientation. The true sign is
    // whatever `tiny`'s sign is (here positive: `c` sits fractionally "above"
    // the line, making `a, b, c` wind clockwise, i.e. orient2d < 0), but the
    // plain `f64` computation cancels `tiny`'s contribution away entirely and
    // reports exactly collinear (0) instead of the true nonzero sign — the
    // double-double-refined path resolves it correctly.
    //
    // This fixture was **not** hand-derived — it was found by a Python search
    // script (exact `fractions.Fraction` rational determinant vs. plain-`f64`
    // determinant, scanning random large-magnitude near-collinear triples for
    // a sign mismatch; see the crate's git history for this file for the
    // script). At these coordinates the two cross-product terms
    // (`detleft = acx*bcy`, `detright = acy*bcx`) round to the **exact same
    // `f64` value** (`-1731181.3838254893`), so the naive subtraction
    // `detleft - detright` collapses to exactly `0.0` — [`orient2d_fast`]
    // reports "collinear" — while the true (exact-rational) determinant is
    // small but strictly negative. `detleft == detright` also means
    // [`orient2d`]'s cheap opposite-sign short-circuit does not fire (both
    // are negative), so it correctly falls through to the double-double
    // refinement, which resolves the true nonzero sign.
    fn near_collinear_points() -> (Vec2, Vec2, Vec2) {
        let a = Vec2::new(216.71927942731116, -602.9672492496101);
        let b = Vec2::new(-395.16791284110053, 377.0800080674168);
        let c = Vec2::new(994.4985243693698, -1848.7204296319146);
        (a, b, c)
    }

    #[test]
    fn orient2d_fast_gets_near_collinear_case_wrong() {
        let (a, b, c) = near_collinear_points();
        // The true sign (exact rational arithmetic, computed offline — see
        // the construction comment above) is negative. The naive f64 path
        // loses that signal to cancellation (detleft and detright round to
        // the identical f64 value) and reports 0 instead.
        assert_eq!(
            orient2d_fast(a, b, c),
            0,
            "fixture must actually exhibit the fast-path failure it is documented to \
             (if this fails, the fixture no longer demonstrates the bug and needs re-tuning)"
        );
    }

    #[test]
    fn orient2d_robust_resolves_the_same_case_correctly() {
        let (a, b, c) = near_collinear_points();
        let robust = orient2d(a, b, c);
        let fast = orient2d_fast(a, b, c);
        assert_ne!(
            robust, fast,
            "robust path must disagree with the known-wrong fast path"
        );
        assert_eq!(
            robust, -1,
            "true sign, per the offline exact-rational computation"
        );
    }
}
