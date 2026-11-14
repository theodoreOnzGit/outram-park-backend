// Translated from NJOY2016.80 (commit ac5adf5) `src/matxsr.f90`, `subroutine cmatxs`
// (`:531-793`), its coded (`nmatx > 0`) branch.
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under AGPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! `cmatxs`: the MATXS records, written from the scratch files in the order
//! upstream reads them back.
//!
//! The formats are written as gfortran executes them: in `(' 7d ',8x,1p,5e12.5/
//! (6e12.5))` a list that ends exactly at the `/` still executes the `/`, so an
//! empty line follows (five words, or eight names in `' 3d '`/`' 6d '`).

use crate::matxsr::data::{Ctl2, Matxs};
use crate::NjoyError;

fn e125(x: f32) -> String {
    crate::acer::fortran_fmt::fortran_e(f64::from(x), 5, 12)
}

fn i6(n: i32) -> String {
    crate::viewr::fmt::i(i64::from(n), 6)
}

/// A format `(prefix, <first> items / (<rest> items))`.
fn slash_lines(out: &mut String, prefix: &str, pad: usize, first: usize, rest: usize, items: &[String]) {
    let mut line = prefix.to_string();
    if !items.is_empty() && first > 0 {
        line.push_str(&" ".repeat(pad));
    }
    let n1 = first.min(items.len());
    for it in &items[..n1] {
        line.push_str(it);
    }
    out.push_str(&line);
    out.push('\n');
    if items.len() < first {
        return;
    }
    if items.len() == first {
        out.push('\n');
        return;
    }
    for ch in items[first..].chunks(rest) {
        out.push_str(&ch.concat());
        out.push('\n');
    }
}

/// A format with no slash and `per` items a line (`(12i6)`).
fn plain_lines(out: &mut String, per: usize, items: &[String]) {
    if items.is_empty() {
        out.push('\n');
        return;
    }
    for ch in items.chunks(per) {
        out.push_str(&ch.concat());
        out.push('\n');
    }
}

