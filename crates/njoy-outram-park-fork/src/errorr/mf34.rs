// SPDX-License-Identifier: AGPL-3.0-only
// Translated from NJOY2016.80 `src/errorr.f90` (git commit ac5adf5f33d893e42f2eed7fb286b0d51c7580da):
//   - `subroutine errorr`, l.1038-1047 (`need1`/`need2`: is a GENDF required);
//   - `subroutine gridd`, l.1091-1483, `mfcov = 34` branches (l.1138-1158, 1170-1176,
//     1228-1240, 1306, 1375);
//   - `subroutine covcal`, l.1770-2417, `mfcov = 34` branches;
//   - `subroutine grpav4`, l.5411-5706, the GENDF branch (`nscr5 /= 0`, l.5547-5660);
//   - `subroutine musigc`, l.5897-6036; `subroutine rdlgnd`, l.6038-6071;
//   - `subroutine alsigc`, l.5708-5761; `subroutine covout`, l.7018-7788, `mfcov = 34`.
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under AGPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! **ERRORR `mfcov = 34`**: covariances of the elastic angular distribution
//! (`P_l` Legendre coefficients, MF=34/MT=2), written as MF=34/MT=251 (gh:#1369).
//!
//! ```text
//! gridd (mfcov = 34)       -> ltt4/lct4/eltt4 from MF=4/MT=2; eni from the NI
//!                             records of the sub-subsections with L, L1 <= legord
//! egngpn = groups + uniong -> egn, un
//! ngchk + colaps           -> union-group flux (MT=1) and sigma_el (MT=2)
//! grpav4 (GENDF branch)    -> alp: GROUPR's MF=3/MT=251 mubar on the union grid
//! covcal (mfcov = 34)      -> the (legord, ifissp) sub-subsection:
//!                             cov = sum F alp1(jg) alp2(jh), rows times
//!                             (sig flx)(jg) (sig flx)(jh)
//! musigc                   -> coarse mubar (MF=3/MT=251), MF=1 on nout
//! covout + alsigc          -> / (clflx clflx); relative: / (xmu xmu)
//! ```
//!
//! **Scope**: `iread = 0`, `nstan = nin = 0`, ENDF-5/6, MF=34 with MT=2 as its
//! only section that is used (upstream stops at the first other MT), one
//! subsection with `MAT1 = 0`, `MT1 = 2`, every sub-subsection with `NI > 0`.
//! Group constants come from an input GENDF ([`run_mf34_gendf`]): with a GENDF
//! upstream never reads MF=4's Legendre coefficients, and `u1lele`/`plele` stay
//! zero (a module array never set, and `plele = 0.`). Or from a PENDF
//! ([`run_mf34_pendf`]): `grpav` for the flux and `sigma_el`, and `grpav4`'s ENDF
//! branch ([`super::grpav4`]) for the coefficients, `plele` and `u1lele`.

use crate::deck::groupr::GendfText;
use crate::endf::records::SectionCursor;
use crate::endf::tape::Tape;
use crate::mixr::mix::sigfig;
use crate::NjoyError;

use super::colaps::{colaps, FissionWindow};
use super::covcal::{contribution, NiRecord};
use super::gendf::{ngchk, CondensedGendf, GendfTape};
use super::gridd::{merge, uniong};
use super::groups::neutron_group_structure;
use super::grpav::grpav;
use super::grpav4::grpav4_endf;
use super::mf33::{endf_version, pendf_temperature};
use super::weight::{ErrorrWeight, WeightSampler};

const NDIG: i32 = 6;

/// The cards of an `mfcov = 34` run that this path reads.
#[derive(Debug, Clone, PartialEq)]
pub struct Mf34Config {
    /// `matd`.
    pub matd: i32,
    /// `ign` (`1`/`-1` take `user_egn`).
    pub ign: i32,
    /// Card 12b bounds for `ign = +-1`.
    pub user_egn: Option<Vec<f64>>,
    /// `irelco`: 1 relative, 0 absolute.
    pub irelco: i32,
    /// Card 7's `legord` (`l`).
    pub legord: i32,
    /// Card 7's `ifissp` (`l1`; `-1000` when not given: 1).
    pub ifissp: i32,
    /// Card 3's `tempin` (K).
    pub tempin: f64,
}

