// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 Theodore Ong and the outram-park contributors.
//
// PORTED from the `faer` crate, version 0.24.4, file
// `src/sparse/linalg/lu.rs`, read from the published crates.io source on
// 2026-10-20.
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
// Ported items (lu.rs):
//   factorize_symbolic_lu (COLAMD, the column etree and AᵀA column counts,
//     the supernodal-or-simplicial decision, LU_SUPERNODAL_RATIO_FACTOR = 40,
//     and supernodal::factorize_supernodal_symbolic_lu)  -> SparseLu::factorize_with
//   simplicial::{depth_first_search, reach, l_incomplete_solve_sparse,
//     factorize_simplicial_numeric_lu}                  -> the private fns below
//   simplicial::SimplicialLu::solve_in_place_with_conj  -> SparseLu::solve_in_place
//   crate::perm::permute_rows (src/perm/mod.rs)          -> permute_rows
// Subscripts are checked accesses (see `super`'s module docs).

//! Sparse LU with partial (row) pivoting and a COLAMD column ordering — faer's
//! `sp_lu` on its simplicial path.

use super::colamd::{self, Control};
use super::etree::{col_etree, postorder};
use super::lu_supernodal::{self, SupernodalLu};
use super::supernodal::{column_counts_aat, factorize_supernodal_symbolic};
use super::{g, m, sl, triangular, CscMatrix, CscPattern, SymbolicKind};
use crate::{PetirError, Result};
use alloc::vec;
use alloc::vec::Vec;

/// A sparse `PAQ = LU` factorisation: `Q` from COLAMD, `P` from partial
/// pivoting, `L` unit lower, `U` upper. Port of faer 0.24.4's `sp_lu` on its
/// simplicial path (see the [module docs](super) for the deviations).
///
/// A numerically zero pivot is *not* an error, as in faer: the factorisation
/// succeeds and the solve propagates `inf`/`NaN`. A column with no candidate
/// pivot at all (structural singularity) is [`PetirError::Singular`].
#[derive(Debug, Clone)]
pub struct SparseLu {
    n: usize,
    col_perm_fwd: Vec<usize>,
    col_perm_inv: Vec<usize>,
    row_perm_fwd: Vec<usize>,
    l_col_ptr: Vec<usize>,
    l_row_idx: Vec<usize>,
    l_val: Vec<f64>,
    u_col_ptr: Vec<usize>,
    u_row_idx: Vec<usize>,
    u_val: Vec<f64>,
    /// The supernodal factors, when that path was taken (the simplicial
    /// fields above are then empty).
    supernodal: Option<SupernodalLu>,
}

/// faer's `LU_SUPERNODAL_RATIO_FACTOR`.
const LU_SUPERNODAL_RATIO_FACTOR: f64 = 40.0;

/// The partial `L` (its first `l_ncols` columns) that the reach walks.
struct PartialL<'a> {
    col_ptr: &'a [usize],
    row_idx: &'a [usize],
    ncols: usize,
}

fn depth_first_search(
    marked: &mut [usize],
    mark: usize,
    xi: &mut [usize],
    xi_len: usize,
    l: &PartialL<'_>,
    row_perm_inv: &[usize],
    b: usize,
    stack: &mut [usize],
) -> Result<usize> {
    let mut tail_start = xi_len;
    let mut head_len = 1usize;
    *m(xi, 0)? = b;
    'dfs_loop: while head_len > 0 {
        let b = g(xi, head_len - 1)?;
        let pb = g(row_perm_inv, b)?;
        let range = if pb < l.ncols {
            g(l.col_ptr, pb)?..g(l.col_ptr, pb + 1)?
        } else {
            0..0
        };
        if g(marked, b)? < mark {
            *m(marked, b)? = mark;
            *m(stack, head_len - 1)? = range.start;
        }
        let start = g(stack, head_len - 1)?;
        let end = range.end;
        for ptr in start..end {
            let i = g(l.row_idx, ptr)?;
            if g(marked, i)? == mark {
                continue;
            }
            *m(stack, head_len - 1)? = ptr;
            *m(xi, head_len)? = i;
            head_len += 1;
            continue 'dfs_loop;
        }
        head_len -= 1;
        tail_start = tail_start.checked_sub(1).ok_or(PetirError::Invalid)?;
        *m(xi, tail_start)? = b;
    }
    Ok(tail_start)
}

