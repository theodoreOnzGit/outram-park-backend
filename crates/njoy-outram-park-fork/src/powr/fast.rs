// SPDX-License-Identifier: AGPL-3.0-only
// Translated from NJOY2016.80 `src/powr.f90` (git commit ac5adf5f33d893e42f2eed7fb286b0d51c7580da):
//   - `fast` (l.362-651), `mergt` (l.752-779), `gamll` (l.781-1020),
//     `gamxs` (l.1022-1399); `gamff` is in `gamff.rs`.
//   `pgam` and `pgamff` (l.653-750) print to the listing only and are not
//   translated (the listing is not written).
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under AGPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! `lib = 1`: the EPRI-CELL fast library (input for GAMTAP).
//!
//! The control flow keeps upstream's statement labels as a `match` over
//! [`L`]-named states, so each arm can be read beside the Fortran.
//!
//! Upstream quirks kept, on purpose:
//! - a card 3 with `matd < 0` ends the run like `matd = 0`: the
//!   absorption-only branch (`iread = 1`, card 6) sits inside `if (matd.gt.0)`
//!   and cannot be reached, so it is not translated;
//! - `gamxs`'s fission-spectrum part writes chi at `jg2c = ngnd-ig2c-1`
//!   (two groups low, the last two words landing in the fission cross
//!   section), and its delayed-neutron branch reads `a(jgt+locb)` with an
//!   `scr` offset;
//! - `gamff` does not reset `l` after paging a record.
//! - `gamll`'s (n,2n) branch (label 390) returns to 250, not 300, so with
//!   more group records to come it reads their lines as CONT records
//!   (`contio`'s `6e11.0` accepts a data line) until the section ends.
//!
//! Refused by name, because upstream's result is undefined: delayed-neutron
//! normalisation with no MF=5 MT=455 (`nldla` undefined).

use super::fmt::{chars, i, six_e12_5};
use super::reader::{Reader, NWSCR};
use super::FastMaterial;
use crate::NjoyError;

fn nint(x: f64) -> i32 {
    x.round() as i32
}

fn err(sub: &str, msg: &str) -> NjoyError {
    NjoyError::InvalidInput(format!("{sub}: {msg}"))
}

/// The pointers `gamxs` and `gamff` set (`fast`'s `loc(10)`), 1-based into `a`.
#[derive(Debug, Clone, Copy, Default)]
pub(super) struct Locs {
    pub(super) locab0: usize,
    pub(super) locsf0: usize,
    pub(super) locchi: usize,
    pub(super) locnus: usize,
    pub(super) loce0: usize,
    pub(super) loce1: usize,
    pub(super) locin: usize,
    pub(super) loc2n: usize,
    pub(super) locabs: usize,
    pub(super) locsf: usize,
}

