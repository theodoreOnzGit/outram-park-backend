// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 Theodore Ong and the outram-park contributors.
//
// PORTED from the `faer` crate, version 0.24.4, read from the published
// crates.io source (faer-0.24.4 in the cargo registry) on 2026-10-20.
//
//     https://codeberg.org/sarah-quinones/faer
//     Copyright (c) sarah quiñones <sarah@veganb.tw> and the faer contributors
//     Licensed MIT (faer's Cargo.toml `license = "MIT"`; the published crate
//     ships no LICENSE file, so the standard MIT text is reproduced here).
//
// Its notice, retained as that licence requires of a redistribution in source
// form:
//
//   Permission is hereby granted, free of charge, to any person obtaining a
//   copy of this software and associated documentation files (the
//   "Software"), to deal in the Software without restriction, including
//   without limitation the rights to use, copy, modify, merge, publish,
//   distribute, sublicense, and/or sell copies of the Software, and to permit
//   persons to whom the Software is furnished to do so, subject to the
//   following conditions:
//
//   The above copyright notice and this permission notice shall be included in
//   all copies or substantial portions of the Software.
//
//   THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
//   IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
//   FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL
//   THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
//   LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING
//   FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER
//   DEALINGS IN THE SOFTWARE.
//
// MIT into AGPL-3.0-only is a ONE-WAY flow: this file may not be contributed
// back to `faer` under its original licence without its authors' agreement.
// Maintainer direction for this port: gh #996, 2026-10-20 ("Whatever faer
// does, port into petir"). See the crate NOTICE and CLAUDE.md.
//
// Ported items (faer 0.24.4 unless noted):
//   faer-traits 0.24.0 src/lib.rs:337    abs_impl (faer's `hypot`)
//   src/linalg/matmul/mod.rs:692         dot::inner_prod_no_conj_simd /
//                                        inner_prod_conj_lhs_simd
//   src/linalg/matmul/mod.rs:727         dot::inner_prod_schoolbook
//   src/linalg/matmul/mod.rs:880         matvec_colmajor::matvec (Seq arm)
//   src/linalg/reductions/norm_l2.rs     norm_l2_simd, norm_l2_x3, norm_l2
//   src/linalg/reductions/norm_max.rs    norm_max (finite input)
//   src/linalg/householder.rs:59         make_householder_imp
//   src/linalg/householder.rs:370        apply_block_householder_on_the_left_
//                                        in_place_generic, the N == 1 SIMD arm
//                                        (ApplyOnLeft)
//   src/linalg/jacobi.rs                 JacobiRotation::{make_givens,
//                                        apply_on_the_left_in_place_with_simd}

//! The faer building blocks the dense SVD calls, one function per faer
//! routine, each keeping faer's operation order. Every vector kernel goes
//! through [`super::simd`] so that lanes and accumulators match faer's.

use alloc::vec::Vec;

#[allow(unused_imports)]
use crate::real::Real;
use crate::{PetirError, Result};

use super::simd::{self, Lanes, SimdCtx, ZERO};

/// `f64::MIN_POSITIVE` (faer `min_positive`).
pub(crate) const MIN_POSITIVE: f64 = f64::MIN_POSITIVE;

/// faer's `max_positive` for `f64`: `MIN_POSITIVE.recip()` (2^1022), NOT
/// `f64::MAX` (`faer-traits/src/lib.rs`, `RealField for f64`).
pub(crate) const MAX_POSITIVE: f64 = 1.0 / f64::MIN_POSITIVE;

/// faer's `sqrt_min_positive`: `sqrt(MIN_POSITIVE)`.
pub(crate) fn sqrt_min_positive() -> f64 {
    Real::sqrt(MIN_POSITIVE)
}

/// faer's `sqrt_max_positive`: `sqrt(MIN_POSITIVE.recip())`.
pub(crate) fn sqrt_max_positive() -> f64 {
    Real::sqrt(MAX_POSITIVE)
}

/// Read `s[i]`, or `Invalid` if out of range (no subscript may panic here).
pub(crate) fn at(s: &[f64], i: usize) -> Result<f64> {
    s.get(i).copied().ok_or(PetirError::Invalid)
}

/// A mutable reference to `s[i]`, or `Invalid` if out of range.
pub(crate) fn at_mut(s: &mut [f64], i: usize) -> Result<&mut f64> {
    s.get_mut(i).ok_or(PetirError::Invalid)
}

