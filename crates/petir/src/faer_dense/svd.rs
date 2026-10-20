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
// Ported items (faer 0.24.4):
//   src/linalg/solvers.rs:1339       Svd::new_imp (full U and V)
//   src/linalg/svd/mod.rs:530        svd (square arm)
//   src/linalg/svd/mod.rs:374        svd_imp
//   src/linalg/svd/mod.rs:331        compute_bidiag_real_svd (n < 128 arm)
//   src/linalg/svd/bidiag.rs:48      bidiag_in_place (Par::Seq) and
//                                    bidiag_fused_op_simd (line 344)
//   src/linalg/svd/bidiag_svd.rs:274 qr_algorithm
//   src/linalg/householder.rs:724    apply_block_householder_sequence_on_
//                                    the_left_in_place_with_conj (block 1)

//! faer's dense SVD for a small square `f64` matrix: Householder
//! bidiagonalisation, the implicit-shift bidiagonal QR algorithm, and the
//! back-transformation of the singular vectors.

use alloc::vec;
use alloc::vec::Vec;

#[allow(unused_imports)]
use crate::real::Real;
use crate::{PetirError, Result};

use super::kernels::{
    apply_householder_left, at, at_mut, inner_prod_schoolbook, inner_prod_simd, make_givens,
    make_householder, matvec_colmajor, norm_l2_contiguous, norm_l2_strided, norm_max, rotate_left,
    MIN_POSITIVE,
};
use super::simd::{self, simd_align, SimdCtx, ZERO};

/// Largest order handled. faer's Householder block size
/// (`qr::no_pivoting::factor::recommended_block_size`) is 1 while
/// `n * n <= 16 * 16`; above that faer switches to blocked reflectors applied
/// through its GEMM kernels, which this port does not carry.
pub const MAX_ORDER: usize = 16;

/// A row-major `3 x 3` matrix, `m[i][j]` = entry `(i, j)`, as [`svd3`] takes
/// and returns it.
pub type Mat3 = [[f64; 3]; 3];

/// A column-major square matrix, `data[j * n + i]` = entry `(i, j)`.
#[derive(Debug, Clone, PartialEq)]
struct Square {
    n: usize,
    data: Vec<f64>,
}

impl Square {
    fn zeros(n: usize) -> Self {
        Self {
            n,
            data: vec![0.0; n * n],
        }
    }
    fn identity(n: usize) -> Result<Self> {
        let mut m = Self::zeros(n);
        for i in 0..n {
            m.set(i, i, 1.0)?;
        }
        Ok(m)
    }
    fn get(&self, i: usize, j: usize) -> Result<f64> {
        at(&self.data, j * self.n + i)
    }
    fn set(&mut self, i: usize, j: usize, v: f64) -> Result<()> {
        *at_mut(&mut self.data, j * self.n + i)? = v;
        Ok(())
    }
    /// Rows `r0..` of column `j`, contiguous.
    fn col(&self, j: usize, r0: usize) -> Result<&[f64]> {
        self.data
            .get(j * self.n + r0..(j + 1) * self.n)
            .ok_or(PetirError::Invalid)
    }
    fn col_mut(&mut self, j: usize, r0: usize) -> Result<&mut [f64]> {
        let n = self.n;
        self.data
            .get_mut(j * n + r0..(j + 1) * n)
            .ok_or(PetirError::Invalid)
    }
    /// Columns `c0..` of row `i` (a strided view in faer), copied.
    fn row(&self, i: usize, c0: usize) -> Result<Vec<f64>> {
        (c0..self.n).map(|j| self.get(i, j)).collect()
    }
    fn set_row(&mut self, i: usize, c0: usize, v: &[f64]) -> Result<()> {
        for (j, x) in (c0..self.n).zip(v) {
            self.set(i, j, *x)?;
        }
        Ok(())
    }
    /// Two distinct columns, mutably.
    fn two_cols(&mut self, a: usize, b: usize) -> Result<(&mut [f64], &mut [f64])> {
        let n = self.n;
        if a == b {
            return Err(PetirError::Invalid);
        }
        let (lo, hi, swapped) = if a < b { (a, b, false) } else { (b, a, true) };
        let (left, right) = self.data.split_at_mut(hi * n);
        let x = left
            .get_mut(lo * n..(lo + 1) * n)
            .ok_or(PetirError::Invalid)?;
        let y = right.get_mut(..n).ok_or(PetirError::Invalid)?;
        Ok(if swapped { (y, x) } else { (x, y) })
    }
    fn swap_cols(&mut self, a: usize, b: usize) -> Result<()> {
        let (x, y) = self.two_cols(a, b)?;
        x.swap_with_slice(y);
        Ok(())
    }
}