/// `fast(iprint,igprnt)` (`powr.f90:362-651`) on the materials of cards
/// 3-5. Returns the text `mergt` copies to `nout`.
///
/// # Errors
/// Upstream's `error` stops, and the refusals named in the module doc.
pub fn fast(r: &mut Reader, mats: &[FastMaterial]) -> Result<String, NjoyError> {
    let na = 10000usize;
    let mut a = vec![0.0f64; na + 1];
    let mut nscr: Vec<String> = Vec::new();
    let mut kscr: Vec<String> = Vec::new();
    let mut igprnt = 0;
    let misc = 1;
    let izero = 0;
    let mut ifiss = 0;
    r.ngnd = 68;
    let ngmin = 19usize;
    let ngmax = 62usize;
    r.nff = 1;
    r.matd = 1;
    let ngnd = 68usize;
    for m in mats {
        if m.matd <= 0 {
            break;
        }
        r.matd = m.matd;
        r.rtemp = m.rtemp;
        let mut iff = m.iff;
        let nsgz = m.nsgz;
        r.izref = m.izref;
        // read(hid,'(4a4)') word: 12 characters and four blanks
        let word = chars(&m.hid, 16);
        // read(fsp,'(10a4)') fsn
        let fsn = chars(&m.fsp, 40);

        gamll(r, &mut igprnt, &mut iff, nsgz)?;
        let locs = gamxs(r, &mut a, na)?;
        let locs = if r.nff > 0 && r.iwr > 0 { super::gamff::gamff(r, ngmin, ngmax, locs, &mut a, na)? } else { locs };

        let xld1 = r.xld[1] + 1.0;
        let xld2 = r.xld[2] + 1.0;
        let xld3 = r.xld[3] + 1.0;
        if r.nfs + r.iwf != 0 {
            ifiss = r.iwf + r.nfs;
            kscr.push(format!("{}{}{}", i(r.nid, 6), i(izero, 2), fsn));
            let mut l = locs.locchi;
            let mut lim = l + ngnd - 1;
            six_e12_5(&mut kscr, &a[l..=lim]);
            if r.nfs != 0 {
                for ifs in 1..=r.nfs {
                    kscr.push(format!("{}{}{}", i(r.nid, 6), i(ifs, 2), fsn));
                    l += ngnd;
                    lim += ngnd;
                    six_e12_5(&mut kscr, &a[l..=lim]);
                }
                r.nfs += r.iwf;
            }
        }

        nscr.push(format!("{}{}{}{}", i(r.nid, 10), i(r.iwa, 10), i(r.iwf, 10), i(r.iwr, 10)));
        nscr.push(word.clone());
        let mut v = Vec::new();
        six_e12_5(&mut v, &[r.xlol[3], r.xla[3], xld3]);
        nscr.push(format!("{:36}{}", "", v[0]));
        six_e12_5(&mut nscr, &[r.xlol[1], r.xla[1], xld1, r.xlol[2], r.xla[2], xld2]);
        if r.iwa != 0 {
            let lim = locs.locab0 + ngnd - 1;
            six_e12_5(&mut nscr, &a[locs.locab0..=lim]);
        }
        if r.iwf != 0 {
            let limsf = locs.locsf0 + ngnd - 1;
            six_e12_5(&mut nscr, &a[locs.locsf0..=limsf]);
            let lim = locs.locnus + ngnd - 1;
            six_e12_5(&mut nscr, &a[locs.locnus..=lim]);
        }
        if r.xlol[3] != 0.0 {
            let n = nint(r.xlol[3]) as usize;
            six_e12_5(&mut nscr, &a[locs.loce0..locs.loce0 + n]);
            six_e12_5(&mut nscr, &a[locs.loce1..locs.loce1 + n]);
        }
        if r.xlol[1] != 0.0 {
            let n = nint(r.xlol[1]) as usize;
            six_e12_5(&mut nscr, &a[locs.locin..locs.locin + n]);
        }
        if r.xlol[2] != 0.0 {
            let n = nint(r.xlol[2]) as usize;
            six_e12_5(&mut nscr, &a[locs.loc2n..locs.loc2n + n]);
        }
        if r.nff != 0 && r.iwr != 0 {
            let (nsigz, ntp, iwr) = (r.nsigz as usize, r.ntp as usize, r.iwr as usize);
            nscr.push(format!("{}{}{}{}", i(r.nsigz, 6), i(r.ntp, 6), i(misc, 6), i(r.jwf, 6)));
            six_e12_5(&mut nscr, &r.sigz[1..=nsigz]);
            six_e12_5(&mut nscr, &r.tmpr[1..=ntp]);
            let istart = locs.locabs + (ngmax - iwr) * nsigz * ntp;
            let lim = istart + iwr * nsigz * ntp - 1;
            six_e12_5(&mut nscr, &a[istart..=lim]);
            if r.jwf != 0 {
                let istart = locs.locsf + (ngmax - iwr) * nsigz * ntp;
                let lim = istart + iwr * nsigz * ntp - 1;
                six_e12_5(&mut nscr, &a[istart..=lim]);
            }
            // a dummy amisc array
            six_e12_5(&mut nscr, &vec![0.0; iwr * 10]);
        }
    }
    Ok(mergt(ifiss, &kscr, &nscr))
}

/// `mergt(ifiss,kscr,nscr,nout,z)` (`powr.f90:752-779`): `kscr` (when
/// `ifiss > 0`) then `nscr`, each record read and written `(20a4)`, so cut or
/// blank-padded to 80 columns.
fn mergt(ifiss: i32, n1: &[String], n2: &[String]) -> String {
    let mut out = String::new();
    let first: &[String] = if ifiss > 0 { n1 } else { &[] };
    for l in first.iter().chain(n2.iter()) {
        out.push_str(&chars(l, 80));
        out.push('\n');
    }
    out
}