/// MF=4/MT=2's `ltt`, `lct` and the Legendre-to-tabulated transition energy
/// (`gridd`, `:1138-1158`; `eltt4 = emaxx` unless `ltt = 3`, returned as `None`).
pub fn mf4_flags(endf: &Tape, matd: i32) -> Result<(i32, i32, Option<f64>), NjoyError> {
    let sec = endf.section(matd, 4, 2).ok_or(NjoyError::SectionNotFound { mat: matd, mf: 4, mt: 2 })?;
    let mut cur = SectionCursor::new(&sec.rows);
    let ltt4 = cur.read_cont()?.l2;
    let lct4 = cur.read_cont()?.l2;
    let mut eltt4 = None;
    if ltt4 == 3 {
        let t2 = cur.read_tab2()?;
        let mut c2 = 0.0;
        for _ in 0..t2.head.n2.max(0) {
            c2 = cur.read_list()?.head.c2;
        }
        eltt4 = Some(c2);
    }
    Ok((ltt4, lct4, eltt4))
}

/// One MF=34 sub-subsection: `L`, `L1`, `LCT` and its NI records as `covcal` stages them.
#[derive(Debug, Clone)]
pub struct Mf34Block {
    /// `L` (`ld`).
    pub ld: i32,
    /// `L1` (`ld1`).
    pub ld1: i32,
    /// The NI-type LIST records, every word `sigfig`'d to 6 figures.
    pub recs: Vec<NiRecord>,
    /// The raw LIST heads and data (for `gridd`'s energies).
    pub raw: Vec<(i32, i32, i32, i32, Vec<f64>)>,
}

/// MF=34/MT=2 of `matd`: `(za, awr, blocks)` (`gridd`/`covcal`'s walk).
///
/// # Errors
/// [`NjoyError::NotPorted`] for more than one subsection (upstream: "not coded for
/// nmt1>1 of mf=34"), `MAT1 /= 0` or `MT1 /= 2`, or a sub-subsection with `NI = 0`
/// (upstream writes a null matrix there whatever its `L`).
pub fn read_mf34(endf: &Tape, matd: i32) -> Result<(f64, f64, Vec<Mf34Block>), NjoyError> {
    let sec = endf.section(matd, 34, 2).ok_or(NjoyError::SectionNotFound { mat: matd, mf: 34, mt: 2 })?;
    let mut cur = SectionCursor::new(&sec.rows);
    let head = cur.read_cont()?;
    if head.n2 != 1 {
        return Err(NjoyError::NotPorted("errorr mfcov=34: more than one MF=34/MT=2 subsection (nmt1>1)"));
    }
    let sub = cur.read_cont()?;
    if sub.l1 != 0 || sub.l2 != 2 {
        return Err(NjoyError::NotPorted("errorr mfcov=34: a subsection other than MAT1 = 0, MT1 = 2"));
    }
    let (nl, nl1) = (sub.n1.max(0), sub.n2.max(0));
    let nsub = nl * (nl + 1) / 2; // mth = mt2 (`:1176`)
    let _ = nl1;
    let mut blocks = Vec::new();
    for _ in 0..nsub {
        let ss = cur.read_cont()?;
        let ni = ss.n2;
        if ni <= 0 {
            return Err(NjoyError::NotPorted("errorr mfcov=34: a sub-subsection with NI = 0 (null matrix)"));
        }
        let mut recs = Vec::new();
        let mut raw = Vec::new();
        for _ in 0..ni {
            let l = cur.read_list()?;
            recs.push(NiRecord::from_list(l.head.l1, l.head.l2, l.head.n1, l.head.n2, &l.data));
            raw.push((l.head.l1, l.head.l2, l.head.n1, l.head.n2, l.data));
        }
        blocks.push(Mf34Block { ld: ss.l1, ld1: ss.l2, recs, raw });
    }
    Ok((head.c1, head.c2, blocks))
}