/// faer's real `hypot`, which is NOT `libm::hypot`: `math_utils::hypot` ->
/// `abs_impl(re, im)` (`faer-traits 0.24.0, src/lib.rs:337`), a three-way
/// scaled `sqrt(re^2 + im^2)`.
pub(crate) fn hypot(re: f64, im: f64) -> f64 {
    let small = sqrt_min_positive();
    let big = sqrt_max_positive();
    let re_abs = re.abs();
    let im_abs = im.abs();
    if re_abs > big || im_abs > big {
        let a = re * small;
        let b = im * small;
        Real::sqrt(a * a + b * b) * big
    } else if re_abs > 1.0 || im_abs > 1.0 {
        Real::sqrt(re * re + im * im)
    } else {
        let a = re * big;
        let b = im * big;
        Real::sqrt(a * a + b * b) * small
    }
}

/// `dot::inner_prod_no_conj_simd` / `inner_prod_conj_lhs_simd` (identical
/// for real data) over a contiguous pair, with `SimdCtx::new(len)`: four
/// accumulators fed by `batch_indices::<4>`, each lane a fused `l * r + acc`,
/// folded `(a0 + a1) + (a2 + a3)` and reduced.
pub(crate) fn inner_prod_simd(lhs: &[f64], rhs: &[f64]) -> Result<f64> {
    let ctx = SimdCtx::new(lhs.len());
    let mut acc = [ZERO; 4];
    for (idx, c) in ctx.batch_indices(4) {
        let l = simd::read(lhs, c)?;
        let r = simd::read(rhs, c)?;
        let a = acc.get_mut(idx).ok_or(PetirError::Invalid)?;
        *a = simd::mul_add(l, r, *a);
    }
    Ok(simd::fold4(acc))
}

/// `dot::inner_prod_schoolbook`, real arm: `acc = l * r + acc`, unfused, in
/// index order from `0`. faer takes it whenever either operand is strided.
pub(crate) fn inner_prod_schoolbook(lhs: &[f64], rhs: &[f64]) -> f64 {
    let mut acc = 0.0;
    for (l, r) in lhs.iter().zip(rhs) {
        acc = l * r + acc;
    }
    acc
}

/// `norm_l2` of a **contiguous** column (`norm_l2_simd` through
/// `norm_l2_simd_pairwise_rows`, which does not split at or below
/// `LINEAR_IMPL_THRESHOLD = 128`). Three accumulator pairs (scaled small,
/// unscaled, scaled big) fed by `batch_indices::<2>`, then the choice in
/// `norm_l2`.
pub(crate) fn norm_l2_contiguous(x: &[f64]) -> Result<f64> {
    if x.is_empty() {
        return Ok(norm_l2_select([0.0, 0.0, 0.0]));
    }
    if x.len() > 128 {
        // faer splits pairwise above this; not ported (the SVD port is
        // limited to n <= 16, see the module docs).
        return Err(PetirError::Invalid);
    }
    let ctx = SimdCtx::new(x.len());
    let sml = simd::splat(sqrt_min_positive());
    let big = simd::splat(sqrt_max_positive());
    let mut acc_sml = [ZERO; 2];
    let mut acc_med = [ZERO; 2];
    let mut acc_big = [ZERO; 2];
    for (idx, c) in ctx.batch_indices(2) {
        let v = simd::read(x, c)?;
        let s = acc_sml.get_mut(idx).ok_or(PetirError::Invalid)?;
        let vs = simd::mul(v, sml);
        *s = simd::mul_add(vs, vs, *s);
        let m = acc_med.get_mut(idx).ok_or(PetirError::Invalid)?;
        *m = simd::mul_add(v, v, *m);
        let b = acc_big.get_mut(idx).ok_or(PetirError::Invalid)?;
        let vb = simd::mul(v, big);
        *b = simd::mul_add(vb, vb, *b);
    }
    let fold2 = |a: [Lanes; 2]| {
        let [a0, a1] = a;
        simd::reduce_sum(simd::add(a0, a1))
    };
    Ok(norm_l2_select([
        fold2(acc_sml),
        fold2(acc_med),
        fold2(acc_big),
    ]))
}

