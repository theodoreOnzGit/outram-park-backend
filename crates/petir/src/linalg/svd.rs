// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 Theodore Ong and the outram-park contributors.
//
// PORTED from the GNU Scientific Library (GSL) 2.8, commit
// cf180cd7fbd06039a577f9c9ff0b428784765ac1, read 2026-10-20.
// GPL-3.0-or-later (verified from each file's own header; see NOTICE).
//
//   linalg/svd.c        Copyright (C) 1996-2000, 2004, 2007, 2010 Gerard
//                       Jungman, Brian Gough
//     :49   gsl_linalg_SV_decomp
//     :421  gsl_linalg_SV_solve
//   linalg/svdstep.c    Copyright (C) 2007, 2010 Brian Gough
//     :21   chop_small_elements, :44 trailing_eigenvalue (the default,
//           non-GOLUB_VAN_LOAN_8_3_2 branch), :118 create_schur, :171 svd2,
//     :300  chase_out_intermediate_zero, :358 chase_out_trailing_zero,
//     :416  qrstep  (every rotation loop is the !USE_BLAS branch, which is
//           what the vendored build compiles: USE_BLAS is never defined)
//   linalg/bidiag.c     Copyright (C) 2001, 2007 Brian Gough
//     :75   gsl_linalg_bidiag_decomp, :244 gsl_linalg_bidiag_unpack2
//   linalg/householder.c Copyright (C) 1996-2000, 2004, 2007, 2010 Gerard
//                       Jungman, Brian Gough
//     :397  gsl_linalg_householder_right, :434 gsl_linalg_householder_hm1
//           (!USE_BLAS branch)
//   linalg/gsl_linalg.h Copyright (C) 1996-2000, 2006, 2007, 2019 Gerard
//                       Jungman, Brian Gough, Patrick Alken
//     :940  gsl_linalg_givens -- textually identical to eigen/qrstep.c's
//           create_givens, so `eigen_symm::create_givens` is reused
//   cblas/source_gemv_r.h, source_ger.h   Copyright (C) 1996-2000 Gerard
//                       Jungman -- inlined, RowMajor branches

//! Singular value decomposition `A = U S V^T` of an `m x n` matrix with
//! `m >= n` — GSL's `gsl_linalg_SV_decomp` (Golub–Reinsch).
//!
//! # Algorithm
//!
//! Householder bidiagonalisation (`linalg/bidiag.c`), then implicit-shift QR
//! sweeps on the bidiagonal (`linalg/svdstep.c`), with GSL's chasing of zero
//! diagonal entries, its direct 2x2 SVD, and its temporary rescaling of a
//! block whose norm is near overflow or underflow. Finally singular values are
//! made non-negative and sorted into **decreasing** order, columns of `U` and
//! `V` with them.
//!
//! # Storage
//!
//! [`Svd::u`] is `m x n` (GSL overwrites `A` with it), [`Svd::s`] has `n`
//! entries, [`Svd::v`] is `n x n` and is `V`, not `V^T`.
//!
//! # Not ported
//!
//! `SV_decomp_mod` (the `m >> n` variant), `SV_decomp_jacobi`, `SV_solve2`,
//! `SV_lssolve` and `SV_leverage`. `m < n` is refused, as GSL refuses it
//! (`GSL_EUNIMPL`); decompose the transpose.
//!
//! # Units
//!
//! Bare dimensionless `f64`.

use alloc::vec;
use alloc::vec::Vec;

#[allow(unused_imports)]
use crate::real::Real;

use crate::linalg::blas1;
use crate::linalg::eigen_symm::create_givens as givens;
use crate::linalg::elem::{acc, rd, wr};
use crate::linalg::householder;
use crate::linalg::symmtd::householder_left;
use crate::linalg::Matrix;
use crate::scalar::DBL_EPSILON as GSL_DBL_EPSILON;
use crate::{PetirError, Result};

/// `GSL_SQRT_DBL_MIN`, `gsl_machine.h:26`, as upstream writes it.
const GSL_SQRT_DBL_MIN: f64 = 1.4916681462400413e-154;
/// `GSL_SQRT_DBL_MAX`, `gsl_machine.h:34`, as upstream writes it.
const GSL_SQRT_DBL_MAX: f64 = 1.3407807929942596e+154;

