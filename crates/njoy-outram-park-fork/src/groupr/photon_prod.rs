// Translated from NJOY2016.80 `src/groupr.f90`: `getgyl` (:10706-10886, the
// `nyl = 1`, `mfd = 13` branch and the `mfd = 12` multiplicity branch with its
// constant-range scan), `getgfl` (:10286-10460, `LTT = 1` or `LI = 1`),
// `getco` (:10462-10575, the coefficient-retrieval branch) and `getff` label 300
// (:7216-7380, photon production matrices, including the `econst` constant-
// spectrum branch with `jconst` and the label-395 yield-only path).
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under AGPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! **Photon production matrices** (`mfd = 16/17`, `getff` label 300): the
//! yield or MF=13 production cross section (`getgyl`) times the MF=14 photon
//! angular Legendre coefficients (`getgfl`), binned into the gamma groups.
//!
//! Ported: a single discrete photon (`nyl = 1`) from MF=13 (the yield is 1 and
//! the cross section is the section's own `MF=3` rate), with MF=14 either
//! isotropic (`LI = 1`) or `LTT = 1` Legendre LISTs; and (`mfd = 16`) MF=12
//! `LO = 1` multiplicities for any number of photons with isotropic MF=14,
//! including the `econst` constant-spectrum branch (below `econst` the feed is
//! the yield alone, `iglo = -1`, and the spectrum is written once as the
//! `ig = 0` record). Not ported ([`NjoyError::NotPorted`]): MF=12 `LO = 2`,
//! several photons with anisotropy (`getgfl`'s sort), `LTT = 2`, and analytic
//! `LF > 1` `getsed` spectra. ~~`getsed` continuum photons~~ **CORRECTED 2026-11-11
//! (gh:#1481)**: tabulated `LF = 1` File-15 photons (`EY = 0`, MT 3, 18, 102 of
//! U-235) are ported, `continuum::PhotonSed`; `econst` starts at `ebig` only for
//! `MT = 18, 19, 102, 452, 455, 456` (`:5831-5836`), as upstream.

use crate::endf::interp::{terp1, terpa, IntLaw};
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

/// One MF=12 `LO = 1` photon sub-section (`getgyl`, `mfd = 12`).
#[derive(Debug, Clone)]
struct Mf12Photon {
    /// `EY` (or `ES` binding energy for `LP = 2`).
    ey: f64,
    /// `LP`: 2 = primary photon (energy shifts with the neutron energy).
    lp: i32,
    interp: Vec<(u32, u32)>,
    pairs: Vec<(f64, f64)>,
}

/// MF=12 multiplicities for `getgyl` with `mfd = 12`.
#[derive(Debug, Clone)]
struct Mf12 {
    awr: f64,
    photons: Vec<Mf12Photon>,
}

/// `ebig` of `groupr`: the starting value of `econst` (`:5792`).
const EBIG: f64 = 1.0e8;

