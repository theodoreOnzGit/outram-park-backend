// Ported from NJOY2016 `src/heatr.f90` (`capdam`, lines 1741-1828;
// `disbar`, lines 1829-2014).
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under GPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! `disbar` (discrete two-body scattering: mean outgoing energy and damage)
//! and `capdam` (damage from capture and charged-particle emission).
//!
//! Both evaluate their answer at nodes 10 % apart in incident energy and
//! interpolate linearly between nodes; the node chain depends on the order
//! of the calls, so they must be walked over ascending energies from an
//! `e = 0` initialisation, as `nheat` walks them. `crate::heatr::twobody`
//! holds the mean-energy half of `disbar` for the ACE route's
//! `Kerma::from_endf`; this is the whole routine, damage included.

use super::flat::{legndr1, nint, terp1, QP4, QP64, QW4, QW64};
use super::state::Heatr;
use crate::common::phys::{ANRATIO, DNRATIO, HNRATIO, PNRATIO, TNRATIO};
use crate::NjoyError;

impl Heatr {
    /// `capdam(ee, dame, q, za, awr, mtd)`. `dame` is in/out as upstream:
    /// the `e = 0` call zeroes it.
    pub(super) fn capdam(&mut self, ee: f64, dame: &mut f64, q: f64, za: f64, awr: f64, mtd: i32) {
        const THIRD: f64 = 0.333333333;
        const ECON: f64 = 1.029e6;
        const EPS: f64 = 1.0e-10;
        const STEP: f64 = 1.1;
        let mut e = ee;
        if e == 0.0 {
            let iverf = self.iverf;
            let s = &mut self.cap;
            s.zx = 0.0;
            if mtd > 102 {
                s.zx = 1.0;
            }
            if mtd == 106 || mtd == 107 {
                s.zx = 2.0;
            }
            if iverf <= 5 && (mtd == 778 || mtd == 798) {
                s.zx = 2.0;
            }
            s.ax = 0.0;
            if mtd > 102 {
                s.ax = 1.0;
            }
            if mtd == 104 {
                s.ax = 2.0;
            }
            if iverf <= 5 && mtd == 738 {
                s.ax = 2.0;
            }
            if mtd == 105 || mtd == 106 {
                s.ax = 3.0;
            }
            if iverf <= 5 && (mtd == 758 || mtd == 778) {
                s.ax = 3.0;
            }
            if mtd == 107 {
                s.ax = 4.0;
            }
            if iverf <= 5 && mtd == 798 {
                s.ax = 4.0;
            }
            s.denom = 1.0 / awr.powf(THIRD);
            if s.ax != 0.0 {
                s.denom = 1.0 / (s.ax.powf(THIRD) + awr.powf(THIRD));
            }
            let iz = nint(za / 1000.0);
            s.z = iz as f64;
            s.en = 0.0;
            let (z, zx, ax, en) = (s.z, s.zx, s.ax, s.en);
            let damn = self.df(en, z - zx, awr + 1.0 - ax, z, awr);
            self.cap.damn = damn;
            *dame = 0.0;
            self.cap.aw1fac = 1.0 / (awr + 1.0);
            return;
        }
        if ee >= self.cap.en * (1.0 - EPS) {
            self.cap.el = self.cap.en;
            self.cap.daml = self.cap.damn;
            e = STEP * self.cap.el;
            if e < ee * (1.0 - EPS) {
                e = (1.0 - EPS) * ee;
            }
            let mut d = 0.0;
            let (z, zx, ax, denom, aw1fac) = (
                self.cap.z,
                self.cap.zx,
                self.cap.ax,
                self.cap.denom,
                self.cap.aw1fac,
            );
            if mtd == 102 {
                // Capture followed by gamma emission.
                let mut er = e * aw1fac;
                let ea = awr * er + q;
                er += ea * ea * self.rtm / 2.0;
                d = self.df(er, z, awr + 1.0, z, awr);
            } else if ax != 0.0 {
                // Capture followed by particle emission.
                let ec = ECON * zx * z * denom;
                let mut ea = q + awr * e * aw1fac;
                if ea >= 0.0 {
                    let et = (awr + 1.0 - ax) * e * aw1fac;
                    if ea > ec * (1.0 + EPS) {
                        ea = ec;
                    }
                    for iq in 0..4 {
                        let er = (et - 2.0 * (et * ax * ea).sqrt() * QP4[iq] + ax * ea) * aw1fac;
                        d += QW4[iq] * self.df(er, z - zx, awr + 1.0 - ax, z, awr) / 2.0;
                    }
                }
            }
            self.cap.en = e;
            self.cap.damn = d;
        }
        *dame = terp1(
            self.cap.el,
            self.cap.daml,
            self.cap.en,
            self.cap.damn,
            ee,
            2,
        );
    }

