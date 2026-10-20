// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 Theodore Ong and the outram-park contributors.
//
// PORTED from the `faer` crate, version 0.24.4, file
// `src/sparse/linalg/amd.rs`, read from the published crates.io source on
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
// faer's amd.rs is itself a port of SuiteSparse AMD, whose notice faer ships
// as COPYING.SUITE_SPARSE.AMD.BSD and which is retained here:
//
//   AMD, Copyright (c), 1996-2022, Timothy A. Davis,
//   Patrick R. Amestoy, and Iain S. Duff.  All Rights Reserved.
//
//   AMD License: BSD 3-clause:
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
// Ported items (all from amd.rs): post_tree, postorder, flip, clear_flag,
// amd_2, amd_1, preprocess, aat, order, order_maybe_unsorted, Control,
// FlopCount. faer's `I::Signed` is `isize` here, `NONE` is `-1`, its
// `MemStack` scratch is replaced by `Vec`s, its `assert!`s are `ensure(..)?`
// and subscripts are checked accesses (see `super`'s module docs).

//! **AMD** — the approximate-minimum-degree symmetric ordering that
//! [`super::SparseLlt`] uses, ported from `faer` 0.24.4
//! (`sparse/linalg/amd.rs`), itself a port of SuiteSparse AMD. Integer-only
//! apart from the flop count, so the permutation reproduces faer's exactly.

#[allow(unused_imports)]
use crate::real::Real;

use super::{ensure, g, m, sl, CscPattern};
use crate::{PetirError, Result};
use alloc::vec;
use alloc::vec::Vec;

const NONE_S: isize = -1;
const NONE_U: usize = usize::MAX;

/// Tuning parameters (faer's `amd::Control`, same defaults).
#[derive(Debug, Copy, Clone, PartialEq)]
pub struct Control {
    /// "dense" if degree > dense * sqrt(n)
    pub dense: f64,
    /// do aggressive absorption
    pub aggressive: bool,
}

impl Default for Control {
    fn default() -> Self {
        Self {
            dense: 10.0,
            aggressive: true,
        }
    }
}

/// Flop count of the LDLᵀ and LU factorisations under the computed ordering.
#[derive(Default, Debug, Copy, Clone, PartialEq)]
pub struct FlopCount {
    /// number of divisions
    pub n_div: f64,
    /// multiplications and subtractions for LDLᵀ
    pub n_mult_subs_ldl: f64,
    /// multiplications and subtractions for LU
    pub n_mult_subs_lu: f64,
}

#[inline]
fn zx(x: isize) -> usize {
    x as usize
}

fn post_tree(
    root: usize,
    k: usize,
    child: &mut [isize],
    sibling: &[isize],
    order: &mut [isize],
    stack: &mut [usize],
) -> Result<usize> {
    let mut top = 1usize;
    *m(stack, 0)? = root;
    let mut k = k;
    while top > 0 {
        let i = g(stack, top - 1)?;
        let ci = g(child, i)?;
        if ci >= 0 {
            let child_ = zx(ci);
            let mut f = child_;
            loop {
                top += 1;
                let s = g(sibling, f)?;
                if s >= 0 {
                    f = zx(s);
                } else {
                    break;
                }
            }
            let mut t = top;
            let mut f = child_;
            loop {
                t -= 1;
                *m(stack, t)? = f;
                let s = g(sibling, f)?;
                if s >= 0 {
                    f = zx(s);
                } else {
                    break;
                }
            }
            *m(child, i)? = NONE_S;
        } else {
            top -= 1;
            *m(order, i)? = k as isize;
            k += 1;
        }
    }
    Ok(k)
}