/// `getff` label 300 state for one photon-production reaction.
#[derive(Debug, Clone)]
pub struct PhotonFeed {
    egg: Vec<f64>,
    /// `nl` after the `getgfl` initialisation (`LI = 1` reduces it to 1).
    pub nl: usize,
    eyl: f64,
    ang: Option<GammaAngular>,
    igmin: usize,
    mf12: Option<Mf12>,
    /// `econst` after initialisation (0 = no constant-spectrum range).
    econst: f64,
    /// `jconst`: the last group of the constant-spectrum range.
    jconst: usize,
    /// `ifirst`: number of `e > 0` calls so far.
    ifirst: usize,
    /// `en` returned by the initialisation call (the first call's energy).
    en0: f64,
    /// `getsed` File-15 continuum spectrum when the last photon has `EY = 0`.
    sed: Option<crate::groupr::continuum::PhotonSed>,
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
    /// `getff(e = 0)` for `mfd = 17` (MF=13, one photon) or `mfd = 16` (MF=12
    /// `LO = 1` multiplicities, any number of photons, isotropic MF=14).
    ///
    /// `egn` are the neutron group boundaries (for `jconst`).
    ///
    /// # Errors
    /// [`NjoyError::NotPorted`] outside the ported branch; reading errors.
    pub fn new(tape: &Tape, mat: i32, mtd: i32, mfd: i32, egn: &[f64], egg: &[f64], nl_req: usize) -> Result<Self, NjoyError> {
        if mfd == 16 {
            return Self::new_mf12(tape, mat, mtd, egn, egg, nl_req);
        }
        if mfd != 17 {
            return Err(NjoyError::NotPorted("groupr getff label 300: only mfd = 16 and 17 are translated"));
        }
        let sec = tape.section(mat, 13, mtd).ok_or(NjoyError::SectionNotFound { mat, mf: 13, mt: mtd })?;
        let mut cur = SectionCursor::new(&sec.rows);
        let head = cur.read_cont()?;
        if head.n1 != 1 {
            return Err(NjoyError::NotPorted("groupr getgyl: several MF=13 photons are not translated"));
        }
        let tab = cur.read_tab1()?;
        let eyl = tab.head.c1;
        let (nl, ang) = Self::init_getgfl(tape, mat, mtd, nl_req, 1)?;
        // `igmin`: the group containing the lowest photon energy.
        // `econ = 0` (a continuum photon): `getsed` on MF=15 (`getff` label 300, `:7243-7250`).
        // `econst = 0` for `mfd = 17` (`getgyl`'s `else econst = zero`).
        let sed = if eyl == 0.0 { Some(crate::groupr::continuum::PhotonSed::from_tape(tape, mat, mtd, egg, 0.0)?.0) } else { None };
        let mut igmin = 1usize;
        while eyl >= egg[igmin] {
            igmin += 1;
        }
        Ok(Self { egg: egg.to_vec(), nl, eyl, ang, igmin, mf12: None, econst: 0.0, jconst: 0, ifirst: 0, en0: EMAX, sed })
    }

    /// `getgfl(e = 0)`: returns `(nl, angular data)`; only `LI = 1` (any `nyl`)
    /// and `LTT = 1` with one photon are translated.
    fn init_getgfl(tape: &Tape, mat: i32, mtd: i32, nl_req: usize, nyl: usize) -> Result<(usize, Option<GammaAngular>), NjoyError> {
        let s14 = tape.section(mat, 14, mtd).ok_or(NjoyError::SectionNotFound { mat, mf: 14, mt: mtd })?;
        let mut c14 = SectionCursor::new(&s14.rows);
        let h14 = c14.read_cont()?;
        let (li, ltt, ni) = (h14.l1, h14.l2, h14.n2);
        if li == 1 {
            return Ok((1, None));
        }
        if ltt != 1 || ni != 0 || h14.n1 != 1 || nyl != 1 {
            return Err(NjoyError::NotPorted("groupr getgfl: only LTT = 1 with one anisotropic photon is translated"));
        }
        let t2 = c14.read_tab2()?;
        let ne = t2.head.n2 as usize;
        let mut lists = Vec::with_capacity(ne);
        for _ in 0..ne {
            let l = c14.read_list()?;
            lists.push((l.head.c2, l.data));
        }
        Ok((nl_req, Some(GammaAngular { lists, interp: t2.interp })))
    }

