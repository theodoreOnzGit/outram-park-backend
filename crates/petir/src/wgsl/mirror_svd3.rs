// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 Theodore Ong and the outram-park contributors.
//
// TRANSCRIBED (f32) from PETIR's own port `crate::faer_dense::svd` and
// `crate::faer_dense::kernels`, which were PORTED from faer 0.24.4
// (MIT, Copyright (c) sarah quiñones and the faer contributors; the full
// notice is in `src/faer_dense/svd.rs`, retained here by reference as the
// licence requires). Upstream items: `Svd::new_imp`, `svd_imp`,
// `compute_bidiag_real_svd`, `bidiag_in_place`, `bidiag_fused_op_simd`,
// `qr_algorithm`, `apply_block_householder_sequence_on_the_left_in_place_with_conj`.

//! CPU mirror of `shaders/svd3.wgsl` ([`super::SVD3`]): faer's dense SVD of a
//! `3 x 3` matrix, and the closest proper rotation built from it, in `f32`.
//!
//! **What this is.** A line-for-line `f32` transcription of the shader, which
//! is itself a transcription of [`crate::faer_dense::svd3`] specialised to
//! `n = 3`: Householder bidiagonalisation, the implicit-shift bidiagonal QR
//! algorithm, the sign fix and sort, and the back-transformation. The GPU is
//! judged against this file (near-exact: a driver may contract `a * b + c`
//! into an FMA), and this file is judged against the `f64` port (the
//! precision cost).
//!
//! **Where it deliberately differs from the `f64` port**, all in summation
//! order or representation, never in the algorithm:
//!
//! - faer's SIMD reductions (`inner_prod_simd`, `norm_l2_contiguous`,
//!   `bidiag_fused_op_simd`, `matvec_colmajor`, `rotate_left`) accumulate in
//!   lanes and fold, with FMAs. A compute invocation is scalar, so here every
//!   reduction is a plain left-to-right `acc = x * y + acc` and every
//!   `mul_add(a, b, c)` is `a * b + c`. Same value in exact arithmetic.
//! - faer signals "no reflector" with `tau = +inf` and multiplies by
//!   `1 / tau = 0`. WGSL has no infinity literal and leaves overflow
//!   indeterminate, so the shader stores `1 / tau` directly (0 for the
//!   infinite case) and a flag for an infinite `1 / (head + beta)`. Every
//!   product is the same: `x * (1 / inf)` is `x * 0`.
//! - The QR iteration cap is faer's `max(30, nbits / 2) * n * n` with
//!   `nbits = 24` for `f32`, so 270; `eps` and the underflow threshold are the
//!   `f32` ones, as faer's generic code would use for `f32`.
//!
//! On failure (a non-finite bidiagonal, or the iteration cap) the `f64`
//! port returns an error and `outram-blender`'s `closest_rotation` falls back
//! to the identity; [`closest_rotation3`] does the same and [`svd3`] returns
//! `None`.

// `acc = x * y + acc` is written as the shader and faer write it; `+=` would
// read as the same thing but invites reordering the operands.
#![allow(clippy::assign_op_pattern)]

#[allow(unused_imports)]
use crate::real::Real;

/// A row-major `3 x 3` matrix, `m[i][j]` = entry `(i, j)`.
pub type Mat3f = [[f32; 3]; 3];

const N: usize = 3;
const MIN_POSITIVE: f32 = f32::MIN_POSITIVE;
const MAX_POSITIVE: f32 = 1.0 / f32::MIN_POSITIVE;

fn sqrt_min_positive() -> f32 {
    Real::sqrt(MIN_POSITIVE)
}

fn sqrt_max_positive() -> f32 {
    Real::sqrt(MAX_POSITIVE)
}

/// `a[i]` without a subscript (PETIR's no-panic gate): every index here is
/// in range by construction; an out-of-range one would read NaN, as
/// `mirror_matrix::mat_get` does.
#[inline]
fn ld(a: &[f32], i: usize) -> f32 {
    a.get(i).copied().unwrap_or(f32::NAN)
}

/// `a[i] = v` without a subscript (see [`ld`]).
#[inline]
fn st(a: &mut [f32], i: usize, v: f32) {
    if let Some(x) = a.get_mut(i) {
        *x = v;
    }
}

/// Column-major index of entry `(i, j)`.
#[inline]
fn ix(i: usize, j: usize) -> usize {
    j * N + i
}