/// `norm_l2` of a **strided** vector (a matrix row, or a column view with a
/// non-unit stride): `norm_l2_x3`'s non-SIMD fallback, a running faer
/// `hypot`, squared, returned as `[MIN_POSITIVE * acc, acc, MAX_POSITIVE *
/// acc]`, then the same choice.
pub(crate) fn norm_l2_strided(x: &[f64]) -> f64 {
    if x.is_empty() {
        return norm_l2_select([0.0, 0.0, 0.0]);
    }
    let mut acc = 0.0;
    for v in x {
        acc = hypot(acc, v.abs());
    }
    acc = acc * acc;
    norm_l2_select([MIN_POSITIVE * acc, acc, MAX_POSITIVE * acc])
}

/// The tail of faer's `norm_l2`: pick the first of the three scaled sums of
/// squares that is at least one.
fn norm_l2_select(x3: [f64; 3]) -> f64 {
    let [acc_sml, acc_med, acc_big] = x3;
    if acc_sml >= 1.0 {
        Real::sqrt(acc_sml) * sqrt_max_positive()
    } else if acc_med >= 1.0 {
        Real::sqrt(acc_med)
    } else {
        Real::sqrt(acc_big) * sqrt_min_positive()
    }
}

/// `norm_max` of a contiguous column of FINITE values. faer's SIMD
/// `max(acc, |x|)` starting from zero; on finite input the result is the
/// largest `|x|` whatever the lane order.
pub(crate) fn norm_max(x: &[f64]) -> f64 {
    let mut m = 0.0;
    for v in x {
        let a = v.abs();
        if a > m {
            m = a;
        }
    }
    m
}

/// `matvec_colmajor::matvec` (Seq arm) with `beta = Replace`, `alpha = 1`:
/// `dst = 0`, then for each column `j`, `dst = fma(lhs[:, j], rhs[j] * 1,
/// dst)` lane by lane. `cols` are the contiguous columns of `lhs`.
pub(crate) fn matvec_colmajor(dst: &mut [f64], cols: &[Vec<f64>], rhs: &[f64]) -> Result<()> {
    let ctx = SimdCtx::new(dst.len());
    let chunks = ctx.indices();
    for &c in &chunks {
        simd::write(dst, c, ZERO)?;
    }
    for (col, r) in cols.iter().zip(rhs) {
        let vr = simd::splat(r * 1.0);
        for &c in &chunks {
            let y = simd::read(dst, c)?;
            let x = simd::read(col, c)?;
            simd::write(dst, c, simd::mul_add(x, vr, y))?;
        }
    }
    Ok(())
}

/// What `make_householder_imp` returns: `tau` and `(head + beta)^-1`.
#[derive(Debug, Clone, Copy)]
pub(crate) struct HouseholderInfo {
    pub(crate) tau: f64,
    pub(crate) head_with_beta_inv: f64,
}

/// `make_householder_in_place` (`householder.rs:59`, real arm). `tail_norm`
/// is passed in because faer computes it with a different kernel for a
/// contiguous tail ([`norm_l2_contiguous`]) and a strided one
/// ([`norm_l2_strided`]).
pub(crate) fn make_householder(
    head: &mut f64,
    tail: &mut [f64],
    tail_norm: f64,
) -> HouseholderInfo {
    let mut head_norm = head.abs();
    if head_norm < MIN_POSITIVE {
        *head = 0.0;
        head_norm = 0.0;
    }
    if tail_norm < MIN_POSITIVE {
        return HouseholderInfo {
            tau: f64::INFINITY,
            head_with_beta_inv: f64::INFINITY,
        };
    }
    let norm = hypot(head_norm, tail_norm);
    let sign = if head_norm != 0.0 {
        *head * (1.0 / head_norm)
    } else {
        1.0
    };
    let signed_norm = sign * norm;
    let head_with_beta = *head + signed_norm;
    let head_with_beta_inv = 1.0 / head_with_beta;
    for e in tail.iter_mut() {
        *e *= head_with_beta_inv;
    }
    *head = -signed_norm;
    let t = tail_norm * head_with_beta_inv.abs();
    let tau = 0.5 * (1.0 + t * t);
    HouseholderInfo {
        tau,
        head_with_beta_inv,
    }
}

