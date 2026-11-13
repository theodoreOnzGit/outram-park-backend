// Translated from NJOY2016.80 `src/groupr.f90`: `getsed` (:11829-12279, the
// analytic-subsection branch), `anased` (:12281-12502, laws 5, 7 and 9) and `getff`
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
//!
//! **CORRECTED 2026-11-13 (gh:#1369):** the tabulated law 1 branch and analytic law 5
//! are ported in [`PhotonSed`] (MF=15 and MF=5), used by the photon feed and the
//! `mfd = 5` spectra (`groupr::spectrum`); ~~[`ContinuumFeed`] (MF=6 continuum,
//! `mfd = 6`) still reads law 9 only through [`SedFeed`].~~
//!
//! **CORRECTED 2026-11-13 (gh:#1369, round 3):** [`ContinuumFeed`] reads law 9
//! through [`SedFeed`] and `LF = 1/5` through [`PhotonSed`] on MF=5. It also carries
//! fission (`MT = 18-21, 38`): `getyld`'s prompt nu (MF=1/MT=456) and the `econst`
//! branch with `jconst`, its spectrum call and its yield-only calls. Still refused:
//! `mfd = 6` for `MT = 102, 452, 455, 456`~~, and a fission spectrum given by law 9~~.
//!
//! **CHANGED 2026-11-13 (gh:#1369, round 5):** [`PhotonSed`]'s analytic subsection
//! carries `anased` laws 5, 7 (simple fission spectrum, with `mathm`'s `erfc`) and 9,
//! so a fission spectrum given by law 7 or 9 is read with its `econst` search. Laws 11
//! (`hnab`) and 12 (`e1`, `gami`) and `LF = 3` are still refused by name.

use petir::mathf::RealMath;
use crate::endf::records::{SectionCursor, Tab1};
use crate::endf::tape::Tape;
use crate::errorr::weight::terpa;
use crate::mixr::mix::sigfig;
use crate::groupr::file4::File4Angular;
use crate::NjoyError;

const EMAX: f64 = 1.0e10;
const SMALL: f64 = 1.0e-10;
const EBIG: f64 = 1.0e8;
const XMIN: f64 = 1.0e-3;
const ETEST: f64 = 1.1e-5;

/// A TAB1 packed as ENDF words `c1 c2 l1 l2 nr np (nbt int)*nr (x y)*np`.
pub(crate) fn pack(t: &Tab1) -> Vec<f64> {
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
        st.bot = if rc < XMIN { rc * rc / 2.0 } else { 1.0 - (-rc).r_exp() * (1.0 + rc) };
    }
    let (r1, r2) = (xlo, xhi);
    let top = if xhi < XMIN { (r2 * r2 - r1 * r1) / 2.0 } else { (-r1).r_exp() * (r1 + 1.0) - (-r2).r_exp() * (r2 + 1.0) };
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

/// The File-5 half of [`ContinuumFeed`]: `getsed`'s analytic law 9 ([`SedFeed`])
/// or its tabulated `LF = 1` / general-evaporation `LF = 5` branch ([`PhotonSed`]
/// on MF=5, which also carries `getsed`'s `econst` search).
#[derive(Debug, Clone)]
enum ContinuumSed {
    Law9(SedFeed),
    Tabulated(PhotonSed),
}

impl ContinuumSed {
    /// `getsed(e > 0)` with `nk = 1`.
    fn get(&self, e: f64, egn: &[f64]) -> Result<(Vec<f64>, f64, i32), NjoyError> {
        match self {
            ContinuumSed::Law9(s) => s.get(e, egn),
            ContinuumSed::Tabulated(s) => s.get(e),
        }
    }
}

/// `getff` label 200 for `mfd = 6` (`groupr.f90:7132-7232`): `getfle` x `getsed`,
/// times `getyld`'s prompt nu (MF=1/MT=456) for fission (`MT = 18-21, 38`), with the
/// `econst` constant-spectrum branch (`econst = ebig` for `MT = 18, 19`,
/// `:5831-5836`, lowered by `getsed` and snapped to a group bound by `jconst`).
#[derive(Debug, Clone)]
pub struct ContinuumFeed {
    ang: File4Angular,
    sed: ContinuumSed,
    egn: Vec<f64>,
    mtd: i32,
    /// `nl` after `getff(e = 0)` (`nl = 1` when `getfle` finds isotropy).
    pub nl: usize,
    /// `getyld` on MF=1/MT=456 for fission reactions.
    nu: Option<crate::groupr::spectrum::NuYield>,
    /// `econst` after initialisation (0: no constant-spectrum range).
    econst: f64,
    /// `jconst`, the last group of the constant-spectrum range.
    jconst: usize,
    /// `getff`'s saved `ifirst`.
    ifirst: usize,
    /// `en` of the initialisation call: the energy of the spectrum call (`:767`).
    en0: f64,
}