    /// `getff(e = 0)` for `mfd = 16`: `getgyl`'s MF=12 read and constant-range
    /// scan, then `getff`'s `jconst` search.
    fn new_mf12(tape: &Tape, mat: i32, mtd: i32, egn: &[f64], egg: &[f64], nl_req: usize) -> Result<Self, NjoyError> {
        let sec = tape.section(mat, 12, mtd).ok_or(NjoyError::SectionNotFound { mat, mf: 12, mt: mtd })?;
        let mut cur = SectionCursor::new(&sec.rows);
        let head = cur.read_cont()?;
        if head.l1 == 2 {
            return Err(NjoyError::NotPorted("getgyl: lo=2 not coded (upstream errors too)"));
        }
        let (awr, nyl) = (head.c2, head.n1 as usize);
        if nyl == 0 {
            return Err(NjoyError::InvalidInput("getgyl: MF=12 with no photons".into()));
        }
        // `nyl > 1`: the first TAB1 is the total multiplicity (skipped, but its first
        // energy seeds `enext`); `nyl = 1`: the single TAB1 is the photon itself.
        let first = cur.read_tab1()?;
        let mut en0 = first.pairs[0].0;
        let mut photons = Vec::with_capacity(nyl);
        if nyl == 1 {
            photons.push(Mf12Photon { ey: first.head.c1, lp: first.head.l1, interp: first.interp, pairs: first.pairs });
        } else {
            for _ in 0..nyl {
                let t = cur.read_tab1()?;
                if t.pairs[0].0 < en0 {
                    en0 = t.pairs[0].0;
                }
                photons.push(Mf12Photon { ey: t.head.c1, lp: t.head.l1, interp: t.interp, pairs: t.pairs });
            }
        }
        // `getgyl` constant range at low energies (`:10791-10808`).
        // `econst = ebig` only for `mtd = 18, 19, 452, 455, 456, 102` (`groupr.f90:5831-5836`).
        let mut econst = if matches!(mtd, 18 | 19 | 102 | 452 | 455 | 456) { EBIG } else { 0.0 };
        let mut i = 0usize;
        let mut idone = false;
        while i < nyl && !idone {
            let p = &photons[i];
            i += 1;
            let np = p.pairs.len();
            let mut e = 0.0f64;
            // histogram interpolation: start with `e` at the second energy point
            if p.interp[0].1 == 1 {
                e = p.pairs[1].0;
            }
            for j in 2..=np {
                if p.pairs[j - 1].1 == p.pairs[0].1 {
                    e = p.pairs[j - 1].0;
                }
            }
            if p.lp == 2 && p.ey / 100.0 < e {
                e = p.ey / 100.0;
            }
            if e < econst {
                econst = e;
            }
            if econst == 0.0 {
                idone = true;
            }
        }
        let (nl, ang) = Self::init_getgfl(tape, mat, mtd, nl_req, nyl)?;
        // `econ = eccn(1)`: the LAST photon's `EY` for `nyl > 1`, the only photon's otherwise.
        let econ = photons[nyl - 1].ey;
        // `econ = 0`: `getsed(e = 0)` on MF=15 lowers `econst` further and `enext` (`:7243-7250`).
        let mut sed = None;
        if econ == 0.0 {
            let (ps, en, ec) = crate::groupr::continuum::PhotonSed::from_tape(tape, mat, mtd, egg, econst)?;
            econst = ec;
            if en < en0 * (1.0 - SMALL) {
                en0 = en;
            }
            sed = Some(ps);
        }
        let mut igmin = 1usize;
        while econ >= egg[igmin] {
            igmin += 1;
        }
        let mut jconst = 0usize;
        if econst != 0.0 {
            for ig in 1..=egn.len() {
                if econst > egn[ig - 1] {
                    jconst = ig - 1;
                }
            }
            econst = egn[jconst];
            if jconst <= 2 {
                econst = 0.0;
            }
        }
        Ok(Self { egg: egg.to_vec(), nl, eyl: econ, ang, igmin, mf12: Some(Mf12 { awr, photons }), econst, jconst, ifirst: 0, en0, sed })
    }

    /// `jconst` when the constant-spectrum branch is active (`econst != 0`), else 0.
    pub fn jconst(&self) -> usize {
        if self.econst != 0.0 { self.jconst } else { 0 }
    }

    /// `en` of the initialisation call: the energy of the spectrum call (`:757`).
    pub fn initial_energy(&self) -> f64 {
        self.en0
    }

