// Translated from NJOY2016.80 `src/groupr.f90`: `getsed` (:11829-12279, the
// analytic-subsection branch), `anased` (:12281-12502, law 9) and `getff`
// label 200 (:7108-7236, neutron continuum transfer matrices, `mfd = 6`).
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under AGPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! **Continuum transfer matrices** (`getff` label 200): the File-4 Legendre
//! feed `getfle` times the File-5 secondary-energy distribution `getsed`.
//!
//! Ported: `getsed`'s analytic subsections with law 9 (evaporation) and
//! `anased` law 9. Not ported (each returns [`NjoyError::NotPorted`]): the
//! tabulated law 1 branch, analytic laws 5, 7, 11, 12 (`erfc`, `hnab`, `gami`,
//! `e1`), `getyld` (`MT = 18-21, 38`), and any reaction with `econst != 0`
//! (`MT = 18, 19, 102, 452, 455, 456`), whose `jconst` yield-only branch is not
//! translated. `econst = 0` makes the `econst` search in `getsed` inert
//! (`ee < econst` never holds), so it is not carried.

use crate::endf::records::{SectionCursor, Tab1};
use crate::endf::tape::Tape;
use crate::errorr::weight::terpa;
use crate::groupr::file4::File4Angular;
use crate::NjoyError;

const EMAX: f64 = 1.0e10;
const SMALL: f64 = 1.0e-10;
const EBIG: f64 = 1.0e8;
const XMIN: f64 = 1.0e-3;
const ETEST: f64 = 1.1e-5;

/// A TAB1 packed as ENDF words `c1 c2 l1 l2 nr np (nbt int)*nr (x y)*np`.
fn pack(t: &Tab1) -> Vec<f64> {
    let mut v = vec![t.head.c1, t.head.c2, f64::from(t.head.l1), f64::from(t.head.l2), t.interp.len() as f64, t.pairs.len() as f64];
    for &(n, i) in &t.interp {
        v.push(f64::from(n));
        v.push(f64::from(i));
    }
    for &(x, y) in &t.pairs {
        v.push(x);
        v.push(y);
    }
    v
}

/// `getsed` for `nk` subsections: each is the packed probability TAB1 followed
/// by the packed `theta` TAB1 (analytic law).
#[derive(Debug, Clone)]
pub struct SedFeed {
    subs: Vec<Vec<f64>>,
}

/// What `anased` keeps between calls (`save new,theta,xc,rc,bot,...`).
#[derive(Debug, Clone, Default)]
struct AnasedState {
    theta: f64,
    xc: f64,
    bot: f64,
}

/// `anased` for law 9: `(g, enext, idis)`.
fn anased9(st: &mut AnasedState, e: f64, e1: f64, e2: f64, a: &[f64]) -> Result<(f64, f64, i32), NjoyError> {
    let new = e1 < ETEST;
    let mut enext = EMAX;
    let mut idis = 0;
    let u = a[0];
    let de = e - u;
    if e1 > de * (1.0 + SMALL) || de <= 0.0 {
        return Ok((0.0, enext, idis));
    }
    if new {
        let nr = a[4].round() as usize;
        let np = a[5].round() as usize;
        let loct = 6 + 2 * nr + 2 * np;
        let (th, en, idisc) = terpa(&a[loct..], e, &mut 2, &mut 1)?;
        st.theta = th;
        st.xc = de / st.theta;
        if en < enext * (1.0 - SMALL) {
            idis = idisc;
            enext = en;
        }
    }
    let xlo = e1 / st.theta;
    let xhi = if e2 <= de * (1.0 + SMALL) { e2 / st.theta } else { st.xc };
    if new {
        let rc = st.xc;
        st.bot = if rc < XMIN { rc * rc / 2.0 } else { 1.0 - (-rc).exp() * (1.0 + rc) };
    }
    let (r1, r2) = (xlo, xhi);
    let top = if xhi < XMIN { (r2 * r2 - r1 * r1) / 2.0 } else { (-r1).exp() * (r1 + 1.0) - (-r2).exp() * (r2 + 1.0) };
    Ok((top / st.bot, enext, idis))
}

impl SedFeed {
    /// `getsed(e = 0)`: read MF=5/`mt`; returns the feed and `enext`, the
    /// lowest first incident energy of the subsections.
    ///
    /// # Errors
    /// [`NjoyError::SectionNotFound`] without the section; [`NjoyError::NotPorted`]
    /// for laws other than 9.
    pub fn from_tape(tape: &Tape, mat: i32, mt: i32) -> Result<(Self, f64), NjoyError> {
        let sec = tape.section(mat, 5, mt).ok_or(NjoyError::SectionNotFound { mat, mf: 5, mt })?;
        let mut cur = SectionCursor::new(&sec.rows);
        let head = cur.read_cont()?;
        let nk = head.n1 as usize;
        if nk > 20 {
            return Err(NjoyError::InvalidInput("getsed: too many subsections.".into()));
        }
        let mut subs = Vec::new();
        let mut enext = EMAX;
        for _ in 0..nk {
            let p = cur.read_tab1()?;
            if p.head.l2 != 9 {
                return Err(NjoyError::NotPorted("groupr getsed: only analytic law 9 (evaporation) is translated"));
            }
            let elo = p.pairs[0].0;
            if elo < enext {
                enext = elo;
            }
            let th = cur.read_tab1()?;
            let mut v = pack(&p);
            v.extend(pack(&th));
            subs.push(v);
        }
        Ok((Self { subs }, enext))
    }