/// One block-size-1 reflection applied from the left
/// (`apply_block_householder_on_the_left_in_place_generic`, the `N == 1`
/// SIMD arm `ApplyOnLeft`, real). `essential` is the reflector below its
/// unit head; `rows` are the target's rows from the head down, stored as
/// columns `cols[k]` (head element first). For each target column:
/// `dot = col0 + <essential, col_tail>`, `k = -dot * (1 / tau)`,
/// `col0 += k`, `col_tail = fma(essential, k, col_tail)`.
pub(crate) fn apply_householder_left(
    essential: &[f64],
    tau: f64,
    cols: &mut [Vec<f64>],
) -> Result<()> {
    if essential.is_empty() {
        return Ok(());
    }
    let tau_inv = 1.0 / tau;
    let ctx = SimdCtx::new(essential.len());
    let chunks = ctx.indices();
    for col in cols.iter_mut() {
        let Some((col0, tail)) = col.split_first_mut() else {
            return Err(PetirError::Invalid);
        };
        let dot = *col0 + inner_prod_simd(essential, tail)?;
        let k = -dot * tau_inv;
        *col0 += k;
        let kv = simd::splat(k);
        for &c in &chunks {
            let a = simd::read(tail, c)?;
            let b = simd::read(essential, c)?;
            simd::write(tail, c, simd::mul_add(b, kv, a))?;
        }
    }
    Ok(())
}

/// faer's `JacobiRotation` (real).
#[derive(Debug, Clone, Copy)]
pub(crate) struct Givens {
    pub(crate) c: f64,
    pub(crate) s: f64,
}

/// `JacobiRotation::make_givens` (`jacobi.rs:16`), with faer's [`hypot`].
pub(crate) fn make_givens(p: f64, q: f64) -> Givens {
    if q == 0.0 {
        let c = if p < 0.0 { -1.0 } else { 1.0 };
        Givens { c, s: 0.0 }
    } else if p == 0.0 {
        let s = if q < 0.0 { 1.0 } else { -1.0 };
        Givens { c: 0.0, s }
    } else if p.abs() > q.abs() {
        let t = q / p;
        let mut u = hypot(t, 1.0);
        if p < 0.0 {
            u = -u;
        }
        let c = 1.0 / u;
        let s = -t * c;
        Givens { c, s }
    } else {
        let t = p / q;
        let mut u = hypot(t, 1.0);
        if q < 0.0 {
            u = -u;
        }
        let s = -(1.0 / u);
        let c = -t * s;
        Givens { c, s }
    }
}

/// `apply_on_the_left_in_place_with_simd` (`jacobi.rs:311`) for a rotation
/// `(c, s)` on two contiguous vectors: nothing when `c == 1 && s == 0`, else
/// lane by lane `x' = fma(-s, y, x * c)`, `y' = fma(s, x, y * c)`.
///
/// faer's `apply_on_the_right_in_place((x, y))` calls this with
/// `self.transpose()`, i.e. `s -> -s`; callers pass the effective `s`.
pub(crate) fn rotate_left(c: f64, s: f64, x: &mut [f64], y: &mut [f64]) -> Result<()> {
    if c == 1.0 && s == 0.0 {
        return Ok(());
    }
    let ctx = SimdCtx::new(x.len());
    let cv = simd::splat(c);
    let sv = simd::splat(s);
    let nsv = simd::splat(-s);
    for c in ctx.indices() {
        let xx = simd::read(x, c)?;
        let yy = simd::read(y, c)?;
        let nx = simd::mul_add(nsv, yy, simd::mul(xx, cv));
        let ny = simd::mul_add(sv, xx, simd::mul(yy, cv));
        simd::write(x, c, nx)?;
        simd::write(y, c, ny)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn faer_hypot_is_the_scaled_three_way_form() {
        // Middle branch: plain sqrt(a^2 + b^2).
        assert_eq!(hypot(3.0, 4.0), 5.0);
        // Large branch scales down by sqrt(MIN_POSITIVE) and back up.
        let h = hypot(1e300, 1e300);
        assert!((h / 1.414_213_562_373_095e300 - 1.0).abs() < 1e-15);
    }

    #[test]
    fn strided_and_contiguous_norms_agree_to_rounding() {
        let x = [3.0, 4.0, 12.0];
        assert_eq!(norm_l2_contiguous(&x).unwrap(), 13.0);
        assert!((norm_l2_strided(&x) - 13.0).abs() < 1e-14);
        assert_eq!(norm_l2_strided(&[]), 0.0);
    }
}
