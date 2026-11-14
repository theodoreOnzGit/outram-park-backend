// SPDX-License-Identifier: AGPL-3.0-only
// Translated from NJOY2016.80 `src/ccccr.f90` (git commit ac5adf5f33d893e42f2eed7fb286b0d51c7580da):
//   - `cisotx`, l.278-459; `ruinis`, l.461-506; `isxdat`, l.508-735;
//     `prinxs`, l.737-1036; `isomtx`, l.1038-1199; `shuffl`, l.1201-1269;
//     `wrtmtx` (its `ifopt = 1` branch), l.1271-1454.
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under AGPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! **ISOTXS** (gh:#1369): isotope-ordered multigroup cross sections and
//! scattering matrices, written word for word as `cisotx` writes them.
//!
//! Indices are upstream's, 1-based, into the equivalenced container; a routine
//! that takes an array argument (`prinxs(spec, b, a, ia, cspc, ...)`) is given
//! the base index of each argument. `a` is `real(4)`: an assignment from a
//! `real(8)` expression rounds to nearest ([`Core::set_a64`]), and an
//! expression of two `a` words alone is evaluated in single precision.
//!
//! Translated: `ifopt = 1` (one block per reaction), `nsblk = 1`, `ichix` of
//! -1, 0 or 1. Refused by name: `ifopt = 2`, `nsblk = ngroup` (asked for, or
//! forced by `isomtx` when the matrix does not fit), and a chi matrix
//! (`ichix > 1`). No upstream test deck runs them.

use crate::ccccr::core::{text6, Core, Items, Records, ISIZA4, MULT};
use crate::ccccr::state::{IsotxsCards, State};
use crate::errorr::gendf::{GendfKind, GendfRecord};
use crate::mixr::mix::sigfig;
use crate::NjoyError;

fn refuse(what: &'static str) -> NjoyError {
    NjoyError::NotPorted(what)
}

fn ga(c: &mut Core, base: i64, k: i64) -> f32 {
    c.a(base + k - 1)
}

fn sa64(c: &mut Core, base: i64, k: i64, v: f64) {
    c.set_a64(base + k - 1, v);
}

fn nint(x: f64) -> i64 {
    x.round() as i64
}

/// `ruinis` (`ccccr.f90:461-506`): the ISOTXS cards and the file control words.
fn ruinis(s: &mut State, c: &IsotxsCards) -> Result<(), NjoyError> {
    let [nsblk, maxup, maxdn, ichix] = c.control.map(i64::from);
    s.nsblk = nsblk;
    s.ichix = ichix;
    if s.ifopt != 1 {
        return Err(refuse("ccccr: ISOTXS with ifopt = 2 (blocking by Legendre order)"));
    }
    if nsblk != 1 {
        return Err(refuse("ccccr: ISOTXS with nsblok = ngroup"));
    }
    if ichix > 1 {
        return Err(refuse("ccccr: ISOTXS chi matrix (ichix > 1)"));
    }
    let ichist = 0;
    s.nscmax = 4 + 4 * s.maxord * (s.ifopt - 1);
    let mut nwds = s.ngps;
    if 2 * (nwds / 2) != nwds {
        nwds += 1;
    }
    s.isopec = s.next;
    s.next = s.isopec + nwds;
    if ichix > 0 {
        // `read(nsysi,*) (a(isopec+i-1),i=1,ngps)`: list-directed into `real(4)`.
        // Read here through `real(8)` and rounded, which can differ from a direct
        // single-precision conversion in a double-rounding case (flagged).
        for i in 1..=s.ngps {
            let v = c.spec.get((i - 1) as usize).copied().unwrap_or(0.0);
            s.core.set_a64(s.isopec + i - 1, v);
        }
    }
    nwds = 3 * MULT + 8 + MULT;
    s.l1 = s.next;
    s.next = s.l1 + nwds;
    nwds = 12 * MULT;
    s.l13 = s.next;
    s.next = s.l13 + nwds;
    let ndex = s.l1 + 3 * MULT;
    let vals = [s.ngps, s.niso, maxup, maxdn, s.maxord, ichist, s.nscmax, s.nsblk];
    for (k, v) in vals.iter().enumerate() {
        s.core.set_ia(ndex + 1 + k as i64, *v as i32);
    }
    let j = (s.l13 - 1) / MULT;
    for i in 1..=12 {
        let h = s.hsetid[(i - 1) as usize].clone();
        s.core.set_ta(i + j, &h);
    }
    for i in 1..=s.niso {
        let iso = c.isotopes.get((i - 1) as usize).ok_or_else(|| NjoyError::InvalidInput("ccccr: missing cisotx card 4".into()))?;
        s.kbr[i as usize] = i64::from(iso.kbr);
        s.isodata[i as usize] = iso.values;
    }
    Ok(())
}

/// `prinxs` (`ccccr.f90:737-1036`): the principal cross sections of one isotope,
/// read from `nscrt1`, written to `nscrt2`. Returns `ichid`.
#[allow(clippy::too_many_arguments)] // upstream's argument list, one base index per array
fn prinxs(s: &mut State, spec: i64, b: i64, ab: i64, cspc: i64, iiso: i64, nsz: i64) -> Result<i64, NjoyError> {
    const NSM: [i64; 15] = [0, 1, 102, 0, 0, 0, 107, 103, 16, 104, 105, 6, 7, 8, 9];
    const NSTX: [i64; 11] = [0, 1, 102, 0, 0, 0, 107, 103, 0, 104, 105];
    const NTN: [i64; 5] = [6, 7, 8, 9, 16];
    const EPSN: f64 = 1.0e-10;
    let ngps = s.ngps;
    s.ltot = 1;
    s.ltrn = 1;
    let mut cnorm = [0.0_f64; 21];
    let mut dnorm = [0.0_f64; 21];
    for i in 1..=ngps {
        s.core.set_a(cspc + i - 1, 0.0);
    }
    let mut mt19 = 0;
    let mut iptr = [0_i64; 13];
    for i in 1..=12 {
        iptr[i as usize] = (i - 1) * ngps + 1;
    }
    let mut nptr = iptr[12] + ngps - 1 + ngps * nsz * (s.maxord + 1);
    let mut mchi = 0;
    if s.ichix > 1 {
        mchi = nptr + 1;
        nptr += s.ichix * ngps;
    }
    for i in 1..=nptr {
        s.core.set_a(ab + i - 1, 0.0);
    }
    s.iflags = [0; 16];
    let mut ichid = 0;
    let nwd = nsz * (s.maxord + 1) * ngps;
    s.scr1.rewind();
    let mut ig = ngps;
    let mut flux = 0.0_f64;
    let (mut nl, mut nz) = (0_i64, 0_i64);
    let spec_at = |c: &mut Core, k: i64| f64::from(ga(c, spec, k));
    loop {
        // 115
        if ig >= ngps {
            s.contio_scr1()?;
            nl = i64::from(s.h.l1h);
            nz = i64::from(s.h.l2h);
            if s.h.math == 0 {
                break;
            }
            if s.h.mfh == 0 || s.h.mth == 0 {
                continue;
            }
            if s.h.mfh <= 1 {
                s.scr1.tofend(None)?;
                continue;
            }
        }
        // 120
        s.listio_scr1()?;
        let ng2 = i64::from(s.h.l1h);
        let ig2lo = i64::from(s.h.l2h);
        ig = i64::from(s.h.n2h);
        let lz = 6;
        let mth = i64::from(s.h.mth);
        let mfh = s.h.mfh;
        if mfh == 3 {
            if mth == 259 {
                if iiso != 1 {
                    continue;
                }
                let indx = lz + nl * nz + 1;
                let mptr = ngps - ig + 1;
                let v = sigfig(100.0 / s.e(indx), 7, 0);
                sa64(&mut s.core, b, mptr, v);
                continue;
            }
            // 125
            let mut go170 = false;
            if let Some(i) = NSM.iter().position(|&m| m == mth) {
                s.iflags[i + 1] = 1;
                // 140
                if NTN.contains(&mth) {
                    // 160
                    let mptr = iptr[9] - 1 + ngps - ig + 1 + nwd;
                    let indx = lz + nl * nz + 1;
                    let v = f64::from(ga(&mut s.core, ab, mptr)) + s.e(indx);
                    sa64(&mut s.core, ab, mptr, v);
                    s.iflags[9] = 1;
                    continue;
                }
                go170 = true;
            }
            if go170 {
                if let Some(i) = NSTX.iter().position(|&m| m == mth) {
                    let mptr = iptr[i + 1] - 1 + ngps - ig + 1 + nwd;
                    let indx = lz + nl * nz + 1;
                    let v = s.e(indx);
                    sa64(&mut s.core, ab, mptr, v);
                }
            }
            // 190
            if mth == 1 {
                let mptr = iptr[1] - 1 + ngps - ig + 1 + nwd;
                let v = s.e(lz + nl * (nz + 1));
                sa64(&mut s.core, ab, mptr, v);
                let mptr = iptr[12] - 1 + ngps - ig + 1 + nwd;
                let v = s.e(lz + nl);
                sa64(&mut s.core, ab, mptr, v);
            } else if matches!(mth, 18 | 19 | 20 | 21 | 38) {
                if mth == 19 {
                    mt19 = 1;
                }
                if mth == 18 {
                    s.iflags[4] = 1;
                    s.iflags[5] = 1;
                    if s.ichix != 0 {
                        ichid = s.ichix.abs();
                    }
                    s.iflags[6] = ichid;
                    let mptr = iptr[4] - 1 + ngps - ig + 1 + nwd;
                    let v = s.e(lz + nl * nz + 1);
                    sa64(&mut s.core, ab, mptr, v);
                }
            } else if mth == 455 {
                let mptr = iptr[5] - 1 + ngps - ig + 1 + nwd;
                let v = f64::from(ga(&mut s.core, ab, mptr)) + s.e(lz + 2) * s.e(lz + 3);
                sa64(&mut s.core, ab, mptr, v);
                let ispec;
                if ichid == 1 {
                    ispec = 1;
                    if s.ichix < 0 {
                        flux = s.e(lz + 1);
                    }
                    if s.ichix > 0 {
                        flux = spec_at(&mut s.core, ngps - ig + 1);
                    }
                } else {
                    ispec = nint(spec_at(&mut s.core, ngps - ig + 1));
                    flux = s.e(lz + 1);
                }
                if !(1..=20).contains(&ispec) {
                    return Err(NjoyError::InvalidInput(format!(
                        "ccccr: prinxs indexes dnorm({ispec}) (MT=455 before MT=18; upstream reads outside the array)"
                    )));
                }
                dnorm[ispec as usize] += flux * s.e(lz + 2) * s.e(lz + 3);
            }
        }
        // 205: mf=6
        if mfh == 6 {
            if !matches!(mth, 18 | 19 | 20 | 21 | 38) {
                for _j in 2..=ng2 {
                    if nl >= 2 {
                        let locsg = lz + nl * nz * (_j - 1) + 2;
                        let mptr = iptr[1] - 1 + ngps + 1 - ig + nwd;
                        let v = f64::from(ga(&mut s.core, ab, mptr)) - s.e(locsg);
                        sa64(&mut s.core, ab, mptr, v);
                    }
                }
            } else if !((mt19 == 0 && mth != 18) || (mt19 == 1 && mth == 18)) {
                if ig != 0 {
                    for j in 2..=ng2 {
                        let jg2 = ngps + 1 - ig2lo - j + 2;
                        let locsg = lz + nl * nz * (j - 1) + 1;
                        if ig2lo > 0 {
                            let mptr = iptr[5] - 1 + ngps - ig + 1 + nwd;
                            let v = f64::from(ga(&mut s.core, ab, mptr)) + s.e(locsg);
                            sa64(&mut s.core, ab, mptr, v);
                            // ichid == 1 only (ichix > 1 refused in `ruinis`)
                            let ispec = 1;
                            if s.ichix < 0 {
                                flux = s.e(lz + 1);
                            }
                            if s.ichix > 0 {
                                flux = spec_at(&mut s.core, ngps - ig + 1);
                            }
                            let mptr = iptr[6] - 1 + jg2 + nwd;
                            let v = f64::from(ga(&mut s.core, ab, mptr)) + s.e(locsg) * flux;
                            sa64(&mut s.core, ab, mptr, v);
                            cnorm[ispec] += s.e(locsg) * flux;
                        } else {
                            for i in 1..=ngps {
                                let sig = s.e(locsg) * f64::from(ga(&mut s.core, cspc, i));
                                let mptr = iptr[5] - 1 + ngps - ig + 1 + nwd;
                                let v = f64::from(ga(&mut s.core, ab, mptr)) + sig;
                                sa64(&mut s.core, ab, mptr, v);
                                let ispec = 1;
                                if s.ichix < 0 {
                                    flux = s.e(lz + 1);
                                }
                                if s.ichix > 0 {
                                    flux = spec_at(&mut s.core, ngps - ig + 1);
                                }
                                let mptr = iptr[6] - 1 + i + nwd;
                                let v = f64::from(ga(&mut s.core, ab, mptr)) + sig * flux;
                                sa64(&mut s.core, ab, mptr, v);
                                cnorm[ispec] += sig * flux;
                            }
                        }
                    }
                } else {
                    for j in 1..=ng2 {
                        let jg2 = ngps - ig2lo - j + 2;
                        let locsg = lz + nl * nz * (j - 1) + 1;
                        let v = s.e(locsg);
                        sa64(&mut s.core, cspc, jg2, v);
                    }
                }
            }
        }
        // 230: mf=5
        if mfh == 5 && mth == 455 {
            for ig2 in 2..=ng2 {
                let jg = ngps + 1 - ig2lo - ig2 + 2;
                for ispec in 1..=ichid {
                    let locch = iptr[6] - 1 + jg + nwd;
                    let loca = lz + nl * (ig2 - 1);
                    for id in 1..=nl {
                        let x = s.e(id + loca) * dnorm[ispec as usize];
                        let v = x + f64::from(ga(&mut s.core, ab, locch));
                        sa64(&mut s.core, ab, locch, v);
                        cnorm[ispec as usize] += x;
                    }
                }
            }
        }
    }
    // 240
    if s.iflags[4] != 0 {
        for j in 1..=ngps {
            let locn = iptr[5] - 1 + j + nwd;
            let locd = iptr[4] - 1 + j + nwd;
            let d = ga(&mut s.core, ab, locd);
            if f64::from(d) <= EPSN {
                s.core.set_a(ab + locn - 1, 0.0);
            } else {
                let n = ga(&mut s.core, ab, locn);
                s.core.set_a(ab + locn - 1, n / d);
            }
            for ispec in 1..=ichid {
                let locn = iptr[6] - 1 + j + nwd;
                if cnorm[ispec as usize] != 0.0 {
                    let v = f64::from(ga(&mut s.core, ab, locn)) / cnorm[ispec as usize];
                    sa64(&mut s.core, ab, locn, v);
                }
            }
        }
    }
    // collapse out zero reactions
    let mut irct = s.ltot + s.ltrn + 1;
    let mut ndex = irct * ngps + 1 + nwd;
    let mut mdex = ndex;
    for i in 4..=11 {
        if s.iflags[i] == 1 {
            let (jndx, jmdx) = (ndex - 1, mdex - 1);
            for j in 1..=ngps {
                let v = s.core.word(ab + j + jmdx - 1);
                s.core.set_word(ab + j + jndx - 1, v);
            }
            irct += 1;
            ndex += ngps;
        }
        mdex += ngps;
    }
    let nwds = irct * ngps;
    let it = Items::new().w(ab + nwd, nwds);
    s.scr2.put(&mut s.core, &it);
    let _ = mchi;
    Ok(ichid)
}

/// `shuffl` (`ccccr.f90:1201-1269`), `ifopt = 1`: one MT's matrix into `b`.
fn shuffl(s: &mut State, b: i64) -> Result<(), NjoyError> {
    let ngps = s.ngps;
    let nl = i64::from(s.h.l1h);
    let nz = i64::from(s.h.l2h);
    let mut ig = 0;
    while ig < ngps {
        s.listio_scr1()?;
        let ng2 = i64::from(s.h.l1h);
        let ig2lo = i64::from(s.h.l2h);
        ig = i64::from(s.h.n2h);
        if nl > s.lord1 {
            s.lord1 = nl.min(s.maxord + 1);
        }
        let jg1 = ngps + 1 - ig;
        for il in 1..=nl {
            if il <= s.maxord + 1 {
                for ik in 2..=ng2 {
                    let niloc = 6 + il + nz * nl * (ik - 1);
                    let jg2 = ngps + 1 - (ig2lo + ik - 2);
                    let noloc = s.irsize + (s.maxord + 1) * (jg2 * (jg2 - 1) - s.ng2z * (s.ng2z + 1)) / 2 + (il - 1) * jg2 + jg2 - jg1 + 1;
                    if noloc > s.irsize && noloc <= s.nsiza {
                        let v = f64::from(ga(&mut s.core, b, noloc)) + s.e(niloc);
                        sa64(&mut s.core, b, noloc, v);
                    }
                }
            }
        }
    }
    Ok(())
}

/// `wrtmtx` (`ccccr.f90:1271-1454`), `ifopt = 1, nsblk = 1`: bandwidths, thinned
/// rows, the record on `nscrt2`.
fn wrtmtx(s: &mut State, ia: i64, b: i64, i: i64, nrec: i64) {
    const EPS: f64 = 1.0e-10;
    let ngps = s.ngps;
    let nscmax = s.nscmax;
    let nzero = s.irsize;
    for nrc in 1..=nrec {
        let nlow = nzero + 1;
        let inc = (ngps - 1) / s.nsblk + 1;
        for jg in 1..=inc {
            let jg2 = s.ng2z + (nrc - 1) + jg;
            let mut ibw = 0;
            for nrd in 1..=jg2 {
                let noloca = nlow + (nrd - 1) + ((s.maxord + 1) * jg * (jg - 1)) / 2;
                if f64::from(ga(&mut s.core, b, noloca)).abs() >= EPS {
                    ibw = nrd;
                }
            }
            let jbloc = 2 * nscmax + jg2 + (i - 1) * ngps;
            s.core.set_ia(ia + jbloc - 1, ibw as i32);
        }
        let lord = s.maxord + 1;
        let mut ndex = nlow - 1;
        let mut mdex = ndex;
        let mut lout = 0;
        for jg in 1..=inc {
            let jj = s.ng2z + (nrc - 1) + jg;
            let loc = 2 * nscmax + (i - 1) * ngps + jj;
            let ibw = i64::from(s.core.ia(ia + loc - 1));
            if ibw > 0 {
                for jm in 1..=lord {
                    for jk in 1..=ibw {
                        lout = ndex + (jm - 1) * ibw + jk;
                        let lin = mdex + (jm - 1) * jj + jk;
                        let v = s.core.word(b + lin - 1);
                        s.core.set_word(b + lout - 1, v);
                    }
                }
                ndex = lout;
            }
            mdex += jj * lord;
        }
        let nwds = ndex - nlow + 1;
        if nwds > 0 {
            let it = Items::new().w(b + nlow - 1, nwds);
            s.scr2.put(&mut s.core, &it);
        }
    }
    s.ng2z += nrec;
    s.jlz = 0;
}

/// `isomtx` (`ccccr.f90:1038-1199`): the four scattering blocks (inelastic,
/// elastic, (n,2n), total) of one isotope. Returns `loci`.
fn isomtx(s: &mut State, l21: i64, l22: i64, nsz: i64) -> Result<i64, NjoyError> {
    let ngps = s.ngps;
    let (ia, b) = (l21, l22);
    s.nsiza = ISIZA4 - l22 + 1;
    s.irsize = 6 + (ngps + 1) * (s.maxord + 1) * nsz;
    if s.irsize < 6 * ngps {
        s.irsize = 6 * ngps;
    }
    if s.irsize > s.nsiza {
        return Err(NjoyError::InvalidInput("ccccr: isomtx input record too large".into()));
    }
    let nosiza = s.nsiza - s.irsize;
    let lrsize = ((s.maxord + 1) * ngps * (ngps + 1)) / 2;
    if nosiza < lrsize {
        return Err(refuse("ccccr: ISOTXS matrix larger than the container (isomtx sets nsblk = ngroup)"));
    }
    let nrmx = 1;
    let mut nrec = nosiza / lrsize;
    if nrec >= nrmx {
        nrec = nrmx;
    }
    let npass = (nrmx + nrec - 1) / nrec;
    for j in 1..=(l22 - l21) {
        s.core.set_ia(ia + j - 1, 0);
    }
    for i in 1..=4_i64 {
        s.lord1 = 0;
        s.ng2z = 0;
        s.jlz = 0;
        'passes: for _nj in 1..=npass {
            for j in 1..=s.nsiza {
                s.core.set_a(b + j - 1, 0.0);
            }
            s.scr1.rewind();
            loop {
                s.contio_scr1()?;
                if s.h.math == 0 {
                    break;
                }
                if s.h.mfh == 0 || s.h.mth == 0 {
                    continue;
                }
                if s.h.mfh != 6 {
                    s.scr1.tosend(None)?;
                    continue;
                }
                if s.ng2z >= ngps || s.jlz > s.maxord {
                    break 'passes;
                }
                let mth = s.h.mth;
                let fission = matches!(mth, 18 | 19 | 20 | 21 | 38);
                match i {
                    1 => {
                        if mth == 2 || mth == 16 || (6..=9).contains(&mth) || (46..=49).contains(&mth) || fission {
                            continue;
                        }
                    }
                    2 => {
                        if mth > 2 {
                            break;
                        }
                        s.lord1 = 1;
                    }
                    3 => {
                        if mth > 49 {
                            break;
                        }
                        if mth < 6 || (mth > 9 && mth < 46 && mth != 16) {
                            continue;
                        }
                    }
                    _ => {
                        if fission {
                            continue;
                        }
                    }
                }
                if s.lord1 < 1 {
                    s.lord1 = 1;
                }
                // 270
                shuffl(s, b)?;
            }
            // 300
            wrtmtx(s, ia, b, i, nrec);
        }
        // 405 (ifopt = 1)
        let l1 = s.lord1 as i32;
        s.core.set_ia(ia + 4 + i - 1, l1);
    }
    let mut loci = 0;
    for l in 1..=s.nscmax {
        let mn = s.nscmax + l;
        if s.core.ia(ia + mn - 1) != 0 {
            loci += 1;
        }
    }
    Ok(loci)
}