/// faer's SVD of a square matrix, `A = U diag(s) Vᵀ`.
#[derive(Debug, Clone, PartialEq)]
pub struct FaerSvd {
    /// Order `n`.
    pub n: usize,
    /// `U`, column-major `n x n`.
    pub u: Vec<f64>,
    /// Singular values, non-increasing.
    pub s: Vec<f64>,
    /// `V`, column-major `n x n`.
    pub v: Vec<f64>,
}

/// faer's `Mat::svd()` (`Svd::new`) for a square `n x n` matrix, `1 <= n <=`
/// [`MAX_ORDER`], given **column-major**.
///
/// # Errors
///
/// - [`PetirError::Invalid`] if `n` is 0 or above [`MAX_ORDER`], or `a` is not
///   `n * n` long;
/// - [`PetirError::Domain`] where faer returns `SvdError::NoConvergence`
///   because the bidiagonal form holds a non-finite value (a NaN or infinite
///   input);
/// - [`PetirError::MaxIterations`] where faer returns
///   `SvdError::NoConvergence` after `max(30, 26) * n * n` QR sweeps.
pub fn svd_square(a: &[f64], n: usize) -> Result<FaerSvd> {
    if n == 0 || n > MAX_ORDER {
        return Err(PetirError::Invalid);
    }
    if a.len() != n * n {
        return Err(PetirError::LengthMismatch {
            expected: n * n,
            found: a.len(),
        });
    }
    let mut bid = Square {
        n,
        data: a.to_vec(),
    };
    let (hl, hr) = bidiag_in_place(&mut bid)?;

    // svd_imp: read off the bidiagonal.
    let mut diag = Vec::with_capacity(n);
    let mut subdiag = Vec::with_capacity(n);
    for i in 0..n {
        diag.push(bid.get(i, i)?);
        subdiag.push(if i + 1 < n { bid.get(i, i + 1)? } else { 0.0 });
    }

    // compute_bidiag_real_svd: non-finite -> NoConvergence; then, n < 128,
    // identity starts and the QR algorithm. faer's `u` here (`ub`) becomes V,
    // and its `v` (`vb`) becomes U.
    if diag
        .iter()
        .zip(&subdiag)
        .any(|(d, e)| !(d.is_finite() && e.is_finite()))
    {
        return Err(PetirError::Domain);
    }
    let mut ub = Square::identity(n)?;
    let mut vb = Square::identity(n)?;
    qr_algorithm(&mut diag, &mut subdiag, &mut ub, &mut vb)?;

    // U = H_left sequence applied to vb.
    let mut u = vb;
    apply_sequence_left(&bid, 0, &hl, &mut u, 0)?;

    // V = H_right sequence applied to rows 1.. of ub, after mirroring the
    // upper triangle of `bid` (the right reflectors) into the lower one.
    let mut v = ub;
    for j in 1..n {
        for i in 0..j {
            let x = bid.get(i, j)?;
            bid.set(j, i, x)?;
        }
    }
    apply_sequence_left(&bid, 1, &hr, &mut v, 1)?;

    Ok(FaerSvd {
        n,
        u: u.data,
        s: diag,
        v: v.data,
    })
}

/// faer's `Svd::new` on a `3 x 3` matrix given **row-major** (`m[i][j]`),
/// returning `(U, s, V)` row-major. This is the call
/// `outram-blender`'s `arap::closest_rotation` makes.
///
/// # Errors
///
/// As [`svd_square`].
pub fn svd3(m: Mat3) -> Result<(Mat3, [f64; 3], Mat3)> {
    let mut a = [0.0; 9];
    for (i, row) in m.iter().enumerate() {
        for (j, x) in row.iter().enumerate() {
            *a.get_mut(j * 3 + i).ok_or(PetirError::Invalid)? = *x;
        }
    }
    let r = svd_square(&a, 3)?;
    let to_rows = |cm: &[f64]| -> Result<[[f64; 3]; 3]> {
        let mut out = [[0.0; 3]; 3];
        for (i, row) in out.iter_mut().enumerate() {
            for (j, x) in row.iter_mut().enumerate() {
                *x = at(cm, j * 3 + i)?;
            }
        }
        Ok(out)
    };
    let [s0, s1, s2] = <[f64; 3]>::try_from(r.s.as_slice()).map_err(|_| PetirError::Invalid)?;
    Ok((to_rows(&r.u)?, [s0, s1, s2], to_rows(&r.v)?))
}