/// A singular value decomposition, GSL's layout.
#[derive(Debug, Clone, PartialEq)]
pub struct Svd {
    u: Matrix,
    s: Vec<f64>,
    v: Matrix,
}

impl Svd {
    /// Decompose `a` (`m x n`, `m >= n`), `gsl_linalg_SV_decomp`
    /// (`linalg/svd.c:49`).
    ///
    /// # Errors
    ///
    /// - [`PetirError::Unimplemented`] if `m < n` (`GSL_EUNIMPL`).
    /// - [`PetirError::MaxIterations`] if the QR sweeps exceed `100 n`
    ///   iterations (`GSL_EMAXITER`).
    ///
    /// # Example
    ///
    /// ```
    /// use petir::linalg::{Matrix, Svd};
    /// let a = Matrix::from_row_major(2, 2, vec![3.0, 0.0, 4.0, 5.0]).unwrap();
    /// let svd = Svd::new(&a).unwrap();
    /// // singular values of [[3,0],[4,5]] are 3 sqrt(5) and sqrt(5)
    /// assert!((svd.s()[0] - 45.0_f64.sqrt()).abs() < 1e-14);
    /// assert!((svd.s()[1] - 5.0_f64.sqrt()).abs() < 1e-14);
    /// ```
    pub fn new(a: &Matrix) -> Result<Self> {
        let m = a.rows();
        let n = a.cols();
        if m < n {
            return Err(PetirError::Unimplemented);
        }
        let mut u = a.clone();
        let mut v = Matrix::zeros(n, n)?;
        let mut s = vec![0.0_f64; n];
        sv_decomp(&mut u, &mut v, &mut s)?;
        Ok(Self { u, s, v })
    }

    /// `U`, `m x n`, orthonormal columns.
    pub fn u(&self) -> &Matrix {
        &self.u
    }

    /// The singular values, non-negative and decreasing.
    pub fn s(&self) -> &[f64] {
        &self.s
    }

    /// `V`, `n x n`, orthogonal (not its transpose).
    pub fn v(&self) -> &Matrix {
        &self.v
    }

    /// Solve `A x = b` in the least-squares sense, `gsl_linalg_SV_solve`
    /// (`svd.c:421`): `x = V diag(1/s) U^T b`, where a zero singular value
    /// contributes zero (GSL's "`alpha != 0`" test — no other threshold).
    ///
    /// # Errors
    ///
    /// [`PetirError::LengthMismatch`] if `b.len() != m`.
    pub fn solve(&self, b: &[f64]) -> Result<Vec<f64>> {
        let m = self.u.rows();
        let n = self.u.cols();
        if b.len() != m {
            return Err(PetirError::LengthMismatch {
                expected: m,
                found: b.len(),
            });
        }
        // w = U^T b: dgemv(CblasTrans, 1.0, U, b, 0.0, w)
        let mut w = vec![0.0_f64; n];
        for j in 0..m {
            let temp = 1.0 * rd(b, j);
            if temp != 0.0 {
                for i in 0..n {
                    acc(&mut w, i, temp * self.u.get(j, i));
                }
            }
        }
        for i in 0..n {
            let wi = rd(&w, i);
            let mut alpha = rd(&self.s, i);
            if alpha != 0.0 {
                alpha = 1.0 / alpha;
            }
            wr(&mut w, i, alpha * wi);
        }
        // x = V w: dgemv(CblasNoTrans, 1.0, V, w, 0.0, x)
        let mut x = vec![0.0_f64; n];
        for i in 0..n {
            let mut temp = 0.0_f64;
            for j in 0..n {
                temp += rd(&w, j) * self.v.get(i, j);
            }
            acc(&mut x, i, 1.0 * temp);
        }
        Ok(x)
    }
}

