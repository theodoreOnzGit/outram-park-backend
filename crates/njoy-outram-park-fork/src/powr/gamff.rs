// SPDX-License-Identifier: AGPL-3.0-only
// Translated from NJOY2016.80 `src/powr.f90` (git commit ac5adf5f33d893e42f2eed7fb286b0d51c7580da):
//   - `gamff` (l.1401-1542).
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under AGPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! `lib = 1`'s f-factors (`gamff`). Upstream does not reset `l` after paging
//! a record here (as `gamxs` does); that is kept.

use super::fast::Locs;
use super::reader::{Reader, NWSCR};
use crate::NjoyError;

fn err(sub: &str, msg: &str) -> NjoyError {
    NjoyError::InvalidInput(format!("{sub}: {msg}"))
}

/// `gamff(...)` (`powr.f90:1401-1542`): absorption and fission f-factors over
/// the sigma zeroes and temperatures, from the reference temperature on.
pub(super) fn gamff(r: &mut Reader, ngmin: usize, ngmax: usize, mut o: Locs, a: &mut [f64], nloc: usize) -> Result<Locs, NjoyError> {
    let toler = 0.01f64;
    let iloc = 1usize;
    let lz = 6usize;
    let ngnd = r.ngnd as usize;
    let (ntp, nsigz) = (r.ntp.max(0) as usize, r.nsigz.max(0) as usize);
    o.locsf = o.locabs + ngnd * ntp * nsigz;
    let na = o.locsf + ngnd * ntp * nsigz - 1;
    if na.saturating_sub(iloc) > nloc {
        return Err(err("gamff", "storage exceeded."));
    }
    for v in a.iter_mut().take(na + 1).skip(o.locabs) {
        *v = 0.0;
    }
    let mut itp = 1usize;
    let mut mf3mt1 = 1;
    let mut i318 = 0;
    let mut l;
    // 110
    'l110: loop {
        l = 1;
        r.contio(l)?;
        if r.h.math <= 0 {
            if itp == ntp {
                break 'l110;
            }
            itp += 1;
            i318 = 0;
        }
        // 120
        if r.h.mfh == 0 || r.h.mth == 0 {
            continue;
        }
        if r.h.mfh != 3 {
            r.tosend()?;
            continue;
        }
        // 130
        if mf3mt1 <= 0 && r.h.mfh == 3 && r.h.mth >= 2 {
            mf3mt1 = 1;
            for i in 1..=ngnd {
                if r.cflux[i] != 0.0 {
                    r.cflux[i] = 1.0 / r.cflux[i];
                }
            }
        }
        let mth = r.h.mth;
        let take = (mth > 101 && mth < 108) || (mth > 17 && mth < 22) || mth == 38 || mth == 1;
        if !take {
            // 150
            r.tosend()?;
            continue;
        }
        // 160
        let nl = r.h.l1h as usize;
        let nz = r.h.l2h as usize;
        let mut lim = nz;
        if nsigz < nz {
            lim = nsigz;
        }
        // 170
        loop {
            l = 1;
            let (mut nb, mut nw) = r.listio(l)?;
            let ig = r.h.n2h;
            let igc = r.icgrp(ig);
            while nb != 0 {
                l += nw;
                if l + nw - 1 > NWSCR {
                    return Err(err("gamff", "storage exceeded."));
                }
                (nb, nw) = r.moreio(l, nb)?;
            }
            if !(igc == 0 || igc > ngnd as i32) {
                let jg = ngnd - igc as usize + 1;
                if mth >= 102 {
                    // capture
                    let loc = o.locabs - 1 + nsigz * ((itp - 1) + ntp * (jg - 1));
                    let loca = l + lz + nl * (nz - 1);
                    for jz in 1..=lim {
                        a[jz + loc] = r.sc(nl * jz + loca) * r.sc(l + lz) * r.cflux[jg] + a[jz + loc];
                    }
                } else if mth != 1 {
                    // fission
                    if mth == 18 {
                        i318 = 1;
                    }
                    if mth > 18 && i318 > 0 {
                        // 150
                        r.tosend()?;
                        continue 'l110;
                    }
                    r.jwf = 1;
                    // upstream: iadd = nsigz*((itp-1)+ntp*(jg-1))-1; a(jz+iadd+...)
                    let iadd = nsigz * ((itp - 1) + ntp * (jg - 1));
                    let loca = l + lz + nl * (nz - 1);
                    for jz in 1..=lim {
                        let v = r.sc(nl * jz + loca) * r.sc(l + lz) * r.cflux[jg];
                        a[jz + iadd - 1 + o.locsf] += v;
                        a[jz + iadd - 1 + o.locabs] += v;
                    }
                } else {
                    // flux
                    if igc <= 1 {
                        for i in 1..=ngnd {
                            r.cflux[i] = 0.0;
                        }
                        mf3mt1 = 0;
                    }
                    r.cflux[jg] += r.sc(l + lz);
                }
            }
            // 210
            if ig >= r.ngn {
                continue 'l110;
            }
        }
    }
    // 300: calculate f-factors
    let mut iglo = ngmin;
    for ig in ngmin..=ngmax {
        let mut ab0 = a[o.locab0 + ig - 1];
        if ab0 != 0.0 {
            ab0 = 1.0 / ab0;
        }
        let mut sf0 = a[o.locsf0 + ig - 1];
        if sf0 != 0.0 {
            sf0 = 1.0 / sf0;
        }
        for jz in 1..=nsigz {
            for itp in 1..=ntp {
                let iadd = jz - 1 + nsigz * ((itp - 1) + ntp * (ig - 1));
                let loc = o.locabs + iadd;
                if ab0 == 0.0 {
                    a[loc] = 1.0;
                }
                if ab0 != 0.0 {
                    a[loc] *= ab0;
                }
                if a[loc] <= 0.0 {
                    a[loc] = 1.0;
                }
                if (1.0 - a[loc]).abs() > toler && iglo == ngmin {
                    iglo = ig;
                }
                a[loc] = r.sigz[jz].ln() - 2.0 * a[loc].ln();
                if r.jwf != 0 {
                    let loc = o.locsf + iadd;
                    if sf0 == 0.0 {
                        a[loc] = 1.0;
                    }
                    if sf0 != 0.0 {
                        a[loc] *= sf0;
                    }
                    if a[loc] <= 0.0 {
                        a[loc] = 1.0;
                    }
                    a[loc] = r.sigz[jz].ln() - 2.0 * a[loc].ln();
                }
            }
        }
    }
    r.iwr = (ngmax - iglo + 1) as i32;
    Ok(o)
}