/// `bidiag_in_place` (`svd/bidiag.rs:48`) for a square matrix with block
/// size 1, `Par::Seq`. Returns the left and right Householder factors
/// (`H_left[0, k]`, `H_right[0, k]`).
fn bidiag_in_place(a: &mut Square) -> Result<(Vec<f64>, Vec<f64>)> {
    let n = a.n;
    let mut y = vec![0.0; n];
    let mut z = vec![0.0; n];
    let mut hl = vec![0.0; n];
    let mut hr = vec![0.0; n.saturating_sub(1)];

    for k in 0..n {
        let y1 = at(&y, k)?;
        let z1 = at(&z, k)?;
        if let Some(k1) = k.checked_sub(1) {
            // a11 -= up0 * y1 + z1; A21 -= up * y1 + z2; A12 -= up0 * y2 + z1 * vp.
            let up0 = a.get(k, k1)?;
            let a11 = a.get(k, k)?;
            a.set(k, k, a11 - (up0 * y1 + z1))?;
            for i in k + 1..n {
                let u = a.get(i, k1)?;
                let zi = at(&z, i)?;
                let v = a.get(i, k)?;
                a.set(i, k, v - (u * y1 + zi))?;
            }
            for j in k + 1..n {
                let yj = at(&y, j)?;
                let vp = a.get(k1, j)?;
                let v = a.get(k, j)?;
                a.set(k, j, v - (up0 * yj + z1 * vp))?;
            }
        }

        // Left reflector on column k (contiguous tail).
        let tl = {
            let tail_norm = norm_l2_contiguous(a.col(k, k + 1)?)?;
            let mut head = a.get(k, k)?;
            let info = make_householder(&mut head, a.col_mut(k, k + 1)?, tail_norm);
            a.set(k, k, head)?;
            info.tau
        };
        let tl_inv = 1.0 / tl;
        *at_mut(&mut hl, k)? = tl;

        if let Some(k1) = k.checked_sub(1) {
            bidiag_fused_op(a, k, k1, &z, &mut y)?;
        } else {
            // y2 = A21ᴴ A22: matvec_rowmajor, one SIMD dot per column of A22.
            let u = a.col(k, k + 1)?.to_vec();
            for j in k + 1..n {
                let d = inner_prod_simd(a.col(j, k + 1)?, &u)?;
                *at_mut(&mut y, j)? = 1.0 * d;
            }
        }

        // y2 = (y2 + A12) / tl; A12 -= y2.
        let mut a12 = a.row(k, k + 1)?;
        for (j, aj) in (k + 1..n).zip(a12.iter_mut()) {
            let yj = at_mut(&mut y, j)?;
            *yj = (*yj + *aj) * tl_inv;
            *aj -= *yj;
        }
        let norm = norm_l2_strided(&a12);
        let norm_inv = 1.0 / norm;
        if norm != 0.0 {
            for x in a12.iter_mut() {
                *x *= norm_inv;
            }
        }
        a.set_row(k, k + 1, &a12)?;

        // z2 = A22 A12ᴴ: matvec_colmajor.
        {
            let cols: Vec<Vec<f64>> = (k + 1..n)
                .map(|j| a.col(j, k + 1).map(<[f64]>::to_vec))
                .collect::<Result<_>>()?;
            let z2 = z.get_mut(k + 1..).ok_or(PetirError::Invalid)?;
            matvec_colmajor(z2, &cols, &a12)?;
        }

        if k + 1 == n {
            break;
        }

        // Right reflector on A12 (strided tail A12_b).
        let mut a12_a = at(&a12, 0)?;
        let mut a12_b = a12.get(1..).ok_or(PetirError::Invalid)?.to_vec();
        let tail_norm = norm_l2_strided(&a12_b);
        let info = make_householder(&mut a12_a, &mut a12_b, tail_norm);
        let tr = info.tau;
        let m = info.head_with_beta_inv;
        let tr_inv = 1.0 / tr;
        *at_mut(&mut hr, k)? = tr;
        let beta = a12_a;
        a12_a *= norm;
        a.set(k, k + 1, a12_a)?;
        a.set_row(k, k + 2, &a12_b)?;

        // b = y2_a + <y2_b, A12_b>: rhs strided, so faer's schoolbook dot.
        let y2_a = at(&y, k + 1)?;
        let y2_b = y.get(k + 2..).ok_or(PetirError::Invalid)?;
        let b = y2_a + inner_prod_schoolbook(y2_b, &a12_b);

        let u = a.col(k, k + 1)?.to_vec();
        let a22_a = a.col(k + 1, k + 1)?.to_vec();
        let z2 = z.get_mut(k + 1..).ok_or(PetirError::Invalid)?;
        if m != f64::INFINITY {
            for ((zi, ui), ai) in z2.iter_mut().zip(&u).zip(&a22_a) {
                let w = *zi - ai * beta;
                let w = w * m;
                let w = w - ui * b;
                *zi = w * tr_inv;
            }
        } else {
            for ((zi, ui), ai) in z2.iter_mut().zip(&u).zip(&a22_a) {
                let w = ai - ui * b;
                *zi = w * tr_inv;
            }
        }
    }
    Ok((hl, hr))
}

