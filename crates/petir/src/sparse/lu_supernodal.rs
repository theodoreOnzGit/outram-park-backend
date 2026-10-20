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
// port: gh #994, 2026-10-20 ("Port lah"). See the crate NOTICE.
//
// Ported items (lu.rs, module `supernodal`):
//   factorize_supernodal_numeric_lu (with its LPanel, UPanel and Front
//     phases)                                       -> factorize_numeric
//   SupernodalLu::{l_solve_in_place_with_conj, u_solve_in_place_with_conj}
//                                                   -> solve_in_place
//   crate::sparse::utils::transpose (values)        -> transpose
// Subscripts are checked accesses (see `super`'s module docs).

//! faer's **supernodal** sparse LU numeric factorisation and solve: dense
//! supernode panels factorised by faer's partial-pivoting dense LU and
//! updated by its dense GEMM, both reproduced in [`super::dense`].

use super::dense::factor::lu_in_place_recursion;
use super::dense::matmul::matmul;
use super::dense::{trsm, Accum, View};
use super::{g, m, sl, slm, CscMatrix, NONE};
use crate::{PetirError, Result};
use alloc::vec;
use alloc::vec::Vec;

/// A transposed CSC matrix (faer `utils::transpose`: sorted output).
pub(crate) struct Transposed {
    pub col_ptr: Vec<usize>,
    pub row_idx: Vec<usize>,
    pub val: Vec<f64>,
}

impl Transposed {
    fn col(&self, j: usize) -> Result<(&[usize], &[f64])> {
        let r = g(&self.col_ptr, j)?..g(&self.col_ptr, j + 1)?;
        Ok((sl(&self.row_idx, r.clone())?, sl(&self.val, r)?))
    }
}

/// faer `utils::transpose_imp`.
pub(crate) fn transpose(a: &CscMatrix) -> Result<Transposed> {
    let (mrows, n) = (a.nrows(), a.ncols());
    let mut count = vec![0usize; mrows];
    for &i in a.row_idx() {
        *m(&mut count, i)? += 1;
    }
    let mut col_ptr = vec![0usize; mrows + 1];
    for j in 0..mrows {
        let c = g(&count, j)?;
        let p = g(&col_ptr, j)?;
        *m(&mut col_ptr, j + 1)? = p + c;
        *m(&mut count, j)? = p;
    }
    let nnz = g(&col_ptr, mrows)?;
    let mut row_idx = vec![0usize; nnz];
    let mut val = vec![0.0f64; nnz];
    for j in 0..n {
        for (&i, &v) in a.row_idx_of_col(j)?.iter().zip(a.val_of_col(j)?) {
            let ci = g(&count, i)?;
            *m(&mut row_idx, ci)? = j;
            *m(&mut val, ci)? = v;
            *m(&mut count, i)? = ci + 1;
        }
    }
    Ok(Transposed {
        col_ptr,
        row_idx,
        val,
    })
}

/// One supernode's pending contribution block (faer's `contrib_work` tuple:
/// values, per-column active-row counts, active column count, and the `u8`
/// active mask).
#[derive(Debug, Clone, Default)]
struct Contrib {
    val: Vec<f64>,
    active: Vec<usize>,
    count: usize,
    mask: Vec<u8>,
    mask_nrows: usize,
}

impl Contrib {
    fn clear(&mut self) {
        self.val.clear();
        self.active = Vec::new();
        self.count = 0;
        self.mask = Vec::new();
        self.mask_nrows = 0;
    }
}

/// The numeric supernodal factors (faer `SupernodalLu`).
#[derive(Debug, Clone)]
pub(crate) struct SupernodalLu {
    supernode_ptr: Vec<usize>,
    l_col_ptr_for_row_idx: Vec<usize>,
    l_col_ptr_for_val: Vec<usize>,
    l_row_idx: Vec<usize>,
    l_val: Vec<f64>,
    ut_col_ptr_for_row_idx: Vec<usize>,
    ut_col_ptr_for_val: Vec<usize>,
    ut_row_idx: Vec<usize>,
    ut_val: Vec<f64>,
}

/// `partition_point(|i| i < idx)`.
fn pp(s: &[usize], idx: usize) -> usize {
    s.partition_point(|&i| i < idx)
}

