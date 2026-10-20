// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 Theodore Ong and the outram-park contributors.
//
// PORTED from the GNU Scientific Library (GSL) 2.8, commit
// cf180cd7fbd06039a577f9c9ff0b428784765ac1, read 2026-10-20.
// GPL-3.0-or-later (verified from each file's own header; see NOTICE).
//
//   eigen/symm.c    Copyright (C) 2001, 2007 Brian Gough
//     :86   gsl_eigen_symm
//   eigen/symmv.c   Copyright (C) 2001, 2007 Brian Gough
//     :100  gsl_eigen_symmv
//   eigen/qrstep.c  Copyright (C) 2007, 2010 Brian Gough
//     :24   chop_small_elements, :48 create_givens,
//     :72   trailing_eigenvalue, :99 qrstep
//   eigen/sort.c    Copyright (C) 1996-2000, 2006, 2007 Gerard Jungman,
//                   Patrick Alken
//     :42   gsl_eigen_symmv_sort

//! Eigenvalues and eigenvectors of a real symmetric matrix — GSL's
//! `eigen/symm.c` and `eigen/symmv.c`.
//!
//! # Algorithm
//!
//! Householder reduction to tridiagonal form ([`crate::linalg::SymmTd`]),
//! then implicit symmetric QR steps with Wilkinson shifts, deflating
//! off-diagonals below `DBL_EPSILON * (|d_i| + |d_(i+1)|)` (Golub & Van
//! Loan, 3rd ed., section 8.3). This is GSL's algorithm line for line.
//!
//! # Only the lower triangle is read
//!
//! As in GSL. An asymmetric input is not refused; its upper triangle is
//! ignored.
//!
//! # Order of the results
//!
//! **Unordered**, as GSL returns them. Call [`eigen_symmv_sort`] (GSL's
//! `gsl_eigen_symmv_sort`) for a defined order.
//!
//! # No iteration cap, as upstream
//!
//! GSL's loop has none: every QR step either deflates or moves the shift, and
//! a `NaN` subdiagonal deflates immediately. The port keeps that.
//!
//! # Units
//!
//! Bare dimensionless `f64`.

use alloc::vec;
use alloc::vec::Vec;

#[allow(unused_imports)]
use crate::real::Real;

use crate::linalg::elem::{rd, wr};
use crate::linalg::symmtd::{self, SymmTd};
use crate::linalg::Matrix;
use crate::scalar::DBL_EPSILON as GSL_DBL_EPSILON;
use crate::{PetirError, Result};

/// `chop_small_elements` (`qrstep.c:24`): zero off-diagonals negligible
/// against their neighbouring diagonal entries. `d` has `n` entries, `sd`
/// at least `n - 1`.
fn chop_small_elements(n: usize, d: &[f64], sd: &mut [f64]) {
    let mut d_i = rd(d, 0);
    for i in 0..n.saturating_sub(1) {
        let sd_i = rd(sd, i);
        let d_ip1 = rd(d, i + 1);
        if sd_i.abs() < GSL_DBL_EPSILON * (d_i.abs() + d_ip1.abs()) {
            wr(sd, i, 0.0);
        }
        d_i = d_ip1;
    }
}

/// `create_givens` (`qrstep.c:48`): `(c, s)` taking `(a, b)` to `(|v|, 0)`.
pub(crate) fn create_givens(a: f64, b: f64) -> (f64, f64) {
    if b == 0.0 {
        (1.0, 0.0)
    } else if b.abs() > a.abs() {
        let t = -a / b;
        let s1 = 1.0 / Real::sqrt(1.0 + t * t);
        (s1 * t, s1)
    } else {
        let t = -b / a;
        let c1 = 1.0 / Real::sqrt(1.0 + t * t);
        (c1, c1 * t)
    }
}

/// `trailing_eigenvalue` (`qrstep.c:72`): the Wilkinson shift.
fn trailing_eigenvalue(n: usize, d: &[f64], sd: &[f64]) -> f64 {
    let ta = rd(d, n - 2);
    let tb = rd(d, n - 1);
    let tab = rd(sd, n - 2);
    let dt = (ta - tb) / 2.0;
    if dt > 0.0 {
        tb - tab * (tab / (dt + Real::hypot(dt, tab)))
    } else if dt == 0.0 {
        tb - tab.abs()
    } else {
        tb + tab * (tab / ((-dt) + Real::hypot(dt, tab)))
    }
}

