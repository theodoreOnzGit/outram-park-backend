// Ported from NJOY2016 `src/heatr.f90` (`h6cm` 3437-3683, `h6ddx`
// 3684-3873, `h6dis` 3874-3961, `bacha` 3962-4071, `h6psp` 4072-4134).
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under GPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! MF=6 LAW=1 and LAW=6 in the centre-of-mass system, for `getsix`'s
//! integration over the laboratory secondary energy.
//!
//! `h6cm` returns the Legendre moments of the double-differential cross
//! section at one laboratory energy, by adaptive integration along the
//! trajectory of constant laboratory energy in the CM plane; `h6ddx` (the
//! continuum, Legendre, Kalbach or tabulated in angle), `h6dis` (the discrete
//! lines) and `h6psp` (the n-body phase-space law) supply the CM density.
//! GROUPR has its own integrator for the same problem
//! (`groupr::kinematics::cm`); HEATR's is a different routine with different
//! tolerances and is translated as it stands.

use super::flat::{legndr1, nint, terp1};
use super::state::Heatr;
use crate::mixr::mix::sigfig;
use crate::NjoyError;

const EMAX: f64 = 1.0e10;
const SMALL: f64 = 1.0e-10;

impl Heatr {
    /// `h6cm(ep, epnext, term, nl, lang, lep, cnow)`. `ep = 0` initialises
    /// for the incident energy in `cnow`. Returns `term(1..=nl)` (1-based) and
    /// sets `epnext`.
    pub(super) fn h6cm(
        &mut self,
        ep: f64,
        epnext: &mut f64,
        nl: usize,
        lang: i32,
        lep: i32,
        cnow: &[f64],
    ) -> Result<Vec<f64>, NjoyError> {
        const IMAX: usize = 10;
        const MXLG: usize = 65;
        const TOL: f64 = 0.02;
        const STEP: f64 = 0.05;
        const CHECK: f64 = 0.99999;
        const RDN: f64 = 0.999995;
        const RUP: f64 = 1.000005;
        const TINY: f64 = 1.0e-9;
        const DN: f64 = 0.999999;
        const UP: f64 = 1.000001;
        const THOU: f64 = 1.0e-3;
        const AMIL: f64 = 1.0e-6;
        const ONEP5: f64 = 1.5;
        const DU: f64 = 0.0001;
        let at = |k: usize| cnow[k - 1];
        let mut term = vec![0.0; nl + 1];
        if ep <= 0.0 {
            let s = &mut self.h6c;
            s.e = at(2);
            s.ndnow = nint(at(3)) as usize;
            s.npnow = nint(at(6)) as usize;
            s.ncnow = nint(at(5)) as usize / s.npnow.max(1);
            let aprime = self.awp;
            s.xc = aprime / ((self.awrt + 1.0) * (self.awrt + 1.0));
            *epnext = EMAX;
            s.elmax = 0.0;
            let mut epnn = EMAX;
            s.epmax = 0.0;
            let (mut epm, mut epn, w) = (0.0, 0.0, 0.0);
            let (e, xc, ndnow, npnow) = (s.e, s.xc, s.ndnow, s.npnow);
            if npnow != ndnow {
                if lang > 0 {
                    self.h6ddx(ep, &mut epn, &mut epm, w, cnow, lang, lep)?;
                } else {
                    self.h6psp(ep, &mut epn, &mut epm, w, e, cnow)?;
                }
                let s = &mut self.h6c;
                if epn < epnn * (1.0 - SMALL) {
                    epnn = epn;
                }
                if epm > s.epmax * (1.0 + SMALL) {
                    s.epmax = epm;
                }
                s.elmax = e * ((s.epmax / e).sqrt() + xc.sqrt()).powi(2);
                s.elmax *= UP;
                epn = STEP * s.elmax;
                if s.epmax * UP < xc * e {
                    epn = e * ((s.epmax / e).sqrt() - xc.sqrt()).powi(2);
                }
                if epn < *epnext {
                    *epnext = epn;
                }
                s.eps = TINY / s.elmax;
                *epnext *= DN;
            }
            if ndnow != 0 {
                let mut epn = 0.0;
                let mut epm = 0.0;
                self.h6dis(0, &mut epn, &mut epm, w, cnow, lang)?;
                for i in 1..=ndnow {
                    self.h6dis(i, &mut epn, &mut epm, w, cnow, lang)?;
                    let s = &mut self.h6c;
                    if epn < epnn * (1.0 - SMALL) {
                        epnn = epn;
                    }
                    if epn > s.epmax * (1.0 + SMALL) {
                        s.epmax = epn;
                    }
                    let epx = DN * e * ((epn / e).sqrt() - xc.sqrt()).powi(2);
                    if epx < *epnext * (1.0 - SMALL) {
                        *epnext = epx;
                    }
                    let epx = UP * e * ((epn / e).sqrt() + xc.sqrt()).powi(2);
                    if epx > s.elmax * (1.0 + SMALL) {
                        s.elmax = epx;
                    }
                }
            }
            self.h6c.na = nl.saturating_sub(1);
            let _ = epnn;
            return Ok(term);
        }

        // Normal entry.
        let (e, xc, ndnow, npnow, ncnow, epmax, elmax, na) = (
            self.h6c.e,
            self.h6c.xc,
            self.h6c.ndnow,
            self.h6c.npnow,
            self.h6c.ncnow,
            self.h6c.epmax,
            self.h6c.elmax,
            self.h6c.na,
        );
        let mut epnxt;
        if ndnow != npnow {
            let xx = ep / e;
            let mut cc = xc / xx;
            let mut c = cc.sqrt();
            let mut umin = (1.0 + cc - epmax / ep) / (2.0 * c);
            if umin >= CHECK {
                if c <= 1.0 {
                    // 420
                    *epnext = EMAX;
                    return Ok(term);
                }
                // 320
                epnxt = (*epnext + elmax) / 2.0;
                *epnext = epnxt;
                return Ok(term);
            }
            // 215
            if umin < -1.0 {
                umin = -1.0;
            }
            let mut dm = (1.0 - umin) / 1000.0 + AMIL;
            if dm < THOU / 5.0 {
                dm = THOU / 5.0;
            }
            let mut epnn = EMAX;
            let umax = 1.0;
            let mut un = umax;
            let mut x = [0.0; IMAX + 1];
            let mut y = vec![[0.0f64; MXLG + 1]; IMAX + 1];
            let mut yt = [0.0f64; MXLG + 1];
            x[2] = un;
            let eps = self.h6c.eps;
            let (mut epn, mut epm) = (0.0f64, 0.0f64);
            loop {
                // 220
                if x[2] <= umin {
                    break;
                }
                let mut j = 2;
                while j == 2 {
                    let u = un;
                    let mut yy = 1.0 + cc - 2.0 * c * u;
                    if yy < AMIL {
                        yy = AMIL;
                        c = u - THOU;
                        cc = c * c;
                    }
                    let epp = yy * ep;
                    let w = (u - c) / yy.sqrt();
                    let s = if lang > 0 {
                        self.h6ddx(epp, &mut epn, &mut epm, w, cnow, lang, lep)?
                    } else {
                        self.h6psp(epp, &mut epn, &mut epm, w, e, cnow)?
                    };
                    if u == umax && epn < epnn {
                        epnn = epn;
                    }
                    let p = legndr1(u, na);
                    j = 1;
                    if u == umax {
                        j = 2;
                    }
                    x[j] = u;
                    for l in 1..=nl {
                        y[j][l] = p[l] * s / yy.sqrt();
                    }
                    un = u - (epn - epp) / (2.0 * c * ep);
                    if un > u - DU {
                        un = u - DU;
                    }
                    if un < umin + DU / 10.0 {
                        un = umin;
                    }
                }
                let mut i = 2usize;
                loop {
                    // 240
                    let mut pass = true;
                    if i != IMAX {
                        let dx = x[i] - x[i - 1];
                        if dx >= dm {
                            let da = dx * (y[i][1] + y[i - 1][1]) / 2.0;
                            if da >= eps {
                                let u = (x[i - 1] + x[i]) / 2.0;
                                let yy = 1.0 + cc - 2.0 * c * u;
                                let epp = yy * ep;
                                let w = (u - c) / yy.sqrt();
                                let s = if lang > 0 {
                                    self.h6ddx(epp, &mut epn, &mut epm, w, cnow, lang, lep)?
                                } else {
                                    self.h6psp(epp, &mut epn, &mut epm, w, e, cnow)?
                                };
                                let p = legndr1(u, na);
                                for l in 1..=nl {
                                    yt[l] = p[l] * s / yy.sqrt();
                                }
                                for l in 1..=nl {
                                    let ym = (y[i - 1][l] + y[i][l]) / 2.0;
                                    if (yt[l] - ym).abs() > l as f64 * TOL * ym.abs() + eps {
                                        pass = false;
                                        break;
                                    }
                                }
                                if !pass {
                                    // 260
                                    i += 1;
                                    x[i] = x[i - 1];
                                    y[i] = y[i - 1];
                                    x[i - 1] = u;
                                    for l in 1..=nl {
                                        y[i - 1][l] = yt[l];
                                    }
                                    continue;
                                }
                            }
                        }
                    }
                    // 290
                    for l in 1..=nl {
                        term[l] += (x[i] - x[i - 1]) * (y[i][l] + y[i - 1][l]) / 2.0;
                    }
                    i -= 1;
                    if i > 1 {
                        continue;
                    }
                    break;
                }
                x[2] = x[1];
                y[2] = y[1];
            }
            // 310
            epnxt = e * ((epnn / e).sqrt() - xc.sqrt()).powi(2);
            if epnxt <= ep * (1.0 + SMALL) {
                epnxt = e * ((epnn / e).sqrt() + xc.sqrt()).powi(2);
            }
            if epnxt <= ep * (1.0 + SMALL) {
                epnxt = ONEP5 * ep;
            }
            if epnxt > elmax * (1.0 + SMALL) && elmax > ep * (1.0 + SMALL) {
                epnxt = elmax;
            }
            *epnext = epnxt;
            return Ok(term);
        }
        // 330: contributions from the delta functions.
        if ndnow == 0 {
            // `epnxt` is unset on this path upstream; it keeps the value
            // passed in.
            return Ok(term);
        }
        epnxt = EMAX;
        let (mut epn, mut epm) = (0.0f64, 0.0f64);
        for i in 1..=ndnow {
            let ll = 7 + ncnow * (i - 1);
            let epp = at(ll);
            let w = (ep - epp - xc * e) / (2.0 * (xc * e).sqrt() * epp.sqrt());
            let u = (xc * e + ep - epp) / (2.0 * (xc * e).sqrt() * ep.sqrt());
            if (-1.0..=1.0).contains(&w) {
                let p = legndr1(u, na);
                let mut f = self.h6dis(i, &mut epn, &mut epm, w, cnow, lang)?;
                f /= 2.0 * (xc * e * epp).sqrt();
                epn = e * ((epp / e).sqrt() + xc.sqrt()).powi(2);
                if ep < CHECK * epn {
                    epn *= RDN;
                } else {
                    epn *= RUP;
                }
                if epn < epnxt {
                    epnxt = epn;
                }
                for l in 1..=nl {
                    term[l] += f * p[l];
                }
            } else if w < -1.0 {
                epn = e * ((epp / e).sqrt() - xc.sqrt()).powi(2);
                if ep < CHECK * epn {
                    epn *= RDN;
                } else {
                    epn *= RUP;
                }
                if epn < epnxt {
                    epnxt = epn;
                }
            }
        }
        *epnext = epnxt;
        Ok(term)
    }