/// `gridd`'s energies for `mfcov = 34` (`:1340-1372`): the NI records of every
/// sub-subsection with `L, L1 <= legord`.
pub fn gridd34(blocks: &[Mf34Block], legord: i32) -> Result<Vec<f64>, NjoyError> {
    let mut eni = Vec::new();
    for b in blocks {
        if b.ld > legord || b.ld1 > legord {
            continue;
        }
        for (l1, lb, n1, n2, data) in &b.raw {
            let nx = (*n2).max(0) as usize;
            if *lb < 5 || *lb == 8 {
                let x: Vec<f64> = (0..nx).map(|i| data.get(2 * i).copied().unwrap_or(0.0)).collect();
                let nl = (*l1).max(0) as usize;
                let nx1 = nx.saturating_sub(nl);
                merge(&x[..nx1], &mut eni, 0.0, 0.0)?;
                merge(&x[nx1..], &mut eni, 0.0, 0.0)?;
            } else {
                merge(&data[..nx.min(data.len())], &mut eni, 0.0, 0.0)?;
                if *lb == 5 {
                    continue;
                }
                let nec = ((*n1 - 1) / (*n2).max(1)).max(0) as usize;
                merge(data.get(nx..nx + nec).unwrap_or(&[]), &mut eni, 0.0, 0.0)?;
            }
        }
    }
    Ok(eni)
}

/// `grpav4`'s GENDF branch (`:5547-5660`): GROUPR's MF=3/MT=251 `mubar`, weighted by
/// its elastic cross section, onto the union grid. Returns `(ans(1,1), ans(1,2))`
/// per union group: the width `ehi - elo` and the P1 coefficient, before the
/// `< 1e-30` cut that the `nscr4` record applies.
///
/// # Errors
/// [`NjoyError::SectionNotFound`] without MF=3/MT=251; [`NjoyError::NotPorted`] when
/// the section has fewer records than groups (upstream would read past its end) or
/// when the group bounds run into a continuation record (`n2 > npage+6`).
pub fn grpav4_gendf(cg: &CondensedGendf, un: &[f64]) -> Result<Vec<(f64, f64)>, NjoyError> {
    const ONEEPS: f64 = 0.999_999_99;
    let sec = cg.section(3, 251).ok_or(NjoyError::SectionNotFound { mat: 0, mf: 3, mt: 251 })?;
    let ge = &cg.egn;
    let nng = ge.len() - 1;
    if nng + 1 + 2 > 306 {
        return Err(NjoyError::NotPorted("errorr grpav4: GENDF group bounds past one binary record (npage)"));
    }
    let nmu = sec.head.n[3].max(0) as usize;
    if sec.groups.len() < nmu {
        return Err(NjoyError::NotPorted("errorr grpav4: MF=3/MT=251 with fewer records than groups"));
    }
    let mut gmu = vec![0.0f64; nng + 1];
    let mut gels = vec![0.0f64; nng + 1];
    for r in sec.groups.iter().take(nmu) {
        let ig = r.ig.max(0) as usize;
        if ig == 0 || ig > nng {
            return Err(NjoyError::EndfParse(format!("errorr grpav4: MF=3/MT=251 group {ig} off the grid")));
        }
        gmu[ig] = r.words.get(1).copied().unwrap_or(0.0);
        gels[ig] = r.words.get(2).copied().unwrap_or(0.0);
    }
    // 1-based ge(n) = ge[n-1]
    let g = |n: usize| ge[n - 1];
    let nunion = un.len() - 1;
    let mut out = Vec::with_capacity(nunion);
    let mut n = 1usize;
    for ig in 0..nunion {
        let (elo, ehi) = (un[ig], un[ig + 1]);
        while n + 1 <= nng + 1 && g(n + 1) / elo < ONEEPS {
            n += 1;
        }
        let mut dele = g(n + 1).min(ehi) - elo;
        let mut emu = dele * gels[n] * gmu[n];
        let mut els = dele * gels[n];
        if g(n + 1) / ehi < ONEEPS {
            n += 1;
            while n + 1 <= nng + 1 && g(n + 1) / ehi < ONEEPS {
                dele = g(n + 1) - g(n);
                emu += dele * gels[n] * gmu[n];
                els += dele * gels[n];
                n += 1;
            }
            dele = ehi - g(n);
            emu += dele * gels[n] * gmu[n];
            els += dele * gels[n];
        }
        out.push((ehi - elo, emu / els));
    }
    Ok(out)
}