fn postorder(order: &mut [isize], etree: &[isize], nv: &[isize], f_size: &[isize]) -> Result<()> {
    let n = order.len();
    if n == 0 {
        return Ok(());
    }
    let mut child = vec![NONE_S; n];
    let mut sibling = vec![NONE_S; n];
    let mut stack = vec![0usize; n];
    for j in (0..n).rev() {
        let pj = g(etree, j)?;
        if g(nv, j)? > 0 && pj >= 0 {
            let parent = zx(pj);
            *m(&mut sibling, j)? = g(&child, parent)?;
            *m(&mut child, parent)? = j as isize;
        }
    }
    for i in 0..n {
        let ci = g(&child, i)?;
        if g(nv, i)? > 0 && ci >= 0 {
            let child_ = zx(ci);
            let mut fprev = NONE_S;
            let mut bigfprev = NONE_S;
            let mut bigf = NONE_S;
            let mut maxfrsize = NONE_S;
            let mut f = child_ as isize;
            while f >= 0 {
                let f_ = zx(f);
                let frsize = g(f_size, f_)?;
                if frsize >= maxfrsize {
                    maxfrsize = frsize;
                    bigfprev = fprev;
                    bigf = f;
                }
                fprev = f;
                f = g(&sibling, f_)?;
            }
            ensure(bigf >= 0)?;
            let bigf = zx(bigf);
            let fnext = g(&sibling, bigf)?;
            if fnext >= 0 {
                if bigfprev >= 0 {
                    *m(&mut sibling, zx(bigfprev))? = fnext;
                } else {
                    *m(&mut child, i)? = fnext;
                }
                ensure(fprev >= 0)?;
                let fprev = zx(fprev);
                *m(&mut sibling, bigf)? = NONE_S;
                *m(&mut sibling, fprev)? = bigf as isize;
            }
        }
    }
    order.fill(NONE_S);
    let mut k = 0usize;
    for i in 0..n {
        if g(etree, i)? < 0 && g(nv, i)? > 0 {
            k = post_tree(i, k, &mut child, &sibling, order, &mut stack)?;
        }
    }
    Ok(())
}

#[inline(always)]
fn flip(i: isize) -> isize {
    -2 - i
}

fn clear_flag(wflg: isize, wbig: isize, w: &mut [isize]) -> isize {
    if wflg < 2 || wflg >= wbig {
        for x in w {
            if *x != 0 {
                *x = 1;
            }
        }
        return 2;
    }
    wflg
}

/// Unlink `i` from its degree list (a step faer repeats inline).
#[inline(always)]
fn unlink(
    i: usize,
    last: &mut [isize],
    next: &mut [isize],
    head: &mut [isize],
    degree: &[isize],
) -> Result<()> {
    let ilast = g(last, i)?;
    let inext = g(next, i)?;
    if inext != NONE_S {
        *m(last, zx(inext))? = ilast;
    }
    if ilast != NONE_S {
        *m(next, zx(ilast))? = inext;
    } else {
        *m(head, zx(g(degree, i)?))? = inext;
    }
    Ok(())
}