/// `gsl_linalg_SV_decomp`'s body: `a` becomes `U`.
fn sv_decomp(a: &mut Matrix, v: &mut Matrix, s: &mut [f64]) -> Result<()> {
    let m = a.rows();
    let n = a.cols();
    let k_min = m.min(n);

    if n == 1 {
        let mut column = a.column_from(0, 0);
        let norm = blas1::nrm2(&column);
        wr(s, 0, norm);
        v.set(0, 0, 1.0);
        if norm != 0.0 {
            let alpha = 1.0 / norm;
            for x in column.iter_mut() {
                *x *= alpha;
            }
            a.set_column_from(0, 0, &column);
        }
        return Ok(());
    }

    let mut f = vec![0.0_f64; k_min - 1];
    bidiag_decomp(a, s, &mut f);
    bidiag_unpack2(a, s, &mut f, v);
    chop_small_elements(s, &mut f);

    let mut b = n - 1;
    let mut iter = 0usize;
    while b > 0 {
        let fbm1 = rd(&f, b - 1);
        if fbm1 == 0.0 || fbm1.is_nan() {
            b -= 1;
            continue;
        }

        let mut a_ = b - 1;
        while a_ > 0 {
            let fam1 = rd(&f, a_ - 1);
            if fam1 == 0.0 || fam1.is_nan() {
                break;
            }
            a_ -= 1;
        }

        iter += 1;
        if iter > 100 * n {
            return Err(PetirError::MaxIterations);
        }

        let (Some(s_block), Some(f_block)) = (s.get_mut(a_..=b), f.get_mut(a_..b)) else {
            return Err(PetirError::Invalid);
        };

        let mut rescale = false;
        let mut scale = 1.0_f64;
        let mut norm = 0.0_f64;
        for &s_i in s_block.iter() {
            let ab = s_i.abs();
            if ab > norm {
                norm = ab;
            }
        }
        for &f_i in f_block.iter() {
            let ab = f_i.abs();
            if ab > norm {
                norm = ab;
            }
        }
        if norm > GSL_SQRT_DBL_MAX {
            scale = norm / GSL_SQRT_DBL_MAX;
            rescale = true;
        } else if norm < GSL_SQRT_DBL_MIN && norm > 0.0 {
            scale = norm / GSL_SQRT_DBL_MIN;
            rescale = true;
        }
        if rescale {
            let inv = 1.0 / scale;
            s_block.iter_mut().for_each(|x| *x *= inv);
            f_block.iter_mut().for_each(|x| *x *= inv);
        }

        qrstep(s_block, f_block, a, v, a_);
        chop_small_elements(s_block, f_block);

        if rescale {
            s_block.iter_mut().for_each(|x| *x *= scale);
            f_block.iter_mut().for_each(|x| *x *= scale);
        }
    }

    // Make singular values positive by reflections if necessary.
    for j in 0..k_min {
        let sj = rd(s, j);
        if sj < 0.0 {
            for i in 0..n {
                let vij = v.get(i, j);
                v.set(i, j, -vij);
            }
            wr(s, j, -sj);
        }
    }

    // Sort into decreasing order (selection sort, as upstream).
    for i in 0..k_min {
        let mut s_max = rd(s, i);
        let mut i_max = i;
        for j in (i + 1)..k_min {
            let sj = rd(s, j);
            if sj > s_max {
                s_max = sj;
                i_max = j;
            }
        }
        if i_max != i {
            s.swap(i, i_max);
            swap_columns(a, i, i_max);
            swap_columns(v, i, i_max);
        }
    }
    Ok(())
}

/// `gsl_matrix_swap_columns`. No arithmetic.
fn swap_columns(m: &mut Matrix, i: usize, j: usize) {
    for r in 0..m.rows() {
        let t = m.get(r, i);
        m.set(r, i, m.get(r, j));
        m.set(r, j, t);
    }
}

/// `gsl_linalg_bidiag_decomp` (`bidiag.c:75`).
fn bidiag_decomp(a: &mut Matrix, tau_u: &mut [f64], tau_v: &mut [f64]) {
    let m = a.rows();
    let n = a.cols();
    for j in 0..n {
        // Householder on the current column, A(j.., j)
        let mut v = a.column_from(j, j);
        let tau_j = householder::transform(&mut v);
        a.set_column_from(j, j, &v);
        if j + 1 < n {
            wr(&mut v, 0, 1.0);
            householder_left(tau_j, &v, a, j, j + 1);
        }
        wr(tau_u, j, tau_j);

        // Householder on the current row, A(j, j+1..)
        if j + 1 < n {
            let mut r: Vec<f64> = (j + 1..n).map(|c| a.get(j, c)).collect();
            let tau_j = householder::transform(&mut r);
            for (c, &x) in (j + 1..n).zip(r.iter()) {
                a.set(j, c, x);
            }
            if j + 1 < m {
                householder_right(tau_j, &r, a, j + 1, j + 1);
            }
            wr(tau_v, j, tau_j);
        }
    }
}

