// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 Theodore Ong and the outram-park contributors.
//
// PORTED from the GNU Scientific Library (GSL) 2.8, commit
// cf180cd7fbd06039a577f9c9ff0b428784765ac1, read 2026-10-20.
// GPL-3.0-or-later (verified from each file's own header; see NOTICE).
//
//   eigen/nonsymm.c     Copyright (C) 2006 Patrick Alken
//     :172  gsl_eigen_nonsymm (the eigenvalues-only path: Z == NULL)
//   eigen/francis.c     Copyright (C) 2006, 2007 Patrick Alken
//     :151  gsl_eigen_francis, :291 francis_schur_decomp,
//     :448  francis_qrstep, :694 francis_search_subdiag_small_elements,
//     :728  francis_schur_standardize, :900 francis_standard_form
//     (compute_t == 0 and Z == NULL: the branches that build T or Z are
//      not ported; see the module docs)
//   linalg/hessenberg.c Copyright (C) 2006 Patrick Alken
//     :67   gsl_linalg_hessenberg_decomp
//   linalg/balancemat.c Copyright (C) 2006 Patrick Alken
//     :48   gsl_linalg_balance_matrix
//   linalg/householder.c Copyright (C) 1996-2000, 2004, 2007, 2010 Gerard
//                       Jungman, Brian Gough
//     :162  gsl_linalg_householder_hm, :231 gsl_linalg_householder_mh
//           (!USE_BLAS branches, on an explicitly sized submatrix)
//   sys/hypot.c         Copyright (C) 1996-2000, 2007 Gerard Jungman,
//                       Brian Gough, Patrick Alken
//     :24   gsl_hypot (GSL's own, called by name in francis.c -- not libm's)

//! Eigenvalues of a general real matrix — GSL's `gsl_eigen_nonsymm`.
//!
//! # Algorithm
//!
//! Optional balancing (`gsl_linalg_balance_matrix`, off by default as in
//! GSL's `gsl_eigen_nonsymm_alloc`), Householder reduction to upper
//! Hessenberg form, then the Francis double-shift QR algorithm (Golub & Van
//! Loan, 3rd ed., algorithm 7.5.2) with LAPACK `DLAHQR`'s exceptional shifts
//! every tenth iteration, and `DLANV2`'s standardisation of each converged
//! 2x2 block. This is GSL's code path line for line.
//!
//! # Complex eigenvalues
//!
//! Returned as [`Complex64`], the type `crate::depletion` already uses for
//! CRAM — reused rather than duplicated. A complex pair appears as
//! consecutive entries `re + i*im`, `re - i*im`.
//!
//! # Order
//!
//! **Unordered**: the order in which the QR iteration deflates them, which is
//! GSL's order.
//!
//! # Not ported
//!
//! The Schur form `T` (`gsl_eigen_nonsymm_params(compute_t = 1)`), the Schur
//! vectors (`gsl_eigen_nonsymm_Z`) and eigenvectors (`gsl_eigen_nonsymmv`).
//! Their branches in `francis.c` are skipped, not stubbed.
//!
//! # Units
//!
//! Bare dimensionless `f64`.

use alloc::vec;
use alloc::vec::Vec;

#[allow(unused_imports)]
use crate::real::Real;

use crate::depletion::Complex64;
use crate::linalg::elem::{rd, wr};
use crate::linalg::householder;
use crate::linalg::Matrix;
use crate::scalar::DBL_EPSILON as GSL_DBL_EPSILON;
use crate::{PetirError, Result};

/// `GSL_FRANCIS_COEFF1`, from LAPACK `DLAHQR` (`francis.c:44`).
const GSL_FRANCIS_COEFF1: f64 = 0.75;
/// `GSL_FRANCIS_COEFF2`, from LAPACK `DLAHQR` (`francis.c:45`).
const GSL_FRANCIS_COEFF2: f64 = -0.4375;
/// `FLOAT_RADIX` (`balancemat.c:44`).
const FLOAT_RADIX: f64 = 2.0;
/// `FLOAT_RADIX_SQ` (`balancemat.c:45`).
const FLOAT_RADIX_SQ: f64 = FLOAT_RADIX * FLOAT_RADIX;

