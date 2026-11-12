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
//! `e1`), `getyld` (`MT = 18-21, 38`), and any MF=6 reaction with `econst != 0`
//! (`MT = 18, 19, 452, 455, 456`), whose `jconst` yield-only branch is not
//! translated here (the `econst` branch of the MF=12 photon feed, `MT = 102`, IS
//! ported, in `photon_prod.rs`). `econst = 0` makes the `econst` search in `getsed` inert
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

// ---------------------------------------------------------------------------
// Tabulated (LF = 1) File-15 photon spectra: `getsed` for `mfd = 15`.
// ---------------------------------------------------------------------------

/// One incident energy of a tabulated subsection: the TAB1 itself (for the unit-base
/// `intega`) and the `ng` group integrals `getsed` stores beside it.
#[derive(Debug, Clone)]
struct TabEnergy {
    /// The TAB1's `E` (`C2`).
    e: f64,
    interp: Vec<(u32, u32)>,
    pairs: Vec<(f64, f64)>,
    /// `intega` of the TAB1 over each gamma group (`:11984-11992`).
    raw: Vec<f64>,
}

/// A tabulated `LF = 1` subsection of `getsed`: probability TAB1, TAB2 of incident
/// energies and the spectra.
#[derive(Debug, Clone)]
struct TabSub {
    prob_interp: Vec<(u32, u32)>,
    prob: Vec<(f64, f64)>,
    e_interp: Vec<(u32, u32)>,
    energies: Vec<TabEnergy>,
}

/// `getsed` for tabulated File-15 (`LF = 1`) subsections, summed (`nk = 1`), with the
/// gamma group boundaries `eg` of the initialisation call.
///
/// Translated from NJOY2016.80 `groupr.f90` `getsed` (:11829-12279): the `lf <= 1`
/// initialisation branch (`tab2io`, `intega` per group, `econst` determination) and the
/// normal-entry tabulated branch (Cartesian, and unit-base for `INT >= 21`;
/// "corresponding points" `INT 11-15` return zeros as upstream). Not translated: analytic
/// `LF > 1` subsections here (the neutron `LF = 9` path is [`SedFeed`]), the `mt = 455`
/// `sqrt(E)` extension of the lowest delayed bin, and an incident energy above the last
/// table energy, where upstream reads past its stored table.
#[derive(Debug, Clone)]
pub struct PhotonSed {
    subs: Vec<TabSub>,
    eg: Vec<f64>,
}

impl PhotonSed {
    /// `getsed(e = 0)` on `MF=15`, `mt`: returns the feed, `enext` (lowest first
    /// energy) and the lowered `econst` (`econst` in, as `getsed` shares it with
    /// `getgyl` through the module variable).
    ///
    /// # Errors
    /// [`NjoyError::SectionNotFound`], [`NjoyError::NotPorted`] for `LF > 1`.
    pub fn from_tape(tape: &Tape, mat: i32, mt: i32, eg: &[f64], mut econst: f64) -> Result<(Self, f64, f64), NjoyError> {
        const EPS: f64 = 1.0e-3;
        let sec = tape.section(mat, 15, mt).ok_or(NjoyError::SectionNotFound { mat, mf: 15, mt })?;
        let mut cur = SectionCursor::new(&sec.rows);
        let head = cur.read_cont()?;
        let nk = head.n1 as usize;
        if nk > 20 {
            return Err(NjoyError::InvalidInput("getsed: too many subsections.".into()));
        }
        let ng = eg.len() - 1;
        let mut enext = EMAX;
        let mut subs = Vec::new();
        for _ in 0..nk {
            let p = cur.read_tab1()?;
            if p.head.l2 != 1 {
                return Err(NjoyError::NotPorted("groupr getsed: only tabulated LF = 1 photon spectra are translated"));
            }
            if p.pairs[0].0 < enext {
                enext = p.pairs[0].0;
            }
            let t2 = cur.read_tab2()?;
            let ne = t2.head.n2 as usize;
            let mut energies = Vec::with_capacity(ne);
            for _ in 0..ne {
                let t = cur.read_tab1()?;
                let mut raw = Vec::with_capacity(ng);
                for ig in 0..ng {
                    let e1 = if ig == 0 { 0.0 } else { eg[ig] };
                    let e2 = if ig == ng - 1 { EBIG } else { eg[ig + 1] };
                    raw.push(crate::endf::interp::intega(&t.interp, &t.pairs, e1, e2)?);
                }
                energies.push(TabEnergy { e: t.head.c2, interp: t.interp, pairs: t.pairs, raw });
            }
            // determine econst (`:12088-12130`)
            let mut ir = 0usize;
            let mut nbt = t2.interp[ir].0 as usize;
            let mut int = t2.interp[ir].1;
            if int == 2 {
                int = 22;
            }
            for i in 2..=ne {
                if i > nbt {
                    ir += 1;
                    nbt = t2.interp[ir].0 as usize;
                    int = t2.interp[ir].1;
                    if int == 2 {
                        int = 22;
                    }
                }
                let (lo, hi) = (&energies[i - 2], &energies[i - 1]);
                let (elo, ehi) = (lo.e, hi.e);
                for ig in 0..ng {
                    let delta = (hi.raw[ig] - lo.raw[ig]).abs();
                    if delta > EPS * lo.raw[ig] {
                        if int == 1 && ehi < econst {
                            econst = ehi;
                        }
                        if int != 1 {
                            let ee = elo + EPS * lo.raw[ig] * (ehi - elo) / delta;
                            if ee < econst {
                                econst = ee;
                            }
                        }
                    }
                }
            }
            subs.push(TabSub { prob_interp: p.interp, prob: p.pairs, e_interp: t2.interp, energies });
        }
        Ok((Self { subs, eg: eg.to_vec() }, enext, econst))
    }

