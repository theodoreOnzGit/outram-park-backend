// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 Theodore Ong and the outram-park contributors.
//
// PORTED from the `faer` crate, version 0.24.4, read from the published
// crates.io source on 2026-10-20.
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
// MIT into AGPL-3.0-only is a ONE-WAY flow. Maintainer direction for this
// port: gh #994, 2026-10-20. See the crate NOTICE.
//
// Ported items:
//   src/sparse/linalg/cholesky.rs  factorize_symbolic_cholesky (AMD ordering,
//     the permutation to the upper triangle, the column counts, and the
//     supernodal-or-simplicial decision; the supernodal path itself is in
//     cholesky_supernodal.rs)
//                                                  -> SparseLlt::factorize_lower
//   src/sparse/linalg/cholesky.rs  simplicial::ghost_factorize_simplicial_symbolic_cholesky
//   src/sparse/linalg/cholesky.rs  simplicial::factorize_simplicial_numeric_cholesky
//     (the Llt kind, no dynamic regularisation, as faer's default)
//   src/sparse/linalg/cholesky.rs  LltRef::solve_in_place_with_conj (simplicial arm)
//                                  and simplicial::SimplicialLltRef::solve_in_place_with_conj
//                                                  -> SparseLlt::solve_in_place
//   src/sparse/utils.rs            permute_self_adjoint_imp (unsorted, real)
//                                                  -> permute_self_adjoint_to_upper
//   src/sparse/linalg/mod.rs       CHOLESKY_SUPERNODAL_RATIO_FACTOR = 40.0
// Subscripts are checked accesses (see `super`'s module docs).

//! Sparse Cholesky `Pᵀ A P = L Lᵀ` — faer's `sp_cholesky(Side::Lower)` on its
//! simplicial path: AMD ordering, up-looking `LLᵀ`, two triangular solves.

#[allow(unused_imports)]
use crate::real::Real;

use super::amd::{self, Control};
use super::cholesky_supernodal::{self, permute_self_adjoint_to_lower};
use super::supernodal::{factorize_supernodal_symbolic, SymbolicSupernodal};
use super::SymbolicKind;
use super::etree::{ereach, prefactorize_symbolic_cholesky};
use super::{g, m, sl, triangular, CscMatrix, CscPattern};
use crate::{PetirError, Result};
use alloc::vec;
use alloc::vec::Vec;

/// faer's `CHOLESKY_SUPERNODAL_RATIO_FACTOR`.
const CHOLESKY_SUPERNODAL_RATIO_FACTOR: f64 = 40.0;

/// A sparse Cholesky factorisation `Pᵀ A P = L Lᵀ`, `P` from AMD. Port of
/// faer 0.24.4's `sp_cholesky(Side::Lower)` on its simplicial path (see the
/// [module docs](super) for the deviations and how they are measured).
#[derive(Debug, Clone)]
pub struct SparseLlt {
    n: usize,
    perm_fwd: Vec<usize>,
    perm_inv: Vec<usize>,
    l_col_ptr: Vec<usize>,
    l_row_idx: Vec<usize>,
    l_val: Vec<f64>,
    faer_supernodal: bool,
    /// The supernodal factor, when that path was taken (the simplicial
    /// fields are then empty).
    supernodal: Option<(SymbolicSupernodal, Vec<f64>)>,
}

/// The permuted matrix in upper-triangle CSC form.
type UpperCsc = (Vec<usize>, Vec<usize>, Vec<f64>);