impl ContinuumFeed {
    /// Read MF=4 and MF=5 of `mtd` from the ENDF tape and run `getff(e = 0)`.
    /// `ismooth` is passed to `getsed` (it acts on `MT = 455` only).
    ///
    /// # Errors
    /// [`NjoyError::NotPorted`] for `MT = 102, 452, 455, 456` with `mfd = 6`
    /// ~~, for a fission reaction given by analytic law 9,~~ (**CHANGED 2026-11-13,
    /// round 5:** law 9 fission goes through [`PhotonSed`]) and for laws `getsed` does
    /// not translate (`LF = 3, 11, 12`); reading errors otherwise.
    pub fn new(tape: &Tape, mat: i32, mtd: i32, egn: &[f64], nl_req: usize, ismooth: bool) -> Result<Self, NjoyError> {
        if matches!(mtd, 102 | 452 | 455 | 456) {
            return Err(NjoyError::NotPorted("groupr getff label 200: mfd = 6 for MT = 102, 452, 455 or 456"));
        }
        let fission = matches!(mtd, 18..=21 | 38);
        let mut econst = if matches!(mtd, 18 | 19) { EBIG } else { 0.0 };
        let mut enext = EMAX;
        // `getyld(e = 0)` (`:7146-7149`): `enext` is the TAB1's first energy (LNU = 2),
        // `emax` for the polynomial.
        let nu = if fission {
            let (y, _) = crate::groupr::spectrum::NuYield::from_tape(tape, mat, 456)?;
            let en = y.first_energy();
            if en < enext * (1.0 - SMALL) {
                enext = en;
            }
            Some(y)
        } else {
            None
        };
        // `getfle` with `lcd = 1` (`:7152-7153`).
        let ang = File4Angular::from_tape_lab(tape, mat, mtd, crate::groupr::file4::NLD)?;
        let en = ang.first_energy();
        if en < enext * (1.0 - SMALL) {
            enext = en;
        }
        // `getsed(e = 0)`: non-fission law 9 through `SedFeed`, `LF = 1/5/7/9` through
        // `PhotonSed`.
        let lf = {
            let sec = tape.section(mat, 5, mtd).ok_or(NjoyError::SectionNotFound { mat, mf: 5, mt: mtd })?;
            let mut cur = SectionCursor::new(&sec.rows);
            let _ = cur.read_cont()?;
            cur.read_tab1()?.head.l2
        };
        // ~~A fission spectrum given by law 9 was refused~~ **CHANGED 2026-11-13 (gh:#1369
        // round 5):** it goes through `PhotonSed`, which carries the `econst` search and
        // now `anased` laws 7 and 9; non-fission law 9 stays on `SedFeed`.
        let (sed, en) = if lf == 9 && !fission {
            let (s, en) = SedFeed::from_tape(tape, mat, mtd)?;
            (ContinuumSed::Law9(s), en)
        } else {
            let (s, en, ec) = PhotonSed::from_tape_mf(tape, mat, 5, mtd, egn, econst, ismooth)?;
            econst = ec;
            (ContinuumSed::Tabulated(s), en)
        };
        if en < enext * (1.0 - SMALL) {
            enext = en;
        }
        // `jconst` (`:7168-7175`), with `egn(ig)*(1+small)`.
        let mut jconst = 0usize;
        if econst != 0.0 {
            let ngn1 = egn.len();
            for ig in 1..=ngn1 {
                if econst > egn[ig - 1] * (1.0 + SMALL) {
                    jconst = ig - 1;
                }
            }
            econst = egn[jconst];
            if jconst <= 2 {
                econst = 0.0;
            }
        }
        // `if (e.eq.0.and.nle.eq.1) nl=1` (`:7154`): isotropy gives nle = 1.
        let nl = if ang.isotropic { 1 } else { nl_req };
        Ok(Self { ang, sed, egn: egn.to_vec(), mtd, nl, nu, econst, jconst, ifirst: 0, en0: enext })
    }