/// The labels of `gamll`.
#[derive(Clone, Copy)]
enum L {
    L130,
    L140,
    L150,
    L220,
    L250,
    L260,
    L270,
    L300,
    L350,
    L360,
    L370,
    L390,
    L400,
}

/// `gamll(igprnt,iff,nsgz)` (`powr.f90:781-1020`): read through the GENDF
/// and set the matrix shapes (`xla`, `xld`, `xlol`) and the counts.
fn gamll(r: &mut Reader, igprnt: &mut i32, iff: &mut i32, nsgz: i32) -> Result<(), NjoyError> {
    let eps = 1.0e-4;
    let ntpmax = 5;
    let nszmax = 7;
    let ngnd = r.ngnd;
    r.repoz();
    r.tpidio()?;
    // 110
    loop {
        r.contio(1)?;
        if r.h.math == -1 {
            return Err(err("gamll", &format!("mat={:4} not on tape {:3}", r.matd, 0)));
        }
        if r.h.math == 0 || r.h.mfh == 0 || r.h.mth == 0 {
            continue;
        }
        if r.h.math == r.matd {
            break;
        }
        r.tomend()?;
    }
    // 120
    r.iwa = 1;
    r.iwf = 0;
    r.nfs = 0;
    r.iwr = 0;
    r.ntp = -1;
    r.jwf = 0;
    let (mut ldin, mut ldp, mut ld2n) = (0, 0, 0);
    let lz = 6usize;
    let iza = nint(r.h.c1h);
    r.nid = iza * 10;
    let (mut lain, mut la2n, mut lap) = (0, 0, 0);
    let mut ig = 0;
    let mut igc = 0;
    let mut ig2loc = 0;
    let mut temp = 0.0;
    let mut lab = L::L140;
    loop {
        match lab {
            L::L130 => {
                r.contio(1)?;
                lab = if r.h.math != r.matd { L::L400 } else { L::L140 };
            }
            L::L140 => {
                r.nsigz = r.h.l2h;
                if r.nsigz == 1 {
                    *iff = 0;
                }
                if r.nsigz > 1 && *iff > 0 {
                    r.iwr = 1;
                }
                if r.nsigz > nsgz && nsgz > 0 {
                    r.nsigz = nsgz;
                }
                if r.nsigz > nszmax {
                    return Err(err("gamll", &format!("reqested too many sigma zeroes {nszmax:2} allowed")));
                }
                let ntw = r.h.n2h;
                r.listio(1)?;
                temp = r.h.c1h;
                if (temp - r.rtemp).abs() <= eps {
                    // 160
                    r.ngn = r.h.l1h;
                    gamll_160(r, igprnt, ntw, lz, temp)?;
                    lain = ngnd;
                    la2n = ngnd;
                    lap = ngnd;
                    lab = L::L220;
                    continue;
                }
                if temp > r.rtemp && r.ntp < 0 {
                    return Err(err("gamll", "reference temp not on gendf tape."));
                }
                if r.ntp < 0 {
                    lab = L::L150;
                    continue;
                }
                r.ntp += 1;
                if r.ntp > ntpmax {
                    return Err(err("gamll", &format!("requested too many temperatures {ntpmax:2} allowed")));
                }
                r.tmpr[r.ntp as usize] = temp;
                lab = L::L220;
            }
            L::L150 => {
                r.tomend()?;
                lab = L::L130;
            }
            L::L220 => {
                r.tosend()?;
                lab = L::L250;
            }
            L::L250 => {
                r.contio(1)?;
                lab = if r.h.math == 0 && r.nsigz > 1 {
                    L::L130
                } else if r.h.math != r.matd {
                    L::L400
                } else if r.h.mfh == 0 || r.h.mth == 0 {
                    L::L250
                } else if r.h.mfh == 3 {
                    L::L260
                } else if r.h.mfh == 5 {
                    L::L270
                } else if r.h.mfh == 6 {
                    L::L300
                } else {
                    r.tosend()?;
                    L::L250
                };
            }
            L::L260 => {
                if r.h.mth == 18 || r.h.mth == 19 {
                    r.iwf = 1;
                }
                r.tosend()?;
                lab = L::L250;
            }
            L::L270 => {
                if r.h.mth == 455 {
                    let nl = r.h.l1h;
                    r.nfs += nl;
                }
                r.tosend()?;
                lab = L::L250;
            }
            L::L300 => {
                let (mut nb, mut nw) = r.listio(1)?;
                let mut l = 1;
                while nb != 0 {
                    l += nw;
                    (nb, nw) = r.moreio(l, nb)?;
                }
                ig = r.h.n2h;
                igc = r.icgrp(ig);
                if igc == 0 || igc > ngnd {
                    lab = L::L350;
                    continue;
                }
                let mut ig2lo = r.h.l2h;
                loop {
                    ig2loc = r.icgrp(ig2lo);
                    if (ig2loc != 0 && ig2loc <= ngnd) || ig2lo == ig {
                        break;
                    }
                    ig2lo += 1;
                }
                if r.h.mth < 51 || r.h.mth > 91 {
                    lab = L::L360;
                    continue;
                }
                // inelastic
                if lain > igc {
                    lain = igc;
                }
                if ldin < igc - ig2loc {
                    ldin = igc - ig2loc;
                }
                lab = L::L350;
            }
            L::L350 => {
                lab = if ig < r.ngn { L::L300 } else { L::L250 };
            }
            L::L360 => {
                if r.h.mth != 2 {
                    lab = L::L370;
                    continue;
                }
                // elastic
                if lap > igc {
                    lap = igc;
                }
                if ldp < igc - ig2loc {
                    ldp = igc - ig2loc;
                }
                lab = if ig < r.ngn { L::L300 } else { L::L250 };
            }
            L::L370 => {
                let mth = r.h.mth;
                if mth == 16 || (mth > 5 && mth < 10) || (mth > 45 && mth < 50) {
                    lab = L::L390;
                    continue;
                }
                r.tosend()?;
                lab = L::L250;
            }
            L::L390 => {
                // n2n
                if la2n > igc {
                    la2n = igc;
                }
                if ld2n < igc - ig2loc {
                    ld2n = igc - ig2loc;
                }
                // upstream goes to 250, not 300: with more groups to come,
                // the next record's lines are read as CONT records (6e11.0)
                lab = L::L250;
            }
            L::L400 => break,
        }
    }
    // 400: set up storage pointers
    let _ = temp;
    r.xla = [0.0; 4];
    r.xlol = [0.0; 4];
    r.xld[1] = f64::from(ldin);
    r.xld[2] = f64::from(ld2n);
    r.xld[3] = f64::from(ldp);
    for (k, (ld, la)) in [(ldin, lain), (ld2n, la2n), (ldp, lap)].into_iter().enumerate() {
        let k = k + 1;
        if ld != 0 {
            r.xla[k] = f64::from(ngnd - la + 1);
            let mut lx = nint(r.xla[k]) + nint(r.xld[k]) - ngnd - 1;
            if lx < 0 {
                lx = 0;
            }
            r.xlol[k] = r.xla[k] * (r.xld[k] + 1.0) - f64::from((lx * (lx - 1)) / 2);
        }
    }
    Ok(())
}