/// One union group's `nscr4` record as `grpav4` writes it (`:5650-5661`):
/// `z(7) = ans(1,1)` and `z(8:7+legord)`, a coefficient below 1e-30 written as 0.
/// `None` where `grpav4` wrote no record (the ENDF branch skips a group whose
/// coefficients are all zero, the last group excepted).
type Nscr4 = Vec<Option<(f64, Vec<f64>)>>;

/// `grpav4`'s record of one group from `ans(1,1)` and `ans(1:legord,2)`.
fn nscr4_record(ans11: f64, coef: &[f64]) -> (f64, Vec<f64>) {
    (ans11, coef.iter().map(|&a| if a < 1.0e-30 { 0.0 } else { a }).collect())
}

/// `rdlgnd` (`:6038-6071`) on the `nscr4` records: word `npl + 7` per union group
/// (`npl = 0` is `ans(1,1)`), 0 where no record was written and for `npl > legord`.
fn rdlgnd(nscr4: &Nscr4, legord: i32, npl: i32) -> Vec<f64> {
    if npl > legord || npl < 0 {
        return vec![0.0; nscr4.len()];
    }
    nscr4
        .iter()
        .map(|r| match r {
            None => 0.0,
            Some((a11, c)) => {
                if npl == 0 {
                    *a11
                } else {
                    c.get(npl as usize - 1).copied().unwrap_or(0.0)
                }
            }
        })
        .collect()
}

/// The GENDF branch's records: every group, `ans(1,1) = ehi - elo`, `ans(1,2)` the
/// collapsed mubar and `ans(i,2) = 0` for `i > 1`.
fn nscr4_from_gendf(ans: &[(f64, f64)], legord: i32) -> Nscr4 {
    ans.iter()
        .map(|&(w, a1)| {
            let mut c = vec![0.0; legord.max(0) as usize];
            if let Some(x) = c.first_mut() {
                *x = a1;
            }
            Some(nscr4_record(w, &c))
        })
        .collect()
}

/// `covcal`'s rows for the `(legord, ifissp)` sub-subsection (`:2062-2311`):
/// `(jg, b(1:jgend))`.
fn covcal34(block: &Mf34Block, un: &[f64], flx: &[f64], sig: &[f64], alp1: &[f64], alp2: &[f64]) -> Result<Vec<(usize, Vec<f64>)>, NjoyError> {
    for r in &block.recs {
        if r.lb < 0 || r.lb == 3 || r.lb == 4 || r.lb == 7 || r.lb > 8 {
            return Err(NjoyError::EndfParse(format!("errorr covcal: unpermitted for lb={} in mf=34", r.lb)));
        }
        if r.lb < 3 && r.lt > 0 {
            return Err(NjoyError::EndfParse(format!("errorr covcal: lb={} when lt={}", r.lb, r.lt)));
        }
    }
    let nunion = un.len() - 1;
    let mut rows = Vec::new();
    let mut cov = vec![0.0f64; nunion];
    for jg in 0..nunion {
        let eg = un[jg];
        let dun = un[jg + 1] - un[jg];
        for jh in 0..nunion {
            let mut c = 0.0;
            for r in &block.recs {
                c += contribution(r, jg, jh, eg, un[jh], dun, alp1[jg], alp2[jh]);
            }
            cov[jh] = c;
        }
        let mut jgend = cov.iter().rposition(|&c| c != 0.0).map_or(0, |p| p + 1);
        if jgend == 0 {
            if jg + 1 < nunion {
                continue;
            }
            jgend = 1;
        }
        let row = (0..jgend).map(|ij| cov[ij] * (sig[jg] * flx[jg]) * (sig[ij] * flx[ij])).collect();
        rows.push((jg + 1, row));
    }
    Ok(rows)
}