    /// `getsed(e > 0)` with `nk = 1`: `(sed(1..ng), enext, idis)`.
    ///
    /// # Errors
    /// From `terp1`, or an energy above the last table energy.
    pub fn get(&self, ed: f64) -> Result<(Vec<f64>, f64, i32), NjoyError> {
        use crate::endf::interp::{terp1, terpa as terpa_st, IntLaw};
        let eg = &self.eg;
        let ng = eg.len() - 1;
        let mut sed = vec![0.0f64; ng];
        let (mut enext, mut idis) = (EMAX, 0i32);
        for sub in &self.subs {
            let (pe, eihi, d) = terpa_st(&sub.prob_interp, &sub.prob, ed);
            let idisc = i32::from(d);
            if (eihi - enext).abs() < enext * SMALL && idisc > idis {
                idis = idisc;
            }
            if eihi < enext * (1.0 - SMALL) {
                idis = idisc;
                enext = eihi;
            }
            if pe <= 0.0 {
                continue;
            }
            let ne = sub.energies.len();
            let mut nne = 0usize;
            let mut nnt = 0usize;
            let mut nbt = sub.e_interp[0].0 as usize;
            let mut int = sub.e_interp[0].1;
            if int == 2 {
                int = 22;
            }
            let (mut elo, mut ehi) = (0.0f64, 0.0f64);
            while nne < ne && ed > ehi * (1.0 + SMALL) {
                nne += 1;
                if nne > nbt {
                    nnt += 1;
                    nbt = sub.e_interp[nnt].0 as usize;
                    int = sub.e_interp[nnt].1;
                    if int == 2 {
                        int = 22;
                    }
                }
                if nne >= ne {
                    return Err(NjoyError::InvalidInput("getsed: incident energy above the last tabulated energy".into()));
                }
                elo = sub.energies[nne - 1].e;
                ehi = sub.energies[nne].e;
            }
            let (mut klo, mut khi) = (nne.saturating_sub(1), nne);
            let (mut xlo, mut xhi) = (0.0f64, 0.0f64);
            if int > 5 {
                klo = 0;
                khi = 1;
                elo = sub.energies[klo].e;
                ehi = sub.energies[khi].e;
                let last = |k: usize| sub.energies[k].pairs.last().map_or(0.0, |p| p.0);
                xlo = last(klo);
                xhi = last(khi);
                while nne < ne && ed > ehi * (1.0 + SMALL) {
                    klo = khi;
                    elo = ehi;
                    xlo = xhi;
                    khi = klo + 1;
                    if khi >= ne {
                        return Err(NjoyError::InvalidInput("getsed: incident energy above the last tabulated energy".into()));
                    }
                    ehi = sub.energies[khi].e;
                    xhi = last(khi);
                }
            }
            if int >= 21 {
                let jnt = IntLaw::from_code(int - 20);
                let xend = terp1(elo, xlo, ehi, xhi, ed, jnt)?;
                for ig in 0..ng {
                    let e1 = if ig == 0 { 0.0 } else { eg[ig] };
                    let e2 = if ig == ng - 1 { EBIG } else { eg[ig + 1] };
                    let (e1lo, e1hi) = (e1 * xlo / xend, e1 * xhi / xend);
                    let (e2lo, e2hi) = (e2 * xlo / xend, e2 * xhi / xend);
                    let (a, b) = (&sub.energies[klo], &sub.energies[khi]);
                    let flo = crate::endf::interp::intega(&a.interp, &a.pairs, e1lo, e2lo)?;
                    let fhi = crate::endf::interp::intega(&b.interp, &b.pairs, e1hi, e2hi)?;
                    let fe = terp1(elo, flo, ehi, fhi, ed, jnt)?;
                    sed[ig] += pe * fe;
                }
            } else if int < 11 {
                for ig in 0..ng {
                    let s = terp1(elo, sub.energies[klo].raw[ig], ehi, sub.energies[khi].raw[ig], ed, IntLaw::from_code(int))?;
                    sed[ig] += s * pe;
                }
            }
            if ehi < enext * (1.0 - SMALL) {
                enext = ehi;
            }
        }
        Ok((sed, enext, idis))
    }
}