#[allow(clippy::too_many_arguments)]
fn reach(
    marked: &mut [usize],
    mark: usize,
    xi: &mut [usize],
    l: &PartialL<'_>,
    row_perm_inv: &[usize],
    bi: &[usize],
    stack: &mut [usize],
) -> Result<usize> {
    let mut tail_start = xi.len();
    for &b in bi {
        if g(marked, b)? < mark {
            tail_start =
                depth_first_search(marked, mark, xi, tail_start, l, row_perm_inv, b, stack)?;
        }
    }
    Ok(tail_start)
}

/// `dst[i] = src[perm[i]]` — faer's `permute_rows`.
fn permute_rows(x: &mut [f64], perm: &[usize]) -> Result<()> {
    let src: Vec<f64> = x.to_vec();
    for (xi, &p) in x.iter_mut().zip(perm) {
        *xi = g(&src, p)?;
    }
    Ok(())
}

impl SparseLu {
    /// Factorise a square sparse matrix (faer's `SparseColMat::sp_lu`): the
    /// supernodal path when faer would take it, the simplicial one otherwise.
    ///
    /// # Errors
    /// [`PetirError::NotSquare`]; [`PetirError::Singular`] when a column has
    /// no candidate pivot (faer's `LuError::SymbolicSingular`);
    /// [`PetirError::Invalid`] on a malformed matrix.
    pub fn factorize(a: &CscMatrix) -> Result<Self> {
        Self::factorize_with(a, SymbolicKind::Auto)
    }

    /// Factorise, choosing the numeric path explicitly
    /// ([`SymbolicKind::Auto`] is faer's own rule, as [`Self::factorize`]).
    ///
    /// # Errors
    /// As [`Self::factorize`].
    pub fn factorize_with(a: &CscMatrix, kind: SymbolicKind) -> Result<Self> {
        if a.nrows() != a.ncols() {
            return Err(PetirError::NotSquare {
                rows: a.nrows(),
                cols: a.ncols(),
            });
        }
        let n = a.ncols();
        let (col_perm_fwd, col_perm_inv) = colamd::order_vec(a.pattern(), Control::default())?;
        let mut lu = Self {
            n,
            col_perm_fwd,
            col_perm_inv,
            row_perm_fwd: vec![0; n],
            l_col_ptr: vec![0; n + 1],
            l_row_idx: Vec::new(),
            l_val: Vec::new(),
            u_col_ptr: vec![0; n + 1],
            u_row_idx: Vec::new(),
            u_val: Vec::new(),
            supernodal: None,
        };
        let supernode_ptr = match kind {
            SymbolicKind::Simplicial => None,
            _ => lu.symbolic_supernodal(a, kind)?,
        };
        match supernode_ptr {
            None => lu.factorize_numeric(a)?,
            Some(ptr) => {
                let at = lu_supernodal::transpose(a)?;
                let (f, row_perm, _) = lu_supernodal::factorize_numeric(
                    a,
                    &at,
                    &lu.col_perm_fwd,
                    &lu.col_perm_inv,
                    &ptr,
                )?;
                lu.row_perm_fwd = row_perm;
                lu.l_col_ptr = Vec::new();
                lu.u_col_ptr = Vec::new();
                lu.supernodal = Some(f);
            }
        }
        Ok(lu)
    }

