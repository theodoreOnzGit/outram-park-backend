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
// port: gh #994, 2026-10-20 ("Port lah"). See the crate NOTICE.
//
// Ported items:
//   src/sparse/linalg/qr.rs  ghost_least_common_ancestor   -> least_common_ancestor
//                            ghost_column_counts_aat       -> column_counts_aat
//   src/sparse/linalg/cholesky.rs  supernodal::{ereach_super, ereach_super_ata,
//     ghost_factorize_supernodal_symbolic (fundamental supernodes, relaxed
//     amalgamation with DEFAULT_RELAX, the row structure)}  -> factorize_supernodal_symbolic
//   src/sparse/linalg/mod.rs  DEFAULT_RELAX
//   src/sparse/utils.rs  transpose_dedup_imp (pattern only) -> the supernode row-major structure
// The supernode postorder and descendant counts faer computes at the end of
// ghost_factorize_supernodal_symbolic are not stored in its result and are
// not computed here.
// Subscripts are checked accesses (see `super`'s module docs).

//! **Supernodal symbolic analysis**: the column counts of `AᵀA`'s Cholesky
//! factor (for LU) and the relaxed supernode partition with each supernode's
//! row pattern, shared by the supernodal LU and `LLᵀ`.

use super::{g, m, CscPattern, NONE};
use crate::{PetirError, Result};
use alloc::vec;
use alloc::vec::Vec;

/// faer's `DEFAULT_RELAX`: merge two adjacent supernodes of `n` columns in
/// total when the fraction of explicit zeros stays below the paired bound.
const DEFAULT_RELAX: [(usize, f64); 4] = [(4, 1.0), (16, 0.8), (48, 0.1), (usize::MAX, 0.05)];

/// faer `ghost_least_common_ancestor`; `-1`/`NONE` as signed `-1`.
fn least_common_ancestor(
    i: usize,
    j: usize,
    first: &[isize],
    max_first: &mut [isize],
    prev_leaf: &mut [isize],
    ancestor: &mut [usize],
) -> Result<isize> {
    if i <= j || g(first, j)? <= g(max_first, i)? {
        return Ok(-2);
    }
    *m(max_first, i)? = g(first, j)?;
    let j_prev = g(prev_leaf, i)?;
    *m(prev_leaf, i)? = j as isize;
    if j_prev < 0 {
        return Ok(-1);
    }
    let j_prev = j_prev as usize;
    let mut lca = j_prev;
    while lca != g(ancestor, lca)? {
        lca = g(ancestor, lca)?;
    }
    let mut node = j_prev;
    while node != lca {
        let next = g(ancestor, node)?;
        *m(ancestor, node)? = lca;
        node = next;
    }
    Ok(lca as isize)
}

