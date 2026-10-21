// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 Theodore Ong and the outram-park contributors.
//
// PORTED from the `faer` crate, version 0.24.4, file
// `src/sparse/linalg/cholesky.rs`, read from the published crates.io source
// on 2026-10-20.
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
// port: gh #994, 2026-10-20 ("Port lah"). See the crate NOTICE.
//
// Ported items (cholesky.rs, module `supernodal`):
//   factorize_supernodal_numeric_llt (no dynamic regularisation, faer's
//     default)                                        -> factorize_numeric
//   SupernodalLltRef::solve_in_place_with_conj        -> solve_in_place
//   src/sparse/utils.rs permute_self_adjoint_imp (in_side = out_side =
//     Lower, unsorted, real)                          -> permute_self_adjoint_to_lower
// Subscripts are checked accesses (see `super`'s module docs).

//! faer's **supernodal** sparse `LLᵀ`: dense supernode blocks factorised by
//! faer's dense `LLᵀ`, with descendants' updates applied by private-gemm's
//! lower-triangle scatter kernel, all reproduced in [`super::dense`].

use super::dense::factor::llt_in_place;
use super::dense::matmul::{matmul, spicy_matmul, DstKind};
use super::dense::{trsm, Accum, View};
use super::supernodal::SymbolicSupernodal;
use super::{g, m, sl, CscMatrix, NONE};
use crate::{PetirError, Result};
use alloc::vec;
use alloc::vec::Vec;

/// The permuted matrix in lower-triangle CSC form.
pub(crate) type LowerCsc = (Vec<usize>, Vec<usize>, Vec<f64>);

/// `Pᵀ A P`, reading the LOWER triangle of `a` and writing the LOWER
/// triangle, unsorted, as faer's `permute_self_adjoint_imp(.., in_side =
/// Lower, out_side = Lower, sort = false)` does for a real matrix.
pub(crate) fn permute_self_adjoint_to_lower(a: &CscMatrix, perm_inv: &[usize]) -> Result<LowerCsc> {
    let n = a.ncols();
    let mut col_counts = vec![0usize; n];
    for old_j in 0..n {
        let new_j = g(perm_inv, old_j)?;
        for &old_i in a.row_idx_of_col(old_j)? {
            let new_i = g(perm_inv, old_i)?;
            if old_i >= old_j {
                let c = if new_i >= new_j { new_j } else { new_i };
                *m(&mut col_counts, c)? += 1;
            }
        }
    }
    let mut col_ptr = vec![0usize; n + 1];
    for j in 0..n {
        let c0 = g(&col_ptr, j)?;
        *m(&mut col_ptr, j + 1)? = c0 + g(&col_counts, j)?;
        *m(&mut col_counts, j)? = c0;
    }
    let nnz = g(&col_ptr, n)?;
    let mut row_idx = vec![0usize; nnz];
    let mut val = vec![0.0f64; nnz];
    for old_j in 0..n {
        let new_j = g(perm_inv, old_j)?;
        for (&old_i, &v) in a.row_idx_of_col(old_j)?.iter().zip(a.val_of_col(old_j)?) {
            let new_i = g(perm_inv, old_i)?;
            if old_i >= old_j {
                let (c, r) = if new_i >= new_j {
                    (new_j, new_i)
                } else {
                    (new_i, new_j)
                };
                let p = g(&col_counts, c)?;
                *m(&mut col_counts, c)? = p + 1;
                *m(&mut val, p)? = v;
                *m(&mut row_idx, p)? = r;
            }
        }
    }
    Ok((col_ptr, row_idx, val))
}

/// One supernode's place in the factor.
struct Super {
    start: usize,
    ncols: usize,
    pattern: core::ops::Range<usize>,
    val0: usize,
}

fn supernode(sym: &SymbolicSupernodal, s: usize) -> Result<Super> {
    let start = g(&sym.supernode_begin, s)?;
    let end = g(&sym.supernode_begin, s + 1)?;
    Ok(Super {
        start,
        ncols: end - start,
        pattern: g(&sym.col_ptr_for_row_idx, s)?..g(&sym.col_ptr_for_row_idx, s + 1)?,
        val0: g(&sym.col_ptr_for_val, s)?,
    })
}

