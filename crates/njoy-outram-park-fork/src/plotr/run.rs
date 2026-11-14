// SPDX-License-Identifier: AGPL-3.0-only
// Translated from NJOY2016.80 `src/plotr.f90` (git commit ac5adf5f33d893e42f2eed7fb286b0d51c7580da):
//   `subroutine plotr`, l.362-1369 and l.1688-1797.
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under AGPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! `subroutine plotr`: the curve loop, the ENDF/PENDF 2-d path, input data and
//! the 2-d plot commands.
//!
//! Upstream is one subroutine whose locals live for the whole run, so a value
//! one curve sets is what the next curve reads if it does not set it again
//! (`iauto`, `jnoth`, `lf`, `t1`, `itype`, the axis labels, ...). They are the
//! fields of [`Plotr`]. The `go to` labels are [`Go`]; each method returns the
//! label upstream jumps to next. A local upstream can read before any curve has
//! set it is an `Option`, and such a read is refused (undefined upstream).
//! `x`, `y`, `b`, `dxm`, `dxp`, `dym`, `dyp` are `equivalence`d into `aa`
//! (`plotr.f90:392-394`), and so they are here.

use petir::mathf::RealMath;

use crate::deck::tapes::TapeUnits;
use crate::NjoyError;

use super::cards::{CurveCards, PlotrCards, StyleCards};
use super::getters::ascale;
use super::stream::NPAGE;

pub use super::state::Plotr;
pub(super) use super::state::{c60, fe, ff, fi, perr, undefined};

pub(super) type Res<T> = Result<T, NjoyError>;

/// `mmax` (same in plotr and viewr): points per 2-d curve.
pub(super) const MMAX: usize = 20000;
/// `nwamax`: the work array `a`.
pub(super) const NWAMAX: usize = 45000;
/// `maxaa`: the work array `aa`.
pub(super) const MAXAA: usize = 200000;
const EPS: f64 = 1.0e-7;
const DELTA: f64 = 1.26;
const ETOP: f64 = 1.0e10;
const TINY: f64 = 1.0e-33;
const ELOW: f64 = 1.0e-5;
const BIG: f64 = 9.0e9;
const SMALL: f64 = 1.0e-12;
/// `ten**(-15)` (`plotr.f90:1242`).
const TEST15: f64 = 1.0e-15;
pub(super) const XLABLD: &str = "<e>nergy (e<v)";
pub(super) const YLABLD: &str = "<c>ross section (barns)";

/// A label of `subroutine plotr`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Go {
    /// 110: the next curve's card 2.
    L110,
    /// 320: `findf` the section.
    L320,
    /// 321: read the section's HEAD.
    L321,
    /// 1409: the 2-d curve's first point.
    L1409,
    /// 1410: MF=7.
    L1410,
    /// 2300: the ENDF 3-d plots.
    L2300,
    /// 400: GENDF data.
    L400,
    /// 610: write the 2-d plot.
    L610,
    /// 700: finished.
    L700,
}

impl Plotr {
    /// Run PLOTR on its cards; the plot commands go on unit `nplt`.
    ///
    /// # Errors
    /// Upstream's `error` calls; an undefined read (see the module doc); tape errors.
    pub fn run(cards: &PlotrCards, units: &mut TapeUnits) -> Res<()> {
        let mut p = Plotr::default();
        p.line(&format!(
            "{}{}{}{} /",
            fi(cards.lori, 2),
            fi(cards.istyle, 2),
            ff(cards.size, 8, 3),
            fi(cards.ipcol, 8)
        ));
        let mut curves = cards.curves.iter();
        let mut go = Go::L110;
        loop {
            go = match go {
                Go::L110 => {
                    let c = curves
                        .next()
                        .ok_or_else(|| perr("end of cards before iplot = 99"))?;
                    p.card_curve(c, units)?
                }
                Go::L320 => p.at_320()?,
                Go::L321 => p.at_321()?,
                Go::L1409 => p.at_1409()?,
                Go::L1410 => p.at_1410()?,
                Go::L2300 => p.at_2300()?,
                Go::L400 => p.at_400()?,
                Go::L610 => p.at_610()?,
                Go::L700 => break,
            };
        }
        p.line(" 99/");
        units.put_text(cards.nplt, std::mem::take(&mut p.out));
        units.closz(p.nin)?;
        Ok(())
    }