    /// `getsed(e > 0)` with `nk = 1`: `(sed(1..ng), enext, idis)` for the group
    /// boundaries `eg` (`ng + 1` words).
    ///
    /// # Errors
    /// From `terpa`.
    pub fn get(&self, ed: f64, eg: &[f64]) -> Result<(Vec<f64>, f64, i32), NjoyError> {
        let ng = eg.len() - 1;
        let mut sed = vec![0.0f64; ng];
        let mut enext = EMAX;
        let mut idis = 0;
        for a in &self.subs {
            let (pe, eihi, idisc) = terpa(a, ed, &mut 2, &mut 1)?;
            if (eihi - enext).abs() < enext * SMALL && idisc > idis {
                idis = idisc;
            }
            if eihi < enext * (1.0 - SMALL) {
                idis = idisc;
                enext = eihi;
            }
            if pe > 0.0 {
                let mut st = AnasedState::default();
                let mut ethi = 0.0;
                let mut idisc = 0;
                for ig in 0..ng {
                    let e1 = if ig == 0 { 0.0 } else { eg[ig] };
                    let e2 = if ig == ng - 1 { EBIG } else { eg[ig + 1] };
                    let (s, en, id) = anased9(&mut st, ed, e1, e2, a)?;
                    ethi = en;
                    idisc = id;
                    sed[ig] += s * pe;
                }
                if (ethi - enext).abs() < enext * SMALL && idisc > idis {
                    idis = idisc;
                }
                if ethi < enext * (1.0 - SMALL) {
                    idis = idisc;
                    enext = ethi;
                }
            }
        }
        Ok((sed, enext, idis))
    }
}

/// `getff` label 200 for `mfd = 6` with `econst = 0`.
#[derive(Debug, Clone)]
pub struct ContinuumFeed {
    ang: File4Angular,
    sed: SedFeed,
    egn: Vec<f64>,
    mtd: i32,
    /// `nl` after `getff(e = 0)` (`nl = 1` when `getfle` finds isotropy).
    pub nl: usize,
}

impl ContinuumFeed {
    /// Read MF=4 and MF=5 of `mtd` from the ENDF tape and run `getff(e = 0)`.
    ///
    /// # Errors
    /// [`NjoyError::NotPorted`] for reactions with `econst != 0` or a yield from
    /// `getyld`; reading errors otherwise.
    pub fn new(tape: &Tape, mat: i32, mtd: i32, egn: &[f64], nl_req: usize) -> Result<Self, NjoyError> {
        if matches!(mtd, 18..=21 | 38 | 102 | 452 | 455 | 456) {
            return Err(NjoyError::NotPorted("groupr getff label 200: fission/capture reactions (getyld, econst) are not translated"));
        }
        let ang = File4Angular::from_tape(tape, mat, mtd, crate::groupr::file4::NLD)?;
        let (sed, _) = SedFeed::from_tape(tape, mat, mtd)?;
        // `if (e.eq.0.and.nle.eq.1) nl=1` (`:7154`): isotropy gives nle = 1.
        let nl = if ang.isotropic { 1 } else { nl_req };
        Ok(Self { ang, sed, egn: egn.to_vec(), mtd, nl })
    }

    /// `getff(e > 0)`: `(en, idisc, ff[il][ig], iglo)` with `nq = 0`.
    ///
    /// # Errors
    /// From `getfle` or `getsed`.
    pub fn at(&mut self, e: f64) -> Result<crate::groupr::njoy_panel::FeedOut, NjoyError> {
        let mut enext = EMAX;
        let mut idisc = 0;
        let yld = match self.mtd {
            16 | 24 | 26 | 30 | 11 | 41 => 2.0,
            17 | 25 | 42 => 3.0,
            37 => 4.0,
            _ => 1.0,
        };
        let fl = self.ang.coefficients_at(e, self.nl)?;
        let (en, idis) = (fl.enext, i32::from(fl.idis));
        if (en - enext).abs() < enext * SMALL && idis > idisc {
            idisc = idis;
        }
        if en < enext * (1.0 - SMALL) {
            idisc = idis;
            enext = en;
        }
        let (sede, en, idis) = self.sed.get(e, &self.egn)?;
        if (en - enext).abs() < enext * SMALL && idis > idisc {
            idisc = idis;
        }
        if en < enext * (1.0 - SMALL) {
            idisc = idis;
            enext = en;
        }
        let mut ighi = 1;
        for (ig, &s) in sede.iter().enumerate() {
            if yld * s > 0.0 {
                ighi = ig + 1;
            }
        }
        let ff: Vec<Vec<f64>> = (0..self.nl)
            .map(|il| {
                let f = fl.fle.get(il).copied().unwrap_or(0.0);
                (0..ighi).map(|ig| yld * sede[ig] * f).collect()
            })
            .collect();
        Ok(crate::groupr::njoy_panel::FeedOut { en: enext, idisc, nq: 0, ff, iglo: 1 })
    }
}