/// `gsl_linalg_householder_right` (`householder.c:397`): `A := A H` on the
/// trailing submatrix at `(r0, c0)`, `v.len()` columns wide.
fn householder_right(tau: f64, v: &[f64], a: &mut Matrix, r0: usize, c0: usize) {
    if tau == 0.0 {
        return;
    }
    let rows = a.rows().saturating_sub(r0);
    let n = v.len();
    let mut v1 = v.to_vec();
    wr(&mut v1, 0, 1.0);
    // work := A v, dgemv(NoTrans, 1.0, A, v, 0.0, work)
    let mut work = vec![0.0_f64; rows];
    for i in 0..rows {
        let mut temp = 0.0_f64;
        for j in 0..n {
            temp += rd(&v1, j) * a.get(r0 + i, c0 + j);
        }
        acc(&mut work, i, 1.0 * temp);
    }
    // A := A - tau work v^T, dger(-tau, work, v, A)
    for i in 0..rows {
        let tmp = -tau * rd(&work, i);
        for j in 0..n {
            let cur = a.get(r0 + i, c0 + j);
            a.set(r0 + i, c0 + j, cur + rd(&v1, j) * tmp);
        }
    }
}

/// `gsl_linalg_householder_hm1` (`householder.c:434`, `!USE_BLAS`): build up
/// from the identity, using the first column of the trailing submatrix at
/// `(r0, c0)` as the Householder vector.
fn householder_hm1(tau: f64, a: &mut Matrix, r0: usize, c0: usize) {
    let rows = a.rows().saturating_sub(r0);
    let cols = a.cols().saturating_sub(c0);
    if tau == 0.0 {
        a.set(r0, c0, 1.0);
        for j in 1..cols {
            a.set(r0, c0 + j, 0.0);
        }
        for i in 1..rows {
            a.set(r0 + i, c0, 0.0);
        }
        return;
    }
    for j in 1..cols {
        let mut wj = 0.0_f64;
        for i in 1..rows {
            let vi = a.get(r0 + i, c0);
            wj += a.get(r0 + i, c0 + j) * vi;
        }
        a.set(r0, c0 + j, -tau * wj);
        for i in 1..rows {
            let vi = a.get(r0 + i, c0);
            let aij = a.get(r0 + i, c0 + j);
            a.set(r0 + i, c0 + j, aij - tau * vi * wj);
        }
    }
    for i in 1..rows {
        let vi = a.get(r0 + i, c0);
        a.set(r0 + i, c0, -tau * vi);
    }
    a.set(r0, c0, 1.0 - tau);
}

/// `gsl_linalg_bidiag_unpack2` (`bidiag.c:244`): `a` becomes `U`, `tau_u`
/// the diagonal and `tau_v` the superdiagonal of `B`, and `v` is formed.
fn bidiag_unpack2(a: &mut Matrix, tau_u: &mut [f64], tau_v: &mut [f64], v: &mut Matrix) {
    let n = a.cols();
    for i in 0..n {
        for j in 0..n {
            v.set(i, j, if i == j { 1.0 } else { 0.0 });
        }
    }
    // for (i = N - 1; i-- > 0;): i = N-2 down to 0
    for i in (0..n.saturating_sub(1)).rev() {
        let h: Vec<f64> = (i + 1..n).map(|c| a.get(i, c)).collect();
        let ti = rd(tau_v, i);
        householder::hm(ti, &h, v, i + 1, i + 1);
    }
    for i in 0..n.saturating_sub(1) {
        wr(tau_v, i, a.get(i, i + 1));
    }
    for j in (0..n).rev() {
        let tj = rd(tau_u, j);
        let ajj = a.get(j, j);
        wr(tau_u, j, ajj);
        householder_hm1(tj, a, j, j);
    }
}