/// Port of faer 0.24.4 `factorize_supernodal_numeric_llt`. `lower` is the
/// permuted matrix's lower triangle; returns `L`'s values.
///
/// # Errors
/// [`PetirError::Domain`] on a pivot that is not strictly positive.
pub(crate) fn factorize_numeric(lower: &LowerCsc, sym: &SymbolicSupernodal) -> Result<Vec<f64>> {
    let (a_col_ptr, a_row_idx, a_val) = lower;
    let ns = sym.n_supernodes();
    let n = g(&sym.supernode_begin, ns)?;
    let len = g(&sym.col_ptr_for_val, ns)?;
    let mut l = vec![0.0f64; len];
    let mut global_to_local = vec![NONE; n];
    let mut ridx: Vec<usize> = Vec::new();
    let mut cidx: Vec<usize> = Vec::new();
    for s in 0..ns {
        let sn = supernode(sym, s)?;
        let s_end = sn.start + sn.ncols;
        let s_pattern = sl(&sym.row_idx, sn.pattern.clone())?;
        let s_nrows = s_pattern.len() + sn.ncols;
        for (i, &row) in s_pattern.iter().enumerate() {
            *m(&mut global_to_local, row)? = i + sn.ncols;
        }
        let ls = View::col_major(sn.val0, s_nrows, sn.ncols, s_nrows);
        for j in sn.start..s_end {
            let r = g(a_col_ptr, j)?..g(a_col_ptr, j + 1)?;
            for (&i, &v) in sl(a_row_idx, r.clone())?.iter().zip(sl(a_val, r)?) {
                if i < j {
                    continue;
                }
                let ix = if i >= s_end {
                    g(&global_to_local, i)?
                } else {
                    i - sn.start
                };
                let y = super::dense::wr(&mut l, ls.at(ix, j - sn.start))?;
                *y += v;
            }
        }
        for &d in sl(
            &sym.row_major_idx,
            g(&sym.row_major_ptr, s)?..g(&sym.row_major_ptr, s + 1)?,
        )? {
            let dn = supernode(sym, d)?;
            let d_pattern = sl(&sym.row_idx, dn.pattern.clone())?;
            let d_nrows = d_pattern.len() + dn.ncols;
            let ld = View::col_major(dn.val0, d_nrows, dn.ncols, d_nrows);
            let d_pattern_start = d_pattern.partition_point(|&i| i < sn.start);
            let rest = sl(d_pattern, d_pattern_start..d_pattern.len())?;
            let d_pattern_mid_len = rest.partition_point(|&i| i < s_end);
            let ld_mid_bot = ld.rows(dn.ncols + d_pattern_start, rest.len());
            let ld_mid = ld_mid_bot.rows(0, d_pattern_mid_len);
            ridx.clear();
            for (i, &row) in rest.iter().enumerate() {
                ridx.push(if i < d_pattern_mid_len {
                    row - sn.start
                } else {
                    g(&global_to_local, row)?
                });
            }
            cidx.clear();
            for &row in sl(rest, 0..d_pattern_mid_len)? {
                cidx.push(row - sn.start);
            }
            spicy_matmul(
                &mut l,
                ls,
                DstKind::Lower,
                Some(&ridx),
                Some(&cidx),
                Accum::Add,
                None,
                ld_mid_bot,
                None,
                ld_mid.t(),
                -1.0,
            )?;
        }
        let top = ls.rows(0, sn.ncols);
        let bot = ls.rows(sn.ncols, s_nrows - sn.ncols);
        llt_in_place(&mut l, top)?;
        trsm::lower(&mut l, None, top, bot.t())?;
        for &row in s_pattern {
            *m(&mut global_to_local, row)? = NONE;
        }
    }
    Ok(l)
}

/// faer `SupernodalLltRef::solve_in_place_with_conj` for one right-hand side
/// (already permuted).
pub(crate) fn solve_in_place(sym: &SymbolicSupernodal, l: &[f64], x: &mut [f64]) -> Result<()> {
    let n = x.len();
    let ns = sym.n_supernodes();
    let xv = |b: usize, len: usize| View::col_major(b, len, 1, n.max(1));
    let mut tmp: Vec<f64> = Vec::new();
    for s in 0..ns {
        let sn = supernode(sym, s)?;
        let pattern = sl(&sym.row_idx, sn.pattern.clone())?;
        let nrows = pattern.len() + sn.ncols;
        let ls = View::col_major(sn.val0, nrows, sn.ncols, nrows);
        let top = xv(sn.start, sn.ncols);
        trsm::lower(x, Some(l), ls.rows(0, sn.ncols), top)?;
        tmp.clear();
        tmp.resize(pattern.len(), 0.0);
        let k = pattern.len();
        matmul(
            &mut tmp,
            View::col_major(0, k, 1, k.max(1)),
            Accum::Replace,
            Some(l),
            ls.rows(sn.ncols, k),
            Some(x),
            top,
            1.0,
        )?;
        for (idx, &i) in pattern.iter().enumerate() {
            let t = g(&tmp, idx)?;
            let y = m(x, i)?;
            *y -= t;
        }
    }
    for s in (0..ns).rev() {
        let sn = supernode(sym, s)?;
        let pattern = sl(&sym.row_idx, sn.pattern.clone())?;
        let nrows = pattern.len() + sn.ncols;
        let ls = View::col_major(sn.val0, nrows, sn.ncols, nrows);
        let k = pattern.len();
        tmp.clear();
        for &i in pattern {
            tmp.push(g(x, i)?);
        }
        let top = xv(sn.start, sn.ncols);
        matmul(
            x,
            top,
            Accum::Add,
            Some(l),
            ls.rows(sn.ncols, k).t(),
            Some(&tmp),
            View::col_major(0, k, 1, k.max(1)),
            -1.0,
        )?;
        trsm::upper(x, Some(l), ls.rows(0, sn.ncols).t(), top)?;
    }
    if ns > 0 && g(&sym.supernode_begin, ns)? != n {
        return Err(PetirError::Invalid);
    }
    Ok(())
}
