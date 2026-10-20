// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of PETIR, a component of OUTRAM PARK.
//
// Ported from the GNU Scientific Library 2.8 (vendored at
// `upstream_source/GSL`, commit cf180cd7fbd06039a577f9c9ff0b428784765ac1):
//
// - `linalg/lu.c` — `gsl_linalg_LU_decomp`, `LU_decomp_L2`, `singular`,
//   `gsl_linalg_LU_solve`, `gsl_linalg_LU_svx`.
//   Copyright (C) 1996, 1997, 1998, 1999, 2000, 2007, 2009 Gerard Jungman,
//   Brian Gough; Copyright (C) 2019, 2021 Patrick Alken.
// - `cblas/source_ger.h` (row-major branch) and `cblas/source_trsv_r.h`
//   (the two row-major `NoTrans` branches), the kernels `LU_decomp_L2` and
//   `LU_svx` reach through `gsl_blas_dger` / `gsl_blas_dtrsv`.
//   Copyright (C) 1996, 1997, 1998, 1999, 2000 Gerard Jungman.
// - `permutation/permute_source.c` — `gsl_permute`.
//   Copyright (C) 1996, 1997, 1998, 1999, 2000, 2007 Brian Gough.
//
// GSL is GPL-3.0-or-later; this port is distributed under GPL-3.0-only.
//
// PETIR is distributed in the hope that it will be useful, but WITHOUT ANY
// WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
// FOR A PARTICULAR PURPOSE.  See the GNU General Public License for more
// details.

//! GSL's LU factorisation with **unscaled** partial pivoting, and its solve.
//!
//! # Why this exists beside [`SquareMatrix::lu_decompose`](crate::linalg::SquareMatrix::lu_decompose)
//!
//! The two are different algorithms and give different bits:
//!
//! | | [`SquareMatrix`](crate::linalg::SquareMatrix) (OpenFOAM lift) | this module (GSL port) |
//! |---|---|---|
//! | form | Crout, inner products | right-looking Gaussian elimination (`DGETF2`) |
//! | pivot | **scaled** partial pivoting (`|a_ij| / max_k |a_ik|`), last row on ties | **unscaled** partial pivoting (`idamax`), first row on ties |
//! | multipliers | `l_ij * (1 / u_jj)` | `l_ij * (1 / u_jj)`, or `l_ij / u_jj` when `|u_jj| < DBL_MIN` |
//! | zero pivot | writes `f64::EPSILON` and carries on | leaves it zero, records the column, solve refuses |
//!
//! Unscaled partial pivoting with reciprocal multipliers is also what LAPACK's
//! `DGETF2` and ALGLIB's `rmatrixlu2` do, so a port of a code that called
//! either of those (DWSIM's `MathEx.SysLin.rsolve.rmatrixsolve`, gh:#992)
//! reproduces its upstream's pivot sequence with this module and not with the
//! OpenFOAM lift.
//!
//! # Scope cuts, each stated
//!
//! - **Square matrices only.** `gsl_linalg_LU_decomp` also factors an `M x N`
//!   rectangle (`M < N` goes through `dtrsm`); nothing here needs it. A
//!   non-square input is [`PetirError::Invalid`].
//! - **DEVIATION — the Level-2 path is used at every size.** GSL 2.8 calls
//!   `LU_decomp_L3` (`linalg/lu.c:218`), which recurses down to
//!   `LU_decomp_L2` only once `N <= CROSSOVER_LU = 24` (`linalg/recurse.h:7`).
//!   Above 24 it factors blocks with `dtrsm` and `dgemm`, which are not
//!   ported. The pivot rule is the same, so the factorisation is the same in
//!   exact arithmetic. Measured against compiled GSL (with GSL's own CBLAS)
//!   on a 30 x 30 case, the rounding is the same too: 930/930 values
//!   bit-identical, because GSL's reference `dtrsm`/`dgemm` loops perform the
//!   same rank-1 updates in the same order. That is one case, not a proof;
//!   see `tests/gsl_lu_code_to_code.rs`. At `N <= 24` this is GSL's own code
//!   path.
//! - **No `LU_refine`, `LU_invert`, `LU_det`.** The determinant and inverse
//!   in [`crate::linalg::lu`] are built on the OpenFOAM lift and are not
//!   duplicated here.
//!
//! # Units
//!
//! Bare dimensionless `f64`.

