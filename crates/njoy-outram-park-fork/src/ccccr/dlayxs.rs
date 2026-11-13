// SPDX-License-Identifier: AGPL-3.0-only
// Translated from NJOY2016.80 `src/ccccr.f90` (git commit ac5adf5f33d893e42f2eed7fb286b0d51c7580da):
//   - `cdlyxs`, l.3005-3076; `dldata`, l.3078-3291.
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under AGPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! **DLAYXS** (gh:#1369): delayed-neutron precursor decay constants, spectra
//! and yields, as `cdlyxs` writes them (a CCCC-III file).

use crate::ccccr::core::{text8, Items, Records, ISIZA4, MULT};
use crate::ccccr::state::State;
use crate::NjoyError;

/// `ndmax` (`ccccr.f90:3108`): most delayed-neutron groups accepted.
const NDMAX: usize = 8;

/// `dldata` (`ccccr.f90:3078-3291`): the delayed data of the requested isotopes
/// into the container. Sets `nisod` (0: none found) and `nfam`.
fn dldata(s: &mut State) -> Result<(), NjoyError> {
    let nisoo = s.niso;
    let ngps = s.ngps;
    s.nin.rewind();
    s.nin.tpidio(&mut s.h)?;
    while s.h.mfh < 5 {
        s.contio_nin()?;
    }
    let ndg;
    if s.h.mth == 455 {
        ndg = s.e(3).round() as i64;
    } else if s.h.mth < 455 {
        while s.h.mfh == 5 && s.h.mth < 455 {
            s.contio_nin()?;
        }
        if s.h.mfh == 5 {
            ndg = s.e(3).round() as i64;
        } else {
            s.nisod = 0;
            return Ok(());
        }
    } else {
        s.nisod = 0;
        return Ok(());
    }
    if ndg == 0 {
        s.nisod = 0;
        return Ok(());
    } else if ndg as usize > NDMAX {
        s.notes.push("dldata: too many delayed neutron groups; dlayxs request ignored".into());
        s.nisod = 0;
        return Ok(());
    }

    s.nfam = ndg * nisoo;
    let nfam = s.nfam;
    let lzero = s.next;
    s.next = lzero + 3 * MULT + MULT + 4;
    s.l1 = s.next;
    s.next = s.l1 + nisoo * MULT;
    s.l2 = s.next;
    s.next = s.l2 + nfam;
    let l3 = s.next;
    s.next = l3 + ngps * nfam;
    let l4 = s.next;
    s.next = l4 + ngps;
    let l5 = s.next;
    s.next = l5 + 1;
    let l6 = s.next;
    s.next = l6 + nisoo;
    let l7 = s.next;
    s.next = l7 + nisoo;
    let l8 = s.next;
    s.next = l8 + ngps * nfam + nfam;
    let l9 = s.next;
    let la = l9 + 5;
    let last = la + 10 * ngps;
    if last > ISIZA4 {
        return Err(NjoyError::InvalidInput("ccccr: dldata max4a for container a exceeded".into()));
    }
    let (l1, l2) = (s.l1, s.l2);

    let mut imusd = [0_i64; 201];
    let mut ipr = 0;
    s.nisod = 0;
    for i in l1..=last {
        s.core.set_word(i, 0);
    }
    s.nin.rewind();
    s.nin.tpidio(&mut s.h)?;
    let mut fract = [0.0_f64; NDMAX + 1];
    let mut hisnm = String::new();
    let (mut nl, mut nz, mut ng, mut ig, mut ngn) = (0_i64, 0_i64, 0_i64, 0_i64, 0_i64);
    // `state` 110: find a material; 130: next section; 170: next record.
    let mut at = 110;
    loop {
        match at {
            110 => {
                s.contio_nin()?;
                if s.h.math == -1 {
                    return Ok(());
                }
                if s.h.mfh == 0 {
                    continue;
                }
                let math = i64::from(s.h.math);
                let mut found = false;
                for i in 1..=nisoo {
                    hisnm = s.hisonm[i as usize].clone();
                    if math != s.imat[i as usize] || imusd[i as usize] > 0 {
                        continue;
                    }
                    imusd[i as usize] = 1;
                    found = true;
                    break;
                }
                if found {
                    nl = i64::from(s.h.l1h);
                    nz = i64::from(s.h.l2h);
                    ng = i64::from(s.h.n2h);
                    at = 170;
                } else {
                    s.nin.tomend(None)?;
                }
            }
            130 => {
                if ig < ngps {
                    at = 170;
                    continue;
                }
                s.contio_nin()?;
                if s.h.math == 0 {
                    at = 110;
                    continue;
                }
                if s.h.mfh == 0 || s.h.mth == 0 {
                    continue;
                }
                nl = i64::from(s.h.l1h);
                nz = i64::from(s.h.l2h);
                ng = i64::from(s.h.n2h);
                at = 170;
            }
            _ => {
                // 170
                s.listio_nin()?;
                let ng2 = i64::from(s.h.l1h);
                ig = i64::from(s.h.n2h);
                at = 130;
                let (mfh, mth) = (s.h.mfh, s.h.mth);
                if mth == 451 {
                    // 210
                    s.nisod += 1;
                    if ipr > 0 {
                        continue;
                    }
                    ngn = ng2;
                    if ngn != ngps {
                        return Err(NjoyError::InvalidInput("ccccr: dldata incompatible group structures".into()));
                    }
                    for i in 1..=ngn + 1 {
                        let loca = l4 + ngn - i + 1;
                        let v = s.e(6 + ng + nz + i);
                        s.core.set_a64(loca, v);
                    }
                    ipr = 1;
                    ig = ngps;
                } else if mfh == 3 && mth == 455 {
                    // 310
                    let nisod = s.nisod;
                    for i in 1..=ndg {
                        let ifam = ndg * (nisod - 1) + i;
                        let loca = l8 + ngn - ig + ngn * (ifam - 1) + ndg * (nisod - 1);
                        let v = s.e(8);
                        s.core.set_a64(loca, v);
                    }
                } else if mfh == 5 && mth == 455 {
                    // 410
                    let nisod = s.nisod;
                    let nd = nl;
                    if nd as usize > NDMAX {
                        return Err(NjoyError::InvalidInput("ccccr: dldata more families than ndmax".into()));
                    }
                    let ig1 = i64::from(s.h.l1h) - 1;
                    let jj = (l1 - 1) / MULT + nisod;
                    s.core.set_ha(jj, text8(&hisnm));
                    s.core.set_ia(l7 + nisod - 1, (nisod - 1) as i32);
                    s.core.set_ia(l6 + nisod - 1, nd as i32);
                    let ifam0 = ndg * (nisod - 1) - 1 + l2;
                    for i in 1..=nd {
                        let v = s.e(i + 6);
                        s.core.set_a64(i + ifam0, v);
                    }
                    for i in 1..=nd {
                        let ifam = ndg * (nisod - 1) + i;
                        let loca = l3 + ngn + ngn * (ifam - 1);
                        let locb = 6 + i;
                        fract[i as usize] = 0.0;
                        for j in 1..=ig1 {
                            let v = s.e(nl * j + locb);
                            s.core.set_a64(loca - j, v);
                            fract[i as usize] += v;
                        }
                    }
                    for i in 1..=nd {
                        let ifam = i + ndg * (nisod - 1);
                        let loca = l8 + ngn + ngn * (ifam - 1) + ndg * (nisod - 1);
                        let locb = l8 - 1 + ndg * ngn * nisod + ndg * (nisod - 1) + i;
                        s.core.set_ia(locb, ifam as i32);
                        for j in 1..=ngn {
                            let v = fract[i as usize] * f64::from(s.core.a(loca - j));
                            s.core.set_a64(loca - j, v);
                        }
                    }
                }
            }
        }
    }
}

/// `cdlyxs` (`ccccr.f90:3005-3076`): the DLAYXS file, `None` when the tape has
/// no delayed-neutron data (upstream writes nothing then).
///
/// # Errors
/// A GENDF tape that ends early; an index outside the container.
pub fn cdlyxs(s: &mut State) -> Result<Option<Vec<u8>>, NjoyError> {
    let lzero = s.next;
    dldata(s)?;
    if s.nisod == 0 {
        s.notes.push("cdlyxs: no delayed neutron data found".into());
        s.next = s.l1;
        return Ok(None);
    }
    let mut out = Records::default();
    let nwds = 1 + 3 * MULT;
    let lzeroh = (lzero - 1) / MULT + 1;
    s.core.set_ta(lzeroh, "dlayxs");
    let [h1, h2] = s.huse;
    s.core.set_ha(lzeroh + 1, h1);
    s.core.set_ha(lzeroh + 2, h2);
    let iv = s.ivers as i32;
    s.core.set_ia(3 * MULT + lzero, iv);
    out.put(&mut s.core, &Items::new().ha(lzeroh, 3).w(lzeroh + nwds - 1, 1));

    let (ngps, nisod, nfam) = (s.ngps, s.nisod, s.nfam);
    s.core.set_ia(lzero, ngps as i32);
    s.core.set_ia(lzero + 1, nisod as i32);
    s.core.set_ia(lzero + 2, nfam as i32);
    s.core.set_ia(lzero + 3, nfam as i32);
    out.put(&mut s.core, &Items::new().w(lzero, 4));

    let l1 = s.l1;
    let l1h = (l1 - 1) / MULT;
    let nwh = nisod;
    let l1r = l1 + MULT * nwh - 1;
    let nwr = (ngps + 1) * (nfam + 1);
    let l1i = l1r + nwr;
    let nwi = 2 * nisod;
    out.put(&mut s.core, &Items::new().ha(l1h + 1, nwh).w(l1r + 1, nwr).w(l1i + 1, nwi));
    let mut l3 = l1 + MULT * nwh + nwr + nwi;
    for n in 1..=nisod {
        let l4 = l1 + nisod * MULT + nfam + ngps * nfam + ngps;
        let nkfam = i64::from(s.core.ia(l4 + n));
        let llr = l3 - 1;
        let nwr = ngps * nkfam;
        let lli = llr + nwr;
        out.put(&mut s.core, &Items::new().w(llr + 1, nwr).w(lli + 1, nkfam));
        l3 += nwr + nkfam;
    }
    s.next = s.l1;
    s.core.check("cdlyxs")?;
    Ok(Some(out.bytes))
}