#[allow(clippy::comparison_chain, clippy::too_many_arguments)]
fn amd_2(
    pe: &mut [isize],
    iw: &mut [isize],
    len: &mut [isize],
    pfree: usize,
    next: &mut [isize],
    last: &mut [isize],
    control: Control,
) -> Result<FlopCount> {
    let n = pe.len();
    ensure(n < isize::MAX as usize)?;
    let mut pfree = pfree;
    let iwlen = iw.len();
    let none = NONE_S;
    let alpha = control.dense;
    let aggressive = control.aggressive;
    let mut mindeg = 0usize;
    let mut ncmpa = 0usize;
    let mut lemax = 0usize;
    let mut ndiv = 0.0f64;
    let mut nms_lu = 0.0f64;
    let mut nms_ldl = 0.0f64;
    let mut nel = 0usize;
    let mut me = none;
    let dense = if alpha < 0.0 {
        n - 2
    } else {
        (alpha * Real::sqrt(n as f64)) as usize
    };
    let dense = Ord::max(dense, 16);
    let dense = Ord::min(dense, n);
    let mut w = vec![1isize; n];
    let mut nv = vec![1isize; n];
    let mut elen = vec![0isize; n];
    let wbig = isize::MAX - n as isize;
    let mut wflg = 0isize;
    let mut ndense = 0isize;
    {
        let mut head = vec![none; n];
        let mut degree = vec![none; n];
        last.fill(none);
        head.fill(none);
        next.fill(none);
        ensure(degree.len() == len.len())?;
        degree.copy_from_slice(len);
        for i in 0..n {
            let deg = zx(g(&degree, i)?);
            if deg == 0 {
                *m(&mut elen, i)? = flip(1);
                nel += 1;
                *m(pe, i)? = none;
                *m(&mut w, i)? = 0;
            } else if deg > dense {
                ndense += 1;
                *m(&mut nv, i)? = 0;
                *m(&mut elen, i)? = none;
                *m(pe, i)? = none;
                nel += 1;
            } else {
                let inext = g(&head, deg)?;
                if inext != none {
                    *m(last, zx(inext))? = i as isize;
                }
                *m(next, i)? = inext;
                *m(&mut head, deg)? = i as isize;
            }
        }
        while nel < n {
            let mut deg = mindeg;
            while deg < n {
                me = g(&head, deg)?;
                if me != none {
                    break;
                }
                deg += 1;
            }
            mindeg = deg;
            let me = zx(me);
            let inext = g(next, me)?;
            if inext != none {
                *m(last, zx(inext))? = none;
            }
            *m(&mut head, deg)? = inext;
            let elenme = g(&elen, me)?;
            let mut nvpiv = g(&nv, me)?;
            nel += zx(nvpiv);
            *m(&mut nv, me)? = -nvpiv;
            let mut degme = 0usize;
            let mut pme1;
            let mut pme2;
            if elenme == 0 {
                pme1 = g(pe, me)?;
                pme2 = pme1 - 1;
                for p in zx(pme1)..zx(pme1 + g(len, me)?) {
                    let i = zx(g(iw, p)?);
                    let nvi = g(&nv, i)?;
                    if nvi > 0 {
                        degme += zx(nvi);
                        *m(&mut nv, i)? = -nvi;
                        pme2 += 1;
                        *m(iw, zx(pme2))? = i as isize;
                        unlink(i, last, next, &mut head, &degree)?;
                    }
                }
            } else {
                let mut p = zx(g(pe, me)?);
                pme1 = pfree as isize;
                let slenme = zx(g(len, me)? - elenme);
                for knt1 in 1..zx(elenme) + 2 {
                    let e;
                    let mut pj;
                    let ln;
                    if knt1 as isize > elenme {
                        e = me;
                        pj = p as isize;
                        ln = slenme;
                    } else {
                        e = zx(g(iw, p)?);
                        p += 1;
                        pj = g(pe, e)?;
                        ln = zx(g(len, e)?);
                    }
                    for knt2 in 1..ln + 1 {
                        let i = zx(g(iw, zx(pj))?);
                        pj += 1;
                        let nvi = g(&nv, i)?;
                        if nvi > 0 {
                            if pfree >= iwlen {
                                *m(pe, me)? = p as isize;
                                *m(len, me)? -= knt1 as isize;
                                if g(len, me)? == 0 {
                                    *m(pe, me)? = none;
                                }
                                *m(pe, e)? = pj;
                                *m(len, e)? = (ln - knt2) as isize;
                                if g(len, e)? == 0 {
                                    *m(pe, e)? = none;
                                }
                                ncmpa += 1;
                                for (j, pe) in pe.iter_mut().enumerate() {
                                    let pn = *pe;
                                    if pn >= 0 {
                                        let pn = zx(pn);
                                        *pe = g(iw, pn)?;
                                        *m(iw, pn)? = flip(j as isize);
                                    }
                                }
                                let mut psrc = 0usize;
                                let mut pdst = 0usize;
                                let pend = zx(pme1);
                                while psrc < pend {
                                    let j = flip(g(iw, psrc)?);
                                    psrc += 1;
                                    if j >= 0 {
                                        let j = zx(j);
                                        *m(iw, pdst)? = g(pe, j)?;
                                        *m(pe, j)? = pdst as isize;
                                        pdst += 1;
                                        let lenj = zx(g(len, j)?);
                                        if lenj > 0 {
                                            ensure(
                                                psrc + lenj - 1 <= iw.len()
                                                    && pdst + lenj - 1 <= iw.len(),
                                            )?;
                                            iw.copy_within(psrc..psrc + lenj - 1, pdst);
                                            psrc += lenj - 1;
                                            pdst += lenj - 1;
                                        }
                                    }
                                }
                                let p1 = pdst;
                                ensure(zx(pme1) <= pfree && pfree <= iw.len())?;
                                ensure(pdst + (pfree - zx(pme1)) <= iw.len())?;
                                iw.copy_within(zx(pme1)..pfree, pdst);
                                pdst += pfree - zx(pme1);
                                pme1 = p1 as isize;
                                pfree = pdst;
                                pj = g(pe, e)?;
                                p = zx(g(pe, me)?);
                            }
                            degme += zx(nvi);
                            *m(&mut nv, i)? = -nvi;
                            *m(iw, pfree)? = i as isize;
                            pfree += 1;
                            unlink(i, last, next, &mut head, &degree)?;
                        }
                    }
                    if e != me {
                        *m(pe, e)? = flip(me as isize);
                        *m(&mut w, e)? = 0;
                    }
                }
                pme2 = pfree as isize - 1;
            }
            *m(&mut degree, me)? = degme as isize;
            *m(pe, me)? = pme1;
            *m(len, me)? = pme2 - pme1 + 1;
            *m(&mut elen, me)? = flip(nvpiv + degme as isize);
            wflg = clear_flag(wflg, wbig, &mut w);
            ensure(pme1 >= 0)?;
            for pme in zx(pme1)..zx(pme2 + 1) {
                let i = zx(g(iw, pme)?);
                let eln = g(&elen, i)?;
                if eln > 0 {
                    let nvi = -g(&nv, i)?;
                    let wnvi = wflg - nvi;
                    let start = zx(g(pe, i)?);
                    for &e in sl(iw, start..start + zx(eln))? {
                        let e = zx(e);
                        let mut we = g(&w, e)?;
                        if we >= wflg {
                            we -= nvi;
                        } else if we != 0 {
                            we = g(&degree, e)? + wnvi;
                        }
                        *m(&mut w, e)? = we;
                    }
                }
            }
            for pme in zx(pme1)..zx(pme2 + 1) {
                let i = zx(g(iw, pme)?);
                let p1 = zx(g(pe, i)?);
                let p2 = p1 + zx(g(&elen, i)?);
                let mut pn = p1;
                let mut hash = 0usize;
                deg = 0usize;
                if aggressive {
                    for p in p1..p2 {
                        let e = zx(g(iw, p)?);
                        let we = g(&w, e)?;
                        if we != 0 {
                            let dext = we - wflg;
                            if dext > 0 {
                                deg += zx(dext);
                                *m(iw, pn)? = e as isize;
                                pn += 1;
                                hash = hash.wrapping_add(e);
                            } else {
                                *m(pe, e)? = flip(me as isize);
                                *m(&mut w, e)? = 0;
                            }
                        }
                    }
                } else {
                    for p in p1..p2 {
                        let e = zx(g(iw, p)?);
                        let we = g(&w, e)?;
                        if we != 0 {
                            let dext = zx(we - wflg);
                            deg += dext;
                            *m(iw, pn)? = e as isize;
                            pn += 1;
                            hash = hash.wrapping_add(e);
                        }
                    }
                }
                *m(&mut elen, i)? = (pn - p1 + 1) as isize;
                let p3 = pn;
                let p4 = p1 + zx(g(len, i)?);
                for p in p2..p4 {
                    let j = zx(g(iw, p)?);
                    let nvj = g(&nv, j)?;
                    if nvj > 0 {
                        deg += zx(nvj);
                        *m(iw, pn)? = j as isize;
                        pn += 1;
                        hash = hash.wrapping_add(j);
                    }
                }
                if g(&elen, i)? == 1 && p3 == pn {
                    *m(pe, i)? = flip(me as isize);
                    let nvi = -g(&nv, i)?;
                    ensure(nvi >= 0)?;
                    degme -= zx(nvi);
                    nvpiv += nvi;
                    nel += zx(nvi);
                    *m(&mut nv, i)? = 0;
                    *m(&mut elen, i)? = none;
                } else {
                    let di = g(&degree, i)?;
                    *m(&mut degree, i)? = Ord::min(di, deg as isize);
                    *m(iw, pn)? = g(iw, p3)?;
                    *m(iw, p3)? = g(iw, p1)?;
                    *m(iw, p1)? = me as isize;
                    *m(len, i)? = (pn - p1 + 1) as isize;
                    hash %= n;
                    let j = g(&head, hash)?;
                    if j <= none {
                        *m(next, i)? = flip(j);
                        *m(&mut head, hash)? = flip(i as isize);
                    } else {
                        *m(next, i)? = g(last, zx(j))?;
                        *m(last, zx(j))? = i as isize;
                    }
                    *m(last, i)? = hash as isize;
                }
            }
            *m(&mut degree, me)? = degme as isize;
            lemax = Ord::max(lemax, degme);
            wflg += lemax as isize;
            wflg = clear_flag(wflg, wbig, &mut w);
            for pme in zx(pme1)..zx(pme2 + 1) {
                let mut i = zx(g(iw, pme)?);
                if g(&nv, i)? < 0 {
                    let hash = zx(g(last, i)?);
                    let j = g(&head, hash)?;
                    if j == none {
                        i = NONE_U;
                    } else if j < none {
                        i = zx(flip(j));
                        *m(&mut head, hash)? = none;
                    } else {
                        i = zx(g(last, zx(j))?);
                        *m(last, zx(j))? = none;
                    }
                    while i != NONE_U && g(next, i)? != none {
                        let ln = g(len, i)?;
                        let eln = g(&elen, i)?;
                        let pei = g(pe, i)?;
                        for p in zx(pei + 1)..zx(pei + ln) {
                            *m(&mut w, zx(g(iw, p)?))? = wflg;
                        }
                        let mut jlast = i;
                        let mut j = zx(g(next, i)?);
                        while j != NONE_U {
                            let mut ok = g(len, j)? == ln && g(&elen, j)? == eln;
                            if ok {
                                let pej = g(pe, j)?;
                                for p in zx(pej + 1)..zx(pej + ln) {
                                    if g(&w, zx(g(iw, p)?))? != wflg {
                                        ok = false;
                                        break;
                                    }
                                }
                            }
                            if ok {
                                *m(pe, j)? = flip(i as isize);
                                let nvj = g(&nv, j)?;
                                *m(&mut nv, i)? += nvj;
                                *m(&mut nv, j)? = 0;
                                *m(&mut elen, j)? = none;
                                j = zx(g(next, j)?);
                                *m(next, jlast)? = j as isize;
                            } else {
                                jlast = j;
                                j = zx(g(next, j)?);
                            }
                        }
                        wflg += 1;
                        i = zx(g(next, i)?);
                    }
                }
            }
            let mut p = zx(pme1);
            let nleft = n - nel;
            for pme in zx(pme1)..zx(pme2 + 1) {
                let i = zx(g(iw, pme)?);
                let nvi = -g(&nv, i)?;
                if nvi > 0 {
                    *m(&mut nv, i)? = nvi;
                    deg = zx(g(&degree, i)?) + degme - zx(nvi);
                    deg = Ord::min(deg, nleft - zx(nvi));
                    let inext = g(&head, deg)?;
                    if inext != none {
                        *m(last, zx(inext))? = i as isize;
                    }
                    *m(next, i)? = inext;
                    *m(last, i)? = none;
                    *m(&mut head, deg)? = i as isize;
                    mindeg = Ord::min(mindeg, deg);
                    *m(&mut degree, i)? = deg as isize;
                    *m(iw, p)? = i as isize;
                    p += 1;
                }
            }
            *m(&mut nv, me)? = nvpiv;
            *m(len, me)? = p as isize - pme1;
            if g(len, me)? == 0 {
                *m(pe, me)? = none;
                *m(&mut w, me)? = 0;
            }
            if elenme != 0 {
                pfree = p;
            }
            {
                let f = nvpiv as f64;
                let r = degme as f64 + ndense as f64;
                let lnzme = f * r + (f - 1.0) * f / 2.0;
                ndiv += lnzme;
                let s = f * r * r + r * (f - 1.0) * f + (f - 1.0) * f * (2.0 * f - 1.0) / 6.0;
                nms_lu += s;
                nms_ldl += (s + lnzme) / 2.0;
            }
        }
        {
            let f = ndense as f64;
            let lnzme = (f - 1.0) * f / 2.0;
            ndiv += lnzme;
            let s = (f - 1.0) * f * (2.0 * f - 1.0) / 6.0;
            nms_lu += s;
            nms_ldl += (s + lnzme) / 2.0;
        }
        for pe in pe.iter_mut() {
            *pe = flip(*pe);
        }
        for elen in elen.iter_mut() {
            *elen = flip(*elen);
        }
        for i in 0..n {
            if g(&nv, i)? == 0 {
                let mut j = zx(g(pe, i)?);
                if j == NONE_U {
                    continue;
                }
                while g(&nv, j)? == 0 {
                    j = zx(g(pe, j)?);
                }
                let e = j as isize;
                let mut j = i;
                while g(&nv, j)? == 0 {
                    let jnext = g(pe, j)?;
                    *m(pe, j)? = e;
                    j = zx(jnext);
                }
            }
        }
    }
    postorder(&mut w, pe, &nv, &elen)?;
    let mut head = vec![none; n];
    next.fill(none);
    for (e, &k) in w.iter().enumerate() {
        if k != none {
            *m(&mut head, zx(k))? = e as isize;
        }
    }
    nel = 0;
    for &e in head.iter() {
        if e == none {
            break;
        }
        let e = zx(e);
        *m(next, e)? = nel as isize;
        nel += zx(g(&nv, e)?);
    }
    ensure(nel == n - zx(ndense))?;
    for i in 0..n {
        if g(&nv, i)? == 0 {
            let e = g(pe, i)?;
            if e != none {
                let e = zx(e);
                *m(next, i)? = g(next, e)?;
                *m(next, e)? += 1;
            } else {
                *m(next, i)? = nel as isize;
                nel += 1;
            }
        }
    }
    ensure(nel == n)?;
    for i in 0..n {
        *m(last, zx(g(next, i)?))? = i as isize;
    }
    let _ = ncmpa;
    Ok(FlopCount {
        n_div: ndiv,
        n_mult_subs_ldl: nms_ldl,
        n_mult_subs_lu: nms_lu,
    })
}