/// `GSL_SIGN`: `+1` for `x >= 0`, else `-1`.
#[inline]
fn gsl_sign(x: f64) -> f64 {
    if x >= 0.0 {
        1.0
    } else {
        -1.0
    }
}

/// `gsl_hypot` (`sys/hypot.c:24`).
fn gsl_hypot(x: f64, y: f64) -> f64 {
    let xabs = x.abs();
    let yabs = y.abs();
    if x.is_infinite() || y.is_infinite() {
        return f64::INFINITY;
    }
    let (min, max) = if xabs < yabs {
        (xabs, yabs)
    } else {
        (yabs, xabs)
    };
    if min == 0.0 {
        return max;
    }
    let u = min / max;
    max * Real::sqrt(1.0 + u * u)
}

/// `gsl_linalg_householder_hm` (`!USE_BLAS`) on the `v.len() x ncols`
/// submatrix at `(r0, c0)`; `v[0]` is implicitly `1`.
fn hm_sub(tau: f64, v: &[f64], a: &mut Matrix, r0: usize, c0: usize, ncols: usize) {
    if tau == 0.0 {
        return;
    }
    let nrows = v.len();
    for j in 0..ncols {
        let mut wj = a.get(r0, c0 + j);
        for i in 1..nrows {
            wj += a.get(r0 + i, c0 + j) * rd(v, i);
        }
        let a0j = a.get(r0, c0 + j);
        a.set(r0, c0 + j, a0j - tau * wj);
        for i in 1..nrows {
            let aij = a.get(r0 + i, c0 + j);
            let vi = rd(v, i);
            a.set(r0 + i, c0 + j, aij - tau * vi * wj);
        }
    }
}

/// `gsl_linalg_householder_mh` (`!USE_BLAS`) on the `nrows x v.len()`
/// submatrix at `(r0, c0)`; `v[0]` is implicitly `1`.
fn mh_sub(tau: f64, v: &[f64], a: &mut Matrix, r0: usize, c0: usize, nrows: usize) {
    if tau == 0.0 {
        return;
    }
    let ncols = v.len();
    for i in 0..nrows {
        let mut wi = a.get(r0 + i, c0);
        for j in 1..ncols {
            wi += a.get(r0 + i, c0 + j) * rd(v, j);
        }
        let ai0 = a.get(r0 + i, c0);
        a.set(r0 + i, c0, ai0 - tau * wi);
        for j in 1..ncols {
            let vj = rd(v, j);
            let aij = a.get(r0 + i, c0 + j);
            a.set(r0 + i, c0 + j, aij - tau * wi * vj);
        }
    }
}

/// Balance `a` in place by powers of two, `gsl_linalg_balance_matrix`
/// (`balancemat.c:48`), returning the diagonal `D` with
/// `a_balanced = D^{-1} a D`. The eigenvalues are unchanged; the norm is
/// reduced, which usually improves their accuracy.
pub fn balance_matrix(a: &mut Matrix) -> Vec<f64> {
    let n = a.rows();
    let mut d = vec![1.0_f64; n];
    let mut not_converged = true;
    while not_converged {
        not_converged = false;
        for i in 0..n {
            let mut row_norm = 0.0_f64;
            let mut col_norm = 0.0_f64;
            for j in 0..n {
                if j != i {
                    col_norm += a.get(j, i).abs();
                    row_norm += a.get(i, j).abs();
                }
            }
            if col_norm == 0.0 || row_norm == 0.0 {
                continue;
            }
            let mut g = row_norm / FLOAT_RADIX;
            let mut f = 1.0_f64;
            let s = col_norm + row_norm;
            while col_norm < g {
                f *= FLOAT_RADIX;
                col_norm *= FLOAT_RADIX_SQ;
            }
            g = row_norm * FLOAT_RADIX;
            while col_norm > g {
                f /= FLOAT_RADIX;
                col_norm /= FLOAT_RADIX_SQ;
            }
            if (row_norm + col_norm) < 0.95 * s * f {
                not_converged = true;
                g = 1.0 / f;
                for j in 0..n {
                    a.set(i, j, a.get(i, j) * g);
                }
                for j in 0..n {
                    a.set(j, i, a.get(j, i) * f);
                }
                let di = rd(&d, i) * f;
                wr(&mut d, i, di);
            }
        }
    }
    d
}