    /// `jconst` when the constant-spectrum branch is active (`econst != 0`), else 0.
    pub fn jconst(&self) -> usize {
        if self.econst != 0.0 { self.jconst } else { 0 }
    }

    /// `en` of the initialisation call: the energy of the spectrum call (`:767`).
    pub fn initial_energy(&self) -> f64 {
        self.en0
    }

    /// `getff(e > 0)`: `(en, idisc, ff[il][ig], iglo)` with `nq = 0`.
    ///
    /// The first call below `econst` is the spectrum (label 260: `sede` alone,
    /// from the first nonzero group, or group 1 for `MT = 18`); later calls below
    /// it return the yield alone with `en = econst`, a discontinuity (label 230).
    ///
    /// # Errors
    /// From `getyld`, `getfle` or `getsed`.
    pub fn at(&mut self, e: f64) -> Result<crate::groupr::njoy_panel::FeedOut, NjoyError> {
        let mut enext = EMAX;
        let mut idisc = 0;
        self.ifirst += 1;
        let below = e < self.econst * (1.0 - SMALL);
        if below && self.ifirst == 1 {
            // `go to 202`, then label 205 -> 230 -> 260: the spectrum.
            let (sede, _, _) = self.sed.get(e, &self.egn)?;
            const SMIN: f64 = 1.0e-9;
            let ngn = sede.len();
            let iglo = if self.mtd == 18 { 1 } else { (1..=ngn).find(|&ig| sede[ig - 1] > SMIN / 1000.0).unwrap_or(0) };
            let (iglo, ff) = if iglo == 0 { (1, vec![0.0]) } else { (iglo, sede[iglo - 1..].to_vec()) };
            return Ok(crate::groupr::njoy_panel::FeedOut { en: enext, idisc, nq: 0, ff: vec![ff], iglo: iglo as i32 });
        }
        let mut yld = match self.mtd {
            16 | 24 | 26 | 30 | 11 | 41 => 2.0,
            17 | 25 | 42 => 3.0,
            37 => 4.0,
            _ => 1.0,
        };
        if let Some(nu) = &mut self.nu {
            let (y, en, idis) = nu.at(e)?;
            yld = y;
            if en < enext * (1.0 - SMALL) {
                idisc = idis;
                enext = en;
            }
        }
        let fl = self.ang.coefficients_at(e, self.nl)?;
        if below {
            // label 230: yield only (`getfle` was still called, as upstream calls it).
            return Ok(crate::groupr::njoy_panel::FeedOut { en: self.econst, idisc: 1, nq: 0, ff: vec![vec![yld]], iglo: 1 });
        }
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
    /// For an analytic `LF = 5` subsection (general evaporation) in place of the
    /// tabulated part (`e_interp`, `energies` then empty).
    lf5: Option<Lf5>,
}

/// The analytic part of a subsection (`anased`, `groupr.f90:12281-12502`): the law
/// `lf` (5, 7 or 9), `U` (the probability TAB1's C1), the packed `theta(E)` TAB1 and,
/// for law 5 only, `g(x)`.
///
/// ~~`LF = 5` only~~ **CHANGED 2026-11-13 (gh:#1369 round 5):** laws 7 (simple fission
/// spectrum, `:12364-12374`) and 9 (evaporation, `:12376-12387`) as well. Laws 11 and
/// 12 (`hnab`, `e1`, `gami`) are refused when the section is read.
#[derive(Debug, Clone)]
struct Lf5 {
    lf: i32,
    u: f64,
    theta: Vec<f64>,
    g_interp: Vec<(u32, u32)>,
    g_pairs: Vec<(f64, f64)>,
}

/// `anased`'s saved `theta`, `xc`, `bot` (`save new,theta,xc,rc,bot,ca,loct`).
#[derive(Debug, Clone, Copy, Default)]
struct Lf5State {
    theta: f64,
    xc: f64,
    bot: f64,
}

impl Lf5 {
    /// `anased` at incident `e` over `[e1, e2]`: `(g, enext, idis)`.
    /// `new` (`e1 < etest`) re-interpolates `theta` (`:12318-12331`) and, for laws 7
    /// and 9, recomputes `bot`.
    fn anased(&self, st: &mut Lf5State, e: f64, e1: f64, e2: f64) -> Result<(f64, f64, i32), NjoyError> {
        let new = e1 < ETEST;
        let (mut enext, mut idis) = (EMAX, 0);
        let de = e - self.u;
        if e1 > de * (1.0 + SMALL) || de <= 0.0 {
            return Ok((0.0, enext, idis));
        }
        if new {
            let (th, en, idisc) = terpa(&self.theta, e, &mut 2, &mut 1)?;
            st.theta = th;
            st.xc = de / st.theta;
            if en < enext * (1.0 - SMALL) {
                idis = idisc;
                enext = en;
            }
        }
        let xlo = e1 / st.theta;
        let xhi = if e2 <= de * (1.0 + SMALL) { e2 / st.theta } else { st.xc };
        let g = match self.lf {
            5 => crate::endf::interp::intega(&self.g_interp, &self.g_pairs, xlo, xhi)? * st.theta,
            7 => anased7(st, new, xlo, xhi),
            _ => {
                // law 9 (`:12376-12387`), as `anased9`.
                let rc = st.xc;
                if new {
                    st.bot = if st.xc < XMIN { rc * rc / 2.0 } else { 1.0 - (-rc).r_exp() * (1.0 + rc) };
                }
                let top = if xhi < XMIN { (xhi * xhi - xlo * xlo) / 2.0 } else { (-xlo).r_exp() * (xlo + 1.0) - (-xhi).r_exp() * (xhi + 1.0) };
                top / st.bot
            }
        };
        Ok((g, enext, idis))
    }
}

/// `anased` law 7, the simple fission spectrum (`groupr.f90:12364-12374`):
/// `bot = rp4 (1 - erfc(sqrt xc)) - sqrt(xc) exp(-xc)` on a `new` call, and
/// `g = (rl exp(-xlo) - rh exp(-xhi) - rp4 (erfc(rh) - erfc(rl))) / bot` with
/// `rl = sqrt(xlo)`, `rh = sqrt(xhi)`, `rp4 = sqrt(pi/4)`. `erfc` is `mathm`'s
/// SLATEC `derfc`, the one BROADR uses ([`crate::broadr::joint::erfc`]).
fn anased7(st: &mut Lf5State, new: bool, xlo: f64, xhi: f64) -> f64 {
    use crate::broadr::joint::erfc;
    let rp4 = (std::f64::consts::PI / 4.0).sqrt();
    if new {
        let rc = st.xc.sqrt();
        let r4 = -st.xc;
        st.bot = rp4 * (1.0 - erfc(rc)) - rc * r4.r_exp();
    }
    let rl = xlo.sqrt();
    let rh = xhi.sqrt();
    let r3 = -xlo;
    let r4 = -xhi;
    let top = rl * r3.r_exp() - rh * r4.r_exp() - rp4 * (erfc(rh) - erfc(rl));
    top / st.bot
}

/// `getsed` for tabulated File-15 (`LF = 1`) subsections, summed (`nk = 1`), with the
/// gamma group boundaries `eg` of the initialisation call.
///
/// Translated from NJOY2016.80 `groupr.f90` `getsed` (:11829-12279): the `lf <= 1`
/// initialisation branch (`tab2io`, `intega` per group, `econst` determination) and the
/// normal-entry tabulated branch (Cartesian, and unit-base for `INT >= 21`;
/// "corresponding points" `INT 11-15` return zeros as upstream). ~~Not translated: analytic
/// `LF > 1` subsections here (the neutron `LF = 9` path is [`SedFeed`]), the `mt = 455`
/// `sqrt(E)` extension of the lowest delayed bin,~~ **CORRECTED 2026-11-13 (gh:#1369):**
/// the analytic `LF = 5` subsection (`anased` law 5), the `mt = 455` `sqrt(E)` extension,
/// separate subsections (`nk > 1`), the upscatter fold and file 5 (`mfd = 5`,
/// [`PhotonSed::from_tape_mf`]) are translated. Not translated: analytic ~~`LF = 7, 9, 11,
/// 12`~~ `LF = 11, 12` here (**CHANGED 2026-11-13, round 5:** laws 7 and 9 are
/// translated; non-fission `mfd = 6` law 9 still uses [`SedFeed`]), and an incident
/// energy above the last table energy, where upstream reads past its stored table.
#[derive(Debug, Clone)]
pub struct PhotonSed {
    subs: Vec<TabSub>,
    eg: Vec<f64>,
    /// The file read: 15 (photons) or 5 (neutrons, `mfd = 5` spectra).
    mf: i32,
    /// The section's MT (the upscatter fold and the delayed-bin extension test it).
    mt: i32,
}

/// `getsed`'s `ismooth` extension of the lowest delayed bin (`groupr.f90:11971-11994`,
/// `mtd = 455`, a TAB1 whose first INT is 1): while the second energy is above
/// `ex = 40` eV, split the first bin at `fx x2`. `fx = .8409` is a default-real
/// (single-precision) literal assigned to a `real(kr)`, so it is `0.8409f32` widened.
///
/// # Errors
/// [`NjoyError::NotPorted`] for a TAB1 with more than one interpolation range: upstream
/// addresses the pairs as `tmp(m1+8..)`, which assumes `NR = 1`.
fn extend_lowest_delayed_bin(interp: &mut Vec<(u32, u32)>, pairs: &mut Vec<(f64, f64)>) -> Result<(), NjoyError> {
    if interp.first().map(|r| r.1) != Some(1) {
        return Ok(());
    }
    if interp.len() != 1 {
        return Err(NjoyError::NotPorted("groupr getsed: delayed-bin sqrt(E) extension of a TAB1 with NR > 1"));
    }
    let ex = 40.0f64;
    let fx = f64::from(0.8409f32);
    while pairs.len() >= 2 && pairs[1].0 > ex {
        let val = pairs[0].1;
        let y1 = sigfig(fx.sqrt() * val, 7, 0);
        let y2 = sigfig((1.0 - fx * fx.sqrt()) * val / (1.0 - fx), 7, 0);
        let x2 = fx * pairs[1].0;
        pairs[0].1 = y1;
        pairs.insert(1, (x2, y2));
    }
    // `tmp(m1+5) = mm`, `tmp(m1+6) = mm`: one range, NBT = NP.
    interp[0].0 = pairs.len() as u32;
    Ok(())
}

impl PhotonSed {
    /// `getsed(e = 0)` on `MF=15`, `mt`: returns the feed, `enext` (lowest first
    /// energy) and the lowered `econst` (`econst` in, as `getsed` shares it with
    /// `getgyl` through the module variable).
    ///
    /// # Errors
    /// [`NjoyError::SectionNotFound`], [`NjoyError::NotPorted`] for `LF > 1`.
    pub fn from_tape(tape: &Tape, mat: i32, mt: i32, eg: &[f64], econst: f64) -> Result<(Self, f64, f64), NjoyError> {
        Self::from_tape_mf(tape, mat, 15, mt, eg, econst, false)
    }

