// Translated from NJOY2016.80 `src/groupr.f90`: `getgyl` (:10706-10886, the
// `nyl = 1`, `mfd = 13` branch), `getgfl` (:10286-10460, `LTT = 1` or `LI = 1`),
// `getco` (:10462-10575, the coefficient-retrieval branch) and `getff` label 300
// (:7216-7380, photon production matrices with `econst = 0`).
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under AGPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! **Photon production matrices** (`mfd = 16/17`, `getff` label 300): the
//! yield or MF=13 production cross section (`getgyl`) times the MF=14 photon
//! angular Legendre coefficients (`getgfl`), binned into the gamma groups.
//!
//! Ported: a single discrete photon (`nyl = 1`) from MF=13 (the yield is 1 and
//! the cross section is the section's own `MF=3` rate), with MF=14 either
//! isotropic (`LI = 1`) or `LTT = 1` Legendre LISTs. Not ported
//! ([`NjoyError::NotPorted`]): MF=12 multiplicities (the `econst` constant-
//! spectrum branch), several photons with anisotropy (`getgfl`'s sort),
//! `LTT = 2`, and `getsed` continuum photons.

use crate::endf::interp::{terp1, IntLaw};
use crate::endf::records::SectionCursor;
use crate::endf::tape::Tape;
use crate::groupr::njoy_panel::FeedOut;
use crate::NjoyError;

const EMAX: f64 = 1.0e10;
const SMALL: f64 = 1.0e-10;
const UP: f64 = 1.1;

/// One photon's File-14 angular data (`getgfl` for `LTT = 1`).
#[derive(Debug, Clone)]
struct GammaAngular {
    /// `(E, a_1..a_NPL)` per incident energy.
    lists: Vec<(f64, Vec<f64>)>,
    /// The TAB2's `(nbt, int)` ranges.
    interp: Vec<(u32, u32)>,
}

/// `getff` label 300 state for one photon-production reaction.
#[derive(Debug, Clone)]
pub struct PhotonFeed {
    egg: Vec<f64>,
    /// `nl` after the `getgfl` initialisation (`LI = 1` reduces it to 1).
    pub nl: usize,
    eyl: f64,
    ang: Option<GammaAngular>,
    igmin: usize,
}

/// `getco`, the coefficient branch: `fl(1) = 1`, `fl(il) = c(il + 5)` up to `NPL + 1`.
fn getco(nl: usize, data: &[f64]) -> Vec<f64> {
    let np = data.len() + 1;
    let mut fl = vec![0.0f64; nl];
    fl[0] = 1.0;
    for il in 2..=nl {
        if il <= np {
            fl[il - 1] = data[il - 2];
        }
    }
    fl
}

impl PhotonFeed {
    /// `getff(e = 0)` for `mfd = 17` (MF=13) with one photon.
    ///
    /// # Errors
    /// [`NjoyError::NotPorted`] outside the ported branch; reading errors.
    pub fn new(tape: &Tape, mat: i32, mtd: i32, mfd: i32, egg: &[f64], nl_req: usize) -> Result<Self, NjoyError> {
        if mfd != 17 {
            return Err(NjoyError::NotPorted("groupr getff label 300: MF=12 multiplicities (econst branch) are not translated"));
        }
        let sec = tape.section(mat, 13, mtd).ok_or(NjoyError::SectionNotFound { mat, mf: 13, mt: mtd })?;
        let mut cur = SectionCursor::new(&sec.rows);
        let head = cur.read_cont()?;
        if head.n1 != 1 {
            return Err(NjoyError::NotPorted("groupr getgyl: several MF=13 photons are not translated"));
        }
        let tab = cur.read_tab1()?;
        let eyl = tab.head.c1;
        // `getgfl(e = 0)`.
        let s14 = tape.section(mat, 14, mtd).ok_or(NjoyError::SectionNotFound { mat, mf: 14, mt: mtd })?;
        let mut c14 = SectionCursor::new(&s14.rows);
        let h14 = c14.read_cont()?;
        let (li, ltt, ni) = (h14.l1, h14.l2, h14.n2);
        let mut nl = nl_req;
        let ang = if li == 1 {
            nl = 1;
            None
        } else {
            if ltt != 1 || ni != 0 || h14.n1 != 1 {
                return Err(NjoyError::NotPorted("groupr getgfl: only LTT = 1 with one anisotropic photon is translated"));
            }
            let t2 = c14.read_tab2()?;
            let ne = t2.head.n2 as usize;
            let mut lists = Vec::with_capacity(ne);
            for _ in 0..ne {
                let l = c14.read_list()?;
                lists.push((l.head.c2, l.data));
            }
            Some(GammaAngular { lists, interp: t2.interp })
        };
        // `igmin`: the group containing the lowest photon energy.
        let mut igmin = 1usize;
        while eyl >= egg[igmin] {
            igmin += 1;
        }
        Ok(Self { egg: egg.to_vec(), nl, eyl, ang, igmin })
    }