/// Grow `v` to at least `n` (faer `resize_vec`, never shrinks).
fn grow<T: Clone>(v: &mut Vec<T>, n: usize, value: T) {
    if n > v.len() {
        v.resize(n, value);
    }
}

/// The ranges of supernode `d`'s row pattern (below its diagonal block) and
/// column pattern.
fn d_ranges(
    lu: &SupernodalLu,
    d: usize,
) -> Result<(core::ops::Range<usize>, core::ops::Range<usize>)> {
    let d_size = g(&lu.supernode_ptr, d + 1)? - g(&lu.supernode_ptr, d)?;
    let r0 = g(&lu.l_col_ptr_for_row_idx, d)?;
    let r1 = g(&lu.l_col_ptr_for_row_idx, d + 1)?;
    let c0 = g(&lu.ut_col_ptr_for_row_idx, d)?;
    let c1 = g(&lu.ut_col_ptr_for_row_idx, d + 1)?;
    Ok((r0 + d_size..r1, c0..c1))
}

/// Port of faer 0.24.4 `supernodal::factorize_supernodal_numeric_lu`.
/// Returns the factors and the row permutation (forward and inverse).
#[allow(clippy::too_many_lines)]
pub(crate) fn factorize_numeric(
    a: &CscMatrix,
    at: &Transposed,
    col_perm: &[usize],
    col_perm_inv: &[usize],
    supernode_ptr: &[usize],
) -> Result<(SupernodalLu, Vec<usize>, Vec<usize>)> {
    let mrows = a.nrows();
    let n = a.ncols();
    if mrows < n {
        return Err(PetirError::Invalid);
    }
    let n_super = supernode_ptr
        .len()
        .checked_sub(1)
        .ok_or(PetirError::Invalid)?;
    if g(supernode_ptr, n_super)? != n {
        return Err(PetirError::Invalid);
    }
    let mut lu = SupernodalLu {
        supernode_ptr: supernode_ptr.to_vec(),
        l_col_ptr_for_row_idx: vec![0; n_super + 1],
        l_col_ptr_for_val: vec![0; n_super + 1],
        l_row_idx: Vec::new(),
        l_val: Vec::new(),
        ut_col_ptr_for_row_idx: vec![0; n_super + 1],
        ut_col_ptr_for_val: vec![0; n_super + 1],
        ut_row_idx: Vec::new(),
        ut_val: Vec::new(),
    };
    let mut index_to_super = vec![0usize; n];
    let mut supernode_visited = vec![0usize; n_super];
    let mut u_compress_cols: Vec<Vec<usize>> = vec![Vec::new(); n_super];
    let mut l_compress_rows: Vec<Vec<usize>> = vec![Vec::new(); mrows];
    for s in 0..n_super {
        for x in slm(
            &mut index_to_super,
            g(supernode_ptr, s)?..g(supernode_ptr, s + 1)?,
        )? {
            *x = s;
        }
    }
    let mut col_global_to_local = vec![NONE; n];
    let mut row_global_to_local = vec![NONE; mrows];
    let mut marked = vec![0usize; mrows];
    let mut indices = vec![0usize; mrows];
    let mut transpositions = vec![0usize; mrows];
    let mut d_active_rows = vec![0usize; mrows];
    let mut row_perm: Vec<usize> = (0..mrows).collect();
    let mut row_perm_inv: Vec<usize> = (0..mrows).collect();
    let mut contrib: Vec<Contrib> = vec![Contrib::default(); n_super];
    let mut a_leftover = a.nnz();
    let mut sel: Vec<(usize, usize)> = Vec::new();

    for s in 0..n_super {
        let s_begin = g(supernode_ptr, s)?;
        let s_end = g(supernode_ptr, s + 1)?;
        let s_size = s_end - s_begin;
        let (left_contrib, right_contrib) = contrib.split_at_mut(s);
        let own = right_contrib.first_mut().ok_or(PetirError::Invalid)?;

        // --- the row pattern of supernode s ---
        let mut s_row_idx_count = 0usize;
        for j in s_begin..s_end {
            let pj = g(col_perm, j)?;
            for &i in a.row_idx_of_col(pj)? {
                let pi = g(&row_perm_inv, i)?;
                if pi < s_begin {
                    continue;
                }
                if g(&marked, i)? < 2 * s + 1 {
                    *m(&mut indices, s_row_idx_count)? = i;
                    s_row_idx_count += 1;
                    *m(&mut marked, i)? = 2 * s + 1;
                }
            }
        }
        for &d in g_vec(&u_compress_cols, s)? {
            let (rr, cr) = d_ranges(&lu, d)?;
            let d_row_idx = sl(&lu.l_row_idx, rr)?;
            let d_col_idx = sl(&lu.ut_row_idx, cr)?;
            let d_col_start = pp(d_col_idx, s_begin);
            if d_col_start < d_col_idx.len() && g(d_col_idx, d_col_start)? < s_end {
                for &i in d_row_idx {
                    let pi = g(&row_perm_inv, i)?;
                    if pi < s_begin {
                        continue;
                    }
                    if g(&marked, i)? < 2 * s + 1 {
                        *m(&mut indices, s_row_idx_count)? = i;
                        s_row_idx_count += 1;
                        *m(&mut marked, i)? = 2 * s + 1;
                    }
                }
            }
        }
        let l_r0 = g(&lu.l_col_ptr_for_row_idx, s)?;
        let l_r1 = l_r0 + s_row_idx_count;
        *m(&mut lu.l_col_ptr_for_row_idx, s + 1)? = l_r1;
        let l_v0 = g(&lu.l_col_ptr_for_val, s)?;
        let l_v1 = s_row_idx_count
            .checked_mul(s_size)
            .and_then(|x| x.checked_add(l_v0))
            .ok_or(PetirError::Invalid)?;
        *m(&mut lu.l_col_ptr_for_val, s + 1)? = l_v1;
        grow(&mut lu.l_row_idx, l_r1, 0);
        grow(&mut lu.l_val, l_v1, 0.0);
        {
            let dst = slm(&mut lu.l_row_idx, l_r0..l_r1)?;
            dst.copy_from_slice(sl(&indices, 0..s_row_idx_count)?);
            dst.sort_unstable();
        }
        for (idx, &i) in sl(&lu.l_row_idx, l_r0..l_r1)?.iter().enumerate() {
            *m(&mut row_global_to_local, i)? = idx;
        }
        let s_l = View::col_major(l_v0, s_row_idx_count, s_size, s_row_idx_count);
        for x in slm(&mut lu.l_val, l_v0..l_v1)? {
            *x = 0.0;
        }
        for j in s_begin..s_end {
            let pj = g(col_perm, j)?;
            for (&i, &val) in a.row_idx_of_col(pj)?.iter().zip(a.val_of_col(pj)?) {
                let pi = g(&row_perm_inv, i)?;
                if pi < s_begin {
                    continue;
                }
                a_leftover = a_leftover.checked_sub(1).ok_or(PetirError::Invalid)?;
                let ix = g(&row_global_to_local, i)?;
                let iy = j - s_begin;
                let y = super::dense::wr(&mut lu.l_val, s_l.at(ix, iy))?;
                *y += val;
            }
        }

        // --- LPanel: subtract descendants' pending updates from s_L ---
        for &d in g_vec(&u_compress_cols, s)? {
            let lc = left_contrib.get_mut(d).ok_or(PetirError::Invalid)?;
            if lc.val.is_empty() {
                continue;
            }
            let (rr, cr) = d_ranges(&lu, d)?;
            let d_row_idx = sl(&lu.l_row_idx, rr)?;
            let d_col_idx = sl(&lu.ut_row_idx, cr)?;
            let d_col_start = pp(d_col_idx, s_begin);
            if d_col_start < d_col_idx.len() && g(d_col_idx, d_col_start)? < s_end {
                let d_col_mid =
                    d_col_start + pp(sl(d_col_idx, d_col_start..d_col_idx.len())?, s_end);
                let ld = d_row_idx.len();
                let mrw = lc.mask_nrows;
                // The rows faer's inner loop keeps (`pi >= s_begin`), found
                // once per descendant instead of once per column: the same
                // updates, in the same order, to the same elements.
                sel.clear();
                for (d_i, &i) in d_row_idx.iter().enumerate() {
                    if g(&row_perm_inv, i)? >= s_begin {
                        sel.push((d_i, g(&row_global_to_local, i)?));
                    }
                }
                for (d_j, &j) in sl(d_col_idx, d_col_start..d_col_mid)?.iter().enumerate() {
                    if g(&lc.active, d_col_start + d_j)? > 0 {
                        let mut taken_rows = 0usize;
                        let s_j = j - s_begin;
                        for &(d_i, s_i) in &sel {
                            let p = d_i + (d_col_start + d_j) * ld;
                            let v = g(&lc.val, p)?;
                            let y = super::dense::wr(&mut lu.l_val, s_l.at(s_i, s_j))?;
                            *y -= v;
                            *m(&mut lc.val, p)? = 0.0;
                            let q = d_i + (d_j + d_col_start) * mrw;
                            taken_rows += g(&lc.mask, q)? as usize;
                            *m(&mut lc.mask, q)? = 0;
                        }
                        let a_ = m(&mut lc.active, d_col_start + d_j)?;
                        *a_ = a_.checked_sub(taken_rows).ok_or(PetirError::Invalid)?;
                        if *a_ == 0 {
                            lc.count = lc.count.checked_sub(1).ok_or(PetirError::Invalid)?;
                        }
                    }
                }
                if lc.count == 0 {
                    lc.clear();
                }
            }
        }
        if s_row_idx_count < s_size {
            return Err(PetirError::Singular {
                col: s_begin + s_row_idx_count,
            });
        }

        // --- dense partial-pivoting LU of the panel ---
        let trans = slm(&mut transpositions, s_begin..s_end)?;
        lu_in_place_recursion(&mut lu.l_val, s_l, 0, s_size, trans)?;
        {
            let s_row_idx = slm(&mut lu.l_row_idx, l_r0..l_r1)?;
            for (idx, &t) in trans.iter().enumerate() {
                let i_t = g(s_row_idx, idx + t)?;
                let kk = g(&row_perm_inv, i_t)?;
                if s_begin + idx >= mrows || kk >= mrows {
                    return Err(PetirError::Invalid);
                }
                row_perm.swap(s_begin + idx, kk);
                let (a0, a1) = (g(&row_perm, s_begin + idx)?, g(&row_perm, kk)?);
                if a0 >= mrows || a1 >= mrows {
                    return Err(PetirError::Invalid);
                }
                row_perm_inv.swap(a0, a1);
                if idx + t >= s_row_idx.len() {
                    return Err(PetirError::Invalid);
                }
                s_row_idx.swap(idx, idx + t);
            }
            for (idx, &t) in trans.iter().enumerate().rev() {
                let (r0, r1) = (g(s_row_idx, idx)?, g(s_row_idx, idx + t)?);
                if r0 >= mrows || r1 >= mrows {
                    return Err(PetirError::Invalid);
                }
                row_global_to_local.swap(r0, r1);
            }
        }
        for &i in sl(&lu.l_row_idx, l_r0..l_r1)? {
            let pi = g(&row_perm_inv, i)?;
            if pi >= s_end {
                l_compress_rows
                    .get_mut(i)
                    .ok_or(PetirError::Invalid)?
                    .push(s);
            }
        }

        // --- the column pattern of supernode s ---
        let mut s_col_index_count = 0usize;
        for i in s_begin..s_end {
            let pi = g(&row_perm, i)?;
            for &j in at.col(pi)?.0 {
                let pj = g(col_perm_inv, j)?;
                if pj < s_end {
                    continue;
                }
                if g(&marked, pj)? < 2 * s + 2 {
                    *m(&mut indices, s_col_index_count)? = pj;
                    s_col_index_count += 1;
                    *m(&mut marked, pj)? = 2 * s + 2;
                }
            }
        }
        for i in s_begin..s_end {
            let rp = g(&row_perm, i)?;
            for &d in g_vec(&l_compress_rows, rp)? {
                if g(&supernode_visited, d)? < 2 * s + 1 {
                    *m(&mut supernode_visited, d)? = 2 * s + 1;
                    if !left_contrib
                        .get(d)
                        .ok_or(PetirError::Invalid)?
                        .val
                        .is_empty()
                    {
                        let (_, cr) = d_ranges(&lu, d)?;
                        let d_col_idx = sl(&lu.ut_row_idx, cr)?;
                        let d_col_start = pp(d_col_idx, s_end);
                        for &j in sl(d_col_idx, d_col_start..d_col_idx.len())? {
                            if g(&marked, j)? < 2 * s + 2 {
                                *m(&mut indices, s_col_index_count)? = j;
                                s_col_index_count += 1;
                                *m(&mut marked, j)? = 2 * s + 2;
                            }
                        }
                    }
                }
            }
        }
        let u_r0 = g(&lu.ut_col_ptr_for_row_idx, s)?;
        let u_r1 = u_r0 + s_col_index_count;
        *m(&mut lu.ut_col_ptr_for_row_idx, s + 1)? = u_r1;
        let u_v0 = g(&lu.ut_col_ptr_for_val, s)?;
        let u_v1 = s_col_index_count
            .checked_mul(s_size)
            .and_then(|x| x.checked_add(u_v0))
            .ok_or(PetirError::Invalid)?;
        *m(&mut lu.ut_col_ptr_for_val, s + 1)? = u_v1;
        grow(&mut lu.ut_row_idx, u_r1, 0);
        grow(&mut lu.ut_val, u_v1, 0.0);
        {
            let dst = slm(&mut lu.ut_row_idx, u_r0..u_r1)?;
            dst.copy_from_slice(sl(&indices, 0..s_col_index_count)?);
            dst.sort_unstable();
        }
        for (idx, &j) in sl(&lu.ut_row_idx, u_r0..u_r1)?.iter().enumerate() {
            *m(&mut col_global_to_local, j)? = idx;
            let j_node = g(&index_to_super, j)?;
            if j_node > s {
                let list = u_compress_cols.get_mut(j_node).ok_or(PetirError::Invalid)?;
                if list.last().copied() != Some(s) {
                    list.push(s);
                }
            }
        }
        let s_u = View::col_major(u_v0, s_col_index_count, s_size, s_col_index_count).t();
        for x in slm(&mut lu.ut_val, u_v0..u_v1)? {
            *x = 0.0;
        }
        for i in s_begin..s_end {
            let pi = g(&row_perm, i)?;
            let (cols, vals) = at.col(pi)?;
            for (&j, &val) in cols.iter().zip(vals) {
                let pj = g(col_perm_inv, j)?;
                if pj < s_end {
                    continue;
                }
                a_leftover = a_leftover.checked_sub(1).ok_or(PetirError::Invalid)?;
                let ix = i - s_begin;
                let iy = g(&col_global_to_local, pj)?;
                let y = super::dense::wr(&mut lu.ut_val, s_u.at(ix, iy))?;
                *y += val;
            }
        }

        // --- UPanel: subtract descendants' pending updates from s_U ---
        for i in s_begin..s_end {
            let rp = g(&row_perm, i)?;
            for &d in g_vec(&l_compress_rows, rp)? {
                if g(&supernode_visited, d)? < 2 * s + 2 {
                    *m(&mut supernode_visited, d)? = 2 * s + 2;
                    let lc = left_contrib.get_mut(d).ok_or(PetirError::Invalid)?;
                    if lc.val.is_empty() {
                        continue;
                    }
                    let (rr, cr) = d_ranges(&lu, d)?;
                    let d_row_idx = sl(&lu.l_row_idx, rr)?;
                    let d_col_idx = sl(&lu.ut_row_idx, cr)?;
                    let d_col_start = pp(d_col_idx, s_end);
                    let ld = d_row_idx.len();
                    let mrw = lc.mask_nrows;
                    // faer's row test (`s_begin <= pi < s_end`), hoisted out
                    // of the column loop as in the L panel.
                    sel.clear();
                    for (d_i, &i) in d_row_idx.iter().enumerate() {
                        let pi = g(&row_perm_inv, i)?;
                        if pi >= s_begin && pi < s_end {
                            sel.push((d_i, g(&row_global_to_local, i)?));
                        }
                    }
                    for (d_j, &j) in sl(d_col_idx, d_col_start..d_col_idx.len())?
                        .iter()
                        .enumerate()
                    {
                        if g(&lc.active, d_col_start + d_j)? > 0 {
                            let mut taken_rows = 0usize;
                            let s_j = g(&col_global_to_local, j)?;
                            for &(d_i, s_i) in &sel {
                                let p = d_i + (d_col_start + d_j) * ld;
                                let v = g(&lc.val, p)?;
                                let y = super::dense::wr(&mut lu.ut_val, s_u.at(s_i, s_j))?;
                                *y -= v;
                                *m(&mut lc.val, p)? = 0.0;
                                let q = d_i + (d_j + d_col_start) * mrw;
                                taken_rows += g(&lc.mask, q)? as usize;
                                *m(&mut lc.mask, q)? = 0;
                            }
                            let a_ = m(&mut lc.active, d_col_start + d_j)?;
                            *a_ = a_.checked_sub(taken_rows).ok_or(PetirError::Invalid)?;
                            if *a_ == 0 {
                                lc.count = lc.count.checked_sub(1).ok_or(PetirError::Invalid)?;
                            }
                        }
                    }
                    if lc.count == 0 {
                        lc.clear();
                    }
                }
            }
        }
        trsm::unit_lower(&mut lu.ut_val, Some(&lu.l_val), s_l.rows(0, s_size), s_u)?;

        // --- Front: form s's own update block and gather descendants' ---
        if s_row_idx_count > s_size && s_col_index_count > 0 {
            let fr = s_row_idx_count - s_size;
            let len = fr
                .checked_mul(s_col_index_count)
                .ok_or(PetirError::Invalid)?;
            grow(&mut own.val, len, 0.0);
            own.active = vec![fr; s_col_index_count];
            own.count = s_col_index_count;
            own.mask = vec![1u8; len];
            own.mask_nrows = fr;
            let s_lu = View::col_major(0, fr, s_col_index_count, fr);
            matmul(
                &mut own.val,
                s_lu,
                Accum::Replace,
                Some(&lu.l_val),
                s_l.rows(s_size, fr),
                Some(&lu.ut_val),
                s_u,
                1.0,
            )?;
            for &d in g_vec(&u_compress_cols, s)? {
                let lc = left_contrib.get_mut(d).ok_or(PetirError::Invalid)?;
                if lc.val.is_empty() {
                    continue;
                }
                let (rr, cr) = d_ranges(&lu, d)?;
                let d_row_idx = sl(&lu.l_row_idx, rr)?;
                let d_col_idx = sl(&lu.ut_row_idx, cr)?;
                let mut contributes_to_front = false;
                for &i in d_row_idx {
                    if g(&row_perm_inv, i)? >= s_end {
                        contributes_to_front = true;
                        break;
                    }
                }
                if !contributes_to_front {
                    continue;
                }
                let d_col_start = pp(d_col_idx, s_end);
                let ld = d_row_idx.len();
                let mrw = lc.mask_nrows;
                let mut d_active_row_count = 0usize;
                let mut first_iter = true;
                for (d_j, &j) in sl(d_col_idx, d_col_start..d_col_idx.len())?
                    .iter()
                    .enumerate()
                {
                    if g(&lc.active, d_col_start + d_j)? > 0 {
                        if first_iter {
                            first_iter = false;
                            for (d_i, &i) in d_row_idx.iter().enumerate() {
                                let pi = g(&row_perm_inv, i)?;
                                if pi < s_end || g(&row_global_to_local, i)? == NONE {
                                    continue;
                                }
                                *m(&mut d_active_rows, d_active_row_count)? = d_i;
                                d_active_row_count += 1;
                            }
                        }
                        let mut taken_rows = 0usize;
                        let s_j = g(&col_global_to_local, j)?;
                        if s_j == NONE {
                            continue;
                        }
                        for &d_i in sl(&d_active_rows, 0..d_active_row_count)? {
                            let i = g(d_row_idx, d_i)?;
                            let q = d_i + (d_j + d_col_start) * mrw;
                            if g(&lc.mask, q)? == 0 {
                                continue;
                            }
                            let s_i = g(&row_global_to_local, i)?
                                .checked_sub(s_size)
                                .ok_or(PetirError::Invalid)?;
                            let p = d_i + (d_col_start + d_j) * ld;
                            let v = g(&lc.val, p)?;
                            let y = m(&mut own.val, s_i + s_j * fr)?;
                            *y += v;
                            *m(&mut lc.val, p)? = 0.0;
                            taken_rows += 1;
                            *m(&mut lc.mask, q)? = 0;
                        }
                        let a_ = m(&mut lc.active, d_col_start + d_j)?;
                        *a_ = a_.checked_sub(taken_rows).ok_or(PetirError::Invalid)?;
                        if *a_ == 0 {
                            lc.count = lc.count.checked_sub(1).ok_or(PetirError::Invalid)?;
                        }
                    }
                }
                if lc.count == 0 {
                    lc.clear();
                }
            }
        }
        for &i in sl(&lu.l_row_idx, l_r0..l_r1)? {
            *m(&mut row_global_to_local, i)? = NONE;
        }
        for &j in sl(&lu.ut_row_idx, u_r0..u_r1)? {
            *m(&mut col_global_to_local, j)? = NONE;
        }
    }
    if a_leftover != 0 {
        return Err(PetirError::Invalid);
    }
    let total = g(&lu.l_col_ptr_for_row_idx, n_super)?;
    for idx in slm(&mut lu.l_row_idx, 0..total)? {
        *idx = g(&row_perm_inv, *idx)?;
    }
    Ok((lu, row_perm, row_perm_inv))
}