/// faer `ghost_column_counts_aat`: column counts of the Cholesky factor of
/// `A Aᵀ` (`col_counts`, `a.nrows` long) with rows of `a` renumbered by
/// `row_perm_inv`, and per column of `a` its smallest renumbered row
/// (`min_row`, `a.ncols` long, `-1` for an empty column).
pub(crate) fn column_counts_aat(
    col_counts: &mut [usize],
    min_row: &mut [isize],
    a: CscPattern<'_>,
    row_perm_inv: Option<&[usize]>,
    etree: &[isize],
    post: &[usize],
) -> Result<()> {
    let mm = a.nrows;
    let nn = a.ncols;
    let inv = |i: usize| -> Result<usize> {
        match row_perm_inv {
            Some(p) => g(p, i),
            None => Ok(i),
        }
    };
    let mut delta = vec![0isize; mm];
    let mut post_inv = vec![0usize; mm];
    for j in 0..mm {
        *m(&mut post_inv, g(post, j)?)? = j;
    }
    let mut next = vec![-1isize; nn];
    let mut head = vec![-1isize; mm];
    for j in 0..nn {
        let mut mr: Option<usize> = None;
        let mut mp: Option<usize> = None;
        for &i in a.col(j)? {
            let r = inv(i)?;
            mr = Some(mr.map_or(r, |x| x.min(r)));
            let p = g(&post_inv, r)?;
            mp = Some(mp.map_or(p, |x| x.min(p)));
        }
        *m(min_row, j)? = mr.map_or(-1, |x| x as isize);
        if let Some(p) = mp {
            let h = m(&mut head, p)?;
            *m(&mut next, j)? = *h;
            *h = j as isize;
        }
    }
    let mut first = vec![-1isize; mm];
    let mut max_first = vec![-1isize; mm];
    let mut prev_leaf = vec![-1isize; mm];
    let mut ancestor: Vec<usize> = (0..mm).collect();
    for k in 0..mm {
        let mut pk = g(post, k)?;
        *m(&mut delta, pk)? = if g(&first, pk)? < 0 { 1 } else { 0 };
        loop {
            if g(&first, pk)? >= 0 {
                break;
            }
            *m(&mut first, pk)? = k as isize;
            let parent = g(etree, pk)?;
            if parent >= 0 {
                pk = parent as usize;
            } else {
                break;
            }
        }
    }
    for k in 0..mm {
        let pk = g(post, k)?;
        let parent = g(etree, pk)?;
        if parent >= 0 {
            *m(&mut delta, parent as usize)? -= 1;
        }
        let mut j = g(&head, k)?;
        *m(&mut head, k)? = -1;
        while j >= 0 {
            let ju = j as usize;
            for &i in a.col(ju)? {
                let i = inv(i)?;
                let lca = least_common_ancestor(
                    i,
                    pk,
                    &first,
                    &mut max_first,
                    &mut prev_leaf,
                    &mut ancestor,
                )?;
                if lca != -2 {
                    *m(&mut delta, pk)? += 1;
                    if lca != -1 {
                        *m(&mut delta, lca as usize)? -= 1;
                    }
                }
            }
            j = g(&next, ju)?;
        }
        if parent >= 0 {
            *m(&mut ancestor, pk)? = parent as usize;
        }
    }
    for k in 0..mm {
        let parent = g(etree, k)?;
        if parent >= 0 {
            let dk = g(&delta, k)?;
            *m(&mut delta, parent as usize)? += dk;
        }
    }
    for (c, &dv) in col_counts.iter_mut().zip(&delta) {
        *c = usize::try_from(dv).map_err(|_| PetirError::Invalid)?;
    }
    Ok(())
}

/// faer's `SymbolicSupernodalCholesky`, the parts the numeric phases read.
#[derive(Debug, Clone)]
pub(crate) struct SymbolicSupernodal {
    pub supernode_begin: Vec<usize>,
    pub col_ptr_for_row_idx: Vec<usize>,
    pub col_ptr_for_val: Vec<usize>,
    pub row_idx: Vec<usize>,
    /// row-major (transposed, deduplicated) supernode structure: for
    /// supernode `s`, the supernodes `d` whose pattern reaches into it
    pub row_major_ptr: Vec<usize>,
    pub row_major_idx: Vec<usize>,
}

impl SymbolicSupernodal {
    pub fn n_supernodes(&self) -> usize {
        self.supernode_begin.len().saturating_sub(1)
    }
}

fn ereach_super(
    a: CscPattern<'_>,
    ata: Option<(&[usize], &[isize])>,
    super_etree: &[isize],
    index_to_super: &[usize],
    pos: &mut [usize],
    row_idx: &mut [usize],
    k: usize,
    visited: &mut [isize],
) -> Result<()> {
    *m(visited, g(index_to_super, k)?)? = k as isize;
    let (col, min_col) = match ata {
        None => (k, None),
        Some((col_perm_fwd, min_col)) => (g(col_perm_fwd, k)?, Some(min_col)),
    };
    for &i0 in a.col(col)? {
        let i = match min_col {
            None => i0,
            Some(mc) => {
                let c = g(mc, i0)?;
                if c < 0 {
                    continue;
                }
                c as usize
            }
        };
        if i >= k {
            continue;
        }
        let mut s = g(index_to_super, i)?;
        loop {
            if g(visited, s)? == k as isize {
                break;
            }
            let p = g(pos, s)?;
            *m(row_idx, p)? = k;
            *m(pos, s)? = p + 1;
            *m(visited, s)? = k as isize;
            let parent = g(super_etree, s)?;
            if parent < 0 {
                return Err(PetirError::Invalid);
            }
            s = parent as usize;
        }
    }
    Ok(())
}