    /// `h6ddx(ep, epnext, epmax, w, cnow, lang, lep)`: the continuum part of
    /// the CM double-differential cross section.
    pub(super) fn h6ddx(
        &mut self,
        ep: f64,
        epnext: &mut f64,
        epmax: &mut f64,
        w: f64,
        cnow: &[f64],
        lang: i32,
        lep: i32,
    ) -> Result<f64, NjoyError> {
        const NLMAX: usize = 65;
        const UP: f64 = 1.00001;
        const DN: f64 = 0.99999;
        const OFF: f64 = 0.999995;
        let at = |k: usize| cnow.get(k.wrapping_sub(1)).copied().unwrap_or(0.0);
        if ep == 0.0 {
            let s = &mut self.h6d;
            s.enow = at(2);
            let ndnow = nint(at(3)) as usize;
            let npnow = nint(at(6)) as usize;
            s.ncnow = nint(at(5)) as usize / npnow.max(1);
            s.mnow = 7 + (npnow - 1) * s.ncnow;
            s.inow = 7 + s.ncnow * ndnow;
            s.lnow = s.inow;
            if at(s.lnow) <= 0.0 {
                s.lnow += s.ncnow;
            }
            *epnext = at(s.lnow);
            *epmax = at(s.mnow);
            s.nl = s.ncnow - 1;
            if s.nl > NLMAX {
                return Err(NjoyError::EndfParse(
                    "h6ddx: too many legendre terms".into(),
                ));
            }
            s.na = s.nl.saturating_sub(1);
            s.efirst = 0.0;
            s.illdef = 0;
            return Ok(0.0);
        }
        let (ncnow, mnow, inow) = (self.h6d.ncnow, self.h6d.mnow, self.h6d.inow);
        let mut t;
        let region: u8; // 0 = 200 (zero), 1 = 190 (above), 2 = 210 (evaluate)
        if ep < self.h6d.efirst * (1.0 - SMALL) {
            region = 0;
        } else if ep > at(mnow) {
            region = 1;
        } else {
            let mut r = 2u8;
            loop {
                // 130
                *epnext = at(self.h6d.lnow);
                if ep < OFF * *epnext {
                    // 140
                    if self.h6d.lnow <= ncnow {
                        r = 0;
                        break;
                    }
                    let eplast = at(self.h6d.lnow - ncnow);
                    if ep >= OFF * eplast {
                        break;
                    }
                    if self.h6d.lnow <= inow + ncnow {
                        break;
                    }
                    self.h6d.lnow -= ncnow;
                    continue;
                }
                if self.h6d.lnow >= mnow {
                    break;
                }
                self.h6d.lnow += ncnow;
            }
            region = r;
        }
        match region {
            0 => t = 0.0,
            1 => {
                t = 0.0;
                *epnext = EMAX;
            }
            _ => {
                let lnow = self.h6d.lnow;
                let in_range = ep >= at(inow) * (1.0 - SMALL) && ep <= at(mnow) * (1.0 + SMALL);
                let x1 = at(lnow - ncnow);
                let mut x2 = at(lnow);
                if x1 == x2 && lep > 1 {
                    x2 = sigfig(x2, 6, 1);
                    self.h6d.illdef = 1;
                }
                if lang == 1 {
                    let (nl, na) = (self.h6d.nl, self.h6d.na);
                    let p = legndr1(w, na);
                    t = 0.0;
                    for l in 1..=nl {
                        let mut tt = 0.0;
                        if l <= ncnow - 1 && in_range {
                            let lll = lnow + l;
                            tt = terp1(x1, at(lll - ncnow), x2, at(lll), ep, i64::from(lep));
                        }
                        t += p[l] * (2 * l - 1) as f64 * tt / 2.0;
                    }
                    if t < 0.0 {
                        t = 0.0;
                    }
                } else if lang == 2 {
                    let (mut s, mut r) = (0.0, 0.0);
                    if in_range {
                        s = terp1(
                            x1,
                            at(lnow - ncnow + 1),
                            x2,
                            at(lnow + 1),
                            ep,
                            i64::from(lep),
                        );
                        r = terp1(
                            x1,
                            at(lnow - ncnow + 2),
                            x2,
                            at(lnow + 2),
                            ep,
                            i64::from(lep),
                        );
                    }
                    let iza2 = nint(self.zap);
                    let aa = bacha(self.izap, iza2, self.izat, self.h6d.enow, ep)?;
                    t = aa * ((aa * w).cosh() + r * (aa * w).sinh()) / (2.0 * aa.sinh());
                    t *= s;
                    if t < 0.0 {
                        t = 0.0;
                    }
                } else if (11..=15).contains(&lang) {
                    let inn = i64::from(lang - 10);
                    let na = self.h6d.na;
                    let ii = lnow - ncnow;
                    let jj = lnow;
                    let s = terp1(x1, at(ii + 1), x2, at(jj + 1), ep, i64::from(lep));
                    let mut tii = 0.0;
                    let mut ia = 1;
                    while ia <= na {
                        if w >= at(ii + 1 + ia) && w <= at(ii + 3 + ia) {
                            tii = terp1(
                                at(ii + 1 + ia),
                                at(ii + 2 + ia),
                                at(ii + 3 + ia),
                                at(ii + 4 + ia),
                                w,
                                inn,
                            );
                        }
                        ia += 2;
                    }
                    let mut tjj = 0.0;
                    let mut ia = 1;
                    while ia <= na {
                        if w >= at(jj + 1 + ia) && w <= at(jj + 3 + ia) {
                            tjj = terp1(
                                at(jj + 1 + ia),
                                at(jj + 2 + ia),
                                at(jj + 3 + ia),
                                at(jj + 4 + ia),
                                w,
                                inn,
                            );
                        }
                        ia += 2;
                    }
                    t = terp1(x1, tii, x2, tjj, ep, i64::from(lep));
                    t *= s;
                } else {
                    return Err(NjoyError::EndfParse("h6ddx: illegal lang".into()));
                }
            }
        }
        // 350
        if lep > 1 || *epnext == EMAX {
            return Ok(t);
        }
        if ep < DN * DN * *epnext {
            *epnext *= DN;
        } else {
            *epnext *= UP;
        }
        Ok(t)
    }

