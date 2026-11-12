// Translated from NJOY2016.80 `src/groupr.f90`: `getaed` (:9874-10239), `aedi`
// (:10241-10284) and `getff` label 700 (:7385-7412, thermal `mfd = 6`).
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under AGPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! **Thermal angle-energy distributions** (`MT = 221-250`, `mfd = 6`): `getaed`
//! on the THERMR MF=6 section of the PENDF.
//!
//! Three branches, chosen by `itt = -LIP` and `law`: incoherent inelastic
//! (`itt = 1`, law 1, E-E'-mu ordering, `aedi` interpolation), incoherent
//! elastic (`itt = 2`), coherent elastic (`itt > 2`, Bragg edges rebuilt from
//! the MF=3 cross section). Law 7 is refused upstream too ("not coded").
//! The `save`d variables of `getaed` (`ie`, the low/high LIST records, `elo`,
//! `ehi`, `nlo`, `nhi`) are the fields of [`ThermalFeed`].

use crate::endf::gety1::Gety1;
use crate::endf::records::{SectionCursor, Tab1};
use crate::endf::tape::Tape;
use crate::groupr::kinematics::shared::legndr;
use crate::NjoyError;

const EMIN: f64 = 1.0e-5;
const EMAXB: f64 = 10.0;
const ETOP: f64 = 1.0e10;
const EPS: f64 = 1.0e-5;
const SHADE: f64 = 0.999999;
const STEP: f64 = 1.1;
const UP: f64 = 1.00001;
const SMALL: f64 = 1.0e-10;

/// `getaed` state for one thermal reaction.
#[derive(Debug, Clone)]
pub struct ThermalFeed {
    itt: i32,
    egn: Vec<f64>,
    nl: usize,
    // law 1: the TAB2's NE and the LIST records, read in order.
    lists: Vec<Vec<f64>>,
    next_list: usize,
    ne: usize,
    ie: usize,
    ncyc: usize,
    nu: usize,
    lo: Vec<f64>,
    hi: Vec<f64>,
    elo: f64,
    ehi: f64,
    nlo: usize,
    nhi: usize,
    // coherent: Bragg edges and weights.
    bragg: Vec<(f64, f64)>,
}

fn list_words(l: &crate::endf::records::List) -> Vec<f64> {
    let h = &l.head;
    let mut v = vec![h.c1, h.c2, f64::from(h.l1), f64::from(h.l2), f64::from(h.n1), f64::from(h.n2)];
    v.extend_from_slice(&l.data);
    v
}

/// `aedi`: Legendre components at `ee` from the LIST `aa` (0-based words).
fn aedi(ee: f64, nl: usize, aa: &[f64]) -> Vec<f64> {
    let ncyc = aa[5].round() as usize;
    let nw = aa[4].round() as usize;
    let np = nw / ncyc;
    let nu = ncyc - 2;
    let mut fl = vec![0.0f64; nl];
    // Fortran aa(k) = aa[k - 1].
    let a = |k: usize| aa[k - 1];
    let mut ip = 0usize;
    let mut i = 1usize;
    while ip == 0 && i < np {
        if ee <= a(7 + ncyc * i) {
            ip = i;
        }
        i += 1;
    }
    if ip > 0 {
        let d = a(7 + ncyc * ip) - a(7 + ncyc * (ip - 1));
        let f1 = (a(7 + ncyc * ip) - ee) / d;
        let f2 = (ee - a(7 + ncyc * (ip - 1))) / d;
        for iu in 1..=nu {
            let u = a(8 + iu + ncyc * (ip - 1));
            let p = legndr(u, nl);
            for il in 0..nl {
                fl[il] += f1 * a(8 + ncyc * (ip - 1)) * p[il] / nu as f64;
            }
            let u = a(8 + iu + ncyc * ip);
            let p = legndr(u, nl);
            for il in 0..nl {
                fl[il] += f2 * a(8 + ncyc * ip) * p[il] / nu as f64;
            }
        }
    }
    fl
}