use alloc::vec;
use alloc::vec::Vec;

#[allow(unused_imports)]
use crate::real::Real;

use crate::linalg::blas1::{iamax, scal};
use crate::linalg::Matrix;
use crate::{PetirError, Result};

/// `GSL_DBL_MIN`, the smallest positive normal `f64` (`gsl_machine.h`).
const GSL_DBL_MIN: f64 = f64::MIN_POSITIVE;

/// The row permutation and status of a GSL LU factorisation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LuPermutation {
    /// GSL's `gsl_permutation` data: column `j` of `P` is column `perm[j]` of
    /// the identity, so `(P b)[i] = b[perm[i]]`.
    pub perm: Vec<usize>,
    /// `(-1)^(number of interchanges)`.
    pub signum: i32,
    /// `None` on success; otherwise the zero-based column `k` at which
    /// `U(k, k)` came out exactly zero — the **last** such column, because
    /// GSL overwrites its status at each one. GSL returns `k + 1` as the
    /// status and still completes the factorisation, and so does this port.
    pub zero_pivot: Option<usize>,
}

/// Factor the square matrix `a` in place as `P A = L U` — a port of
/// `gsl_linalg_LU_decomp` (`linalg/lu.c:62`) through `LU_decomp_L2`
/// (`linalg/lu.c:133`).
///
/// On return `a` holds `U` on and above the diagonal and the unit-lower `L`'s
/// multipliers strictly below it, exactly as GSL stores them.
///
/// # Errors
///
/// [`PetirError::Invalid`] if `a` is not square. An exactly-zero pivot is not
/// an error here — it is reported in [`LuPermutation::zero_pivot`], as GSL
/// reports it in its return status, and [`lu_svx`] then refuses to solve.
///
/// # Example
///
/// ```
/// use petir::linalg::{lu_decomp, lu_solve, Matrix};
/// let mut a = Matrix::from_row_major(2, 2, vec![1.0, 2.0, 3.0, 4.0]).unwrap();
/// let p = lu_decomp(&mut a).unwrap();
/// assert_eq!(p.perm, vec![1, 0], "row 1 has the larger first-column entry");
/// assert_eq!(p.signum, -1);
/// let x = lu_solve(&a, &p, &[5.0, 6.0]).unwrap();
/// assert!((x[0] + 4.0).abs() < 1e-14 && (x[1] - 4.5).abs() < 1e-14);
/// ```
pub fn lu_decomp(a: &mut Matrix) -> Result<LuPermutation> {
    let m = a.rows();
    let n = a.cols();
    if m != n {
        return Err(PetirError::Invalid);
    }
    let min_mn = m.min(n);
    let mut ipiv = vec![0_usize; min_mn];
    let zero_pivot = lu_decomp_l2(a, &mut ipiv);

    // Convert the ipiv swap list to a permutation (`linalg/lu.c:95-109`).
    let mut perm: Vec<usize> = (0..m).collect();
    let mut signum = 1_i32;
    for (i, &pivi) in ipiv.iter().enumerate() {
        let (Some(&dp), Some(&di)) = (perm.get(pivi), perm.get(i)) else {
            return Err(PetirError::Invalid);
        };
        if dp != di {
            perm.swap(pivi, i);
            signum = -signum;
        }
    }

    Ok(LuPermutation {
        perm,
        signum,
        zero_pivot,
    })
}