/// `Pᵀ A P`, reading the LOWER triangle of `a` (entries with row >= col) and
/// writing the UPPER triangle, unsorted, exactly as faer's
/// `permute_self_adjoint_imp(.., in_side = Lower, out_side = Upper, sort =
/// false)` does for a real matrix.
fn permute_self_adjoint_to_upper(a: &CscMatrix, perm_inv: &[usize]) -> Result<UpperCsc> {
    let n = a.ncols();
    // in_side = Lower: cmp(i) = i.  out_side = Upper: cmp(i) = !i.
    let src = |i: usize| i;
    let dst = |i: usize| !i;
    let mut col_counts = vec![0usize; n];
    for old_j in 0..n {
        let new_j = g(perm_inv, old_j)?;
        let old_j_cmp = src(old_j);
        let new_j_cmp = dst(new_j);
        for &old_i in a.row_idx_of_col(old_j)? {
            let new_i = g(perm_inv, old_i)?;
            if src(old_i) >= old_j_cmp {
                let lower = dst(new_i) >= new_j_cmp;
                let new_j = if lower { new_j } else { new_i };
                *m(&mut col_counts, new_j)? += 1;
            }
        }
    }
    let mut new_col_ptr = vec![0usize; n + 1];
    let cur_row_pos = &mut col_counts;
    for j in 0..n {
        let ci0 = g(&new_col_ptr, j)?;
        *m(&mut new_col_ptr, j + 1)? = ci0 + g(cur_row_pos, j)?;
        *m(cur_row_pos, j)? = ci0;
    }
    let nnz = g(&new_col_ptr, n)?;
    let mut new_row_idx = vec![0usize; nnz];
    let mut new_val = vec![0.0f64; nnz];
    for old_j in 0..n {
        let new_j = g(perm_inv, old_j)?;
        let old_j_cmp = src(old_j);
        let new_j_cmp = dst(new_j);
        for (&old_i, &val) in a.row_idx_of_col(old_j)?.iter().zip(a.val_of_col(old_j)?) {
            let new_i = g(perm_inv, old_i)?;
            if src(old_i) >= old_j_cmp {
                let lower = dst(new_i) >= new_j_cmp;
                let (new_j, new_i) = if lower {
                    (new_j, new_i)
                } else {
                    (new_i, new_j)
                };
                let row_pos = g(cur_row_pos, new_j)?;
                *m(cur_row_pos, new_j)? += 1;
                *m(&mut new_val, row_pos)? = val;
                *m(&mut new_row_idx, row_pos)? = new_i;
            }
        }
    }
    Ok((new_col_ptr, new_row_idx, new_val))
}

impl SparseLlt {
    /// Factorise a symmetric positive-definite matrix given by its **lower
    /// triangle** (entries with `row >= col`; upper entries are ignored by the
    /// factorisation, as faer's `Side::Lower` does — but the AMD ordering sees
    /// whatever pattern is stored, so store only the lower triangle to
    /// reproduce faer).
    ///
    /// # Errors
    /// [`PetirError::NotSquare`]; [`PetirError::Domain`] when a pivot is not
    /// strictly positive, i.e. the matrix is not positive definite (faer's
    /// `LltError::NonPositivePivot`); [`PetirError::Invalid`] on a malformed
    /// matrix.
    pub fn factorize_lower(a: &CscMatrix) -> Result<Self> {
        Self::factorize_lower_with(a, SymbolicKind::Auto)
    }

