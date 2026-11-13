// SPDX-License-Identifier: AGPL-3.0-only
// Translated from NJOY2016.80 `src/plotr.f90` (git commit ac5adf5f33d893e42f2eed7fb286b0d51c7580da):
//   `subroutine plotr` l.1371-1462 (ENDF 3-d plots), `ad3d` l.1799-1993,
//   `ed3d` l.1995-2188.
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under AGPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! ENDF 3-d plots: angular distributions (`ad3d`, MF=4 and MF=6 LAW=2) and
//! energy distributions (`ed3d`, MF=5 LF=1 and MF=6 LAW=1/7).
//!
//! Both receive PLOTR's axis variables by reference and change them (the
//! default labels they set persist into later curves), as here. Two upstream
//! quirks are kept: `ed3d` writes its view card without the closing `/`
//! (`:2158`), and `ad3d` and `gg3d` write `-x3`.

use petir::mathf::RealMath;

use super::run::{c60, fe, ff, fi, perr, undefined, Go, Plotr, Res, XLABLD};

const MAXXY: usize = 200;
const MAXX3: usize = 400;

impl Plotr {
    fn back_to(&self) -> Go {
        if self.iauto.unwrap_or(0) > 0 {
            Go::L321
        } else {
            Go::L110
        }
    }

    /// 2300 to 1462.
    pub(super) fn at_2300(&mut self) -> Res<Go> {
        let nin = self.nin;
        let mut law = 0;
        let mfd = self.mfd;
        if mfd == 4 {
            self.ltt = self.h.l2h;
            let (nb, _) = self.listio(nin, 1)?;
            self.skip_pages(nin, 1, nb)?;
            self.ad3d()?;
            return Ok(self.back_to());
        }
        if mfd == 5 {
            self.tab1io(nin, 1)?;
            let lf = self.h.l2h;
            self.lf = Some(lf);
            if lf == 1 {
                self.ed3d(law)?;
                return Ok(self.back_to());
            }
            // `mess('plotr','can only plot mf5/lf1',' ')`
            self.stream(nin)?.tosend()?;
            return Ok(self.back_to());
        }
        if mfd == 7 {
            return Err(perr("3d mf7 plots not available"));
        }
        if mfd == 6 && (221..=250).contains(&self.mtd) {
            self.ed3d(law)?;
            return Ok(self.back_to());
        }
        for _ in 0..(self.nkh - 1).max(0) {
            self.tab1io(nin, 1)?;
            law = self.h.l2h;
            self.tab2io(nin, 1)?;
            let ne = self.h.n2h;
            for _ in 0..ne.max(0) {
                if law != 7 {
                    let (nb, _) = self.listio(nin, 1)?;
                    self.skip_pages(nin, 1, nb)?;
                } else {
                    self.tab2io(nin, 1)?;
                    let nmu = self.h.n2h;
                    for _ in 0..nmu.max(0) {
                        let (nb, _) = self.tab1io(nin, 1)?;
                        self.skip_pages(nin, 1, nb)?;
                    }
                }
            }
        }
        self.tab1io(nin, 1)?;
        law = self.h.l2h;
        match law {
            0 | 3 | 4 => {
                // 2380: `mess('plotr','no distribution, no plot', ...)`
                self.stream(nin)?.tosend()?;
                Ok(self.back_to())
            }
            1 | 7 => {
                self.ed3d(law)?;
                Ok(self.back_to())
            }
            2 => {
                self.ad3d()?;
                Ok(self.back_to())
            }
            _ => Err(perr("illegal mf6 law.")),
        }
    }

