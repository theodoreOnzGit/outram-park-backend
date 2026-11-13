// SPDX-License-Identifier: AGPL-3.0-only
// Translated from NJOY2016.80 `src/plotr.f90` (git commit ac5adf5f33d893e42f2eed7fb286b0d51c7580da):
//   `subroutine plotr` l.1464-1686 (GENDF data), `gg3d` l.2190-2424.
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under AGPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! GENDF curves (`iverf = 1`): group cross sections or fluxes as histograms,
//! group-to-group spectra, and the `gg3d` matrix header.
//!
//! Kept from upstream: `gg3d` scales the axes and writes the 3-d header but
//! no matrix data (`:2394-2423` write nothing after `' 1/'`), and sets
//! `jtype = 2` and the default labels for later curves; a 2-d curve whose
//! values are all zero leaves `n` as the previous curve left it.

use petir::mathf::RealMath;

use super::getters::ascale;
use super::run::{c60, fi, ff, perr, undefined, Go, Plotr, Res, NWAMAX};
use super::stream::{nint, NPAGE};

impl Plotr {
    /// 400 to 1663.
    pub(super) fn at_400(&mut self) -> Res<Go> {
        let (nin, nin2) = (self.nin, self.nin2);
        for u in [nin, nin2] {
            if u != 0 {
                self.stream(u)?.rewind();
                let (s, h, _, _) = self.parts(u)?;
                s.tpidio(h)?;
            }
        }
        self.jnoth = Some(0);
        let (matd, temper) = (self.matd, self.temper);
        let ok = |t: f64| (t - temper).abs() < 1.0 / 1000.0;
        let notfound =
            |m: &str, p: &Plotr| perr(&format!("{m}{}{}", fi(p.matd, 4), ff(temper, 10, 1)));
        // 410-440
        let (mut loc, mut nw);
        loop {
            self.contio(nin, 1)?;
            if self.h.math == matd {
                loc = 7;
                let (mut nb, w) = self.listio(nin, loc)?;
                nw = w;
                if ok(self.a[6]) {
                    while nb != 0 {
                        loc += nw;
                        (nb, nw) = self.moreio(nin, loc, nb)?;
                    }
                    break;
                }
                self.stream(nin)?.tomend()?;
                continue;
            }
            if self.h.math < 0 {
                return Err(notfound("2desired mat and temp not found ", self));
            }
            if self.h.mfh != 0 {
                self.stream(nin)?.tomend()?;
            }
        }
        if nin2 != 0 {
            // 441-443: `contio` leaves `nw = 6`, so `loc` steps by 6 per record skipped.
            let matd2 = self.matd2;
            loop {
                loc += nw;
                self.contio(nin2, loc)?;
                nw = 6;
                if self.h.math == matd2 {
                    loc += 6;
                    let (mut nb, w) = self.listio(nin2, loc)?;
                    nw = w;
                    if ok(self.a[loc - 1]) {
                        while nb != 0 {
                            loc += nw;
                            (nb, nw) = self.moreio(nin2, loc, nb)?;
                        }
                        break;
                    }
                    self.stream(nin2)?.tomend()?;
                    continue;
                }
                if self.h.math < 0 {
                    return Err(notfound("3desired mat and temp not found ", self));
                }
                if self.h.mfh != 0 {
                    self.stream(nin2)?.tomend()?;
                }
            }
        }
        let nsigz = nint(self.a[3]) as usize;
        let ntw = nint(self.a[5]) as usize;
        let ngn = nint(self.a[8]);
        let ngg = nint(self.a[9]);
        let mut jbase = 12 + ntw + nsigz;
        let locngn = jbase + 1;
        jbase += ngn.max(0) as usize + 1;
        let locngg = jbase + 1;
        jbase += ngg.max(0) as usize + 1;
        jbase += 1;
        if self.i3d == 1 {
            return self.at_3400(jbase, locngn, locngg, ngn, ngg);
        }
        let (mut nl, mut nz) = (0usize, 0usize);
        let mut ig1 = 0i32;
        let mut ig = ngn;
        let mut sigig: Option<f64> = None;
        let mut loc2 = 0usize;
        let (factx, facty, nth, ntp, nkh, mfd, mtd) = (
            self.factx, self.facty, self.nth, self.ntp, self.nkh, self.mfd, self.mtd,
        );
        loop {
            // 455
            if ig >= ngn {
                self.contio(nin, jbase)?;
                if nin2 != 0 {
                    self.contio(nin2, jbase + 6)?;
                }
                if self.h.math == 0 || self.h.mfh > mfd {
                    return Ok(Go::L610);
                }
                if self.h.mfh == 0 || self.h.mth == 0 {
                    continue;
                }
                nl = self.h.l1h.max(0) as usize;
                nz = self.h.l2h.max(0) as usize;
            }
            // 465
            let (mut nb, mut nw) = self.listio(nin, jbase)?;
            let ng2 = self.h.l1h;
            let ig2lo = self.h.l2h;
            ig = self.h.n2h;
            let mut nwa = jbase + nw;
            while nb != 0 {
                (nb, nw) = self.moreio(nin, nwa, nb)?;
                nwa += nw;
                if nw + NPAGE + 12 > NWAMAX {
                    return Err(perr("storage exceeded2"));
                }
            }
            if nin2 != 0 {
                loc2 = nwa + nw;
                nwa = loc2;
                (nb, nw) = self.listio(nin2, nwa)?;
                nwa += nw;
                while nb != 0 {
                    (nb, nw) = self.moreio(nin2, nwa, nb)?;
                    nwa += nw;
                    if nw + NPAGE + 12 > NWAMAX {
                        return Err(perr("storage exceeded3"));
                    }
                }
            }
            // 467
            let (mfh, mth) = (self.h.mfh, self.h.mth);
            if mth != mtd {
                continue;
            }
            if mfd == 6 && mfh == 3 {
                // 495
                if ig == nth {
                    sigig = Some(self.a[jbase + 6 + nl * nz - 1]);
                }
                continue;
            }
            if mfh != mfd {
                continue;
            }
            if mfh == 6 {
                // 490
                if ig == nth {
                    for j in 2..=ng2.max(0) as usize {
                        let jj = (ig2lo + j as i32 - 2) as usize;
                        let jnow = 2 * (j - 2);
                        self.set_x(jnow + 1, self.a[locngn + jj - 2] * factx);
                        self.set_x(jnow + 2, self.a[locngn + jj - 1] * factx);
                        let mut s = self.a[jbase + 6 + nl * nz * (j - 1) - 1];
                        let dener = self.a[locngn + jj - 1] - self.a[locngn + jj - 2];
                        s /= dener;
                        s /= sigig.ok_or(undefined(
                            "plotr: GENDF MF=6 spectrum before its MF=3 (sigig undefined)",
                        ))?;
                        self.set_y(jnow + 1, s * facty);
                        self.set_y(jnow + 2, s * facty);
                        if s != 0.0 {
                            self.n = Some(jnow + 2);
                        }
                    }
                }
                continue;
            }
            if mfh != 3 {
                continue;
            }
            if ig1 == 0 {
                ig1 = ig;
            }
            let jnow = 2 * (ig - ig1).max(0) as usize;
            let igu = ig as usize;
            self.set_x(jnow + 1, self.a[locngn + igu - 2] * factx);
            self.set_x(jnow + 2, self.a[locngn + igu - 1] * factx);
            let (nlz, nli) = ((nl * nz) as i64, nl as i64);
            let w = |a: &[f64], b: usize, k: i32| -> f64 {
                let i =
                    b as i64 + 6 + i64::from(nth) * nlz + i64::from(nkh - 1) + nli * i64::from(k);
                usize::try_from(i - 1)
                    .ok()
                    .and_then(|i| a.get(i))
                    .copied()
                    .unwrap_or(0.0)
            };
            let a = &self.a;
            let mut s;
            if ntp > 0 {
                if ntp < 20 {
                    s = w(a, jbase, ntp - 1);
                } else if ntp < 40 {
                    s = 0.0;
                    let d = w(a, jbase, ntp - 21);
                    if d != 0.0 {
                        s = 100.0 * (w(a, loc2, ntp - 21) - d) / d;
                    }
                } else {
                    s = 0.0;
                    let d = w(a, jbase, ntp - 41);
                    if d != 0.0 {
                        s = w(a, loc2, ntp - 41) / d;
                    }
                }
            } else if ntp > -20 {
                s = 0.0;
                if w(a, jbase, 0) != 0.0 {
                    s = w(a, jbase, -ntp - 1) / w(a, jbase, 0);
                }
            } else if ntp > -40 {
                s = 0.0;
                if w(a, loc2, 0) != 0.0 {
                    let k = -ntp - 21;
                    let r1 = w(a, jbase, k) / w(a, jbase, 0);
                    s = 100.0 * ((w(a, loc2, k) / w(a, loc2, 0)) - r1) / r1;
                }
            } else {
                s = 0.0;
                if w(a, loc2, 0) != 0.0 {
                    let k = -ntp - 41;
                    s = (w(a, loc2, k) / w(a, loc2, 0)) / (w(a, jbase, k) / w(a, jbase, 0));
                }
            }
            if nth == 0 {
                let dleth = (self.a[locngn + igu - 1] / self.a[locngn + igu - 2]).r_ln();
                s /= dleth;
            }
            self.set_y(jnow + 1, s * facty);
            self.set_y(jnow + 2, s * facty);
            if s != 0.0 {
                self.n = Some(jnow + 2);
            }
        }
    }

