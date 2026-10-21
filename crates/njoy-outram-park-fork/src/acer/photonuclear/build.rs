// Ported from NJOY2016 `src/acepn.f90` (subroutine `acephn`, lines 27-1853).
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under GPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! `acephn` — build a photo-nuclear ACE table from an ENDF photo-nuclear
//! evaluation and its PENDF.
//!
//! ## Scope, stated up front
//!
//! This is a **partial** port, and every path it does not take returns
//! [`NjoyError::NotPorted`] naming itself rather than quietly producing a
//! table that is missing something. What is here is the path a LANL-style
//! photo-nuclear file takes — the one the verification tape exercises:
//!
//! | supported | not ported (refused by name) |
//! |---|---|
//! | MF=3 reactions with `MT = 2` or `MT > 4` | `ielas = 1`, i.e. an MF=3/MT=2 elastic section |
//! | MF=6 `LAW=1`, `LANG=1`, `NA=0`, `LCT=1` → ACE law 61 | `LANG=2` (Kalbach-Mann → law 44), `NA > 0` |
//! | heating from the `e+q` fallback for MF=6 sections with no recoil | heating from an explicit recoil subsection |
//! | | MF=4/MF=5 representations (neutron-only, → laws 33 and 4) |
//! | | MF=6 `LAW=2` (→ law 33) and `LAW=4` two-body recoils |
//! | | MT=18 fission with its MF=1/452 or /456 nubar substitution |
//!
//! Refusing is the point. A photo-nuclear table that silently omits an
//! emitted particle or an angular law still *reads*, still looks plausible,
//! and is wrong in a way no consumer can detect — which is the failure mode
//! this crate's rules name explicitly.
//!
//! ## Two inputs, and why
//!
//! `acephn` takes the evaluation **and** a PENDF, and takes the ACE energy
//! grid from the PENDF's *first* MF=3 section (`acepn.f90:207-227`) rather
//! than from the evaluation. A photo-nuclear evaluation has no resonances, so
//! the two are usually the same file — but the distinction is upstream's and
//! is kept, because the grid is what every block is built on.

use crate::acer::read::{AceClass, AceFileType, AceHeader, RawAceTable};
use crate::endf::gety1::Gety1;
use crate::endf::records::{SectionCursor, Tab1};
use crate::endf::tape::Tape;
use crate::mixr::mix::sigfig;
use crate::NjoyError;

/// `emev` (`acepn.f90:70`).
const EMEV: f64 = 1.0e6;
/// `etop` (`:71`) — the grid walk's upper sentinel.
const ETOP: f64 = 1.0e10;
/// `eps` (`:74`) — the threshold-index comparison's slack.
const EPS: f64 = 1.0e-10;
/// `small` (`:73`) — the floor a positive density is held above.
const SMALL: f64 = 1.0e-12;

/// The MCNP particle number for each emitted `ZAP`, in the fixed order
/// `acephn` assigns them (`acepn.f90:645-688`).
const PARTICLE_ORDER: [(i32, i32); 7] = [
    (1, 1),       // neutron
    (0, 2),       // photon
    (1001, 9),    // proton
    (1002, 31),   // deuteron
    (1003, 32),   // triton
    (2003, 33),   // He-3
    (2004, 34),   // alpha
];

/// Everything `acephn` needs that is not on the tapes.
#[derive(Debug, Clone, Default)]
pub struct PhotonuclearOptions {
    /// `suff` — the ZAID suffix.
    pub suffix: f64,
    /// `hk` — the 70-character comment.
    pub comment: String,
    /// `hd` — the `mm/dd/yy` body of the processing date.
    pub date: String,
    /// `mcnpx` — a 13-character ZAID written `f10.3` then `"pu "`.
    pub mcnpx: bool,
    /// The 16 `IZ` entries.
    pub iz: [i32; 16],
    /// The 16 `AW` entries.
    pub aw: [f64; 16],
}

/// A built photo-nuclear table.
#[derive(Debug, Clone)]
pub struct PhotonuclearAce {
    /// `matd`.
    pub mat: i32,
    /// `za`.
    pub za: i32,
    /// `aw0` — atomic weight ratio.
    pub awr: f64,
    /// NXS(1..16).
    pub nxs: [i32; 16],
    /// JXS(1..32).
    pub jxs: [i32; 32],
    /// XSS.
    pub xss: Vec<f64>,
    /// Which XSS words `phnout` writes as integers.
    pub xss_is_int: Vec<bool>,
}

/// One `(MF, MT)` the MF=1/451 dictionary lists, with what the passes learn.
#[derive(Debug, Clone, Copy)]
struct DictEntry {
    mf: i32,
    mt: i32,
    /// `nr6` — how many recoil subsections this MF=6 section carries.
    nr6: usize,
}

/// A stored reaction: its MT, Q, threshold index and cross section.
#[derive(Debug, Clone)]
struct Reaction {
    mt: i32,
    q_mev: f64,
    /// 1-based index into the energy grid of the first stored point.
    ie: usize,
    sigma: Vec<f64>,
}