    /// The rest of faer's `factorize_symbolic_lu` after COLAMD: the column
    /// elimination tree, its postorder, the column counts of `AᵀA`'s
    /// factor, the flop-ratio decision, and for the supernodal path the
    /// supernode partition. `None` means simplicial.
    fn symbolic_supernodal(&self, a: &CscMatrix, kind: SymbolicKind) -> Result<Option<Vec<usize>>> {
        let (mrows, n) = (a.nrows(), a.ncols());
        let at = lu_supernodal::transpose(a)?;
        let at_pat = CscPattern {
            nrows: n,
            ncols: mrows,
            col_ptr: &at.col_ptr,
            row_idx: &at.row_idx,
        };
        let mut etree = vec![0isize; n];
        col_etree(a.pattern(), Some(&self.col_perm_fwd), &mut etree)?;
        let mut post = vec![0usize; n];
        postorder(&mut post, &etree)?;
        let mut col_counts = vec![0usize; n];
        let mut min_row = vec![0isize; mrows];
        column_counts_aat(
            &mut col_counts,
            &mut min_row,
            at_pat,
            Some(&self.col_perm_inv),
            &etree,
            &post,
        )?;
        let supernodal = match kind {
            SymbolicKind::Supernodal => true,
            SymbolicKind::Simplicial => false,
            SymbolicKind::Auto => {
                let mut h_col_counts = vec![0usize; n];
                for &mc in &min_row {
                    if mc < 0 {
                        continue;
                    }
                    *m(&mut h_col_counts, mc as usize)? += 1;
                }
                for j in 0..n {
                    let parent = g(&etree, j)?;
                    if parent < 0 {
                        continue;
                    }
                    let hj = g(&h_col_counts, j)?.saturating_sub(1);
                    *m(&mut h_col_counts, parent as usize)? += hj;
                }
                let mut nnz = 0.0f64;
                let mut flops = 0.0f64;
                for (&h, &c) in h_col_counts.iter().zip(&col_counts) {
                    let hj = h as f64;
                    let rj = c as f64;
                    flops += hj + hj * rj;
                    nnz += hj + rj;
                }
                flops / nnz > 1.0 * LU_SUPERNODAL_RATIO_FACTOR
            }
        };
        if !supernodal {
            return Ok(None);
        }
        let sym = factorize_supernodal_symbolic(
            a.pattern(),
            Some((&self.col_perm_fwd, &min_row)),
            &etree,
            &col_counts,
        )?;
        Ok(Some(sym.supernode_begin))
    }

    /// Whether this factorisation took faer's supernodal path.
    pub fn is_supernodal(&self) -> bool {
        self.supernodal.is_some()
    }

    /// Port of faer 0.24.4 `simplicial::factorize_simplicial_numeric_lu`.
    fn factorize_numeric(&mut self, a: &CscMatrix) -> Result<()> {
        let mrows = a.nrows();
        let n = a.ncols();
        let mut x = vec![0.0f64; mrows];
        let mut marked = vec![0usize; mrows];
        let mut xj_buf = vec![0usize; mrows];
        let mut stack = vec![0usize; mrows];
        let mut row_perm_inv = vec![n; mrows];
        *m(&mut self.l_col_ptr, 0)? = 0;
        *m(&mut self.u_col_ptr, 0)? = 0;
        for j in 0..n {
            let pj = g(&self.col_perm_fwd, j)?;
            let bi = a.row_idx_of_col(pj)?;
            let bx = a.val_of_col(pj)?;
            // l_incomplete_solve_sparse, with L restricted to its first j columns.
            let l = PartialL {
                col_ptr: sl(&self.l_col_ptr, 0..j + 1)?,
                row_idx: &self.l_row_idx,
                ncols: j,
            };
            let tail_start = reach(
                &mut marked,
                j + 1,
                &mut xj_buf,
                &l,
                &row_perm_inv,
                bi,
                &mut stack,
            )?;
            for (&i, &b) in bi.iter().zip(bx) {
                *m(&mut x, i)? += b;
            }
            let xj = sl(&xj_buf, tail_start..xj_buf.len())?;
            for &i in xj {
                let pi = g(&row_perm_inv, i)?;
                if pi >= j {
                    continue;
                }
                let range = g(&self.l_col_ptr, pi)?..g(&self.l_col_ptr, pi + 1)?;
                let li = sl(&self.l_row_idx, range.clone())?;
                let lx = sl(&self.l_val, range)?;
                let xi = g(&x, i)?;
                for (&li, &lx) in li.iter().skip(1).zip(lx.iter().skip(1)) {
                    *m(&mut x, li)? -= lx * xi;
                }
            }
            let mut pivot_idx = n;
            let mut pivot_val = -1.0f64;
            for &i in xj {
                let xi = g(&x, i)?;
                if g(&row_perm_inv, i)? == n {
                    let val = xi.abs();
                    if matches!(
                        val.partial_cmp(&pivot_val),
                        None | Some(core::cmp::Ordering::Greater)
                    ) {
                        pivot_idx = i;
                        pivot_val = val;
                    }
                } else {
                    self.u_row_idx.push(g(&row_perm_inv, i)?);
                    self.u_val.push(xi);
                }
            }
            if pivot_idx == n {
                return Err(PetirError::Singular { col: j });
            }
            let x_piv = g(&x, pivot_idx)?;
            let x_piv_inv = 1.0 / x_piv;
            *m(&mut row_perm_inv, pivot_idx)? = j;
            self.u_row_idx.push(j);
            self.u_val.push(x_piv);
            *m(&mut self.u_col_ptr, j + 1)? = self.u_row_idx.len();
            self.l_row_idx.push(pivot_idx);
            self.l_val.push(1.0);
            for &i in xj {
                let xi = g(&x, i)?;
                if g(&row_perm_inv, i)? == n {
                    self.l_row_idx.push(i);
                    self.l_val.push(xi * x_piv_inv);
                }
                *m(&mut x, i)? = 0.0;
            }
            *m(&mut self.l_col_ptr, j + 1)? = self.l_row_idx.len();
        }
        for i in self.l_row_idx.iter_mut() {
            *i = g(&row_perm_inv, *i)?;
        }
        for (idx, &p) in row_perm_inv.iter().enumerate() {
            *m(&mut self.row_perm_fwd, p)? = idx;
        }
        Ok(())
    }