/// `chop_small_elements` (`svdstep.c:21`).
fn chop_small_elements(d: &[f64], f: &mut [f64]) {
    let n = d.len();
    let mut d_i = rd(d, 0);
    for i in 0..n.saturating_sub(1) {
        let f_i = rd(f, i);
        let d_ip1 = rd(d, i + 1);
        if f_i.abs() < GSL_DBL_EPSILON * (d_i.abs() + d_ip1.abs()) {
            wr(f, i, 0.0);
        }
        d_i = d_ip1;
    }
}

/// `trailing_eigenvalue` (`svdstep.c:44`), the default branch: the root of
/// the trailing 2x2 of `B^T B` nearest its bottom-right entry, computed
/// without cancellation.
fn trailing_eigenvalue(d: &[f64], f: &[f64]) -> f64 {
    let n = d.len();
    let da = rd(d, n - 2);
    let db = rd(d, n - 1);
    let fa = if n > 2 { rd(f, n - 3) } else { 0.0 };
    let fb = rd(f, n - 2);

    let ta = da * da + fa * fa;
    let tb = db * db + fb * fb;
    let tab = da * fb;

    let dt = (ta - tb) / 2.0;

    let s = ta + tb;
    let da2 = da * da;
    let db2 = db * db;
    let fa2 = fa * fa;
    let fb2 = fb * fb;
    let p = (da2 * db2) + (fa2 * db2) + (fa2 * fb2);
    let dd = Real::hypot(dt, tab);
    let r1 = s / 2.0 + dd;

    if dt >= 0.0 {
        if r1 > 0.0 {
            p / r1
        } else {
            0.0
        }
    } else {
        r1
    }
}

/// `create_schur` (`svdstep.c:118`).
fn create_schur(d0: f64, f0: f64, d1: f64) -> (f64, f64) {
    let mut d0 = d0;
    let mut f0 = f0;
    let mut d1 = d1;
    let mut apq = 2.0 * d0 * f0;

    if d0 == 0.0 || f0 == 0.0 {
        return (1.0, 0.0);
    }

    if d0.abs() < GSL_SQRT_DBL_MIN
        || d0.abs() > GSL_SQRT_DBL_MAX
        || f0.abs() < GSL_SQRT_DBL_MIN
        || f0.abs() > GSL_SQRT_DBL_MAX
        || d1.abs() < GSL_SQRT_DBL_MIN
        || d1.abs() > GSL_SQRT_DBL_MAX
    {
        let (_, d0_exp) = libm::frexp(d0);
        let (_, f0_exp) = libm::frexp(f0);
        let scale = libm::ldexp(1.0, -(d0_exp + f0_exp) / 4);
        d0 *= scale;
        f0 *= scale;
        d1 *= scale;
        apq = 2.0 * d0 * f0;
    }

    if apq != 0.0 {
        let tau = (f0 * f0 + (d1 + d0) * (d1 - d0)) / apq;
        let t = if tau >= 0.0 {
            1.0 / (tau + Real::hypot(1.0, tau))
        } else {
            -1.0 / (-tau + Real::hypot(1.0, tau))
        };
        let c = 1.0 / Real::hypot(1.0, t);
        (c, t * c)
    } else {
        (1.0, 0.0)
    }
}

/// `U <- U G` (or `V`) on columns `p`, `q` of `m`: GSL's inlined
/// `!USE_BLAS` rotation loop.
fn rotate_columns(m: &mut Matrix, p: usize, q: usize, c: f64, s: f64) {
    for i in 0..m.rows() {
        let uip = m.get(i, p);
        let uiq = m.get(i, q);
        m.set(i, p, c * uip - s * uiq);
        m.set(i, q, s * uip + c * uiq);
    }
}

