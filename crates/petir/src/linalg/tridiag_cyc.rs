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
// PORTED from the GNU Scientific Library (GSL) 2.8, commit
// cf180cd7fbd06039a577f9c9ff0b428784765ac1, read 2026-11-13.
// Copyright (C) 1996, 1997, 1998, 1999, 2000, 2002, 2004, 2007 Gerard Jungman,
// Brian Gough, David Necas
// GPL-3.0-or-later (verified from the per-file headers; see NOTICE).
//
//   linalg/tridiag.c  solve_cyc_tridiag (lines 200-322)
//                     gsl_linalg_solve_symm_cyc_tridiag (lines 519-550)

//! Symmetric cyclic tridiagonal solver, GSL's `gsl_linalg_solve_symm_cyc_tridiag`
//! (gh:#1341; needed by the periodic cubic spline).
//!
//! The matrix is symmetric tridiagonal with the two corner entries
//! `offdiag[n-1]` (see the picture in `tridiag.c`, after Engeln-Mullges &
//! Uhlig p. 96): row 0 is `diag[0] offdiag[0] 0 ... offdiag[n-1]`. `n >= 3`.
//!
//! As upstream, a zero pivot `alpha[i]` is remembered, the factorisation runs
//! on regardless (producing infinities) and `ZeroDivide` is reported at the
//! end; the solution is still returned by [`solve_symm_cyc_tridiag_with_status`].

use alloc::vec;
use alloc::vec::Vec;

use crate::{PetirError, Result};

fn at(v: &[f64], i: usize) -> f64 {
    v.get(i).copied().unwrap_or(f64::NAN)
}

fn put(v: &mut [f64], i: usize, x: f64) {
    if let Some(s) = v.get_mut(i) {
        *s = x;
    }
}

/// `gsl_linalg_solve_symm_cyc_tridiag`: solve the cyclic system.
///
/// # Errors
///
/// [`PetirError::LengthMismatch`] if the three slices differ in length;
/// [`PetirError::Invalid`] for `n < 3` (`GSL_EBADLEN`); [`PetirError::ZeroDivide`]
/// when a pivot is zero ("matrix must be positive definite").
///
/// # Example
///
/// ```
/// use petir::linalg::solve_symm_cyc_tridiag;
/// // [[4,1,1],[1,4,1],[1,1,4]] x = [6,6,6]  =>  x = [1,1,1]
/// let x = solve_symm_cyc_tridiag(&[4.0, 4.0, 4.0], &[1.0, 1.0, 1.0], &[6.0, 6.0, 6.0]).unwrap();
/// for xi in x {
///     assert!((xi - 1.0).abs() < 1e-14);
/// }
/// ```
pub fn solve_symm_cyc_tridiag(diag: &[f64], offdiag: &[f64], b: &[f64]) -> Result<Vec<f64>> {
    let (x, status) = solve_symm_cyc_tridiag_with_status(diag, offdiag, b)?;
    status.map(|()| x)
}