/// Upper Hessenberg reduction `A = U H U^T`, `gsl_linalg_hessenberg_decomp`
/// (`hessenberg.c:67`), in GSL's packed layout: `H` on and above the first
/// subdiagonal, Householder vector tails below it, and `tau` (`n` long; the
/// last two entries are unused and zero). `n < 3` is returned unchanged.
///
/// # Errors
///
/// [`PetirError::NotSquare`] if `a` is not square.
pub fn hessenberg(a: &Matrix) -> Result<(Matrix, Vec<f64>)> {
    let n = a.rows();
    if n != a.cols() {
        return Err(PetirError::NotSquare {
            rows: n,
            cols: a.cols(),
        });
    }
    let mut h = a.clone();
    let mut tau = vec![0.0_f64; n];
    hessenberg_decomp(&mut h, &mut tau);
    Ok((h, tau))
}

/// `gsl_linalg_hessenberg_decomp` (`hessenberg.c:67`): reduce `a` to upper
/// Hessenberg form in place, Householder vectors below the subdiagonal.
fn hessenberg_decomp(a: &mut Matrix, tau: &mut [f64]) {
    let n = a.rows();
    if n < 3 {
        return;
    }
    for i in 0..(n - 2) {
        let mut hv = a.column_from(i, i + 1);
        let tau_i = householder::transform(&mut hv);
        hm_sub(tau_i, &hv, a, i + 1, i, n - i);
        mh_sub(tau_i, &hv, a, 0, i + 1, n);
        wr(tau, i, tau_i);
        // store hv(1..) below the subdiagonal of column i
        for (k, &x) in hv.iter().enumerate().skip(1) {
            a.set(i + 1 + k, i, x);
        }
    }
}

/// Shared state of `gsl_eigen_francis_workspace` that the eigenvalue path
/// reads.
struct Francis {
    evals: Vec<Complex64>,
    n_iter: usize,
    max_iterations: usize,
}

/// `francis_search_subdiag_small_elements` (`francis.c:694`) on the active
/// `n x n` block at `(top, top)`.
fn search_subdiag_small_elements(h: &mut Matrix, top: usize, n: usize) -> usize {
    for i in (1..n).rev() {
        let sel = h.get(top + i, top + i - 1);
        let del = h.get(top + i, top + i);
        let dpel = h.get(top + i - 1, top + i - 1);
        if sel == 0.0 || sel.abs() < GSL_DBL_EPSILON * (del.abs() + dpel.abs()) {
            h.set(top + i, top + i - 1, 0.0);
            return i;
        }
    }
    0
}