    /// `h6dis(i, epnext, epmax, w, cnow, lang)`: discrete line `i` of the CM
    /// double-differential cross section; `i = 0` initialises.
    pub(super) fn h6dis(
        &mut self,
        i: usize,
        epnext: &mut f64,
        epmax: &mut f64,
        w: f64,
        cnow: &[f64],
        lang: i32,
    ) -> Result<f64, NjoyError> {
        let at = |k: usize| cnow.get(k.wrapping_sub(1)).copied().unwrap_or(0.0);
        if i == 0 {
            let s = &mut self.h6s;
            s.enow = at(2);
            let ndnow = nint(at(3)) as usize;
            let npnow = nint(at(6)) as usize;
            s.ncnow = nint(at(5)) as usize / npnow.max(1);
            s.inow = 7;
            s.lnow = s.inow;
            s.mnow = s.lnow + ndnow.saturating_sub(1) * s.ncnow;
            *epnext = at(s.lnow);
            *epmax = at(s.mnow);
            s.nl = s.ncnow - 1;
            s.na = s.nl.saturating_sub(1);
            return Ok(0.0);
        }
        let ncnow = self.h6s.ncnow;
        self.h6s.inow = 7 + (i - 1) * ncnow;
        let inow = self.h6s.inow;
        *epnext = at(inow);
        let mut t;
        if lang == 1 {
            let (nl, na) = (self.h6s.nl, self.h6s.na);
            let p = legndr1(w, na);
            t = 0.0;
            for l in 1..=nl {
                if l <= ncnow - 1 {
                    t += (2 * l - 1) as f64 * p[l] * at(inow + 1 + l) / 2.0;
                }
            }
            t *= at(inow + 1);
            if t < 0.0 {
                t = 0.0;
            }
        } else if lang == 2 {
            let s = at(inow + 1);
            let r = at(inow + 2);
            let iza2 = nint(self.zap);
            let aa = bacha(self.izap, iza2, self.izat, self.h6s.enow, *epnext)?;
            t = aa * ((aa * w).cosh() + r * (aa * w).sinh()) / (2.0 * aa.sinh());
            t *= s;
            if t < 0.0 {
                t = 0.0;
            }
        } else if (11..=15).contains(&lang) {
            let inn = i64::from(lang - 10);
            let na = self.h6s.na;
            t = 0.0;
            let mut ia = 1;
            while ia + 2 <= na {
                if w >= at(inow + 1 + ia) && w <= at(inow + 3 + ia) {
                    t = terp1(
                        at(inow + 1 + ia),
                        at(inow + 2 + ia),
                        at(inow + 3 + ia),
                        at(inow + 4 + ia),
                        w,
                        inn,
                    );
                }
                ia += 2;
            }
            t *= at(inow + 1);
        } else {
            return Err(NjoyError::EndfParse("h6dis: illegal lang".into()));
        }
        Ok(t)
    }