impl ThermalFeed {
    /// `getaed(e = 0)`: read MF=6/`mt` of `pend` (and MF=3 for the Bragg edges).
    ///
    /// # Errors
    /// [`NjoyError::SectionNotFound`], [`NjoyError::NotPorted`] for law 7.
    pub fn new(pend: &Tape, mat: i32, mt: i32, egn: &[f64], nl: usize) -> Result<Self, NjoyError> {
        let sec = pend.section(mat, 6, mt).ok_or(NjoyError::SectionNotFound { mat, mf: 6, mt })?;
        let mut cur = SectionCursor::new(&sec.rows);
        let _head = cur.read_cont()?;
        let t: Tab1 = cur.read_tab1()?;
        let itt = -t.head.l1;
        let law = t.head.l2;
        let mut f = Self {
            itt,
            egn: egn.to_vec(),
            nl,
            lists: Vec::new(),
            next_list: 0,
            ne: 0,
            ie: 1,
            ncyc: 0,
            nu: 0,
            lo: Vec::new(),
            hi: Vec::new(),
            elo: 0.0,
            ehi: 0.0,
            nlo: 0,
            nhi: 0,
            bragg: Vec::new(),
        };
        if law == 1 {
            let t2 = cur.read_tab2()?;
            f.ne = t2.head.n2 as usize;
            for _ in 0..f.ne {
                f.lists.push(list_words(&cur.read_list()?));
            }
            f.lo = f.lists[0].clone();
            f.next_list = 1;
            f.elo = f.lo[1];
            f.ncyc = f.lo[5].round() as usize;
            f.nu = f.ncyc - 2;
            f.nlo = f.lo[4].round() as usize / f.ncyc;
            f.ehi = 0.0;
        } else if law == 7 {
            return Err(NjoyError::NotPorted("getaed: thermal mf6/law7 not coded (upstream does not code it either)"));
        } else {
            // coherent elastic: store Bragg edges from the MF=3 cross section.
            let s3 = pend.section(mat, 3, mt).ok_or(NjoyError::SectionNotFound { mat, mf: 3, mt })?;
            let mut c3 = SectionCursor::new(&s3.rows);
            let _ = c3.read_cont()?;
            let tab = c3.read_tab1()?;
            let mut g = Gety1::new(&tab);
            let mut enext = EMIN;
            let mut clast = 0.0;
            loop {
                let ebrag = enext;
                let v = g.get(ebrag);
                enext = v.xnext;
                let cnow = ebrag * v.y;
                if enext > EMAXB * (1.0 + SMALL) || (enext - EMAXB).abs() <= EPS {
                    break;
                }
                if cnow - clast > EPS * clast + EPS {
                    f.bragg.push((ebrag, cnow - clast));
                    clast = cnow;
                }
            }
        }
        Ok(f)
    }

    fn read_hi(&mut self) {
        self.hi = self.lists[self.next_list].clone();
        self.next_list += 1;
    }

    /// `ig` (1-based): the group containing `e` (label 490).
    fn in_group(&self, e: f64) -> usize {
        let ngn = self.egn.len() - 1;
        let mut ig = 1;
        while e >= self.egn[ig] * (1.0 - SMALL) && ig != ngn {
            ig += 1;
        }
        ig
    }