/// `francis_standard_form` (`francis.c:900`, LAPACK `DLANV2`) on the 2x2
/// block at `(t, t)`. The rotation `(cs, sn)` is only needed to build `T`
/// or `Z`, so it is computed (to keep the arithmetic identical) and dropped.
fn standard_form(h: &mut Matrix, t: usize) {
    let mut a = h.get(t, t);
    let mut b = h.get(t, t + 1);
    let mut c = h.get(t + 1, t);
    let mut d = h.get(t + 1, t + 1);
    let mut cs: f64;
    let mut sn: f64;

    if c == 0.0 {
        // already upper triangular; rotation is the identity
    } else if b == 0.0 {
        // swap rows and columns to make it upper triangular
        core::mem::swap(&mut a, &mut d);
        b = -c;
        c = 0.0;
    } else if (a - d) == 0.0 && gsl_sign(b) != gsl_sign(c) {
        // complex eigenvalues with a == d
    } else {
        let mut tmp = a - d;
        let mut p = 0.5 * tmp;
        let bcmax = if b.abs() > c.abs() { b.abs() } else { c.abs() };
        let bcmis = (if b.abs() < c.abs() { b.abs() } else { c.abs() }) * gsl_sign(b) * gsl_sign(c);
        let scale = if p.abs() > bcmax { p.abs() } else { bcmax };
        let mut z = (p / scale) * p + (bcmax / scale) * bcmis;

        if z >= 4.0 * GSL_DBL_EPSILON {
            // real eigenvalues
            z = p + gsl_sign(p) * (Real::sqrt(scale) * Real::sqrt(z)).abs();
            a = d + z;
            d -= (bcmax / z) * bcmis;
            let tau = gsl_hypot(c, z);
            cs = z / tau;
            sn = c / tau;
            b -= c;
            c = 0.0;
            let _ = (cs, sn);
        } else {
            // complex or (almost) equal real eigenvalues: equal diagonal
            let sigma = b + c;
            let tau = gsl_hypot(sigma, tmp);
            cs = Real::sqrt(0.5 * (1.0 + sigma.abs() / tau));
            sn = -(p / (tau * cs)) * gsl_sign(sigma);

            let aa = a * cs + b * sn;
            let bb = -a * sn + b * cs;
            let cc = c * cs + d * sn;
            let dd = -c * sn + d * cs;

            a = aa * cs + cc * sn;
            b = bb * cs + dd * sn;
            c = -aa * sn + cc * cs;
            d = -bb * sn + dd * cs;

            tmp = 0.5 * (a + d);
            a = tmp;
            d = tmp;

            if c != 0.0 {
                if b != 0.0 {
                    if gsl_sign(b) == gsl_sign(c) {
                        // real eigenvalues: reduce to upper triangular
                        let sab = Real::sqrt(b.abs());
                        let sac = Real::sqrt(c.abs());
                        p = gsl_sign(c) * (sab * sac).abs();
                        let tau = 1.0 / Real::sqrt((b + c).abs());
                        a = tmp + p;
                        d = tmp - p;
                        b -= c;
                        c = 0.0;
                        let cs1 = sab * tau;
                        let sn1 = sac * tau;
                        let t2 = cs * cs1 - sn * sn1;
                        sn = cs * sn1 + sn * cs1;
                        cs = t2;
                    }
                } else {
                    b = -c;
                    c = 0.0;
                    let t2 = cs;
                    cs = -sn;
                    sn = t2;
                }
            }
            let _ = (cs, sn);
        }
    }

    h.set(t, t, a);
    h.set(t, t + 1, b);
    h.set(t + 1, t, c);
    h.set(t + 1, t + 1, d);
}

/// `francis_schur_standardize` (`francis.c:728`), eigenvalue part: returns
/// the block's two eigenvalues.
fn schur_standardize(h: &mut Matrix, t: usize) -> (Complex64, Complex64) {
    standard_form(h, t);
    let re1 = h.get(t, t);
    let re2 = h.get(t + 1, t + 1);
    if h.get(t + 1, t) == 0.0 {
        (Complex64::new(re1, 0.0), Complex64::new(re2, 0.0))
    } else {
        let tmp = Real::sqrt(h.get(t, t + 1).abs() * h.get(t + 1, t).abs());
        (Complex64::new(re1, tmp), Complex64::new(re2, -tmp))
    }
}

/// How many times the exceptional-shift branch ran, so a test can show the
/// cases claimed to reach it do.
#[cfg(test)]
static EXCEPTIONAL_SHIFTS: core::sync::atomic::AtomicUsize =
    core::sync::atomic::AtomicUsize::new(0);