/// Port of faer 0.24.4 `ghost_factorize_supernodal_symbolic` with the default
/// `SymbolicSupernodalParams` (`relax = DEFAULT_RELAX`). `a` is the matrix
/// whose structure is analysed (the upper-triangle permuted `A` for Cholesky,
/// `A` itself for LU), `etree` and `col_counts` its (column) elimination tree
/// and column counts. `ata` is faer's `CholeskyInput::ATA`: `None` analyses
/// `a` itself (`CholeskyInput::A`); `Some((col_perm_fwd, min_col))` analyses
/// `AᵀA` for the LU of `a`, with COLAMD's forward permutation and each row's
/// first permuted column.
pub(crate) fn factorize_supernodal_symbolic(
    a: CscPattern<'_>,
    ata: Option<(&[usize], &[isize])>,
    etree: &[isize],
    col_counts: &[usize],
) -> Result<SymbolicSupernodal> {
    let n = a.ncols;
    if n == 0 {
        return Ok(SymbolicSupernodal {
            supernode_begin: vec![0],
            col_ptr_for_row_idx: vec![0],
            col_ptr_for_val: vec![0],
            row_idx: Vec::new(),
            row_major_ptr: vec![0],
            row_major_idx: Vec::new(),
        });
    }
    let mut index_to_super = vec![0usize; n];
    let mut super_etree = vec![0isize; n];
    let mut supernode_sizes = vec![0usize; n];
    let mut child_count = vec![0usize; n];
    for j in 0..n {
        let p = g(etree, j)?;
        if p >= 0 {
            *m(&mut child_count, p as usize)? += 1;
        }
    }
    let mut current = 0usize;
    *m(&mut supernode_sizes, 0)? = 1;
    for j in 1..n {
        let j_prev = j - 1;
        let is_parent_of_prev = g(etree, j_prev)? == j as isize;
        let is_parent_of_only_prev = g(&child_count, j)? == 1;
        let same_pattern_as_prev = g(col_counts, j_prev)? == g(col_counts, j)? + 1;
        if !(is_parent_of_prev && is_parent_of_only_prev && same_pattern_as_prev) {
            current += 1;
        }
        *m(&mut supernode_sizes, current)? += 1;
    }
    let n_fund = current + 1;
    let fill_index_to_super =
        |index_to_super: &mut [usize], sizes: &[usize], ns: usize| -> Result<()> {
            let mut begin = 0usize;
            for s in 0..ns {
                let size = g(sizes, s)?;
                for x in index_to_super
                    .get_mut(begin..begin + size)
                    .ok_or(PetirError::Invalid)?
                {
                    *x = s;
                }
                begin += size;
            }
            Ok(())
        };
    let fill_super_etree = |super_etree: &mut [isize],
                            index_to_super: &[usize],
                            sizes: &[usize],
                            ns: usize|
     -> Result<()> {
        let mut begin = 0usize;
        for s in 0..ns {
            let size = g(sizes, s)?;
            let last = (begin + size).checked_sub(1).ok_or(PetirError::Invalid)?;
            let parent = g(etree, last)?;
            *m(super_etree, s)? = if parent >= 0 {
                g(index_to_super, parent as usize)? as isize
            } else {
                -1
            };
            begin += size;
        }
        Ok(())
    };
    fill_index_to_super(&mut index_to_super, &supernode_sizes, n_fund)?;
    fill_super_etree(&mut super_etree, &index_to_super, &supernode_sizes, n_fund)?;

    // --- relaxed amalgamation ---
    let mut child_lists = vec![-1isize; n_fund];
    let mut child_list_heads = vec![-1isize; n_fund];
    let mut last_merged_children = vec![-1isize; n_fund];
    let mut merge_parents = vec![-1isize; n_fund];
    let mut degrees = vec![0usize; n_fund];
    let mut num_zeros = vec![0usize; n_fund];
    {
        let mut begin = 0usize;
        for s in 0..n_fund {
            let size = g(&supernode_sizes, s)?;
            *m(&mut degrees, s)? = g(col_counts, begin + size - 1)?
                .checked_sub(1)
                .ok_or(PetirError::Invalid)?;
            begin += size;
        }
    }
    for s in 0..n_fund {
        let parent = g(&super_etree, s)?;
        if parent >= 0 {
            let p = parent as usize;
            *m(&mut child_lists, s)? = g(&child_list_heads, p)?;
            *m(&mut child_list_heads, p)? = s as isize;
        }
    }
    for parent in 0..n_fund {
        loop {
            let mut merging_child = -1isize;
            let mut num_new_zeros = 0usize;
            let mut num_merged_zeros = 0usize;
            let mut largest_mergable_size = 0usize;
            let mut child_ = g(&child_list_heads, parent)?;
            while child_ >= 0 {
                let child = child_ as usize;
                if child + 1 != parent {
                    child_ = g(&child_lists, child)?;
                    continue;
                }
                if g(&merge_parents, child)? >= 0 {
                    child_ = g(&child_lists, child)?;
                    continue;
                }
                let parent_size = g(&supernode_sizes, parent)?;
                let child_size = g(&supernode_sizes, child)?;
                if child_size < largest_mergable_size {
                    child_ = g(&child_lists, child)?;
                    continue;
                }
                let parent_degree = g(&degrees, parent)?;
                let child_degree = g(&degrees, child)?;
                let num_parent_zeros = g(&num_zeros, parent)?;
                let num_child_zeros = g(&num_zeros, child)?;
                let status_num_merged_zeros = {
                    let nnz = (parent_size + parent_degree)
                        .checked_sub(child_degree)
                        .ok_or(PetirError::Invalid)?
                        * child_size;
                    if nnz == 0 {
                        num_parent_zeros + num_child_zeros
                    } else {
                        let num_old_zeros = num_child_zeros + num_parent_zeros;
                        let nz = nnz + num_old_zeros;
                        let combined_size = child_size + parent_size;
                        let num_expanded_entries = (combined_size * (combined_size + 1)) / 2
                            + parent_degree * combined_size;
                        let mut out = NONE;
                        for cutoff in DEFAULT_RELAX {
                            let num_zeros_cutoff = num_expanded_entries as f64 * cutoff.1;
                            if cutoff.0 >= combined_size && num_zeros_cutoff >= nz as f64 {
                                out = nz;
                                break;
                            }
                        }
                        out
                    }
                };
                if status_num_merged_zeros == NONE {
                    child_ = g(&child_lists, child)?;
                    continue;
                }
                let num_proposed_new_zeros =
                    status_num_merged_zeros - (num_child_zeros + num_parent_zeros);
                if child_size > largest_mergable_size || num_proposed_new_zeros < num_new_zeros {
                    merging_child = child as isize;
                    num_new_zeros = num_proposed_new_zeros;
                    num_merged_zeros = status_num_merged_zeros;
                    largest_mergable_size = child_size;
                }
                child_ = g(&child_lists, child)?;
            }
            if merging_child >= 0 {
                let mc = merging_child as usize;
                let add = g(&supernode_sizes, mc)?;
                *m(&mut supernode_sizes, parent)? += add;
                *m(&mut supernode_sizes, mc)? = 0;
                *m(&mut num_zeros, parent)? = num_merged_zeros;
                let lmp = g(&last_merged_children, parent)?;
                *m(&mut merge_parents, mc)? = if lmp >= 0 { lmp } else { parent as isize };
                let lmc = g(&last_merged_children, mc)?;
                *m(&mut last_merged_children, parent)? = if lmc >= 0 { lmc } else { mc as isize };
            } else {
                break;
            }
        }
    }
    let mut pos = 0usize;
    for s in 0..n_fund {
        let size = g(&supernode_sizes, s)?;
        let degree = g(&degrees, s)?;
        if size > 0 {
            *m(&mut supernode_sizes, pos)? = size;
            *m(&mut degrees, pos)? = degree;
            pos += 1;
        }
    }
    let ns = pos;
    // supernode_begin temporarily holds the degrees, shifted by one, as in faer.
    let mut supernode_begin = vec![0usize; ns + 1];
    for s in 0..ns {
        *m(&mut supernode_begin, s + 1)? = g(&degrees, s)?;
    }
    if ns != n_fund {
        fill_index_to_super(&mut index_to_super, &supernode_sizes, ns)?;
        fill_super_etree(&mut super_etree, &index_to_super, &supernode_sizes, ns)?;
    }

    let mut col_ptr_for_row_idx = vec![0usize; ns + 1];
    let mut col_ptr_for_val = vec![0usize; ns + 1];
    let mut row_ptr = 0usize;
    let mut val_ptr = 0usize;
    *m(&mut supernode_begin, 0)? = 0;
    for s in 0..ns {
        let degree = g(&supernode_begin, s + 1)?;
        let ncols = g(&supernode_sizes, s)?;
        let nrows = degree + ncols;
        *m(&mut supernode_sizes, s)? = row_ptr;
        let cur = g(&supernode_begin, s)?;
        *m(&mut supernode_begin, s + 1)? = cur + ncols;
        *m(&mut col_ptr_for_row_idx, s)? = row_ptr;
        *m(&mut col_ptr_for_val, s)? = val_ptr;
        row_ptr += degree;
        val_ptr = nrows
            .checked_mul(ncols)
            .and_then(|x| x.checked_add(val_ptr))
            .ok_or(PetirError::Invalid)?;
    }
    *m(&mut col_ptr_for_row_idx, ns)? = row_ptr;
    *m(&mut col_ptr_for_val, ns)? = val_ptr;
    let mut row_idx = vec![0usize; row_ptr];
    let current_row_positions = &mut supernode_sizes;
    let mut visited = vec![-1isize; ns];
    let super_etree = super_etree.get(..ns).ok_or(PetirError::Invalid)?;
    for s in 0..ns {
        for k in g(&supernode_begin, s)?..g(&supernode_begin, s + 1)? {
            ereach_super(
                a,
                ata,
                super_etree,
                &index_to_super,
                current_row_positions,
                &mut row_idx,
                k,
                &mut visited,
            )?;
        }
    }
    // row-major structure of the compressed (supernode x supernode) pattern:
    // faer's `to_row_major`, i.e. `transpose_dedup`.
    let mut count = vec![0usize; ns];
    let mut last_seen = vec![NONE; ns];
    for d in 0..ns {
        for &r in super::sl(
            &row_idx,
            g(&col_ptr_for_row_idx, d)?..g(&col_ptr_for_row_idx, d + 1)?,
        )? {
            let s = g(&index_to_super, r)?;
            if g(&last_seen, s)? == d {
                continue;
            }
            *m(&mut last_seen, s)? = d;
            *m(&mut count, s)? += 1;
        }
    }
    let mut row_major_ptr = vec![0usize; ns + 1];
    for s in 0..ns {
        *m(&mut row_major_ptr, s + 1)? = g(&row_major_ptr, s)? + g(&count, s)?;
    }
    let mut cur: Vec<usize> = row_major_ptr.get(..ns).ok_or(PetirError::Invalid)?.to_vec();
    let mut row_major_idx = vec![0usize; g(&row_major_ptr, ns)?];
    last_seen.fill(NONE);
    for d in 0..ns {
        for &r in super::sl(
            &row_idx,
            g(&col_ptr_for_row_idx, d)?..g(&col_ptr_for_row_idx, d + 1)?,
        )? {
            let s = g(&index_to_super, r)?;
            if g(&last_seen, s)? == d {
                continue;
            }
            *m(&mut last_seen, s)? = d;
            let c = g(&cur, s)?;
            *m(&mut row_major_idx, c)? = d;
            *m(&mut cur, s)? = c + 1;
        }
    }
    Ok(SymbolicSupernodal {
        supernode_begin,
        col_ptr_for_row_idx,
        col_ptr_for_val,
        row_idx,
        row_major_ptr,
        row_major_idx,
    })
}
