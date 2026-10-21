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
//   src/sparse/linalg/qr.rs        ghost_col_etree                -> col_etree
//   src/sparse/linalg/cholesky.rs  ghost_postorder,
//                                  postorder_depth_first_search   -> postorder
//   src/sparse/linalg/cholesky.rs  simplicial::ghost_prefactorize_symbolic_cholesky
//                                                                 -> prefactorize_symbolic_cholesky
//   src/sparse/linalg/cholesky.rs  simplicial::ereach             -> ereach
// An elimination tree is `&[isize]`, `-1` meaning "no parent" (faer's NONE).
// Subscripts are checked accesses (see `super`'s module docs).

//! **Elimination trees** and their postorder — the symbolic plumbing shared by
//! [`super::colamd`], [`super::SparseLu`] and [`super::SparseLlt`], ported
//! from `faer` 0.24.4.

use super::{g, m, CscPattern};
use crate::Result;
use alloc::vec;

/// The column elimination tree of `a` (the elimination tree of `AᵀA`),
/// columns taken in the order `col_perm_fwd` when given. `etree` must be
/// `a.ncols` long; `-1` marks a root.
///
/// Port of faer 0.24.4 `qr::ghost_col_etree`.
///
/// # Errors
/// `PetirError::Invalid` on an out-of-range index.
pub fn col_etree(
    a: CscPattern<'_>,
    col_perm_fwd: Option<&[usize]>,
    etree: &mut [isize],
) -> Result<()> {
    let n = a.ncols;
    let mrows = a.nrows;
    let mut ancestor = vec![-1isize; n];
    let mut prev = vec![-1isize; mrows];
    etree.fill(-1);
    for j in 0..n {
        let pj = match col_perm_fwd {
            Some(perm) => g(perm, j)?,
            None => j,
        };
        for &i_ in a.col(pj)? {
            let mut i = g(&prev, i_)?;
            while i >= 0 {
                let iu = i as usize;
                if iu == j {
                    break;
                }
                let next_i = g(&ancestor, iu)?;
                *m(&mut ancestor, iu)? = j as isize;
                if next_i < 0 {
                    *m(etree, iu)? = j as isize;
                    break;
                }
                i = next_i;
            }
            *m(&mut prev, i_)? = j as isize;
        }
    }
    Ok(())
}

fn postorder_depth_first_search(
    post: &mut [usize],
    root: usize,
    mut start_index: usize,
    stack: &mut [usize],
    first_child: &mut [isize],
    next_child: &[isize],
) -> Result<usize> {
    let mut top = 1usize;
    *m(stack, 0)? = root;
    while top != 0 {
        let current_node = g(stack, top - 1)?;
        let current_child = g(first_child, current_node)?;
        if current_child >= 0 {
            let cc = current_child as usize;
            *m(stack, top)? = cc;
            top += 1;
            *m(first_child, current_node)? = g(next_child, cc)?;
        } else {
            *m(post, start_index)? = current_node;
            start_index += 1;
            top -= 1;
        }
    }
    Ok(start_index)
}

/// A postordering of the forest `etree` into `post` (same length).
///
/// Port of faer 0.24.4 `cholesky::ghost_postorder`.
///
/// # Errors
/// `PetirError::Invalid` on an out-of-range index.
pub fn postorder(post: &mut [usize], etree: &[isize]) -> Result<()> {
    let n = post.len();
    if n == 0 {
        return Ok(());
    }
    let mut stack = vec![0usize; n];
    let mut first_child = vec![-1isize; n];
    // faer leaves `next_child` uninitialised; every entry read is written first.
    let mut next_child = vec![-1isize; n];
    for j in (0..n).rev() {
        let parent = g(etree, j)?;
        if parent >= 0 {
            let first = m(&mut first_child, parent as usize)?;
            *m(&mut next_child, j)? = *first;
            *first = j as isize;
        }
    }
    let mut start_index = 0usize;
    for (root, &parent) in etree.iter().enumerate() {
        if parent < 0 {
            start_index = postorder_depth_first_search(
                post,
                root,
                start_index,
                &mut stack,
                &mut first_child,
                &next_child,
            )?;
        }
    }
    Ok(())
}

/// Elimination tree and column counts of the Cholesky factor of `a`, reading
/// only its upper triangle.
///
/// Port of faer 0.24.4 `simplicial::ghost_prefactorize_symbolic_cholesky`.
///
/// # Errors
/// `PetirError::Invalid` on an out-of-range index.
pub fn prefactorize_symbolic_cholesky(
    etree: &mut [isize],
    col_counts: &mut [usize],
    a: CscPattern<'_>,
) -> Result<()> {
    let n = a.ncols;
    let mut visited = vec![0usize; n];
    etree.fill(-1);
    for j in 0..n {
        *m(&mut visited, j)? = j;
        *m(col_counts, j)? = 1;
        for &i0 in a.col(j)? {
            let mut i = i0;
            if i < j {
                loop {
                    if g(&visited, i)? == j {
                        break;
                    }
                    let parent = g(etree, i)?;
                    let next_i = if parent >= 0 {
                        parent as usize
                    } else {
                        *m(etree, i)? = j as isize;
                        j
                    };
                    *m(col_counts, i)? += 1;
                    *m(&mut visited, i)? = j;
                    i = next_i;
                }
            }
        }
    }
    Ok(())
}

/// The nonzero pattern of row `k` of the Cholesky factor (the "elimination
/// reach"), written to the tail of `stack`; returns where it starts.
///
/// Port of faer 0.24.4 `simplicial::ereach`.
///
/// # Errors
/// `PetirError::Invalid` on an out-of-range index.
pub fn ereach(
    stack: &mut [usize],
    a: CscPattern<'_>,
    etree: &[isize],
    k: usize,
    visited: &mut [isize],
) -> Result<usize> {
    let n = a.ncols;
    let mut top = n;
    *m(visited, k)? = k as isize;
    for &i0 in a.col(k)? {
        let mut i = i0;
        if i >= k {
            continue;
        }
        let mut len = 0usize;
        loop {
            if g(visited, i)? == k as isize {
                break;
            }
            *m(stack, len)? = i;
            len += 1;
            *m(visited, i)? = k as isize;
            i = g(etree, i)? as usize;
        }
        let dst = top.checked_sub(len).ok_or(crate::PetirError::Invalid)?;
        super::ensure(top <= stack.len())?;
        stack.copy_within(..len, dst);
        top = dst;
    }
    Ok(top)
}