#[allow(clippy::comparison_chain)]
fn amd_1(
    perm: &mut [isize],
    perm_inv: &mut [isize],
    a: CscPattern<'_>,
    len: &mut [isize],
    iwlen: usize,
    control: Control,
) -> Result<FlopCount> {
    let n = perm.len();
    let mut p_e = vec![0isize; n];
    let mut s_p = vec![0isize; n];
    let mut i_w = vec![0isize; iwlen];
    let mut pfree = 0isize;
    for j in 0..n {
        *m(&mut p_e, j)? = pfree;
        *m(&mut s_p, j)? = pfree;
        pfree += g(len, j)?;
    }
    let pfree = zx(pfree);
    ensure(iwlen >= pfree + n)?;
    {
        let mut t_p = vec![0isize; n];
        for k in 0..n {
            let mut seen = 0isize;
            let mut j_prev = NONE_U;
            for &j in a.col(k)? {
                if j_prev == j {
                    j_prev = j;
                    continue;
                }
                j_prev = j;
                if j < k {
                    *m(&mut i_w, zx(g(&s_p, j)?))? = k as isize;
                    *m(&mut s_p, j)? += 1;
                    *m(&mut i_w, zx(g(&s_p, k)?))? = j as isize;
                    *m(&mut s_p, k)? += 1;
                    seen += 1;
                } else {
                    if j == k {
                        seen += 1;
                    }
                    break;
                }
                let mut seen_j = 0isize;
                let mut i_prev = NONE_U;
                let colj = a.col(j)?;
                for &i in colj.get(zx(g(&t_p, j)?)..).ok_or(PetirError::Invalid)? {
                    if i_prev == i {
                        i_prev = i;
                        continue;
                    }
                    i_prev = i;
                    if i < k {
                        *m(&mut i_w, zx(g(&s_p, i)?))? = j as isize;
                        *m(&mut s_p, i)? += 1;
                        *m(&mut i_w, zx(g(&s_p, j)?))? = i as isize;
                        *m(&mut s_p, j)? += 1;
                        seen_j += 1;
                    } else {
                        if i == k {
                            seen_j += 1;
                        }
                        break;
                    }
                }
                *m(&mut t_p, j)? += seen_j;
            }
            *m(&mut t_p, k)? = seen;
        }
        for j in 0..n {
            let mut i_prev = NONE_U;
            let colj = a.col(j)?;
            for &i in colj.get(zx(g(&t_p, j)?)..).ok_or(PetirError::Invalid)? {
                if i_prev == i {
                    i_prev = i;
                    continue;
                }
                i_prev = i;
                *m(&mut i_w, zx(g(&s_p, i)?))? = j as isize;
                *m(&mut s_p, i)? += 1;
                *m(&mut i_w, zx(g(&s_p, j)?))? = i as isize;
                *m(&mut s_p, j)? += 1;
            }
        }
    }
    amd_2(&mut p_e, &mut i_w, len, pfree, perm_inv, perm, control)
}