/// Build the photo-nuclear ACE table.
///
/// # Errors
/// [`NjoyError::EndfParse`] for a malformed or non-photo-nuclear tape, and
/// [`NjoyError::NotPorted`] for any representation this port does not cover —
/// see the module header for the list.
pub fn photonuclear_ace(
    endf: &Tape,
    pendf: &Tape,
    mat: i32,
    opts: &PhotonuclearOptions,
) -> Result<PhotonuclearAce, NjoyError> {
    // ── MF=1/451: confirm this is a photo-nuclear tape (`acepn.f90:112-129`) ─
    let sec451 = endf.section(mat, 1, 451).ok_or_else(|| {
        NjoyError::EndfParse(format!("acephn: no MF=1/MT=451 for MAT {mat}"))
    })?;
    let mut c = SectionCursor::new(&sec451.rows);
    let head = c.read_cont()?;
    let za = head.c1.round() as i32;
    let awr = head.c2;
    let _second = c.read_cont()?;
    let third = c.read_cont()?;
    // `izai = NSUB/10`; a photo-nuclear tape has NSUB = 0, so `izai = 0`.
    let izai = third.n1 / 10;
    if izai != 0 {
        return Err(NjoyError::EndfParse(format!(
            "acephn: NSUB = {} means izai = {izai}; this is not a photonuclear \
             tape (acepn.f90:127-129)",
            third.n1
        )));
    }

    // ── the dictionary scan (`:139-176`) ────────────────────────────────────
    let mut dict: Vec<DictEntry> = Vec::new();
    let mut ntr = 0usize;
    let (mut ielas, mut mf4, mut mf6) = (false, false, false);
    for s in endf.sections() {
        if s.key.mat != mat {
            continue;
        }
        let (mf, mt) = (s.key.mf, s.key.mt);
        if !((3..30).contains(&mf) && (mt == 2 || mt > 4)) {
            continue;
        }
        if mf == 3 {
            ntr += 1;
        }
        if mt == 2 {
            ielas = true;
        }
        if mf == 4 {
            mf4 = true;
        }
        if mf == 6 {
            mf6 = true;
        }
        dict.push(DictEntry { mf, mt, nr6: 0 });
    }
    if ielas {
        return Err(NjoyError::NotPorted(
            "acephn: an MF=3/MT=2 elastic section (ielas = 1) -- the ELS block \
             and its separate NON accounting are not ported",
        ));
    }
    if mf4 {
        return Err(NjoyError::NotPorted(
            "acephn: MF=4 angular data -- the MF=4/MF=5 neutron-only \
             representation (ACE laws 33 and 4) is not ported",
        ));
    }
    if !mf6 {
        return Err(NjoyError::NotPorted(
            "acephn: no MF=6 -- this port builds production blocks from MF=6 only",
        ));
    }
    if endf.section(mat, 1, 452).is_some() || endf.section(mat, 1, 456).is_some() {
        return Err(NjoyError::NotPorted(
            "acephn: MF=1/MT=452 or /456 nubar -- the MT=18 fission yield \
             substitution (acepn.f90:178-205, 469-475) is not ported",
        ));
    }

    // ── the energy grid, from the PENDF's FIRST MF=3 section (`:207-227`) ───
    let first_mf3 = pendf
        .sections()
        .iter()
        .find(|s| s.key.mat == mat && s.key.mf == 3)
        .ok_or_else(|| {
            NjoyError::EndfParse(format!("acephn: the PENDF has no MF=3 for MAT {mat}"))
        })?;
    let t_tot = tab1_of(&first_mf3.rows)?;
    let mut g = Gety1::new(&t_tot);
    let mut grid: Vec<f64> = Vec::new();
    let mut v = g.get(0.0);
    let mut enext = v.xnext;
    while enext < ETOP {
        let e = enext;
        v = g.get(e);
        enext = v.xnext;
        if grid.is_empty() && v.y != 0.0 {
            // The one point below the threshold, so the grid brackets it.
            grid.push(sigfig(e, 7, -1));
        }
        grid.push(e);
        if grid.len() > 5_000_000 {
            return Err(NjoyError::EndfParse(
                "acephn: the MF=3 grid walk did not terminate".into(),
            ));
        }
    }
    let nes = grid.len();

    // ── locators (`:236-256`) ───────────────────────────────────────────────
    let esz = 1usize;
    let tot = esz + nes;
    // With no elastic, NON aliases TOT and THN follows it directly.
    let non = tot;
    let els = 0usize;
    let thn = tot + nes;
    let mtr = thn + nes;
    let lqr = mtr + ntr;
    let lsig = lqr + ntr;
    let sig = lsig + ntr;

    // ── MF=3: the partial cross sections, and TOT accumulated from them ─────
    let mut totals = vec![0.0f64; nes];
    let mut heating = vec![0.0f64; nes];
    let mut reactions: Vec<Reaction> = Vec::new();
    for s in endf.sections() {
        if s.key.mat != mat || s.key.mf != 3 || !(s.key.mt == 2 || s.key.mt > 4) {
            continue;
        }
        let t = tab1_of(&s.rows)?;
        let q_mev = sigfig(t.head.c2 / EMEV, 7, 0);
        let mut gg = Gety1::new(&t);
        // The reaction threshold: `gety1`'s second point, shaded down when the
        // cross section is already non-zero there (`:272-277`).
        let first = gg.get(0.0);
        let mut e = first.xnext;
        let at = gg.get(e);
        if at.y != 0.0 {
            e = sigfig(e, 7, -1);
        }
        // Walk the grid down to the first point at or below the threshold.
        let mut j = nes;
        while j >= 1 && e <= (1.0 + EPS) * grid[j - 1] {
            j -= 1;
        }
        j += 1;
        let mut gg = Gety1::new(&t);
        let mut sigma = Vec::with_capacity(nes - j + 1);
        for (i, &ei) in grid.iter().enumerate().skip(j - 1) {
            let sv = sigfig(gg.get(ei).y, 7, 0);
            totals[i] += sv;
            sigma.push(sv);
        }
        reactions.push(Reaction {
            mt: s.key.mt,
            q_mev,
            ie: j,
            sigma,
        });
    }
    if reactions.len() != ntr {
        return Err(NjoyError::EndfParse(format!(
            "acephn: the dictionary promised {ntr} MF=3 reactions and the tape \
             has {}",
            reactions.len()
        )));
    }

    // ── MF=6: count the emitted particles and find their thresholds ─────────
    let mut counts: std::collections::BTreeMap<i32, usize> = Default::default();
    let mut thresholds: std::collections::BTreeMap<i32, f64> = Default::default();
    for d in dict.iter_mut().filter(|d| d.mf == 6) {
        let sec = endf.section(mat, 6, d.mt).ok_or_else(|| {
            NjoyError::EndfParse(format!("acephn: MF=6/MT={} vanished", d.mt))
        })?;
        let mut cur = SectionCursor::new(&sec.rows);
        let h = cur.read_cont()?;
        let lct = h.l2;
        if lct != 1 {
            return Err(NjoyError::NotPorted(
                "acephn: MF=6 with LCT /= 1 -- only the laboratory frame is ported",
            ));
        }
        let nk = h.n1.max(0) as usize;
        let rx = reactions
            .iter()
            .find(|r| r.mt == d.mt)
            .ok_or_else(|| {
                NjoyError::EndfParse(format!(
                    "acephn: MF=6/MT={} has no MF=3 cross section",
                    d.mt
                ))
            })?;
        let thresh = grid[rx.ie - 1];
        for _ in 0..nk {
            let y = cur.read_tab1()?;
            let izap = y.head.c1.round() as i32;
            let law = y.head.l2;
            if izap > 2004 {
                // A recoil: upstream uses it for heating only. Not ported, and
                // its presence changes the heating of the whole table, so it
                // is refused rather than ignored.
                return Err(NjoyError::NotPorted(
                    "acephn: an MF=6 recoil subsection (ZAP > 2004) -- the \
                     recoil heating path (acepn.f90:441-500) is not ported",
                ));
            }
            if law != 1 {
                return Err(NjoyError::NotPorted(
                    "acephn: MF=6 LAW /= 1 -- only the continuum energy-angle \
                     law is ported",
                ));
            }
            *counts.entry(izap).or_insert(0) += 1;
            // The production threshold: the first energy whose yield is
            // non-zero, floored at the reaction threshold (`:417-436`).
            let mut e = thresh;
            for w in y.pairs.windows(2) {
                if w[1].1 > 0.0 {
                    e = w[0].0;
                    break;
                }
            }
            if e < thresh {
                e = thresh;
            }
            let slot = thresholds.entry(izap).or_insert(ETOP);
            if e < *slot {
                *slot = e;
            }
            skip_law1(&mut cur)?;
        }
    }

    // ── heating: the `e+q` fallback for MF=6 sections with no recoil (`:626-641`)
    for rx in &reactions {
        let has_mf6_no_recoil = dict.iter().any(|d| d.mf == 6 && d.mt == rx.mt && d.nr6 == 0);
        if !has_mf6_no_recoil {
            continue;
        }
        for (k, &s) in rx.sigma.iter().enumerate() {
            let i = rx.ie - 1 + k;
            if totals[i] != 0.0 {
                heating[i] += s * (grid[i] / EMEV + rx.q_mev) / totals[i];
            }
        }
    }

    // ── the IXSA table (`:643-692`) ─────────────────────────────────────────
    let types: Vec<(i32, i32, usize)> = PARTICLE_ORDER
        .iter()
        .filter_map(|&(zap, ipt)| counts.get(&zap).map(|&n| (zap, ipt, n)))
        .collect();
    let ntype = types.len();
    let npixs = 2usize;
    let neixs = 12usize;

    Ok(assemble(
        Assembly {
            mat,
            za,
            awr,
            grid,
            totals,
            heating,
            reactions,
            types,
            thresholds,
            ntr,
            ntype,
            npixs,
            neixs,
            locators: Locators {
                esz,
                tot,
                non,
                els,
                thn,
                mtr,
                lqr,
                lsig,
                sig,
            },
        },
        endf,
        opts,
    )?)
}