    /// `getsed(e = 0)` on file `mf` (15, or 5 for the `mfd = 5` spectra): as
    /// [`PhotonSed::from_tape`], with `ismooth` switching on the lowest-delayed-bin
    /// extension for `mt = 455` ([`extend_lowest_delayed_bin`]).
    ///
    /// # Errors
    /// [`NjoyError::SectionNotFound`], [`NjoyError::NotPorted`] for `LF > 1`.
    pub fn from_tape_mf(tape: &Tape, mat: i32, mf: i32, mt: i32, eg: &[f64], mut econst: f64, ismooth: bool) -> Result<(Self, f64, f64), NjoyError> {
        const EPS: f64 = 1.0e-3;
        let sec = tape.section(mat, mf, mt).ok_or(NjoyError::SectionNotFound { mat, mf, mt })?;
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
            // ~~LF = 1 and 5 only~~ **CHANGED 2026-11-13 (gh:#1369 round 5):** analytic
            // laws 7 and 9 too. `LF = 3` (discrete) and laws 11, 12 are still refused.
            if !matches!(p.head.l2, 1 | 5 | 7 | 9) {
                return Err(NjoyError::NotPorted("groupr getsed: only tabulated LF = 1 and analytic LF = 5, 7 and 9 spectra are translated here (LF = 11 needs hnab, LF = 12 mathm's e1 and gami)"));
            }
            if p.pairs[0].0 < enext {
                enext = p.pairs[0].0;
            }
            if p.head.l2 > 1 {
                // `:11902-11968`: `theta(E)` (its `econst` scan), then for LF = 5 `g(x)`,
                // whose lowest delayed bin `ismooth` extends; no `econst` scan on `g` for
                // LF = 5. Laws 7 and 9 have `theta` alone.
                let th = cur.read_tab1()?;
                for ip in 1..th.pairs.len() {
                    let ((x0, y0), (x1, y1)) = (th.pairs[ip - 1], th.pairs[ip]);
                    let delta = (y1 - y0).abs();
                    if delta >= EPS * y0 {
                        let ee = x0 + EPS * y0 * (x1 - x0) / delta;
                        if ee < econst {
                            econst = ee;
                        }
                    }
                }
                let (g_interp, g_pairs) = if p.head.l2 == 5 {
                    let mut gt = cur.read_tab1()?;
                    if ismooth && mt == 455 {
                        extend_lowest_delayed_bin(&mut gt.interp, &mut gt.pairs)?;
                    }
                    (gt.interp, gt.pairs)
                } else {
                    (Vec::new(), Vec::new())
                };
                let lf5 = Lf5 { lf: p.head.l2, u: p.head.c1, theta: pack(&th), g_interp, g_pairs };
                subs.push(TabSub { prob_interp: p.interp, prob: p.pairs, e_interp: Vec::new(), energies: Vec::new(), lf5: Some(lf5) });
                continue;
            }
            let t2 = cur.read_tab2()?;
            let ne = t2.head.n2 as usize;
            let mut energies = Vec::with_capacity(ne);
            for _ in 0..ne {
                let mut t = cur.read_tab1()?;
                if ismooth && mt == 455 {
                    extend_lowest_delayed_bin(&mut t.interp, &mut t.pairs)?;
                }
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
            subs.push(TabSub { prob_interp: p.interp, prob: p.pairs, e_interp: t2.interp, energies, lf5: None });
        }
        Ok((Self { subs, eg: eg.to_vec(), mf, mt }, enext, econst))
    }

