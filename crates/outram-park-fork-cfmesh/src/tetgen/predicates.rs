// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Line-for-line Rust port of TetGen 1.6.0 `predicates.cxx`:
//   https://github.com/libigl/tetgen, commit e05aca7df74e (TetGen 1.6.0),
//   file `predicates.cxx` (and the declarations in `tetgen.h`).
//
// The arithmetic and predicates are Jonathan Richard Shewchuk's
//   "Routines for Arbitrary Precision Floating-point Arithmetic and Fast
//   Robust Geometric Predicates" (predicates.c), May 18, 1996,
//   "Placed in the public domain by Jonathan Richard Shewchuk, School of
//   Computer Science, Carnegie Mellon University, 5000 Forbes Avenue,
//   Pittsburgh, Pennsylvania 15213-3891, jrs@cs.cmu.edu".
//   Paper: J. R. Shewchuk, "Adaptive Precision Floating-Point Arithmetic and
//   Fast Robust Geometric Predicates", Technical Report CMU-CS-96-140, School
//   of Computer Science, Carnegie Mellon University, May 1996.
//   http://www.cs.cmu.edu/~quake/robust.html
//
// TetGen's modifications (the static filters, the `-X` options in
// `exactinit()`, `orient4d()`): Copyright (C) 2002-2020 Hang Si, Weierstrass
// Institute for Applied Analysis and Stochastics (WIAS), Berlin.
// TetGen licence: GNU Affero General Public License v3.0 or later.
//
// This file is part of OUTRAM PARK. It is free software: you can redistribute
// it and/or modify it under the terms of the GNU Affero General Public License
// as published by the Free Software Foundation, either version 3 of the
// License, or (at your option) any later version. It is distributed WITHOUT
// ANY WARRANTY; see the GNU Affero General Public License for details.

//! # TetGen's exact geometric predicates (gh:#1790, stage 1 of gh:#1789)
//!
//! A line-for-line port of `predicates.cxx` from TetGen 1.6.0: Shewchuk's
//! adaptive-precision floating-point expansions and robust predicates, as
//! modified by Hang Si with a **static filter** and the `-X` options.
//!
//! ## What is ported
//!
//! - `exactinit` → [`Predicates::exactinit`]. The C globals (`epsilon`,
//!   `splitter`, every error bound, `_use_inexact_arith`,
//!   `_use_static_filter`, `o3dstaticfilter`, `ispstaticfilter`) are fields of
//!   [`Predicates`]; every predicate is a method on it.
//! - `orient3d` (the non-CGAL branch), `orient3dfast`, `orient3dadapt`,
//!   `orient3dexact`; `insphere`, `insphereadapt`, `insphereexact`;
//!   `orient4d`, `orient4dadapt`, `orient4dexact`; `orient2dexact`.
//! - The expansion routines they call (`fast_expansion_sum_zeroelim`,
//!   `scale_expansion_zeroelim`, `estimate`) and the macros they expand
//!   (`Fast_Two_Sum`, `Two_Sum`, `Two_Diff` and their tails, `Split`,
//!   `Two_Product`, `Two_Product_Presplit`, `Two_One_Diff`, `Two_Two_Diff`,
//!   `Two_One_Product`). Each macro is an `#[inline]` function returning a
//!   tuple (workspace rule: no `macro_rules!`), with the same operations in the
//!   same order. `Absolute` is `fabs`, i.e. [`f64::abs`].
//!
//! Macros and routines the listed predicates never reach (`Square`,
//! `Two_Two_Sum`, `Four_*`, `Eight_*`, `grow_expansion`, `compress`, the
//! `*slow` and 2D-incircle routines, the CGAL branch) are not ported.
//!
//! ## Deliberate differences from the C, none of which changes a value
//!
//! - **The FPU control word is not set.** `exactinit` calls `_FPU_SETCW` /
//!   `_control87` only under `LINUX` / `CPU86`, for x87 extended precision.
//!   Rust's `f64` is IEEE binary64 with round-to-nearest-even on every target
//!   this workspace builds for (SSE2 on x86_64, NEON on aarch64, wasm32), so
//!   there is no register precision to control.
//! - **`test_double` and the `verbose` printouts are skipped**: they only
//!   print, and the crate is `no_std` without `std`.
//! - **Reads past the end of an expansion.** `fast_expansion_sum_zeroelim`
//!   reads `e[++eindex]` once past the last component and never uses the
//!   value; the port reads 0.0 there instead (`at`).
//! - **No floating-point contraction.** Rust never fuses `a * b + c` into an
//!   FMA; the C harness that checks this port is built with
//!   `-ffp-contract=off` for the same reason.
//! - **`insphereexact`'s scratch arrays are on the heap** (`alloc::vec`), about
//!   180 KB of them; on the stack, as in the C, they would be a hazard on a
//!   small thread stack. The values are unchanged.
//!
//! ## Verification
//!
//! Bit-identical (`to_bits()` equality) against upstream `predicates.cxx`
//! compiled by a C harness, on committed vectors:
//! `tests/tetgen_predicates_vs_upstream.rs`; methodology, regeneration and the
//! case counts in
//! `verification_and_validation/tetgen_predicates_gh1790/README.md`.

#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
use crate::nostd_prelude::*;

// ---------------------------------------------------------------------------
// The macros, as functions. Shewchuk: "Many of the operations are broken up
// into two pieces, a main part that performs an approximate operation, and a
// 'tail' that computes the roundoff error of that operation."
// ---------------------------------------------------------------------------

/// `Fast_Two_Sum(a, b, x, y)`: `x = a + b`, `y` its roundoff (needs
/// `|a| >= |b|`). Returns `(x, y)`.
#[inline]
pub fn fast_two_sum(a: f64, b: f64) -> (f64, f64) {
    let x = a + b;
    let bvirt = x - a;
    let y = b - bvirt;
    (x, y)
}

/// `Two_Sum_Tail(a, b, x, y)`: the roundoff `y` of `x = a + b`.
#[inline]
pub fn two_sum_tail(a: f64, b: f64, x: f64) -> f64 {
    let bvirt = x - a;
    let avirt = x - bvirt;
    let bround = b - bvirt;
    let around = a - avirt;
    around + bround
}

/// `Two_Sum(a, b, x, y)`: `x = a + b` and its exact roundoff. Returns `(x, y)`.
#[inline]
pub fn two_sum(a: f64, b: f64) -> (f64, f64) {
    let x = a + b;
    (x, two_sum_tail(a, b, x))
}

/// `Two_Diff_Tail(a, b, x, y)`: the roundoff `y` of `x = a - b`.
#[inline]
pub fn two_diff_tail(a: f64, b: f64, x: f64) -> f64 {
    let bvirt = a - x;
    let avirt = x + bvirt;
    let bround = bvirt - b;
    let around = a - avirt;
    around + bround
}

/// `Two_Diff(a, b, x, y)`: `x = a - b` and its exact roundoff. Returns `(x, y)`.
#[inline]
pub fn two_diff(a: f64, b: f64) -> (f64, f64) {
    let x = a - b;
    (x, two_diff_tail(a, b, x))
}

/// `Two_One_Diff(a1, a0, b, x2, x1, x0)`: `(a1, a0) - b`. Returns
/// `(x2, x1, x0)`.
#[inline]
pub fn two_one_diff(a1: f64, a0: f64, b: f64) -> (f64, f64, f64) {
    let (i, x0) = two_diff(a0, b);
    let (x2, x1) = two_sum(a1, i);
    (x2, x1, x0)
}

/// `Two_Two_Diff(a1, a0, b1, b0, x3, x2, x1, x0)`: `(a1, a0) - (b1, b0)`.
/// Returns `(x3, x2, x1, x0)`.
#[inline]
pub fn two_two_diff(a1: f64, a0: f64, b1: f64, b0: f64) -> (f64, f64, f64, f64) {
    let (j, z0, x0) = two_one_diff(a1, a0, b0);
    let (x3, x2, x1) = two_one_diff(j, z0, b1);
    (x3, x2, x1, x0)
}

/// `e[i]`, or 0.0 one past the end. The C reads `e[++eindex]` past the last
/// component and never uses the value; this keeps the read in bounds.
#[inline]
fn at(e: &[f64], i: usize) -> f64 {
    if i < e.len() {
        e[i]
    } else {
        0.0
    }
}

/// `fast_expansion_sum_zeroelim()`: sum two expansions, eliminating zero
/// components from the output expansion.
///
/// Sets `h = e + f`. If round-to-even is used (as with IEEE 754), maintains
/// the strongly nonoverlapping property. (That is, if `e` is strongly
/// nonoverlapping, `h` will be also.) Does NOT maintain the nonoverlapping or
/// nonadjacent properties. `h` cannot be `e` or `f`. Returns the length of
/// `h`.
pub fn fast_expansion_sum_zeroelim(
    elen: usize,
    e: &[f64],
    flen: usize,
    f: &[f64],
    h: &mut [f64],
) -> usize {
    let mut q: f64;
    let mut qnew: f64;
    let mut hh: f64;
    let mut eindex: usize;
    let mut findex: usize;
    let mut hindex: usize;
    let mut enow: f64;
    let mut fnow: f64;

    enow = e[0];
    fnow = f[0];
    eindex = 0;
    findex = 0;
    if (fnow > enow) == (fnow > -enow) {
        q = enow;
        eindex += 1;
        enow = at(e, eindex);
    } else {
        q = fnow;
        findex += 1;
        fnow = at(f, findex);
    }
    hindex = 0;
    if (eindex < elen) && (findex < flen) {
        if (fnow > enow) == (fnow > -enow) {
            (qnew, hh) = fast_two_sum(enow, q);
            eindex += 1;
            enow = at(e, eindex);
        } else {
            (qnew, hh) = fast_two_sum(fnow, q);
            findex += 1;
            fnow = at(f, findex);
        }
        q = qnew;
        if hh != 0.0 {
            h[hindex] = hh;
            hindex += 1;
        }
        while (eindex < elen) && (findex < flen) {
            if (fnow > enow) == (fnow > -enow) {
                (qnew, hh) = two_sum(q, enow);
                eindex += 1;
                enow = at(e, eindex);
            } else {
                (qnew, hh) = two_sum(q, fnow);
                findex += 1;
                fnow = at(f, findex);
            }
            q = qnew;
            if hh != 0.0 {
                h[hindex] = hh;
                hindex += 1;
            }
        }
    }
    while eindex < elen {
        (qnew, hh) = two_sum(q, enow);
        eindex += 1;
        enow = at(e, eindex);
        q = qnew;
        if hh != 0.0 {
            h[hindex] = hh;
            hindex += 1;
        }
    }
    while findex < flen {
        (qnew, hh) = two_sum(q, fnow);
        findex += 1;
        fnow = at(f, findex);
        q = qnew;
        if hh != 0.0 {
            h[hindex] = hh;
            hindex += 1;
        }
    }
    if (q != 0.0) || (hindex == 0) {
        h[hindex] = q;
        hindex += 1;
    }
    hindex
}

/// `estimate()`: produce a one-word estimate of an expansion's value.
pub fn estimate(elen: usize, e: &[f64]) -> f64 {
    let mut q = e[0];
    for eindex in 1..elen {
        q += e[eindex];
    }
    q
}