/// `bidiag_fused_op_simd` (`svd/bidiag.rs:344`) on `A22 = A[k+1.., k+1..]`
/// with `u = A[k+1.., k]`, `up = A[k+1.., k-1]`, `z = z[k+1..]`,
/// `vp = A[k-1, k+1..]`, and `SimdCtx::new_align(m, simd_align(k + 1))`.
/// Per column `j`: `a = fma(up, -y[j], a)`, `a = fma(z, -vp[j], a)`, store,
/// accumulate `fma(u, a, acc)`; `y[j]` = the folded sum.
fn bidiag_fused_op(a: &mut Square, k: usize, k1: usize, z: &[f64], y: &mut [f64]) -> Result<()> {
    let n = a.n;
    let u = a.col(k, k + 1)?.to_vec();
    let up = a.col(k1, k + 1)?.to_vec();
    let z2 = z.get(k + 1..).ok_or(PetirError::Invalid)?;
    let ctx = SimdCtx::new_align(u.len(), simd_align(k + 1));
    let chunks = ctx.batch_indices(4);
    for j in k + 1..n {
        let yj = simd::splat(-at(y, j)?);
        let vj = simd::splat(-a.get(k1, j)?);
        let col = a.col_mut(j, k + 1)?;
        let mut acc = [ZERO; 4];
        for &(idx, c) in &chunks {
            let mut v = simd::read(col, c)?;
            v = simd::mul_add(simd::read(&up, c)?, yj, v);
            v = simd::mul_add(simd::read(z2, c)?, vj, v);
            simd::write(col, c, v)?;
            let slot = acc.get_mut(idx).ok_or(PetirError::Invalid)?;
            *slot = simd::mul_add(simd::read(&u, c)?, v, *slot);
        }
        *at_mut(y, j)? = simd::fold4(acc);
    }
    Ok(())
}

/// `apply_block_householder_sequence_on_the_left_in_place_with_conj` with
/// block size 1: reflectors `taus.len() - 1` down to `0`, reflector `j`
/// having essential part `basis[r0 + j + 1.., j]` and acting on rows
/// `t0 + j..` of `target`.
fn apply_sequence_left(
    basis: &Square,
    r0: usize,
    taus: &[f64],
    target: &mut Square,
    t0: usize,
) -> Result<()> {
    let n = target.n;
    for (j, &tau) in taus.iter().enumerate().rev() {
        let essential = basis.col(j, r0 + j + 1)?.to_vec();
        if essential.is_empty() {
            continue;
        }
        // Rows t0 + j.. of every column, head first.
        let mut cols: Vec<Vec<f64>> = (0..n)
            .map(|c| target.col(c, t0 + j).map(<[f64]>::to_vec))
            .collect::<Result<_>>()?;
        apply_householder_left(&essential, tau, &mut cols)?;
        for (c, col) in cols.iter().enumerate() {
            target.col_mut(c, t0 + j)?.copy_from_slice(col);
        }
    }
    Ok(())
}