    /// `nktot`: the number of subsections.
    pub fn nk(&self) -> usize {
        self.subs.len()
    }

    /// `getsed(e > 0)` with `nk = 1`: `(sed(1..ng), enext, idis)`.
    ///
    /// # Errors
    /// From `terp1`, or an energy above the last table energy.
    pub fn get(&self, ed: f64) -> Result<(Vec<f64>, f64, i32), NjoyError> {
        let (mut rows, enext, idis) = self.get_nk(ed, false)?;
        Ok((rows.swap_remove(0), enext, idis))
    }

    /// `getsed(e > 0)`: with `separate` (`nk > 1`) one row per subsection
    /// (`sed(ik, ig)`), else the one summed row (`ikt = 1`). Applies the upscatter
    /// fold (`:12225-12236`) for neutron spectra other than fission (`MT = 18-21,
    /// 38`) and delayed neutrons (`MT = 455`).
    ///
    /// # Errors
    /// From `terp1`, or an energy above the last table energy.
    pub fn get_nk(&self, ed: f64, separate: bool) -> Result<(Vec<Vec<f64>>, f64, i32), NjoyError> {
        use crate::endf::interp::{terp1, terpa as terpa_st, IntLaw};
        const EPS: f64 = 1.0e-3;
        let eg = &self.eg;
        let ng = eg.len() - 1;
        let nrow = if separate { self.subs.len() } else { 1 };
        let mut rows = vec![vec![0.0f64; ng]; nrow];
        let (mut enext, mut idis) = (EMAX, 0i32);
        let fold = self.mf != 15 && self.mt != 455 && (self.mt < 18 || (self.mt > 21 && self.mt != 38));
        for (ik, sub) in self.subs.iter().enumerate() {
            let sed = &mut rows[if separate { ik } else { 0 }];
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
            if let Some(a) = &sub.lf5 {
                // analytic subsection (`:12124-12141`): `anased` per group, times `pe`;
                // `enext` from the last group's call. No upscatter fold here.
                let mut st = Lf5State::default();
                let (mut ethi, mut idisc) = (EMAX, 0);
                for ig in 0..ng {
                    let e1 = if ig == 0 { 0.0 } else { eg[ig] };
                    let e2 = if ig == ng - 1 { EBIG } else { eg[ig + 1] };
                    let (s, en, id) = a.anased(&mut st, ed, e1, e2)?;
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
            if fold {
                // upscatter is put into the in-group (`ed <= eg(ig) (1 + eps)`).
                let mut ig = ng;
                while ig > 1 && ed <= eg[ig - 1] * (1.0 + EPS) {
                    sed[ig - 2] += sed[ig - 1];
                    sed[ig - 1] = 0.0;
                    ig -= 1;
                }
            }
            if ehi < enext * (1.0 - SMALL) {
                enext = ehi;
            }
        }
        Ok((rows, enext, idis))
    }
}

#[cfg(test)]
mod analytic_tests {
    use super::*;

    /// Pu-238 ENDF/B-IV (NJOY2016's `tests/resources/t404`, MAT 1050) MF=5/MT=18
    /// (`LF = 7`, `U = -3e7`, `theta = 1.3333e6` constant) and MT=91 (`LF = 9`), as on
    /// the tape.
    const T404_MF5: &str = "\
 test tape                                                           1 0  0    0
 9.42380+ 4 2.36167+ 2          0          0          1          01050 5 18  992
-3.00000+ 7 0.00000+ 0          0          7          1          21050 5 18  993
          2          2                                            1050 5 18  994
 1.00000- 5 1.00000+ 0 2.00000+ 7 1.00000+ 0                      1050 5 18  995
 0.00000+ 0 0.00000+ 0          0          0          1          31050 5 18  996
          3          5                                            1050 5 18  997
 1.00000- 5 1.33330+ 6 1.50000+ 7 1.33330+ 6 2.00000+ 7 1.33330+ 61050 5 18  998
                                                                  1050 5  0  999
 9.42380+ 4 2.36167+ 2          0          0          1          01050 5 91 1000
 1.49000+ 5 0.00000+ 0          0          9          1          21050 5 91 1001
          2          2                                            1050 5 91 1002
 1.20000+ 6 1.00000+ 0 2.00000+ 7 1.00000+ 0                      1050 5 91 1003
 0.00000+ 0 0.00000+ 0          0          0          1          41050 5 91 1004
          4          2                                            1050 5 91 1005
 1.20000+ 6 2.25289+ 5 2.00000+ 6 2.79436+ 5 1.50000+ 7 7.65267+ 51050 5 91 1006
 2.00000+ 7 9.52125+ 5                                            1050 5 91 1007
                                                                  1050 5  0 1008
                                                                  1050 0  0    0
                                                                     0 0  0    0
                                                                    -1 0  0    0
";

    fn tape() -> Tape {
        Tape::read(T404_MF5.as_bytes()).unwrap()
    }

    /// A 12-group structure from 1e-5 eV to 2e7 eV.
    fn groups() -> Vec<f64> {
        vec![1.0e-5, 1.0, 1.0e2, 1.0e4, 1.0e5, 3.0e5, 6.0e5, 1.0e6, 2.0e6, 4.0e6, 8.0e6, 1.4e7, 2.0e7]
    }

    /// `anased` law 9 through [`PhotonSed`] is [`SedFeed`]'s `anased9`, bit for bit, on
    /// MT=91 at five incident energies: the two routes `ContinuumFeed` now chooses
    /// between (fission law 9 on `PhotonSed`, the rest on `SedFeed`) cannot disagree.
    #[test]
    fn law9_through_photon_sed_is_sed_feed_bit_for_bit() {
        let (t, eg) = (tape(), groups());
        let (sf, en1) = SedFeed::from_tape(&t, 1050, 91).unwrap();
        let (ps, en2, _) = PhotonSed::from_tape_mf(&t, 1050, 5, 91, &eg, 0.0, false).unwrap();
        assert_eq!(en1, en2);
        for e in [1.3e6, 2.0e6, 5.5e6, 1.4e7, 2.0e7] {
            let (a, ea, ia) = sf.get(e, &eg).unwrap();
            let (b, eb, ib) = ps.get(e).unwrap();
            assert_eq!((ea, ia), (eb, ib), "enext at {e}");
            for (x, y) in a.iter().zip(&b) {
                assert_eq!(x.to_bits(), y.to_bits(), "group value at {e}: {x} vs {y}");
            }
        }
    }

    /// `anased` law 7 on MT=18: the group integrals sum to 1 (`top` telescopes to
    /// `bot` when the last group reaches `de`), and each equals the integral of the
    /// normalised Maxwellian `sqrt(E') exp(-E'/theta)` over its group, by a
    /// 20 000-panel Simpson rule in `x = sqrt(E'/theta)` (an independent quadrature,
    /// so a sign or a factor in `top` would show). `econst` stays where it was:
    /// `theta` is constant, so the scan finds no change.
    #[test]
    fn law7_groups_are_the_maxwellian_integrals() {
        let (t, eg) = (tape(), groups());
        let (ps, en, econst) = PhotonSed::from_tape_mf(&t, 1050, 5, 18, &eg, EBIG, false).unwrap();
        assert_eq!((en, econst), (1.0e-5, EBIG));
        let theta = 1.3333e6;
        // E' = theta x^2: sqrt(E') exp(-E'/theta) dE' is proportional to x^2 exp(-x^2) dx.
        let simpson = |a: f64, b: f64| {
            let n = 20_000usize;
            let h = (b - a) / n as f64;
            let f = |x: f64| x * x * (-x * x).r_exp();
            let mut s = f(a) + f(b);
            for k in 1..n {
                s += f(a + k as f64 * h) * if k % 2 == 1 { 4.0 } else { 2.0 };
            }
            s * h / 3.0
        };
        for e in [1.0, 1.0e6, 1.4e7] {
            let (sed, _, _) = ps.get(e).unwrap();
            let sum: f64 = sed.iter().sum();
            assert!((sum - 1.0).abs() < 1.0e-12, "sum at {e}: {sum}");
            let de = e + 3.0e7;
            let bot = simpson(0.0, (de / theta).sqrt());
            for ig in 0..sed.len() {
                let e1 = if ig == 0 { 0.0 } else { eg[ig] };
                let e2 = if ig == sed.len() - 1 { de } else { eg[ig + 1] };
                let exact = simpson((e1 / theta).sqrt(), (e2 / theta).sqrt()) / bot;
                assert!((sed[ig] - exact).abs() < 1.0e-10, "group {ig} at {e}: {} vs {exact}", sed[ig]);
            }
        }
    }

    /// `getwtf` `iwt = 4` after `genwtf` (deck 02's card 8c): the three branches, their
    /// `ab`/`ac` normalisation (continuous at `eb` and `ec`), and `enext` stopped at
    /// the breakpoints before `sigfig(., 6, 1)`.
    #[test]
    fn getwtf_iwt4_is_thermal_one_over_e_fission() {
        use crate::groupr::njoy_panel::{genwtf_iwt4, getwtf};
        let p = crate::groupr::weights::ThermalFissionParams { thermal_break_ev: 0.1, thermal_temp_ev: 0.025, fission_break_ev: 0.8208e6, fission_temp_ev: 1.4e6 };
        genwtf_iwt4(&p);
        // At `eb` the thermal branch (`e <= eb`) equals `1/eb`: `ab = 1/(exp(-eb/tb) eb^2)`.
        let (at_eb, _, _) = getwtf(4, 0.1).unwrap();
        assert!((at_eb * 0.1 - 1.0).abs() < 1.0e-14, "{at_eb}");
        let (_, en_lo, _) = getwtf(4, 0.0999).unwrap();
        assert_eq!(en_lo, sigfig(0.1, 6, 1));
        let (w, _, _) = getwtf(4, 10.0).unwrap();
        assert_eq!(w, 0.1);
        let (_, en_c, _) = getwtf(4, 0.8208e6 / 1.005).unwrap();
        assert_eq!(en_c, sigfig(0.8208e6, 6, 1));
        let (below, _, _) = getwtf(4, 0.8208e6).unwrap();
        let (above, en, _) = getwtf(4, 0.8208e6 * 1.0000001).unwrap();
        assert!((below / above - 1.0).abs() < 1.0e-6, "{below} vs {above}");
        assert_eq!(en, sigfig(1.01 * 0.8208e6 * 1.0000001, 6, 1));
    }
}