/// The computed constants and options of TetGen's predicates: the C file's
/// `static` globals, set by `exactinit()`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Predicates {
    /// `splitter = 2^ceiling(p / 2) + 1`. Used to split floats in half.
    pub splitter: f64,
    /// `= 2^(-p)`. Used to estimate roundoff errors.
    pub epsilon: f64,
    /// Coefficient used to calculate maximum roundoff errors.
    pub resulterrbound: f64,
    /// 2D orientation error bound A.
    pub ccwerrbound_a: f64,
    /// 2D orientation error bound B.
    pub ccwerrbound_b: f64,
    /// 2D orientation error bound C.
    pub ccwerrbound_c: f64,
    /// 3D orientation error bound A.
    pub o3derrbound_a: f64,
    /// 3D orientation error bound B.
    pub o3derrbound_b: f64,
    /// 3D orientation error bound C.
    pub o3derrbound_c: f64,
    /// Incircle error bound A.
    pub iccerrbound_a: f64,
    /// Incircle error bound B.
    pub iccerrbound_b: f64,
    /// Incircle error bound C.
    pub iccerrbound_c: f64,
    /// Insphere error bound A.
    pub isperrbound_a: f64,
    /// Insphere error bound B.
    pub isperrbound_b: f64,
    /// Insphere error bound C.
    pub isperrbound_c: f64,
    /// `_use_inexact_arith`: TetGen's `-X` option (`noexact`). When set,
    /// `orient3d` and `insphere` return the floating-point determinant.
    pub use_inexact_arith: bool,
    /// `_use_static_filter`: on by default, off with `-X1` (`nofilter`).
    pub use_static_filter: bool,
    /// Static filter for `orient3d()` (H. Si, 2012-08-23).
    pub o3dstaticfilter: f64,
    /// Static filter for `insphere()` (H. Si, 2012-08-23).
    pub ispstaticfilter: f64,
}

impl Predicates {
    /// `exactinit()`: initialize the variables used for exact arithmetic.
    ///
    /// `epsilon` is the largest power of two such that 1.0 + epsilon = 1.0 in
    /// floating-point arithmetic. `epsilon` bounds the relative roundoff
    /// error. It is used for floating-point error analysis.
    ///
    /// `splitter` is used to split floating-point numbers into two
    /// half-length significands for exact multiplication.
    ///
    /// TetGen's additions (H. Si, 2012-08-23): `noexact` (`-X`) sets
    /// `use_inexact_arith`; `nofilter` (`-X1`) clears `use_static_filter`;
    /// `maxx`, `maxy`, `maxz` (TetGen passes the bounding box's extents) are
    /// sorted ascending and give the two static filters.
    ///
    /// Not ported: the FPU control word (`_FPU_SETCW` / `_control87`, x87
    /// only), `test_double` and the `verbose` printouts; see the module docs.
    pub fn exactinit(noexact: bool, nofilter: bool, maxx: f64, maxy: f64, maxz: f64) -> Self {
        let (mut maxx, mut maxy, mut maxz) = (maxx, maxy, maxz);
        let mut half: f64;
        let mut check: f64;
        let mut lastcheck: f64;
        let mut every_other: bool;

        every_other = true;
        half = 0.5;
        let mut epsilon: f64 = 1.0;
        let mut splitter: f64 = 1.0;
        check = 1.0;
        // Repeatedly divide `epsilon' by two until it is too small to add to
        //   one without causing roundoff.  (Also check if the sum is equal to
        //   the previous sum, for machines that round up instead of using
        //   exact rounding.  Not that this library will work on such machines
        //   anyway.
        loop {
            lastcheck = check;
            epsilon *= half;
            if every_other {
                splitter *= 2.0;
            }
            every_other = !every_other;
            check = 1.0 + epsilon;
            if !((check != 1.0) && (check != lastcheck)) {
                break;
            }
        }
        splitter += 1.0;

        // Error bounds for orientation and incircle tests.
        let resulterrbound = (3.0 + 8.0 * epsilon) * epsilon;
        let ccwerrbound_a = (3.0 + 16.0 * epsilon) * epsilon;
        let ccwerrbound_b = (2.0 + 12.0 * epsilon) * epsilon;
        let ccwerrbound_c = (9.0 + 64.0 * epsilon) * epsilon * epsilon;
        let o3derrbound_a = (7.0 + 56.0 * epsilon) * epsilon;
        let o3derrbound_b = (3.0 + 28.0 * epsilon) * epsilon;
        let o3derrbound_c = (26.0 + 288.0 * epsilon) * epsilon * epsilon;
        let iccerrbound_a = (10.0 + 96.0 * epsilon) * epsilon;
        let iccerrbound_b = (4.0 + 48.0 * epsilon) * epsilon;
        let iccerrbound_c = (44.0 + 576.0 * epsilon) * epsilon * epsilon;
        let isperrbound_a = (16.0 + 224.0 * epsilon) * epsilon;
        let isperrbound_b = (5.0 + 72.0 * epsilon) * epsilon;
        let isperrbound_c = (71.0 + 1408.0 * epsilon) * epsilon * epsilon;

        // Set TetGen options.  Added by H. Si, 2012-08-23.
        let use_inexact_arith = noexact;
        let use_static_filter = !nofilter;

        // Calculate the two static filters for orient3d() and insphere() tests.
        // Added by H. Si, 2012-08-23.

        // Sort maxx < maxy < maxz. Re-use 'half' for swapping.
        if maxx > maxz {
            half = maxx;
            maxx = maxz;
            maxz = half;
        }
        if maxy > maxz {
            half = maxy;
            maxy = maxz;
            maxz = half;
        } else if maxy < maxx {
            half = maxy;
            maxy = maxx;
            maxx = half;
        }
        let _ = half;

        let o3dstaticfilter = 5.1107127829973299e-15 * maxx * maxy * maxz;
        let ispstaticfilter = 1.2466136531027298e-13 * maxx * maxy * maxz * (maxz * maxz);

        Predicates {
            splitter,
            epsilon,
            resulterrbound,
            ccwerrbound_a,
            ccwerrbound_b,
            ccwerrbound_c,
            o3derrbound_a,
            o3derrbound_b,
            o3derrbound_c,
            iccerrbound_a,
            iccerrbound_b,
            iccerrbound_c,
            isperrbound_a,
            isperrbound_b,
            isperrbound_c,
            use_inexact_arith,
            use_static_filter,
            o3dstaticfilter,
            ispstaticfilter,
        }
    }

    // -----------------------------------------------------------------------
    // The macros that need `splitter`.
    // -----------------------------------------------------------------------

    /// `Split(a, ahi, alo)`: split `a` into two half-length significands.
    /// Returns `(ahi, alo)`.
    #[inline]
    pub fn split(&self, a: f64) -> (f64, f64) {
        let c = self.splitter * a;
        let abig = c - a;
        let ahi = c - abig;
        let alo = a - ahi;
        (ahi, alo)
    }

    /// `Two_Product(a, b, x, y)`: `x = a * b` and its exact roundoff. Returns
    /// `(x, y)`.
    #[inline]
    pub fn two_product(&self, a: f64, b: f64) -> (f64, f64) {
        let x = a * b;
        // Two_Product_Tail(a, b, x, y)
        let (ahi, alo) = self.split(a);
        let (bhi, blo) = self.split(b);
        let err1 = x - (ahi * bhi);
        let err2 = err1 - (alo * bhi);
        let err3 = err2 - (ahi * blo);
        let y = (alo * blo) - err3;
        (x, y)
    }

    /// `Two_Product_Presplit(a, b, bhi, blo, x, y)`: `Two_Product()` where
    /// one of the inputs has already been split. Avoids redundant splitting.
    #[inline]
    pub fn two_product_presplit(&self, a: f64, b: f64, bhi: f64, blo: f64) -> (f64, f64) {
        let x = a * b;
        let (ahi, alo) = self.split(a);
        let err1 = x - (ahi * bhi);
        let err2 = err1 - (alo * bhi);
        let err3 = err2 - (ahi * blo);
        let y = (alo * blo) - err3;
        (x, y)
    }

    /// `Two_One_Product(a1, a0, b, x3, x2, x1, x0)`: `(a1, a0) * b`. Returns
    /// `(x3, x2, x1, x0)`.
    #[inline]
    pub fn two_one_product(&self, a1: f64, a0: f64, b: f64) -> (f64, f64, f64, f64) {
        let (bhi, blo) = self.split(b);
        let (i, x0) = self.two_product_presplit(a0, b, bhi, blo);
        let (j, z0) = self.two_product_presplit(a1, b, bhi, blo);
        let (k, x1) = two_sum(i, z0);
        let (x3, x2) = fast_two_sum(j, k);
        (x3, x2, x1, x0)
    }

    /// `Two_Product` of `(a, b)` and of `(c, d)`, then `Two_Two_Diff` of the
    /// two products: the 4-component expansion of `a*b - c*d`, written into
    /// `out` as `[x0, x1, x2, x3]`. The pattern every exact routine opens with.
    #[inline]
    fn two_two_diff_of_products(&self, a: f64, b: f64, c: f64, d: f64, out: &mut [f64; 4]) {
        let (ab1, ab0) = self.two_product(a, b);
        let (cd1, cd0) = self.two_product(c, d);
        let (x3, x2, x1, x0) = two_two_diff(ab1, ab0, cd1, cd0);
        out[3] = x3;
        out[2] = x2;
        out[1] = x1;
        out[0] = x0;
    }

    /// `scale_expansion_zeroelim()`: multiply an expansion by a scalar,
    /// eliminating zero components from the output expansion.
    ///
    /// Sets `h = be`. Maintains the nonoverlapping property. If round-to-even
    /// is used (as with IEEE 754), maintains the strongly nonoverlapping and
    /// nonadjacent properties as well. `e` and `h` cannot be the same.
    /// Returns the length of `h`.
    pub fn scale_expansion_zeroelim(&self, elen: usize, e: &[f64], b: f64, h: &mut [f64]) -> usize {
        let mut q: f64;
        let mut sum: f64;
        let mut hh: f64;
        let mut product1: f64;
        let mut product0: f64;
        let mut hindex: usize;
        let mut enow: f64;

        let (bhi, blo) = self.split(b);
        (q, hh) = self.two_product_presplit(e[0], b, bhi, blo);
        hindex = 0;
        if hh != 0.0 {
            h[hindex] = hh;
            hindex += 1;
        }
        for eindex in 1..elen {
            enow = e[eindex];
            (product1, product0) = self.two_product_presplit(enow, b, bhi, blo);
            (sum, hh) = two_sum(q, product0);
            if hh != 0.0 {
                h[hindex] = hh;
                hindex += 1;
            }
            (q, hh) = fast_two_sum(product1, sum);
            if hh != 0.0 {
                h[hindex] = hh;
                hindex += 1;
            }
        }
        if (q != 0.0) || (hindex == 0) {
            h[hindex] = q;
            hindex += 1;
        }
        hindex
    }

    // -----------------------------------------------------------------------
    // orient2d
    // -----------------------------------------------------------------------