/// Transpose-and-deduplicate the pattern (faer's `amd::preprocess`).
fn preprocess(a: CscPattern<'_>) -> Result<(Vec<usize>, Vec<usize>)> {
    let n = a.nrows;
    let mut w = vec![0usize; n];
    let mut flag = vec![NONE_U; n];
    for j in 0..n {
        for &i in a.col(j)? {
            if g(&flag, i)? != j {
                *m(&mut w, i)? += 1;
                *m(&mut flag, i)? = j;
            }
        }
    }
    let mut new_col_ptr = vec![0usize; n + 1];
    for (i, &wi) in w.iter().enumerate() {
        let prev = g(&new_col_ptr, i)?;
        *m(&mut new_col_ptr, i + 1)? = prev + wi;
    }
    for (wi, &c) in w.iter_mut().zip(&new_col_ptr) {
        *wi = c;
    }
    flag.fill(NONE_U);
    let mut new_row_idx = vec![0usize; g(&new_col_ptr, n)?];
    for j in 0..n {
        for &i in a.col(j)? {
            if g(&flag, i)? != j {
                *m(&mut new_row_idx, g(&w, i)?)? = j;
                *m(&mut w, i)? += 1;
                *m(&mut flag, i)? = j;
            }
        }
    }
    Ok((new_col_ptr, new_row_idx))
}