    /// 110 to 752: card 2 and what follows it for one curve.
    fn card_curve(&mut self, c: &CurveCards, units: &mut TapeUnits) -> Res<Go> {
        self.iplot = c.c2[0] as i32;
        self.iwcol = c.c2[1] as i32;
        self.factx = c.c2[2];
        self.facty = c.c2[3];
        self.xll = c.c2[4];
        self.yll = c.c2[5];
        self.ww = c.c2[6];
        self.wh = c.c2[7];
        self.wr = c.c2[8];
        if self.iplot == 99 {
            return Ok(Go::L700);
        }
        self.ierrb = 0;
        if let Some(ax) = &c.axes {
            self.t1 = ax.t1.clone();
            self.t2 = ax.t2.clone();
            self.itype = ax.itype;
            self.jtype = ax.jtype;
            self.igrid = ax.igrid;
            self.ileg = ax.ileg;
            self.xtag = ax.xtag;
            self.ytag = ax.ytag;
            self.i3d = 0;
            if self.itype <= 0 {
                self.i3d = 1;
                self.itype = -self.itype;
            }
            let [xlf, xh, mut xstep] = ax.x;
            let nz = if xlf == 0.0 && xh == 0.0 {
                0
            } else if xstep != 0.0 {
                3
            } else {
                2
            };
            if self.itype > 2 && nz >= 2 {
                xstep = 1.0;
            }
            if nz > 0 && xstep == 0.0 {
                return Err(perr("error in axis input"));
            }
            self.xstep = xstep;
            self.xleft = xlf;
            self.xright = xh;
            self.xl = ax.xl.clone();
            self.nx = ax.nx;
            let [yb, yt, yystep] = ax.y;
            self.iyauto = 0;
            self.yb = yb;
            self.yt = yt;
            self.yystep = yystep;
            self.ystep = yystep;
            let nz = if yb == 0.0 && yt == 0.0 {
                0
            } else if self.ystep != 0.0 {
                3
            } else {
                2
            };
            if (self.itype == 2 || self.itype == 4) && nz >= 2 {
                self.ystep = 1.0;
            }
            if nz > 0 && self.ystep == 0.0 {
                return Err(perr("error in axis input"));
            }
            self.ybot = yb;
            self.ytop = yt;
            self.yl = ax.yl.clone();
            self.ny = ax.ny;
            if let Some((rv, rl, nr)) = &ax.r {
                if rv[1] < rv[0] {
                    return Err(perr("error in axis input"));
                }
                self.r = Some(*rv);
                self.rl = Some(rl.clone());
                self.nr = Some(*nr as i32);
            }
        }
        let s = c.src;
        self.iverf = s.iverf;
        self.nin = s.nin;
        self.matd = s.matd;
        self.mfd = s.mfd;
        self.mtd = s.mtd;
        self.temper = s.temper;
        self.nth = s.nth;
        self.ntp = s.ntp;
        self.nkh = s.nkh;
        self.nin2 = 0;
        if self.iverf > 0 {
            if self.nin != self.ninl {
                units.closz(self.ninl)?;
                self.open_in(units, self.nin)?;
            }
            self.ninl = self.nin;
            self.iauto = Some(i32::from(self.mtd == 0));
            self.ipass = 0;
            if let Some(s2) = c.src2 {
                self.iverf2 = s2.iverf;
                self.nin2 = s2.nin;
                self.matd2 = s2.matd;
                self.mfd2 = s2.mfd;
                self.mtd2 = s2.mtd;
                self.temper2 = s2.temper;
                self.open_in(units, self.nin2)?;
                if self.ntp == 3 && self.mtd == 0 {
                    self.mtd2 = self.mtd;
                }
            }
        }
        match &c.style {
            StyleCards::TwoD { c9, aleg, tag } => {
                self.c9 = *c9;
                if let Some(t) = aleg {
                    self.aleg = t.clone();
                }
                if let Some(t) = tag {
                    self.xtag = t[0];
                    self.ytag = t[1];
                    self.xpoint = t[2];
                }
            }
            StyleCards::ThreeD(v) => self.view = *v,
        }
        if self.iverf == 0 {
            let (_, pts) = c
                .data
                .as_ref()
                .ok_or_else(|| perr("input data cards missing"))?;
            return self.at_500(pts);
        }
        if self.iverf == 1 {
            return Ok(Go::L400);
        }
        self.locate()?;
        Ok(Go::L320)
    }