    /// Number of gamma groups.
    pub fn ngg(&self) -> usize {
        self.egg.len() - 1
    }

    /// `getgfl(e > 0)` for one photon: `(gfl(1..nl), enext, idis)`.
    fn gfl(&self, ed: f64) -> Result<(Vec<f64>, f64, i32), NjoyError> {
        let nl = self.nl;
        let iso = || {
            let mut v = vec![0.0; nl];
            v[0] = 1.0;
            (v, EMAX, 0)
        };
        let Some(a) = &self.ang else { return Ok(iso()) };
        let np = a.lists.len();
        let enext = EMAX;
        let mut im = 1usize;
        let mut inn = 1usize;
        let mut l = 0usize;
        loop {
            let elo = a.lists[l].0;
            if elo > ed * (1.0 + SMALL) && im == 1 {
                return Ok(iso());
            }
            let ehi = a.lists[l + 1].0;
            im += 1;
            if im > np {
                return Err(NjoyError::InvalidInput("getgfl: desired energy at highest given energy.".into()));
            }
            if im > a.interp[inn - 1].0 as usize {
                inn += 1;
            }
            if ehi <= ed * (1.0 + SMALL) && im < np {
                l += 1;
                continue;
            }
            if im == np && ed > UP * ehi {
                return Err(NjoyError::InvalidInput("getgfl: desired energy at highest given energy.".into()));
            }
            let int = a.interp[inn - 1].1;
            let (alo, ahi) = (getco(nl, &a.lists[l].1), getco(nl, &a.lists[l + 1].1));
            let mut out = vec![0.0; nl];
            for i in 0..nl {
                out[i] = terp1(elo, alo[i], ehi, ahi[i], ed, IntLaw::from_code(int))?;
            }
            if ehi > enext * (1.0 + SMALL) {
                return Ok((out, enext, 0));
            }
            return Ok((out, ehi, i32::from(int == 1)));
        }
    }

    /// `getff(e > 0)`: the feed `ff[il][ig]` from group `iglo`.
    ///
    /// # Errors
    /// From `getgfl`.
    pub fn at(&mut self, e: f64) -> Result<FeedOut, NjoyError> {
        let (gfl, en, idis) = self.gfl(e)?;
        let (mut enext, mut idisc) = (EMAX, 0);
        if (en - enext).abs() < enext * SMALL && idis > idisc {
            idisc = idis;
        }
        if en < enext * (1.0 - SMALL) {
            idisc = idis;
            enext = en;
        }
        let ngg = self.egg.len() - 1;
        let iglo = self.igmin;
        let mut ighi = 1usize;
        let mut ff = vec![vec![0.0f64; ngg]; self.nl];
        for ig in 1..=ngg {
            let (epl, eph) = (self.egg[ig - 1], self.egg[ig]);
            if self.eyl < epl * (1.0 - SMALL) || self.eyl >= eph * (1.0 - SMALL) {
                continue;
            }
            for il in 0..self.nl {
                ff[il][ig - 1] += gfl[il];
            }
            if ig > ighi {
                ighi = ig;
            }
        }
        let ff = ff.into_iter().map(|r| r[iglo - 1..ighi].to_vec()).collect();
        Ok(FeedOut { en: enext, idisc, nq: 0, ff, iglo })
    }
}