/// What [`run_mf34_gendf`] computed, and the tape `nout` holds.
#[derive(Debug, Clone)]
pub struct Mf34Result {
    /// The output covariance tape as NJOY writes it (`tpidio` label first).
    pub tape: GendfText,
    /// The union grid `un` (`sigfig`'d, as `musigc` leaves it).
    pub un: Vec<f64>,
    /// The user groups `egn`.
    pub egn: Vec<f64>,
    /// The coarse-group mubar `xmu` (MF=3/MT=251).
    pub xmu: Vec<f64>,
}

/// The union-grid inputs [`finish`] needs, from whichever group-constant path.
struct UnionInputs {
    tpid: String,
    tempin: f64,
    un: Vec<f64>,
    egn: Vec<f64>,
    flx: Vec<f64>,
    sig: Vec<f64>,
    /// `grpav4`'s `nscr4` records.
    nscr4: Nscr4,
    /// `plele(jg, 1:9)`: the CM Legendre coefficients per union group (ENDF branch).
    plele: Vec<[f64; 9]>,
    /// `u1lele(1:10)`: the CM-to-lab row of `matrixej` (ENDF branch).
    u1lele: [f64; 10],
}

/// `egngpn`'s user grid and `uniong` (as [`super::mf35::run_mf35`] does).
fn union_grid(cfg: &Mf34Config, eni: &[f64]) -> Result<(Vec<f64>, Vec<f64>), NjoyError> {
    let raw = if cfg.ign.abs() == 1 {
        cfg.user_egn.clone().ok_or_else(|| NjoyError::InvalidInput("errorr: ign=+-1 needs card 12".into()))?
    } else {
        neutron_group_structure(cfg.ign)?
    };
    let mut egn: Vec<f64> = raw.iter().map(|&x| sigfig(x, NDIG, 0)).collect();
    let un = uniong(&egn, eni)?;
    if cfg.ign == -1 {
        egn = un.clone();
    }
    Ok((egn, un))
}

/// ERRORR with `mfcov = 34` on an ENDF tape and an input GENDF (`ngout /= 0`).
///
/// # Errors
/// [`NjoyError::NotPorted`] outside the scope in the module doc; parse errors for
/// malformed records; [`NjoyError::InvalidInput`] when no sub-subsection has
/// `L = legord`, `L1 = ifissp`.
pub fn run_mf34_gendf(endf: &Tape, gendf: &GendfTape, cfg: &Mf34Config) -> Result<Mf34Result, NjoyError> {
    let matd = cfg.matd;
    let (za, awr, blocks, iverf) = prelude(endf, cfg)?;
    let eni = gridd34(&blocks, cfg.legord)?;
    let (egn, un) = union_grid(cfg, &eni)?;
    let cg = ngchk(gendf, matd, cfg.tempin)?;
    let nunion = un.len() - 1;
    let u = colaps(&cg, &un, 0, &FissionWindow { eclo: 0.0, echi: 0.0, efmean: 0.0, igflag: 0 })?;
    let ans = grpav4_gendf(&cg, &un)?;
    let inp = UnionInputs {
        nscr4: nscr4_from_gendf(&ans, cfg.legord),
        plele: vec![[0.0; 9]; nunion],
        u1lele: [0.0; 10],
        tpid: cg.tpid.clone(),
        tempin: u.tempin,
        flx: u.flux(nunion)?,
        sig: u.sigma(2, nunion),
        un,
        egn,
    };
    finish(cfg, za, awr, iverf, &blocks, inp)
}