/// `svd2` (`svdstep.c:171`): direct SVD of a 2x2 block. `off` is the
/// block's first column in `U` and `V`.
fn svd2(d: &mut [f64], f: &mut [f64], u: &mut Matrix, v: &mut Matrix, off: usize) {
    let d0 = rd(d, 0);
    let f0 = rd(f, 0);
    let d1 = rd(d, 1);

    if d0 == 0.0 {
        let (c, s) = givens(f0, d1);
        wr(d, 0, c * f0 - s * d1);
        wr(f, 0, s * f0 + c * d1);
        wr(d, 1, 0.0);
        rotate_columns(u, off, off + 1, c, s);
        swap_columns(v, off, off + 1);
    } else if d1 == 0.0 {
        let (c, s) = givens(d0, f0);
        wr(d, 0, d0 * c - f0 * s);
        wr(f, 0, 0.0);
        rotate_columns(v, off, off + 1, c, s);
    } else {
        let (c, s) = create_schur(d0, f0, d1);
        let mut a11 = c * d0 - s * f0;
        let mut a21 = -s * d1;
        let mut a12 = s * d0 + c * f0;
        let mut a22 = c * d1;
        rotate_columns(v, off, off + 1, c, s);
        if Real::hypot(a11, a21) < Real::hypot(a12, a22) {
            core::mem::swap(&mut a11, &mut a12);
            core::mem::swap(&mut a21, &mut a22);
            swap_columns(v, off, off + 1);
        }
        let (c, s) = givens(a11, a21);
        wr(d, 0, c * a11 - s * a21);
        wr(f, 0, c * a12 - s * a22);
        wr(d, 1, s * a12 + c * a22);
        rotate_columns(u, off, off + 1, c, s);
    }
}

/// `chase_out_intermediate_zero` (`svdstep.c:300`).
fn chase_out_intermediate_zero(
    d: &mut [f64],
    f: &mut [f64],
    u: &mut Matrix,
    off: usize,
    k0: usize,
) {
    let n = d.len();
    let mut x = rd(f, k0);
    let mut y = rd(d, k0 + 1);
    for k in k0..(n - 1) {
        let (c, s) = givens(y, -x);
        rotate_columns(u, off + k0, off + k + 1, c, s);
        wr(d, k + 1, s * x + c * y);
        if k == k0 {
            wr(f, k, c * x - s * y);
        }
        if k < n - 2 {
            let z = rd(f, k + 1);
            wr(f, k + 1, c * z);
            x = -s * z;
            y = rd(d, k + 2);
        }
    }
}

/// `chase_out_trailing_zero` (`svdstep.c:358`).
fn chase_out_trailing_zero(d: &mut [f64], f: &mut [f64], v: &mut Matrix, off: usize) {
    let n = d.len();
    let mut x = rd(d, n - 2);
    let mut y = rd(f, n - 2);
    // for (k = n - 1; k-- > 0;): k = n-2 down to 0
    for k in (0..(n - 1)).rev() {
        let (c, s) = givens(x, y);
        rotate_columns(v, off + k, off + n - 1, c, s);
        wr(d, k, c * x - s * y);
        if k == n - 2 {
            wr(f, k, s * x + c * y);
        }
        if k > 0 {
            let z = rd(f, k - 1);
            wr(f, k - 1, c * z);
            x = rd(d, k - 1);
            y = s * z;
        }
    }
}