/// `LU_decomp_L2` (`linalg/lu.c:133-204`), LAPACK `DGETF2`. Returns the
/// zero-based column of the first exactly-zero pivot, GSL's `status - 1`.
fn lu_decomp_l2(a: &mut Matrix, ipiv: &mut [usize]) -> Option<usize> {
    let m = a.rows();
    let n = a.cols();
    let min_mn = m.min(n);
    let mut status: Option<usize> = None;

    for j in 0..min_mn {
        // Find the maximum in the j-th column (`gsl_blas_idamax` on the
        // subcolumn A(j:M, j)); ties take the first, as BLAS specifies.
        let v = a.column_from(j, j);
        let j_pivot = j + iamax(&v).unwrap_or(0);
        let ajpj = a.get(j_pivot, j);

        if let Some(slot) = ipiv.get_mut(j) {
            *slot = j_pivot;
        }

        if ajpj != 0.0 {
            if j_pivot != j {
                // Swap rows j and j_pivot (`gsl_blas_dswap`).
                for k in 0..n {
                    let t = a.get(j, k);
                    a.set(j, k, a.get(j_pivot, k));
                    a.set(j_pivot, k, t);
                }
            }

            // `j < M - 1`, written without the subtraction.
            if j + 1 < m {
                let ajj = a.get(j, j);
                let mut col = a.column_from(j, j + 1);
                if ajj.abs() >= GSL_DBL_MIN {
                    // `gsl_blas_dscal(1.0 / Ajj, v1)`.
                    scal(1.0 / ajj, &mut col);
                } else {
                    for x in col.iter_mut() {
                        *x /= ajj;
                    }
                }
                a.set_column_from(j, j + 1, &col);
            }
        } else {
            // GSL overwrites `status = j + 1` at every zero pivot, so its
            // return names the LAST one.
            status = Some(j);
        }

        if j + 1 < min_mn {
            // `gsl_blas_dger(-1.0, v1, v2, A22)` with v1 = A(j+1:M, j),
            // v2 = A(j, j+1:N). Row-major branch of `cblas/source_ger.h`:
            //   tmp = alpha * X[i];  A[i][j] += Y[j] * tmp;
            for i in (j + 1)..m {
                let tmp = -1.0 * a.get(i, j);
                for k in (j + 1)..n {
                    let updated = a.get(i, k) + a.get(j, k) * tmp;
                    a.set(i, k, updated);
                }
            }
        }
    }

    status
}

/// Solve `A x = b` in place from the factors, `x` holding `b` on entry — a
/// port of `gsl_linalg_LU_svx` (`linalg/lu.c:342-373`).
///
/// # Errors
///
/// - [`PetirError::Invalid`] if `lu` is not square or the permutation's length
///   does not match it.
/// - [`PetirError::LengthMismatch`] if `x.len()` does not match.
/// - [`PetirError::Singular`] if any `U(i, i)` is exactly zero (GSL's
///   `singular`, `linalg/lu.c:553`), naming the first such column.
pub fn lu_svx(lu: &Matrix, p: &LuPermutation, x: &mut [f64]) -> Result<()> {
    let n = lu.rows();
    if lu.cols() != n || p.perm.len() != n {
        return Err(PetirError::Invalid);
    }
    if x.len() != n {
        return Err(PetirError::LengthMismatch {
            expected: n,
            found: x.len(),
        });
    }
    if let Some(col) = (0..n).find(|&i| lu.get(i, i) == 0.0) {
        return Err(PetirError::Singular { col });
    }

    permute(&p.perm, x);

    // Forward substitution, L c = P b: `dtrsv(Lower, NoTrans, Unit)`,
    // row-major branch of `cblas/source_trsv_r.h`.
    for i in 1..n {
        let mut tmp = x.get(i).copied().unwrap_or(0.0);
        for j in 0..i {
            tmp -= lu.get(i, j) * x.get(j).copied().unwrap_or(0.0);
        }
        if let Some(slot) = x.get_mut(i) {
            *slot = tmp;
        }
    }

    // Back substitution, U x = c: `dtrsv(Upper, NoTrans, NonUnit)`.
    if let Some(last) = n.checked_sub(1) {
        if let Some(slot) = x.get_mut(last) {
            *slot /= lu.get(last, last);
        }
        for i in (0..last).rev() {
            let mut tmp = x.get(i).copied().unwrap_or(0.0);
            for j in (i + 1)..n {
                tmp -= lu.get(i, j) * x.get(j).copied().unwrap_or(0.0);
            }
            if let Some(slot) = x.get_mut(i) {
                *slot = tmp / lu.get(i, i);
            }
        }
    }
    Ok(())
}