    /// As [`Self::factorize_lower`], choosing the numeric path explicitly
    /// ([`SymbolicKind::Auto`] is faer's own rule).
    ///
    /// # Errors
    /// As [`Self::factorize_lower`].
    pub fn factorize_lower_with(a: &CscMatrix, kind: SymbolicKind) -> Result<Self> {
        if a.nrows() != a.ncols() {
            return Err(PetirError::NotSquare {
                rows: a.nrows(),
                cols: a.ncols(),
            });
        }
        let n = a.ncols();
        // --- factorize_symbolic_cholesky ---
        let mut perm_fwd = vec![0usize; n];
        let mut perm_inv = vec![0usize; n];
        let flops = amd::order_maybe_unsorted(
            &mut perm_fwd,
            &mut perm_inv,
            a.pattern(),
            Control::default(),
        )?;
        let (u_col_ptr, u_row_idx, u_val) = permute_self_adjoint_to_upper(a, &perm_inv)?;
        let pat = CscPattern {
            nrows: n,
            ncols: n,
            col_ptr: &u_col_ptr,
            row_idx: &u_row_idx,
        };
        let mut etree = vec![0isize; n];
        let mut col_counts = vec![0usize; n];
        prefactorize_symbolic_cholesky(&mut etree, &mut col_counts, pat)?;
        let l_nnz: usize = col_counts.iter().sum();
        // faer's AMD reports a zero flop count for n == 0; its fallback
        // branch (only for a non-AMD ordering) is never reached here.
        let flops = flops.n_div + flops.n_mult_subs_ldl;
        let faer_supernodal = (flops / l_nnz as f64) > 1.0 * CHOLESKY_SUPERNODAL_RATIO_FACTOR;
        let take_supernodal = match kind {
            SymbolicKind::Auto => faer_supernodal,
            SymbolicKind::Simplicial => false,
            SymbolicKind::Supernodal => true,
        };
        if take_supernodal {
            let sym = factorize_supernodal_symbolic(pat, None, &etree, &col_counts)?;
            let lower = permute_self_adjoint_to_lower(a, &perm_inv)?;
            let l = cholesky_supernodal::factorize_numeric(&lower, &sym)?;
            return Ok(Self {
                n,
                perm_fwd,
                perm_inv,
                l_col_ptr: Vec::new(),
                l_row_idx: Vec::new(),
                l_val: Vec::new(),
                faer_supernodal,
                supernodal: Some((sym, l)),
            });
        }
        // simplicial::ghost_factorize_simplicial_symbolic_cholesky
        let mut l_col_ptr = vec![0usize; n + 1];
        for (j, &cj) in col_counts.iter().enumerate() {
            *m(&mut l_col_ptr, j + 1)? = g(&l_col_ptr, j)? + cj;
        }
        let mut l_row_idx = vec![0usize; l_nnz];
        let mut ereach_stack = vec![0usize; n];
        let mut visited = vec![-1isize; n];
        {
            let mut current_row_idx: Vec<usize> = sl(&l_col_ptr, 0..n)?.to_vec();
            for k in 0..n {
                let top = ereach(&mut ereach_stack, pat, &etree, k, &mut visited)?;
                for &j in sl(&ereach_stack, top..n)? {
                    let cj = m(&mut current_row_idx, j)?;
                    let row_idx = *cj + 1;
                    *cj = row_idx;
                    *m(&mut l_row_idx, row_idx)? = k;
                }
                *m(&mut l_row_idx, g(&l_col_ptr, k)?)? = k;
            }
        }
        // --- factorize_numeric_llt -> simplicial numeric (Llt kind) ---
        let mut l_val = vec![0.0f64; l_nnz];
        let mut x = vec![0.0f64; n];
        visited.fill(-1);
        let mut current_row_idx: Vec<usize> = sl(&l_col_ptr, 0..n)?.to_vec();
        for k in 0..n {
            let top = ereach(&mut ereach_stack, pat, &etree, k, &mut visited)?;
            let range = g(&u_col_ptr, k)?..g(&u_col_ptr, k + 1)?;
            for (&i, &aik) in sl(&u_row_idx, range.clone())?
                .iter()
                .zip(sl(&u_val, range)?)
            {
                *m(&mut x, i)? += aik;
            }
            let mut d = g(&x, k)?;
            *m(&mut x, k)? = 0.0;
            for &j in sl(&ereach_stack, top..n)? {
                let j_start = g(&l_col_ptr, j)?;
                let cj = m(&mut current_row_idx, j)?;
                let row_idx = *cj + 1;
                *cj = row_idx;
                let mut xj = g(&x, j)?;
                *m(&mut x, j)? = 0.0;
                let dj = 1.0 / g(&l_val, j_start)?;
                let lkj = xj * dj;
                xj = lkj;
                let rows = sl(&l_row_idx, j_start + 1..row_idx)?;
                let vals = sl(&l_val, j_start + 1..row_idx)?;
                for (&i, &lij) in rows.iter().zip(vals) {
                    *m(&mut x, i)? -= lij * xj;
                }
                d -= lkj * xj;
                *m(&mut l_val, row_idx)? = lkj;
            }
            let k_start = g(&l_col_ptr, k)?;
            if !(d > 0.0) {
                return Err(PetirError::Domain);
            }
            *m(&mut l_val, k_start)? = Real::sqrt(d);
        }
        Ok(Self {
            n,
            perm_fwd,
            perm_inv,
            l_col_ptr,
            l_row_idx,
            l_val,
            faer_supernodal,
            supernodal: None,
        })
    }