/// `qrstep` (`svdstep.c:416`): one implicit-shift QR sweep on the
/// unreduced bidiagonal block `(d, f)` whose first column is `off`.
fn qrstep(d: &mut [f64], f: &mut [f64], u: &mut Matrix, v: &mut Matrix, off: usize) {
    let n = d.len();
    if n == 1 {
        return; // shouldn't happen
    }
    if n == 2 {
        svd2(d, f, u, v, off);
        return;
    }
    for i in 0..(n - 1) {
        if rd(d, i) == 0.0 {
            chase_out_intermediate_zero(d, f, u, off, i);
            return;
        }
    }
    if rd(d, n - 1) == 0.0 {
        chase_out_trailing_zero(d, f, v, off);
        return;
    }

    let d0 = rd(d, 0);
    let f0 = rd(f, 0);
    let d1 = rd(d, 1);
    let mu = trailing_eigenvalue(d, f);
    let mut y = d0 * d0 - mu;
    let mut z = d0 * f0;

    let mut ak;
    let mut bk = 0.0_f64;
    let mut zk;
    let mut ap = d0;
    let mut bp = f0;
    let mut aq = d1;

    for k in 0..(n - 1) {
        let (c, s) = givens(y, z);
        rotate_columns(v, off + k, off + k + 1, c, s);

        // B <= B G
        {
            let bk1 = c * bk - s * z;
            let ap1 = c * ap - s * bp;
            let bp1 = s * ap + c * bp;
            let zp1 = -s * aq;
            let aq1 = c * aq;
            if k > 0 {
                wr(f, k - 1, bk1);
            }
            ak = ap1;
            bk = bp1;
            zk = zp1;
            ap = aq1;
            bp = if k < n - 2 { rd(f, k + 1) } else { 0.0 };
            y = ak;
            z = zk;
        }

        let (c, s) = givens(y, z);
        rotate_columns(u, off + k, off + k + 1, c, s);

        // B <= G^T B
        {
            let ak1 = c * ak - s * zk;
            let bk1 = c * bk - s * ap;
            let zk1 = -s * bp;
            let ap1 = s * bk + c * ap;
            let bp1 = c * bp;
            wr(d, k, ak1);
            bk = bk1;
            zk = zk1;
            ap = ap1;
            bp = bp1;
            aq = if k < n - 2 { rd(d, k + 2) } else { 0.0 };
            y = bk;
            z = zk;
        }
    }
    wr(f, n - 2, bk);
    wr(d, n - 1, ap);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rebuild(svd: &Svd) -> Matrix {
        let (m, n) = (svd.u().rows(), svd.u().cols());
        let mut a = Matrix::zeros(m, n).unwrap();
        for i in 0..m {
            for j in 0..n {
                let mut x = 0.0;
                for k in 0..n {
                    x += svd.u().get(i, k) * svd.s()[k] * svd.v().get(j, k);
                }
                a.set(i, j, x);
            }
        }
        a
    }

    /// `U S V^T` reproduces `A`, values are sorted and non-negative, and the
    /// solve of a consistent system returns its solution.
    #[test]
    fn decomposition_reproduces_a_and_solves() {
        let a = Matrix::from_row_major(
            4,
            3,
            vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 10.0, 2.0, -1.0, 4.0],
        )
        .unwrap();
        let svd = Svd::new(&a).unwrap();
        let r = rebuild(&svd);
        for i in 0..4 {
            for j in 0..3 {
                assert!((r.get(i, j) - a.get(i, j)).abs() < 1e-13);
            }
        }
        assert!(svd.s().windows(2).all(|w| w[0] >= w[1] && w[1] >= 0.0));
        let x0 = [1.0, -2.0, 0.5];
        let b = a.mul_vec(&x0).unwrap();
        let x = svd.solve(&b).unwrap();
        for (xi, ei) in x.iter().zip(x0) {
            assert!((xi - ei).abs() < 1e-12);
        }
        assert!(svd.solve(&[1.0]).is_err());
    }

    /// A column vector, a rank-deficient matrix (zero on the bidiagonal), a
    /// wide matrix (refused) and the `create_schur` rescaling branch.
    #[test]
    fn special_paths() {
        let col = Matrix::from_row_major(2, 1, vec![3.0, 4.0]).unwrap();
        let s = Svd::new(&col).unwrap();
        assert_eq!(s.s(), &[5.0]);
        assert_eq!(s.u().get(1, 0), 0.8);

        let rank1 = Matrix::from_row_major(3, 3, vec![1.0, 2.0, 3.0, 2.0, 4.0, 6.0, 3.0, 6.0, 9.0])
            .unwrap();
        let s = Svd::new(&rank1).unwrap();
        assert!(s.s()[1].abs() < 1e-14 && s.s()[2].abs() < 1e-14);
        let r = rebuild(&s);
        for i in 0..3 {
            for j in 0..3 {
                assert!((r.get(i, j) - rank1.get(i, j)).abs() < 1e-13);
            }
        }

        assert_eq!(
            Svd::new(&Matrix::zeros(2, 3).unwrap()),
            Err(PetirError::Unimplemented)
        );

        let tiny = Matrix::from_row_major(2, 2, vec![1e-160, 2e-160, 0.0, 3e-160]).unwrap();
        let s = Svd::new(&tiny).unwrap();
        let r = rebuild(&s);
        for i in 0..2 {
            for j in 0..2 {
                assert!((r.get(i, j) - tiny.get(i, j)).abs() < 1e-173);
            }
        }
    }
}