/// The fixed part of the layout, decided before any block is written.
struct Locators {
    esz: usize,
    tot: usize,
    non: usize,
    els: usize,
    thn: usize,
    mtr: usize,
    lqr: usize,
    lsig: usize,
    sig: usize,
}

/// Everything the first two passes learned, handed to the assembler.
struct Assembly {
    mat: i32,
    za: i32,
    awr: f64,
    grid: Vec<f64>,
    totals: Vec<f64>,
    heating: Vec<f64>,
    reactions: Vec<Reaction>,
    types: Vec<(i32, i32, usize)>,
    thresholds: std::collections::BTreeMap<i32, f64>,
    ntr: usize,
    ntype: usize,
    npixs: usize,
    neixs: usize,
    locators: Locators,
}

/// Read a section's single TAB1 (after its HEAD).
fn tab1_of(rows: &[[f64; 6]]) -> Result<Tab1, NjoyError> {
    let mut c = SectionCursor::new(rows);
    c.read_cont()?;
    c.read_tab1()
}

/// Skip one MF=6 LAW=1 subsection's TAB2 and its LIST records.
fn skip_law1(cur: &mut SectionCursor<'_>) -> Result<(), NjoyError> {
    let t2 = cur.read_tab2()?;
    for _ in 0..t2.head.n2.max(0) {
        cur.read_list()?;
    }
    Ok(())
}