    /// `orient2dexact()`: exact 2D orientation test. Robust.
    ///
    /// Returns a positive value if the points `pa`, `pb` and `pc` occur in
    /// counterclockwise order; a negative value if they occur in clockwise
    /// order; and zero if they are collinear. Reads `p[0]` and `p[1]` of each
    /// point (TetGen passes 3D points).
    pub fn orient2dexact(&self, pa: &[f64], pb: &[f64], pc: &[f64]) -> f64 {
        let mut aterms = [0.0f64; 4];
        let mut bterms = [0.0f64; 4];
        let mut cterms = [0.0f64; 4];
        let mut v = [0.0f64; 8];
        let mut w = [0.0f64; 12];

        self.two_two_diff_of_products(pa[0], pb[1], pa[0], pc[1], &mut aterms);
        self.two_two_diff_of_products(pb[0], pc[1], pb[0], pa[1], &mut bterms);
        self.two_two_diff_of_products(pc[0], pa[1], pc[0], pb[1], &mut cterms);

        let vlength = fast_expansion_sum_zeroelim(4, &aterms, 4, &bterms, &mut v);
        let wlength = fast_expansion_sum_zeroelim(vlength, &v, 4, &cterms, &mut w);

        w[wlength - 1]
    }

    // -----------------------------------------------------------------------
    // orient3d
    // -----------------------------------------------------------------------

    /// `orient3dfast()`: approximate 3D orientation test. Nonrobust.
    pub fn orient3dfast(&self, pa: &[f64; 3], pb: &[f64; 3], pc: &[f64; 3], pd: &[f64; 3]) -> f64 {
        let adx = pa[0] - pd[0];
        let bdx = pb[0] - pd[0];
        let cdx = pc[0] - pd[0];
        let ady = pa[1] - pd[1];
        let bdy = pb[1] - pd[1];
        let cdy = pc[1] - pd[1];
        let adz = pa[2] - pd[2];
        let bdz = pb[2] - pd[2];
        let cdz = pc[2] - pd[2];

        adx * (bdy * cdz - bdz * cdy)
            + bdx * (cdy * adz - cdz * ady)
            + cdx * (ady * bdz - adz * bdy)
    }

    /// `orient3dexact()`: exact 3D orientation test. Robust.
    pub fn orient3dexact(&self, pa: &[f64; 3], pb: &[f64; 3], pc: &[f64; 3], pd: &[f64; 3]) -> f64 {
        let mut ab = [0.0f64; 4];
        let mut bc = [0.0f64; 4];
        let mut cd = [0.0f64; 4];
        let mut da = [0.0f64; 4];
        let mut ac = [0.0f64; 4];
        let mut bd = [0.0f64; 4];
        let mut temp8 = [0.0f64; 8];
        let mut abc = [0.0f64; 12];
        let mut bcd = [0.0f64; 12];
        let mut cda = [0.0f64; 12];
        let mut dab = [0.0f64; 12];
        let mut adet = [0.0f64; 24];
        let mut bdet = [0.0f64; 24];
        let mut cdet = [0.0f64; 24];
        let mut ddet = [0.0f64; 24];
        let mut abdet = [0.0f64; 48];
        let mut cddet = [0.0f64; 48];
        let mut deter = [0.0f64; 96];

        self.two_two_diff_of_products(pa[0], pb[1], pb[0], pa[1], &mut ab);
        self.two_two_diff_of_products(pb[0], pc[1], pc[0], pb[1], &mut bc);
        self.two_two_diff_of_products(pc[0], pd[1], pd[0], pc[1], &mut cd);
        self.two_two_diff_of_products(pd[0], pa[1], pa[0], pd[1], &mut da);
        self.two_two_diff_of_products(pa[0], pc[1], pc[0], pa[1], &mut ac);
        self.two_two_diff_of_products(pb[0], pd[1], pd[0], pb[1], &mut bd);

        let mut templen = fast_expansion_sum_zeroelim(4, &cd, 4, &da, &mut temp8);
        let cdalen = fast_expansion_sum_zeroelim(templen, &temp8, 4, &ac, &mut cda);
        templen = fast_expansion_sum_zeroelim(4, &da, 4, &ab, &mut temp8);
        let dablen = fast_expansion_sum_zeroelim(templen, &temp8, 4, &bd, &mut dab);
        for i in 0..4 {
            bd[i] = -bd[i];
            ac[i] = -ac[i];
        }
        templen = fast_expansion_sum_zeroelim(4, &ab, 4, &bc, &mut temp8);
        let abclen = fast_expansion_sum_zeroelim(templen, &temp8, 4, &ac, &mut abc);
        templen = fast_expansion_sum_zeroelim(4, &bc, 4, &cd, &mut temp8);
        let bcdlen = fast_expansion_sum_zeroelim(templen, &temp8, 4, &bd, &mut bcd);

        let alen = self.scale_expansion_zeroelim(bcdlen, &bcd, pa[2], &mut adet);
        let blen = self.scale_expansion_zeroelim(cdalen, &cda, -pb[2], &mut bdet);
        let clen = self.scale_expansion_zeroelim(dablen, &dab, pc[2], &mut cdet);
        let dlen = self.scale_expansion_zeroelim(abclen, &abc, -pd[2], &mut ddet);

        let ablen = fast_expansion_sum_zeroelim(alen, &adet, blen, &bdet, &mut abdet);
        let cdlen = fast_expansion_sum_zeroelim(clen, &cdet, dlen, &ddet, &mut cddet);
        let deterlen = fast_expansion_sum_zeroelim(ablen, &abdet, cdlen, &cddet, &mut deter);

        deter[deterlen - 1]
    }