/// `qrstep` (`qrstep.c:99`): one implicit QR step on the unreduced block
/// `d[0..n]`, `sd[0..n-1]`, recording the rotations in `g` when asked.
fn qrstep(n: usize, d: &mut [f64], sd: &mut [f64], mut g: Option<(&mut [f64], &mut [f64])>) {
    let mut mu = trailing_eigenvalue(n, d, sd);

    // If mu is large relative to d_0 and sd_0 the rotation has no effect and
    // the loop would never end; upstream sets mu to zero.
    if GSL_DBL_EPSILON * mu.abs() > (rd(d, 0).abs() + rd(sd, 0).abs()) {
        mu = 0.0;
    }

    let mut x = rd(d, 0) - mu;
    let mut z = rd(sd, 0);

    let mut ak;
    let mut bk = 0.0_f64;
    let mut zk = 0.0_f64;

    let mut ap = rd(d, 0);
    let mut bp = rd(sd, 0);
    let mut aq = rd(d, 1);

    if n == 2 {
        let (c, s) = create_givens(x, z);
        if let Some((gc, gs)) = g.as_mut() {
            wr(gc, 0, c);
            wr(gs, 0, s);
        }
        let ap1 = c * (c * ap - s * bp) + s * (s * aq - c * bp);
        let bp1 = c * (s * ap + c * bp) - s * (s * bp + c * aq);
        let aq1 = s * (s * ap + c * bp) + c * (s * bp + c * aq);
        ak = ap1;
        bk = bp1;
        ap = aq1;
        wr(d, 0, ak);
        wr(sd, 0, bk);
        wr(d, 1, ap);
        return;
    }

    let mut bq = rd(sd, 1);

    let mut k = 0usize;
    while k < n - 1 {
        let (c, s) = create_givens(x, z);
        if let Some((gc, gs)) = g.as_mut() {
            wr(gc, k, c);
            wr(gs, k, s);
        }

        // compute G' T G
        let bk1 = c * bk - s * zk;

        let ap1 = c * (c * ap - s * bp) + s * (s * aq - c * bp);
        let bp1 = c * (s * ap + c * bp) - s * (s * bp + c * aq);
        let zp1 = -s * bq;

        let aq1 = s * (s * ap + c * bp) + c * (s * bp + c * aq);
        let bq1 = c * bq;

        ak = ap1;
        bk = bp1;
        zk = zp1;

        ap = aq1;
        bp = bq1;

        if k < n - 2 {
            aq = rd(d, k + 2);
        }
        if k < n - 3 {
            bq = rd(sd, k + 2);
        }

        wr(d, k, ak);

        if k > 0 {
            wr(sd, k - 1, bk1);
        }

        if k < n - 2 {
            wr(sd, k + 1, bp);
        }

        x = bk;
        z = zk;
        k += 1;
    }

    // k = n - 1
    wr(d, k, ap);
    wr(sd, k - 1, bk);
}