/// `francis_qrstep` (`francis.c:448`) on the active `n x n` block at
/// `(top, top)`, `compute_t == 0`, `Z == NULL`.
fn qrstep(h: &mut Matrix, top: usize, n: usize, n_iter: usize) {
    let hh = |h: &Matrix, i: usize, j: usize| h.get(top + i, top + j);
    let mut h_nn;
    let mut h_nm1nm1;
    let mut h_cross;

    if n_iter % 10 == 0 {
        #[cfg(test)]
        EXCEPTIONAL_SHIFTS.fetch_add(1, core::sync::atomic::Ordering::Relaxed);
        // exceptional shifts (LAPACK DLAHQR)
        let s = hh(h, n - 1, n - 2).abs() + hh(h, n - 2, n - 3).abs();
        h_nn = hh(h, n - 1, n - 1) + GSL_FRANCIS_COEFF1 * s;
        h_nm1nm1 = h_nn;
        h_cross = GSL_FRANCIS_COEFF2 * s * s;
    } else {
        h_nn = hh(h, n - 1, n - 1);
        h_nm1nm1 = hh(h, n - 2, n - 2);
        h_cross = hh(h, n - 1, n - 2) * hh(h, n - 2, n - 1);
        let mut disc = 0.5 * (h_nm1nm1 - h_nn);
        disc = disc * disc + h_cross;
        if disc > 0.0 {
            // real roots: Wilkinson's shift twice
            disc = Real::sqrt(disc);
            let ave = 0.5 * (h_nm1nm1 + h_nn);
            if h_nm1nm1.abs() - h_nn.abs() > 0.0 {
                h_nm1nm1 = h_nm1nm1 * h_nn - h_cross;
                h_nn = h_nm1nm1 / (disc * gsl_sign(ave) + ave);
            } else {
                h_nn = disc * gsl_sign(ave) + ave;
            }
            h_nm1nm1 = h_nn;
            h_cross = 0.0;
        }
    }

    let h_tmp1 = h_nm1nm1 - hh(h, 0, 0);
    let h_tmp2 = h_nn - hh(h, 0, 0);

    let mut dat = [0.0_f64; 3];
    wr(
        &mut dat,
        0,
        (h_tmp1 * h_tmp2 - h_cross) / hh(h, 1, 0) + hh(h, 0, 1),
    );
    wr(&mut dat, 1, hh(h, 1, 1) - hh(h, 0, 0) - h_tmp1 - h_tmp2);
    wr(&mut dat, 2, hh(h, 2, 1));

    let rescale3 = |dat: &mut [f64; 3]| {
        let scale = rd(dat, 0).abs() + rd(dat, 1).abs() + rd(dat, 2).abs();
        if scale != 0.0 {
            for k in 0..3 {
                let x = rd(dat, k) / scale;
                wr(dat, k, x);
            }
        }
    };
    rescale3(&mut dat);

    for i in 0..(n - 2) {
        let tau_i = householder::transform(&mut dat);
        if tau_i != 0.0 {
            let q = if i <= 1 { 0 } else { i - 1 };
            let r = if (i + 3) < (n - 1) { i + 3 } else { n - 1 };
            hm_sub(tau_i, &dat, h, top + i, top + q, n - q);
            mh_sub(tau_i, &dat, h, top, top + i, r + 1);
        }
        wr(&mut dat, 0, hh(h, i + 1, i));
        wr(&mut dat, 1, hh(h, i + 2, i));
        if i < n - 3 {
            wr(&mut dat, 2, hh(h, i + 3, i));
        }
        rescale3(&mut dat);
    }

    let scale = rd(&dat, 0).abs() + rd(&dat, 1).abs();
    if scale != 0.0 {
        let x0 = rd(&dat, 0) / scale;
        wr(&mut dat, 0, x0);
        let x1 = rd(&dat, 1) / scale;
        wr(&mut dat, 1, x1);
    }
    let Some(v2) = dat.get_mut(0..2) else {
        return;
    };
    let tau_i = householder::transform(v2);
    hm_sub(tau_i, v2, h, top + n - 2, top + n - 3, 3);
    mh_sub(tau_i, v2, h, top, top + n - 2, n);
}