/// [`solve_symm_cyc_tridiag`], returning the solution upstream writes even
/// when it then reports `GSL_EZERODIV`. The outer `Err` is for the argument
/// errors, where GSL writes nothing.
pub fn solve_symm_cyc_tridiag_with_status(
    diag: &[f64],
    offdiag: &[f64],
    b: &[f64],
) -> Result<(Vec<f64>, Result<()>)> {
    let n = b.len();
    if diag.len() != n {
        return Err(PetirError::LengthMismatch {
            expected: n,
            found: diag.len(),
        });
    }
    if offdiag.len() != n {
        return Err(PetirError::LengthMismatch {
            expected: n,
            found: offdiag.len(),
        });
    }
    if n < 3 {
        return Err(PetirError::Invalid);
    }

    let mut delta = vec![0.0_f64; n];
    let mut gamma = vec![0.0_f64; n];
    let mut alpha = vec![0.0_f64; n];
    let mut c = vec![0.0_f64; n];
    let mut z = vec![0.0_f64; n];
    let mut x = vec![0.0_f64; n];
    let mut zero_div = false;

    // factor
    put(&mut alpha, 0, at(diag, 0));
    put(&mut gamma, 0, at(offdiag, 0) / at(&alpha, 0));
    put(&mut delta, 0, at(offdiag, n - 1) / at(&alpha, 0));

    if at(&alpha, 0) == 0.0 {
        zero_div = true;
    }

    for i in 1..(n - 2) {
        let a = at(diag, i) - at(offdiag, i - 1) * at(&gamma, i - 1);
        put(&mut alpha, i, a);
        put(&mut gamma, i, at(offdiag, i) / a);
        {
            let t = -at(&delta, i - 1) * at(offdiag, i - 1) / a;
            put(&mut delta, i, t);
        }
        if a == 0.0 {
            zero_div = true;
        }
    }

    let mut sum = 0.0;
    for i in 0..(n - 2) {
        sum += at(&alpha, i) * at(&delta, i) * at(&delta, i);
    }

    put(
        &mut alpha,
        n - 2,
        at(diag, n - 2) - at(offdiag, n - 3) * at(&gamma, n - 3),
    );

    put(
        &mut gamma,
        n - 2,
        (at(offdiag, n - 2) - at(offdiag, n - 3) * at(&delta, n - 3)) / at(&alpha, n - 2),
    );

    {
        let t = at(diag, n - 1) - sum - at(&alpha, n - 2) * at(&gamma, n - 2) * at(&gamma, n - 2);
        put(&mut alpha, n - 1, t);
    }

    // update
    put(&mut z, 0, at(b, 0));
    for i in 1..(n - 1) {
        {
            let t = at(b, i) - at(&z, i - 1) * at(&gamma, i - 1);
            put(&mut z, i, t);
        }
    }
    sum = 0.0;
    for i in 0..(n - 2) {
        sum += at(&delta, i) * at(&z, i);
    }
    {
        let t = at(b, n - 1) - sum - at(&gamma, n - 2) * at(&z, n - 2);
        put(&mut z, n - 1, t);
    }
    for i in 0..n {
        put(&mut c, i, at(&z, i) / at(&alpha, i));
    }

    // backsubstitution
    put(&mut x, n - 1, at(&c, n - 1));
    {
        let t = at(&c, n - 2) - at(&gamma, n - 2) * at(&x, n - 1);
        put(&mut x, n - 2, t);
    }
    if n >= 3 {
        let mut i = n - 3;
        for _ in 0..=(n - 3) {
            {
                let t = at(&c, i) - at(&gamma, i) * at(&x, i + 1) - at(&delta, i) * at(&x, n - 1);
                put(&mut x, i, t);
            }
            i = i.wrapping_sub(1);
        }
    }

    let status = if zero_div {
        Err(PetirError::ZeroDivide)
    } else {
        Ok(())
    };
    Ok((x, status))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn solves_a_known_system_and_refuses_bad_shapes() {
        let d = [4.0, 5.0, 6.0, 7.0];
        let o = [1.0, 2.0, 1.0, 0.5];
        let want = [1.0, -2.0, 3.0, 0.5];
        // A x with the cyclic structure, row i: o[i-1] x[i-1] + d[i] x[i] + o[i] x[i+1].
        let n = 4;
        let mut b = [0.0; 4];
        for i in 0..n {
            let prev = (i + n - 1) % n;
            let next = (i + 1) % n;
            b[i] = o[prev] * want[prev] + d[i] * want[i] + o[i] * want[next];
        }
        let x = solve_symm_cyc_tridiag(&d, &o, &b).unwrap();
        for (a, w) in x.iter().zip(want.iter()) {
            assert!((a - w).abs() < 1e-13, "{x:?}");
        }
        assert!(solve_symm_cyc_tridiag(&d[..2], &o[..2], &b[..2]).is_err());
        assert!(solve_symm_cyc_tridiag(&d, &o[..3], &b).is_err());
        let z = solve_symm_cyc_tridiag(&[0.0, 1.0, 1.0], &[0.0, 0.0, 0.0], &[1.0, 1.0, 1.0]);
        assert_eq!(z, Err(PetirError::ZeroDivide));
    }
}