    /// `disbar(ee, ebar, dame, q, za, awr, nend4, matd, mtd, ...)`. `ebar`
    /// and `dame` are in/out as upstream: the `e = 0` call leaves them.
    pub(super) fn disbar(
        &mut self,
        ee: f64,
        ebar: &mut f64,
        dame: &mut f64,
        q: f64,
        za: f64,
        awr: f64,
        mtd: i32,
    ) -> Result<(), NjoyError> {
        const EDIS: f64 = 25.0;
        const STEP: f64 = 1.1;
        const SMALL: f64 = 1.0e-10;
        let mut e = ee;
        if e == 0.0 {
            self.dis.imiss = i32::from(self.miss4[1..].contains(&mtd));
            if self.dis.imiss != 1 {
                let f = self.hgtfle(e, 60, 2, mtd)?;
                self.dis.enext = f.enext;
            } else {
                self.dis.enext = self.etop;
            }
            self.dis.thresh = ((awr + 1.0) / awr) * (-q);
            let iverf = self.iverf;
            let mut awp = 1.0;
            let r = |lo: i32, hi: i32| mtd >= lo && mtd < hi;
            if iverf <= 5 {
                if r(700, 718) {
                    awp = PNRATIO;
                }
                if r(720, 738) {
                    awp = DNRATIO;
                }
                if r(740, 758) {
                    awp = TNRATIO;
                }
                if r(760, 778) {
                    awp = HNRATIO;
                }
                if r(780, 798) {
                    awp = ANRATIO;
                }
            } else {
                if r(600, 649) {
                    awp = PNRATIO;
                }
                if r(650, 699) {
                    awp = DNRATIO;
                }
                if r(700, 749) {
                    awp = TNRATIO;
                }
                if r(750, 799) {
                    awp = HNRATIO;
                }
                if r(800, 849) {
                    awp = ANRATIO;
                }
            }
            let s = &mut self.dis;
            s.awp = awp;
            s.afact = awp / ((awr + 1.0) * (awr + 1.0));
            s.arat = awp / (awr + 1.0 - awp);
            s.en = 0.0;
            s.cn = 0.0;
            s.el = 0.0;
            s.cl = 0.0;
            s.enx = 0.0;
            if self.idame != 0 {
                let iz = nint(za / 1000.0);
                self.dis.z = iz as f64;
                let z = self.dis.z;
                let damn = self.df(e, z, awr, z, awr);
                let s = &mut self.dis;
                s.damn = damn;
                s.daml = damn;
                if mtd == 2 {
                    s.daml = 0.0;
                    s.enx = EDIS * s.arat / (4.0 * s.afact);
                    s.damn = 0.0;
                }
            }
            return Ok(());
        }

        // Normal entry.
        if ee > self.dis.en * (1.0 + SMALL) {
            self.dis.el = self.dis.en;
            self.dis.cl = self.dis.cn;
            if self.idame != 0 {
                self.dis.daml = self.dis.damn;
            }
            e = STEP * self.dis.el;
            if self.dis.enext < e * (1.0 - SMALL) {
                e = self.dis.enext;
            }
            if ee > e * (1.0 + SMALL) {
                e = ee;
            }
            let mut r = 1.0;
            if mtd != 2 {
                if self.dis.thresh >= e * (1.0 - SMALL) {
                    r = 0.0;
                } else {
                    r = (1.0 - self.dis.thresh / e).sqrt();
                }
            }
            let (nld, fl) = if self.dis.imiss != 1 {
                let f = self.hgtfle(e, 60, 2, mtd)?;
                self.dis.enext = f.enext;
                (f.nle, f.fle)
            } else {
                let mut fl = [0.0; 66];
                fl[1] = 1.0;
                fl[2] = 0.0;
                (1, fl)
            };
            self.dis.en = e;
            let b = r * (awr / self.dis.arat).sqrt();
            let g = b * self.dis.arat;
            self.dis.cn = 0.0;
            let test = 1.0 / 1_000_000.0;
            let mut angle_integration = (self.dis.awp - 1.0).abs() > test;
            if !angle_integration {
                // Compute ebar.
                let mut wbar = fl[2];
                if wbar > QP64[63] {
                    wbar = QP64[63];
                }
                self.dis.cn = (1.0 + 2.0 * b * wbar + b * b) * self.dis.afact;
                angle_integration = self.idame != 0 && e >= self.dis.enx * (1.0 - SMALL);
            }
            if angle_integration {
                // 120: angle integration by Gauss-Legendre.
                let mut d = 0.0;
                let z = self.dis.z;
                for iq in 0..64 {
                    let u = QP64[iq];
                    let p = legndr1(u, nld);
                    let mut f = 0.0;
                    for il in 1..=nld {
                        f += (2 * il - 1) as f64 * fl[il] * p[il] / 2.0;
                    }
                    let e2 = e * (1.0 - 2.0 * g * u + g * g) * self.dis.afact / self.dis.arat;
                    d += QW64[iq] * f * self.df(e2, z, awr, z, awr);
                }
                if d < 0.0 {
                    d = 0.0;
                }
                self.dis.damn = d;
            }
        }
        // 130: interpolate.
        let ce = terp1(self.dis.el, self.dis.cl, self.dis.en, self.dis.cn, ee, 2);
        *ebar = ee * ce;
        if self.idame > 0 {
            *dame = terp1(
                self.dis.el,
                self.dis.daml,
                self.dis.en,
                self.dis.damn,
                ee,
                2,
            );
        }
        Ok(())
    }
}