/// `francis_schur_decomp` (`francis.c:291`) on the `n x n` block at
/// `(top, top)`.
fn schur_decomp(h: &mut Matrix, top: usize, n: usize, w: &mut Francis) {
    let mut big_n = n;
    let mut m_top = top;

    while big_n > 2 && {
        let go = w.n_iter < w.max_iterations;
        w.n_iter += 1;
        go
    } {
        let q = search_subdiag_small_elements(h, m_top, big_n);
        if q == 0 {
            qrstep(h, m_top, big_n, w.n_iter);
            continue;
        }
        if q == big_n - 1 {
            w.evals
                .push(Complex64::new(h.get(m_top + q, m_top + q), 0.0));
            w.n_iter = 0;
            big_n -= 1;
        } else if q == big_n - 2 {
            let (l1, l2) = schur_standardize(h, m_top + q);
            w.evals.push(l1);
            w.evals.push(l2);
            w.n_iter = 0;
            big_n -= 2;
        } else if q == 1 {
            w.evals.push(Complex64::new(h.get(m_top, m_top), 0.0));
            w.n_iter = 0;
            big_n -= 1;
            m_top += 1;
        } else if q == 2 {
            let (l1, l2) = schur_standardize(h, m_top);
            w.evals.push(l1);
            w.evals.push(l2);
            w.n_iter = 0;
            big_n -= 2;
            m_top += 2;
        } else {
            // split at q: lower right block first, then upper left
            schur_decomp(h, m_top + q, big_n - q, w);
            schur_decomp(h, m_top, q, w);
            big_n = 0;
        }
    }

    if big_n == 1 {
        w.evals.push(Complex64::new(h.get(m_top, m_top), 0.0));
        w.n_iter = 0;
    } else if big_n == 2 {
        let (l1, l2) = schur_standardize(h, m_top);
        w.evals.push(l1);
        w.evals.push(l2);
        w.n_iter = 0;
    }
}