    /// `orient3dadapt()`: the adaptive stages B, C and the exact tail of
    /// [`Predicates::orient3d`]. `permanent` is the stage-A permanent.
    pub fn orient3dadapt(
        &self,
        pa: &[f64; 3],
        pb: &[f64; 3],
        pc: &[f64; 3],
        pd: &[f64; 3],
        permanent: f64,
    ) -> f64 {
        let mut det: f64;
        let mut errbound: f64;

        let mut bc = [0.0f64; 4];
        let mut ca = [0.0f64; 4];
        let mut ab = [0.0f64; 4];
        let mut adet = [0.0f64; 8];
        let mut bdet = [0.0f64; 8];
        let mut cdet = [0.0f64; 8];
        let mut abdet = [0.0f64; 16];
        let mut fin1 = [0.0f64; 192];
        let mut fin2 = [0.0f64; 192];
        let mut finlength: usize;

        let mut at_b = [0.0f64; 4];
        let mut at_c = [0.0f64; 4];
        let mut bt_c = [0.0f64; 4];
        let mut bt_a = [0.0f64; 4];
        let mut ct_a = [0.0f64; 4];
        let mut ct_b = [0.0f64; 4];
        let at_blen: usize;
        let at_clen: usize;
        let bt_clen: usize;
        let bt_alen: usize;
        let ct_alen: usize;
        let ct_blen: usize;
        let mut bct = [0.0f64; 8];
        let mut cat = [0.0f64; 8];
        let mut abt = [0.0f64; 8];
        let mut u = [0.0f64; 4];
        let mut v = [0.0f64; 12];
        let mut w = [0.0f64; 16];
        let mut vlength: usize;
        let mut wlength: usize;
        let mut negate: f64;

        let adx = pa[0] - pd[0];
        let bdx = pb[0] - pd[0];
        let cdx = pc[0] - pd[0];
        let ady = pa[1] - pd[1];
        let bdy = pb[1] - pd[1];
        let cdy = pc[1] - pd[1];
        let adz = pa[2] - pd[2];
        let bdz = pb[2] - pd[2];
        let cdz = pc[2] - pd[2];

        self.two_two_diff_of_products(bdx, cdy, cdx, bdy, &mut bc);
        let alen = self.scale_expansion_zeroelim(4, &bc, adz, &mut adet);

        self.two_two_diff_of_products(cdx, ady, adx, cdy, &mut ca);
        let blen = self.scale_expansion_zeroelim(4, &ca, bdz, &mut bdet);

        self.two_two_diff_of_products(adx, bdy, bdx, ady, &mut ab);
        let clen = self.scale_expansion_zeroelim(4, &ab, cdz, &mut cdet);

        let ablen = fast_expansion_sum_zeroelim(alen, &adet, blen, &bdet, &mut abdet);
        finlength = fast_expansion_sum_zeroelim(ablen, &abdet, clen, &cdet, &mut fin1);

        det = estimate(finlength, &fin1);
        errbound = self.o3derrbound_b * permanent;
        if (det >= errbound) || (-det >= errbound) {
            return det;
        }

        let adxtail = two_diff_tail(pa[0], pd[0], adx);
        let bdxtail = two_diff_tail(pb[0], pd[0], bdx);
        let cdxtail = two_diff_tail(pc[0], pd[0], cdx);
        let adytail = two_diff_tail(pa[1], pd[1], ady);
        let bdytail = two_diff_tail(pb[1], pd[1], bdy);
        let cdytail = two_diff_tail(pc[1], pd[1], cdy);
        let adztail = two_diff_tail(pa[2], pd[2], adz);
        let bdztail = two_diff_tail(pb[2], pd[2], bdz);
        let cdztail = two_diff_tail(pc[2], pd[2], cdz);

        if (adxtail == 0.0)
            && (bdxtail == 0.0)
            && (cdxtail == 0.0)
            && (adytail == 0.0)
            && (bdytail == 0.0)
            && (cdytail == 0.0)
            && (adztail == 0.0)
            && (bdztail == 0.0)
            && (cdztail == 0.0)
        {
            return det;
        }

        errbound = self.o3derrbound_c * permanent + self.resulterrbound * det.abs();
        det += (adz * ((bdx * cdytail + cdy * bdxtail) - (bdy * cdxtail + cdx * bdytail))
            + adztail * (bdx * cdy - bdy * cdx))
            + (bdz * ((cdx * adytail + ady * cdxtail) - (cdy * adxtail + adx * cdytail))
                + bdztail * (cdx * ady - cdy * adx))
            + (cdz * ((adx * bdytail + bdy * adxtail) - (ady * bdxtail + bdx * adytail))
                + cdztail * (adx * bdy - ady * bdx));
        if (det >= errbound) || (-det >= errbound) {
            return det;
        }

        let mut finnow: &mut [f64; 192] = &mut fin1;
        let mut finother: &mut [f64; 192] = &mut fin2;

        if adxtail == 0.0 {
            if adytail == 0.0 {
                at_b[0] = 0.0;
                at_blen = 1;
                at_c[0] = 0.0;
                at_clen = 1;
            } else {
                negate = -adytail;
                let (at_blarge, x0) = self.two_product(negate, bdx);
                at_b[0] = x0;
                at_b[1] = at_blarge;
                at_blen = 2;
                let (at_clarge, x0) = self.two_product(adytail, cdx);
                at_c[0] = x0;
                at_c[1] = at_clarge;
                at_clen = 2;
            }
        } else if adytail == 0.0 {
            let (at_blarge, x0) = self.two_product(adxtail, bdy);
            at_b[0] = x0;
            at_b[1] = at_blarge;
            at_blen = 2;
            negate = -adxtail;
            let (at_clarge, x0) = self.two_product(negate, cdy);
            at_c[0] = x0;
            at_c[1] = at_clarge;
            at_clen = 2;
        } else {
            self.two_two_diff_of_products(adxtail, bdy, adytail, bdx, &mut at_b);
            at_blen = 4;
            self.two_two_diff_of_products(adytail, cdx, adxtail, cdy, &mut at_c);
            at_clen = 4;
        }
        if bdxtail == 0.0 {
            if bdytail == 0.0 {
                bt_c[0] = 0.0;
                bt_clen = 1;
                bt_a[0] = 0.0;
                bt_alen = 1;
            } else {
                negate = -bdytail;
                let (bt_clarge, x0) = self.two_product(negate, cdx);
                bt_c[0] = x0;
                bt_c[1] = bt_clarge;
                bt_clen = 2;
                let (bt_alarge, x0) = self.two_product(bdytail, adx);
                bt_a[0] = x0;
                bt_a[1] = bt_alarge;
                bt_alen = 2;
            }
        } else if bdytail == 0.0 {
            let (bt_clarge, x0) = self.two_product(bdxtail, cdy);
            bt_c[0] = x0;
            bt_c[1] = bt_clarge;
            bt_clen = 2;
            negate = -bdxtail;
            let (bt_alarge, x0) = self.two_product(negate, ady);
            bt_a[0] = x0;
            bt_a[1] = bt_alarge;
            bt_alen = 2;
        } else {
            self.two_two_diff_of_products(bdxtail, cdy, bdytail, cdx, &mut bt_c);
            bt_clen = 4;
            self.two_two_diff_of_products(bdytail, adx, bdxtail, ady, &mut bt_a);
            bt_alen = 4;
        }
        if cdxtail == 0.0 {
            if cdytail == 0.0 {
                ct_a[0] = 0.0;
                ct_alen = 1;
                ct_b[0] = 0.0;
                ct_blen = 1;
            } else {
                negate = -cdytail;
                let (ct_alarge, x0) = self.two_product(negate, adx);
                ct_a[0] = x0;
                ct_a[1] = ct_alarge;
                ct_alen = 2;
                let (ct_blarge, x0) = self.two_product(cdytail, bdx);
                ct_b[0] = x0;
                ct_b[1] = ct_blarge;
                ct_blen = 2;
            }
        } else if cdytail == 0.0 {
            let (ct_alarge, x0) = self.two_product(cdxtail, ady);
            ct_a[0] = x0;
            ct_a[1] = ct_alarge;
            ct_alen = 2;
            negate = -cdxtail;
            let (ct_blarge, x0) = self.two_product(negate, bdy);
            ct_b[0] = x0;
            ct_b[1] = ct_blarge;
            ct_blen = 2;
        } else {
            self.two_two_diff_of_products(cdxtail, ady, cdytail, adx, &mut ct_a);
            ct_alen = 4;
            self.two_two_diff_of_products(cdytail, bdx, cdxtail, bdy, &mut ct_b);
            ct_blen = 4;
        }

        let bctlen = fast_expansion_sum_zeroelim(bt_clen, &bt_c, ct_blen, &ct_b, &mut bct);
        wlength = self.scale_expansion_zeroelim(bctlen, &bct, adz, &mut w);
        finlength =
            fast_expansion_sum_zeroelim(finlength, &finnow[..], wlength, &w, &mut finother[..]);
        core::mem::swap(&mut finnow, &mut finother);

        let catlen = fast_expansion_sum_zeroelim(ct_alen, &ct_a, at_clen, &at_c, &mut cat);
        wlength = self.scale_expansion_zeroelim(catlen, &cat, bdz, &mut w);
        finlength =
            fast_expansion_sum_zeroelim(finlength, &finnow[..], wlength, &w, &mut finother[..]);
        core::mem::swap(&mut finnow, &mut finother);

        let abtlen = fast_expansion_sum_zeroelim(at_blen, &at_b, bt_alen, &bt_a, &mut abt);
        wlength = self.scale_expansion_zeroelim(abtlen, &abt, cdz, &mut w);
        finlength =
            fast_expansion_sum_zeroelim(finlength, &finnow[..], wlength, &w, &mut finother[..]);
        core::mem::swap(&mut finnow, &mut finother);

        if adztail != 0.0 {
            vlength = self.scale_expansion_zeroelim(4, &bc, adztail, &mut v);
            finlength =
                fast_expansion_sum_zeroelim(finlength, &finnow[..], vlength, &v, &mut finother[..]);
            core::mem::swap(&mut finnow, &mut finother);
        }
        if bdztail != 0.0 {
            vlength = self.scale_expansion_zeroelim(4, &ca, bdztail, &mut v);
            finlength =
                fast_expansion_sum_zeroelim(finlength, &finnow[..], vlength, &v, &mut finother[..]);
            core::mem::swap(&mut finnow, &mut finother);
        }
        if cdztail != 0.0 {
            vlength = self.scale_expansion_zeroelim(4, &ab, cdztail, &mut v);
            finlength =
                fast_expansion_sum_zeroelim(finlength, &finnow[..], vlength, &v, &mut finother[..]);
            core::mem::swap(&mut finnow, &mut finother);
        }

        if adxtail != 0.0 {
            if bdytail != 0.0 {
                let (adxt_bdyt1, adxt_bdyt0) = self.two_product(adxtail, bdytail);
                self.two_one_product_into(adxt_bdyt1, adxt_bdyt0, cdz, &mut u);
                finlength =
                    fast_expansion_sum_zeroelim(finlength, &finnow[..], 4, &u, &mut finother[..]);
                core::mem::swap(&mut finnow, &mut finother);
                if cdztail != 0.0 {
                    self.two_one_product_into(adxt_bdyt1, adxt_bdyt0, cdztail, &mut u);
                    finlength = fast_expansion_sum_zeroelim(
                        finlength,
                        &finnow[..],
                        4,
                        &u,
                        &mut finother[..],
                    );
                    core::mem::swap(&mut finnow, &mut finother);
                }
            }
            if cdytail != 0.0 {
                negate = -adxtail;
                let (adxt_cdyt1, adxt_cdyt0) = self.two_product(negate, cdytail);
                self.two_one_product_into(adxt_cdyt1, adxt_cdyt0, bdz, &mut u);
                finlength =
                    fast_expansion_sum_zeroelim(finlength, &finnow[..], 4, &u, &mut finother[..]);
                core::mem::swap(&mut finnow, &mut finother);
                if bdztail != 0.0 {
                    self.two_one_product_into(adxt_cdyt1, adxt_cdyt0, bdztail, &mut u);
                    finlength = fast_expansion_sum_zeroelim(
                        finlength,
                        &finnow[..],
                        4,
                        &u,
                        &mut finother[..],
                    );
                    core::mem::swap(&mut finnow, &mut finother);
                }
            }
        }
        if bdxtail != 0.0 {
            if cdytail != 0.0 {
                let (bdxt_cdyt1, bdxt_cdyt0) = self.two_product(bdxtail, cdytail);
                self.two_one_product_into(bdxt_cdyt1, bdxt_cdyt0, adz, &mut u);
                finlength =
                    fast_expansion_sum_zeroelim(finlength, &finnow[..], 4, &u, &mut finother[..]);
                core::mem::swap(&mut finnow, &mut finother);
                if adztail != 0.0 {
                    self.two_one_product_into(bdxt_cdyt1, bdxt_cdyt0, adztail, &mut u);
                    finlength = fast_expansion_sum_zeroelim(
                        finlength,
                        &finnow[..],
                        4,
                        &u,
                        &mut finother[..],
                    );
                    core::mem::swap(&mut finnow, &mut finother);
                }
            }
            if adytail != 0.0 {
                negate = -bdxtail;
                let (bdxt_adyt1, bdxt_adyt0) = self.two_product(negate, adytail);
                self.two_one_product_into(bdxt_adyt1, bdxt_adyt0, cdz, &mut u);
                finlength =
                    fast_expansion_sum_zeroelim(finlength, &finnow[..], 4, &u, &mut finother[..]);
                core::mem::swap(&mut finnow, &mut finother);
                if cdztail != 0.0 {
                    self.two_one_product_into(bdxt_adyt1, bdxt_adyt0, cdztail, &mut u);
                    finlength = fast_expansion_sum_zeroelim(
                        finlength,
                        &finnow[..],
                        4,
                        &u,
                        &mut finother[..],
                    );
                    core::mem::swap(&mut finnow, &mut finother);
                }
            }
        }
        if cdxtail != 0.0 {
            if adytail != 0.0 {
                let (cdxt_adyt1, cdxt_adyt0) = self.two_product(cdxtail, adytail);
                self.two_one_product_into(cdxt_adyt1, cdxt_adyt0, bdz, &mut u);
                finlength =
                    fast_expansion_sum_zeroelim(finlength, &finnow[..], 4, &u, &mut finother[..]);
                core::mem::swap(&mut finnow, &mut finother);
                if bdztail != 0.0 {
                    self.two_one_product_into(cdxt_adyt1, cdxt_adyt0, bdztail, &mut u);
                    finlength = fast_expansion_sum_zeroelim(
                        finlength,
                        &finnow[..],
                        4,
                        &u,
                        &mut finother[..],
                    );
                    core::mem::swap(&mut finnow, &mut finother);
                }
            }
            if bdytail != 0.0 {
                negate = -cdxtail;
                let (cdxt_bdyt1, cdxt_bdyt0) = self.two_product(negate, bdytail);
                self.two_one_product_into(cdxt_bdyt1, cdxt_bdyt0, adz, &mut u);
                finlength =
                    fast_expansion_sum_zeroelim(finlength, &finnow[..], 4, &u, &mut finother[..]);
                core::mem::swap(&mut finnow, &mut finother);
                if adztail != 0.0 {
                    self.two_one_product_into(cdxt_bdyt1, cdxt_bdyt0, adztail, &mut u);
                    finlength = fast_expansion_sum_zeroelim(
                        finlength,
                        &finnow[..],
                        4,
                        &u,
                        &mut finother[..],
                    );
                    core::mem::swap(&mut finnow, &mut finother);
                }
            }
        }

        if adztail != 0.0 {
            wlength = self.scale_expansion_zeroelim(bctlen, &bct, adztail, &mut w);
            finlength =
                fast_expansion_sum_zeroelim(finlength, &finnow[..], wlength, &w, &mut finother[..]);
            core::mem::swap(&mut finnow, &mut finother);
        }
        if bdztail != 0.0 {
            wlength = self.scale_expansion_zeroelim(catlen, &cat, bdztail, &mut w);
            finlength =
                fast_expansion_sum_zeroelim(finlength, &finnow[..], wlength, &w, &mut finother[..]);
            core::mem::swap(&mut finnow, &mut finother);
        }
        if cdztail != 0.0 {
            wlength = self.scale_expansion_zeroelim(abtlen, &abt, cdztail, &mut w);
            finlength =
                fast_expansion_sum_zeroelim(finlength, &finnow[..], wlength, &w, &mut finother[..]);
            core::mem::swap(&mut finnow, &mut finother);
        }

        finnow[finlength - 1]
    }

    /// `Two_One_Product(a1, a0, b, u3, u[2], u[1], u[0]); u[3] = u3;`
    #[inline]
    fn two_one_product_into(&self, a1: f64, a0: f64, b: f64, u: &mut [f64; 4]) {
        let (u3, u2, u1, u0) = self.two_one_product(a1, a0, b);
        u[2] = u2;
        u[1] = u1;
        u[0] = u0;
        u[3] = u3;
    }

