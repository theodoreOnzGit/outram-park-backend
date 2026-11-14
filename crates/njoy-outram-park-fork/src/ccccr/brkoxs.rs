// SPDX-License-Identifier: AGPL-3.0-only
// Translated from NJOY2016.80 `src/ccccr.f90` (git commit ac5adf5f33d893e42f2eed7fb286b0d51c7580da):
//   - `cbrkxs`, l.1873-1972; `ruinbr`, l.1974-1987; `brkdat`, l.1989-2419;
//     `xsproc`, l.2421-2566; `mxproc`, l.2568-2711; `thnwrt`, l.2713-2770.
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under AGPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! **BRKOXS** (gh:#1369): Bondarenko self-shielding factors by temperature and
//! sigma-zero, and the infinite-dilution cross sections, as `cbrkxs` writes them.
//!
//! Same conventions as [`crate::ccccr::isotxs`]. Single-precision operations
//! kept as such: `a(locsg)/a(locsg0)`, `abs(a(noloc)-1.0)` and
//! `a(mdex)-273.16` (a default-real literal). Refused by name: a file whose
//! factors do not fit the container in one block (`nsblk = nreact`).

use crate::ccccr::core::{Core, Items, Records, ISIZA4, MULT};
use crate::ccccr::state::{BrkoxsCards, State};
use crate::NjoyError;

/// `sstol` (`ccccr.f90:68`).
const SSTOL: f64 = 0.00001;

fn ga(c: &mut Core, base: i64, k: i64) -> f32 {
    c.a(base + k - 1)
}

fn sa64(c: &mut Core, base: i64, k: i64, v: f64) {
    c.set_a64(base + k - 1, v);
}

/// The first `j` in `1..=ntj` with `|temp - ctem(j)| <= eps*temp`.
fn find_temp(s: &State, temp: f64, eps: f64) -> Option<i64> {
    (1..=s.ntj).find(|&j| (temp - s.ctem[j as usize]).abs() <= eps * temp)
}

/// `xsproc` (`ccccr.f90:2421-2566`): one MF=3 reaction at every temperature.
fn xsproc(s: &mut State, l18: i64, l19: i64, l20: i64) -> Result<(), NjoyError> {
    const EPS: f64 = 1.0e-06;
    let ab = l18;
    let ngps = s.ngps;
    let inv = s.nreact / s.nsblk;
    let nl = i64::from(s.h.l1h);
    let nz = i64::from(s.h.l2h);
    let lz = 6;
    let nwon = l19 - l18 + 1;
    let (nzj, ntj) = (s.nzj, s.ntj);
    loop {
        // 120
        s.listio_scr1()?;
        let temp = s.h.c1h;
        let abtm = (temp - s.tzro).abs() - 0.01;
        let mut jmt = 0;
        let ig = i64::from(s.h.n2h);
        let mth = s.h.mth;
        if mth == 1 {
            let noloc = 13 * ngps + nzj * (ngps - ig) + s.irsize;
            for i in 1..=nzj {
                let isig2 = (s.isig[i as usize] * s.isig[i as usize]) as i64;
                let v = s.e(lz + isig2);
                sa64(&mut s.core, ab, i + noloc, v);
            }
        }
        let shelf = s.nsb == 1 && abtm <= EPS * temp;
        let mut to200 = true;
        match mth {
            1 => {
                jmt = 1;
                if shelf {
                    let v = s.e(lz + nl * nz + 1);
                    sa64(&mut s.core, ab, nwon + ngps - ig, v);
                    let v = s.e(lz + nl * nz + nl);
                    sa64(&mut s.core, ab, nwon + 4 * ngps - ig, v);
                    sa64(&mut s.core, ab, nwon + 6 * ngps - ig, v);
                    let v = s.e(lz + nl);
                    sa64(&mut s.core, ab, nwon + 7 * ngps - ig, v);
                }
            }
            102 => {
                jmt = 2;
                if shelf {
                    let v = s.e(lz + nl * nz + 1);
                    sa64(&mut s.core, ab, nwon + 2 * ngps - ig, v);
                }
            }
            18 => {
                jmt = 3;
                if shelf {
                    let v = s.e(lz + nl * nz + 1);
                    sa64(&mut s.core, ab, nwon + 3 * ngps - ig, v);
                }
            }
            2 => {
                jmt = 5;
                if shelf {
                    let v = s.e(lz + nl * nz + 1);
                    sa64(&mut s.core, ab, nwon + 5 * ngps - ig, v);
                    let x = s.xspo;
                    sa64(&mut s.core, ab, nwon + 8 * ngps - ig, x);
                }
            }
            _ => {
                if mth == 251 && shelf {
                    let v = s.e(lz + nl * nz + 1);
                    sa64(&mut s.core, ab, nwon + 11 * ngps - ig, v);
                }
                if mth == 252 && shelf {
                    let v = s.e(lz + nl * nz + 1);
                    sa64(&mut s.core, ab, nwon + 13 * ngps - ig, v);
                }
                // 195
                if ig < ngps {
                    continue;
                }
                to200 = true;
            }
        }
        if to200 {
            // 200
            let none = l20 - l18 + 1;
            if let Some(itm) = find_temp(s, temp, EPS) {
                let jg = ngps + 1 - ig;
                for j in 1..=inv {
                    let imt = j + s.nsb - 1;
                    let (idum, sg0);
                    if imt == jmt {
                        idum = if mth == 1 && nl == 2 { 2 } else { 1 };
                        let locsg0 = (l19 - l18) + (imt - 1) * ngps + jg;
                        sg0 = f64::from(ga(&mut s.core, ab, locsg0));
                    } else if imt == 4 && mth == 1 {
                        idum = nl;
                        sg0 = 1.0;
                    } else {
                        continue;
                    }
                    for jz in 1..=nzj {
                        let locsg = (lz as f64 + (nl * nz) as f64 + nl as f64 * (s.isig[jz as usize] - 1.0) + idum as f64) as i64;
                        if s.e(locsg) != 0.0 {
                            let locss = (none - 1) + (imt - s.nsb) * ntj * nzj * ngps + (jg - 1) * nzj * ntj + (itm - 1) * nzj + jz;
                            if sg0 == 0.0 {
                                s.notes.push(format!("xsproc: infinite f-factor {mth} {jg} {jz} {temp:.3e}"));
                            } else {
                                let v = f64::from(ga(&mut s.core, ab, locss)) + s.e(locsg) / sg0;
                                sa64(&mut s.core, ab, locss, v);
                            }
                        }
                    }
                }
            }
        }
        // 395
        if ig >= ngps {
            return Ok(());
        }
    }
}

/// `mxproc` (`ccccr.f90:2568-2711`): one MF=6 reaction at every temperature.
fn mxproc(s: &mut State, l18: i64, l20: i64) -> Result<(), NjoyError> {
    const EPS: f64 = 1.0e-6;
    let ab = l18;
    let ngps = s.ngps;
    let nl = i64::from(s.h.l1h);
    let nz = i64::from(s.h.l2h);
    let lz = 6;
    let (nzj, ntj, irsize) = (s.nzj, s.ntj, s.irsize);
    loop {
        s.listio_scr1()?;
        let temp = s.h.c1h;
        let abtm = (temp - s.tzro).abs() - 0.01;
        let ng2 = i64::from(s.h.l1h);
        let ig2lo = i64::from(s.h.l2h);
        let ig = i64::from(s.h.n2h);
        let jg1 = ngps + 1 - ig;
        let mth = i64::from(s.h.mth);
        let skip = mth > 91 || (18..=21).contains(&mth) || mth == 38;
        if !skip {
            if abtm <= EPS * temp && s.nsb <= 1 {
                if mth == 2 {
                    for j in 2..=ng2 {
                        let niloc = lz + 1 + nl * nz * (j - 1);
                        let jg2 = (ngps + 1) - (ig2lo + j - 2);
                        let noloc = irsize + 9 * ngps + jg1;
                        let v = f64::from(ga(&mut s.core, ab, noloc)) + s.e(niloc);
                        sa64(&mut s.core, ab, noloc, v);
                        if jg2 != jg1 {
                            let noloc = irsize + 11 * ngps + jg1;
                            let v = f64::from(ga(&mut s.core, ab, noloc)) + s.e(niloc);
                            sa64(&mut s.core, ab, noloc, v);
                        }
                    }
                }
                // 160
                let not_inel = mth == 2 || mth == 16 || (6..=9).contains(&mth) || (46..=49).contains(&mth);
                if !not_inel {
                    for j in 2..=ng2 {
                        let niloc = lz + 1 + nl * nz * (j - 1);
                        let noloc = irsize + 8 * ngps + jg1;
                        let v = f64::from(ga(&mut s.core, ab, noloc)) + s.e(niloc);
                        sa64(&mut s.core, ab, noloc, v);
                    }
                }
                // 180
                if nl >= 2 {
                    for j in 2..=ng2 {
                        let niloc = lz + 1 + nl * nz * (j - 1) + 1;
                        let noloc = irsize + 3 * ngps + jg1;
                        let v = f64::from(ga(&mut s.core, ab, noloc)) - s.e(niloc);
                        sa64(&mut s.core, ab, noloc, v);
                    }
                }
            }
            // 200: transport shielding factor
            if nl >= 2 {
                let inc = if s.nsblk > 1 { 1 } else { 4 };
                if inc + s.nsb == 5 {
                    if abtm > EPS * temp || (nz > 1 && mth == 2) {
                        // 250: finite dilution for elastic scattering
                        if !(mth != 2 || s.ntfl == 1 || nz == 1) {
                            if let Some(itemp) = find_temp(s, temp, EPS) {
                                s.mt2tem[itemp as usize] = 1;
                                for j in 1..=nzj {
                                    let noloc = l20 - l18 + (inc - 1) * ngps * nzj * ntj + (jg1 - 1) * nzj * ntj + (itemp - 1) * nzj + j;
                                    let locsg = (lz as f64 - (nl * nz) as f64 + nl as f64 * (s.isig[j as usize] - 1.0) + 2.0) as i64;
                                    for k in 2..=ng2 {
                                        let v = f64::from(ga(&mut s.core, ab, noloc)) - s.e(nl * nz * k + locsg);
                                        sa64(&mut s.core, ab, noloc, v);
                                    }
                                }
                            }
                        }
                    } else {
                        if mth == 2 {
                            s.ntfl = 1;
                        }
                        for j in 1..=nzj {
                            let noloc = l20 - l18 + (inc - 1) * ngps * nzj * ntj + (jg1 - 1) * nzj * ntj - nzj + j;
                            for k in 2..=ng2 {
                                let locsg = lz + nl * nz * (k - 1) + 2;
                                for l in 1..=ntj {
                                    let v = f64::from(ga(&mut s.core, ab, nzj * l + noloc)) - s.e(locsg);
                                    sa64(&mut s.core, ab, nzj * l + noloc, v);
                                }
                            }
                        }
                    }
                }
            }
            // 300: elastic removal shielding factor
            if mth == 2 {
                let inc = if s.nsblk > 1 { 1 } else { 6 };
                if inc + s.nsb == 7 {
                    if let Some(itemp) = find_temp(s, temp, EPS) {
                        for j in 1..=nzj {
                            for k in 2..=ng2 {
                                let jg2 = (ngps + 1) - (ig2lo + k - 2);
                                if jg2 != jg1 {
                                    let locsg = (lz as f64 + (nl * nz * (k - 1)) as f64 + (s.isig[j as usize] - 1.0) * nl as f64 + 1.0) as i64;
                                    let noloc = l20 - l18 + (inc - 1) * ngps * nzj * ntj + (jg1 - 1) * nzj * ntj + (itemp - 1) * nzj + j;
                                    let lrem = irsize + 11 * ngps + jg1;
                                    let r = f64::from(ga(&mut s.core, ab, lrem));
                                    let v = f64::from(ga(&mut s.core, ab, noloc)) + s.e(locsg) / r;
                                    sa64(&mut s.core, ab, noloc, v);
                                }
                            }
                        }
                    }
                }
            }
        }
        // 140
        if ig >= ngps {
            return Ok(());
        }
    }
}

/// `thnwrt` (`ccccr.f90:2713-2770`): thin the factors to the groups, temperatures
/// and sigma-zeros that differ from 1, onto `nscrt3`.
fn thnwrt(s: &mut State, ab: i64) -> Result<(), NjoyError> {
    let ngps = s.ngps;
    let (ntj, nzj) = (s.ntj, s.nzj);
    s.scr2.rewind();
    let nrnge = 6 * ngps;
    s.scr2.get(&mut s.core, &Items::new().w(ab, nrnge))?;
    for i in 1..=s.nsblk {
        let nlow = nrnge + ngps * ntj * nzj * (i - 1) + 1;
        let lr = s.lrsize;
        s.scr2.get(&mut s.core, &Items::new().w(ab + nlow - 1, lr))?;
        let mut ndex = nrnge;
        let inv = s.nreact / s.nsblk;
        for ir in 1..=inv {
            for jg in 1..=ngps {
                if jg < s.jbl || jg > s.jbh {
                    continue;
                }
                for jt in 1..=ntj {
                    if s.ntat[jt as usize] == 0.0 {
                        continue;
                    }
                    for jz in 1..=nzj {
                        if s.ntap[jz as usize] == 0.0 {
                            continue;
                        }
                        let mdex = (ir - 1) * ngps * ntj * nzj + (jg - 1) * ntj * nzj + (jt - 1) * nzj + jz + nrnge;
                        ndex += 1;
                        if ga(&mut s.core, ab, mdex) == 0.0 {
                            s.core.set_a(ab + mdex - 1, 1.0);
                        }
                        let v = s.core.word(ab + mdex - 1);
                        s.core.set_word(ab + ndex - 1, v);
                    }
                }
            }
        }
        let nup = ndex - nrnge;
        s.scr3.put(&mut s.core, &Items::new().w(ab + nrnge, nup));
    }
    s.scr3.put(&mut s.core, &Items::new().w(ab, nrnge));
    Ok(())
}

/// `brkdat` (`ccccr.f90:1989-2419`): every isotope's factors onto `nscrt3`, and
/// the file data in the container.
fn brkdat(s: &mut State) -> Result<(), NjoyError> {
    const EPS: f64 = 1.0e-04;
    const EPS2: f64 = 1.0e-10;
    let l1 = s.l1;
    let (ngps, niso) = (s.ngps, s.niso);
    s.core.set_ia(l1 + 3 * MULT + 1, ngps as i32);
    s.core.set_ia(l1 + 3 * MULT + 2, niso as i32);
    s.core.set_ia(l1 + 3 * MULT + 5, s.nreact as i32);
    let ntl = s.nti.abs();
    let nzl = s.nzi.abs();
    s.item = [0.0; 21];
    s.isig = [0.0; 21];
    let l9 = s.next;
    let l10 = l9 + MULT * niso;
    let l11 = l10 + ngps;
    let l12 = l11 + 1;
    s.l13 = l12 + niso;
    let l13 = s.l13;
    let l14 = l13 + niso;
    let l15 = l14 + niso;
    let l16 = l15 + niso;
    let l17 = l16 + niso * nzl;
    s.next = l17 + niso * ntl;
    let l18 = s.next;

    s.nin.rewind();
    s.nin.tpidio(&mut s.h)?;
    loop {
        s.contio_nin()?;
        if !(s.h.mfh == 0 || s.h.mth == 0) {
            break;
        }
    }
    let nz = i64::from(s.h.l2h);
    let lz = 6;
    let ntw = i64::from(s.h.n2h);
    s.listio_nin()?;
    let ngpn = i64::from(s.h.l1h);
    if ngpn != i64::from(s.core.ia(l1 + 3 * MULT + 1)) {
        return Err(NjoyError::InvalidInput("ccccr: brkdat incompatible group structures".into()));
    }
    let iqd = 1 + lz + ntw + nz + ngpn;
    for i in 1..=ngpn + 1 {
        let v = s.e(-i + 1 + iqd);
        s.core.set_a64(i - 1 + l10, v);
    }
    s.irsize = ((s.maxord + 1) * (ngpn + 1) * 7 + 100).min(2000);
    s.next = l18 + s.irsize;
    let l19 = s.next;
    s.next = l19 + (13 + nzl) * ngps;
    let l20 = s.next;
    let nosize = ISIZA4 - l20 + 1;
    let iffile = s.nreact * ngpn * ntl * nzl;
    s.nsblk = 1;
    if iffile > nosize {
        return Err(NjoyError::NotPorted("ccccr: BRKOXS factors larger than the container (nsblk = nreact)"));
    }
    s.lrsize = iffile / s.nsblk;
    let inv = s.nreact / s.nsblk;
    s.core.set_ia(l1 + 3 * MULT + 6, 0);

    let mut imusd = [0_i64; 201];
    s.core.set_ia(l1 + 3 * MULT + 3, 0);
    s.core.set_ia(l1 + 3 * MULT + 4, 0);
    let mut jiso = 0;
    s.scr3.rewind();
    let mut ntabt = [0_i64; 201];
    let mut ntabp = [0_i64; 201];
    let mut l20 = l20;
    'isotopes: for i in 1..=niso {
        s.nin.rewind();
        s.scr2.rewind();
        s.jbl = ngpn;
        s.jbh = 0;
        for j in l18..=ISIZA4 {
            s.core.set_word(j, 0);
        }
        s.mt2tem = [0; 11];
        s.ntfl = 0;
        s.nin.tpidio(&mut s.h)?;
        let (matd, iin, head_pos);
        loop {
            let at = s.nin.pos;
            s.contio_nin()?;
            if s.h.mfh == 0 || s.h.mth == 0 {
                continue;
            }
            if s.h.math == -1 {
                s.notes.push("brkdat: all available mats have been processed".into());
                return Ok(());
            }
            let math = i64::from(s.h.math);
            if let Some(j) = (1..=niso).find(|&j| math == s.imat[j as usize] && imusd[j as usize] != 1) {
                iin = j;
                matd = s.imat[j as usize];
                s.xspo = s.xspot[j as usize];
                imusd[j as usize] = 1;
                head_pos = at;
                break;
            }
            s.nin.tomend(None)?;
        }
        let ntw = i64::from(s.h.n2h);
        s.nzj = 0;
        let nzt = i64::from(s.h.l2h);
        s.listio_nin()?;
        for k in 1..=nzt {
            let lcsg0 = 6 + ntw + k;
            let ek = s.e(lcsg0);
            let take = s.nzi < 0 || (1..=nzl).any(|l| (s.asig[l as usize] - ek).abs() <= EPS * s.asig[l as usize]);
            if take {
                s.nzj += 1;
                if s.nzj > 20 {
                    return Err(NjoyError::InvalidInput("ccccr: more than 20 sigma-zeros".into()));
                }
                s.isig[s.nzj as usize] = k as f64;
                s.csig[s.nzj as usize] = ek;
            }
        }
        // 190: back to the head of this material (`skiprz(nin,-nrec)`)
        s.scr1.rewind();
        s.nin.pos = head_pos;
        s.ntj = 0;
        let mut itzro = 0;
        'temps: for _k in 1..=ntl {
            loop {
                // 210
                s.contio_nin()?;
                if i64::from(s.h.math) != matd {
                    break 'temps;
                }
                s.listio_nin()?;
                let c1h = s.h.c1h;
                if itzro == 0 {
                    s.tzro = c1h;
                    itzro = 1;
                    s.nin.tofend(None)?;
                    s.nin.tomend(Some(&mut s.scr1))?;
                    let tz = s.tzro;
                    if s.nti < 0 || (1..=ntl).any(|l| (s.atem[l as usize] - tz).abs() <= EPS * tz) {
                        s.ntj += 1;
                        s.ctem[s.ntj as usize] = tz;
                        s.item[s.ntj as usize] = s.ntj as f64;
                        break;
                    }
                    continue;
                }
                // 235
                if s.nti < 0 || (1..=ntl).any(|l| (s.atem[l as usize] - c1h).abs() <= EPS * c1h) {
                    s.nin.tofend(None)?;
                    s.ntj += 1;
                    if s.ntj > 20 {
                        return Err(NjoyError::InvalidInput("ccccr: more than 20 temperatures".into()));
                    }
                    s.ctem[s.ntj as usize] = c1h;
                    s.item[s.ntj as usize] = s.ntj as f64;
                    s.nin.tomend(Some(&mut s.scr1))?;
                    break;
                }
                s.nin.tomend(None)?;
            }
        }
        // 255
        s.scr1.atend();
        if s.ntj == 0 {
            s.notes.push(format!("brkdat: no temperatures for mat={matd}"));
            continue 'isotopes;
        }
        jiso += 1;
        let jj = (l9 - 1) / MULT + jiso;
        let h = s.hisonm[iin as usize].clone();
        s.core.set_ta(jj, &h);
        s.core.set_ia(l1 + 3 * MULT + 2, jiso as i32);
        s.next = l18 + s.irsize;
        let nwd = (13 + s.nzj) * ngpn;
        let l19 = s.next;
        s.next = l19 + nwd;
        l20 = s.next;
        let (ntj, nzj) = (s.ntj, s.nzj);
        for nsb in 1..=s.nsblk {
            s.nsb = nsb;
            s.scr1.rewind();
            loop {
                s.contio_scr1()?;
                if s.h.math == -1 {
                    break;
                }
                if s.h.mfh == 0 || s.h.mth == 0 {
                    continue;
                }
                if s.h.mfh == 3 {
                    xsproc(s, l18, l19, l20)?;
                }
                if s.h.mfh == 6 {
                    mxproc(s, l18, l20)?;
                }
                s.scr1.tosend(None)?;
            }
            // 270: transport self-shielding factors
            if s.nsblk == 1 || nsb == 4 {
                for j in 1..=ngpn {
                    let locsg0 = l19 - 1 + 3 * ngpn + j;
                    for k in 1..=ntj {
                        for l in 1..=nzj {
                            let locsg = l20 - 1 + (3 / nsb) * ngpn * nzj * ntj + (j - 1) * nzj * ntj + nzj * (k - 1) + l;
                            let d = s.core.a(locsg0);
                            if f64::from(d) >= EPS2 {
                                let n = s.core.a(locsg);
                                s.core.set_a(locsg, n / d);
                            } else {
                                s.core.set_a(locsg, 1.0);
                            }
                        }
                    }
                }
            }
            if s.ntfl != 1 {
                let mtc: i64 = s.mt2tem[1..].iter().sum();
                if mtc != ntj {
                    s.notes.push("brkdat: need elastic matrices at higher temperatures".into());
                }
            }
            if nsb == 1 {
                let nwon = l19 + 7 * ngpn;
                s.scr2.put(&mut s.core, &Items::new().w(nwon, 6 * ngpn));
            }
            for iv in 1..=inv {
                for j in 1..=ngpn {
                    for k in 1..=ntj {
                        for l in 1..=nzj {
                            let noloc = l20 - 1 + (iv - 1) * ngpn * nzj * ntj + (j - 1) * ntj * nzj + (k - 1) * nzj + l;
                            let abe = f64::from(s.core.a(noloc)).abs();
                            if abe < EPS2 {
                                s.core.set_a(noloc, 1.0);
                            }
                            let test = 1.0_f64 / 100.0;
                            if f64::from(s.core.a(noloc)) < test {
                                s.core.set_a64(noloc, test);
                            }
                            if f64::from((s.core.a(noloc) - 1.0_f32).abs()) >= SSTOL {
                                s.jbh = s.jbh.max(j);
                                s.jbl = s.jbl.min(j);
                                s.ntat[k as usize] = 1.0;
                                s.ntap[l as usize] = 1.0;
                            }
                        }
                    }
                }
            }
            let (jbl, jbh) = (s.jbl as i32, s.jbh as i32);
            s.core.set_ia(l12 - 1 + i, jbl);
            s.core.set_ia(l13 - 1 + i, jbh);
            let lr = s.lrsize;
            s.scr2.put(&mut s.core, &Items::new().w(l20, lr));
        }
        // isotope flags
        let iu = i as usize;
        ntabt[iu] = 0;
        for j in 1..=ntj {
            if s.ntat[j as usize] != 0.0 {
                ntabt[iu] += 1;
                let ndex = l17 + (i - 1) * ntl + ntabt[iu] - 1;
                let v = s.ctem[j as usize];
                s.core.set_a64(ndex, v);
            }
        }
        ntabp[iu] = 0;
        for j in 1..=nzj {
            if s.ntap[j as usize] != 0.0 {
                ntabp[iu] += 1;
                let ndex = l16 + (i - 1) * nzl + ntabp[iu] - 1;
                let v = s.csig[j as usize];
                s.core.set_a64(ndex, v);
            }
        }
        s.core.set_ia(l14 + i - 1, ntabp[iu] as i32);
        s.core.set_ia(l15 + i - 1, ntabt[iu] as i32);
        thnwrt(s, l18)?;
    }

    // rearrange and thin the file data
    for i in 1..=ngpn + 1 {
        let v = s.core.word(l10 + i - 1);
        s.core.set_word(l20 + i - 1, v);
        s.core.set_word(l10 + i - 1, 0);
    }
    for i in 1..=4 * niso {
        let ndex = ngpn + i;
        let v = s.core.word(l10 + ndex);
        s.core.set_word(l20 + ndex, v);
        s.core.set_word(l10 + ndex, 0);
    }
    let mut ndex = l10 - 1;
    s.core.set_ia(l1 + 3 * MULT + 3, 0);
    for i in 1..=jiso {
        let nzf = i64::from(s.core.ia(l20 - 1 + ngpn + 1 + 2 * niso + i));
        for jz in 1..=nzf {
            ndex += 1;
            let mdex = l16 + (i - 1) * nzl + jz - 1;
            let amd = f64::from(s.core.a(mdex));
            s.core.set_a64(ndex, petir::real::log10(amd));
            let test = 1.0_f64 / 100000.0;
            if f64::from(s.core.a(ndex)).abs() < test {
                s.core.set_a(ndex, 0.0);
            }
            s.core.set_a(mdex, 0.0);
            let n = s.core.ia(l1 + 3 * MULT + 3);
            s.core.set_ia(l1 + 3 * MULT + 3, n + 1);
        }
    }
    s.core.set_ia(l1 + 3 * MULT + 4, 0);
    for i in 1..=jiso {
        let ntf = i64::from(s.core.ia(l20 - 1 + ngpn + 1 + 3 * niso + i));
        for jt in 1..=ntf {
            ndex += 1;
            let mdex = l17 + (i - 1) * ntl + jt - 1;
            // `a(mdex)-273.16`: a default-real literal, so single precision
            let v = s.core.a(mdex) - 273.16_f32;
            s.core.set_a(ndex, v);
            let n = s.core.ia(l1 + 3 * MULT + 4);
            s.core.set_ia(l1 + 3 * MULT + 4, n + 1);
            s.core.set_a(mdex, 0.0);
        }
    }
    for i in 1..=ngpn + 1 {
        let v = s.core.word(l20 + i - 1);
        s.core.set_word(ndex + i, v);
        s.core.set_word(l20 + i - 1, 0);
    }
    ndex += ngpn + 1;
    for i in 1..=4 {
        let k = (i - 1) * jiso;
        let l = ngpn + (i - 1) * niso;
        for j in 1..=jiso {
            let v = s.core.word(j + l + l20);
            s.core.set_word(j + k + ndex, v);
            s.core.set_word(j + l + l20, 0);
        }
    }
    Ok(())
}