    /// 757-809: position each tape at its material and temperature.
    fn locate(&mut self) -> Res<()> {
        let (nin, nin2) = (self.nin, self.nin2);
        self.stream(nin)?.rewind();
        {
            let (s, h, _, _) = self.parts(nin)?;
            s.tpidio(h)?;
        }
        if nin2 != 0 {
            self.stream(nin2)?.rewind();
            let (s, h, _, _) = self.parts(nin2)?;
            s.tpidio(h)?;
        }
        self.elt = self.xleft / self.factx;
        self.eht = self.xright / self.factx;
        let (matd, mfd, iverf, temper) = (self.matd, self.mfd, self.iverf, self.temper);
        self.find_mat(
            nin,
            matd,
            mfd,
            iverf,
            temper,
            "1desired mat and temp not found ",
        )?;
        if nin2 != 0 {
            let (m2, f2, v2, t2) = (self.matd2, self.mfd2, self.iverf2, self.temper2);
            self.find_mat(nin2, m2, f2, v2, t2, "desired mat2 and temp2 not found ")?;
        }
        Ok(())
    }

    fn find_mat(
        &mut self,
        unit: i32,
        matd: i32,
        mfd: i32,
        iverf: i32,
        temper: f64,
        msg: &str,
    ) -> Res<()> {
        loop {
            self.contio(unit, 1)?;
            if self.h.math < 0 {
                return Err(perr(&format!("{msg}{}{}", fi(matd, 4), ff(temper, 10, 1))));
            }
            let mut idone = false;
            if self.h.math == matd {
                if mfd == 7 {
                    idone = true;
                } else {
                    if iverf >= 5 {
                        self.contio(unit, 1)?;
                    }
                    if iverf >= 6 {
                        self.contio(unit, 1)?;
                    }
                    self.contio(unit, 1)?;
                    let tem = self.a[0];
                    if (tem - temper).abs() <= temper / 1000.0 {
                        idone = true;
                    }
                }
            }
            if idone {
                return Ok(());
            }
            self.stream(unit)?.tomend()?;
        }
    }

    fn at_320(&mut self) -> Res<Go> {
        let (matd, mfd, mtd, nin) = (self.matd, self.mfd, self.mtd, self.nin);
        self.stream(nin)?.findf(matd, mfd, mtd, nin)?;
        if self.nin2 != 0 {
            let (m2, f2, t2, n2) = (self.matd2, self.mfd2, self.mtd2, self.nin2);
            self.stream(n2)?.findf(m2, f2, t2, n2)?;
        }
        Ok(Go::L321)
    }