/// Eigenvalues of the general real square matrix `a`, unordered,
/// `gsl_eigen_nonsymm` (`eigen/nonsymm.c:172`) with
/// `gsl_eigen_francis` (`francis.c:151`).
///
/// `balance` is `gsl_eigen_nonsymm_params`'s `balance` flag; GSL's default
/// (from `gsl_eigen_nonsymm_alloc`) is `false`.
///
/// # Errors
///
/// - [`PetirError::NotSquare`] if `a` is not square.
/// - [`PetirError::MaxIterations`] if the QR iteration runs `30 n`
///   iterations without deflating (`GSL_EMAXITER`).
///
/// # Example
///
/// ```
/// use petir::linalg::{eigen_nonsymm, Matrix};
/// // a rotation generator: eigenvalues +-i
/// let a = Matrix::from_row_major(2, 2, vec![0.0, -1.0, 1.0, 0.0]).unwrap();
/// let ev = eigen_nonsymm(&a, false).unwrap();
/// assert_eq!((ev[0].re, ev[0].im.abs()), (0.0, 1.0));
/// assert_eq!(ev[1].im, -ev[0].im);
/// ```
pub fn eigen_nonsymm(a: &Matrix, balance: bool) -> Result<Vec<Complex64>> {
    let n = a.rows();
    if n != a.cols() {
        return Err(PetirError::NotSquare {
            rows: n,
            cols: a.cols(),
        });
    }
    let mut h = a.clone();
    if balance {
        balance_matrix(&mut h);
    }
    let mut tau = vec![0.0_f64; n];
    hessenberg_decomp(&mut h, &mut tau);

    // gsl_eigen_francis: zero the two subdiagonals below the main one
    for j in 0..n.saturating_sub(3) {
        h.set(j + 2, j, 0.0);
        h.set(j + 3, j, 0.0);
    }
    if n > 2 {
        h.set(n - 1, n - 3, 0.0);
    }

    let mut w = Francis {
        evals: Vec::with_capacity(n),
        n_iter: 0,
        max_iterations: 30 * n,
    };
    schur_decomp(&mut h, 0, n, &mut w);
    if w.evals.len() != n {
        return Err(PetirError::MaxIterations);
    }
    Ok(w.evals)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Companion matrix of (x-1)(x-2)(x-3)(x^2+1): eigenvalues 1, 2, 3, +-i,
    /// with and without balancing; trace and the conjugate pair checked.
    #[test]
    fn companion_matrix_eigenvalues() {
        // x^5 - 6x^4 + 12x^3 - 12x^2 + 11x - 6
        let c = [-6.0, 11.0, -12.0, 12.0, -6.0];
        let mut a = Matrix::zeros(5, 5).unwrap();
        for i in 1..5 {
            a.set(i, i - 1, 1.0);
        }
        for (i, ci) in c.iter().enumerate() {
            a.set(i, 4, -ci);
        }
        for bal in [false, true] {
            let ev = eigen_nonsymm(&a, bal).unwrap();
            assert_eq!(ev.len(), 5);
            let tr: f64 = ev.iter().map(|z| z.re).sum();
            assert!((tr - 6.0).abs() < 1e-12);
            let mut found = [false; 5];
            let want = [(1.0, 0.0), (2.0, 0.0), (3.0, 0.0), (0.0, 1.0), (0.0, -1.0)];
            for z in &ev {
                for (k, w) in want.iter().enumerate() {
                    if (z.re - w.0).abs() < 1e-9 && (z.im - w.1).abs() < 1e-9 {
                        found[k] = true;
                    }
                }
            }
            assert!(found.iter().all(|f| *f), "{ev:?}");
        }
    }

    /// The cyclic permutations (roots of unity) are the reference cases
    /// claimed to reach the exceptional shift. Other tests may run in
    /// parallel and add to the counter, so only an increase is asserted.
    #[test]
    fn cyclic_permutations_reach_the_exceptional_shift() {
        use core::sync::atomic::Ordering;
        for n in [3usize, 4] {
            let mut a = Matrix::zeros(n, n).unwrap();
            for i in 0..n {
                a.set(i, (i + 1) % n, 1.0);
            }
            let before = EXCEPTIONAL_SHIFTS.load(Ordering::Relaxed);
            let ev = eigen_nonsymm(&a, false).unwrap();
            assert_eq!(ev.len(), n);
            assert!(
                EXCEPTIONAL_SHIFTS.load(Ordering::Relaxed) > before,
                "n = {n}"
            );
            for z in &ev {
                assert!((z.re.hypot(z.im) - 1.0).abs() < 1e-12);
            }
        }
    }

    #[test]
    fn balancing_and_hypot_and_shapes() {
        let mut a = Matrix::from_row_major(2, 2, vec![1.0, 1e6, 1e-6, 1.0]).unwrap();
        let d = balance_matrix(&mut a);
        assert!(d.iter().all(|x| x.log2().fract() == 0.0));
        assert!((a.get(0, 1) / a.get(1, 0)).abs() < 4.0);
        assert_eq!(gsl_hypot(3.0, 4.0), 5.0);
        assert_eq!(gsl_hypot(f64::INFINITY, f64::NAN), f64::INFINITY);
        assert_eq!(gsl_hypot(0.0, -2.0), 2.0);
        assert!(eigen_nonsymm(&Matrix::zeros(2, 3).unwrap(), false).is_err());
        let one = Matrix::from_row_major(1, 1, vec![4.0]).unwrap();
        assert_eq!(eigen_nonsymm(&one, false).unwrap()[0].re, 4.0);
    }
}