/// ERRORR with `mfcov = 34` on an ENDF tape and its PENDF (`ngout = 0`): `grpav`
/// for the MF=34 reactions and the flux, `grpav4` from MF=4's Legendre
/// coefficients ([`super::grpav4::grpav4_endf`]).
///
/// # Errors
/// As [`run_mf34_gendf`]; [`NjoyError::EndfParse`] for a PENDF at another
/// temperature (`grpav`'s "unable to find temp").
pub fn run_mf34_pendf(endf: &Tape, pendf: &Tape, cfg: &Mf34Config, weight: &ErrorrWeight) -> Result<Mf34Result, NjoyError> {
    let matd = cfg.matd;
    let (za, awr, blocks, iverf) = prelude(endf, cfg)?;
    let temp = pendf_temperature(pendf, matd, iverf)?;
    if (temp - cfg.tempin).abs() > cfg.tempin / 10_000.0 {
        return Err(NjoyError::EndfParse(format!("errorr::grpav: unable to find temp={:e} on the pendf tape (found {:e})", cfg.tempin, temp)));
    }
    let eni = gridd34(&blocks, cfg.legord)?;
    let (egn, un) = union_grid(cfg, &eni)?;
    // iga: the MF=34 MTs of the dictionary (`errorr.f90:767-773`)
    let iga: Vec<i32> = endf.sections().iter().filter(|s| s.key.mat == matd && s.key.mf == 34).map(|s| s.key.mt).collect();
    let sampler = WeightSampler::new(weight.clone(), cfg.tempin);
    let groups = grpav(pendf, matd, &iga, &un, &sampler)?;
    let g4 = grpav4_endf(endf, matd, &un, &sampler, cfg.legord)?;
    let nscr4 = g4.records.iter().map(|r| r.as_ref().map(|(a11, c)| nscr4_record(*a11, c))).collect();
    let inp = UnionInputs {
        tpid: String::new(),
        tempin: cfg.tempin,
        flx: groups.flux_vector(),
        sig: groups.sigma_for(2),
        nscr4,
        plele: g4.plele,
        u1lele: g4.u1lele,
        un,
        egn,
    };
    finish(cfg, za, awr, iverf, &blocks, inp)
}

/// The ENDF checks every `mfcov = 34` run makes first: `(za, awr, blocks, iverf)`.
fn prelude(endf: &Tape, cfg: &Mf34Config) -> Result<(f64, f64, Vec<Mf34Block>, i32), NjoyError> {
    let iverf = endf_version(endf, cfg.matd)?;
    if iverf <= 4 {
        return Err(NjoyError::NotPorted("errorr: ENDF/B-IV covariance layout (iverf=4)"));
    }
    if cfg.legord < 0 || cfg.ifissp < 0 {
        return Err(NjoyError::InvalidInput("errorr: Legendre order l or l1 for mf34 cannot be less than zero".into()));
    }
    let (za, awr, blocks) = read_mf34(endf, cfg.matd)?;
    Ok((za, awr, blocks, iverf))
}

