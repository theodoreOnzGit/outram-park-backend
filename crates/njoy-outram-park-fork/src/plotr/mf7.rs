// SPDX-License-Identifier: AGPL-3.0-only
// Translated from NJOY2016.80 `src/plotr.f90` (git commit ac5adf5f33d893e42f2eed7fb286b0d51c7580da):
//   `subroutine plotr`, l.949-1096 (curves from file 7).
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under AGPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! MF=7 `S(alpha, beta)` curves: against alpha at beta number `nth`
//! (`ntp = 1`), or against beta at alpha number `nth` (`ntp > 1`).
//!
//! Upstream overwrites PLOTR's `nr` (the right-axis label length) with the
//! TAB1's `NR` here, and so does this port.

use petir::mathf::RealMath;

use super::run::{perr, Go, Plotr, Res};
use super::stream::{nint, NPAGE};

/// `therm` (`plotr.f90:408`).
const THERM: f64 = 296.0;
/// `sabmin` (`plotr.f90:409`).
const SABMIN: f64 = 1.0e-32;

impl Plotr {
    /// 1410 to 1096.
    pub(super) fn at_1410(&mut self) -> Res<Go> {
        let nin = self.nin;
        self.mmf = 7;
        let lat = self.h.l2h;
        let lasym = self.h.n1h;
        self.listio(nin, 1)?;
        let lln = self.h.l1h;
        self.tab2io(nin, 1)?;
        let nbeta = self.h.n2h;
        let mut int2 = nint(self.a[7]);
        let temper = self.temper;
        let s_of = |mut s: f64, beta0: f64, nkh: i32| {
            let mut beta = beta0;
            if lat == 1 {
                beta = beta * THERM / temper;
            }
            if lln == 1 {
                s = s.r_exp();
            }
            if nkh == 2 {
                s *= (beta / 2.0).r_exp();
            }
            if nkh == 3 {
                s *= (-beta / 2.0).r_exp();
            }
            if s < SABMIN {
                s = SABMIN;
            }
            s
        };
        if self.ntp <= 1 {
            let mut ibeta = 1;
            while ibeta != self.nth {
                let (nb, _) = self.tab1io(nin, 1)?;
                let lt = self.h.l1h;
                self.skip_pages(nin, 1, nb)?;
                for _ in 0..lt.max(0) {
                    let (nb, _) = self.listio(nin, 1)?;
                    self.skip_pages(nin, 1, nb)?;
                }
                ibeta += 1;
                if ibeta > nbeta {
                    return Err(perr("illegal nth for mf7"));
                }
            }
            let mut l = 1usize;
            self.l2 = Some(1);
            let (nb, nw) = self.tab1io(nin, l)?;
            let mut tnow = self.h.c1h;
            let lt = self.h.l1h;
            let mut nt = 0;
            let nr = self.h.n1h;
            self.nr = Some(nr);
            let mut np = self.h.n2h;
            l += nw;
            self.skip_pages(nin, l, nb)?;
            let nr = nr.max(0) as usize;
            loop {
                let test = 1.0 / 10.0 + tnow / 100.0;
                if (temper - tnow).abs() < test {
                    break;
                }
                let ll = l;
                let (mut nb, nw) = self.listio(nin, l)?;
                tnow = self.h.c1h;
                np = self.h.n1h;
                l += nw;
                while nb != 0 {
                    let (b, w) = self.moreio(nin, l, nb)?;
                    nb = b;
                    l += w;
                }
                for i in 1..=np.max(0) as usize {
                    self.a[6 + 2 * nr + 2 * i - 1] = self.a[ll + 5 + i - 1];
                }
                l = ll;
                nt += 1;
                if nt > lt {
                    return Err(perr("temperature not found"));
                }
            }
            let beta = self.a[1];
            let nkh = self.nkh;
            for i in 1..=np.max(0) as usize {
                let k = 6 + 2 * nr + 2 * i - 1;
                self.a[k] = s_of(self.a[k], beta, nkh);
            }
            if lln != 0 {
                for i in 1..=nr {
                    self.a[5 + 2 * i - 1] += 1.0;
                }
            }
            return Ok(Go::L1409);
        }
        // 1440
        let l2 = NPAGE + 50;
        self.l2 = Some(l2);
        for (k, v) in [0.0, 0.0, 0.0, 0.0, 1.0, f64::from(nbeta), f64::from(nbeta)]
            .iter()
            .enumerate()
        {
            self.a[l2 - 1 + k] = *v;
        }
        if lln == 1 {
            int2 += 1;
        }
        self.a[l2 + 6] = f64::from(int2);
        for ibeta in 1..=nbeta.max(0) as usize {
            let mut l = 1usize;
            let mut ithere = false;
            let (mut nb, nw) = self.tab1io(nin, l)?;
            let mut tnow = self.h.c1h;
            let lt = self.h.l1h;
            let mut nt = 0;
            let nr = self.h.n1h;
            self.nr = Some(nr);
            let nr = nr.max(0) as usize;
            l += nw;
            while nb != 0 {
                let (b, w) = self.moreio(nin, l, nb)?;
                nb = b;
                l += w;
            }
            let mut ll = l;
            while nt <= lt {
                if nt != 0 {
                    ll = l;
                    let (mut nb, nw) = self.listio(nin, l)?;
                    tnow = self.h.c1h;
                    l += nw;
                    while nb != 0 {
                        let (b, w) = self.moreio(nin, l, nb)?;
                        nb = b;
                        l += w;
                    }
                    l = ll;
                }
                if (temper - tnow).abs() < 0.01 {
                    self.a[l2 + 6 + 2 * ibeta - 1] = self.a[1];
                    if lasym > 0 {
                        let v = self.a[1] * self.factx;
                        self.set_x(ibeta, v);
                    }
                    let nth = self.nth.max(0) as usize;
                    let raw = if nt == 0 {
                        self.a[6 + 2 * nr + 2 * nth - 1]
                    } else {
                        self.a[ll + 5 + nth - 1]
                    };
                    let yf = s_of(raw, self.a[1], self.nkh);
                    self.yf = yf;
                    self.a[l2 + 7 + 2 * ibeta - 1] = yf;
                    if lasym > 0 {
                        let v = yf * self.facty;
                        self.set_y(ibeta, v);
                    }
                    ithere = true;
                }
                nt += 1;
            }
            if !ithere {
                return Err(perr("temperature not found"));
            }
        }
        if lasym == 0 {
            return Ok(Go::L1409);
        }
        self.n = Some(nbeta.max(0) as usize);
        Ok(Go::L610)
    }
}