    /// The axis cards of a 3-d plot (`ad3d`/`ed3d`/`gg3d` share them).
    pub(super) fn head_3d(&mut self, neg_x3: bool, slash: bool) -> Res<()> {
        let mut l = format!("{}{}", fi(self.iplot, 4), fi(self.iwcol, 8));
        for v in [1.0, 1.0, self.xll, self.yll, self.ww, self.wh, self.wr] {
            l.push_str(&ff(v, 7, 2));
        }
        l.push_str("/ 3d plot");
        self.line(&l);
        let t = format!(" '{}'/", self.t1);
        self.line(&t);
        self.auto_title()?;
        let t = format!(" '{}'/", self.t2);
        self.line(&t);
        let t = format!(
            "{}{}{}/",
            fi(-self.itype, 6),
            fi(self.jtype, 6),
            fi(self.igrid, 6)
        );
        self.line(&t);
        self.axis_line(self.xstep, [self.xleft, self.xright, self.xstep]);
        let t = format!(" '{}'/", self.xl);
        self.line(&t);
        self.axis_line(self.ystep, [self.ybot, self.ytop, self.ystep]);
        let t = format!(" '{}'/", self.yl);
        self.line(&t);
        let r = self.r.ok_or(undefined(
            "plotr: a 3-d plot with no card 7 ever read (undefined upstream)",
        ))?;
        self.axis_line(r[2], r);
        let rl = self.rl.clone().ok_or(undefined(
            "plotr: a 3-d plot with no card 7a ever read (undefined upstream)",
        ))?;
        let t = format!(" '{rl}'/");
        self.line(&t);
        self.line("/");
        let v = self.view;
        let x3 = if neg_x3 { -v[3] } else { v[3] };
        let mut t = String::new();
        for w in [v[0], v[1], v[2], x3, v[4], v[5]] {
            t.push_str(&ff(w, 9, 3));
        }
        if slash {
            t.push('/');
        }
        self.line(&t);
        self.line(" 1/");
        Ok(())
    }

    /// `ad3d`: a perspective view of an angular distribution.
    fn ad3d(&mut self) -> Res<()> {
        const BIG: f64 = 1.0e8;
        const EMEV: f64 = 1.0e6;
        let nin = self.nin;
        self.tab2io(nin, 1)?;
        let ne = self.h.n2h;
        let xmin = self.ybot;
        let mut xmax = self.ytop;
        if xmax == 0.0 {
            xmax = BIG;
        }
        let mut locn = 0usize;
        let base = 1.0 / 100.0;
        let nmu = 101usize;
        let nmum = nmu + 2;
        let dmu = 2.0 / (nmu - 1) as f64;
        let mut i = 0usize;
        let mut lang = 0;
        let mut nl = 0;
        let estep = EMEV / 5.0;
        let varlim = 8.0 / 100.0;
        let mut nthind = 0;
        let mut nmax: Option<usize> = None;
        let mut ey3 = [0.0f64; 200];
        let mut intmu: Option<i32> = None;
        let ltt = self.ltt;
        for ie in 1..=ne.max(0) {
            let nw;
            if self.h.mfh != 4 || ltt != 2 {
                nw = self.listio(nin, 1)?.1;
                lang = self.h.l1h;
                nl = self.h.n1h;
            } else {
                let (mut nb, mut w) = self.tab1io(nin, 1)?;
                intmu = Some(4);
                let mut loc = w + 1;
                while nb != 0 {
                    (nb, w) = self.moreio(nin, loc, nb)?;
                    loc += w;
                }
                // `k=nw-3` below reads the last page's `nw`, as upstream.
                nw = w;
            }
            let mut k = nw - 3;
            let en = self.h.c2h;
            if en >= xmin - xmin / 1000.0
                && en <= xmax + xmax / 1000.0
                && locn + 3 * nmum <= self.aa.len()
                && i + 3 <= MAXX3
                && i < MAXXY
            {
                i += 1;
                ey3[i - 1] = en;
                self.aa[locn] = base;
                locn += 1;
                let mfh = self.h.mfh;
                for imu in 1..=nmu {
                    let amu = 1.0 - dmu * (imu - 1) as f64;
                    let mut sum;
                    if (mfh == 4 && ltt == 2) || (mfh == 6 && lang > 2) {
                        while amu < self.a[k - 1] {
                            k -= 2;
                        }
                        let im = intmu.ok_or(undefined(
                            "plotr ad3d: intmu undefined for an MF=6 tabulated law",
                        ))?;
                        sum = super::getters::terp1(
                            self.a[k - 1],
                            self.a[k],
                            self.a[k + 1],
                            self.a[k + 2],
                            amu,
                            im,
                        )?;
                    } else {
                        let mut pl = 1.0;
                        let mut pn = amu;
                        sum = 1.0 / 2.0;
                        for il in 1..=nl.max(0) {
                            sum += f64::from(2 * il + 1) * pn * self.a[6 + il as usize - 1] / 2.0;
                            let g = amu * pn;
                            let h = g - pl;
                            pl = pn;
                            pn = h + g - h / f64::from(il + 1);
                        }
                    }
                    if sum < base {
                        sum = base;
                    }
                    self.aa[locn + imu - 1] = sum;
                }
                locn += nmu;
                self.aa[locn] = base;
                locn += 1;
                nmax = Some(i);
                if i != 1 && ie != ne && ey3[i - 1] <= ey3[i - 2] + estep {
                    let mut var = 0.0;
                    for imu in 1..=nmu {
                        let d = self.aa[locn - imu - nmum - 1] - self.aa[locn - imu - 1];
                        var += d * d;
                    }
                    if var <= varlim {
                        nthind += 1;
                        locn -= nmum;
                        i -= 1;
                        nmax = Some(i);
                    }
                }
            }
        }
        let _ = nthind; // `mess('ad3d','mf4 incident energy grid thinned',...)`
        self.stream(nin)?.tosend()?;
        if self.nx == 0 {
            self.xl = c60("<c>osine");
        }
        if self.ny == 0 {
            self.yl = c60(XLABLD);
        }
        if self.nr.ok_or(undefined("plotr ad3d: nr undefined"))? == 0 {
            self.rl = Some(c60("<p>rob"));
        }
        self.head_3d(true, true)?;
        let nmax = nmax.ok_or(undefined(
            "plotr ad3d: no incident energy in range (nmax undefined)",
        ))?;
        let mut locn = 0usize;
        for e in ey3.iter().take(nmax) {
            let t = format!("{}/", fe(*e, 4, 13));
            self.line(&t);
            let t = format!("{}{}/", fe(1.0, 4, 13), fe(self.aa[locn], 4, 13));
            self.line(&t);
            locn += 1;
            for imu in 1..=nmu {
                let amu = 1.0 - dmu * (imu - 1) as f64;
                let t = format!("{}{}/", fe(amu, 4, 13), fe(self.aa[locn + imu - 1], 4, 13));
                self.line(&t);
            }
            locn += nmu;
            let t = format!("{}{}/", fe(-1.0, 4, 13), fe(self.aa[locn], 4, 13));
            self.line(&t);
            locn += 1;
            self.line(" /");
        }
        self.line(" /");
        Ok(())
    }