    /// Dimension of the factorised matrix.
    pub fn dim(&self) -> usize {
        self.n
    }

    /// Solve `A x = b` in place (`rhs` holds `b` on entry, `x` on return).
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
        permute_rows(rhs, &self.row_perm_fwd)?;
        if let Some(sn) = &self.supernodal {
            sn.solve_in_place(rhs)?;
            return permute_rows(rhs, &self.col_perm_inv);
        }
        triangular::lower_in_place(&self.l_col_ptr, &self.l_row_idx, &self.l_val, true, rhs)?;
        triangular::upper_in_place(&self.u_col_ptr, &self.u_row_idx, &self.u_val, false, rhs)?;
        permute_rows(rhs, &self.col_perm_inv)
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

    /// The COLAMD column permutation (forward: new column `j` is old column
    /// `col_perm()[j]`).
    pub fn col_perm(&self) -> &[usize] {
        &self.col_perm_fwd
    }

    /// The pivoting row permutation (forward).
    pub fn row_perm(&self) -> &[usize] {
        &self.row_perm_fwd
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn solves_a_small_unsymmetric_system() {
        // [[4,1,0],[2,5,1],[0,3,6]] x = [1,2,3]
        let a = CscMatrix::try_from_triplets(
            3,
            3,
            &[
                (0, 0, 4.0),
                (1, 0, 2.0),
                (0, 1, 1.0),
                (1, 1, 5.0),
                (2, 1, 3.0),
                (1, 2, 1.0),
                (2, 2, 6.0),
            ],
        )
        .unwrap();
        let lu = SparseLu::factorize(&a).unwrap();
        let x = lu.solve(&[1.0, 2.0, 3.0]).unwrap();
        let r = [
            4.0 * x[0] + x[1] - 1.0,
            2.0 * x[0] + 5.0 * x[1] + x[2] - 2.0,
            3.0 * x[1] + 6.0 * x[2] - 3.0,
        ];
        for ri in r {
            assert!(ri.abs() < 1e-14, "residual {ri}");
        }
        assert!(matches!(
            lu.solve(&[1.0]),
            Err(PetirError::LengthMismatch { .. })
        ));
    }

    #[test]
    fn an_empty_column_is_structurally_singular_and_a_rectangle_is_refused() {
        let a = CscMatrix::try_from_triplets(2, 2, &[(0, 0, 1.0), (1, 0, 1.0)]).unwrap();
        assert!(matches!(
            SparseLu::factorize(&a),
            Err(PetirError::Singular { .. })
        ));
        let r = CscMatrix::try_from_triplets(2, 3, &[(0, 0, 1.0)]).unwrap();
        assert!(matches!(
            SparseLu::factorize(&r),
            Err(PetirError::NotSquare { rows: 2, cols: 3 })
        ));
    }
}