/// Shared driver for `gsl_eigen_symm` and `gsl_eigen_symmv`.
///
/// `q` is `Some` (the unpacked `Q`) when vectors are wanted; the two upstream
/// routines differ in exactly two places, both kept: `symmv` applies each
/// block's rotations to `Q`, and it chops the **whole** matrix after a step
/// where `symm` chops only the block.
fn reduce(d: &mut [f64], sd: &mut [f64], mut q: Option<&mut Matrix>) -> Result<()> {
    let n = d.len();
    chop_small_elements(n, d, sd);

    let mut gc = vec![0.0_f64; n];
    let mut gs = vec![0.0_f64; n];

    let mut b = n - 1;
    while b > 0 {
        let sdb = rd(sd, b - 1);
        if sdb == 0.0 || sdb.is_nan() {
            b -= 1;
            continue;
        }

        // Find the largest unreduced block (a, b) working backwards.
        let mut a = b - 1;
        while a > 0 {
            if rd(sd, a - 1) == 0.0 {
                break;
            }
            a -= 1;
        }

        let n_block = b - a + 1;
        let (Some(d_block), Some(sd_block)) = (d.get_mut(a..=b), sd.get_mut(a..b)) else {
            return Err(PetirError::Invalid);
        };

        match q.as_mut() {
            None => {
                qrstep(n_block, d_block, sd_block, None);
                chop_small_elements(n_block, d_block, sd_block);
            }
            Some(qm) => {
                qrstep(n_block, d_block, sd_block, Some((&mut gc, &mut gs)));
                // Q <- Q G
                let nq = qm.rows();
                for i in 0..(n_block - 1) {
                    let (c, s) = (rd(&gc, i), rd(&gs, i));
                    for k in 0..nq {
                        let qki = qm.get(k, a + i);
                        let qkj = qm.get(k, a + i + 1);
                        qm.set(k, a + i, qki * c - qkj * s);
                        qm.set(k, a + i + 1, qki * s + qkj * c);
                    }
                }
                chop_small_elements(n, d, sd);
            }
        }
    }
    Ok(())
}

/// Eigenvalues of the symmetric matrix `a` (lower triangle read), unordered.
/// `gsl_eigen_symm` (`eigen/symm.c:86`).
///
/// # Errors
///
/// [`PetirError::NotSquare`] if `a` is not square.
///
/// # Example
///
/// ```
/// use petir::linalg::{eigen_symm, Matrix};
/// let a = Matrix::from_row_major(2, 2, vec![2.0, 1.0, 1.0, 2.0]).unwrap();
/// let mut ev = eigen_symm(&a).unwrap();
/// ev.sort_by(f64::total_cmp);
/// assert!((ev[0] - 1.0).abs() < 1e-15 && (ev[1] - 3.0).abs() < 1e-15);
/// ```
pub fn eigen_symm(a: &Matrix) -> Result<Vec<f64>> {
    let n = a.rows();
    if n != a.cols() {
        return Err(PetirError::NotSquare {
            rows: n,
            cols: a.cols(),
        });
    }
    if n == 1 {
        return Ok(vec![a.get(0, 0)]);
    }
    let mut packed = a.clone();
    let mut tau = vec![0.0; n - 1];
    symmtd::decomp(&mut packed, &mut tau);
    let mut d: Vec<f64> = (0..n).map(|i| packed.get(i, i)).collect();
    let mut sd: Vec<f64> = (0..n - 1).map(|i| packed.get(i + 1, i)).collect();
    reduce(&mut d, &mut sd, None)?;
    Ok(d)
}

/// Eigenvalues and orthonormal eigenvectors (as the **columns** of the
/// returned matrix) of the symmetric matrix `a`, unordered.
/// `gsl_eigen_symmv` (`eigen/symmv.c:100`).
///
/// # Errors
///
/// [`PetirError::NotSquare`] if `a` is not square.
pub fn eigen_symmv(a: &Matrix) -> Result<(Vec<f64>, Matrix)> {
    let n = a.rows();
    if n != a.cols() {
        return Err(PetirError::NotSquare {
            rows: n,
            cols: a.cols(),
        });
    }
    if n == 1 {
        let mut v = Matrix::zeros(1, 1)?;
        v.set(0, 0, 1.0);
        return Ok((vec![a.get(0, 0)], v));
    }
    let td = SymmTd::new(a)?;
    let (mut q, mut d, mut sd) = td.unpack()?;
    reduce(&mut d, &mut sd, Some(&mut q))?;
    Ok((d, q))
}

/// Sort order for [`eigen_symmv_sort`], GSL's `gsl_eigen_sort_t`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EigenSort {
    /// `GSL_EIGEN_SORT_VAL_ASC`: ascending value.
    ValAsc,
    /// `GSL_EIGEN_SORT_VAL_DESC`: descending value.
    ValDesc,
    /// `GSL_EIGEN_SORT_ABS_ASC`: ascending magnitude.
    AbsAsc,
    /// `GSL_EIGEN_SORT_ABS_DESC`: descending magnitude.
    AbsDesc,
}