/// A growable XSS with the integer flags the writer needs.
struct Xss {
    v: Vec<f64>,
    i: Vec<bool>,
}

impl Xss {
    fn new() -> Self {
        Xss {
            v: Vec::new(),
            i: Vec::new(),
        }
    }
    /// 1-based index of the next word.
    fn next(&self) -> usize {
        self.v.len() + 1
    }
    fn push_int(&mut self, x: f64) {
        self.v.push(x);
        self.i.push(true);
    }
    fn push_real(&mut self, x: f64) {
        self.v.push(x);
        self.i.push(false);
    }
    /// Grow with zeros (written as reals) up to and including 1-based `n`.
    fn reserve_reals(&mut self, n: usize) {
        while self.v.len() < n {
            self.v.push(0.0);
            self.i.push(false);
        }
    }
    fn set(&mut self, one_based: usize, x: f64) {
        self.v[one_based - 1] = x;
    }
    fn get(&self, one_based: usize) -> f64 {
        self.v[one_based - 1]
    }
    fn mark_int(&mut self, one_based: usize) {
        self.i[one_based - 1] = true;
    }
}

/// Lay the blocks out and fill them (`acepn.f90:236-1800`).
fn assemble(
    a: Assembly,
    endf: &Tape,
    opts: &PhotonuclearOptions,
) -> Result<PhotonuclearAce, NjoyError> {
    let Assembly {
        mat,
        za,
        awr,
        grid,
        mut totals,
        mut heating,
        reactions,
        types,
        thresholds,
        ntr,
        ntype,
        npixs,
        neixs,
        locators,
    } = a;
    let nes = grid.len();
    let l = locators;

    let mut x = Xss::new();
    // ESZ (energies, still in eV), TOT, THN.
    x.reserve_reals(l.thn + nes - 1);
    for (k, &e) in grid.iter().enumerate() {
        x.set(l.esz + k, e);
        x.set(l.tot + k, totals[k]);
    }
    // MTR (ints), LQR (reals), LSIG (ints), then SIG.
    x.reserve_reals(l.sig - 1);
    for (r, rx) in reactions.iter().enumerate() {
        x.set(l.mtr + r, rx.mt as f64);
        x.mark_int(l.mtr + r);
        x.set(l.lqr + r, rx.q_mev);
        x.mark_int(l.lsig + r);
    }
    for (r, rx) in reactions.iter().enumerate() {
        x.set(l.lsig + r, (x.next() - l.sig + 1) as f64);
        x.push_int(rx.ie as f64);
        x.push_int(rx.sigma.len() as f64);
        for &s in &rx.sigma {
            x.push_real(s);
        }
    }

    // The IXSA table: `NEIXS` locators for each of `NTYPE` particles.
    let ixsa = x.next();
    for &(_, ipt, n) in &types {
        x.push_int(ipt as f64);
        x.push_int(n as f64);
        for _ in 2..neixs {
            x.push_int(0.0);
        }
    }
    let ixs = x.next();

    // ── one production block per emitted particle (`:693-1800`) ─────────────
    for (ti, &(zap, _ipt, ntrp)) in types.iter().enumerate() {
        let row = ixsa + neixs * ti;
        let thresh = thresholds.get(&zap).copied().unwrap_or(ETOP);
        // `it`: the last grid point strictly below the production threshold.
        let mut it = 1usize;
        while it < nes && grid[it - 1] < thresh {
            it += 1;
        }
        if it > 1 {
            it -= 1;
        }
        let np = nes - it + 1;

        let pxs = x.next();
        x.set(row + 2, pxs as f64);
        x.push_int(it as f64);
        x.push_int(np as f64);
        for _ in 0..np {
            x.push_real(0.0);
        }
        let phn = x.next();
        x.set(row + 3, phn as f64);
        x.push_int(it as f64);
        x.push_int(np as f64);
        for _ in 0..np {
            x.push_real(0.0);
        }
        let mtrp = x.next();
        x.set(row + 4, mtrp as f64);
        for _ in 0..ntrp {
            x.push_int(0.0);
        }
        let tyrp = x.next();
        x.set(row + 5, tyrp as f64);
        for _ in 0..ntrp {
            x.push_int(0.0);
        }
        let lsigp = x.next();
        x.set(row + 6, lsigp as f64);
        for _ in 0..ntrp {
            x.push_int(0.0);
        }
        let sigp = x.next();
        x.set(row + 7, sigp as f64);

        // The production cross section and the SIGP yields (`:836-915`).
        let mut jp = 0usize;
        let mut subsections: Vec<(i32, Tab1, usize)> = Vec::new(); // (mt, yield, reaction index)
        for s in endf.sections() {
            if s.key.mat != mat || s.key.mf != 6 {
                continue;
            }
            let mut cur = SectionCursor::new(&s.rows);
            let h = cur.read_cont()?;
            let nk = h.n1.max(0) as usize;
            let ri = reactions
                .iter()
                .position(|r| r.mt == s.key.mt)
                .ok_or_else(|| {
                    NjoyError::EndfParse(format!("acephn: MF=6/MT={} has no MF=3", s.key.mt))
                })?;
            for _ in 0..nk {
                let y = cur.read_tab1()?;
                let izap = y.head.c1.round() as i32;
                if izap != zap {
                    skip_law1(&mut cur)?;
                    continue;
                }
                jp += 1;
                x.set(mtrp + jp - 1, s.key.mt as f64);
                x.set(tyrp + jp - 1, 1.0); // LCT = 1 was checked above
                x.set(lsigp + jp - 1, (x.next() - sigp + 1) as f64);
                // PXS += yield * sigma, on the reaction's own grid span.
                let rx = &reactions[ri];
                for (k, &s_i) in rx.sigma.iter().enumerate() {
                    let i = rx.ie + k; // 1-based grid index
                    let yv = crate::endf::interp::terpa(&y.interp, &y.pairs, grid[i - 1]).0;
                    let slot = pxs + 2 + i - it;
                    let tt = x.get(slot) + yv * s_i;
                    x.set(slot, sigfig(tt, 7, 0));
                }
                // The SIGP entry: MFTYPE=6, MTMULT, NR=0, NE, then E and Y.
                if y.interp.len() > 1 && y.interp.first().map(|&(_, i)| i) != Some(2) {
                    return Err(NjoyError::NotPorted(
                        "acephn: a multi-region non-lin-lin MF=6 multiplicity -- \
                         upstream warns and this port refuses (acepn.f90:896-903)",
                    ));
                }
                x.push_int(6.0);
                x.push_int(s.key.mt as f64);
                x.push_int(0.0);
                let ne = y.pairs.len();
                x.push_int(ne as f64);
                for &(e, _) in &y.pairs {
                    x.push_real(sigfig(e / EMEV, 7, 0));
                }
                for &(_, yy) in &y.pairs {
                    x.push_real(sigfig(yy, 7, 0));
                }
                subsections.push((s.key.mt, y, ri));
                skip_law1(&mut cur)?;
            }
        }

        // LANDP / ANDP. With LAW=1 everywhere the angular data rides in the
        // energy distribution, so LANDP is -1 and ANDP stays empty
        // (`:925-930`, `:1477`).
        let landp = x.next();
        x.set(row + 8, landp as f64);
        for _ in 0..ntrp {
            x.push_int(0.0);
        }
        let andp = x.next();
        x.set(row + 9, andp as f64);

        // LDLWP / DLWP.
        let ldlwp = x.next();
        x.set(row + 10, ldlwp as f64);
        for _ in 0..ntrp {
            x.push_int(0.0);
        }
        let dlwp = x.next();
        x.set(row + 11, dlwp as f64);

        let mut phn_acc = vec![0.0f64; np];
        for (jpi, (mt, yld, ri)) in subsections.iter().enumerate() {
            let sec = endf.section(mat, 6, *mt).expect("checked above");
            let mut cur = SectionCursor::new(&sec.rows);
            cur.read_cont()?;
            // Re-walk to this particle's subsection.
            loop {
                let y = cur.read_tab1()?;
                if y.head.c1.round() as i32 == zap {
                    break;
                }
                skip_law1(&mut cur)?;
            }
            law61(
                &mut x, &mut cur, landp, dlwp, ldlwp, jpi, &grid, &reactions[*ri], yld,
                &mut phn_acc, it, &mut heating, &totals,
            )?;
        }

        // Divide the heating by the total and fold it in (`:1802-1812`).
        for k in 0..np {
            let i = it + k; // 1-based grid index
            let mut h = phn_acc[k];
            if totals[i - 1] != 0.0 {
                h /= totals[i - 1];
            }
            // `acepn.f90:1809` guards this with `ip > 1`, and `ip` is the
            // **ZAP**, not the MCNP particle number. So the neutron (ZAP 1)
            // and the **photon** (ZAP 0) are both excluded; only the charged
            // particles fold their production heating into the table total.
            // Reading it as "everything but the neutron" puts the photon's
            // whole contribution in twice.
            if zap > 1 {
                heating[i - 1] += h;
            }
            x.set(phn + 2 + k, sigfig(h, 7, 0));
        }
        let _ = ntrp;
    }

    let lxs = x.v.len();
    // The grid and the heating go to MeV and 7 figures last (`:1818-1821`).
    for k in 0..nes {
        let e = sigfig(grid[k] / EMEV, 7, 0);
        x.set(l.esz + k, e);
        x.set(l.thn + k, sigfig(heating[k], 7, 0));
    }
    for k in 0..nes {
        x.set(l.tot + k, totals[k]);
    }
    totals.clear();

    let mut nxs = [0i32; 16];
    nxs[0] = lxs as i32;
    nxs[1] = za;
    nxs[2] = nes as i32;
    nxs[3] = ntr as i32;
    nxs[4] = ntype as i32;
    nxs[5] = npixs as i32;
    nxs[6] = neixs as i32;
    nxs[9] = za / 1000;
    nxs[10] = za % 1000;
    nxs[15] = 1; // tvn
    let mut jxs = [0i32; 32];
    jxs[0] = l.esz as i32;
    jxs[1] = l.tot as i32;
    jxs[2] = l.non as i32;
    jxs[3] = l.els as i32;
    jxs[4] = l.thn as i32;
    jxs[5] = l.mtr as i32;
    jxs[6] = l.lqr as i32;
    jxs[7] = l.lsig as i32;
    jxs[8] = l.sig as i32;
    jxs[9] = ixsa as i32;
    jxs[10] = ixs as i32;

    let _ = opts;
    // The integer/real split comes from the **layout walk**, not from flags
    // tracked while building. There is one description of a photo-nuclear
    // table's structure in this crate (`layout::walk`, a port of `phnout`),
    // it is already byte-verified against NJOY on the read side, and using it
    // here means the builder cannot disagree with the reader about where a
    // locator is. It doubles as a structural check: a table the walk cannot
    // traverse is one this function built wrong.
    let xss_is_int = crate::acer::photonuclear::layout::walk(&nxs, &jxs, &x.v)
        .map_err(|e| {
            NjoyError::EndfParse(format!(
                "acephn: the table just built does not walk -- {e}. This is a                  defect in the builder, not in the input."
            ))
        })?
        .is_int;
    debug_assert_eq!(
        xss_is_int.len(),
        x.v.len(),
        "the walk must classify every word"
    );
    Ok(PhotonuclearAce {
        mat,
        za,
        awr,
        nxs,
        jxs,
        xss: x.v,
        xss_is_int,
    })
}