#[allow(clippy::comparison_chain)]
fn aat(len: &mut [isize], a: CscPattern<'_>) -> Result<usize> {
    let n = a.nrows;
    let mut t_p = vec![0isize; n];
    for k in 0..n {
        let mut seen = 0isize;
        let mut j_prev = NONE_U;
        for &j in a.col(k)? {
            if j_prev == j {
                j_prev = j;
                continue;
            }
            j_prev = j;
            if j < k {
                seen += 1;
                *m(len, j)? += 1;
                *m(len, k)? += 1;
            } else {
                if j == k {
                    seen += 1;
                }
                break;
            }
            let mut seen_j = 0isize;
            let mut i_prev = NONE_U;
            let colj = a.col(j)?;
            for &i in colj.get(zx(g(&t_p, j)?)..).ok_or(PetirError::Invalid)? {
                if i_prev == i {
                    i_prev = i;
                    continue;
                }
                i_prev = i;
                if i < k {
                    *m(len, i)? += 1;
                    *m(len, j)? += 1;
                    seen_j += 1;
                } else {
                    if i == k {
                        seen_j += 1;
                    }
                    break;
                }
            }
            *m(&mut t_p, j)? += seen_j;
        }
        *m(&mut t_p, k)? = seen;
    }
    for j in 0..n {
        let mut i_prev = NONE_U;
        let colj = a.col(j)?;
        for &i in colj.get(zx(g(&t_p, j)?)..).ok_or(PetirError::Invalid)? {
            if i_prev == i {
                i_prev = i;
                continue;
            }
            i_prev = i;
            *m(len, i)? += 1;
            *m(len, j)? += 1;
        }
    }
    Ok(len.iter().map(|&l| zx(l)).sum())
}

