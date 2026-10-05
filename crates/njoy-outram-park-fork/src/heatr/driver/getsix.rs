// Ported from NJOY2016 `src/heatr.f90` (`getsix` 3073-3436, `tabsq6`
// 4135-4213, `hgam102` 5046-5070).
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under GPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! `getsix`: the mean energy and damage energy of one MF=6 subsection at one
//! incident energy, for the emitted particle (`irec = 0`) or for the recoil
//! that one emission of it leaves (`irec > 0`). `tabsq6`: the photon-recoil
//! energy and damage of MF=6 capture photons. `hgam102`: the relativistic
//! discrete capture gamma of MF=6/MT=102 (ENDF/B-VII H-1).
//!
//! Upstream's laws: 1 in the lab (trapezoid in energy, Gauss-Legendre in
//! angle for a recoil), 1 and 6 in the CM (adaptive integration over the lab
//! energy through `h6cm`), 2 and 3 (two-body discrete), 7 (lab angle-energy).
//! The law-7 recoil path keeps upstream's `en = yy*xx` taken before `xx` is
//! replaced by the recoil energy.

use super::flat::{legndr1, nint, terp1, terpa0, QP4, QP64, QW4, QW64};
use super::state::Heatr;
use crate::NjoyError;

impl Heatr {
    /// `getsix(e, ebar, dame, c, law, lang, lep, irec)`. `c` is the record of
    /// one incident energy, flat. Returns `(ebar, dame)`; `lang` is in/out
    /// (law 2 and 3 and 6 set it).
    pub(super) fn getsix(
        &mut self,
        e: f64,
        c: &[f64],
        law: i32,
        lang: &mut i32,
        lep: i32,
        irec: usize,
    ) -> Result<(f64, f64), NjoyError> {
        const TOL: f64 = 0.02;
        const SMALL: f64 = 1.0e-8;
        const EMAX: f64 = 1.0e10;
        const IMAX: usize = 10;
        let at = |k: usize| c.get(k - 1).copied().unwrap_or(0.0);
        let mut zp = (self.zap / 1000.0).trunc();
        let zt = (self.zat / 1000.0).trunc();
        if irec > 0 {
            zp = zt - zp;
        }
        let mut ap = self.awp;
        if irec > 0 {
            ap = self.awrt + 1.0 - self.awp;
        }
        let at_ = self.awrt;
        let (awp, awrt) = (self.awp, self.awrt);
        let mut ebar;
        let mut dame;

        let lab_law1 = (law == 1 && self.lct == 1)
            || (law == 1 && self.lct == 3 && self.zap > 2004.0)
            || (law == 1 && self.zap == 0.0);
        if law == 7 {
            // 510: law 7.
            ebar = 0.0;
            dame = 0.0;
            let (mut dl, mut hl, mut ul) = (0.0, 0.0, 0.0);
            let nmu = nint(at(6)) as usize;
            let mut l = 7 + 2 * nint(at(5)) as usize;
            let (mut xl, mut el, mut fl) = (0.0, 0.0, 0.0);
            for imu in 1..=nmu {
                let u = at(l + 1);
                let nr = nint(at(l + 4)) as usize;
                let np = nint(at(l + 5)) as usize;
                let iint = nint(at(l + 7));
                let next = l + 6 + 2 * nr + 2 * np;
                let ibase = l + 5 + 2 * nr;
                let (mut h, mut d) = (0.0, 0.0);
                for i in 1..=np {
                    let mut xx = at(ibase + 2 * i - 1);
                    let yy = at(ibase + 2 * i);
                    let en = yy * xx;
                    if irec > 0 {
                        xx = (e - 2.0 * (e * awp * xx).sqrt() * u + awp * xx) / (awrt + 1.0 - awp);
                    }
                    if i > 1 {
                        if iint == 1 {
                            h += (xx - xl) * el;
                        } else {
                            h += (xx - xl) * (en + el) / 2.0;
                        }
                    }
                    let f = yy * self.df(xx, zp, ap, zt, at_);
                    if i > 1 {
                        if iint == 1 {
                            d += (xx - xl) * fl;
                        } else {
                            d += (xx - xl) * (f + fl) / 2.0;
                        }
                    }
                    xl = xx;
                    el = en;
                    fl = f;
                }
                l = next;
                if imu > 1 {
                    ebar += (u - ul) * (h + hl) / 2.0;
                    dame += (u - ul) * (d + dl) / 2.0;
                }
                ul = u;
                hl = h;
                dl = d;
            }
            return Ok((ebar, dame));
        }
        if lab_law1 {
            // 400: law 1 in the lab.
            let nd = nint(at(3)) as usize;
            let na = nint(at(4)) as usize;
            let nep = nint(at(6)) as usize;
            let ncyc = na + 2;
            let (mut h, mut d) = (0.0, 0.0);
            let (mut xl, mut yl, mut el, mut fl) = (0.0, 0.0, 0.0, 0.0);
            for i in 1..=nep {
                let l = 7 + ncyc * (i - 1);
                let xx = at(l).abs();
                let yy = at(l + 1);
                let mut x2 = xx;
                let (mut en, mut f, f2);
                if irec == 0 {
                    en = yy * xx;
                    let zpp = (self.zap / 1000.0).trunc();
                    let ztt = (self.zat / 1000.0).trunc();
                    f2 = self.df(xx, zpp, awp, ztt, awrt);
                    f = yy * f2;
                } else {
                    en = 0.0;
                    f = 0.0;
                    for iq in 0..64 {
                        let mut b = 0.0;
                        let p = legndr1(QP64[iq], na);
                        for ia in 1..=na + 1 {
                            b += (2 * ia - 1) as f64 * at(l + ia) * p[ia] / 2.0;
                        }
                        if yy != 0.0 {
                            b /= yy;
                        }
                        let er = (e - 2.0 * (e * awp * xx).sqrt() * QP64[iq] + awp * xx)
                            / (awrt + 1.0 - awp);
                        en += QW64[iq] * b * er;
                        f += QW64[iq] * b * self.df(er, zp, ap, zt, at_);
                    }
                    x2 = en;
                    en *= yy;
                    f2 = f;
                    f *= yy;
                }
                if i <= nd {
                    h += en;
                    d += f;
                } else if i == nd + 1 {
                    xl = xx;
                    yl = yy;
                    el = en;
                    fl = f;
                } else {
                    let mut t1 = 0.0;
                    let mut t2 = 0.0;
                    if lep == 1 {
                        t1 = x2 * yl + el;
                    }
                    if lep > 1 {
                        t1 = en + el;
                    }
                    h += (xx - xl) * t1 / 2.0;
                    if lep == 1 {
                        t2 = f2 * yl + fl;
                    }
                    if lep > 1 {
                        t2 = f + fl;
                    }
                    d += (xx - xl) * t2 / 2.0;
                    xl = xx;
                    yl = yy;
                    el = en;
                    fl = f;
                }
            }
            return Ok((h, d));
        }
        if law == 2 || law == 3 {
            // 450: discrete two-body.
            ebar = 0.0;
            dame = 0.0;
            *lang = 0;
            if law == 2 {
                *lang = nint(at(3)) as i32;
            }
            let nld = if law == 2 { nint(at(6)) as usize } else { 0 };
            let thresh = ((awrt + 1.0) / awrt) * (-self.q);
            if e < thresh {
                return Ok((ebar, dame));
            }
            let mut beta = (awrt * (awrt + 1.0 - awp) * (1.0 - thresh / e) / awp).sqrt();
            let mut afact = awp / (1.0 + awrt).powi(2);
            let arec = awp / (awrt + 1.0 - awp);
            if *lang == 0 {
                let mut f1 = 0.0;
                if law != 3 {
                    f1 = at(7);
                }
                if irec > 0 {
                    f1 = -f1;
                    beta *= arec;
                    afact /= arec;
                }
                ebar = afact * e * (1.0 + 2.0 * beta * f1 + beta * beta);
                if self.idame == 0 {
                    return Ok((ebar, dame));
                }
                for iq in 0..64 {
                    let mut u = QP64[iq];
                    if irec > 0 {
                        u = -u;
                    }
                    let mut f = 0.5;
                    if law != 3 {
                        let p = legndr1(u, nld);
                        for il in 1..=nld {
                            f += (2 * il + 1) as f64 * at(6 + il) * p[il + 1] / 2.0;
                        }
                    }
                    let e2 = afact * e * (1.0 + 2.0 * beta * u + beta * beta);
                    dame += QW64[iq] * f * self.df(e2, zp, ap, zt, at_);
                }
            } else {
                let nmu = nint(at(6)) as usize;
                if irec > 0 {
                    beta *= arec;
                    afact /= arec;
                }
                let (mut el, mut fl, mut ul) = (0.0, 0.0, 0.0);
                for imu in 1..=nmu {
                    let mut l = 6 + 2 * imu - 1;
                    if irec > 0 {
                        l = 6 + 2 * nmu - 2 * imu + 1;
                    }
                    let mut u = at(l);
                    if irec > 0 {
                        u = -u;
                    }
                    let yy = at(l + 1);
                    let e2 = afact * e * (1.0 + 2.0 * beta * u + beta * beta);
                    let en = yy * e2;
                    let fn_ = yy * self.df(e2, zp, ap, zt, at_);
                    if imu > 1 {
                        ebar += (u - ul) * (en + el) / 2.0;
                        dame += (u - ul) * (fn_ + fl) / 2.0;
                    }
                    ul = u;
                    el = en;
                    fl = fn_;
                }
            }
            return Ok((ebar, dame));
        }
        if law == 6 {
            *lang = 0;
        }

        // Law 1 (and 6) in the CM: adaptive integration over lab energy.
        let nl = if irec > 0 { 2 } else { 1 };
        let mut summ = 0.0;
        ebar = 0.0;
        dame = 0.0;
        let mut x = [0.0; IMAX + 1];
        let mut y = [[0.0f64; 3]; IMAX + 1];
        let mut epnext = 0.0;
        x[2] = 0.0;
        let term = self.h6cm(x[2], &mut epnext, nl, *lang, lep, c)?;
        for l in 1..=nl {
            y[2][l] = term[l];
        }
        let mut el = x[2] * y[2][1];
        let mut fl = 0.0;
        'outer: loop {
            // 120
            x[1] = epnext;
            let term = self.h6cm(x[1], &mut epnext, nl, *lang, lep, c)?;
            for l in 1..=nl {
                y[1][l] = term[l];
            }
            let mut i = 2usize;
            let dy = (y[1][1] + y[2][1]) / 10000.0;
            loop {
                // 140
                let mut go_190 = true;
                if i != IMAX {
                    let da = (x[i - 1] - x[i]) * (y[i - 1][1] + y[i][1]) / 2.0;
                    if da >= SMALL {
                        let xm = (x[i - 1] + x[i]) / 2.0;
                        if !(xm >= x[i - 1] || xm <= x[i]) {
                            let mut epn = x[i];
                            let term = self.h6cm(xm, &mut epn, nl, *lang, lep, c)?;
                            let ym = (y[i - 1][1] + y[i][1]) / 2.0;
                            let test = 2.0 * TOL * ym.abs() + 2.0 * dy;
                            if (term[1] - ym).abs() > test {
                                // 160: fails.
                                i += 1;
                                x[i] = x[i - 1];
                                y[i] = y[i - 1];
                                x[i - 1] = xm;
                                for l in 1..=nl {
                                    y[i - 1][l] = term[l];
                                }
                                go_190 = false;
                            }
                        }
                    }
                }
                if !go_190 {
                    continue;
                }
                // 190: passes.
                loop {
                    if i == 1 {
                        break 'outer;
                    }
                    summ += (x[i - 1] - x[i]) * (y[i - 1][1] + y[i][1]) / 2.0;
                    let (en, f);
                    if irec == 0 {
                        en = y[i - 1][1] * x[i - 1];
                        ebar += (x[i - 1] - x[i]) * (en + el) / 2.0;
                        f = y[i - 1][1] * self.df(x[i - 1], zp, ap, zt, at_);
                        dame += (x[i - 1] - x[i]) * (f + fl) / 2.0;
                    } else {
                        let mut f1 = 0.0;
                        if y[i - 1][1] != 0.0 {
                            f1 = y[i - 1][2] / y[i - 1][1];
                        }
                        let mut enr = (e - 2.0 * (e * awp * x[i - 1]).sqrt() * f1 + awp * x[i - 1])
                            / (awrt + 1.0 - awp);
                        f = y[i - 1][1] * self.df(enr, zp, ap, zt, at_);
                        enr *= y[i - 1][1];
                        en = enr;
                        ebar += (x[i - 1] - x[i]) * (en + el) / 2.0;
                        dame += (x[i - 1] - x[i]) * (f + fl) / 2.0;
                    }
                    el = en;
                    fl = f;
                    i -= 1;
                    if i > 1 {
                        break; // back to 140
                    }
                    let test = EMAX - EMAX / 100.0;
                    if epnext >= test {
                        continue; // 190 again, with i = 1: done
                    }
                    x[2] = x[1];
                    y[2] = y[1];
                    continue 'outer;
                }
            }
        }
        let _ = summ;
        Ok((ebar, dame))
    }

    /// `tabsq6(g, h, a, law, z, awr, e, yld)`: the average photon-recoil
    /// energy and its damage for MF=6 capture photons; `a` is the LAW=1 LIST
    /// at incident energy `e`, `yld` the subsection's yield TAB1.
    pub(super) fn tabsq6(
        &mut self,
        a: &[f64],
        z: f64,
        awr: f64,
        e: f64,
        yld: &[f64],
    ) -> (f64, f64) {
        let at = |k: usize| a.get(k - 1).copied().unwrap_or(0.0);
        let ein = 2.0 * self.tm;
        let rein = 1.0 / ein;
        let (mut g, mut h, mut s) = (0.0, 0.0, 0.0);
        let nd = nint(at(3)) as usize;
        let np = nint(at(6)) as usize;
        let ncyc = nint(at(5)) as usize / np.max(1);
        let mut ibase = 6;
        let inn = 2;
        let yield_ = terpa0(yld, e).0;
        if nd != 0 {
            for i in 1..=nd {
                let x = at(ibase + ncyc * (i - 1) + 1);
                let y = at(ibase + ncyc * (i - 1) + 2);
                let xr = x * x * rein;
                g += xr * y;
                let awc = awr + 1.0;
                h += self.df(xr, z, awc, z, awr) * y;
                s += y;
            }
        }
        if np > nd {
            let nc = np - nd;
            ibase += ncyc * nd;
            let mut xh = at(ibase + 1);
            let mut yh = at(ibase + 2);
            for i in 2..=nc {
                let xl = xh;
                xh = at(ibase + ncyc * (i - 1) + 1);
                let yl = yh;
                yh = at(ibase + ncyc * (i - 1) + 2);
                if xl != xh {
                    let dx = xh - xl;
                    for j in 0..4 {
                        let x = xl + (1.0 + QP4[j]) * dx / 2.0;
                        let y = terp1(xl, yl, xh, yh, x, inn);
                        let xr = x * x * rein;
                        g += QW4[j] * xr * dx * y;
                        h += QW4[j] * self.df(xr, z, awr + 1.0, z, awr) * dx * y;
                        s += QW4[j] * dx * y;
                    }
                }
            }
        }
        (yield_ * g / s, yield_ * h / s)
    }

    /// `hgam102(e, ebar, dame, disc102, c, irec, zp, ap, zt, at)`.
    pub(super) fn hgam102(
        &mut self,
        e: f64,
        disc102: f64,
        irec: usize,
        zp: f64,
        ap: f64,
        zt: f64,
        at: f64,
    ) -> (f64, f64) {
        if irec == 0 {
            (disc102 + e * self.awr / (1.0 + self.awr), 0.0)
        } else {
            let er = e / (self.awr + 1.0);
            let eg2 = disc102 * disc102 * self.rtm / 2.0;
            let ebar = er + eg2;
            (ebar, self.df(ebar, zp, ap, zt, at))
        }
    }
}