/// `ptleg2` for an **isotropic** Legendre list (`acecm.f90`, `NA = 0`).
///
/// With no coefficients upstream sets `nord = 1, fl(1) = 0`, so the density is
/// the constant `1/2` and the adaptive reconstruction converges on the three
/// points it primes the stack with. Measured against NJOY's own output:
/// `mu = [-1, 0, 1]`, `pdf = [0.5, 0.5, 0.5]`.
///
/// **Only the isotropic case is ported.** `ptleg2`'s adaptive reconstruction
/// (tolerances `2e-4`/`2e-3`, a 24-deep stack, a `1e-10` floor and a negative
/// -lobe repair pass) is a different routine from this crate's
/// [`crate::acer::angular::legendre_cosine_law`], which uses `ANGLE_TOL =
/// 5e-3` and its own bisection — so reusing that would give a *different grid*
/// and lose byte parity. Checked and rejected rather than assumed.
fn ptleg2_isotropic() -> (Vec<f64>, Vec<f64>) {
    (vec![-1.0, 0.0, 1.0], vec![0.5, 0.5, 0.5])
}

/// MF=6 `LAW=1` with `LANG=1` → ACE law **61** (`acepn.f90:1487-1738`).
#[allow(clippy::too_many_arguments)]
fn law61(
    x: &mut Xss,
    cur: &mut SectionCursor<'_>,
    landp: usize,
    dlwp: usize,
    ldlwp: usize,
    jp0: usize,
    grid: &[f64],
    rx: &Reaction,
    yld: &Tab1,
    phn_acc: &mut [f64],
    it: usize,
    heating: &mut [f64],
    totals: &[f64],
) -> Result<(), NjoyError> {
    let nes = grid.len();
    x.set(ldlwp + jp0, (x.next() - dlwp + 1) as f64);
    let last = x.next();
    x.push_int(0.0); // LNW
    x.push_int(0.0); // LAW, filled below
    x.push_int(0.0); // IDAT, filled below

    let t2 = cur.read_tab2()?;
    let lang = t2.head.l1;
    let lep = t2.head.l2;
    let ne = t2.head.n2.max(0) as usize;
    if lang != 1 {
        return Err(NjoyError::NotPorted(
            "acephn: MF=6 LAW=1 with LANG /= 1 -- Kalbach-Mann (LANG=2, ACE \
             law 44) is not ported",
        ));
    }
    x.set(last + 1, 61.0);
    // The angular data rides in the energy distribution (`acepn.f90:1477`).
    x.set(landp + jp0, -1.0);

    // The law's own applicability table: NR=0, NE=2, then E(1:2), P(1:2).
    let lee = x.next();
    x.push_int(0.0); // NR
    x.push_int(2.0); // NE
    for _ in 0..4 {
        x.push_real(0.0);
    }
    x.set(last + 2, (x.next() - dlwp + 1) as f64);
    x.push_int(0.0); // LDAT NR
    x.push_int(ne as f64); // LDAT NE
    let lle = x.next();
    for _ in 0..2 * ne {
        x.push_real(0.0);
    }

    // Per incident energy: the outgoing law, then its angular tables.
    let mut avlab_tab: Vec<(f64, f64)> = Vec::with_capacity(ne);
    for ie in 1..=ne {
        let rec = cur.read_list()?;
        let e_in = rec.head.c2;
        let nd = rec.head.l1.max(0) as usize;
        let na = rec.head.l2.max(0) as usize;
        let ng = rec.head.n2.max(0) as usize;
        let ncyc = na + 2;
        if na != 0 {
            return Err(NjoyError::NotPorted(
                "acephn: MF=6 LAW=1 LANG=1 with NA > 0 -- the Legendre angular \
                 reconstruction (ptleg2) is ported for the isotropic case only",
            ));
        }
        if nd != 0 {
            return Err(NjoyError::NotPorted(
                "acephn: MF=6 LAW=1 with discrete lines (ND > 0) is not ported",
            ));
        }
        let e_mev = sigfig(e_in / EMEV, 7, 0);
        if ie == 1 {
            x.set(lee + 2, e_mev);
            x.set(lee + 4, 1.0);
        }
        if ie == ne {
            x.set(lee + 3, e_mev);
            x.set(lee + 5, 1.0);
        }
        x.set(lle + ie - 1, e_mev);
        x.set(lle + ne + ie - 1, (x.next() - dlwp + 1) as f64);

        let base = x.next();
        x.push_int((lep + 10 * rec.head.l1) as f64); // INTT
        x.push_int(ng as f64); // NP
        // E', pdf, cdf and the angular pointer, four parallel columns.
        for _ in 0..4 * ng {
            x.push_real(0.0);
        }
        let nexc0 = base + 2 + 4 * ng;
        // The angular pointer column is a locator, hence an integer.
        for k in 0..ng {
            x.mark_int(base + 2 + 3 * ng + k);
        }

        let mut nexc = nexc0;
        let mut avlab = 0.0f64;
        let (mut avll, mut cdf_prev, mut ep_prev, mut f_prev) = (0.0f64, 0.0f64, 0.0f64, 0.0f64);
        for ig in 1..=ng {
            let ep = rec.data[ncyc * (ig - 1)];
            let f = rec.data[1 + ncyc * (ig - 1)];
            let ep_mev = sigfig(ep / EMEV, 7, 0);
            let mut pdf = sigfig(f * EMEV, 7, 0);
            if pdf > 0.0 && pdf < SMALL {
                pdf = SMALL;
            }
            let cdf = if ig == 1 {
                0.0
            } else if lep == 1 {
                cdf_prev + f_prev * (ep - ep_prev)
            } else {
                cdf_prev + (f_prev + f) / 2.0 * (ep - ep_prev)
            };
            x.set(base + 2 + (ig - 1), ep_mev);
            x.set(base + 2 + ng + (ig - 1), pdf);
            x.set(base + 2 + 2 * ng + (ig - 1), cdf);
            x.set(base + 2 + 3 * ng + (ig - 1), (nexc - dlwp + 1) as f64);
            // The isotropic angular table for this outgoing energy.
            let (mu, apdf) = ptleg2_isotropic();
            let nmu = mu.len();
            x.reserve_reals(nexc + 1 + 3 * nmu);
            x.set(nexc, 2.0); // INTMU, lin-lin
            x.mark_int(nexc);
            x.set(nexc + 1, nmu as f64);
            x.mark_int(nexc + 1);
            let mut acdf = 0.0f64;
            for k in 0..nmu {
                x.set(nexc + 2 + k, sigfig(mu[k], 7, 0));
                x.set(nexc + 2 + nmu + k, sigfig(apdf[k], 7, 0));
                let v = if k == 0 {
                    0.0
                } else if k == nmu - 1 {
                    1.0
                } else {
                    let del = mu[k] - mu[k - 1];
                    let av = (apdf[k] + apdf[k - 1]) / 2.0;
                    sigfig(acdf + del * av, 7, 0)
                };
                x.set(nexc + 2 + 2 * nmu + k, v);
                acdf = v;
            }
            nexc += 2 + 3 * nmu;
            // The laboratory mean energy, trapezoid over E'·f (`:1673-1691`).
            if ig > 1 {
                let dele = ep_mev - sigfig(ep_prev / EMEV, 7, 0);
                let pdf_prev = x.get(base + 2 + ng + (ig - 2));
                let avav = if lep == 1 {
                    pdf_prev * (avll + ep_mev) / 2.0
                } else {
                    (pdf_prev * avll + pdf * ep_mev) / 2.0
                };
                avlab += avav * dele;
                // `avll` is updated **inside** this guard upstream
                // (`acepn.f90:1690`), so the ig = 2 term uses `avll = 0`
                // rather than E'(1). Hoisting it out adds
                // `pdf(1)*E'(1)*dele/2` to every law's mean outgoing energy --
                // about 4e-8 MeV here, which is invisible in the value and
                // enough to flip `sigfig(.., 7, 0)` on the heating block.
                avll = ep_mev;
            }
            cdf_prev = cdf;
            ep_prev = ep;
            f_prev = f;
        }
        // Renormalise by the last cumulative (`:1699-1706`).
        let denom = x.get(base + 2 + 3 * ng - 1);
        let renorm = if denom != 0.0 { 1.0 / denom } else { 1.0 };
        for ig in 0..ng {
            let p = x.get(base + 2 + ng + ig);
            x.set(base + 2 + ng + ig, sigfig(renorm * p, 7, 0));
            let cc = x.get(base + 2 + 2 * ng + ig);
            x.set(base + 2 + 2 * ng + ig, sigfig(renorm * cc, 9, 0));
        }
        avlab_tab.push((e_mev, avlab));
        // The next block starts after this energy's angular tables.
        x.reserve_reals(nexc - 1);
    }

    // Heating: yield x sigma x mean outgoing energy (`:1716-1738`).
    let avl_interp: Vec<(u32, u32)> = vec![(avlab_tab.len() as u32, 2)];
    for (k, &s_i) in rx.sigma.iter().enumerate() {
        let i = rx.ie + k; // 1-based grid index
        if i > nes {
            break;
        }
        let y = crate::endf::interp::terpa(&yld.interp, &yld.pairs, grid[i - 1]).0;
        let h = crate::endf::interp::terpa(&avl_interp, &avlab_tab, grid[i - 1] / EMEV).0;
        let hh = h * y * s_i;
        phn_acc[i - it] += hh;
        if totals[i - 1] != 0.0 {
            heating[i - 1] -= hh / totals[i - 1];
        }
    }
    Ok(())
}


