// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 Theodore Ong and the outram-park contributors.
//
// PORTED from the `faer` crate, version 0.24.4, file
// `src/sparse/linalg/colamd.rs`, read from the published crates.io source on
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
// faer's colamd.rs is itself a port of SuiteSparse COLAMD, whose notice faer
// ships as COPYING.SUITE_SPARSE.COLAMD.BSD and which is retained here:
//
//   COLAMD, Copyright 1998-2022, Timothy A. Davis.  http://www.suitesparse.com
//
//   COLAMD License: BSD 3-clause
//
//   Redistribution and use in source and binary forms, with or without
//   modification, are permitted provided that the following conditions are met:
//       * Redistributions of source code must retain the above copyright
//         notice, this list of conditions and the following disclaimer.
//       * Redistributions in binary form must reproduce the above copyright
//         notice, this list of conditions and the following disclaimer in the
//         documentation and/or other materials provided with the distribution.
//       * Neither the name of the organizations to which the authors are
//         affiliated, nor the names of its contributors may be used to endorse
//         or promote products derived from this software without specific prior
//         written permission.
//
//   THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS"
//   AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
//   IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE
//   ARE DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT HOLDERS BE LIABLE FOR ANY
//   DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES
//   (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR
//   SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER
//   CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT
//   LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY
//   OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH
//   DAMAGE.
//
// MIT/BSD-3 into AGPL-3.0-only is a ONE-WAY flow. Maintainer direction for
// this port: gh #994, 2026-10-20. See the crate NOTICE.
//
// Ported items (all from colamd.rs): ColamdCol/ColamdRow and their
// is_dead/kill helpers, clear_mark, order, detect_super_cols,
// garbage_collection, Control. faer's generic `I: Index` is fixed to `usize`
// and `I::Signed` to `isize`; faer's `NONE` (`usize::MAX`) is `-1` signed.
// The `MemStack` scratch is replaced by `Vec`s (no arithmetic depends on it),
// faer's `assert!`s are `ensure(..)?`, and subscripts are checked accesses
// (see `super`'s module docs).

//! **COLAMD** — the approximate-minimum-degree *column* ordering that
//! [`super::SparseLu`] uses to limit fill, ported from `faer` 0.24.4
//! (`sparse/linalg/colamd.rs`), itself a port of SuiteSparse COLAMD.
//!
//! The ordering is integer-only, so the port reproduces faer's permutation
//! exactly; `tests/sparse_vs_faer.rs` checks it.

use super::etree::{col_etree, postorder};
use super::{ensure, g, m, CscPattern, NONE};
use crate::Result;
use alloc::vec;
use alloc::vec::Vec;

const SNONE: isize = -1;

/// Tuning parameters (faer's `colamd::Control`, same defaults).
#[derive(Debug, Copy, Clone, PartialEq)]
pub struct Control {
    /// "dense" if degree > dense_row * sqrt(ncols)
    pub dense_row: f64,
    /// "dense" if degree > dense_col * sqrt(min(nrows, ncols))
    pub dense_col: f64,
    /// do aggressive absorption
    pub aggressive: bool,
}

impl Default for Control {
    fn default() -> Self {
        Self {
            dense_row: 0.5,
            dense_col: 0.5,
            aggressive: true,
        }
    }
}

#[derive(Copy, Clone, Debug)]
struct ColamdRow {
    start: isize,
    length: isize,
    shared1: isize,
    shared2: isize,
}

#[derive(Copy, Clone, Debug)]
struct ColamdCol {
    start: isize,
    length: isize,
    shared1: isize,
    shared2: isize,
    shared3: isize,
    shared4: isize,
}

impl ColamdCol {
    fn is_dead_principal(&self) -> bool {
        self.start == SNONE
    }
    fn is_dead(&self) -> bool {
        self.start < 0
    }
    fn is_alive(&self) -> bool {
        !self.is_dead()
    }
    fn kill_principal(&mut self) {
        self.start = SNONE;
    }
    fn kill_non_principal(&mut self) {
        self.start = SNONE - 1;
    }
}