/// `gamll`'s label 160 (`powr.f90:878-919`): the reference temperature's
/// MF=1 record. Checks the group structure and takes the sigma zeroes.
fn gamll_160(r: &mut Reader, igprnt: &mut i32, ntw: i32, lz: usize, temp: f64) -> Result<(), NjoyError> {
    if r.ngn != r.ngnd && r.ngn != r.ngnf {
        return Err(err("gamll", "incorrect group structure or group type."));
    }
    // the group structure goes to the listing only
    if *igprnt <= 0 {
        *igprnt = 1;
    }
    r.ntp = 0;
    if r.rtemp > 0.0 {
        r.ntp = 1;
        r.tmpr[1] = temp;
    }
    for i in 1..=r.nsigz.max(0) as usize {
        r.sigz[i] = r.sc(i + ntw as usize + lz);
    }
    Ok(())
}

/// The labels of `gamxs`.
#[derive(Clone, Copy)]
enum X {
    L140,
    L150,
    L170,
    L200,
    L210,
    L250,
    L260,
    L270,
    L280,
    L300,
    L320,
    L330,
    L370,
    L410,
    L450,
    L460,
    L500,
}

/// `gamxs(...)` (`powr.f90:1022-1399`): accumulate the cross sections and
/// matrices at the reference temperature into `a`.
fn gamxs(r: &mut Reader, a: &mut [f64], nloc: usize) -> Result<Locs, NjoyError> {
    let eps = 1.0e-4;
    let ngnd = r.ngnd as usize;
    let ngnd1 = ngnd + 1;
    let ldin = nint(r.xld[1]) as usize;
    let kdin = ngnd1 - ldin;
    let ld2n = nint(r.xld[2]) as usize;
    let kd2n = ngnd1 - ld2n;
    let ldp = nint(r.xld[3]) as usize;
    let kdp = ngnd1 - ldp;
    let lz = 6usize;
    let mut mf3mt2 = 0;
    let mut dnu = 0.0f64;
    let mut jgdnu = 0usize;
    let mut dnorm = 0.0f64;
    let mut cnorm = 0.0f64;
    let iloc = 1usize;
    let mut o = Locs { locab0: iloc, ..Locs::default() };
    o.locsf0 = o.locab0 + ngnd;
    o.locchi = o.locsf0 + ngnd;
    let locdla = o.locchi + ngnd;
    o.locnus = locdla + ngnd * r.nfs.max(0) as usize;
    o.loce0 = o.locnus + ngnd;
    o.loce1 = o.loce0 + nint(r.xlol[3]) as usize;
    o.locin = o.loce1 + nint(r.xlol[3]) as usize;
    o.loc2n = o.locin + nint(r.xlol[1]) as usize;
    o.locabs = o.loc2n + nint(r.xlol[2]) as usize;
    let na = o.locabs - 1;
    if na.saturating_sub(iloc) > nloc {
        return Err(err("gamxs", "storage exceeded."));
    }
    for v in a.iter_mut().take(na + 1).skip(iloc) {
        *v = 0.0;
    }
    for i in 1..=ngnd {
        r.cflux[i] = 0.0;
        r.cflux[i + ngnd] = 0.0;
    }
    let mut cspc = [0.0f64; 201];
    let mut i318 = 0;
    let mut i618 = 0;
    let mut sumd = [0.0f64; 7];
    let mut nldla: Option<usize> = None;
    r.repoz();
    r.tpidio()?;
    // 120
    loop {
        r.contio(1)?;
        if r.h.math == r.matd {
            break;
        }
        r.tomend()?;
    }
    // 130
    let mut tfound = 0;
    let mut lab = if r.h.mfh != 1 {
        X::L150
    } else {
        r.tosend()?;
        X::L140
    };
    let mut l = 1usize;
    let (mut nl, mut nz) = (0usize, 0usize);
    let (mut ng2, mut ig2lo, mut ig, mut jg) = (0usize, 0i32, 0i32, 0usize);
    let ngn = |r: &Reader| r.ngn;
    loop {
        match lab {
            X::L140 => {
                l = 1;
                r.contio(l)?;
                lab = if r.h.math == 0 {
                    X::L500
                } else if r.h.mfh == 0 || r.h.mth == 0 {
                    X::L140
                } else if r.h.mfh != 1 {
                    X::L150
                } else {
                    r.tosend()?;
                    X::L140
                };
            }
            X::L150 => {
                nl = r.h.l1h as usize;
                nz = r.h.l2h as usize;
                if !(r.h.mfh != 3 || r.h.mth < 2 || mf3mt2 > 0) {
                    mf3mt2 = 1;
                    for i in 1..=ngnd {
                        if r.cflux[i] != 0.0 {
                            r.cflux[i] = 1.0 / r.cflux[i];
                        }
                        if r.cflux[ngnd + i] != 0.0 {
                            r.cflux[ngnd + i] = 1.0 / r.cflux[ngnd + i];
                        }
                    }
                }
                lab = X::L170;
            }
            X::L170 => {
                let (mut nb, mut nw) = r.listio(l)?;
                let temp = r.h.c1h;
                if (temp - r.rtemp).abs() > eps && tfound == 0 {
                    lab = X::L210;
                    continue;
                }
                if (temp - r.rtemp).abs() <= eps {
                    tfound = 1;
                }
                ng2 = r.h.l1h.max(0) as usize;
                ig2lo = r.h.l2h;
                ig = r.h.n2h;
                let mut igc = 1;
                if ig > 0 {
                    igc = r.icgrp(ig);
                }
                while nb != 0 {
                    l += nw;
                    (nb, nw) = r.moreio(l, nb)?;
                    if l + nw - 1 > NWSCR {
                        return Err(err("gamxs", "storage exceeded."));
                    }
                }
                if igc == 0 || igc > ngnd as i32 {
                    lab = X::L260;
                    continue;
                }
                jg = ngnd - igc as usize + 1;
                l = 1;
                lab = if r.h.mth == 455 {
                    X::L330
                } else if r.h.mfh == 3 {
                    X::L250
                } else if r.h.mfh == 6 {
                    X::L300
                } else {
                    X::L200
                };
            }
            X::L200 => {
                r.tosend()?;
                lab = X::L140;
            }
            X::L210 => {
                r.tomend()?;
                lab = X::L140;
            }
            X::L250 => {
                if r.h.mth != 1 {
                    lab = X::L270;
                    continue;
                }
                // coarse group p0 flux
                r.cflux[jg] += r.sc(l + lz);
                if nl != 1 {
                    // coarse group p1 flux
                    r.cflux[ngnd + jg] += r.sc(l + lz + 1);
                }
                lab = X::L260;
            }
            X::L260 => {
                lab = if ig < ngn(r) { X::L170 } else { X::L140 };
            }
            X::L270 => {
                if r.h.mth < 102 || r.h.mth > 150 {
                    lab = X::L280;
                    continue;
                }
                // capture
                let loc = o.locab0 + jg - 1;
                let loca = l + lz + nl * nz;
                a[loc] += r.sc(loca) * r.sc(l + lz) * r.cflux[jg];
                lab = X::L260;
            }
            X::L280 => {
                let mth = r.h.mth;
                if (!(18..=21).contains(&mth)) && mth != 38 {
                    lab = X::L200;
                    continue;
                }
                // fission
                if mth == 18 {
                    i318 = 1;
                }
                if mth > 18 && i318 > 0 {
                    lab = X::L200;
                    continue;
                }
                let loc = o.locsf0 + jg - 1;
                let loca = l + lz + nl * nz;
                a[loc] += r.sc(loca) * r.sc(l + lz) * r.cflux[jg];
                let loc = o.locab0 + jg - 1;
                a[loc] += r.sc(loca) * r.sc(l + lz) * r.cflux[jg];
                lab = X::L260;
            }
            X::L300 => {
                let mth = r.h.mth;
                if (!(18..=21).contains(&mth)) && mth != 38 {
                    lab = X::L370;
                    continue;
                }
                // fission
                if mth == 18 {
                    i618 = 1;
                }
                if mth > 18 && i618 > 0 {
                    lab = X::L200;
                    continue;
                }
                if ig != 0 {
                    for k in 2..=ng2 {
                        let loca = l + lz + nl * nz * (k - 1);
                        let loc = o.locnus + jg - 1;
                        if ig2lo != 0 {
                            // matrix part
                            a[loc] += r.sc(loca) * r.sc(l + lz) * r.cflux[jg];
                            let locf = l + lz;
                            let ig2 = ig2lo + k as i32 - 2;
                            let ig2c = r.icgrp(ig2);
                            if ig2c != 0 && ig2c <= ngnd as i32 {
                                let jg2c = ngnd - ig2c as usize + 1;
                                let loc = o.locchi + jg2c - 1;
                                a[loc] += r.sc(locf) * r.sc(loca);
                                cnorm += r.sc(locf) * r.sc(loca);
                            }
                        } else {
                            // spectrum part
                            for i in 1..=ngn(r).max(0) as usize {
                                a[loc] += cspc[i] * r.sc(loca) * r.sc(loca - 1) * r.cflux[jg];
                                let ig2c = r.icgrp(i as i32);
                                if ig2c != 0 && ig2c <= ngnd as i32 {
                                    // upstream: jg2c=ngnd-ig2c-1
                                    let jg2c = ngnd as i64 - i64::from(ig2c) - 1;
                                    let locc = (o.locchi as i64 + jg2c - 1) as usize;
                                    a[locc] += cspc[i] * r.sc(loca) * r.sc(loca - 1);
                                    cnorm += r.sc(loca) * r.sc(loca - 1);
                                }
                            }
                        }
                    }
                } else {
                    // save constant spectrum
                    for k in 1..=ng2 {
                        let at = (ig2lo + k as i32 - 1) as usize;
                        cspc[at] = r.sc(l + lz + nl * nz * (k - 1));
                    }
                }
                lab = X::L320;
            }
            X::L320 => {
                lab = if ig < ngn(r) { X::L170 } else { X::L140 };
            }
            X::L330 => {
                // delayed neutrons
                if r.h.mfh != 5 {
                    let loc = o.locnus + jg - 1;
                    let loca = l + lz;
                    a[loc] += r.sc(loca + 1) * r.sc(loca + 2) * r.sc(l + lz) * r.cflux[jg];
                    dnorm += r.sc(loca) * r.sc(loca + 1) * r.sc(loca + 2);
                    let mut test = 1.0f64;
                    test /= 10.0;
                    if r.sc(loca + 2) >= test && dnu == 0.0 {
                        jgdnu = jg;
                    }
                    if r.sc(loca + 2) >= test && dnu == 0.0 {
                        dnu = r.sc(loca + 1);
                    }
                } else {
                    nldla = Some(nl);
                    if nl != 6 {
                        return Err(err("gamxs", "no. deleay groups not 6."));
                    }
                    for s in sumd.iter_mut().take(nl + 1).skip(1) {
                        *s = 0.0;
                    }
                    let locb = l + lz - 1;
                    for k in 2..=ng2 {
                        let ig2 = ig2lo + k as i32 - 2;
                        let ig2c = r.icgrp(ig2);
                        if ig2c != 0 && ig2c <= ngnd as i32 {
                            let jgc = ngnd - ig2c as usize + 1;
                            let loc = o.locchi + jgc - 1;
                            let loca = l + lz - 1 + nl * (k - 1);
                            let locd = locdla + jgc - 1;
                            for jgt in 1..=nl {
                                a[loc] = r.sc(jgt + loca) * dnorm + a[loc];
                                let p = ngnd * (jgt - 1) + locd;
                                a[p] += r.sc(jgt + loca) * dnu;
                                if jgc == ngnd {
                                    a[p] = a[jgt + locb];
                                }
                                if jgc + 1 < ngnd {
                                    sumd[jgt] += r.sc(jgt + loca) * dnu;
                                }
                                cnorm = r.sc(jgt + loca) * dnorm + cnorm;
                            }
                        }
                    }
                }
                lab = if ig < ngn(r) { X::L170 } else { X::L140 };
            }
            X::L370 => {
                if r.h.mth < 51 || r.h.mth > 91 {
                    lab = X::L410;
                    continue;
                }
                // inelastic
                matrix(r, a, &MatrixPart { ld: ldin, kd: kdin, base: o.locin, l, lz, nl, nz, ng2, ig2lo, jg, half: false })?;
                lab = if ig < ngn(r) { X::L170 } else { X::L140 };
            }
            X::L410 => {
                if r.h.mth != 2 {
                    lab = X::L450;
                    continue;
                }
                // elastic
                let mut jgp = jg;
                if jg > kdp {
                    jgp = kdp;
                }
                let mut lc = (ldp + 1) * (jgp - 1);
                if jg > kdp {
                    for jgt in 1..=jg - kdp {
                        lc = lc + ldp + 2 - jgt;
                    }
                }
                for k in 2..=ng2 {
                    let ig2 = ig2lo + k as i32 - 2;
                    let ig2c = r.icgrp(ig2);
                    if ig2c != 0 && ig2c <= ngnd as i32 {
                        let mut jg2c = ngnd - ig2c as usize + 1;
                        if jg2c < jg {
                            jg2c = jg;
                        }
                        let loc = o.loce0 + lc + jg2c - jg;
                        let off = i64::from(r.izref - 1) + (nz * (k - 1)) as i64;
                        let mut loca = (l as i64 + lz as i64 + nl as i64 * off) as usize;
                        a[loc] += r.sc(loca) * r.sc(l + lz) * r.cflux[jg];
                        if nl != 1 {
                            loca += 1;
                            let loc = o.loce1 + lc + jg2c - jg;
                            let mut flx = r.cflux[ngnd + jg];
                            if r.cflux[ngnd + jg] == 0.0 {
                                flx = r.cflux[jg];
                            }
                            a[loc] += 3.0 * r.sc(loca) * r.sc(l + lz + 1) * flx;
                        }
                    }
                }
                lab = if ig < ngn(r) { X::L170 } else { X::L140 };
            }
            X::L450 => {
                let mth = r.h.mth;
                lab = if mth == 16 {
                    X::L460
                } else if !(6..=49).contains(&mth) || (mth > 9 && mth < 46) {
                    X::L200
                } else {
                    X::L460
                };
            }
            X::L460 => {
                // n2n
                matrix(r, a, &MatrixPart { ld: ld2n, kd: kd2n, base: o.loc2n, l, lz, nl, nz, ng2, ig2lo, jg, half: true })?;
                lab = if ig < ngn(r) { X::L170 } else { X::L140 };
            }
            X::L500 => break,
        }
    }
    // 500: find nu from nu times sigma f
    for ig in 1..=ngnd {
        let loc1 = o.locsf0 + ig - 1;
        if a[loc1] != 0.0 {
            let loc2 = o.locnus + ig - 1;
            a[loc2] /= a[loc1];
        }
    }
    // normalize chi and delayed chi
    if cnorm != 0.0 {
        cnorm = 1.0 / cnorm;
        for ig in 1..=ngnd {
            a[ig - 1 + o.locchi] *= cnorm;
        }
        if dnu != 0.0 {
            let Some(nldla) = nldla else {
                return Err(NjoyError::NotPorted(
                    "POWR lib = 1: delayed nu (MF=3 MT=455) with no MF=5 MT=455 (gamxs's nldla undefined upstream)",
                ));
            };
            let rnorm = 1.0 / a[jgdnu - 1 + o.locnus];
            let lim = ngnd - 1;
            for ig in 1..=lim {
                let locd = locdla - 1 + ig;
                for il in 1..=nldla {
                    let p = ngnd * (il - 1) + locd;
                    if ig < lim {
                        a[p] *= rnorm;
                    } else {
                        a[p] = sumd[il] * rnorm;
                    }
                }
            }
        }
    }
    // for reference temperatures greater than zero...
    if r.rtemp > 0.0 {
        r.s.skiprz_back(2);
        r.s.findf(r.matd, 1, 451, 0)?;
    }
    Ok(o)
}