/// `qr_algorithm` (`svd/bidiag_svd.rs:274`): implicit zero-shift-deflating
/// bidiagonal QR on `(diag, subdiag)`, accumulating right rotations into
/// `u` and left ones into `v` (faer's names), then making the values
/// non-negative and sorting them non-increasing.
fn qr_algorithm(
    diag: &mut [f64],
    subdiag: &mut [f64],
    u: &mut Square,
    v: &mut Square,
) -> Result<()> {
    let n = diag.len();
    let eps = f64::EPSILON;
    let sml = MIN_POSITIVE;
    if n == 0 {
        return Ok(());
    }
    // faer: `Ord::max(30, nbits::<T>() / 2) * n * n`, nbits = 53 for f64.
    #[allow(clippy::unnecessary_min_or_max)]
    let max_iters = 30_usize
        .max(f64::MANTISSA_DIGITS as usize / 2)
        .saturating_mul(n)
        .saturating_mul(n);
    let last = n - 1;
    let max = {
        let a = norm_max(diag);
        let b = norm_max(subdiag);
        if a > b {
            a
        } else {
            b
        }
    };
    if max == 0.0 {
        return Ok(());
    }
    let max_inv = 1.0 / max;
    for x in diag.iter_mut() {
        *x *= max_inv;
    }
    for x in subdiag.iter_mut() {
        *x *= max_inv;
    }
    let rescale = |diag: &mut [f64], subdiag: &mut [f64]| {
        for x in diag.iter_mut() {
            *x *= max;
        }
        for x in subdiag.iter_mut() {
            *x *= max;
        }
    };
    let eps2 = eps * eps;
    for iter in 0..max_iters {
        for i0 in 0..last {
            let i1 = i0 + 1;
            let e = at(subdiag, i0)?;
            if e * e <= eps2 * (at(diag, i0)? * at(diag, i1)?).abs() + sml {
                *at_mut(subdiag, i0)? = 0.0;
            }
        }
        let mut end = n;
        while end >= 2 && {
            let e = at(subdiag, end - 2)?;
            e * e <= sml
        } {
            end -= 1;
        }
        if end == 1 {
            break;
        }
        let mut start = end - 1;
        while start >= 1 && at(subdiag, start - 1)? != 0.0 {
            start -= 1;
        }
        let mut found_zero_diag = false;
        for i in start..end - 1 {
            if at(diag, i)?.abs() <= eps {
                found_zero_diag = true;
                let mut val = at(subdiag, i)?;
                *at_mut(subdiag, i)? = 0.0;
                for j in i + 1..end {
                    let rot = make_givens(at(diag, j)?, val);
                    let dj = at(diag, j)?;
                    *at_mut(diag, j)? = rot.c * dj - rot.s * val;
                    if j + 1 < end {
                        let ej = at(subdiag, j)?;
                        val = rot.s * ej;
                        *at_mut(subdiag, j)? = rot.c * ej;
                    }
                    // rot.adjoint().apply_on_the_right_in_place(v.two_cols_mut(j, i)):
                    // adjoint (s -> -s) then transpose (s -> -s) = rot itself.
                    let (x, y) = v.two_cols(j, i)?;
                    rotate_left(rot.c, rot.s, x, y)?;
                }
            }
        }
        if found_zero_diag {
            if iter + 1 == max_iters {
                rescale(diag, subdiag);
                return Err(PetirError::MaxIterations);
            }
            continue;
        }
        let end2 = end - 2;
        let end1 = end - 1;
        let d2 = at(diag, end2)?;
        let t00 = if end - start == 2 {
            d2 * d2
        } else {
            let e3 = at(subdiag, end - 3)?;
            d2 * d2 + e3 * e3
        };
        let d1 = at(diag, end1)?;
        let e2 = at(subdiag, end2)?;
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
        let ds = at(diag, start)?;
        let mut y = ds * ds - mu;
        let mut z = ds * at(subdiag, start)?;
        for k in start..end1 {
            let rot = make_givens(y, z);
            if k > start {
                *at_mut(subdiag, k - 1)? = (rot.c * y - rot.s * z).abs();
            }
            let mut diag_k = at(diag, k)?;
            let ek = at(subdiag, k)?;
            let tmp = (rot.c * diag_k - rot.s * ek, rot.s * diag_k + rot.c * ek);
            diag_k = tmp.0;
            *at_mut(subdiag, k)? = tmp.1;
            let k1 = k + 1;
            y = diag_k;
            let dk1 = at(diag, k1)?;
            z = -rot.s * dk1;
            *at_mut(diag, k1)? = rot.c * dk1;
            {
                // rot.apply_on_the_right_in_place((k1, k)): transpose, s -> -s.
                let (x, yy) = u.two_cols(k1, k)?;
                rotate_left(rot.c, -rot.s, x, yy)?;
            }
            let rot = make_givens(y, z);
            diag_k = rot.c * y - rot.s * z;
            *at_mut(diag, k)? = diag_k;
            let ek = at(subdiag, k)?;
            let dk1 = at(diag, k1)?;
            let tmp = (rot.c * ek - rot.s * dk1, rot.s * ek + rot.c * dk1);
            *at_mut(subdiag, k)? = tmp.0;
            *at_mut(diag, k1)? = tmp.1;
            if k < end - 2 {
                y = at(subdiag, k)?;
                let ek1 = at(subdiag, k1)?;
                z = -rot.s * ek1;
                *at_mut(subdiag, k1)? = rot.c * ek1;
            }
            {
                let (x, yy) = v.two_cols(k1, k)?;
                rotate_left(rot.c, -rot.s, x, yy)?;
            }
        }
        if iter + 1 == max_iters {
            rescale(diag, subdiag);
            return Err(PetirError::MaxIterations);
        }
    }
    for j in 0..n {
        let d = at_mut(diag, j)?;
        if *d < 0.0 {
            *d = -*d;
            for x in v.col_mut(j, 0)? {
                *x = -*x;
            }
        }
    }
    for k in 0..n {
        let mut mx = 0.0;
        let mut idx = k;
        for kk in k..n {
            let dkk = at(diag, kk)?;
            if dkk > mx {
                mx = dkk;
                idx = kk;
            }
        }
        if k != idx {
            let dk = at(diag, k)?;
            let di = at(diag, idx)?;
            *at_mut(diag, idx)? = dk;
            *at_mut(diag, k)? = di;
            u.swap_cols(k, idx)?;
            v.swap_cols(k, idx)?;
        }
    }
    rescale(diag, subdiag);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn recompose(r: &FaerSvd) -> Vec<f64> {
        let n = r.n;
        let mut a = vec![0.0; n * n];
        for i in 0..n {
            for j in 0..n {
                let mut acc = 0.0;
                for k in 0..n {
                    acc += r.u[k * n + i] * r.s[k] * r.v[k * n + j];
                }
                a[j * n + i] = acc;
            }
        }
        a
    }

    #[test]
    fn reconstructs_a_general_3x3() {
        let a = [2.0, -1.0, 0.5, 0.3, 4.0, -2.0, 1.5, 0.7, -3.0];
        let r = svd_square(&a, 3).unwrap();
        for (x, y) in recompose(&r).iter().zip(a) {
            assert!((x - y).abs() < 1e-13, "{x} vs {y}");
        }
        assert!(r.s[0] >= r.s[1] && r.s[1] >= r.s[2] && r.s[2] >= 0.0);
    }

    #[test]
    fn zero_matrix_gives_identity_vectors_and_bad_input_errors() {
        let r = svd_square(&[0.0; 9], 3).unwrap();
        assert_eq!(r.s, [0.0, 0.0, 0.0]);
        assert_eq!(r.u, Square::identity(3).unwrap().data);
        assert_eq!(svd_square(&[f64::NAN; 4], 2), Err(PetirError::Domain));
        assert_eq!(svd_square(&[], 0), Err(PetirError::Invalid));
        assert_eq!(
            svd_square(&[1.0; 3], 2),
            Err(PetirError::LengthMismatch {
                expected: 4,
                found: 3
            })
        );
    }

    #[test]
    fn svd3_is_svd_square_in_row_major() {
        let m = [[1.0, 2.0, 0.0], [0.0, 1.0, -1.0], [3.0, 0.0, 1.0]];
        let (u, s, v) = svd3(m).unwrap();
        let cm = [1.0, 0.0, 3.0, 2.0, 1.0, 0.0, 0.0, -1.0, 1.0];
        let r = svd_square(&cm, 3).unwrap();
        assert_eq!(s.to_vec(), r.s);
        assert_eq!(u[1][2], r.u[2 * 3 + 1]);
        assert_eq!(v[2][0], r.v[2]);
    }
}