impl PhotonuclearAce {
    /// Assemble the writable table (`acepn.f90:1823-1848`).
    ///
    /// The ZAID is `f9.2` of `ZA + suff` then `u`, or `f10.3` then `"pu "` in
    /// the mcnpx variant; `tz` is 0, because a photo-nuclear table carries no
    /// temperature.
    pub fn into_raw(self, opts: &PhotonuclearOptions, file_type: AceFileType) -> RawAceTable {
        let zaid_num = self.za as f64 + opts.suffix;
        let zaid = if opts.mcnpx {
            format!("{zaid_num:10.3}pu ")
        } else {
            format!("{zaid_num:9.2}u")
        };
        let date = format!("  {:<8}", opts.date);
        let mat_id = format!("   mat{:4}", self.mat);
        let comment = format!("{:<70}", opts.comment);
        RawAceTable {
            file_type,
            header: AceHeader {
                raw_text: [
                    zaid.as_bytes().to_vec(),
                    date.as_bytes().to_vec(),
                    comment.as_bytes().to_vec(),
                    mat_id.as_bytes().to_vec(),
                ],
                zaid: zaid.trim().to_string(),
                zaid_num: Some(zaid_num),
                class: AceClass::Photonuclear,
                awr: self.awr,
                kt_mev: 0.0,
                date: opts.date.clone(),
                comment: opts.comment.clone(),
                mat_id: mat_id.trim().to_string(),
                iz: opts.iz,
                aw: opts.aw,
            },
            nxs: self.nxs,
            jxs: self.jxs,
            xss_is_int: Some(self.xss_is_int),
            xss: self.xss,
        }
    }
}
