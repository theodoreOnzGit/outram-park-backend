// SPDX-License-Identifier: AGPL-3.0-only
// Translated from NJOY2016.80 `src/errorr.f90` (git commit ac5adf5f33d893e42f2eed7fb286b0d51c7580da):
//   - `subroutine grpav4`, l.5411-5706, the ENDF branch (`nscr5 = 0`);
//   - `subroutine egtlgc`, l.5763-5895; `subroutine epanel`, l.9645-9714 (`mfcov = 34`);
//   - `subroutine matrixin`/`matrixej`, l.10686-10790 (only row 2 of `t`, which is
//     all `res` takes).
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under AGPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! **`grpav4` from the ENDF tape** (gh:#1369): union-group Legendre coefficients of
//! the elastic angular distribution, MF=4/MT=2, for ERRORR `mfcov = 34` without an
//! input GENDF.
//!
//! The coefficients `a_l(E)` are interpolated by `egtlgc` (one record step per call,
//! `terp1` with the TAB2's law) and integrated against the weight flux by `epanel`'s
//! trapezoid, panel by panel, exactly as `grpav` does for a cross section. `u1lele`
//! is row 2 of `matrixej`'s CM-to-lab matrix: `t(2,1) = 2/(3A)`,
//! `t(2,2) = 1 - 6/(10A^2)`, `t(2,i) = (i1/(2i1-1) - (i1+2)/(A^2(2i1+3))) (-1/A)^(i1-1)`.
//!
//! Upstream idioms kept: `egtlgc` never flags a discontinuity, and past its last
//! Legendre energy it returns zeros (with a 1e-5 relative allowance at the last
//! one); the group record is skipped when every coefficient up to `legord` is
//! zero (the last group excepted); `ans(i,2)` is divided by `ans(1,1)` for
//! `i = 1..9`, stored as `plele`, and only then is `ans(1,2)` multiplied by
//! `u1lele(2)` when MF=4 is in the CM frame (`lct4 = 2`).

use petir::mathf::RealMath;

use crate::endf::interp::{terp1, IntLaw};
use crate::endf::records::SectionCursor;
use crate::endf::tape::Tape;
use crate::NjoyError;

use super::weight::WeightSampler;

const MAXLEG: usize = 64;
const DELTA: f64 = 0.999_995;

/// Row 2 of `matrixej`'s transformation matrix, `res(1:10) = t(2,1:10)`
/// (`errorr.f90:10702-10790`, `nm = 20`, so `mm = 30`).
pub fn u1lele_row(awr: f64) -> [f64; 10] {
    let g = 1.0 / awr;
    let mm = 30usize;
    let mut t2 = [0.0f64; 31]; // t(2, 1..30) at [1..=30]
    t2[1] = 2.0 * g / 3.0;
    t2[2] = 1.0 - 6.0 * (g * g) / 10.0;
    let mut mup = 0usize;
    for i in 3..=mm {
        let i1 = i - 1;
        let z1 = i1 as f64;
        let z2 = (i1 + 2) as f64;
        let z3 = (2 * i1 - 1) as f64;
        let z4 = (2 * i1 + 3) as f64;
        let i2 = (i1 - 1) as i32;
        let x1 = (z1 / z3 - z2 * (g * g) / z4) * (-g).r_powi(i2);
        t2[i] = x1;
        if mup == 0 {
            if x1.abs() >= 1.0e-16 {
                continue;
            }
            mup = i1;
        }
        if x1.abs() < 1.0e-32 {
            break;
        }
    }
    let mut res = [0.0; 10];
    res.copy_from_slice(&t2[1..=10]);
    res
}

/// `egtlgc`'s `save`d state (`errorr.f90:5763-5895`) over MF=4/MT=2's Legendre
/// records.
#[derive(Debug, Clone)]
pub struct Egtlgc {
    nbt: Vec<usize>,
    jnt: Vec<u32>,
    intl: u32,
    nnr: usize,
    ne: usize,
    nne: usize,
    nl1: usize,
    nl2: usize,
    e1: f64,
    e2: f64,
    al1: [f64; MAXLEG],
    al2: [f64; MAXLEG],
    lists: Vec<(f64, Vec<f64>)>,
    next: usize,
}

/// `(lct4, u1lele, egtlgc state, thresh)` from `egtlgc(e = 0)` and `gridd`'s MF=4 read.
pub type Mf4Legendre = (i32, [f64; 10], Egtlgc, f64);

impl Egtlgc {
    /// `egtlgc(e = 0)`: read the head, `u1lele` (`matrixin(aww)`; an `LVT = 1`
    /// matrix on the tape is refused), the TAB2 and the first two Legendre records.
    /// Returns the state with `lct4` and `u1lele`, and `thresh = e2`.
    ///
    /// # Errors
    /// [`NjoyError::EndfParse`] for `LTT = 2` (upstream: "not coded for ltt=2") or
    /// fewer than two Legendre records; [`NjoyError::NotPorted`] for `LVT = 1` or
    /// more than 64 coefficients.
    pub fn init(endf: &Tape, matd: i32) -> Result<Mf4Legendre, NjoyError> {
        let sec = endf.section(matd, 4, 2).ok_or(NjoyError::SectionNotFound { mat: matd, mf: 4, mt: 2 })?;
        let mut cur = SectionCursor::new(&sec.rows);
        let head = cur.read_cont()?;
        let aww = head.c2;
        let (lvt, ltt) = (head.l1, head.l2);
        if lvt == 1 {
            return Err(NjoyError::NotPorted("errorr egtlgc: LVT = 1 (transformation matrix on the tape)"));
        }
        let c2 = cur.read_cont()?;
        let lct4 = c2.l2;
        let u1lele = u1lele_row(aww);
        if ltt == 2 {
            return Err(NjoyError::EndfParse("errorr egtlgc: not coded for ltt=2".into()));
        }
        let t2 = cur.read_tab2()?;
        let ne = t2.head.n2.max(0) as usize;
        let nbt: Vec<usize> = t2.interp.iter().map(|&(n, _)| n as usize).collect();
        let jnt: Vec<u32> = t2.interp.iter().map(|&(_, i)| i).collect();
        let mut lists = Vec::with_capacity(ne);
        for _ in 0..ne {
            let l = cur.read_list()?;
            if l.data.len() > MAXLEG {
                return Err(NjoyError::NotPorted("errorr egtlgc: more than 64 Legendre coefficients"));
            }
            lists.push((l.head.c2, l.data));
        }
        if lists.len() < 2 {
            return Err(NjoyError::EndfParse("errorr egtlgc: fewer than two Legendre records".into()));
        }
        let fill = |d: &[f64]| {
            let mut a = [0.0; MAXLEG];
            a[..d.len()].copy_from_slice(d);
            a
        };
        let s = Egtlgc {
            intl: jnt.first().copied().unwrap_or(2),
            nbt,
            jnt,
            nnr: 1,
            ne,
            nne: 2,
            nl1: lists[0].1.len(),
            nl2: lists[1].1.len(),
            e1: lists[0].0,
            e2: lists[1].0,
            al1: fill(&lists[0].1),
            al2: fill(&lists[1].1),
            next: 2,
            lists,
        };
        let thresh = s.e2;
        Ok((lct4, u1lele, s, thresh))
    }

    /// `egtlgc(e > 0)`: `(al(1:10), enext, idis = 0)`.
    ///
    /// # Errors
    /// A Legendre record missing where the TAB2 promised one; a log law on a
    /// non-positive value.
    pub fn get(&mut self, e: f64) -> Result<([f64; 10], f64, i32), NjoyError> {
        let mut al = [0.0f64; 10];
        if e >= self.e2 {
            if self.nne == self.ne && e <= self.e2 * 1.00001 {
                // label 300
            } else if self.nne >= self.ne {
                return Ok((al, self.e2, 0));
            } else {
                self.al1 = self.al2;
                self.nl1 = self.nl2;
                self.e1 = self.e2;
                let (e2, d) = self
                    .lists
                    .get(self.next)
                    .cloned()
                    .ok_or_else(|| NjoyError::EndfParse("errorr egtlgc: MF=4 Legendre records end early".into()))?;
                self.next += 1;
                self.e2 = e2;
                self.nl2 = d.len();
                self.al2 = [0.0; MAXLEG];
                self.al2[..d.len()].copy_from_slice(&d);
                self.nne += 1;
            }
        }
        // label 300
        let n = self.nl1.max(self.nl2).min(10);
        if self.nnr <= self.nbt.len() && self.nne > self.nbt[self.nnr - 1] {
            self.nnr += 1;
            if let Some(&j) = self.jnt.get(self.nnr - 1) {
                self.intl = j;
            }
        }
        let law = IntLaw::from_code(self.intl);
        for (i, a) in al.iter_mut().enumerate().take(n) {
            *a = terp1(self.e1, self.al1[i], self.e2, self.al2[i], e, law)?;
        }
        Ok((al, self.e2, 0))
    }
}

/// `epanel`'s `save`d state for `mfcov = 34` (`errorr.f90:9655-9663`).
#[derive(Debug, Clone, Default)]
struct Panel34 {
    enext: f64,
    flst: f64,
    slst: [f64; 10],
    elast: f64,
    idisc: i32,
    idiscf: i32,
}

impl Panel34 {
    fn merge_next(&mut self, en: f64) {
        if en == self.enext && self.idiscf > self.idisc {
            self.idisc = self.idiscf;
        }
        if en < self.enext {
            self.idisc = self.idiscf;
            self.enext = en;
        }
    }

    /// One panel (`epanel` with `nl = 10`, `nz = 1`, `mfcov = 34`): adds to
    /// `ans1 = ans(1,1)` and `ans2 = ans(1:10,2)`; `ehi` comes back as the panel top.
    fn epanel(&mut self, lg: &mut Egtlgc, wt: &mut WeightSampler, elo: f64, ehi: &mut f64, ans1: &mut f64, ans2: &mut [f64; 10]) -> Result<(), NjoyError> {
        if self.idisc != 0 || elo != self.elast {
            self.elast = elo;
            let (s, enext, idisc) = lg.get(elo)?;
            self.slst = s;
            self.enext = enext;
            self.idisc = idisc;
            let (f, en, idiscf) = wt.flux(elo)?;
            self.flst = f;
            self.idiscf = idiscf;
            self.merge_next(en);
        }
        if self.enext < *ehi {
            *ehi = self.enext;
        }
        let mut ehigh = *ehi;
        if self.idisc > 0 {
            ehigh = *ehi * DELTA;
        }
        let (sig, enext, idisc) = lg.get(ehigh)?;
        self.enext = enext;
        self.idisc = idisc;
        let (flux, en, idiscf) = wt.flux(ehigh)?;
        self.idiscf = idiscf;
        self.merge_next(en);
        let bq = (ehigh - elo) / 2.0;
        *ans1 += (flux + self.flst) * bq;
        for n in 0..10 {
            let rr = (sig[n] * flux + self.slst[n] * self.flst) * bq;
            ans2[n] += rr;
        }
        self.elast = *ehi;
        self.slst = sig;
        self.flst = flux;
        self.merge_next(en);
        Ok(())
    }
}

/// What `grpav4`'s ENDF branch leaves: the `nscr4` records, `plele` and `u1lele`.
#[derive(Debug, Clone)]
pub struct Grpav4Endf {
    /// Per union group, `Some((ans(1,1), ans(1:legord,2)))` (before the 1e-30 cut)
    /// where a record is written.
    pub records: Vec<Option<(f64, Vec<f64>)>>,
    /// `plele(jg, 1:9)`.
    pub plele: Vec<[f64; 9]>,
    /// `u1lele(1:10)`.
    pub u1lele: [f64; 10],
}

/// `grpav4` without a GENDF (`errorr.f90:5522-5534`, `:5604-5633`) for MT=2, on the
/// union grid `un`, weight `weight` (re-initialised as `egtflx(e = 0)` does).
///
/// # Errors
/// [`NjoyError::NotPorted`] for `legord > 9` and for an MT=2 whose second Legendre
/// energy is above the union grid (upstream writes no section then and `rdlgnd`
/// fails); what [`Egtlgc`] and the weight refuse.
pub fn grpav4_endf(endf: &Tape, matd: i32, un: &[f64], weight: &WeightSampler, legord: i32) -> Result<Grpav4Endf, NjoyError> {
    if !(0..=9).contains(&legord) {
        return Err(NjoyError::NotPorted("errorr grpav4: legord above 9"));
    }
    let nunion = un.len() - 1;
    let etop = un[nunion];
    let (lct4, u1lele, mut lg, thresh) = Egtlgc::init(endf, matd)?;
    if thresh > etop {
        return Err(NjoyError::NotPorted("errorr grpav4: mt 2 has threshold gt highest union energy"));
    }
    let mut wt = weight.clone();
    let mut st = Panel34::default();
    let mut records = Vec::with_capacity(nunion);
    let mut plele = Vec::with_capacity(nunion);
    for ig in 0..nunion {
        let mut elo = un[ig];
        let ehi = un[ig + 1];
        let (mut a1, mut a2) = (0.0f64, [0.0f64; 10]);
        let mut enext = ehi;
        loop {
            st.epanel(&mut lg, &mut wt, elo, &mut enext, &mut a1, &mut a2)?;
            if (enext / ehi - 1.0).abs() < 1.0e-9 {
                break;
            }
            elo = enext;
            enext = ehi;
        }
        let mut p = [0.0f64; 9];
        for i in 0..9 {
            a2[i] /= a1;
            p[i] = a2[i];
        }
        plele.push(p);
        if lct4 == 2 {
            a2[0] *= u1lele[1];
        }
        let coef = a2[..legord as usize].to_vec();
        if ig + 1 != nunion && coef.iter().all(|&c| c == 0.0) {
            records.push(None);
        } else {
            records.push(Some((a1, coef)));
        }
    }
    Ok(Grpav4Endf { records, plele, u1lele })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `matrixej`'s row 2 for A = 1 (hydrogen: `mu_lab = 2/3` for isotropic CM,
    /// `t(2,2) = 0.4`), and the large-A limit (`t(2,1) -> 0`, `t(2,2) -> 1`).
    #[test]
    fn u1lele_row_follows_matrixej() {
        let r = u1lele_row(1.0);
        assert_eq!(r[0], 2.0 / 3.0);
        assert_eq!(r[1], 1.0 - 6.0 / 10.0);
        // i = 3: (2/3 - 4/7) * (-1)^1
        assert_eq!(r[2], (2.0 / 3.0 - 4.0 / 7.0) * -1.0);
        let r = u1lele_row(1.0e6);
        assert!(r[0] < 1e-6 && (r[1] - 1.0).abs() < 1e-11);
    }
}