    /// `getaed(e > 0)`: `(aed[il][ig], enext, idis)`.
    ///
    /// # Errors
    /// [`NjoyError::InvalidInput`] if the LIST records run out.
    pub fn get(&mut self, e: f64) -> Result<(Vec<Vec<f64>>, f64, i32), NjoyError> {
        let ngn = self.egn.len() - 1;
        let nl = self.nl;
        let mut aed = vec![vec![0.0f64; ngn]; nl];
        if self.itt == 1 {
            if e < self.ehi * (1.0 - SMALL) {
                // label 300
            } else if self.ie == self.ne && e > self.ehi {
                return Ok((aed, ETOP, 0));
            } else if self.ie == self.ne {
                // label 300
            } else {
                loop {
                    if self.ie != 1 {
                        // 205: slide
                        self.lo = std::mem::take(&mut self.hi);
                        self.elo = self.ehi;
                        self.nlo = self.nhi;
                    }
                    if self.next_list >= self.lists.len() {
                        return Err(NjoyError::InvalidInput("getaed: ran out of LIST records".into()));
                    }
                    self.read_hi();
                    self.ehi = self.hi[1];
                    self.nhi = self.hi[4].round() as usize / self.ncyc;
                    self.ie += 1;
                    if !(e >= self.ehi * (1.0 - SMALL) && self.ie < self.ne) {
                        break;
                    }
                }
            }
            return Ok(self.inelastic(e, aed));
        }
        let ig = self.in_group(e);
        if self.itt <= 2 {
            // incoherent elastic
            if e < self.ehi * (1.0 - SMALL) {
            } else if self.ie == self.ne && e > STEP * self.ehi {
                return Ok((aed, ETOP, 0));
            } else if self.ie == self.ne {
            } else {
                if self.ie != 1 {
                    self.lo = std::mem::take(&mut self.hi);
                    self.elo = self.ehi;
                }
                self.ehi = 0.0;
                while e >= self.ehi * (1.0 - SMALL) && self.ie < self.ne {
                    self.read_hi();
                    self.ehi = self.hi[1];
                    self.nhi = 1;
                    self.ie += 1;
                }
            }
            let x1 = self.lo[7];
            let x2 = self.hi[7];
            let r = (e - self.elo) / (self.ehi - self.elo);
            let mut xs = x1 + (x2 - x1) * r;
            xs /= self.nu as f64;
            for iu in 1..=self.nu {
                let u1 = self.lo[7 + iu];
                let u2 = self.hi[7 + iu];
                let u = u1 + (u2 - u1) * r;
                let p = legndr(u, nl);
                for il in 0..nl {
                    aed[il][ig - 1] += xs * p[il];
                }
            }
            let mut enext = self.ehi;
            if enext > self.egn[ig] * (1.0 + SMALL) {
                enext = self.egn[ig];
            }
            let mut idis = 1;
            if (enext - self.ehi).abs() < self.ehi * SMALL {
                idis = 0;
            }
            if enext <= e * (1.0 + SMALL) {
                enext = ETOP;
            }
            return Ok((aed, enext, idis));
        }
        // coherent elastic
        if !self.bragg.is_empty() {
            let mut ib = 0;
            while ib < self.bragg.len() && e >= SHADE * self.bragg[ib].0 {
                let u = 1.0 - 2.0 * self.bragg[ib].0 / e;
                let p = legndr(u, nl);
                for il in 0..nl {
                    aed[il][ig - 1] += self.bragg[ib].1 * p[il] / e;
                }
                ib += 1;
            }
            if aed[0][ig - 1] != 0.0 {
                for il in 1..nl {
                    aed[il][ig - 1] /= aed[0][ig - 1];
                }
            }
        }
        aed[0][ig - 1] = 1.0;
        Ok((aed, self.egn[ig], 1))
    }