impl ColamdRow {
    fn is_dead(&self) -> bool {
        self.shared2 < 0
    }
    fn is_alive(&self) -> bool {
        !self.is_dead()
    }
    fn kill(&mut self) {
        self.shared2 = SNONE;
    }
}

fn clear_mark(tag_mark: usize, max_mark: usize, row: &mut [ColamdRow]) -> usize {
    if tag_mark == 0 || tag_mark >= max_mark {
        for r in row {
            if r.is_alive() {
                r.shared2 = 0;
            }
        }
        1
    } else {
        tag_mark
    }
}

#[inline]
fn zx(x: isize) -> usize {
    x as usize
}

/// Compute the COLAMD column ordering of `a`'s pattern, writing the forward
/// permutation to `perm` and its inverse to `perm_inv` (both `a.ncols` long),
/// post-ordered by the column elimination tree exactly as faer does.
///
/// Port of faer 0.24.4 `colamd::order`. Unsorted columns are allowed.
///
/// # Errors
/// `PetirError::Invalid` where faer would panic (a malformed pattern).
pub fn order(
    perm: &mut [usize],
    perm_inv: &mut [usize],
    a: CscPattern<'_>,
    control: Control,
) -> Result<()> {
    let nrows = a.nrows;
    let n = a.ncols;
    {
        let zero_col = ColamdCol {
            start: 0,
            length: 0,
            shared1: 0,
            shared2: 0,
            shared3: 0,
            shared4: 0,
        };
        let mut col = vec![zero_col; n + 1];
        let mut row = vec![
            ColamdRow {
                start: 0,
                length: 0,
                shared1: 0,
                shared2: 0
            };
            nrows + 1
        ];
        let nnz = a.nnz()?;
        let mut p = vec![0usize; n + 1];
        let size = (2 * nnz) + nnz / 5 + n;
        let mut new_row_idx = vec![0usize; size];
        *m(&mut p, 0)? = 0;
        for j in 0..n {
            let row_idx = a.col(j)?;
            let pj = g(&p, j)?;
            let pj1 = pj + row_idx.len();
            *m(&mut p, j + 1)? = pj1;
            new_row_idx
                .get_mut(pj..pj1)
                .ok_or(crate::PetirError::Invalid)?
                .copy_from_slice(row_idx);
        }
        let aa = new_row_idx.as_mut_slice();
        for c in 0..n {
            let (pc, pc1) = (g(&p, c)?, g(&p, c + 1)?);
            *m(&mut col, c)? = ColamdCol {
                start: pc as isize,
                length: pc1 as isize - pc as isize,
                shared1: 1,
                shared2: 0,
                shared3: SNONE,
                shared4: SNONE,
            };
        }
        *m(&mut col, n)? = ColamdCol {
            start: SNONE,
            length: SNONE,
            shared1: SNONE,
            shared2: SNONE,
            shared3: SNONE,
            shared4: SNONE,
        };
        for r in row.iter_mut().take(nrows) {
            *r = ColamdRow {
                start: 0,
                length: 0,
                shared1: SNONE,
                shared2: SNONE,
            };
        }
        *m(&mut row, nrows)? = ColamdRow {
            start: SNONE,
            length: SNONE,
            shared1: SNONE,
            shared2: SNONE,
        };
        let mut jumbled = false;
        for c in 0..n {
            let mut last_row = SNONE;
            let mut cp = g(&p, c)?;
            let cp_end = g(&p, c + 1)?;
            while cp < cp_end {
                let r = g(aa, cp)?;
                cp += 1;
                if (r as isize) <= last_row || g(&row, r)?.shared2 == c as isize {
                    jumbled = true;
                }
                if g(&row, r)?.shared2 != c as isize {
                    m(&mut row, r)?.length += 1;
                } else {
                    m(&mut col, c)?.length -= 1;
                }
                m(&mut row, r)?.shared2 = c as isize;
                last_row = r as isize;
            }
        }
        // faer indexes row[0] unconditionally here; with nrows == 0 that is
        // the sentinel row, as in faer.
        {
            let pn = g(&p, n)?;
            let r0 = m(&mut row, 0)?;
            r0.start = pn as isize;
            r0.shared1 = r0.start;
            r0.shared2 = -1;
        }
        for r in 1..nrows {
            let prev = g(&row, r - 1)?;
            let rr = m(&mut row, r)?;
            rr.start = prev.start + prev.length;
            rr.shared1 = rr.start;
            rr.shared2 = -1;
        }
        if jumbled {
            for c in 0..n {
                let mut cp = g(&p, c)?;
                let cp_end = g(&p, c + 1)?;
                while cp < cp_end {
                    let r = g(aa, cp)?;
                    cp += 1;
                    if g(&row, r)?.shared2 != c as isize {
                        *m(aa, zx(g(&row, r)?.shared1))? = c;
                        let rr = m(&mut row, r)?;
                        rr.shared1 += 1;
                        rr.shared2 = c as isize;
                    }
                }
            }
        } else {
            for c in 0..n {
                let mut cp = g(&p, c)?;
                let cp_end = g(&p, c + 1)?;
                while cp < cp_end {
                    let r = g(aa, cp)?;
                    cp += 1;
                    *m(aa, zx(g(&row, r)?.shared1))? = c;
                    m(&mut row, r)?.shared1 += 1;
                }
            }
        }
        for r in row.iter_mut().take(nrows) {
            r.shared2 = 0;
            r.shared1 = r.length;
        }
        if jumbled {
            m(&mut col, 0)?.start = 0;
            *m(&mut p, 0)? = zx(g(&col, 0)?.start);
            for c in 1..n {
                let prev = g(&col, c - 1)?;
                m(&mut col, c)?.start = prev.start + prev.length;
                *m(&mut p, c)? = zx(g(&col, c)?.start);
            }
            for r in 0..nrows {
                let rr = g(&row, r)?;
                let mut rp = zx(rr.start);
                let rp_end = rp + zx(rr.length);
                while rp < rp_end {
                    let at = g(&p, rp)?;
                    *m(aa, at)? = r;
                    *m(&mut p, rp)? += 1;
                    rp += 1;
                }
            }
        }
        let dense_row_count = if control.dense_row < 0.0 {
            n - 1
        } else {
            Ord::max(16, (control.dense_row * n as f64) as usize)
        };
        let dense_col_count = if control.dense_col < 0.0 {
            nrows - 1
        } else {
            Ord::max(16, (control.dense_col * Ord::min(nrows, n) as f64) as usize)
        };
        let mut max_deg = 0usize;
        let mut ncol2 = n;
        let head = p.as_mut_slice();
        for c in (0..n).rev() {
            let deg = g(&col, c)?.length;
            if deg == 0 {
                ncol2 -= 1;
                let cc = m(&mut col, c)?;
                cc.shared2 = ncol2 as isize;
                cc.kill_principal();
            }
        }
        for c in (0..n).rev() {
            let cc = g(&col, c)?;
            if cc.is_dead() {
                continue;
            }
            let deg = cc.length;
            if zx(deg) > dense_col_count {
                ncol2 -= 1;
                m(&mut col, c)?.shared2 = ncol2 as isize;
                let mut cp = zx(cc.start);
                let cp_end = cp + zx(cc.length);
                while cp < cp_end {
                    m(&mut row, g(aa, cp)?)?.shared1 -= 1;
                    cp += 1;
                }
                m(&mut col, c)?.kill_principal();
            }
        }
        for r in row.iter_mut().take(nrows) {
            let deg = zx(r.shared1);
            ensure(deg <= n)?;
            if deg > dense_row_count || deg == 0 {
                r.kill();
            } else {
                max_deg = Ord::max(deg, max_deg);
            }
        }
        for c in (0..n).rev() {
            let cc = g(&col, c)?;
            if cc.is_dead() {
                continue;
            }
            let mut score = 0usize;
            let mut cp = zx(cc.start);
            let mut new_cp = cp;
            let cp_end = cp + zx(cc.length);
            while cp < cp_end {
                let r = g(aa, cp)?;
                cp += 1;
                let rr = g(&row, r)?;
                if rr.is_dead() {
                    continue;
                }
                *m(aa, new_cp)? = r;
                new_cp += 1;
                score += zx(rr.shared1) - 1;
                score = Ord::min(score, n);
            }
            let col_length = new_cp - zx(cc.start);
            if col_length == 0 {
                ncol2 -= 1;
                let cm = m(&mut col, c)?;
                cm.shared2 = ncol2 as isize;
                cm.kill_principal();
            } else {
                ensure(score <= n)?;
                let cm = m(&mut col, c)?;
                cm.length = col_length as isize;
                cm.shared2 = score as isize;
            }
        }
        head.fill(NONE);
        let mut min_score = n;
        for c in (0..n).rev() {
            if g(&col, c)?.is_alive() {
                let score = zx(g(&col, c)?.shared2);
                ensure(min_score <= n)?;
                ensure(score <= n)?;
                ensure(g(head, score)? as isize >= SNONE)?;
                let next_col = g(head, score)?;
                let cm = m(&mut col, c)?;
                cm.shared3 = SNONE;
                cm.shared4 = next_col as isize;
                if next_col != NONE {
                    m(&mut col, next_col)?.shared3 = c as isize;
                }
                *m(head, score)? = c;
                min_score = Ord::min(score, min_score);
            }
        }
        let max_mark = (isize::MAX as usize) - n;
        let mut tag_mark = clear_mark(0, max_mark, &mut row);
        let mut min_score = 0usize;
        let mut pfree = 2 * nnz;
        let mut k = 0usize;
        while k < ncol2 {
            ensure(min_score <= n)?;
            ensure(g(head, min_score)? as isize >= SNONE)?;
            while g(head, min_score)? == NONE && min_score < n {
                min_score += 1;
            }
            let pivot_col = g(head, min_score)?;
            let mut next_col = g(&col, pivot_col)?.shared4;
            *m(head, min_score)? = next_col as usize;
            if next_col != SNONE {
                m(&mut col, zx(next_col))?.shared3 = SNONE;
            }
            ensure(!g(&col, pivot_col)?.is_dead())?;
            let pivot_col_score = g(&col, pivot_col)?.shared2;
            m(&mut col, pivot_col)?.shared2 = k as isize;
            let pivot_col_thickness = g(&col, pivot_col)?.shared1;
            ensure(pivot_col_thickness > 0)?;
            k += zx(pivot_col_thickness);
            let needed_memory = Ord::min(pivot_col_score, (n - k) as isize);
            if pfree as isize + needed_memory >= aa.len() as isize {
                pfree = garbage_collection(&mut row, &mut col, aa, pfree)?;
                ensure((pfree as isize + needed_memory) < aa.len() as isize)?;
                tag_mark = clear_mark(0, max_mark, &mut row);
            }
            let pivot_row_start = pfree;
            let mut pivot_row_degree = 0usize;
            m(&mut col, pivot_col)?.shared1 = -pivot_col_thickness;
            let pc = g(&col, pivot_col)?;
            let mut cp = zx(pc.start);
            let cp_end = cp + zx(pc.length);
            while cp < cp_end {
                let r = g(aa, cp)?;
                cp += 1;
                let rr = g(&row, r)?;
                if rr.is_alive() {
                    let mut rp = zx(rr.start);
                    let rp_end = rp + zx(rr.length);
                    while rp < rp_end {
                        let c = g(aa, rp)?;
                        rp += 1;
                        let cc = g(&col, c)?;
                        let col_thickness = cc.shared1;
                        if col_thickness > 0 && cc.is_alive() {
                            m(&mut col, c)?.shared1 = -col_thickness;
                            *m(aa, pfree)? = c;
                            pfree += 1;
                            pivot_row_degree += zx(col_thickness);
                        }
                    }
                }
            }
            m(&mut col, pivot_col)?.shared1 = pivot_col_thickness;
            max_deg = Ord::max(max_deg, pivot_row_degree);
            let pc = g(&col, pivot_col)?;
            let mut cp = zx(pc.start);
            let cp_end = cp + zx(pc.length);
            while cp < cp_end {
                let r = g(aa, cp)?;
                cp += 1;
                m(&mut row, r)?.kill();
            }
            let pivot_row_length = pfree - pivot_row_start;
            let pivot_row = if pivot_row_length > 0 {
                g(aa, zx(g(&col, pivot_col)?.start))?
            } else {
                NONE
            };
            let mut rp = pivot_row_start;
            let rp_end = rp + pivot_row_length;
            while rp < rp_end {
                let c = g(aa, rp)?;
                rp += 1;
                ensure(g(&col, c)?.is_alive())?;
                let col_thickness = -g(&col, c)?.shared1;
                ensure(col_thickness > 0)?;
                m(&mut col, c)?.shared1 = col_thickness;
                let cc = g(&col, c)?;
                let cur_score = zx(cc.shared2);
                let prev_col = cc.shared3;
                let next_col = cc.shared4;
                ensure(cur_score <= n)?;
                if prev_col == SNONE {
                    *m(head, cur_score)? = next_col as usize;
                } else {
                    m(&mut col, zx(prev_col))?.shared4 = next_col;
                }
                if next_col != SNONE {
                    m(&mut col, zx(next_col))?.shared3 = prev_col;
                }
                let mut cp = zx(cc.start);
                let cp_end = cp + zx(cc.length);
                while cp < cp_end {
                    let r = g(aa, cp)?;
                    cp += 1;
                    let rr = g(&row, r)?;
                    let row_mark = rr.shared2;
                    if rr.is_dead() {
                        continue;
                    }
                    ensure(r != pivot_row)?;
                    let mut set_difference = row_mark - tag_mark as isize;
                    if set_difference < 0 {
                        ensure(rr.shared1 <= max_deg as isize)?;
                        set_difference = rr.shared1;
                    }
                    set_difference -= col_thickness;
                    ensure(set_difference >= 0)?;
                    if set_difference == 0 && control.aggressive {
                        m(&mut row, r)?.kill();
                    } else {
                        m(&mut row, r)?.shared2 = set_difference + tag_mark as isize;
                    }
                }
            }
            let mut rp = pivot_row_start;
            let rp_end = rp + pivot_row_length;
            while rp < rp_end {
                let c = g(aa, rp)?;
                rp += 1;
                ensure(g(&col, c)?.is_alive())?;
                let mut hash = 0usize;
                let mut cur_score = 0usize;
                let cc = g(&col, c)?;
                let mut cp = zx(cc.start);
                let mut new_cp = cp;
                let cp_end = cp + zx(cc.length);
                while cp < cp_end {
                    let r = g(aa, cp)?;
                    cp += 1;
                    let rr = g(&row, r)?;
                    let row_mark = rr.shared2;
                    if rr.is_dead() {
                        continue;
                    }
                    ensure(row_mark >= tag_mark as isize)?;
                    *m(aa, new_cp)? = r;
                    new_cp += 1;
                    hash += r;
                    cur_score += zx(row_mark - tag_mark as isize);
                    cur_score = Ord::min(cur_score, n);
                }
                m(&mut col, c)?.length = (new_cp - zx(cc.start)) as isize;
                if g(&col, c)?.length == 0 {
                    m(&mut col, c)?.kill_principal();
                    let thick = zx(g(&col, c)?.shared1);
                    pivot_row_degree -= thick;
                    m(&mut col, c)?.shared2 = k as isize;
                    k += thick;
                } else {
                    m(&mut col, c)?.shared2 = cur_score as isize;
                    hash %= n + 1;
                    let head_column = g(head, hash)?;
                    let first_col;
                    if head_column as isize > SNONE {
                        first_col = g(&col, head_column)?.shared3;
                        m(&mut col, head_column)?.shared3 = c as isize;
                    } else {
                        first_col = -(head_column as isize + 2);
                        *m(head, hash)? = (-((c + 2) as isize)) as usize;
                    }
                    let cm = m(&mut col, c)?;
                    cm.shared4 = first_col;
                    cm.shared3 = hash as isize;
                    ensure(cm.is_alive())?;
                }
            }
            detect_super_cols(&mut col, aa, head, pivot_row_start, pivot_row_length)?;
            m(&mut col, pivot_col)?.kill_principal();
            tag_mark = clear_mark(tag_mark + max_deg + 1, max_mark, &mut row);
            let mut rp = pivot_row_start;
            let mut new_rp = rp;
            let rp_end = rp + pivot_row_length;
            while rp < rp_end {
                let c = g(aa, rp)?;
                rp += 1;
                if g(&col, c)?.is_dead() {
                    continue;
                }
                *m(aa, new_rp)? = c;
                new_rp += 1;
                let cc = g(&col, c)?;
                *m(aa, zx(cc.start + cc.length))? = pivot_row;
                m(&mut col, c)?.length += 1;
                let cc = g(&col, c)?;
                let mut cur_score = zx(cc.shared2) + pivot_row_degree;
                let max_score = n - k - zx(cc.shared1);
                cur_score -= zx(cc.shared1);
                cur_score = Ord::min(cur_score, max_score);
                next_col = g(head, cur_score)? as isize;
                let cm = m(&mut col, c)?;
                cm.shared2 = cur_score as isize;
                cm.shared4 = next_col;
                cm.shared3 = SNONE;
                if next_col != SNONE {
                    m(&mut col, zx(next_col))?.shared3 = c as isize;
                }
                *m(head, cur_score)? = c;
                min_score = Ord::min(min_score, cur_score);
            }
            if pivot_row_degree > 0 {
                let pr = m(&mut row, pivot_row)?;
                pr.start = pivot_row_start as isize;
                pr.length = (new_rp - pivot_row_start) as isize;
                pr.shared1 = pivot_row_degree as isize;
                pr.shared2 = 0;
            }
        }
        for i in 0..n {
            let ci = g(&col, i)?;
            ensure(ci.is_dead())?;
            if !ci.is_dead_principal() && ci.shared2 == SNONE {
                let mut parent = i;
                loop {
                    parent = zx(g(&col, parent)?.shared1);
                    if g(&col, parent)?.is_dead_principal() {
                        break;
                    }
                }
                let mut c = i;
                let mut order = zx(g(&col, parent)?.shared2);
                loop {
                    ensure(g(&col, c)?.shared2 == SNONE)?;
                    let cm = m(&mut col, c)?;
                    cm.shared2 = order as isize;
                    order += 1;
                    cm.shared1 = parent as isize;
                    c = zx(cm.shared1);
                    if g(&col, c)?.shared2 != SNONE {
                        break;
                    }
                }
                m(&mut col, parent)?.shared2 = order as isize;
            }
        }
        for c in 0..n {
            *m(perm, zx(g(&col, c)?.shared2))? = c;
        }
        for c in 0..n {
            *m(perm_inv, g(perm, c)?)? = c;
        }
    }
    let mut etree = vec![0isize; n];
    col_etree(a, Some(perm), &mut etree)?;
    let mut post = vec![0usize; n];
    postorder(&mut post, &etree)?;
    for (i, &pi) in post.iter().enumerate() {
        *m(perm, pi)? = i;
    }
    for i in 0..n {
        let v = g(perm, g(perm_inv, i)?)?;
        *m(perm_inv, i)? = v;
    }
    for i in 0..n {
        *m(perm, g(perm_inv, i)?)? = i;
    }
    Ok(())
}