    /// Dimension of the factorised matrix.
    pub fn dim(&self) -> usize {
        self.n
    }

    /// Whether faer 0.24.4 would have taken its **supernodal** path for this
    /// matrix (flops per factor nonzero above 40). When `false`, this
    /// factorisation and its solves are bit-identical to faer's; when `true`
    /// they agree only to rounding. See the [module docs](super).
    pub fn faer_would_use_supernodal(&self) -> bool {
        self.faer_supernodal
    }

    /// Whether this factorisation took the supernodal path.
    pub fn is_supernodal(&self) -> bool {
        self.supernodal.is_some()
    }

    /// Solve `A x = b` in place (`rhs` holds `b` on entry).
    ///
    /// # Errors
    /// [`PetirError::LengthMismatch`] if `rhs.len()` is not [`Self::dim`].
    pub fn solve_in_place(&self, rhs: &mut [f64]) -> Result<()> {
        if rhs.len() != self.n {
            return Err(PetirError::LengthMismatch {
                expected: self.n,
                found: rhs.len(),
            });
        }
        let mut x = Vec::with_capacity(self.n);
        for &f in &self.perm_fwd {
            x.push(g(rhs, f)?);
        }
        if let Some((sym, l)) = &self.supernodal {
            cholesky_supernodal::solve_in_place(sym, l, &mut x)?;
            for (r, &inv) in rhs.iter_mut().zip(&self.perm_inv) {
                *r = g(&x, inv)?;
            }
            return Ok(());
        }
        triangular::lower_in_place(&self.l_col_ptr, &self.l_row_idx, &self.l_val, false, &mut x)?;
        triangular::lower_transpose_in_place(
            &self.l_col_ptr,
            &self.l_row_idx,
            &self.l_val,
            false,
            &mut x,
        )?;
        for (r, &inv) in rhs.iter_mut().zip(&self.perm_inv) {
            *r = g(&x, inv)?;
        }
        Ok(())
    }

    /// Solve `A x = b`, returning `x`.
    ///
    /// # Errors
    /// As [`Self::solve_in_place`].
    pub fn solve(&self, rhs: &[f64]) -> Result<Vec<f64>> {
        let mut x = rhs.to_vec();
        self.solve_in_place(&mut x)?;
        Ok(x)
    }

    /// The AMD permutation (forward).
    pub fn perm(&self) -> &[usize] {
        &self.perm_fwd
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn solves_a_small_spd_system_and_rejects_an_indefinite_one() {
        // [[4,1,0],[1,3,1],[0,1,2]], lower triangle only.
        let a = CscMatrix::try_from_triplets(
            3,
            3,
            &[
                (0, 0, 4.0),
                (1, 0, 1.0),
                (1, 1, 3.0),
                (2, 1, 1.0),
                (2, 2, 2.0),
            ],
        )
        .unwrap();
        let llt = SparseLlt::factorize_lower(&a).unwrap();
        assert!(!llt.faer_would_use_supernodal());
        let x = llt.solve(&[1.0, 2.0, 3.0]).unwrap();
        let r = [
            4.0 * x[0] + x[1] - 1.0,
            x[0] + 3.0 * x[1] + x[2] - 2.0,
            x[1] + 2.0 * x[2] - 3.0,
        ];
        for ri in r {
            assert!(ri.abs() < 1e-14, "residual {ri}");
        }
        assert!(matches!(
            llt.solve(&[1.0]),
            Err(PetirError::LengthMismatch { .. })
        ));
        let b =
            CscMatrix::try_from_triplets(2, 2, &[(0, 0, 1.0), (1, 0, 2.0), (1, 1, 1.0)]).unwrap();
        assert_eq!(
            SparseLlt::factorize_lower(&b).unwrap_err(),
            PetirError::Domain
        );
        let r = CscMatrix::try_from_triplets(2, 3, &[(0, 0, 1.0)]).unwrap();
        assert!(matches!(
            SparseLlt::factorize_lower(&r),
            Err(PetirError::NotSquare { rows: 2, cols: 3 })
        ));
    }
}