    /// `orient3d()` (the non-CGAL branch): adaptive exact 3D orientation test.
    /// Robust.
    ///
    /// Returns a positive value if the point `pd` lies below the plane passing
    /// through `pa`, `pb` and `pc`; "below" is defined so that `pa`, `pb` and
    /// `pc` appear in counterclockwise order when viewed from above the
    /// plane. Returns a negative value if `pd` lies above the plane. Returns
    /// zero if the points are coplanar. The result is also a rough
    /// approximation of six times the signed volume of the tetrahedron defined
    /// by the four points.
    ///
    /// TetGen's order: with `use_inexact_arith` the floating-point determinant
    /// is returned; else with `use_static_filter` it is returned when
    /// `det > o3dstaticfilter` or `det < -o3dstaticfilter` (not `fabs`, as
    /// upstream writes it); else Shewchuk's stage-A bound, then
    /// [`Predicates::orient3dadapt`].
    pub fn orient3d(&self, pa: &[f64; 3], pb: &[f64; 3], pc: &[f64; 3], pd: &[f64; 3]) -> f64 {
        let adx = pa[0] - pd[0];
        let ady = pa[1] - pd[1];
        let adz = pa[2] - pd[2];
        let bdx = pb[0] - pd[0];
        let bdy = pb[1] - pd[1];
        let bdz = pb[2] - pd[2];
        let cdx = pc[0] - pd[0];
        let cdy = pc[1] - pd[1];
        let cdz = pc[2] - pd[2];

        let bdxcdy = bdx * cdy;
        let cdxbdy = cdx * bdy;

        let cdxady = cdx * ady;
        let adxcdy = adx * cdy;

        let adxbdy = adx * bdy;
        let bdxady = bdx * ady;

        let det = adz * (bdxcdy - cdxbdy) + bdz * (cdxady - adxcdy) + cdz * (adxbdy - bdxady);

        if self.use_inexact_arith {
            return det;
        }

        if self.use_static_filter {
            //if (fabs(det) > o3dstaticfilter) return det;
            if det > self.o3dstaticfilter {
                return det;
            }
            if det < -self.o3dstaticfilter {
                return det;
            }
        }

        let permanent = (bdxcdy.abs() + cdxbdy.abs()) * adz.abs()
            + (cdxady.abs() + adxcdy.abs()) * bdz.abs()
            + (adxbdy.abs() + bdxady.abs()) * cdz.abs();
        let errbound = self.o3derrbound_a * permanent;
        if (det > errbound) || (-det > errbound) {
            return det;
        }

        self.orient3dadapt(pa, pb, pc, pd, permanent)
    }

    // -----------------------------------------------------------------------
    // insphere
    // -----------------------------------------------------------------------

    /// `sum = scale(x, xs) + scale(y, ys)`, then `h = scale(z, zs) + sum`: the
    /// repeated five-line block of `insphereexact` and `orient4dexact`
    /// (`temp8a`, `temp8b`, `temp16`, `temp8a` again, then the output), with
    /// the C's operations in the C's order. Returns the length of `h`.
    #[allow(clippy::too_many_arguments)]
    fn three_term_exact(
        &self,
        x: &[f64; 4],
        xs: f64,
        y: &[f64; 4],
        ys: f64,
        z: &[f64; 4],
        zs: f64,
        h: &mut [f64],
    ) -> usize {
        let mut temp8a = [0.0f64; 8];
        let mut temp8b = [0.0f64; 8];
        let mut temp16 = [0.0f64; 16];
        let mut temp8alen = self.scale_expansion_zeroelim(4, x, xs, &mut temp8a);
        let temp8blen = self.scale_expansion_zeroelim(4, y, ys, &mut temp8b);
        let temp16len =
            fast_expansion_sum_zeroelim(temp8alen, &temp8a, temp8blen, &temp8b, &mut temp16);
        temp8alen = self.scale_expansion_zeroelim(4, z, zs, &mut temp8a);
        fast_expansion_sum_zeroelim(temp8alen, &temp8a, temp16len, &temp16, h)
    }

    /// The ten 4-component 2x2 minors `ab, bc, cd, de, ea, ac, bd, ce, da, eb`
    /// of the x-y columns, as `insphereexact` and `orient4dexact` open.
    #[allow(clippy::type_complexity)]
    fn five_point_minors(
        &self,
        pa: &[f64; 3],
        pb: &[f64; 3],
        pc: &[f64; 3],
        pd: &[f64; 3],
        pe: &[f64; 3],
    ) -> [[f64; 4]; 10] {
        let mut m = [[0.0f64; 4]; 10];
        self.two_two_diff_of_products(pa[0], pb[1], pb[0], pa[1], &mut m[0]); // ab
        self.two_two_diff_of_products(pb[0], pc[1], pc[0], pb[1], &mut m[1]); // bc
        self.two_two_diff_of_products(pc[0], pd[1], pd[0], pc[1], &mut m[2]); // cd
        self.two_two_diff_of_products(pd[0], pe[1], pe[0], pd[1], &mut m[3]); // de
        self.two_two_diff_of_products(pe[0], pa[1], pa[0], pe[1], &mut m[4]); // ea
        self.two_two_diff_of_products(pa[0], pc[1], pc[0], pa[1], &mut m[5]); // ac
        self.two_two_diff_of_products(pb[0], pd[1], pd[0], pb[1], &mut m[6]); // bd
        self.two_two_diff_of_products(pc[0], pe[1], pe[0], pc[1], &mut m[7]); // ce
        self.two_two_diff_of_products(pd[0], pa[1], pa[0], pd[1], &mut m[8]); // da
        self.two_two_diff_of_products(pe[0], pb[1], pb[0], pe[1], &mut m[9]); // eb
        m
    }

    /// The ten 3x3 minors `abc, bcd, cde, dea, eab, abd, bce, cda, deb, eac`
    /// (24 components each, lengths returned beside them), as
    /// `insphereexact` and `orient4dexact` build them.
    fn five_point_triples(
        &self,
        pa: &[f64; 3],
        pb: &[f64; 3],
        pc: &[f64; 3],
        pd: &[f64; 3],
        pe: &[f64; 3],
    ) -> ([[f64; 24]; 10], [usize; 10]) {
        let [ab, bc, cd, de, ea, ac, bd, ce, da, eb] = self.five_point_minors(pa, pb, pc, pd, pe);
        let mut t = [[0.0f64; 24]; 10];
        let mut n = [0usize; 10];
        n[0] = self.three_term_exact(&bc, pa[2], &ac, -pb[2], &ab, pc[2], &mut t[0]); // abc
        n[1] = self.three_term_exact(&cd, pb[2], &bd, -pc[2], &bc, pd[2], &mut t[1]); // bcd
        n[2] = self.three_term_exact(&de, pc[2], &ce, -pd[2], &cd, pe[2], &mut t[2]); // cde
        n[3] = self.three_term_exact(&ea, pd[2], &da, -pe[2], &de, pa[2], &mut t[3]); // dea
        n[4] = self.three_term_exact(&ab, pe[2], &eb, -pa[2], &ea, pb[2], &mut t[4]); // eab
        n[5] = self.three_term_exact(&bd, pa[2], &da, pb[2], &ab, pd[2], &mut t[5]); // abd
        n[6] = self.three_term_exact(&ce, pb[2], &eb, pc[2], &bc, pe[2], &mut t[6]); // bce
        n[7] = self.three_term_exact(&da, pc[2], &ac, pd[2], &cd, pa[2], &mut t[7]); // cda
        n[8] = self.three_term_exact(&eb, pd[2], &bd, pe[2], &de, pb[2], &mut t[8]); // deb
        n[9] = self.three_term_exact(&ac, pe[2], &ce, pa[2], &ea, pc[2], &mut t[9]); // eac
        (t, n)
    }

    /// `temp48a = p + q; temp48b = -(r + s); out = temp48a + temp48b`: the
    /// 4x4 minor block of `insphereexact` / `orient4dexact`. Returns the
    /// length of `out` (at most 96).
    #[allow(clippy::too_many_arguments)]
    fn four_minor_exact(
        p: &[f64],
        plen: usize,
        q: &[f64],
        qlen: usize,
        r: &[f64],
        rlen: usize,
        s: &[f64],
        slen: usize,
        out: &mut [f64; 96],
    ) -> usize {
        let mut temp48a = [0.0f64; 48];
        let mut temp48b = [0.0f64; 48];
        let temp48alen = fast_expansion_sum_zeroelim(plen, p, qlen, q, &mut temp48a);
        let temp48blen = fast_expansion_sum_zeroelim(rlen, r, slen, s, &mut temp48b);
        for i in 0..temp48blen {
            temp48b[i] = -temp48b[i];
        }
        fast_expansion_sum_zeroelim(temp48alen, &temp48a, temp48blen, &temp48b, out)
    }

    /// The five 4x4 minors `bcde, cdea, deab, eabc, abcd` (96 components
    /// each) and their lengths, in the C's order of construction.
    fn five_point_quads(
        &self,
        pa: &[f64; 3],
        pb: &[f64; 3],
        pc: &[f64; 3],
        pd: &[f64; 3],
        pe: &[f64; 3],
    ) -> ([[f64; 96]; 5], [usize; 5]) {
        let (t, n) = self.five_point_triples(pa, pb, pc, pd, pe);
        let (abc, bcd, cde, dea, eab) = (&t[0], &t[1], &t[2], &t[3], &t[4]);
        let (abd, bce, cda, deb, eac) = (&t[5], &t[6], &t[7], &t[8], &t[9]);
        let (abclen, bcdlen, cdelen, dealen, eablen) = (n[0], n[1], n[2], n[3], n[4]);
        let (abdlen, bcelen, cdalen, deblen, eaclen) = (n[5], n[6], n[7], n[8], n[9]);
        let mut q = [[0.0f64; 96]; 5];
        let mut l = [0usize; 5];
        // bcde
        l[0] = Self::four_minor_exact(
            cde, cdelen, bce, bcelen, deb, deblen, bcd, bcdlen, &mut q[0],
        );
        // cdea
        l[1] = Self::four_minor_exact(
            dea, dealen, cda, cdalen, eac, eaclen, cde, cdelen, &mut q[1],
        );
        // deab
        l[2] = Self::four_minor_exact(
            eab, eablen, deb, deblen, abd, abdlen, dea, dealen, &mut q[2],
        );
        // eabc
        l[3] = Self::four_minor_exact(
            abc, abclen, eac, eaclen, bce, bcelen, eab, eablen, &mut q[3],
        );
        // abcd
        l[4] = Self::four_minor_exact(
            bcd, bcdlen, abd, abdlen, cda, cdalen, abc, abclen, &mut q[4],
        );
        (q, l)
    }