/// `cbrkxs` (`ccccr.f90:1873-1972`, with `ruinbr`): the BRKOXS file.
///
/// # Errors
/// The refusal of `brkdat`; a GENDF tape that ends early; an index outside the
/// container.
pub fn cbrkxs(s: &mut State, cards: &BrkoxsCards) -> Result<Vec<u8>, NjoyError> {
    let mut out = Records::default();
    s.next = 1;
    s.l1 = s.next;
    s.next = s.l1 + 3 * MULT + 1 + 6;
    s.next += 1;
    // ruinbr
    s.nti = i64::from(cards.nti);
    s.nzi = i64::from(cards.nzi);
    s.nreact = 6;
    for (k, v) in cards.atem.iter().enumerate().take(20) {
        s.atem[k + 1] = *v;
    }
    for (k, v) in cards.asig.iter().enumerate().take(20) {
        s.asig[k + 1] = *v;
    }
    brkdat(s)?;

    let l1 = s.l1;
    let nwds = 3 * MULT + 1;
    let l1h = (l1 - 1) / MULT + 1;
    s.core.set_ta(l1h, "brkoxs");
    let [h1, h2] = s.huse;
    s.core.set_ha(l1h + 1, h1);
    s.core.set_ha(l1h + 2, h2);
    let iv = s.ivers as i32;
    s.core.set_ia(3 * MULT + l1, iv);
    out.put(&mut s.core, &Items::new().ha(1, 3).w(nwds, 1));

    let l2 = l1 + 3 * MULT + 1;
    s.l2 = l2;
    let g = |c: &mut Core, k: i64| i64::from(c.ia(l2 + k));
    let ngps = g(&mut s.core, 0);
    let nisosh = g(&mut s.core, 1);
    let nsigpt = g(&mut s.core, 2);
    let ntempt = g(&mut s.core, 3);
    let nreact = g(&mut s.core, 4);
    let iblk = g(&mut s.core, 5);
    out.put(&mut s.core, &Items::new().w(l2, 6));

    let l3 = l2 + 6 + 1;
    let l3h = (l3 - 1) / MULT;
    let nwh = MULT * nisosh;
    let l3r = l3 + MULT * nwh - 1;
    let nwr = nsigpt + ntempt + ngps + 1;
    let l3i = l3r + nwr;
    let nwi = 4 * nisosh;
    out.put(&mut s.core, &Items::new().ha(l3h + 1, nwh).w(l3r + 1, nwr).w(l3i + 1, nwi));
    let l4 = l3 + MULT * nwh + nwr + nwi;

    s.scr3.rewind();
    for i in 1..=nisosh {
        let loc = l3 + MULT * nisosh + nsigpt + ntempt + ngps + 1 + i - 1;
        let jbl = i64::from(s.core.ia(loc));
        let jbh = i64::from(s.core.ia(loc + nisosh));
        let ntap = i64::from(s.core.ia(loc + 2 * nisosh));
        let ntat = i64::from(s.core.ia(loc + 3 * nisosh));
        let (nblok, ml, mu) = if iblk == 0 { (1, 1, nreact) } else { (nreact, 1, 1) };
        let nwds = ntap * ntat * (jbh - jbl + 1) * (mu - ml + 1);
        for _j in 1..=nblok {
            let it = Items::new().w(l4, nwds);
            s.scr3.get(&mut s.core, &it)?;
            out.put(&mut s.core, &it);
        }
        let it = Items::new().w(l4, 6 * ngps);
        s.scr3.get(&mut s.core, &it)?;
        out.put(&mut s.core, &it);
    }
    s.next = s.l1;
    s.core.check("cbrkxs")?;
    Ok(out.bytes)
}