fn detect_super_cols(
    col: &mut [ColamdCol],
    aa: &mut [usize],
    head: &mut [usize],
    row_start: usize,
    row_length: usize,
) -> Result<()> {
    let mut rp = row_start;
    let rp_end = rp + row_length;
    while rp < rp_end {
        let c = g(aa, rp)?;
        rp += 1;
        if g(col, c)?.is_dead() {
            continue;
        }
        let hash = zx(g(col, c)?.shared3);
        let head_column = g(head, hash)? as isize;
        let first_col = if head_column > SNONE {
            g(col, zx(head_column))?.shared3
        } else {
            -(head_column + 2)
        };
        let mut super_c_ = first_col;
        while super_c_ != SNONE {
            let super_c = zx(super_c_);
            let length = g(col, super_c)?.length;
            let mut prev_c = super_c;
            let mut c_ = g(col, super_c)?.shared4;
            while c_ != SNONE {
                let c = zx(c_);
                let (cc, sc) = (g(col, c)?, g(col, super_c)?);
                if cc.length != length || cc.shared2 != sc.shared2 {
                    prev_c = c;
                    c_ = cc.shared4;
                    continue;
                }
                let mut cp1 = zx(sc.start);
                let mut cp2 = zx(cc.start);
                let mut i = 0usize;
                while i < zx(length) {
                    if g(aa, cp1)? != g(aa, cp2)? {
                        break;
                    }
                    cp1 += 1;
                    cp2 += 1;
                    i += 1;
                }
                if i != zx(length) {
                    prev_c = c;
                    c_ = cc.shared4;
                    continue;
                }
                m(col, super_c)?.shared1 += cc.shared1;
                let cm = m(col, c)?;
                cm.shared1 = super_c as isize;
                cm.kill_non_principal();
                cm.shared2 = SNONE;
                let next = cm.shared4;
                m(col, prev_c)?.shared4 = next;
                c_ = next;
            }
            super_c_ = g(col, super_c)?.shared4;
        }
        if head_column > SNONE {
            m(col, zx(head_column))?.shared3 = SNONE;
        } else {
            *m(head, hash)? = NONE;
        }
    }
    Ok(())
}