/// One transfer-matrix record of `gamxs` (labels 370 and 460).
struct MatrixPart {
    ld: usize,
    kd: usize,
    base: usize,
    l: usize,
    lz: usize,
    nl: usize,
    nz: usize,
    ng2: usize,
    ig2lo: i32,
    jg: usize,
    half: bool,
}

/// `gamxs` labels 370 (inelastic) and 460 (n2n, halved): add one group
/// record into the packed matrix at `base`.
fn matrix(r: &Reader, a: &mut [f64], m: &MatrixPart) -> Result<(), NjoyError> {
    let ngnd = r.ngnd as usize;
    let jg = m.jg;
    let mut jgp = jg;
    if jg > m.kd {
        jgp = m.kd;
    }
    let mut lc = (m.ld + 1) * (jgp - 1);
    if jg > m.kd {
        for jgt in 1..=jg - m.kd {
            lc = lc + m.ld + 2 - jgt;
        }
    }
    for k in 2..=m.ng2 {
        let ig2 = m.ig2lo + k as i32 - 2;
        let ig2c = r.icgrp(ig2);
        if ig2c != 0 && ig2c <= ngnd as i32 {
            let mut jg2c = ngnd - ig2c as usize + 1;
            if jg2c < jg {
                jg2c = jg;
            }
            let loca = m.l + m.lz + m.nl * m.nz * (k - 1);
            let loc = m.base + lc + jg2c - jg;
            if m.half {
                a[loc] += r.sc(loca) * r.sc(m.l + m.lz) * r.cflux[jg] / 2.0;
            } else {
                a[loc] += r.sc(loca) * r.sc(m.l + m.lz) * r.cflux[jg];
            }
        }
    }
    Ok(())
}