    /// `h6psp(ep, epnext, epmax, w, e, c)`: the n-body phase-space density;
    /// `ep = 0` initialises for incident energy `e`.
    pub(super) fn h6psp(
        &mut self,
        ep: f64,
        epnext: &mut f64,
        epmax: &mut f64,
        _w: f64,
        e: f64,
        c: &[f64],
    ) -> Result<f64, NjoyError> {
        const C3: f64 = 1.2732;
        const C4: f64 = 3.2813;
        const C5: f64 = 5.8205;
        const THRHAF: f64 = 1.5;
        const SEVHAF: f64 = 3.5;
        const STEP: f64 = 0.05;
        const RNDUP: f64 = 1.000001;
        if ep == 0.0 {
            let apsx = c[0];
            let npsx = nint(c[5]);
            let f1 = (apsx - self.awp) / apsx;
            let f2 = self.awrt / (self.awrt + 1.0);
            let s = &mut self.psp;
            s.ex = THRHAF * npsx as f64 - 4.0;
            s.eimax = f1 * (f2 * e + self.q);
            if s.eimax <= 0.0 {
                s.eimax = 1.0;
            }
            s.cn = match npsx {
                3 => C3 / s.eimax.powi(2),
                4 => C4 / s.eimax.powf(SEVHAF),
                5 => C5 / s.eimax.powi(5),
                _ => {
                    return Err(NjoyError::EndfParse(
                        "h6psp: 3, 4, or 5 particles only".into(),
                    ))
                }
            };
            *epnext = STEP * s.eimax;
            if s.eimax == 1.0 {
                *epnext = EMAX;
            }
            *epmax = s.eimax;
            return Ok(0.0);
        }
        let s = &self.psp;
        let mut v = 0.0;
        if ep < s.eimax * (1.0 - SMALL) {
            v = s.cn * ep.sqrt() * (s.eimax - ep).powf(s.ex);
        }
        *epnext = ep + STEP * s.eimax;
        if *epnext > s.eimax * (1.0 + SMALL) {
            *epnext = RNDUP * s.eimax;
        }
        if ep >= s.eimax * (1.0 - SMALL) {
            *epnext = EMAX;
        }
        Ok(v)
    }
}