    /// Label 300 of `getaed`: the incoherent inelastic group loop.
    fn inelastic(&self, e: f64, mut aed: Vec<Vec<f64>>) -> (Vec<Vec<f64>>, f64, i32) {
        let ngn = self.egn.len() - 1;
        let nl = self.nl;
        let (elo, ehi, ncyc, nlo, nhi) = (self.elo, self.ehi, self.ncyc, self.nlo, self.nhi);
        let (lo, hi) = (&self.lo, &self.hi);
        let (mut k1, mut k2) = (1usize, 1usize);
        let mut fl = vec![0.0f64; nl];
        let mut eg = 0.0f64;
        let eb = elo / 10.0;
        let mut i = 0usize;
        let (mut egp1, mut egp2) = (0.0f64, 0.0f64);
        let (mut fl1, mut fl2) = (vec![0.0f64; nl], vec![0.0f64; nl]);
        loop {
            i += 1;
            let mut egp = self.egn[i];
            if i == ngn {
                egp += 1.0;
            }
            let mut eg1 = egp1;
            let mut eg2 = egp2;
            egp1 = egp - e + elo;
            egp2 = egp + ehi - e;
            if egp1 < eb * (1.0 - SMALL) {
                egp1 = egp * eb / (e - elo + eb);
                egp2 = egp * (ehi - elo + eb) / (e - elo + eb);
            }
            'l345: loop {
                let ek1;
                loop {
                    if k1 > nlo {
                        ek1 = hi[6 + ncyc * (nhi - 1)];
                        break;
                    }
                    let v = lo[6 + ncyc * (k1 - 1)];
                    if v > UP * eg1 {
                        ek1 = v;
                        break;
                    }
                    k1 += 1;
                }
                let mut ei1 = ek1 + e - elo;
                if ek1 < elo / 10.0 {
                    ei1 = ek1 * (e - elo + eb) / eb;
                }
                let ek2;
                loop {
                    if k2 > nhi {
                        break 'l345;
                    }
                    let v = hi[6 + ncyc * (k2 - 1)];
                    if v > UP * eg2 {
                        ek2 = v;
                        break;
                    }
                    k2 += 1;
                }
                let mut ei2 = ek2 - ehi + e;
                if ek2 < ehi - elo + eb {
                    ei2 = ek2 * (e - elo + eb) / (ehi - elo + eb);
                }
                let mut ei = egp;
                if ei1 < ei {
                    ei = ei1;
                }
                if ei2 < ei {
                    ei = ei2;
                }
                if (ei - egp).abs() < egp * SMALL {
                    fl1 = aedi(egp1, nl, lo);
                    fl2 = aedi(egp2, nl, hi);
                } else if (ei - ei1).abs() < ei1 * SMALL {
                    fl1 = aedi(ek1, nl, lo);
                    let mut ee = ek1 + ehi - elo;
                    if ek1 < eb * (1.0 - SMALL) {
                        ee = ek1 * (ehi - elo + eb) / eb;
                    }
                    fl2 = aedi(ee, nl, hi);
                    eg1 = ek1;
                    eg2 = ee;
                } else if (ei - ei2).abs() < ei2 * SMALL {
                    let mut ee = ek2 - ehi + elo;
                    if ee < eb * (1.0 - SMALL) {
                        ee = ek2 * eb / (ehi - elo + eb);
                    }
                    fl1 = aedi(ee, nl, lo);
                    fl2 = aedi(ek2, nl, hi);
                    eg2 = ek2;
                    eg1 = ee;
                }
                let f1 = (ehi - e) / (ehi - elo);
                let f2 = (e - elo) / (ehi - elo);
                for il in 0..nl {
                    let fi = f1 * fl1[il] + f2 * fl2[il];
                    aed[il][i - 1] += (fi + fl[il]) * (ei - eg) / 2.0;
                    fl[il] = fi;
                }
                eg = ei;
                if ei < egp * (1.0 - SMALL) {
                    continue;
                }
                break;
            }
            if i < ngn && (k1 < nlo || k2 < nhi) {
                continue;
            }
            break;
        }
        let mut enext = ehi;
        if enext <= e * (1.0 + SMALL) {
            enext = ETOP;
        }
        (aed, enext, 0)
    }

    /// `getff` label 700 for `e > 0`: the feed handed to `panel`.
    ///
    /// # Errors
    /// From [`ThermalFeed::get`].
    pub fn at(&mut self, e: f64) -> Result<crate::groupr::njoy_panel::FeedOut, NjoyError> {
        let (aed, enext, idis) = self.get(e)?;
        let ngn = self.egn.len() - 1;
        let mut ighi = 1;
        for ig in 1..=ngn {
            if (0..self.nl).any(|il| aed[il][ig - 1].abs() > SMALL) {
                ighi = ig.max(ighi);
            }
        }
        let ff: Vec<Vec<f64>> = (0..self.nl).map(|il| aed[il][..ighi].to_vec()).collect();
        Ok(crate::groupr::njoy_panel::FeedOut { en: enext, idisc: idis, nq: 0, ff, iglo: 1 })
    }
}