/// `covcal`, `musigc`, `covout` and `alsigc` on the union-grid inputs; the `nout` tape.
fn finish(cfg: &Mf34Config, za: f64, awr: f64, iverf: i32, blocks: &[Mf34Block], inp: UnionInputs) -> Result<Mf34Result, NjoyError> {
    let matd = cfg.matd;
    let UnionInputs { tpid, tempin, un, mut egn, flx, sig, nscr4, plele, u1lele } = inp;
    let nunion = un.len() - 1;
    if (egn[0] - 1.0e-5).abs() <= 1.0e-9 {
        egn[0] = 1.0e-5;
    }
    // covcal: the (legord, ifissp) sub-subsection
    let block = blocks
        .iter()
        .find(|b| b.ld == cfg.legord && b.ld1 == cfg.ifissp)
        .ok_or_else(|| NjoyError::InvalidInput(format!("errorr covcal: no sub-subsection for l={} and l1={} in mf=34", cfg.legord, cfg.ifissp)))?;
    let alp1 = rdlgnd(&nscr4, cfg.legord, block.ld);
    let alp2 = rdlgnd(&nscr4, cfg.legord, block.ld1);
    let rows = covcal34(block, &un, &flx, &sig, &alp1, &alp2)?;

    // musigc: un and egn sigfig'd in place; coarse flux and mubar
    let egt: Vec<f64> = un.iter().map(|&x| sigfig(x, NDIG, 0)).collect();
    let egn: Vec<f64> = egn.iter().map(|&x| sigfig(x, NDIG, 0)).collect();
    let ngn = egn.len() - 1;
    let inside = |x: f64, ig: usize| !(x < egn[ig] || x >= egn[ig + 1]);
    let alp = rdlgnd(&nscr4, cfg.legord, 1);
    let mut xmu = vec![0.0f64; ngn];
    for (ig, x) in xmu.iter_mut().enumerate() {
        let (mut c, mut abit) = (0.0f64, 0.0f64);
        for jg in 0..nunion {
            if !inside(egt[jg], ig) {
                continue;
            }
            let mut sss0 = 0.0f64;
            for ij in 2..=9 {
                sss0 += plele[jg][ij - 1] * u1lele[ij];
            }
            c += sig[jg] * flx[jg] * (alp[jg] + u1lele[0] + sss0);
            abit += sig[jg] * flx[jg];
        }
        *x = c / abit;
    }

    // covout, isd = 1, the one matrix
    let mut cova = vec![0.0f64; ngn * ngn]; // cova(igp, ig) at [ig * ngn + igp]
    let (ek1, ek2) = (1.0e-10, 1.0e10);
    for (jg, cov) in &rows {
        if cov.len() == 1 && cov[0] == 0.0 {
            continue;
        }
        let egtjg = egt[jg - 1];
        let ig = (0..ngn).find(|&i| inside(egtjg, i)).unwrap_or(ngn - 1);
        if egtjg >= egn[ngn] || !(egtjg >= ek1 && egtjg < ek2) {
            continue;
        }
        let mut igp = 0usize;
        for (jgp0, &c) in cov.iter().enumerate() {
            if c == 0.0 {
                continue;
            }
            let x = egt[jgp0];
            if !(x >= ek1 && x < ek2) {
                continue;
            }
            let mut found = true;
            while !inside(x, igp) {
                if igp + 1 == ngn {
                    found = false;
                    break;
                }
                igp += 1;
            }
            if found {
                cova[ig * ngn + igp] += 1.0 * 1.0 * c;
            }
        }
    }
    // alsigc: clflx for mt (1) and mt1 (2), both MT=2 here
    let mut clflx = vec![0.0f64; ngn];
    for (ig, c) in clflx.iter_mut().enumerate() {
        for jg in 0..nunion {
            if inside(egt[jg], ig) {
                *c += sig[jg] * flx[jg];
            }
        }
    }

    // the tape: tpid, MF=1 and MF=3/MT=251 (musigc), MF=34/MT=251 (covout)
    let mut t = GendfText::label(&tpid);
    t.begin(matd, 1, 451);
    t.cont([za, awr], [iverf, 0, -11, 0]);
    t.list([tempin, 0.0], ngn as i32, 0, 0, &egn);
    t.send();
    t.fend();
    t.begin(matd, 3, 251);
    t.list([za, 0.0], 0, 0, 0, &xmu);
    t.send();
    t.fend();
    t.begin(matd, 34, 251);
    t.cont([za, awr], [0, cfg.irelco, cfg.legord, cfg.legord]);
    t.cont([0.0, 0.0], [251, block.ld, block.ld1, ngn as i32]);
    const EPS: f64 = 1.0e-20;
    for ig in 0..ngn {
        let mut vals: Vec<f64> = Vec::with_capacity(ngn);
        let (mut ig2lo, mut ng2) = (0usize, 0usize);
        for igp in 0..ngn {
            let mut v = cova[ig * ngn + igp] / (clflx[ig] * clflx[igp]);
            if v != 0.0 && cfg.irelco != 0 {
                let denom = xmu[igp] * xmu[ig];
                v = if denom > 0.0 { v / denom.max(EPS) } else { 0.0 };
            }
            if v.abs() <= EPS && ig2lo == 0 {
                vals.clear();
                vals.push(v);
                continue;
            }
            if !(v.abs() <= EPS) {
                if ig2lo == 0 {
                    ig2lo = igp + 1;
                    vals.clear();
                }
                ng2 = igp + 1;
            }
            vals.push(v);
        }
        if ng2 != 0 || ig + 1 >= ngn {
            if ng2 == 0 {
                ig2lo = ig + 1;
                ng2 = ig + 1;
            }
            let n = ng2 - ig2lo + 1;
            vals.truncate(n);
            t.list([0.0, 0.0], n as i32, ig2lo as i32, (ig + 1) as i32, &vals);
        }
    }
    t.send();
    t.fend();
    t.mend();
    t.tend();
    Ok(Mf34Result { tape: t, un: egt, egn, xmu })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `grpav4`'s GENDF collapse weights `mubar` by `dE * sigma_el` across GENDF
    /// groups that a union group spans, and splits one GENDF group across union
    /// groups; `rdlgnd` returns the width for `l = 0`, 0 above `legord` and 0 for a
    /// coefficient below 1e-30.
    #[test]
    fn grpav4_gendf_collapse_and_rdlgnd() {
        use crate::errorr::gendf::{CondensedSection, GendfKind, GendfRecord, GroupRecord};
        let mut head = GendfRecord::marker(GendfKind::Cont, 1, 3);
        head.n = [1, 1, 0, 2];
        let sec = CondensedSection {
            mf: 3,
            mt: 251,
            head,
            groups: vec![
                GroupRecord { ng2: 3, ig2lo: 1, ig: 1, words: vec![1.0, 0.2, 2.0] },
                GroupRecord { ng2: 3, ig2lo: 1, ig: 2, words: vec![1.0, 0.5, 1.0] },
            ],
        };
        let cg = CondensedGendf { tpid: String::new(), za_awr: [0.0; 2], temp: 0.0, egn: vec![1.0, 3.0, 5.0], sections: vec![sec] };
        // union groups [1,2], [2,5]: the second spans half of group 1 and all of group 2
        let a = grpav4_gendf(&cg, &[1.0, 2.0, 5.0]).unwrap();
        assert_eq!(a[0], (1.0, 0.2));
        let want = (1.0 * 2.0 * 0.2 + 2.0 * 1.0 * 0.5) / (1.0 * 2.0 + 2.0 * 1.0);
        assert_eq!(a[1], (3.0, want));
        let n4 = nscr4_from_gendf(&a, 1);
        assert_eq!(rdlgnd(&n4, 1, 0), vec![1.0, 3.0]);
        assert_eq!(rdlgnd(&n4, 1, 1), vec![0.2, want]);
        assert_eq!(rdlgnd(&nscr4_from_gendf(&[(1.0, 1.0e-31)], 1), 1, 1), vec![0.0]);
        assert_eq!(rdlgnd(&n4, 1, 2), vec![0.0, 0.0]);
        assert_eq!(rdlgnd(&vec![None, Some((2.0, vec![0.5]))], 1, 1), vec![0.0, 0.5]);
    }
}