fn g_vec<T>(v: &[Vec<T>], i: usize) -> Result<&[T]> {
    v.get(i).map(|x| x.as_slice()).ok_or(PetirError::Invalid)
}

impl SupernodalLu {
    /// faer `SupernodalLu::{l_solve, u_solve}_in_place_with_conj` for one
    /// right-hand side (already row-permuted).
    pub(crate) fn solve_in_place(&self, x: &mut [f64]) -> Result<()> {
        let n = x.len();
        let n_super = self.supernode_ptr.len().saturating_sub(1);
        let mut work = vec![0.0f64; n];
        let xv = |b: usize, len: usize| View::col_major(b, len, 1, n);
        for s in 0..n_super {
            let s_begin = g(&self.supernode_ptr, s)?;
            let s_size = g(&self.supernode_ptr, s + 1)? - s_begin;
            let r0 = g(&self.l_col_ptr_for_row_idx, s)?;
            let cnt = g(&self.l_col_ptr_for_row_idx, s + 1)? - r0;
            let l = View::col_major(g(&self.l_col_ptr_for_val, s)?, cnt, s_size, cnt);
            trsm::unit_lower(x, Some(&self.l_val), l.rows(0, s_size), xv(s_begin, s_size))?;
            matmul(
                &mut work,
                xv(0, cnt - s_size),
                Accum::Replace,
                Some(&self.l_val),
                l.rows(s_size, cnt - s_size),
                Some(x),
                xv(s_begin, s_size),
                1.0,
            )?;
            for (idx, &i) in sl(&self.l_row_idx, r0 + s_size..r0 + cnt)?
                .iter()
                .enumerate()
            {
                let w = g(&work, idx)?;
                let y = m(x, i)?;
                *y -= w;
            }
        }
        for s in (0..n_super).rev() {
            let s_begin = g(&self.supernode_ptr, s)?;
            let s_size = g(&self.supernode_ptr, s + 1)? - s_begin;
            let cnt = g(&self.l_col_ptr_for_row_idx, s + 1)? - g(&self.l_col_ptr_for_row_idx, s)?;
            let c0 = g(&self.ut_col_ptr_for_row_idx, s)?;
            let ccnt = g(&self.ut_col_ptr_for_row_idx, s + 1)? - c0;
            let l = View::col_major(g(&self.l_col_ptr_for_val, s)?, cnt, s_size, cnt);
            let u_right = View::col_major(g(&self.ut_col_ptr_for_val, s)?, ccnt, s_size, ccnt).t();
            for (idx, &i) in sl(&self.ut_row_idx, c0..c0 + ccnt)?.iter().enumerate() {
                *m(&mut work, idx)? = g(x, i)?;
            }
            matmul(
                x,
                xv(s_begin, s_size),
                Accum::Add,
                Some(&self.ut_val),
                u_right,
                Some(&work),
                xv(0, ccnt),
                -1.0,
            )?;
            trsm::upper(x, Some(&self.l_val), l.rows(0, s_size), xv(s_begin, s_size))?;
        }
        Ok(())
    }
}