    /// 3400 to 1686.
    fn at_3400(
        &mut self,
        jbase: usize,
        locngn: usize,
        locngg: usize,
        ngn: i32,
        ngg: i32,
    ) -> Res<Go> {
        let nin = self.nin;
        loop {
            self.contio(nin, jbase)?;
            if self.h.mfh == 0 || self.h.mth == 0 {
                continue;
            }
            if self.h.math <= 0 {
                // `mess('plotr','desired gendf matrix not found',' ')`
                return Ok(Go::L110);
            }
            if self.h.mfh == self.mfd && self.h.mth == self.mtd {
                break;
            }
            self.stream(nin)?.tosend()?;
        }
        self.gg3d(jbase, locngn, locngg, ngn, ngg)?;
        Ok(if self.iauto.unwrap_or(0) > 0 {
            Go::L321
        } else {
            Go::L110
        })
    }

    /// `gg3d`: the axes of a group-to-group matrix plot.
    fn gg3d(&mut self, jbase: usize, locngn: usize, locngg: usize, ngn: i32, ngg: i32) -> Res<()> {
        const MAXX3: usize = 400;
        const MAXY3: usize = 400;
        const SMALL: f64 = 1.0e-12;
        let nin = self.nin;
        let a1 = |p: &Plotr, k: usize| p.a[k - 1];
        let mut ng1 = 0usize;
        let nl = nint(a1(self, jbase + 2)).max(0) as usize;
        let nz = nint(a1(self, jbase + 3)).max(0) as usize;
        let ngl = nint(a1(self, jbase + 5));
        let mut xmin = self.xleft;
        let mut xmax = self.xright;
        let ngl_u = ngl.max(0) as usize;
        if xmax == 0.0 {
            xmax = a1(self, locngn + ngl_u);
        }
        if xmin == 0.0 {
            xmin = a1(self, locngn);
        }
        let mut xstp = self.xstep;
        let mut emin = 0.0;
        let mut ymin = self.ybot;
        let mut ymax = self.ytop;
        let g16 = self.mfd == 16;
        let (locg, ngy) = if g16 { (locngg, ngg) } else { (locngn, ngn) };
        if ymax == 0.0 {
            ymax = a1(self, locg + ngy.max(0) as usize);
        }
        if ymin == 0.0 {
            ymin = a1(self, locg);
        }
        let mut ystp = self.ystep;
        let mut locn = 0usize;
        let mut zmax = -12.0f64;
        let mut ig = 1i32;
        let mut ex3 = vec![0.0f64; 200];
        let mut ey3 = vec![0.0f64; 200];
        let mut j2 = 0usize;
        let set = |v: &mut Vec<f64>, i: usize, x: f64| -> Res<()> {
            *v.get_mut(i - 1).ok_or(undefined(
                "plotr gg3d: more than 200 ex3/ey3 points (overflows upstream)",
            ))? = x;
            Ok(())
        };
        while ig < ngl {
            ng1 += 1;
            let (mut nb, mut nw) = self.listio(nin, jbase)?;
            let mut l = jbase + nw;
            while nb != 0 {
                (nb, nw) = self.moreio(nin, l, nb)?;
                l += nw;
            }
            let ng2 = nint(a1(self, jbase + 2));
            let ig2lo = nint(a1(self, jbase + 3));
            ig = nint(a1(self, jbase + 5));
            if ng1 + 2 > MAXX3 {
                return Err(perr("gg3d: too many incident groups for 3d gendf plot"));
            }
            let igu = ig.max(1) as usize;
            set(&mut ex3, ng1, a1(self, locngn + igu - 1))?;
            if ng1 == 1 {
                emin = ex3[0];
            }
            let ngm = if g16 { ngg + 1 } else { ngn + 1 }.max(0) as usize;
            if locn + 2 * ngm + 4 > self.aa.len() {
                return Err(perr("gg3d: too many data for 3d gendf plot"));
            }
            set(&mut ey3, 1, ymin)?;
            set(&mut ey3, 2, ymin)?;
            j2 = 2;
            for i2 in 1..=ngm {
                let e2 = a1(self, locg + i2 - 1);
                if e2 > ymin && e2 < ymax {
                    if j2 + 4 > MAXY3 {
                        return Err(perr("gg3d: too many secondary groups for 3d gendf plot"));
                    }
                    set(&mut ey3, j2 + 1, e2)?;
                    set(&mut ey3, j2 + 2, e2)?;
                    j2 += 2;
                }
            }
            set(&mut ey3, j2 + 1, ymax)?;
            set(&mut ey3, j2 + 2, ymax)?;
            j2 += 2;
            for i2 in 1..=j2 {
                self.aa[locn + i2 - 1] = -12.0;
                self.aa[locn + j2 + i2 - 1] = -12.0;
            }
            locn += j2;
            ng1 += 1;
            set(&mut ex3, ng1, a1(self, locngn + igu - 1))?;
            let mut k2 = 1usize;
            for j in 2..=ng2.max(0) as usize {
                let ig2 = (ig2lo + j as i32 - 2).max(1) as usize;
                let eglo = a1(self, locg + ig2 - 1);
                let eghi = a1(self, locg + ig2);
                if eghi > ymin && eglo <= ymax {
                    let mut f2 = a1(self, jbase + 6 + nl * nz * (j - 1)) / (eghi - eglo);
                    if f2 < SMALL {
                        f2 = SMALL;
                    }
                    f2 = f2.r_log10();
                    if f2 > zmax {
                        zmax = f2;
                    }
                    k2 += 1;
                    self.aa[locn + k2 - 1] = f2;
                    k2 += 1;
                    self.aa[locn + k2 - 1] = f2;
                }
            }
            locn += j2;
        }
        let igu = ig.max(0) as usize;
        ng1 += 1;
        set(&mut ex3, ng1, a1(self, locngn + igu))?;
        ng1 += 1;
        let v = a1(self, locngn + igu);
        set(&mut ex3, ng1, v + v / 1000.0)?;
        for i2 in 1..=j2 {
            let k = (locn + i2)
                .checked_sub(2 * j2)
                .ok_or(undefined("plotr gg3d: no matrix row read"))?;
            self.aa[locn + i2 - 1] = self.aa[k - 1];
            self.aa[locn + i2 + j2 - 1] = self.aa[locn + i2 - j2 - 1];
        }
        let nmax = ng1;
        let emax = ex3[ng1 - 1];
        let mut i = zmax as i32;
        if f64::from(i) < zmax {
            i += 1;
        }
        zmax = f64::from(i);
        let zmin0 = zmax - 33.0 / 10.0;
        let mut i = zmin0 as i32;
        if f64::from(i) > zmin0 {
            i -= 1;
        }
        let zmin = f64::from(i);
        for i1 in 1..=nmax {
            for i2 in 1..=j2 {
                let l = i2 + j2 * (i1 - 1);
                if self.aa[l - 1] < zmin {
                    self.aa[l - 1] = zmin;
                }
            }
        }
        self.r = Some([zmin, zmax, 1.0]);
        if self.itype == 3 || self.itype == 4 {
            xmin = emin.r_log10();
            xmax = emax.r_log10();
            xstp = 1.0;
            for e in ex3.iter_mut().take(nmax) {
                *e = e.r_log10();
            }
        }
        if xstp == 0.0 {
            let (z1, z2, major, _) = ascale(4, emin, emax);
            xmin = z1;
            xmax = z2;
            xstp = (xmax - xmin) / f64::from(major);
        }
        self.xleft = xmin;
        self.xright = xmax;
        self.xstep = xstp;
        if self.itype == 2 || self.itype == 4 {
            ymin = ymin.r_log10();
            ymax = ymax.r_log10();
            ystp = 1.0;
            for e in ey3.iter_mut().take(j2) {
                *e = e.r_log10();
            }
        }
        if ystp == 0.0 {
            let (z1, z2, major, _) = ascale(2, ymin, ymax);
            ymin = z1;
            ymax = z2;
            ystp = (ymax - ymin) / f64::from(major);
        }
        self.stream(nin)?.tosend()?;
        self.ybot = ymin;
        self.ytop = ymax;
        self.ystep = ystp;
        self.rl = Some(c60("<p>rob/e<v"));
        self.nr = Some(10);
        self.jtype = 2;
        self.yl = c60("<s>ec. energy");
        self.ny = 13;
        self.head_3d(true, true)
    }
}