/// AMD ordering of a **sorted** pattern (faer's `amd::order`). Writes the
/// forward permutation to `perm` and its inverse to `perm_inv`.
///
/// # Errors
/// `PetirError::Invalid` where faer would panic (a malformed pattern).
pub fn order(
    perm: &mut [usize],
    perm_inv: &mut [usize],
    a: CscPattern<'_>,
    control: Control,
) -> Result<FlopCount> {
    let n = perm.len();
    if n == 0 {
        return Ok(FlopCount::default());
    }
    let mut len = vec![0isize; n];
    let nzaat = aat(&mut len, a)?;
    let iwlen = nzaat + nzaat / 5 + n;
    let mut p = vec![0isize; n];
    let mut p_inv = vec![0isize; n];
    let flops = amd_1(&mut p, &mut p_inv, a, &mut len, iwlen, control)?;
    for (dst, &src) in perm.iter_mut().zip(&p) {
        *dst = zx(src);
    }
    for (dst, &src) in perm_inv.iter_mut().zip(&p_inv) {
        *dst = zx(src);
    }
    Ok(flops)
}

/// AMD ordering of the pattern of `A + Aᵀ` for a possibly **unsorted**
/// pattern (faer's `amd::order_maybe_unsorted`, which the Cholesky uses).
///
/// # Errors
/// As [`order`].
pub fn order_maybe_unsorted(
    perm: &mut [usize],
    perm_inv: &mut [usize],
    a: CscPattern<'_>,
    control: Control,
) -> Result<FlopCount> {
    let n = perm.len();
    if n == 0 {
        return Ok(FlopCount::default());
    }
    let (col_ptr, row_idx) = preprocess(a)?;
    let pre = CscPattern {
        nrows: n,
        ncols: n,
        col_ptr: &col_ptr,
        row_idx: &row_idx,
    };
    order(perm, perm_inv, pre, control)
}