/// Solve `A x = b` from the factors, leaving `b` untouched — a port of
/// `gsl_linalg_LU_solve` (`linalg/lu.c:304-338`).
///
/// # Errors
///
/// As [`lu_svx`].
pub fn lu_solve(lu: &Matrix, p: &LuPermutation, b: &[f64]) -> Result<Vec<f64>> {
    let mut x = b.to_vec();
    lu_svx(lu, p, &mut x)?;
    Ok(x)
}

/// `gsl_permute` (`permutation/permute_source.c:35-83`) with unit stride and
/// multiplicity one: `out[i] = in[perm[i]]`, in place, cycle by cycle.
fn permute(p: &[usize], data: &mut [f64]) {
    let n = p.len().min(data.len());
    let at = |k: usize| p.get(k).copied().unwrap_or(k);
    for i in 0..n {
        let mut k = at(i);
        while k > i {
            k = at(k);
        }
        if k < i {
            continue;
        }
        // Now k == i, the least index in its cycle.
        let mut pk = at(k);
        if pk == i {
            continue;
        }
        let t = data.get(i).copied().unwrap_or(0.0);
        while pk != i {
            let r1 = data.get(pk).copied().unwrap_or(0.0);
            if let Some(slot) = data.get_mut(k) {
                *slot = r1;
            }
            k = pk;
            pk = at(k);
        }
        if let Some(slot) = data.get_mut(k) {
            *slot = t;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mat(n: usize, rows: &[f64]) -> Matrix {
        Matrix::from_row_major(n, n, rows.to_vec()).unwrap()
    }

    #[test]
    fn solves_a_system_that_needs_a_row_swap() {
        // [[0,1,1],[1,0,1],[1,1,0]] x = [3,4,5] -> x = [3,2,1].
        let mut a = mat(3, &[0.0, 1.0, 1.0, 1.0, 0.0, 1.0, 1.0, 1.0, 0.0]);
        let p = lu_decomp(&mut a).unwrap();
        assert_eq!(p.zero_pivot, None);
        let x = lu_solve(&a, &p, &[3.0, 4.0, 5.0]).unwrap();
        for (got, want) in x.iter().zip([3.0, 2.0, 1.0]) {
            assert!((got - want).abs() < 1e-14, "{x:?}");
        }
    }

    #[test]
    fn ties_take_the_first_row_and_the_sign_counts_swaps() {
        // Column 0 is [1, -1]: |1| == |-1|, so idamax keeps row 0, no swap.
        let mut a = mat(2, &[1.0, 2.0, -1.0, 3.0]);
        let p = lu_decomp(&mut a).unwrap();
        assert_eq!(p.perm, vec![0, 1]);
        assert_eq!(p.signum, 1);
        // Reversed identity: one swap.
        let mut b = mat(2, &[0.0, 1.0, 1.0, 0.0]);
        assert_eq!(lu_decomp(&mut b).unwrap().signum, -1);
    }

    #[test]
    fn a_singular_matrix_factors_but_refuses_to_solve() {
        let mut a = mat(2, &[1.0, 2.0, 2.0, 4.0]);
        let p = lu_decomp(&mut a).unwrap();
        assert_eq!(p.zero_pivot, Some(1));
        assert_eq!(
            lu_solve(&a, &p, &[1.0, 2.0]).unwrap_err(),
            PetirError::Singular { col: 1 }
        );
    }

    #[test]
    fn shape_errors_are_reported() {
        let mut r = Matrix::zeros(2, 3).unwrap();
        assert_eq!(lu_decomp(&mut r).unwrap_err(), PetirError::Invalid);
        let mut a = mat(2, &[2.0, 0.0, 0.0, 2.0]);
        let p = lu_decomp(&mut a).unwrap();
        assert_eq!(
            lu_solve(&a, &p, &[1.0]).unwrap_err(),
            PetirError::LengthMismatch {
                expected: 2,
                found: 1
            }
        );
    }

    #[test]
    fn permute_matches_its_definition_on_a_three_cycle() {
        // out[i] = in[p[i]].
        let p = [2, 0, 1];
        let mut d = [10.0, 20.0, 30.0];
        permute(&p, &mut d);
        assert_eq!(d, [30.0, 10.0, 20.0]);
    }
}