/// `cmatxs` (`:531-793`): the coded MATXS file.
///
/// # Errors
/// A scratch file shorter than its control records say (upstream's read
/// would fail), or `nmatx < 0` (binary MATXS, not written here).
pub(crate) fn cmatxs(m: &mut Matxs) -> Result<String, NjoyError> {
    if m.c.units[2] < 0 {
        return Err(NjoyError::NotPorted("MATXSR with a binary (negative) nmatx unit"));
    }
    let mut out = String::new();
    let (npart, ntype, nmat) = (m.npart, m.ntype, m.nmat);
    out.push_str(&format!(" 0v {:>8}*{:<8.8}{:<8.8}*{}\n", "matxs ", m.c.tuse[0], m.c.tuse[1], i6(m.c.ivers)));
    let nholl = m.c.control[2] * 9;
    let ctl = [npart as i32, ntype as i32, nholl, nmat as i32, m.maxw, m.length + 5 + npart as i32];
    out.push_str(&format!(" 1d   {}\n", ctl.iter().map(|&v| i6(v)).collect::<String>()));
    let mut holl: Vec<String> = Vec::new();
    for t in &m.c.holl {
        let t = format!("{:<72.72}", t);
        for j in 0..9 {
            holl.push(t[8 * j..8 * j + 8].to_string());
        }
    }
    slash_lines(&mut out, " 2d ", 0, 0, 9, &holl);
    let mut names: Vec<String> = Vec::new();
    names.extend(m.c.hprt.iter().cloned());
    names.extend(m.c.htype.iter().cloned());
    names.extend(m.c.mats.iter().map(|x| x.0.clone()));
    slash_lines(&mut out, " 3d ", 4, 8, 9, &names);
    let mut ints: Vec<i32> = Vec::new();
    ints.extend(m.c.ngrp.iter().copied());
    ints.extend(m.c.jinp.iter().copied());
    ints.extend(m.c.joutp.iter().copied());
    ints.extend(m.nsubm.iter().copied());
    ints.extend(m.loca.iter().copied());
    plain_lines(&mut out, 12, &ints.iter().map(|&v| i6(v)).collect::<Vec<_>>());
    let mut lout = 0usize;
    for i in 0..npart {
        let nwds = m.c.ngrp[i] as usize + 1;
        let g: Vec<String> = m.groups[lout..lout + nwds].iter().map(|&v| e125(v)).collect();
        slash_lines(&mut out, " 4d ", 8, 5, 6, &g);
        lout += nwds;
    }
    let mut s2 = m.scr2.clone().into_iter();
    let mut s3 = m.scr3.clone().into_iter();
    let short = || NjoyError::InvalidInput("cmatxs: scratch file ends early".into());
    let read3 = |s3: &mut std::vec::IntoIter<Vec<f32>>, n: usize| -> Result<Vec<f32>, NjoyError> {
        let r = s3.next().ok_or_else(short)?;
        if r.len() < n {
            return Err(NjoyError::InvalidInput("cmatxs: record shorter than the read".into()));
        }
        Ok(r[..n].to_vec())
    };
    for mc in m.scr6.clone() {
        out.push_str(&format!(" 5d {:<8.8}{}\n", mc.name, e125(mc.awr)));
        for s in &mc.subs {
            out.push_str(&format!("{}{}{}{}{}{}\n", e125(s.0), e125(s.1), i6(s.2), i6(s.3), i6(s.4), i6(s.5)));
        }
        for s in &mc.subs {
            let itype = s.2 as usize;
            let (n1d, n2d) = (s.3 as usize, s.4 as usize);
            let ning = m.c.ngrp[m.c.jinp[itype - 1] as usize - 1] as usize;
            let noutg = m.c.ngrp[m.c.joutp[itype - 1] as usize - 1] as usize;
            let _ = ning;
            if n1d != 0 {
                let Some(Ctl2::Vector { names, ints }) = s2.next() else { return Err(short()) };
                slash_lines(&mut out, " 6d ", 4, 8, 9, &names);
                plain_lines(&mut out, 12, &ints.iter().map(|&v| i6(v)).collect::<Vec<_>>());
                let mut nwds = 0usize;
                for l in 0..n1d {
                    let nw = (ints[n1d + l] - ints[l] + 1) as usize;
                    if (nwds + nw) as i32 >= m.maxw {
                        let r = read3(&mut s3, nwds)?;
                        slash_lines(&mut out, " 7d ", 8, 5, 6, &r.iter().map(|&v| e125(v)).collect::<Vec<_>>());
                        nwds = 0;
                    }
                    nwds += nw;
                }
                if nwds > 0 {
                    let r = read3(&mut s3, nwds)?;
                    slash_lines(&mut out, " 7d ", 8, 5, 6, &r.iter().map(|&v| e125(v)).collect::<Vec<_>>());
                }
            }
            for _ in 0..n2d {
                let Some(Ctl2::Matrix { name, ints }) = s2.next() else { return Err(short()) };
                out.push_str(&format!(" 8d     {:<8.8}\n", name));
                plain_lines(&mut out, 12, &ints.iter().map(|&v| i6(v)).collect::<Vec<_>>());
                let lord = ints[0] as usize;
                let jconst = ints[1] as usize;
                let nwdc = noutg + jconst;
                if lord != 0 {
                    let mut nwds = 0usize;
                    for jj in 0..noutg {
                        let nw = ints[2 + jj].max(0) as usize;
                        if (nwds + nw * lord) as i32 >= m.maxw && nwds > 0 {
                            let r = read3(&mut s3, nwds)?;
                            slash_lines(&mut out, " 9d ", 8, 5, 6, &r.iter().map(|&v| e125(v)).collect::<Vec<_>>());
                            nwds = 0;
                        }
                        nwds += nw * lord;
                    }
                    if nwds > 0 {
                        let r = read3(&mut s3, nwds)?;
                        slash_lines(&mut out, " 9d ", 8, 5, 6, &r.iter().map(|&v| e125(v)).collect::<Vec<_>>());
                    }
                }
                if jconst > 0 {
                    let r = read3(&mut s3, nwdc)?;
                    slash_lines(&mut out, "10d ", 8, 5, 6, &r.iter().map(|&v| e125(v)).collect::<Vec<_>>());
                }
            }
        }
    }
    Ok(out)
}