    /// 321 to 947.
    fn at_321(&mut self) -> Res<Go> {
        let (nin, nin2) = (self.nin, self.nin2);
        self.contio(nin, 1)?;
        let mut mthx = self.h.mth;
        if nin2 != 0 {
            self.contio(nin2, 1)?;
            self.mtd2 = self.h.mth;
            while mthx < self.mtd2 && self.h.mfh > 0 {
                self.stream(nin)?.tosend()?;
                self.contio(nin, 1)?;
                mthx = self.h.mth;
            }
            while self.mtd2 < mthx && self.h.mfh > 0 {
                self.stream(nin2)?.tosend()?;
                self.contio(nin2, 1)?;
                self.mtd2 = self.h.mth;
            }
        }
        let iauto = self.iauto.unwrap_or(0);
        if self.h.mfh == 0 && iauto == 1 {
            return Ok(Go::L110);
        }
        self.mtd = mthx;
        if nin2 != 0 {
            self.mtd2 = self.h.mth;
        }
        if iauto > 0 && self.ipass == 0 {
            self.eht = 0.0;
        }
        if iauto > 0 {
            self.elt = 0.0;
            self.xleft = 0.0;
            self.xright = 0.0;
            self.xstep = 0.0;
            self.ybot = 0.0;
            self.ytop = 0.0;
            self.ystep = 0.0;
            self.iyauto = 0;
            if self.ntp == 3 && self.yb != 0.0 {
                self.ybot = self.yb;
            }
            if self.ntp == 3 && self.yt != 0.0 {
                self.ytop = self.yt;
            }
            if self.ntp == 3 && self.yystep != 0.0 {
                self.ystep = self.yystep;
            }
            if self.ybot != 0.0 && self.ytop != 0.0 {
                self.iyauto = 1;
            }
        }
        self.ik = 1;
        self.mmf = 3;
        if self.i3d == 1 {
            return Ok(Go::L2300);
        }
        match self.mfd {
            3 => return Ok(Go::L1409),
            4 => {
                self.mmf = 4;
                return Ok(Go::L1409);
            }
            7 => return Ok(Go::L1410),
            _ => {}
        }
        let mfd = self.mfd;
        if mfd == 5 || mfd == 15 || mfd == 6 {
            let lttt = self.h.l2h;
            self.mmf = mfd;
            loop {
                if lttt != 5 {
                    self.tab1io(nin, 1)?;
                    let lf = self.h.l2h;
                    self.lf = Some(lf);
                    if mfd != 6 && lf != 1 {
                        return Err(perr("lf=1 only for mf5 or mf15."));
                    }
                    if mfd == 6 && lf != 1 && lf != 7 {
                        return Err(perr("lf=1 or 7 only for file6."));
                    }
                    if lf == 7 {
                        self.mmf = 1;
                    }
                }
                self.tab2io(nin, 1)?;
                self.lep = Some(if lttt == 5 { 2 } else { self.h.l2h });
                if self.ik == self.nkh {
                    break;
                }
                let net = self.h.n2h;
                let mut iet = 1;
                while iet < net {
                    self.skip_one_distribution()?;
                    iet += 1;
                }
                self.ik += 1;
            }
        }
        let mut ith = 1;
        while ith != self.nth {
            self.skip_one_distribution()?;
            ith += 1;
        }
        if self.lf_now()? == 7 {
            self.tab2io(nin, 1)?;
            let mut imu = 1;
            while imu != self.ntp {
                let (nb, _) = self.tab1io(nin, 1)?;
                self.skip_pages(nin, 1, nb)?;
                imu += 1;
            }
        }
        Ok(Go::L1409)
    }

    fn lf_now(&self) -> Res<i32> {
        self.lf.ok_or(undefined(
            "plotr: lf read before any MF=5/6 curve set it (undefined upstream)",
        ))
    }

    /// One incident energy's record(s) of an MF=5/15/6 section, skipped.
    fn skip_one_distribution(&mut self) -> Res<()> {
        let nin = self.nin;
        if self.lf_now()? != 7 {
            let (nb, _) = if self.mfd != 6 {
                self.tab1io(nin, 1)?
            } else {
                self.listio(nin, 1)?
            };
            self.skip_pages(nin, 1, nb)?;
        } else {
            self.tab2io(nin, 1)?;
            let nmu = self.h.n2h;
            for _ in 0..nmu {
                let (nb, _) = self.tab1io(nin, 1)?;
                self.skip_pages(nin, 1, nb)?;
            }
        }
        Ok(())
    }