    /// `insphereexact()`: exact 3D insphere test. Robust.
    ///
    /// The C builds the five 4x4 minors interleaved with their lift products
    /// (each minor is complete before it is used); the port builds the five
    /// minors first and then the products, which performs the same
    /// operations on the same operands. Scratch arrays are on the heap.
    pub fn insphereexact(
        &self,
        pa: &[f64; 3],
        pb: &[f64; 3],
        pc: &[f64; 3],
        pd: &[f64; 3],
        pe: &[f64; 3],
    ) -> f64 {
        let (q, l) = self.five_point_quads(pa, pb, pc, pd, pe);
        let mut temp192 = vec![0.0f64; 192];
        let mut det384x = vec![0.0f64; 384];
        let mut det384y = vec![0.0f64; 384];
        let mut det384z = vec![0.0f64; 384];
        let mut detxy = vec![0.0f64; 768];
        let mut dets = vec![vec![0.0f64; 1152]; 5];
        let mut lens = [0usize; 5];
        // adet from bcde and pa, bdet from cdea and pb, cdet from deab and pc,
        // ddet from eabc and pd, edet from abcd and pe.
        let pts = [pa, pb, pc, pd, pe];
        for k in 0..5 {
            let (m, mlen, p) = (&q[k], l[k], pts[k]);
            let mut xlen = self.scale_expansion_zeroelim(mlen, m, p[0], &mut temp192);
            xlen = self.scale_expansion_zeroelim(xlen, &temp192, p[0], &mut det384x);
            let mut ylen = self.scale_expansion_zeroelim(mlen, m, p[1], &mut temp192);
            ylen = self.scale_expansion_zeroelim(ylen, &temp192, p[1], &mut det384y);
            let mut zlen = self.scale_expansion_zeroelim(mlen, m, p[2], &mut temp192);
            zlen = self.scale_expansion_zeroelim(zlen, &temp192, p[2], &mut det384z);
            let xylen = fast_expansion_sum_zeroelim(xlen, &det384x, ylen, &det384y, &mut detxy);
            lens[k] = fast_expansion_sum_zeroelim(xylen, &detxy, zlen, &det384z, &mut dets[k]);
        }

        let mut abdet = vec![0.0f64; 2304];
        let mut cddet = vec![0.0f64; 2304];
        let mut cdedet = vec![0.0f64; 3456];
        let mut deter = vec![0.0f64; 5760];
        let ablen = fast_expansion_sum_zeroelim(lens[0], &dets[0], lens[1], &dets[1], &mut abdet);
        let cdlen = fast_expansion_sum_zeroelim(lens[2], &dets[2], lens[3], &dets[3], &mut cddet);
        let cdelen = fast_expansion_sum_zeroelim(cdlen, &cddet, lens[4], &dets[4], &mut cdedet);
        let deterlen = fast_expansion_sum_zeroelim(ablen, &abdet, cdelen, &cdedet, &mut deter);

        deter[deterlen - 1]
    }

    /// The 2x2 minors `ab, bc, cd, da, ac, bd` of the x-y differences, and
    /// their leading components `ab3 … bd3`, as `insphereadapt` and
    /// `orient4dadapt` open (both relative to `pe`).
    #[allow(clippy::too_many_arguments)]
    fn adapt_minors(
        &self,
        aex: f64,
        aey: f64,
        bex: f64,
        bey: f64,
        cex: f64,
        cey: f64,
        dex: f64,
        dey: f64,
    ) -> [[f64; 4]; 6] {
        let mut m = [[0.0f64; 4]; 6];
        self.two_two_diff_of_products(aex, bey, bex, aey, &mut m[0]); // ab
        self.two_two_diff_of_products(bex, cey, cex, bey, &mut m[1]); // bc
        self.two_two_diff_of_products(cex, dey, dex, cey, &mut m[2]); // cd
        self.two_two_diff_of_products(dex, aey, aex, dey, &mut m[3]); // da
        self.two_two_diff_of_products(aex, cey, cex, aey, &mut m[4]); // ac
        self.two_two_diff_of_products(bex, dey, dex, bey, &mut m[5]); // bd
        m
    }

    /// `temp16 = scale(x, xs) + scale(y, ys); temp24 = scale(z, zs) + temp16`
    /// (the `temp8a`, `temp8b`, `temp8c` block of `insphereadapt` and
    /// `orient4dadapt`). Returns the length of `temp24`.
    #[allow(clippy::too_many_arguments)]
    fn adapt_three_term(
        &self,
        x: &[f64; 4],
        xs: f64,
        y: &[f64; 4],
        ys: f64,
        z: &[f64; 4],
        zs: f64,
        temp24: &mut [f64; 24],
    ) -> usize {
        let mut temp8a = [0.0f64; 8];
        let mut temp8b = [0.0f64; 8];
        let mut temp8c = [0.0f64; 8];
        let mut temp16 = [0.0f64; 16];
        let temp8alen = self.scale_expansion_zeroelim(4, x, xs, &mut temp8a);
        let temp8blen = self.scale_expansion_zeroelim(4, y, ys, &mut temp8b);
        let temp8clen = self.scale_expansion_zeroelim(4, z, zs, &mut temp8c);
        let temp16len =
            fast_expansion_sum_zeroelim(temp8alen, &temp8a, temp8blen, &temp8b, &mut temp16);
        fast_expansion_sum_zeroelim(temp8clen, &temp8c, temp16len, &temp16, temp24)
    }

    /// One lifted term of `insphereadapt`: `x = (t*ex)*sx`, `y = (t*ey)*sy`,
    /// `z = (t*ez)*sz`, `out = (x + y) + z`, with `sx = ±ex` etc. as the C
    /// passes them. Returns the length of `out`.
    #[allow(clippy::too_many_arguments)]
    fn insphereadapt_lift(
        &self,
        temp24len: usize,
        temp24: &[f64; 24],
        ex: f64,
        sx: f64,
        ey: f64,
        sy: f64,
        ez: f64,
        sz: f64,
        out: &mut [f64; 288],
    ) -> usize {
        let mut temp48 = [0.0f64; 48];
        let mut xdet = [0.0f64; 96];
        let mut ydet = [0.0f64; 96];
        let mut zdet = [0.0f64; 96];
        let mut xydet = [0.0f64; 192];
        let mut temp48len = self.scale_expansion_zeroelim(temp24len, temp24, ex, &mut temp48);
        let xlen = self.scale_expansion_zeroelim(temp48len, &temp48, sx, &mut xdet);
        temp48len = self.scale_expansion_zeroelim(temp24len, temp24, ey, &mut temp48);
        let ylen = self.scale_expansion_zeroelim(temp48len, &temp48, sy, &mut ydet);
        temp48len = self.scale_expansion_zeroelim(temp24len, temp24, ez, &mut temp48);
        let zlen = self.scale_expansion_zeroelim(temp48len, &temp48, sz, &mut zdet);
        let xylen = fast_expansion_sum_zeroelim(xlen, &xdet, ylen, &ydet, &mut xydet);
        fast_expansion_sum_zeroelim(xylen, &xydet, zlen, &zdet, out)
    }

    /// `insphereadapt()`: the adaptive stages B, C and the exact fallback of
    /// [`Predicates::insphere`]. `permanent` is the stage-A permanent.
    pub fn insphereadapt(
        &self,
        pa: &[f64; 3],
        pb: &[f64; 3],
        pc: &[f64; 3],
        pd: &[f64; 3],
        pe: &[f64; 3],
        permanent: f64,
    ) -> f64 {
        let mut det: f64;
        let mut errbound: f64;
        let mut temp24 = [0.0f64; 24];
        let mut temp24len: usize;
        let mut adet = [0.0f64; 288];
        let mut bdet = [0.0f64; 288];
        let mut cdet = [0.0f64; 288];
        let mut ddet = [0.0f64; 288];
        let mut abdet = [0.0f64; 576];
        let mut cddet = [0.0f64; 576];
        let mut fin1 = [0.0f64; 1152];

        let aex = pa[0] - pe[0];
        let bex = pb[0] - pe[0];
        let cex = pc[0] - pe[0];
        let dex = pd[0] - pe[0];
        let aey = pa[1] - pe[1];
        let bey = pb[1] - pe[1];
        let cey = pc[1] - pe[1];
        let dey = pd[1] - pe[1];
        let aez = pa[2] - pe[2];
        let bez = pb[2] - pe[2];
        let cez = pc[2] - pe[2];
        let dez = pd[2] - pe[2];

        let [ab, bc, cd, da, ac, bd] = self.adapt_minors(aex, aey, bex, bey, cex, cey, dex, dey);
        let (ab3, bc3, cd3, da3, ac3, bd3) = (ab[3], bc[3], cd[3], da[3], ac[3], bd[3]);

        temp24len = self.adapt_three_term(&cd, bez, &bd, -cez, &bc, dez, &mut temp24);
        let alen = self.insphereadapt_lift(
            temp24len, &temp24, aex, -aex, aey, -aey, aez, -aez, &mut adet,
        );

        temp24len = self.adapt_three_term(&da, cez, &ac, dez, &cd, aez, &mut temp24);
        let blen =
            self.insphereadapt_lift(temp24len, &temp24, bex, bex, bey, bey, bez, bez, &mut bdet);

        temp24len = self.adapt_three_term(&ab, dez, &bd, aez, &da, bez, &mut temp24);
        let clen = self.insphereadapt_lift(
            temp24len, &temp24, cex, -cex, cey, -cey, cez, -cez, &mut cdet,
        );

        temp24len = self.adapt_three_term(&bc, aez, &ac, -bez, &ab, cez, &mut temp24);
        let dlen =
            self.insphereadapt_lift(temp24len, &temp24, dex, dex, dey, dey, dez, dez, &mut ddet);

        let ablen = fast_expansion_sum_zeroelim(alen, &adet, blen, &bdet, &mut abdet);
        let cdlen = fast_expansion_sum_zeroelim(clen, &cdet, dlen, &ddet, &mut cddet);
        let finlength = fast_expansion_sum_zeroelim(ablen, &abdet, cdlen, &cddet, &mut fin1);

        det = estimate(finlength, &fin1);
        errbound = self.isperrbound_b * permanent;
        if (det >= errbound) || (-det >= errbound) {
            return det;
        }

        let aextail = two_diff_tail(pa[0], pe[0], aex);
        let aeytail = two_diff_tail(pa[1], pe[1], aey);
        let aeztail = two_diff_tail(pa[2], pe[2], aez);
        let bextail = two_diff_tail(pb[0], pe[0], bex);
        let beytail = two_diff_tail(pb[1], pe[1], bey);
        let beztail = two_diff_tail(pb[2], pe[2], bez);
        let cextail = two_diff_tail(pc[0], pe[0], cex);
        let ceytail = two_diff_tail(pc[1], pe[1], cey);
        let ceztail = two_diff_tail(pc[2], pe[2], cez);
        let dextail = two_diff_tail(pd[0], pe[0], dex);
        let deytail = two_diff_tail(pd[1], pe[1], dey);
        let deztail = two_diff_tail(pd[2], pe[2], dez);
        if (aextail == 0.0)
            && (aeytail == 0.0)
            && (aeztail == 0.0)
            && (bextail == 0.0)
            && (beytail == 0.0)
            && (beztail == 0.0)
            && (cextail == 0.0)
            && (ceytail == 0.0)
            && (ceztail == 0.0)
            && (dextail == 0.0)
            && (deytail == 0.0)
            && (deztail == 0.0)
        {
            return det;
        }

        errbound = self.isperrbound_c * permanent + self.resulterrbound * det.abs();
        let abeps = (aex * beytail + bey * aextail) - (aey * bextail + bex * aeytail);
        let bceps = (bex * ceytail + cey * bextail) - (bey * cextail + cex * beytail);
        let cdeps = (cex * deytail + dey * cextail) - (cey * dextail + dex * ceytail);
        let daeps = (dex * aeytail + aey * dextail) - (dey * aextail + aex * deytail);
        let aceps = (aex * ceytail + cey * aextail) - (aey * cextail + cex * aeytail);
        let bdeps = (bex * deytail + dey * bextail) - (bey * dextail + dex * beytail);
        det += (((bex * bex + bey * bey + bez * bez)
            * ((cez * daeps + dez * aceps + aez * cdeps)
                + (ceztail * da3 + deztail * ac3 + aeztail * cd3))
            + (dex * dex + dey * dey + dez * dez)
                * ((aez * bceps - bez * aceps + cez * abeps)
                    + (aeztail * bc3 - beztail * ac3 + ceztail * ab3)))
            - ((aex * aex + aey * aey + aez * aez)
                * ((bez * cdeps - cez * bdeps + dez * bceps)
                    + (beztail * cd3 - ceztail * bd3 + deztail * bc3))
                + (cex * cex + cey * cey + cez * cez)
                    * ((dez * abeps + aez * bdeps + bez * daeps)
                        + (deztail * ab3 + aeztail * bd3 + beztail * da3))))
            + 2.0
                * (((bex * bextail + bey * beytail + bez * beztail)
                    * (cez * da3 + dez * ac3 + aez * cd3)
                    + (dex * dextail + dey * deytail + dez * deztail)
                        * (aez * bc3 - bez * ac3 + cez * ab3))
                    - ((aex * aextail + aey * aeytail + aez * aeztail)
                        * (bez * cd3 - cez * bd3 + dez * bc3)
                        + (cex * cextail + cey * ceytail + cez * ceztail)
                            * (dez * ab3 + aez * bd3 + bez * da3)));
        if (det >= errbound) || (-det >= errbound) {
            return det;
        }

        self.insphereexact(pa, pb, pc, pd, pe)
    }