fn garbage_collection(
    row: &mut [ColamdRow],
    col: &mut [ColamdCol],
    aa: &mut [usize],
    pfree: usize,
) -> Result<usize> {
    let mut pdest = 0usize;
    let nrows = row.len() - 1;
    let n = col.len() - 1;
    for c in 0..n {
        if !g(col, c)?.is_dead() {
            let mut psrc = zx(g(col, c)?.start);
            m(col, c)?.start = pdest as isize;
            let length = zx(g(col, c)?.length);
            for _ in 0..length {
                let r = g(aa, psrc)?;
                psrc += 1;
                if !g(row, r)?.is_dead() {
                    *m(aa, pdest)? = r;
                    pdest += 1;
                }
            }
            let cm = m(col, c)?;
            cm.length = pdest as isize - cm.start;
        }
    }
    for r in 0..nrows {
        let rr = g(row, r)?;
        if rr.is_dead() || rr.length == 0 {
            m(row, r)?.kill();
        } else {
            let psrc = zx(rr.start);
            m(row, r)?.shared2 = g(aa, psrc)? as isize;
            *m(aa, psrc)? = !r;
        }
    }
    let mut psrc = pdest;
    while psrc < pfree {
        let psrc_ = psrc;
        psrc += 1;
        if (g(aa, psrc_)? as isize) < 0 {
            psrc -= 1;
            let r = !g(aa, psrc)?;
            *m(aa, psrc)? = g(row, r)?.shared2 as usize;
            m(row, r)?.start = pdest as isize;
            let length = zx(g(row, r)?.length);
            for _ in 0..length {
                let c = g(aa, psrc)?;
                psrc += 1;
                if !g(col, c)?.is_dead() {
                    *m(aa, pdest)? = c;
                    pdest += 1;
                }
            }
            let rm = m(row, r)?;
            rm.length = pdest as isize - rm.start;
        }
    }
    Ok(pdest)
}

/// Convenience: COLAMD ordering as `(perm, perm_inv)` vectors.
///
/// # Errors
/// As [`order`].
pub fn order_vec(a: CscPattern<'_>, control: Control) -> Result<(Vec<usize>, Vec<usize>)> {
    let mut perm = vec![0usize; a.ncols];
    let mut perm_inv = vec![0usize; a.ncols];
    order(&mut perm, &mut perm_inv, a, control)?;
    Ok((perm, perm_inv))
}