    /// One step of the getter `mmf` selects at `enow` (`yf`, `enext`, `idis`
    /// updated as each upstream branch updates them). `first`: the 1409 call
    /// (`enow = 0`), which only the `nin2` branch treats differently.
    fn step(&mut self, first: bool, ratio: bool) -> Res<()> {
        let enow = self.enow;
        match self.mmf {
            4 => {
                let (y, xn) = self.gety4(enow)?;
                self.yf = y;
                self.enext = xn;
            }
            6 => {
                let lep = self.lep.ok_or(undefined("plotr: lep undefined"))?;
                let lf = self.lf_now()?;
                let (xn, idis) =
                    self.gety6(self.ntp.max(0) as usize, enow, lep, lf, 2 * MMAX, 1)?;
                self.enext = xn;
                self.idis = idis;
            }
            7 => {
                let l2 = self.l2.ok_or(undefined("plotr: l2 undefined"))?;
                let (mut ip, mut ir) = (2usize, 1usize);
                let x = if first { 0.0 } else { enow };
                let (y, xn, idis) =
                    crate::errorr::weight::terpa(&self.a[l2 - 1..], x, &mut ip, &mut ir)?;
                self.yf = y;
                self.enext = xn;
                self.idis = idis;
            }
            _ if self.nin2 != 0 => {
                let (nin, nin2) = (self.nin, self.nin2);
                let loc2 = 50 + NPAGE;
                if first {
                    self.enext = BIG;
                    self.gz.clear();
                    let xn = self.getz_init(nin, 1)?;
                    if xn < self.enext {
                        self.enext = xn;
                    }
                    let xn = self.getz_init(nin2, loc2)?;
                    self.loc2 = Some(loc2);
                    self.nnn = super::stream::nint(self.a[loc2 + 4]);
                    if xn < self.enext {
                        self.enext = xn;
                    }
                    return Ok(());
                }
                if !ratio {
                    // 1148: the frame search walks the second tape only.
                    let (_, xn, idis) = self.gz.get(enow, nin2)?;
                    self.enext = xn;
                    self.idis = idis;
                    return Ok(());
                }
                self.enext = BIG;
                let (zz, xn, _) = self.gz.get(enow, nin)?;
                if xn < self.enext {
                    self.enext = xn;
                }
                let (z2, xn, idis) = self.gz.get(enow, nin2)?;
                self.idis = idis;
                if xn < self.enext {
                    self.enext = xn;
                }
                if self.ntp == 2 {
                    self.yf = 100.0 * (z2 - zz) + SMALL;
                    if zz != 0.0 {
                        self.yf /= zz;
                    }
                } else if self.ntp == 3 {
                    self.yf = z2;
                    if zz != 0.0 {
                        self.yf /= zz;
                    }
                    if z2 == 0.0 && zz == 0.0 {
                        self.yf = 1.0;
                    }
                }
            }
            _ => {
                let (y, xn, idis) = self.gety1(enow)?;
                self.yf = y;
                self.enext = xn;
                self.idis = idis;
                if first {
                    self.nnn = super::stream::nint(self.a[5]);
                }
            }
        }
        Ok(())
    }

    /// 1409 to 1369: the first point, the frame, the walk with its thinning.
    fn at_1409(&mut self) -> Res<Go> {
        self.nnn = 0;
        self.enow = 0.0;
        self.step(true, true)?;
        if self.mmf == 6 {
            self.enext -= self.enext / 10000.0;
        }
        if self.enext < self.elt {
            self.enext = self.elt;
        }
        if self.enext == 0.0 {
            self.enext = ELOW;
        }
        let jnoth = i32::from(self.enext < 1.0e3);
        self.jnoth = Some(jnoth);
        self.ipass += 1;
        self.ee1 = 0.0;
        self.ee2 = 0.0;
        if self.eht <= 0.0 {
            // 1133-1163: find the last point in the frame.
            let mut iii = 0i32;
            loop {
                self.enow = self.enext;
                iii += 1;
                self.step(false, false)?;
                if self.mmf != 4 && self.mmf != 6 && self.mmf != 7 && jnoth == 1 && self.nnn > 3000
                {
                    if iii == 300 {
                        self.ee1 = self.enow;
                    }
                    if iii == self.nnn - 500 {
                        self.ee2 = self.enow;
                    }
                }
                self.eht = self.enow;
                if self.enext >= ETOP - ETOP / 1000.0 {
                    break;
                }
            }
            return Ok(Go::L320);
        }
        self.at_335()
    }