/// `bacha(iza1i, iza2, izat, e, ep)`: the Kalbach `a` parameter.
pub(super) fn bacha(iza1i: i64, iza2: i64, izat: i64, e: f64, ep: f64) -> Result<f64, NjoyError> {
    const THIRD: f64 = 0.333333333;
    const TWOTH: f64 = 0.666666667;
    const FOURTH: f64 = 1.33333333;
    const C1: f64 = 15.68;
    const C2: f64 = -28.07;
    const C3: f64 = -18.56;
    const C4: f64 = 33.22;
    const C5: f64 = -0.717;
    const C6: f64 = 1.211;
    const S2: f64 = 2.22;
    const S3: f64 = 8.48;
    const S4: f64 = 7.72;
    const S5: f64 = 28.3;
    const BRK1: f64 = 130.0;
    const BRK2: f64 = 41.0;
    const HALF: f64 = 0.5;
    const B1: f64 = 0.04;
    const B2: f64 = 1.8e-6;
    const B3: f64 = 6.7e-7;
    const TOMEV: f64 = 1.0e-6;
    let iza1 = if iza1i == 0 { 1 } else { iza1i };
    let iza = match izat {
        6000 => 6012,
        12000 => 12024,
        14000 => 14028,
        16000 => 16032,
        17000 => 17035,
        19000 => 19039,
        20000 => 20040,
        22000 => 22048,
        23000 => 23051,
        24000 => 24052,
        26000 => 26056,
        28000 => 28058,
        29000 => 29063,
        31000 => 31069,
        40000 => 40090,
        42000 => 42096,
        48000 => 48112,
        49000 => 49115,
        50000 => 50120,
        63000 => 63151,
        72000 => 72178,
        74000 => 74184,
        82000 => 82208,
        v => v,
    };
    let aa = (iza % 1000) as f64;
    if aa == 0.0 {
        return Err(NjoyError::EndfParse(format!(
            "bacha: dominant isotope not known for {iza:8}"
        )));
    }
    let za = (iza / 1000) as f64;
    let ac = aa + (iza1 % 1000) as f64;
    let zc = za + (iza1 / 1000) as f64;
    let ab = ac - (iza2 % 1000) as f64;
    let zb = zc - (iza2 / 1000) as f64;
    let na = nint(aa - za) as f64;
    let nb = nint(ab - zb) as f64;
    let nc = nint(ac - zc) as f64;
    let mut sa = C1 * (ac - aa)
        + C2 * ((nc - zc).powi(2) / ac - (na - za).powi(2) / aa)
        + C3 * (ac.powf(TWOTH) - aa.powf(TWOTH))
        + C4 * ((nc - zc).powi(2) / ac.powf(FOURTH) - (na - za).powi(2) / aa.powf(FOURTH))
        + C5 * (zc.powi(2) / ac.powf(THIRD) - za.powi(2) / aa.powf(THIRD))
        + C6 * (zc.powi(2) / ac - za.powi(2) / aa);
    match iza1 {
        1002 => sa -= S2,
        1003 => sa -= S3,
        2003 => sa -= S4,
        2004 => sa -= S5,
        _ => {}
    }
    let mut sb = C1 * (ac - ab)
        + C2 * ((nc - zc).powi(2) / ac - (nb - zb).powi(2) / ab)
        + C3 * (ac.powf(TWOTH) - ab.powf(TWOTH))
        + C4 * ((nc - zc).powi(2) / ac.powf(FOURTH) - (nb - zb).powi(2) / ab.powf(FOURTH))
        + C5 * (zc.powi(2) / ac.powf(THIRD) - zb.powi(2) / ab.powf(THIRD))
        + C6 * (zc.powi(2) / ac - zb.powi(2) / ab);
    match iza2 {
        1002 => sb -= S2,
        1003 => sb -= S3,
        2003 => sb -= S4,
        2004 => sb -= S5,
        _ => {}
    }
    let ecm = aa * e / ac;
    let ea = ecm * TOMEV + sa;
    let eb = TOMEV * ep * ac / ab + sb;
    let mut x1 = eb;
    if ea > BRK1 {
        x1 = BRK1 * eb / ea;
    }
    let mut x3 = eb;
    if ea > BRK2 {
        x3 = BRK2 * eb / ea;
    }
    let fa = if iza1 == 2004 { 0.0 } else { 1.0 };
    let fb = match iza2 {
        1 => HALF,
        2004 => 2.0,
        _ => 1.0,
    };
    Ok(B1 * x1 + B2 * x1.powi(3) + B3 * fa * fb * x3.powi(4))
}