/// `isxdat` (`ccccr.f90:508-735`): every isotope's data onto `nscrt2`/`nscrt3`.
fn isxdat(s: &mut State) -> Result<(), NjoyError> {
    let niso = s.niso;
    let nwds = MULT * niso;
    let l14 = s.next;
    s.next = l14 + nwds;
    let l16 = s.next;
    s.next = l16 + s.ngps;
    let l17 = s.next;
    s.next = l17 + s.ngps;
    let l18 = s.next;
    s.next = l18 + 1;
    let mut nwds = niso + 1;
    if ((l18 + nwds) / MULT) * MULT == l18 + niso {
        nwds = niso;
    }
    let l19 = s.next;
    s.next = l19 + nwds;
    let l20 = s.next;
    s.next = l20 + 3 * MULT + 17;
    let l21 = s.next;
    s.next = l21 + 2 * s.nscmax * (1 + s.ngps);
    let l21a = s.next;
    s.next = l21a + s.ngps;
    let l22 = s.next;

    // header of the first material
    s.nin.rewind();
    s.nin.tpidio(&mut s.h)?;
    loop {
        s.contio_nin()?;
        if !(s.h.mfh == 0 || s.h.mth == 0) {
            break;
        }
    }
    let nz = i64::from(s.h.l2h);
    let ntw = i64::from(s.h.n2h);
    s.listio_nin()?;
    s.ngps = i64::from(s.h.l1h);
    let loc = s.l1 + 1 + 3 * MULT;
    if s.ngps != i64::from(s.core.ia(loc)) {
        return Err(NjoyError::InvalidInput("ccccr: isxdat incompatible group structures".into()));
    }
    let iqd = 1 + 6 + ntw + nz + s.ngps;
    for i in 1..=s.ngps + 1 {
        let v = s.e(-i + 1 + iqd);
        s.core.set_a64(i - 1 + l17, v);
    }

    let mut jiso = 0;
    let mut loca = 0;
    s.core.set_ia(l19, 0);
    let mut imusd = [0_i64; 201];
    s.nin.rewind();
    s.scr2.rewind();
    s.scr3.rewind();
    let ngps = s.ngps;
    'isotopes: for i in 1..=niso {
        s.nin.tpidio(&mut s.h)?;
        let iin;
        loop {
            s.contio_nin()?;
            if s.h.math == -1 {
                s.notes.push("ccccr: input tape does not contain all specified isotopes".into());
                break 'isotopes;
            }
            if s.h.math == 0 || s.h.mfh == 0 || s.h.mth == 0 {
                continue;
            }
            if s.h.mfh <= 1 {
                s.nin.tosend(None)?;
                continue;
            }
            let math = i64::from(s.h.math);
            if let Some(j) = (1..=niso).find(|&j| math == s.imat[j as usize] && imusd[j as usize] != 1) {
                jiso += 1;
                iin = j;
                imusd[j as usize] = 1;
                let jj = (l14 - 1) / MULT + jiso;
                let h = s.hisonm[j as usize].clone();
                s.core.set_ta(jj, &h);
                break;
            }
            s.nin.tomend(None)?;
        }
        // 160: this material onto nscrt1
        let nsz = i64::from(s.h.l2h);
        s.scr1.rewind();
        let head = GendfRecord {
            kind: GendfKind::Cont,
            mat: s.h.math,
            mf: s.h.mfh,
            mt: s.h.mth,
            c: [s.e[0], s.e[1]],
            n: [s.h.l1h, s.h.l2h, s.h.n1h, s.h.n2h],
            data: Vec::new(),
        };
        s.scr1.write(head);
        s.nin.tomend(Some(&mut s.scr1))?;
        s.scr1.atend();
        s.scr1.rewind();

        let ichid = prinxs(s, s.isopec, l16, l22, l21a, i, nsz)?;

        let jj = (l20 - 1) / MULT + 1;
        let names = [s.habsid[iin as usize].clone(), s.hident[iin as usize].clone(), s.hmat[iin as usize].clone()];
        for (k, h) in names.iter().enumerate() {
            s.core.set_ta(jj + k as i64, &text6(h));
        }
        let l20m = l20 + 3 * MULT - 1;
        let d = s.isodata[iin as usize];
        for (k, v) in d.iter().enumerate() {
            s.core.set_a64(l20m + 1 + k as i64, *v);
        }
        let f = s.iflags;
        let ints = [s.kbr[iin as usize], ichid, f[4], f[7], f[8], f[9], f[10], f[11], s.ltrn, s.ltot, 0];
        for (k, v) in ints.iter().enumerate() {
            s.core.set_ia(l20m + 7 + k as i64, *v as i32);
        }

        let loci = isomtx(s, l21, l22, nsz)?;
        let nsblk = s.nsblk as i32;
        s.core.set_ia(s.l1 + 3 * MULT + 8, nsblk);
        let nrct = [200, 100, 300, 0];
        for (k, v) in nrct.iter().enumerate() {
            s.core.set_ia(l21 + k as i64, *v);
        }
        let ijz = l21 - 1 + s.nscmax * (2 + ngps);
        for k in 1..=ngps {
            let lk = ijz + (k - 1) * s.nscmax;
            for l in 1..=s.nscmax {
                s.core.set_ia(l + lk, 1);
            }
        }
        if i != niso {
            loca += loci + 2;
            s.core.set_ia(l19 + i, loca as i32);
        }

        let nwh = 3;
        let nwr = 6;
        let nwi = 11 + 2 * s.nscmax + 2 * ngps * s.nscmax;
        let llh = (l20 - 1) / MULT;
        let llr = l20 + MULT * nwh - 1;
        let lli = llr + nwr;
        let it = Items::new().ha(llh + 1, nwh).w(llr + 1, nwr).w(lli + 1, nwi);
        s.scr3.put(&mut s.core, &it);
    }
    s.core.set_ia(s.l1 + 3 * MULT + 2, jiso as i32);
    let isol = niso - jiso;
    for i in l16..=l20 {
        let v = s.core.word(i);
        s.core.set_word(i - isol, v);
    }
    Ok(())
}