    /// `insphere()` (the non-CGAL branch): adaptive exact 3D insphere test.
    /// Robust.
    ///
    /// Returns a positive value if the point `pe` lies inside the sphere
    /// passing through `pa`, `pb`, `pc` and `pd`; a negative value if it lies
    /// outside; and zero if the five points are cospherical. The points `pa`,
    /// `pb`, `pc` and `pd` must be ordered so that they have a positive
    /// orientation (as defined by `orient3d()`), or the sign of the result
    /// will be reversed.
    ///
    /// TetGen's order: with `use_inexact_arith` the floating-point determinant
    /// is returned; else with `use_static_filter` it is returned when
    /// `fabs(det) > ispstaticfilter` (here `fabs`, unlike `orient3d`); else
    /// Shewchuk's stage-A bound, then [`Predicates::insphereadapt`].
    pub fn insphere(
        &self,
        pa: &[f64; 3],
        pb: &[f64; 3],
        pc: &[f64; 3],
        pd: &[f64; 3],
        pe: &[f64; 3],
    ) -> f64 {
        let aex = pa[0] - pe[0];
        let bex = pb[0] - pe[0];
        let cex = pc[0] - pe[0];
        let dex = pd[0] - pe[0];
        let aey = pa[1] - pe[1];
        let bey = pb[1] - pe[1];
        let cey = pc[1] - pe[1];
        let dey = pd[1] - pe[1];
        let aez = pa[2] - pe[2];
        let bez = pb[2] - pe[2];
        let cez = pc[2] - pe[2];
        let dez = pd[2] - pe[2];

        let aexbey = aex * bey;
        let bexaey = bex * aey;
        let ab = aexbey - bexaey;
        let bexcey = bex * cey;
        let cexbey = cex * bey;
        let bc = bexcey - cexbey;
        let cexdey = cex * dey;
        let dexcey = dex * cey;
        let cd = cexdey - dexcey;
        let dexaey = dex * aey;
        let aexdey = aex * dey;
        let da = dexaey - aexdey;

        let aexcey = aex * cey;
        let cexaey = cex * aey;
        let ac = aexcey - cexaey;
        let bexdey = bex * dey;
        let dexbey = dex * bey;
        let bd = bexdey - dexbey;

        let abc = aez * bc - bez * ac + cez * ab;
        let bcd = bez * cd - cez * bd + dez * bc;
        let cda = cez * da + dez * ac + aez * cd;
        let dab = dez * ab + aez * bd + bez * da;

        let alift = aex * aex + aey * aey + aez * aez;
        let blift = bex * bex + bey * bey + bez * bez;
        let clift = cex * cex + cey * cey + cez * cez;
        let dlift = dex * dex + dey * dey + dez * dez;

        let det = (dlift * abc - clift * dab) + (blift * cda - alift * bcd);

        if self.use_inexact_arith {
            return det;
        }

        if self.use_static_filter && det.abs() > self.ispstaticfilter {
            return det;
            //if (det > ispstaticfilter) return det;
            //if (det < minus_ispstaticfilter) return det;
        }

        let aezplus = aez.abs();
        let bezplus = bez.abs();
        let cezplus = cez.abs();
        let dezplus = dez.abs();
        let aexbeyplus = aexbey.abs();
        let bexaeyplus = bexaey.abs();
        let bexceyplus = bexcey.abs();
        let cexbeyplus = cexbey.abs();
        let cexdeyplus = cexdey.abs();
        let dexceyplus = dexcey.abs();
        let dexaeyplus = dexaey.abs();
        let aexdeyplus = aexdey.abs();
        let aexceyplus = aexcey.abs();
        let cexaeyplus = cexaey.abs();
        let bexdeyplus = bexdey.abs();
        let dexbeyplus = dexbey.abs();
        let permanent = ((cexdeyplus + dexceyplus) * bezplus
            + (dexbeyplus + bexdeyplus) * cezplus
            + (bexceyplus + cexbeyplus) * dezplus)
            * alift
            + ((dexaeyplus + aexdeyplus) * cezplus
                + (aexceyplus + cexaeyplus) * dezplus
                + (cexdeyplus + dexceyplus) * aezplus)
                * blift
            + ((aexbeyplus + bexaeyplus) * dezplus
                + (bexdeyplus + dexbeyplus) * aezplus
                + (dexaeyplus + aexdeyplus) * bezplus)
                * clift
            + ((bexceyplus + cexbeyplus) * aezplus
                + (cexaeyplus + aexceyplus) * bezplus
                + (aexbeyplus + bexaeyplus) * cezplus)
                * dlift;
        let errbound = self.isperrbound_a * permanent;
        if (det > errbound) || (-det > errbound) {
            return det;
        }

        self.insphereadapt(pa, pb, pc, pd, pe, permanent)
    }

    // -----------------------------------------------------------------------
    // orient4d
    // -----------------------------------------------------------------------

    /// `orient4dexact()`: exact orient4d on the lifted points
    /// `(p, height)`. Robust.
    #[allow(clippy::too_many_arguments)]
    pub fn orient4dexact(
        &self,
        pa: &[f64; 3],
        pb: &[f64; 3],
        pc: &[f64; 3],
        pd: &[f64; 3],
        pe: &[f64; 3],
        aheight: f64,
        bheight: f64,
        cheight: f64,
        dheight: f64,
        eheight: f64,
    ) -> f64 {
        let (q, l) = self.five_point_quads(pa, pb, pc, pd, pe);
        let heights = [aheight, bheight, cheight, dheight, eheight];
        let mut dets = [[0.0f64; 192]; 5];
        let mut lens = [0usize; 5];
        // adet = bcde * aheight, bdet = cdea * bheight, cdet = deab * cheight,
        // ddet = eabc * dheight, edet = abcd * eheight.
        for k in 0..5 {
            lens[k] = self.scale_expansion_zeroelim(l[k], &q[k], heights[k], &mut dets[k]);
        }

        let mut abdet = [0.0f64; 384];
        let mut cddet = [0.0f64; 384];
        let mut cdedet = [0.0f64; 576];
        let mut deter = [0.0f64; 960];
        let ablen = fast_expansion_sum_zeroelim(lens[0], &dets[0], lens[1], &dets[1], &mut abdet);
        let cdlen = fast_expansion_sum_zeroelim(lens[2], &dets[2], lens[3], &dets[3], &mut cddet);
        let cdelen = fast_expansion_sum_zeroelim(cdlen, &cddet, lens[4], &dets[4], &mut cdedet);
        let deterlen = fast_expansion_sum_zeroelim(ablen, &abdet, cdelen, &cdedet, &mut deter);

        deter[deterlen - 1]
    }