    /// `ed3d`: a perspective view of an energy distribution.
    fn ed3d(&mut self, law: i32) -> Res<()> {
        const NW7MAX: usize = 6000;
        const EPS: f64 = 1.0e-7;
        const BIG: f64 = 1.0e8;
        const SMALL: f64 = 1.0e-12;
        const EMEV: f64 = 1.0e6;
        const ELOW: f64 = 1.0e-5;
        let ten = 10.0f64;
        let nin = self.nin;
        if self.mfd == 6 && (221..=250).contains(&self.mtd) {
            self.tab1io(nin, 1)?;
        }
        self.tab2io(nin, 1)?;
        let ne = self.h.n2h;
        let mut i1 = 0usize;
        let base = SMALL;
        let xmin = self.ybot;
        let mut xmax = self.ytop;
        if xmax == 0.0 {
            xmax = BIG;
        }
        let mut ymin = self.xleft;
        let mut ymax = self.xright;
        if self.h.mth < 221 || self.h.mth > 250 {
            if ymax == 0.0 {
                ymax = 20.0 * EMEV;
            }
            if ymin == 0.0 {
                ymin = 1.0;
            }
        } else {
            if ymax == 0.0 {
                ymax = 10.0;
            }
            if ymin == 0.0 {
                ymin = EPS;
            }
        }
        let logy = self.itype == 2 || self.itype == 4;
        if logy {
            ymin = ymin.r_log10();
            ymax = ymax.r_log10();
        }
        let ne2 = 198usize;
        let ne2m = ne2 + 2;
        let off = if law == 7 { NW7MAX } else { 0 };
        let mut locn = off;
        let mut zmax = SMALL;
        let mut ex3 = [0.0f64; 200];
        let mut nmax: Option<usize> = None;
        let maxaa = self.aa.len();
        let e2_at = |i2: usize| {
            let mut e2 = ymin + (ymax - ymin) * (i2 as f64 - 1.0) / (ne2 - 1) as f64;
            if logy {
                e2 = ten.r_powf(e2);
            }
            if e2 == 0.0 {
                e2 = ELOW;
            }
            e2
        };
        for _ie in 0..ne.max(0) {
            self.ed3d_get(0.0, law, NW7MAX)?;
            let ei = self.a[1];
            if i1 < MAXXY
                && ei >= xmin - xmin / 1000.0
                && ei <= xmax + xmax / 1000.0
                && locn + 3 * ne2m <= maxaa
                && i1 + 3 <= MAXX3
            {
                i1 += 1;
                ex3[i1 - 1] = ei;
                self.aa[locn] = base;
                locn += 1;
                for i2 in 1..=ne2 {
                    let mut f2 = self.ed3d_get(e2_at(i2), law, NW7MAX)?;
                    if f2 < base {
                        f2 = base;
                    }
                    if f2 > zmax {
                        zmax = f2;
                    }
                    self.aa[locn + i2 - 1] = f2;
                }
                locn += ne2;
                self.aa[locn] = base;
                locn += 1;
                nmax = Some(i1);
            }
            self.ed3d_get(BIG, law, NW7MAX)?;
        }
        self.stream(nin)?.tosend()?;
        zmax = zmax.r_log10();
        let zmmm = 33.0 / 10.0;
        let zmin0 = zmax - zmmm;
        let mut i = zmin0 as i32;
        if f64::from(i as f32) > zmin0 {
            i -= 1;
        }
        let zmin = ten.r_powi(i);
        if self.ny == 0 {
            self.yl = c60(XLABLD);
        }
        if self.nx == 0 {
            self.xl = c60("<s>ec. energy");
        }
        if self.nr.ok_or(undefined("plotr ed3d: nr undefined"))? == 0 {
            self.rl = Some(c60("<p>rob/e<v"));
        }
        self.head_3d(false, false)?;
        let nmax = nmax.ok_or(undefined(
            "plotr ed3d: no incident energy in range (nmax undefined)",
        ))?;
        let mut locn = off;
        for e in ex3.iter().take(nmax) {
            let t = format!("{}/", fe(*e, 4, 13));
            self.line(&t);
            let (mut ii1, mut ii2) = (0usize, 0usize);
            for i2 in 1..=ne2m {
                let v = self.aa[locn + i2 - 1];
                if ii1 == 0 && v >= zmin {
                    ii1 = i2;
                }
                if v >= zmin {
                    ii2 = i2;
                }
                if v < zmin {
                    self.aa[locn + i2 - 1] = zmin;
                }
            }
            if ii1 == 0 {
                ii1 = 2;
            }
            if ii2 == 0 {
                ii2 = 2;
            }
            if ii1 > 1 {
                ii1 -= 1;
            }
            if ii2 < ne2m {
                ii2 += 1;
            }
            for i2 in ii1..=ii2 {
                let mut e2 = ymin + (ymax - ymin) * (i2 as f64 - 2.0) / (ne2 - 1) as f64;
                if i2 == 1 {
                    e2 = ymin;
                }
                if i2 == ne2m {
                    e2 = ymax;
                }
                if logy {
                    e2 = ten.r_powf(e2);
                }
                if e2 == 0.0 {
                    e2 = ELOW;
                }
                let t = format!("{}{}/", fe(e2, 4, 13), fe(self.aa[locn + i2 - 1], 4, 13));
                self.line(&t);
            }
            locn += ne2m;
            self.line(" /");
        }
        self.line(" /");
        Ok(())
    }

    /// `ed3d`'s getter: `gety1` for MF=5, `gety6(1, ..., a, 2, law, aa, nw7max)`
    /// for MF=6. Returns `f2`.
    fn ed3d_get(&mut self, e2: f64, law: i32, nw7max: usize) -> Res<f64> {
        if self.mfd == 5 {
            return Ok(self.gety1(e2)?.0);
        }
        if self.mfd == 6 {
            self.gety6(1, e2, 2, law, 0, nw7max)?;
            return Ok(self.yf);
        }
        Err(undefined(
            "plotr ed3d: f2 undefined for a file other than 5 or 6",
        ))
    }
}