    /// `getgyl`, normal entry: `(gyl, eyl, enext, idis)` per photon.
    fn getgyl(&self, ed: f64) -> (Vec<f64>, Vec<f64>, f64, i32) {
        let Some(m) = &self.mf12 else { return (vec![1.0], vec![self.eyl], EMAX, 0) };
        let (mut enext, mut idis) = (EMAX, 0i32);
        let n = m.photons.len();
        let (mut gyl, mut eyl) = (vec![0.0; n], vec![0.0; n]);
        let ngi = self.egg.len();
        for (i, p) in m.photons.iter().enumerate() {
            eyl[i] = p.ey;
            if p.lp == 2 {
                eyl[i] += ed * m.awr / (m.awr + 1.0);
            }
            let (y, en, d) = terpa(&p.interp, &p.pairs, ed);
            gyl[i] = y;
            let idisc = i32::from(d);
            if en == enext && idisc > idis {
                idis = idisc;
            }
            if en < enext {
                idis = idisc;
                enext = en;
            }
            if p.lp == 2 && p.ey != 0.0 {
                for j in 1..=ngi {
                    let e = (self.egg[j - 1] - p.ey) * (m.awr + 1.0) / m.awr;
                    if e >= ed && e < enext {
                        idis = 1;
                        enext = e;
                    }
                }
            }
        }
        (gyl, eyl, enext, idis)
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
    /// The first call with `e < econst` is the constant spectrum (`iglo > 0`,
    /// normalised by the yield); later calls below `econst` return the yield
    /// alone (`:395`, `iglo = -1`, `en = econst`, a discontinuity).
    ///
    /// # Errors
    /// From `getgfl`.
    pub fn at(&mut self, e: f64) -> Result<FeedOut, NjoyError> {
        self.ifirst += 1;
        let (mut enext, mut idisc) = (EMAX, 0i32);
        let (gyl, eyl, en, idis) = self.getgyl(e);
        if (en - enext).abs() < enext * SMALL && idis > idisc {
            idisc = idis;
        }
        if en < enext * (1.0 - SMALL) {
            idisc = idis;
            enext = en;
        }
        if e < self.econst * (1.0 - SMALL) && self.ifirst > 1 {
            // label 395: yield only.
            let mut yld = 0.0f64;
            for &g in &gyl {
                yld += g;
            }
            return Ok(FeedOut { en: self.econst, idisc: 1, nq: 0, ff: vec![vec![yld]], iglo: -1 });
        }
        let (gfl, en, idis) = self.gfl(e)?;
        if (en - enext).abs() < enext * SMALL && idis > idisc {
            idisc = idis;
        }
        if en < enext * (1.0 - SMALL) {
            idisc = idis;
            enext = en;
        }
        // `eyle(nyl) /= 0` skips `getsed` (`:7292`).
        let sede = if eyl[eyl.len() - 1] == 0.0 {
            let (sd, en, idis) = self.sed.as_ref().ok_or(NjoyError::NotPorted("groupr getff label 300: continuum photon without File 15"))?.get(e)?;
            if (en - enext).abs() < enext * SMALL && idis > idisc {
                idisc = idis;
            }
            if en < enext * (1.0 - SMALL) {
                idisc = idis;
                enext = en;
            }
            Some(sd)
        } else {
            None
        };
        let ngg = self.egg.len() - 1;
        let iglo = self.igmin;
        let mut ighi = 1usize;
        let mut ff = vec![vec![0.0f64; ngg]; self.nl];
        for ig in 1..=ngg {
            let (epl, eph) = (self.egg[ig - 1], self.egg[ig]);
            for ik in 0..gyl.len() {
                if gyl[ik] == 0.0 {
                    continue;
                }
                if eyl[ik] != 0.0 && (eyl[ik] < epl * (1.0 - SMALL) || eyl[ik] >= eph * (1.0 - SMALL)) {
                    continue;
                }
                let mut tempo = gyl[ik];
                if eyl[ik] == 0.0 {
                    tempo *= sede.as_ref().map_or(1.0, |sd| sd[ig - 1]);
                }
                for il in 0..self.nl {
                    ff[il][ig - 1] += tempo * gfl[il];
                    if ig > ighi {
                        ighi = ig;
                    }
                }
            }
        }
        let mut yld = 0.0f64;
        for &g in &gyl {
            yld += g;
        }
        if e >= self.econst || self.ifirst != 1 {
            let ff = ff.into_iter().map(|r| r[iglo - 1..ighi.max(iglo - 1)].to_vec()).collect();
            return Ok(FeedOut { en: enext, idisc, nq: 0, ff, iglo: iglo as i32 });
        }
        // first call below `econst`: the normalised spectrum.
        let (mut slo, mut shi) = (0usize, 0usize);
        for ig in 1..=ngg {
            ff[0][ig - 1] /= yld;
            if ff[0][ig - 1] >= SMALL {
                if slo == 0 {
                    slo = ig;
                }
                shi = ig;
            }
        }
        let spec = if slo == 0 { Vec::new() } else { ff[0][slo - 1..shi].to_vec() };
        Ok(FeedOut { en: enext, idisc, nq: 0, ff: vec![spec], iglo: slo as i32 })
    }
}