    /// `orient4dadapt()`: the adaptive stages B, C and the exact fallback of
    /// [`Predicates::orient4d`]. Upstream uses the insphere error bounds
    /// (`isperrboundB`, `isperrboundC`) here, and so does the port.
    #[allow(clippy::too_many_arguments)]
    pub fn orient4dadapt(
        &self,
        pa: &[f64; 3],
        pb: &[f64; 3],
        pc: &[f64; 3],
        pd: &[f64; 3],
        pe: &[f64; 3],
        aheight: f64,
        bheight: f64,
        cheight: f64,
        dheight: f64,
        eheight: f64,
        permanent: f64,
    ) -> f64 {
        let mut det: f64;
        let mut errbound: f64;
        let mut temp24 = [0.0f64; 24];
        let mut temp24len: usize;
        let mut adet = [0.0f64; 48];
        let mut bdet = [0.0f64; 48];
        let mut cdet = [0.0f64; 48];
        let mut ddet = [0.0f64; 48];
        let mut abdet = [0.0f64; 96];
        let mut cddet = [0.0f64; 96];
        let mut fin1 = [0.0f64; 192];

        let aex = pa[0] - pe[0];
        let bex = pb[0] - pe[0];
        let cex = pc[0] - pe[0];
        let dex = pd[0] - pe[0];
        let aey = pa[1] - pe[1];
        let bey = pb[1] - pe[1];
        let cey = pc[1] - pe[1];
        let dey = pd[1] - pe[1];
        let aez = pa[2] - pe[2];
        let bez = pb[2] - pe[2];
        let cez = pc[2] - pe[2];
        let dez = pd[2] - pe[2];
        let aeheight = aheight - eheight;
        let beheight = bheight - eheight;
        let ceheight = cheight - eheight;
        let deheight = dheight - eheight;

        let [ab, bc, cd, da, ac, bd] = self.adapt_minors(aex, aey, bex, bey, cex, cey, dex, dey);
        let (ab3, bc3, cd3, da3, ac3, bd3) = (ab[3], bc[3], cd[3], da[3], ac[3], bd[3]);

        temp24len = self.adapt_three_term(&cd, bez, &bd, -cez, &bc, dez, &mut temp24);
        let alen = self.scale_expansion_zeroelim(temp24len, &temp24, -aeheight, &mut adet);

        temp24len = self.adapt_three_term(&da, cez, &ac, dez, &cd, aez, &mut temp24);
        let blen = self.scale_expansion_zeroelim(temp24len, &temp24, beheight, &mut bdet);

        temp24len = self.adapt_three_term(&ab, dez, &bd, aez, &da, bez, &mut temp24);
        let clen = self.scale_expansion_zeroelim(temp24len, &temp24, -ceheight, &mut cdet);

        temp24len = self.adapt_three_term(&bc, aez, &ac, -bez, &ab, cez, &mut temp24);
        let dlen = self.scale_expansion_zeroelim(temp24len, &temp24, deheight, &mut ddet);

        let ablen = fast_expansion_sum_zeroelim(alen, &adet, blen, &bdet, &mut abdet);
        let cdlen = fast_expansion_sum_zeroelim(clen, &cdet, dlen, &ddet, &mut cddet);
        let finlength = fast_expansion_sum_zeroelim(ablen, &abdet, cdlen, &cddet, &mut fin1);

        det = estimate(finlength, &fin1);
        errbound = self.isperrbound_b * permanent;
        if (det >= errbound) || (-det >= errbound) {
            return det;
        }

        let aextail = two_diff_tail(pa[0], pe[0], aex);
        let aeytail = two_diff_tail(pa[1], pe[1], aey);
        let aeztail = two_diff_tail(pa[2], pe[2], aez);
        let aeheighttail = two_diff_tail(aheight, eheight, aeheight);
        let bextail = two_diff_tail(pb[0], pe[0], bex);
        let beytail = two_diff_tail(pb[1], pe[1], bey);
        let beztail = two_diff_tail(pb[2], pe[2], bez);
        let beheighttail = two_diff_tail(bheight, eheight, beheight);
        let cextail = two_diff_tail(pc[0], pe[0], cex);
        let ceytail = two_diff_tail(pc[1], pe[1], cey);
        let ceztail = two_diff_tail(pc[2], pe[2], cez);
        let ceheighttail = two_diff_tail(cheight, eheight, ceheight);
        let dextail = two_diff_tail(pd[0], pe[0], dex);
        let deytail = two_diff_tail(pd[1], pe[1], dey);
        let deztail = two_diff_tail(pd[2], pe[2], dez);
        let deheighttail = two_diff_tail(dheight, eheight, deheight);
        if (aextail == 0.0)
            && (aeytail == 0.0)
            && (aeztail == 0.0)
            && (bextail == 0.0)
            && (beytail == 0.0)
            && (beztail == 0.0)
            && (cextail == 0.0)
            && (ceytail == 0.0)
            && (ceztail == 0.0)
            && (dextail == 0.0)
            && (deytail == 0.0)
            && (deztail == 0.0)
            && (aeheighttail == 0.0)
            && (beheighttail == 0.0)
            && (ceheighttail == 0.0)
            && (deheighttail == 0.0)
        {
            return det;
        }

        errbound = self.isperrbound_c * permanent + self.resulterrbound * det.abs();
        let abeps = (aex * beytail + bey * aextail) - (aey * bextail + bex * aeytail);
        let bceps = (bex * ceytail + cey * bextail) - (bey * cextail + cex * beytail);
        let cdeps = (cex * deytail + dey * cextail) - (cey * dextail + dex * ceytail);
        let daeps = (dex * aeytail + aey * dextail) - (dey * aextail + aex * deytail);
        let aceps = (aex * ceytail + cey * aextail) - (aey * cextail + cex * aeytail);
        let bdeps = (bex * deytail + dey * bextail) - (bey * dextail + dex * beytail);
        det += ((beheight
            * ((cez * daeps + dez * aceps + aez * cdeps)
                + (ceztail * da3 + deztail * ac3 + aeztail * cd3))
            + deheight
                * ((aez * bceps - bez * aceps + cez * abeps)
                    + (aeztail * bc3 - beztail * ac3 + ceztail * ab3)))
            - (aeheight
                * ((bez * cdeps - cez * bdeps + dez * bceps)
                    + (beztail * cd3 - ceztail * bd3 + deztail * bc3))
                + ceheight
                    * ((dez * abeps + aez * bdeps + bez * daeps)
                        + (deztail * ab3 + aeztail * bd3 + beztail * da3))))
            + ((beheighttail * (cez * da3 + dez * ac3 + aez * cd3)
                + deheighttail * (aez * bc3 - bez * ac3 + cez * ab3))
                - (aeheighttail * (bez * cd3 - cez * bd3 + dez * bc3)
                    + ceheighttail * (dez * ab3 + aez * bd3 + bez * da3)));
        if (det >= errbound) || (-det >= errbound) {
            return det;
        }

        self.orient4dexact(
            pa, pb, pc, pd, pe, aheight, bheight, cheight, dheight, eheight,
        )
    }

    /// `orient4d()`: returns a positive value if the point `pe` lies above
    /// the hyperplane passing through `pa`, `pb`, `pc` and `pd` (each lifted
    /// by its height); "above" is defined in a manner best found by
    /// trial-and-error. Returns a negative value if `pe` lies below the
    /// hyperplane. Returns zero if the points are co-hyperplanar (not affinely
    /// independent). The result is also a rough approximation of 24 times the
    /// signed volume of the 4-simplex defined by the five points.
    ///
    /// Upstream applies neither the static filter nor the `-X` option here:
    /// only Shewchuk's stage-A bound, then [`Predicates::orient4dadapt`].
    #[allow(clippy::too_many_arguments)]
    pub fn orient4d(
        &self,
        pa: &[f64; 3],
        pb: &[f64; 3],
        pc: &[f64; 3],
        pd: &[f64; 3],
        pe: &[f64; 3],
        aheight: f64,
        bheight: f64,
        cheight: f64,
        dheight: f64,
        eheight: f64,
    ) -> f64 {
        let aex = pa[0] - pe[0];
        let bex = pb[0] - pe[0];
        let cex = pc[0] - pe[0];
        let dex = pd[0] - pe[0];
        let aey = pa[1] - pe[1];
        let bey = pb[1] - pe[1];
        let cey = pc[1] - pe[1];
        let dey = pd[1] - pe[1];
        let aez = pa[2] - pe[2];
        let bez = pb[2] - pe[2];
        let cez = pc[2] - pe[2];
        let dez = pd[2] - pe[2];
        let aeheight = aheight - eheight;
        let beheight = bheight - eheight;
        let ceheight = cheight - eheight;
        let deheight = dheight - eheight;

        let aexbey = aex * bey;
        let bexaey = bex * aey;
        let ab = aexbey - bexaey;
        let bexcey = bex * cey;
        let cexbey = cex * bey;
        let bc = bexcey - cexbey;
        let cexdey = cex * dey;
        let dexcey = dex * cey;
        let cd = cexdey - dexcey;
        let dexaey = dex * aey;
        let aexdey = aex * dey;
        let da = dexaey - aexdey;

        let aexcey = aex * cey;
        let cexaey = cex * aey;
        let ac = aexcey - cexaey;
        let bexdey = bex * dey;
        let dexbey = dex * bey;
        let bd = bexdey - dexbey;

        let abc = aez * bc - bez * ac + cez * ab;
        let bcd = bez * cd - cez * bd + dez * bc;
        let cda = cez * da + dez * ac + aez * cd;
        let dab = dez * ab + aez * bd + bez * da;

        let det = (deheight * abc - ceheight * dab) + (beheight * cda - aeheight * bcd);

        let aezplus = aez.abs();
        let bezplus = bez.abs();
        let cezplus = cez.abs();
        let dezplus = dez.abs();
        let aexbeyplus = aexbey.abs();
        let bexaeyplus = bexaey.abs();
        let bexceyplus = bexcey.abs();
        let cexbeyplus = cexbey.abs();
        let cexdeyplus = cexdey.abs();
        let dexceyplus = dexcey.abs();
        let dexaeyplus = dexaey.abs();
        let aexdeyplus = aexdey.abs();
        let aexceyplus = aexcey.abs();
        let cexaeyplus = cexaey.abs();
        let bexdeyplus = bexdey.abs();
        let dexbeyplus = dexbey.abs();
        let permanent = ((cexdeyplus + dexceyplus) * bezplus
            + (dexbeyplus + bexdeyplus) * cezplus
            + (bexceyplus + cexbeyplus) * dezplus)
            * aeheight.abs()
            + ((dexaeyplus + aexdeyplus) * cezplus
                + (aexceyplus + cexaeyplus) * dezplus
                + (cexdeyplus + dexceyplus) * aezplus)
                * beheight.abs()
            + ((aexbeyplus + bexaeyplus) * dezplus
                + (bexdeyplus + dexbeyplus) * aezplus
                + (dexaeyplus + aexdeyplus) * bezplus)
                * ceheight.abs()
            + ((bexceyplus + cexbeyplus) * aezplus
                + (cexaeyplus + aexceyplus) * bezplus
                + (aexbeyplus + bexaeyplus) * cezplus)
                * deheight.abs();
        let errbound = self.isperrbound_a * permanent;
        if (det > errbound) || (-det > errbound) {
            return det;
        }

        self.orient4dadapt(
            pa, pb, pc, pd, pe, aheight, bheight, cheight, dheight, eheight, permanent,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `exactinit` gives IEEE binary64's epsilon 2^-53 and splitter 2^27 + 1,
    /// and sorts the extents before forming the static filters.
    #[test]
    fn exactinit_constants_are_ieee_double() {
        let p = Predicates::exactinit(false, false, 4.0, 1.0, 2.0);
        assert_eq!(p.epsilon, 2f64.powi(-53));
        assert_eq!(p.splitter, 134217729.0);
        assert!(p.use_static_filter && !p.use_inexact_arith);
        assert_eq!(p.o3dstaticfilter, 5.1107127829973299e-15 * 1.0 * 2.0 * 4.0);
        assert_eq!(
            p.ispstaticfilter,
            1.2466136531027298e-13 * 1.0 * 2.0 * 4.0 * (4.0 * 4.0)
        );
    }

    /// Signs on obvious configurations, through every public predicate, with
    /// the adaptive path forced (filter off) and the exact routines.
    #[test]
    fn predicate_signs_on_a_unit_tetrahedron() {
        let p = Predicates::exactinit(false, true, 1.0, 1.0, 1.0);
        let a = [0.0, 0.0, 0.0];
        let b = [1.0, 0.0, 0.0];
        let c = [0.0, 1.0, 0.0];
        let d = [0.0, 0.0, 1.0];
        // d above the ccw plane (a, b, c): negative.
        assert!(p.orient3d(&a, &b, &c, &d) < 0.0);
        assert_eq!(p.orient3d(&a, &b, &c, &d), p.orient3dexact(&a, &b, &c, &d));
        assert_eq!(p.orient3d(&a, &b, &c, &d), p.orient3dfast(&a, &b, &c, &d));
        assert_eq!(p.orient3d(&a, &b, &c, &[0.5, 0.5, 0.0]), 0.0);
        assert!(p.orient2dexact(&a, &b, &c) > 0.0);
        // Centre of the circumsphere of (b, a, c, d) is inside it.
        let e = [0.25, 0.25, 0.25];
        assert!(p.insphere(&b, &a, &c, &d, &e) > 0.0);
        assert!(p.insphereexact(&b, &a, &c, &d, &e) > 0.0);
        assert_eq!(p.insphere(&b, &a, &c, &d, &[1.0, 1.0, 1.0]), 0.0);
        let h = |q: &[f64; 3]| q[0] * q[0] + q[1] * q[1] + q[2] * q[2];
        let o = p.orient4d(&b, &a, &c, &d, &e, h(&b), h(&a), h(&c), h(&d), h(&e));
        assert_eq!(o, p.insphere(&b, &a, &c, &d, &e));
        let ox = p.orient4dexact(&b, &a, &c, &d, &e, h(&b), h(&a), h(&c), h(&d), h(&e));
        assert_eq!(o.signum(), ox.signum());
    }
}