/// `cisotx` (`ccccr.f90:278-459`): the ISOTXS file.
///
/// # Errors
/// The refusals of [`ruinis`] and [`isomtx`]; a GENDF tape that ends early; an
/// index outside the container.
pub fn cisotx(s: &mut State, cards: &IsotxsCards) -> Result<Vec<u8>, NjoyError> {
    let mut n4 = Records::default();
    ruinis(s, cards)?;
    isxdat(s)?;

    let nwds = 3 * MULT + 1;
    let l1 = s.l1;
    let l1h = (l1 - 1) / MULT + 1;
    s.core.set_ta(l1h, "isotxs");
    let [h1, h2] = s.huse;
    s.core.set_ha(l1h + 1, h1);
    s.core.set_ha(l1h + 2, h2);
    let iv = s.ivers as i32;
    s.core.set_ia(3 * MULT + l1, iv);
    n4.put(&mut s.core, &Items::new().ha(l1h, 3).w(l1 + 3 * MULT, 1));

    let l2 = l1 + nwds;
    s.l2 = l2;
    n4.put(&mut s.core, &Items::new().w(l2, 8));
    let ngroup = i64::from(s.core.ia(l2));
    s.niso = i64::from(s.core.ia(l2 + 1));
    let ichist = i64::from(s.core.ia(l2 + 5));
    s.nscmax = i64::from(s.core.ia(l2 + 6));
    let nsblok = i64::from(s.core.ia(l2 + 7));
    let niso = s.niso;
    let nscmax = s.nscmax;

    let l13h = (s.l13 - 1) / MULT;
    let nwh = 12 + niso;
    let l13r = s.l13 - 1 + nwh * MULT;
    let nwr = ngroup * (2 + ichist * (2 / (ichist + 1))) + 1;
    let l13i = l13r + nwr;
    n4.put(&mut s.core, &Items::new().ha(l13h + 1, nwh).w(l13r + 1, nwr).w(l13i + 1, niso));

    s.scr3.rewind();
    s.scr2.rewind();
    let jlord = l1;
    let jband = jlord + 2 * nscmax;
    let jtemp = jband + nscmax * ngroup;
    for _iso in 1..=niso {
        let l1h = (l1 - 1) / MULT + 1;
        let nwh = 3;
        let l1r = l1 + MULT * nwh;
        let nwr = 6;
        let l1i = l1r + nwr;
        let nwi = 11 + 2 * nscmax + 2 * ngroup * nscmax;
        let it = Items::new().ha(l1h, nwh).w(l1r, nwr).w(l1i, nwi);
        s.scr3.get(&mut s.core, &it)?;
        n4.put(&mut s.core, &it);
        let lnext = l1 + 3 * MULT;
        let g = |c: &mut Core, k: i64| i64::from(c.ia(lnext + k));
        let ichi = g(&mut s.core, 7);
        let ifis = g(&mut s.core, 8);
        let ialf = g(&mut s.core, 9);
        let inp = g(&mut s.core, 10);
        let in2n = g(&mut s.core, 11);
        let ind = g(&mut s.core, 12);
        let int = g(&mut s.core, 13);
        let ltot = g(&mut s.core, 14);
        let ltrn = g(&mut s.core, 15);
        let istrpd = g(&mut s.core, 16);
        s.core.stow(lnext + 17, jlord, 2 * nscmax);
        s.core.stow(lnext + 17 + 2 * nscmax, jband, nscmax * ngroup);

        let nwr = (1 + ltrn + ltot + ialf + inp + in2n + ind + int + istrpd + 2 * ifis + ichi * (2 / (ichi + 1))) * ngroup;
        let it = Items::new().w(jtemp, nwr);
        s.scr2.get(&mut s.core, &it)?;
        n4.put(&mut s.core, &it);
        if ichi > 1 {
            return Err(refuse("ccccr: ISOTXS isotope chi matrix"));
        }

        let inc = (ngroup - 1) / nsblok + 1;
        for n in 1..=nscmax {
            let lordn = i64::from(s.core.ia(jlord + nscmax + n - 1));
            if lordn == 0 {
                continue;
            }
            let lord = s.maxord + 1;
            let k = jlord - 1 + 2 * nscmax + (n - 1) * ngroup;
            for ji in 1..=nsblok {
                let jl = (ji - 1) * inc + 1;
                let ju = ji * inc;
                let mut nwds = 0;
                for jj in jl..=ju {
                    nwds += i64::from(s.core.ia(k + jj));
                }
                nwds *= lord;
                if nwds <= 0 {
                    continue;
                }
                s.scr2.get(&mut s.core, &Items::new().w(jtemp, nwds))?;
                let mut ndex = jtemp - 1;
                let mut mdex = jtemp - 1;
                for j in jl..=ju {
                    let n1 = jlord - 1 + 2 * nscmax + (n - 1) * ngroup + j;
                    let ibw = i64::from(s.core.ia(n1));
                    if ibw != 0 {
                        let mut isnk = ndex;
                        for jm in 1..=lordn {
                            isnk = ndex + (jm - 1) * ibw;
                            let isrce = mdex + (jm - 1) * ibw;
                            for jk in 1..=ibw {
                                let v = s.core.word(jk + isrce);
                                s.core.set_word(jk + isnk, v);
                            }
                            isnk += ibw;
                        }
                        ndex = isnk;
                    }
                    mdex += ibw * lord;
                }
                let nwds = ndex - jtemp + 1;
                if nwds > 0 {
                    // n2niv = 1: the (n,2n) matrix is not halved
                    n4.put(&mut s.core, &Items::new().w(jtemp, nwds));
                }
            }
        }
    }
    s.next = s.isopec;
    s.core.check("cisotx")?;
    Ok(n4.bytes)
}