/// Sort eigenvalues, and the eigenvector columns with them, in place.
/// `gsl_eigen_symmv_sort` (`eigen/sort.c:42`): upstream's selection sort,
/// whose ties keep the earlier index. No arithmetic.
///
/// # Errors
///
/// [`PetirError::LengthMismatch`] if `vectors` does not have
/// `values.len()` columns.
pub fn eigen_symmv_sort(values: &mut [f64], vectors: &mut Matrix, sort: EigenSort) -> Result<()> {
    if vectors.cols() != values.len() {
        return Err(PetirError::LengthMismatch {
            expected: values.len(),
            found: vectors.cols(),
        });
    }
    let n = values.len();
    for i in 0..n.saturating_sub(1) {
        let mut k = i;
        let mut ek = rd(values, i);
        for j in (i + 1)..n {
            let ej = rd(values, j);
            let test = match sort {
                EigenSort::ValAsc => ej < ek,
                EigenSort::ValDesc => ej > ek,
                EigenSort::AbsAsc => ej.abs() < ek.abs(),
                EigenSort::AbsDesc => ej.abs() > ek.abs(),
            };
            if test {
                k = j;
                ek = ej;
            }
        }
        if k != i {
            values.swap(i, k);
            for r in 0..vectors.rows() {
                let t = vectors.get(r, i);
                vectors.set(r, i, vectors.get(r, k));
                vectors.set(r, k, t);
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sym4() -> Matrix {
        Matrix::from_row_major(
            4,
            4,
            vec![
                4.0, 1.0, -2.0, 2.0, 1.0, 2.0, 0.0, 1.0, -2.0, 0.0, 3.0, -2.0, 2.0, 1.0, -2.0, -1.0,
            ],
        )
        .unwrap()
    }

    /// `A v = lambda v` for every pair, the vectors are orthonormal, values
    /// agree between the two entry points, and sorting orders them.
    #[test]
    fn eigenpairs_satisfy_the_definition() {
        let a = sym4();
        let (mut vals, mut vecs) = eigen_symmv(&a).unwrap();
        let only = eigen_symm(&a).unwrap();
        let mut s1 = vals.clone();
        let mut s2 = only.clone();
        s1.sort_by(f64::total_cmp);
        s2.sort_by(f64::total_cmp);
        for (x, y) in s1.iter().zip(&s2) {
            assert!((x - y).abs() < 1e-13);
        }
        eigen_symmv_sort(&mut vals, &mut vecs, EigenSort::ValAsc).unwrap();
        assert!(vals.windows(2).all(|w| w[0] <= w[1]));
        for j in 0..4 {
            for i in 0..4 {
                let mut av = 0.0;
                for k in 0..4 {
                    av += a.get(i, k) * vecs.get(k, j);
                }
                assert!((av - vals[j] * vecs.get(i, j)).abs() < 1e-13);
            }
        }
        // trace is preserved
        let tr: f64 = vals.iter().sum();
        assert!((tr - 8.0).abs() < 1e-13);
        for sort in [EigenSort::ValDesc, EigenSort::AbsAsc, EigenSort::AbsDesc] {
            eigen_symmv_sort(&mut vals, &mut vecs, sort).unwrap();
        }
        assert!(vals.windows(2).all(|w| w[0].abs() >= w[1].abs()));
    }

    #[test]
    fn one_by_one_and_bad_shapes() {
        let a = Matrix::from_row_major(1, 1, vec![7.0]).unwrap();
        assert_eq!(eigen_symm(&a).unwrap(), vec![7.0]);
        assert_eq!(eigen_symmv(&a).unwrap().1.get(0, 0), 1.0);
        let r = Matrix::zeros(2, 3).unwrap();
        assert!(eigen_symm(&r).is_err());
        assert!(eigen_symmv(&r).is_err());
        let mut v = Matrix::zeros(2, 2).unwrap();
        assert!(eigen_symmv_sort(&mut [1.0], &mut v, EigenSort::ValAsc).is_err());
    }
}