    /// 335 to 1369.
    fn at_335(&mut self) -> Res<Go> {
        let ten = 10.0f64;
        self.enow = self.enext;
        if self.ee1 > TINY {
            let jj = self.ee1.r_log10() as i32;
            self.ee1 = ten.r_powi(jj);
        }
        if self.ee2 > TINY {
            let jj = self.ee2.r_log10() as i32;
            self.ee2 = ten.r_powi(jj);
        }
        let jnoth = self.jnoth.unwrap_or(0);
        if self.iauto.unwrap_or(0) > 0 {
            self.itype = 1;
            if jnoth == 1 && self.ipass > 2 {
                self.itype = 4;
            }
            if self.ipass > 2 && self.ee1 == 0.0 {
                self.ipass = 5;
            }
            if jnoth == 1 && self.ee1 > 0.0 && self.ipass == 4 {
                self.elt = self.ee1;
                self.eht = self.ee1 * 100.0;
                if self.eht > self.ee2 {
                    self.eht = self.ee2;
                }
                self.xleft = self.elt;
                self.xright = self.eht;
                self.xstep = 1.0;
                if self.eht == self.ee2 {
                    self.ipass = 5;
                }
            }
            if jnoth == 1 && self.ee1 > 0.0 && self.ipass == 5 {
                self.elt = self.ee1 * 100.0;
                self.eht = self.ee2;
                self.xleft = self.elt;
                self.xright = self.eht;
                self.xstep = 1.0;
            }
            if self.itype == 4 && self.ntp == 3 && self.iyauto != 0 {
                self.itype = 3;
            }
        }
        if self.elt == 0.0 {
            self.elt = self.enext;
        }
        if self.elt > self.enow {
            self.enow = self.elt;
        }
        let (eht, elt) = (self.eht, self.elt);
        let m500 = (MMAX - 500) as f64;
        let thin = if self.itype != 3 && self.itype != 4 {
            (eht - elt) / m500
        } else {
            ten.r_powf((eht / elt).r_log10() / m500)
        };
        self.step(false, true)?;
        let itype = self.itype;
        let mut itypx = itype;
        if self.iplot < -1 {
            if self.jtype == 1 {
                if itype == 2 {
                    itypx = 1;
                }
                if itype == 4 {
                    itypx = 3;
                }
            } else {
                if itype == 1 {
                    itypx = 2;
                }
                if itype == 3 {
                    itypx = 4;
                }
            }
        }
        if itypx == 2 || itypx == 4 {
            if self.yf <= TEST15 {
                self.yf = TEST15;
            }
            if self.enext > DELTA * self.enow {
                self.enext = DELTA * self.enow;
            }
        }
        let mut n = 1usize;
        self.set_x(n, self.enow * self.factx);
        self.set_y(n, self.yf * self.facty);
        let mut elast = self.enow;
        let mut nlast = n;
        let test = self.enext - self.enext / 10000.0;
        if self.idis != 0 && test > self.enow {
            self.enext = test;
        }
        // 360
        'walk: loop {
            if self.enext > self.eht {
                if (self.enow - self.eht).abs() <= EPS {
                    break 'walk;
                }
                if self.enext > self.eht && self.enow < self.eht {
                    self.enext = self.eht;
                }
                if self.enext != self.eht {
                    break 'walk;
                }
            }
            // 370
            loop {
                self.enow = self.enext;
                self.step(false, true)?;
                let test = ETOP - ETOP / 100.0;
                if self.enext > test && self.yf > 0.0 {
                    self.enext = self.enow + self.enow / 10000.0;
                }
                if self.enext > test && self.eht > self.enext {
                    self.eht = self.enow;
                }
                let test = self.enow + self.enow / 1000.0;
                if self.idis != 0 && self.enext > test {
                    self.enext -= self.enext / 10000.0;
                }
                let yn = self.y(n);
                let take;
                if itypx == 3 || itypx == 4 {
                    // 375
                    take = if self.idis > 0 {
                        true
                    } else if itypx == 3
                        && ((self.yf == 0.0 && yn > 0.0) || (self.yf > 0.0 && yn == 0.0))
                    {
                        true
                    } else if itypx != 3 && self.clamp_yf(yn) {
                        true
                    } else if self.enext >= thin * elast {
                        true
                    } else {
                        self.enow >= thin * elast
                    };
                } else {
                    take = if self.idis > 0 {
                        true
                    } else if itypx == 1
                        && ((self.yf == 0.0 && yn > 0.0) || (self.yf > 0.0 && yn == 0.0))
                    {
                        true
                    } else if itypx != 1 && self.clamp_yf(yn) {
                        true
                    } else if self.enext >= self.enow + thin {
                        true
                    } else {
                        self.enow >= elast + thin
                    };
                }
                if take {
                    break;
                }
            }
            // 380
            n += 1;
            if n > MMAX {
                return Err(perr("storage exceeded1"));
            }
            self.set_x(n, self.enow * self.factx);
            self.set_y(n, self.yf * self.facty);
            if self.enext > DELTA * self.enow {
                self.enext = DELTA * self.enow;
            }
            if itypx != 3 && itypx != 4 {
                if self.enext < self.enow + thin {
                    self.enext = self.enow + thin;
                }
            } else if DELTA < thin {
                self.enext = thin * self.enow;
            }
            elast = self.enow;
            if self.yf != 0.0 && itypx != 2 && itypx != 4 {
                nlast = n;
            }
            if self.yf != TEST15 && itype != 1 && itype != 3 {
                nlast = n;
            }
        }
        // 390
        if nlast < n {
            n = nlast + 1;
        }
        let nin = self.nin;
        self.stream(nin)?.tosend()?;
        if self.nin2 != 0 {
            let nin2 = self.nin2;
            self.stream(nin2)?.tosend()?;
        }
        if self.iauto.unwrap_or(0) != 0
            && self.itype <= 2
            && self.iyauto == 0
            && self.x(1) / self.factx <= 1.0
        {
            if n < 2 {
                return Err(undefined(
                    "plotr: automatic scale of a one-point curve reads x(0) (undefined upstream)",
                ));
            }
            let etmax = self.x(n - 1);
            let (mut st1, mut st2, mut st3) = (0.0f64, 0.0f64, 0.0f64);
            let st4 = self.y(n - 1);
            for i in 1..=n {
                let (xi, yi) = (self.x(i), self.y(i));
                if xi > etmax / 5.0 && yi > st1 {
                    st1 = yi;
                }
                if xi > etmax / 3.0 && yi > st2 {
                    st2 = yi;
                }
                if xi > etmax / 2.0 && yi > st3 {
                    st3 = yi;
                }
            }
            let mut stmax = st1 + st1 / 10.0;
            if stmax > 50.0 * st4 {
                stmax = st2 + st2 / 10.0;
            }
            if stmax < st3 {
                stmax = st3;
            }
            let (_, stmax, major, _) = ascale(4, 0.0, stmax);
            self.ybot = 0.0;
            self.ytop = stmax;
            self.ystep = stmax / f64::from(major);
        }
        self.n = Some(n);
        Ok(Go::L610)
    }

    /// The log-axis `yf` floor and its zero-crossing test (`plotr.f90:1300-1303`).
    fn clamp_yf(&mut self, yn: f64) -> bool {
        if self.yf <= TEST15 {
            self.yf = TEST15;
        }
        (self.yf == TEST15 && yn > TEST15) || (self.yf > TEST15 && yn == TEST15)
    }
}