/// `petir_svd3_hypot`: faer's three-branch scaled `hypot`.
fn hypot(re: f32, im: f32) -> f32 {
    let small = sqrt_min_positive();
    let big = sqrt_max_positive();
    let re_abs = f32::abs(re);
    let im_abs = f32::abs(im);
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

/// `petir_svd3_norm_select`: faer's `norm_l2_select`.
fn norm_select(acc_sml: f32, acc_med: f32, acc_big: f32) -> f32 {
    if acc_sml >= 1.0 {
        Real::sqrt(acc_sml) * sqrt_max_positive()
    } else if acc_med >= 1.0 {
        Real::sqrt(acc_med)
    } else {
        Real::sqrt(acc_big) * sqrt_min_positive()
    }
}

/// `petir_svd3_norm_contiguous`: faer's `norm_l2_contiguous` over
/// `x[0..len]`, scalar.
fn norm_contiguous(x: &[f32; 3], len: usize) -> f32 {
    let sml = sqrt_min_positive();
    let big = sqrt_max_positive();
    let mut acc_sml = 0.0_f32;
    let mut acc_med = 0.0_f32;
    let mut acc_big = 0.0_f32;
    for &v in x.iter().take(len) {
        let vs = v * sml;
        acc_sml = vs * vs + acc_sml;
        acc_med = v * v + acc_med;
        let vb = v * big;
        acc_big = vb * vb + acc_big;
    }
    norm_select(acc_sml, acc_med, acc_big)
}

/// `petir_svd3_norm_strided`: faer's `norm_l2_strided` over `x[0..len]`.
fn norm_strided(x: &[f32; 3], len: usize) -> f32 {
    if len == 0 {
        return norm_select(0.0, 0.0, 0.0);
    }
    let mut acc = 0.0_f32;
    for &v in x.iter().take(len) {
        acc = hypot(acc, f32::abs(v));
    }
    acc = acc * acc;
    norm_select(MIN_POSITIVE * acc, acc, MAX_POSITIVE * acc)
}

/// faer's `make_householder` on `head` and `tail[0..len]`. Returns
/// `(1 / tau, 1 / (head + beta), that_inverse_is_infinite)`; the infinite
/// case (`tail_norm < MIN_POSITIVE`) leaves `tail` untouched, as faer does.
fn make_householder(
    head: &mut f32,
    tail: &mut [f32; 3],
    len: usize,
    tail_norm: f32,
) -> (f32, f32, bool) {
    let mut head_norm = f32::abs(*head);
    if head_norm < MIN_POSITIVE {
        *head = 0.0;
        head_norm = 0.0;
    }
    if tail_norm < MIN_POSITIVE {
        return (0.0, 0.0, true);
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
    for e in tail.iter_mut().take(len) {
        *e *= head_with_beta_inv;
    }
    *head = -signed_norm;
    let t = tail_norm * f32::abs(head_with_beta_inv);
    let tau = 0.5 * (1.0 + t * t);
    (1.0 / tau, head_with_beta_inv, false)
}

/// faer's `make_givens`, returning `(c, s)`.
fn make_givens(p: f32, q: f32) -> (f32, f32) {
    if q == 0.0 {
        (if p < 0.0 { -1.0 } else { 1.0 }, 0.0)
    } else if p == 0.0 {
        (0.0, if q < 0.0 { 1.0 } else { -1.0 })
    } else if f32::abs(p) > f32::abs(q) {
        let t = q / p;
        let mut u = hypot(t, 1.0);
        if p < 0.0 {
            u = -u;
        }
        let c = 1.0 / u;
        (c, -t * c)
    } else {
        let t = p / q;
        let mut u = hypot(t, 1.0);
        if q < 0.0 {
            u = -u;
        }
        let s = -(1.0 / u);
        (-t * s, s)
    }
}

/// faer's `rotate_left` on columns `a` and `b` of the column-major `m`.
fn rotate_cols(m: &mut [f32; 9], a: usize, b: usize, c: f32, s: f32) {
    if c == 1.0 && s == 0.0 {
        return;
    }
    for r in 0..N {
        let xx = ld(m.as_slice(), ix(r, a));
        let yy = ld(m.as_slice(), ix(r, b));
        {
            let val = (-s) * yy + xx * c;
            st(m.as_mut_slice(), ix(r, a), val);
        }
        {
            let val = s * xx + yy * c;
            st(m.as_mut_slice(), ix(r, b), val);
        }
    }
}

/// faer's `apply_householder_left`: reflector with essential part
/// `ess[0..len]` and `1 / tau = tau_inv`, on rows `r0..` of every column of
/// the column-major `t`.
fn apply_householder_left(ess: &[f32; 3], len: usize, tau_inv: f32, t: &mut [f32; 9], r0: usize) {
    for c in 0..N {
        let mut dot = 0.0_f32;
        for (q, &e) in ess.iter().enumerate().take(len) {
            dot = e * ld(t.as_slice(), ix(r0 + 1 + q, c)) + dot;
        }
        let dot = ld(t.as_slice(), ix(r0, c)) + dot;
        let k = -dot * tau_inv;
        {
            let val = ld(t.as_slice(), ix(r0, c)) + (k);
            st(t.as_mut_slice(), ix(r0, c), val);
        }
        for (q, &e) in ess.iter().enumerate().take(len) {
            let at = ix(r0 + 1 + q, c);
            {
                let val = e * k + ld(t.as_slice(), at);
                st(t.as_mut_slice(), at, val);
            }
        }
    }
}

/// faer's SVD of the row-major `3 x 3` `m`, in `f32`: `Some((U, s, V))`,
/// row-major, `m = U diag(s) Vᵀ`, `s` non-increasing and non-negative.
/// `None` where the `f64` port errors (non-finite bidiagonal, or the QR
/// iteration cap).
///
/// ```
/// use petir::wgsl::mirror_svd3::svd3;
/// let (u, s, v) = svd3([[2.0, 0.0, 0.0], [0.0, -3.0, 0.0], [0.0, 0.0, 1.0]]).unwrap();
/// assert_eq!(s, [3.0, 2.0, 1.0]);
/// // m = U diag(s) Vᵀ, entry (1, 1):
/// let m11: f32 = (0..3).map(|k| u[1][k] * s[k] * v[1][k]).sum();
/// assert!((m11 + 3.0).abs() < 1e-6);
/// ```
pub fn svd3(m: Mat3f) -> Option<(Mat3f, [f32; 3], Mat3f)> {
    svd3_counted(m).map(|(u, s, v, _)| (u, s, v))
}

/// [`svd3`], also returning the number of QR sweeps (`qr_algorithm`
/// iterations) it ran: the one data-dependent operation count, which the
/// derived error budget in `tests/wgsl_validation.rs` needs. Diagnostic only;
/// the arithmetic is [`svd3`]'s.
pub fn svd3_counted(m: Mat3f) -> Option<(Mat3f, [f32; 3], Mat3f, u32)> {
    // Column-major working copy.
    let mut a = [0.0_f32; 9];
    for (i, row) in m.iter().enumerate() {
        for (j, &x) in row.iter().enumerate() {
            {
                let val = x;
                st(a.as_mut_slice(), ix(i, j), val);
            }
        }
    }

    // ---- bidiag_in_place (block size 1) --------------------------------
    let mut y = [0.0_f32; 3];
    let mut z = [0.0_f32; 3];
    let mut hl_inv = [0.0_f32; 3];
    let mut hr_inv = [0.0_f32; 2];
    for k in 0..N {
        let y1 = ld(y.as_slice(), k);
        let z1 = ld(z.as_slice(), k);
        if k >= 1 {
            let k1 = k - 1;
            let up0 = ld(a.as_slice(), ix(k, k1));
            {
                let val = ld(a.as_slice(), ix(k, k)) - (up0 * y1 + z1);
                st(a.as_mut_slice(), ix(k, k), val);
            }
            for i in k + 1..N {
                {
                    let val = ld(a.as_slice(), ix(i, k))
                        - (ld(a.as_slice(), ix(i, k1)) * y1 + ld(z.as_slice(), i));
                    st(a.as_mut_slice(), ix(i, k), val);
                }
            }
            for j in k + 1..N {
                {
                    let val = ld(a.as_slice(), ix(k, j))
                        - (up0 * ld(y.as_slice(), j) + z1 * ld(a.as_slice(), ix(k1, j)));
                    st(a.as_mut_slice(), ix(k, j), val);
                }
            }
        }

        // Left reflector on column k, tail rows k+1..
        let len = N - k - 1;
        let mut tail = [0.0_f32; 3];
        for q in 0..len {
            {
                let val = ld(a.as_slice(), ix(k + 1 + q, k));
                st(tail.as_mut_slice(), q, val);
            }
        }
        let tail_norm = norm_contiguous(&tail, len);
        let mut head = ld(a.as_slice(), ix(k, k));
        let (tl_inv, _, _) = make_householder(&mut head, &mut tail, len, tail_norm);
        {
            let val = head;
            st(a.as_mut_slice(), ix(k, k), val);
        }
        for q in 0..len {
            {
                let val = ld(tail.as_slice(), q);
                st(a.as_mut_slice(), ix(k + 1 + q, k), val);
            }
        }
        {
            let val = tl_inv;
            st(hl_inv.as_mut_slice(), k, val);
        }

        if k >= 1 {
            // bidiag_fused_op on A22 = A[k+1.., k+1..].
            let k1 = k - 1;
            for j in k + 1..N {
                let yj = -ld(y.as_slice(), j);
                let vj = -ld(a.as_slice(), ix(k1, j));
                let mut acc = 0.0_f32;
                for i in k + 1..N {
                    let mut v = ld(a.as_slice(), ix(i, j));
                    v = ld(a.as_slice(), ix(i, k1)) * yj + v;
                    v = ld(z.as_slice(), i) * vj + v;
                    {
                        let val = v;
                        st(a.as_mut_slice(), ix(i, j), val);
                    }
                    acc = ld(a.as_slice(), ix(i, k)) * v + acc;
                }
                {
                    let val = acc;
                    st(y.as_mut_slice(), j, val);
                }
            }
        } else {
            for j in k + 1..N {
                let mut d = 0.0_f32;
                for i in k + 1..N {
                    d = ld(a.as_slice(), ix(i, j)) * ld(a.as_slice(), ix(i, k)) + d;
                }
                {
                    let val = 1.0 * d;
                    st(y.as_mut_slice(), j, val);
                }
            }
        }

        // y2 = (y2 + A12) / tl; A12 -= y2.
        let mut a12 = [0.0_f32; 3];
        let len12 = N - k - 1;
        for q in 0..len12 {
            let j = k + 1 + q;
            {
                let val = (ld(y.as_slice(), j) + ld(a.as_slice(), ix(k, j))) * tl_inv;
                st(y.as_mut_slice(), j, val);
            }
            {
                let val = ld(a.as_slice(), ix(k, j)) - ld(y.as_slice(), j);
                st(a12.as_mut_slice(), q, val);
            }
        }
        let norm = norm_strided(&a12, len12);
        let norm_inv = 1.0 / norm;
        if norm != 0.0 {
            for x in a12.iter_mut().take(len12) {
                *x *= norm_inv;
            }
        }
        for q in 0..len12 {
            {
                let val = ld(a12.as_slice(), q);
                st(a.as_mut_slice(), ix(k, k + 1 + q), val);
            }
        }

        // z2 = A22 A12ᴴ (column-major matvec: zero, then column by column).
        for i in k + 1..N {
            {
                let val = 0.0;
                st(z.as_mut_slice(), i, val);
            }
        }
        for q in 0..len12 {
            let r = ld(a12.as_slice(), q) * 1.0;
            for i in k + 1..N {
                {
                    let val = ld(a.as_slice(), ix(i, k + 1 + q)) * r + ld(z.as_slice(), i);
                    st(z.as_mut_slice(), i, val);
                }
            }
        }

        if k + 1 == N {
            break;
        }

        // Right reflector on A12 (head a12[0], tail a12[1..]).
        let mut a12_a = ld(a12.as_slice(), 0);
        let mut a12_b = [0.0_f32; 3];
        let lenb = len12 - 1;
        for q in 0..lenb {
            {
                let val = ld(a12.as_slice(), 1 + q);
                st(a12_b.as_mut_slice(), q, val);
            }
        }
        let tail_norm = norm_strided(&a12_b, lenb);
        let (tr_inv, m_inv, m_infinite) = make_householder(&mut a12_a, &mut a12_b, lenb, tail_norm);
        {
            let val = tr_inv;
            st(hr_inv.as_mut_slice(), k, val);
        }
        let beta = a12_a;
        a12_a *= norm;
        {
            let val = a12_a;
            st(a.as_mut_slice(), ix(k, k + 1), val);
        }
        for q in 0..lenb {
            {
                let val = ld(a12_b.as_slice(), q);
                st(a.as_mut_slice(), ix(k, k + 2 + q), val);
            }
        }

        // b = y2_a + <y2_b, A12_b> (schoolbook).
        let mut sb = 0.0_f32;
        for q in 0..lenb {
            sb = ld(y.as_slice(), k + 2 + q) * ld(a12_b.as_slice(), q) + sb;
        }
        let b = ld(y.as_slice(), k + 1) + sb;

        for i in k + 1..N {
            let ui = ld(a.as_slice(), ix(i, k));
            let ai = ld(a.as_slice(), ix(i, k + 1));
            if !m_infinite {
                let w = ld(z.as_slice(), i) - ai * beta;
                let w = w * m_inv;
                let w = w - ui * b;
                {
                    let val = w * tr_inv;
                    st(z.as_mut_slice(), i, val);
                }
            } else {
                let w = ai - ui * b;
                {
                    let val = w * tr_inv;
                    st(z.as_mut_slice(), i, val);
                }
            }
        }
    }

    // ---- svd_imp: read off the bidiagonal ------------------------------
    let mut diag = [
        ld(a.as_slice(), ix(0, 0)),
        ld(a.as_slice(), ix(1, 1)),
        ld(a.as_slice(), ix(2, 2)),
    ];
    let mut sub = [ld(a.as_slice(), ix(0, 1)), ld(a.as_slice(), ix(1, 2)), 0.0];
    for i in 0..N {
        if !(ld(diag.as_slice(), i).is_finite() && ld(sub.as_slice(), i).is_finite()) {
            return None;
        }
    }
    let mut ub = [1.0_f32, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0];
    let mut vb = ub;
    let sweeps = qr_algorithm(&mut diag, &mut sub, &mut ub, &mut vb)?;

    // U = left reflectors (2, 1, 0) applied to vb.
    let mut u = vb;
    for j in (0..N).rev() {
        let len = N - j - 1;
        if len == 0 {
            continue;
        }
        let mut ess = [0.0_f32; 3];
        for q in 0..len {
            {
                let val = ld(a.as_slice(), ix(j + 1 + q, j));
                st(ess.as_mut_slice(), q, val);
            }
        }
        apply_householder_left(&ess, len, ld(hl_inv.as_slice(), j), &mut u, j);
    }
    // V = right reflectors (1, 0) applied to rows 1.. of ub, after mirroring
    // the upper triangle into the lower.
    for j in 1..N {
        for i in 0..j {
            {
                let val = ld(a.as_slice(), ix(i, j));
                st(a.as_mut_slice(), ix(j, i), val);
            }
        }
    }
    let mut v = ub;
    for j in (0..N - 1).rev() {
        let len = N - (1 + j) - 1;
        if len == 0 {
            continue;
        }
        let mut ess = [0.0_f32; 3];
        for q in 0..len {
            {
                let val = ld(a.as_slice(), ix(1 + j + 1 + q, j));
                st(ess.as_mut_slice(), q, val);
            }
        }
        apply_householder_left(&ess, len, ld(hr_inv.as_slice(), j), &mut v, 1 + j);
    }

    let rows = |cm: &[f32; 9]| {
        let mut out = [[0.0_f32; 3]; 3];
        for (i, row) in out.iter_mut().enumerate() {
            for (j, x) in row.iter_mut().enumerate() {
                *x = ld(cm.as_slice(), ix(i, j));
            }
        }
        out
    };
    Some((rows(&u), diag, rows(&v), sweeps))
}

/// faer's `qr_algorithm` for `n = 3`, `f32`. Returns the sweep count, or
/// `None` where faer returns `NoConvergence` (the iteration cap).
fn qr_algorithm(
    diag: &mut [f32; 3],
    sub: &mut [f32; 3],
    u: &mut [f32; 9],
    v: &mut [f32; 9],
) -> Option<u32> {
    let eps = f32::EPSILON;
    let sml = MIN_POSITIVE;
    // max(30, 24 / 2) * 3 * 3.
    let max_iters = 270_usize;
    let last = N - 1;
    let mut max = 0.0_f32;
    for &x in diag.iter() {
        if f32::abs(x) > max {
            max = f32::abs(x);
        }
    }
    let mut max_sub = 0.0_f32;
    for &x in sub.iter() {
        if f32::abs(x) > max_sub {
            max_sub = f32::abs(x);
        }
    }
    if max_sub > max {
        max = max_sub;
    }
    if max == 0.0 {
        return Some(0);
    }
    let max_inv = 1.0 / max;
    for i in 0..N {
        {
            let val = ld(diag.as_slice(), i) * (max_inv);
            st(diag.as_mut_slice(), i, val);
        }
        {
            let val = ld(sub.as_slice(), i) * (max_inv);
            st(sub.as_mut_slice(), i, val);
        }
    }
    let eps2 = eps * eps;
    let mut converged = None;
    for iter in 0..max_iters {
        for i0 in 0..last {
            let i1 = i0 + 1;
            let e = ld(sub.as_slice(), i0);
            if e * e <= eps2 * f32::abs(ld(diag.as_slice(), i0) * ld(diag.as_slice(), i1)) + sml {
                {
                    let val = 0.0;
                    st(sub.as_mut_slice(), i0, val);
                }
            }
        }
        let mut end = N;
        while end >= 2 && ld(sub.as_slice(), end - 2) * ld(sub.as_slice(), end - 2) <= sml {
            end -= 1;
        }
        if end == 1 {
            converged = Some(iter as u32);
            break;
        }
        let mut start = end - 1;
        while start >= 1 && ld(sub.as_slice(), start - 1) != 0.0 {
            start -= 1;
        }
        let mut found_zero_diag = false;
        for i in start..end - 1 {
            if f32::abs(ld(diag.as_slice(), i)) <= eps {
                found_zero_diag = true;
                let mut val = ld(sub.as_slice(), i);
                {
                    let val = 0.0;
                    st(sub.as_mut_slice(), i, val);
                }
                for j in i + 1..end {
                    let (c, s) = make_givens(ld(diag.as_slice(), j), val);
                    {
                        let val = c * ld(diag.as_slice(), j) - s * val;
                        st(diag.as_mut_slice(), j, val);
                    }
                    if j + 1 < end {
                        val = s * ld(sub.as_slice(), j);
                        {
                            let val = c * ld(sub.as_slice(), j);
                            st(sub.as_mut_slice(), j, val);
                        }
                    }
                    rotate_cols(v, j, i, c, s);
                }
            }
        }
        if found_zero_diag {
            if iter + 1 == max_iters {
                return None;
            }
            continue;
        }
        let end2 = end - 2;
        let end1 = end - 1;
        let d2 = ld(diag.as_slice(), end2);
        let t00 = if end - start == 2 {
            d2 * d2
        } else {
            let e3 = ld(sub.as_slice(), end - 3);
            d2 * d2 + e3 * e3
        };
        let d1 = ld(diag.as_slice(), end1);
        let e2 = ld(sub.as_slice(), end2);
        let t11 = d1 * d1 + e2 * e2;
        let t01 = d2 * e2;
        let t01_2 = t01 * t01;
        let mu = if t01_2 > sml {
            let d = (t00 - t11) * 0.5;
            let mut delta = Real::sqrt(d * d + t01_2);
            if d < 0.0 {
                delta = -delta;
            }
            t11 - t01_2 / (d + delta)
        } else {
            t11
        };
        let ds = ld(diag.as_slice(), start);
        let mut yy = ds * ds - mu;
        let mut zz = ds * ld(sub.as_slice(), start);
        for k in start..end1 {
            let (c, s) = make_givens(yy, zz);
            if k > start {
                {
                    let val = f32::abs(c * yy - s * zz);
                    st(sub.as_mut_slice(), k - 1, val);
                }
            }
            let ek = ld(sub.as_slice(), k);
            let diag_k = c * ld(diag.as_slice(), k) - s * ek;
            {
                let val = s * ld(diag.as_slice(), k) + c * ek;
                st(sub.as_mut_slice(), k, val);
            }
            let k1 = k + 1;
            yy = diag_k;
            let dk1 = ld(diag.as_slice(), k1);
            zz = -s * dk1;
            {
                let val = c * dk1;
                st(diag.as_mut_slice(), k1, val);
            }
            rotate_cols(u, k1, k, c, -s);
            let (c, s) = make_givens(yy, zz);
            {
                let val = c * yy - s * zz;
                st(diag.as_mut_slice(), k, val);
            }
            let ek = ld(sub.as_slice(), k);
            let dk1 = ld(diag.as_slice(), k1);
            {
                let val = c * ek - s * dk1;
                st(sub.as_mut_slice(), k, val);
            }
            {
                let val = s * ek + c * dk1;
                st(diag.as_mut_slice(), k1, val);
            }
            if k < end - 2 {
                yy = ld(sub.as_slice(), k);
                let ek1 = ld(sub.as_slice(), k1);
                zz = -s * ek1;
                {
                    let val = c * ek1;
                    st(sub.as_mut_slice(), k1, val);
                }
            }
            rotate_cols(v, k1, k, c, -s);
        }
        if iter + 1 == max_iters {
            return None;
        }
    }
    let sweeps = converged?;
    for j in 0..N {
        if ld(diag.as_slice(), j) < 0.0 {
            {
                let val = -ld(diag.as_slice(), j);
                st(diag.as_mut_slice(), j, val);
            }
            for r in 0..N {
                {
                    let val = -ld(v.as_slice(), ix(r, j));
                    st(v.as_mut_slice(), ix(r, j), val);
                }
            }
        }
    }
    for k in 0..N {
        let mut mx = 0.0_f32;
        let mut idx = k;
        for kk in k..N {
            if ld(diag.as_slice(), kk) > mx {
                mx = ld(diag.as_slice(), kk);
                idx = kk;
            }
        }
        if k != idx {
            diag.swap(k, idx);
            for r in 0..N {
                u.swap(ix(r, k), ix(r, idx));
                v.swap(ix(r, k), ix(r, idx));
            }
        }
    }
    for x in diag.iter_mut() {
        *x *= max;
    }
    Some(sweeps)
}

/// `R = U Vᵀ` for row-major `U`, `V`, as `petir_svd3_mul_abt` computes it.
fn mul_abt(a: &Mat3f, b: &Mat3f) -> Mat3f {
    let mut c = [[0.0_f32; 3]; 3];
    for (row, [a0, a1, a2]) in c.iter_mut().zip(a) {
        for (x, [b0, b1, b2]) in row.iter_mut().zip(b) {
            *x = a0 * b0 + a1 * b1 + a2 * b2;
        }
    }
    c
}

/// The closest proper rotation to the row-major `m` (orthogonal Procrustes /
/// Kabsch), in `f32`: `R = U Vᵀ` from [`svd3`], with `U`'s last column
/// negated when `det(U Vᵀ) < 0`. This is `outram-blender`'s
/// `arap::closest_rotation` in `f32`, including its identity fallback when
/// the SVD fails.
///
/// ```
/// use petir::wgsl::mirror_svd3::closest_rotation3;
/// // A reflection's closest proper rotation is not itself.
/// let r = closest_rotation3([[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, -1.0]]);
/// let det = r[0][0] * (r[1][1] * r[2][2] - r[1][2] * r[2][1])
///     - r[0][1] * (r[1][0] * r[2][2] - r[1][2] * r[2][0])
///     + r[0][2] * (r[1][0] * r[2][1] - r[1][1] * r[2][0]);
/// assert!((det - 1.0).abs() < 1e-6);
/// ```
pub fn closest_rotation3(m: Mat3f) -> Mat3f {
    let Some((mut u, _s, v)) = svd3(m) else {
        return [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
    };
    let [[r00, r01, r02], [r10, r11, r12], [r20, r21, r22]] = mul_abt(&u, &v);
    let det = r00 * (r11 * r22 - r12 * r21) - r01 * (r10 * r22 - r12 * r20)
        + r02 * (r10 * r21 - r11 * r20);
    if det < 0.0 {
        for [_, _, last] in u.iter_mut() {
            *last = -*last;
        }
    }
    mul_abt(&u, &v)
}

/// The rotation angle (radians) between two rotations given row-major in
/// `f64`: `theta = 2 asin(||A - B||_F / (2 sqrt 2))`, exact for two
/// rotations and well conditioned at small angles (unlike
/// `acos((tr(A^T B) - 1) / 2)`). This is how an `f32` rotation is compared
/// with an `f64` one: as one angle, not entry by entry.
pub fn rotation_angle(a: &[[f64; 3]; 3], b: &[[f64; 3]; 3]) -> f64 {
    let mut f2 = 0.0_f64;
    for (x, y) in a.iter().flatten().zip(b.iter().flatten()) {
        let d = x - y;
        f2 += d * d;
    }
    let x = Real::sqrt(f2) / (2.0 * Real::sqrt(2.0_f64));
    2.0 * Real::asin(if x > 1.0 { 1.0 } else { x })
}

/// Unit roundoff of `f32`, `2^-24`.
pub const U32: f64 = 5.960_464_477_539_063e-8;

/// Roundings charged per Householder reflector in the bidiagonalisation
/// (first order, every rounding at its bound, no cancellation). For a
/// reflector of length `m <= 3`: the tail norm (`2m + 2`: squares, sums,
/// `sqrt`), `hypot` and the sign (4), `1 / (head + beta)` and the tail
/// scaling (2), `tau` and its inverse (4); applying it, a length-`m` dot
/// product (`2m`), `-dot / tau` (1) and the update (2). With `m = 3` that is
/// 8 + 4 + 2 + 4 + 6 + 1 + 2 = 27; faer's fused one-sided updates
/// (`y`, `z`: one multiply-add chain of length 3 per entry, 6 more) give 33.
pub const ROUNDINGS_PER_REFLECTOR: f64 = 33.0;

/// Reflectors in a `3 x 3` bidiagonalisation: three left and two right
/// (faer forms all five; the last of each side is trivial, charged anyway).
pub const REFLECTORS: f64 = 5.0;

/// Roundings charged per QR sweep. A sweep over a block of order `b <= 3`
/// chases `b - 1 <= 2` bulges; each step makes two Givens rotations
/// (`make_givens`: a division, `hypot` (4), a reciprocal and a product: 7
/// each) and applies each to the bidiagonal and to one singular-vector
/// matrix (`c x - s y`: 3 roundings, twice per target). Per step:
/// 2 x (7 + 2 x 3 + 2 x 3) = 38; two steps: 76. The shift `mu` adds 8
/// (squares, the `sqrt`, a division); 84 in all.
pub const ROUNDINGS_PER_SWEEP: f64 = 84.0;

/// The derived first-order budget for the rotation-angle error of
/// [`closest_rotation3`] against the exact closest rotation of the same
/// matrix, given the QR sweep count [`svd3_counted`] reports.
///
/// - `n_ops = REFLECTORS * ROUNDINGS_PER_REFLECTOR + sweeps *
///   ROUNDINGS_PER_SWEEP`; the computed SVD is then the exact SVD of
///   `S + dS`, `||dS||_F <= (1 + n_ops) u ||S||_F` (the 1 is rounding the
///   `f64` input to `f32`), with `U`, `V` orthogonal to `n_ops u` each.
/// - The closest proper rotation moves by at most
///   `2 ||dS||_F / (sigma_2 + sign(det S) sigma_3)` under a perturbation
///   `dS` (the polar factor's condition number, with the smallest singular
///   value sign-flipped for the special-orthogonal problem), and the loss of
///   orthogonality of `U` and `V`, and the `3 u` of `U V^T`, add directly.
///
/// So `theta <= 2 n_ops u + 3 u + 2 (1 + n_ops) u ||S||_F / gap`, with
/// `gap = sigma_2 + sign(det S) sigma_3` from the `f64` SVD. `gap <= 0`
/// means the closest rotation is not unique; the budget is then infinite and
/// such a case is reported, not judged.
pub fn angle_budget(sweeps: u32, s_frobenius: f64, gap: f64) -> f64 {
    let n_ops = REFLECTORS * ROUNDINGS_PER_REFLECTOR + f64::from(sweeps) * ROUNDINGS_PER_SWEEP;
    if gap <= 0.0 {
        return f64::INFINITY;
    }
    2.0 * n_ops * U32 + 3.0 * U32 + 2.0 * (1.0 + n_ops) * U32 * s_frobenius / gap
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The `f32` transcription agrees with the `f64` port to `f32` rounding
    /// on a general matrix: same singular values, and `U diag(s) Vᵀ`
    /// reconstructs the input. (The full, budgeted comparison is in
    /// `tests/wgsl_validation.rs`.)
    #[test]
    fn matches_the_f64_port_on_a_general_matrix() {
        let m64 = [[2.0, -1.0, 0.5], [0.3, 4.0, -2.0], [1.5, 0.7, -3.0]];
        let (_, s64, _) = crate::faer_dense::svd3(m64).unwrap();
        let m32 = m64.map(|r| r.map(|x| x as f32));
        let (u, s, v) = svd3(m32).unwrap();
        for k in 0..3 {
            assert!(
                f64::abs(s[k] as f64 - s64[k]) < 1e-5 * s64[0],
                "{s:?} vs {s64:?}"
            );
        }
        for i in 0..3 {
            for j in 0..3 {
                let rec: f32 = (0..3).map(|k| u[i][k] * s[k] * v[j][k]).sum();
                assert!(f32::abs(rec - m32[i][j]) < 1e-5 * s[0]);
            }
        }
    }

    /// The zero matrix and a non-finite input take faer's early exits:
    /// identity vectors, and the identity rotation on failure.
    #[test]
    fn degenerate_inputs_take_faers_exits() {
        let (u, s, v) = svd3([[0.0; 3]; 3]).unwrap();
        assert_eq!(s, [0.0; 3]);
        assert_eq!(u, [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]);
        assert_eq!(v, u);
        assert!(svd3([[f32::NAN; 3]; 3]).is_none());
        assert_eq!(closest_rotation3([[f32::NAN; 3]; 3]), u);
    }
}
