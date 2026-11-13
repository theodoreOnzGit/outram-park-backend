// Ported from NJOY2016 `src/acepn.f90` (subroutine `acephn`, lines 27-1853).
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under AGPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! `acephn` — build a photo-nuclear ACE table from an ENDF photo-nuclear
//! evaluation and its PENDF.
//!
//! ## Scope, stated up front
//!
//! ~~This is a **partial** port of the path a LANL-style photo-nuclear file
//! takes (MF=6 `LAW=1`, `LANG=1`, `NA=0`, `LCT=1` only), refusing `LANG=2`,
//! `NA > 0`, `LCT /= 1`, recoil subsections, `LAW=2` and the MT=18 nubar.~~
//! **CORRECTED 2026-11-13 (gh:#1369):** the body is now a line-for-line
//! translation of `acephn`, XSS written at upstream's offsets in upstream's
//! order. Translated: reactions read from the PENDF, `LCT /= 1` (`TYRP = -1`),
//! `LANG = 2` (ACE law 44, `bachaa`/`eavl`), `NA > 0` (`ptleg2`), MF=6
//! `LAW = 2` (ANDP tables and ACE law 33), LAW=1 recoil heating, MT=102
//! `e+q` heating and the MT=18 nubar (`copynubar`). It reproduces NJOY2016's
//! ACE for upstream decks 57, 58, 66 and 78 line for line
//! (`tests/deck_driver_photonuclear_vs_njoy2016.rs`).
//!
//! Still refused by name ([`NjoyError::NotPorted`]): an MF=3/MT=2 elastic
//! section (`ielas = 1`), ~~MF=4 (deck 56), an MF=5 distribution for a
//! production reaction,~~ and MF=6 `LAW = 4`. **CORRECTED 2026-11-13 (gh:#1369, deck 56):** the MF=4/MF=5
//! neutron branches are translated in [`super::mf45`]; deck 56 is identical. **CORRECTED 2026-11-13
//! (gh:#1369, deck 64):** `LAW = 4`'s recoil heating (`acepn.f90:507-584`) is translated; `LAW = 4`
//! for an emitted light particle (`:975-990`, `:1774-1790`) is still refused. Refusing is the point: a table that
//! silently omits an emitted particle still reads and looks plausible.
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
use crate::acer::acelcp::eavl;
use crate::acer::acelf6::bachaa;
use crate::common::phys::{AMASSN_AMU, AMU_G, CLIGHT_CM_S, EV_ERG};
use crate::endf::records::{SectionCursor, Tab1};
use super::records::{copynubar, flat, read_mf6, terp, terp_lin, Mf6, Xss};
use super::mf45::{law33_discrete, mf4_yields, mf5_distribution, Block, Reaction};
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
/// `rmin` (`:75`) -- the floor below which an angular density is zeroed.
const RMIN: f64 = 1.0e-30;

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

/// A reaction as `MTR`/`LQR`/`LSIG` describe it: `k` is the 1-based XSS
/// index of its `IE` word (so `xss(2+k+i-iaa)` is its cross section at grid
/// point `i`).
#[derive(Debug, Clone, Copy)]
struct Rx {
    mt: i32,
    q: f64,
    k: usize,
    iaa: usize,
}

/// `acephn` (`acepn.f90:27-1853`): build the photo-nuclear ACE table from
/// the evaluation `endf` and its PENDF `pendf`.
///
/// A line-for-line translation: XSS is filled at upstream's offsets, in its
/// order, with its `sigfig` calls, and the integer/real split is taken from
/// [`crate::acer::photonuclear::layout::walk`] afterwards.
///
/// # Errors
/// [`NjoyError::EndfParse`] for a tape that is not photo-nuclear or is
/// malformed, and [`NjoyError::NotPorted`] for the branches not translated:
/// an MF=3/MT=2 elastic section and MF=6 `LAW = 4` for an emitted light
/// particle (its recoil heating is translated, 2026-11-13); `EndfParse` for a
/// polynomial nubar (which upstream refuses too). MF=4/MF=5 are translated
/// (`super::mf45`, 2026-11-13).
pub fn photonuclear_ace(
    endf: &Tape,
    pendf: &Tape,
    mat: i32,
    opts: &PhotonuclearOptions,
) -> Result<PhotonuclearAce, NjoyError> {
    let _ = opts;
    let emc2 = AMASSN_AMU * AMU_G * CLIGHT_CM_S * CLIGHT_CM_S / EV_ERG / EMEV;
    // ── MF=1/451 (`:89-138`) ──────────────────────────────────────────────
    let sec451 = endf
        .section(mat, 1, 451)
        .ok_or_else(|| NjoyError::EndfParse(format!("acephn: no MF=1/MT=451 for MAT {mat}")))?;
    let mut c = SectionCursor::new(&sec451.rows);
    let head = c.read_cont()?;
    let za = head.c1.round() as i32;
    let awr = head.c2;
    let second = c.read_cont()?;
    let is = second.l2;
    let third = c.read_cont()?;
    let izai = third.n1 / 10;
    // Upstream tests only `NSUB/10` (`:126-129`), so a photo-ATOMIC tape
    // (`NSUB = 3`) passes its check and is processed into a table with no
    // MF=3 reactions. This port refuses it as the wrong sublibrary instead.
    if izai != 0 || third.n1 != 0 {
        return Err(NjoyError::EndfParse(format!(
            "acephn: NSUB = {} means izai = {izai}; this is not a photonuclear \
             tape (acepn.f90:127-129)",
            third.n1
        )));
    }

    // ── the dictionary scan (`:139-176`) ────────────────────────────────────
    // (mf, mt, nr6)
    let mut dict: Vec<(i32, i32, usize)> = Vec::new();
    let (mut ntr, mut ielas, mut mf4, mut mf6) = (0usize, false, false, false);
    let mut mt10x = [false; 5];
    for s in endf.sections().iter().filter(|s| s.key.mat == mat) {
        let (mfd, mtd) = (s.key.mf, s.key.mt);
        if (3..30).contains(&mfd) && (mtd == 2 || mtd > 4) {
            if mfd == 3 {
                ntr += 1;
                for (i, lo) in [600, 650, 700, 750, 800].iter().enumerate() {
                    if mtd >= *lo && mtd <= lo + 49 {
                        mt10x[i] = true;
                    }
                }
            }
            if mtd == 2 {
                ielas = true;
            }
            if mfd == 6 && (201..=207).contains(&mtd) {
                return Err(NjoyError::EndfParse("acephn: mf=6/mt=201-207 not supported".into()));
            }
            if mfd == 4 {
                mf4 = true;
            }
            if mfd == 6 {
                mf6 = true;
            }
            dict.push((mfd, mtd, 0));
        }
    }
    if ielas {
        return Err(NjoyError::NotPorted(
            "acephn: an MF=3/MT=2 elastic section (ielas = 1) -- the ELS block \
             and its separate NON accounting are not ported",
        ));
    }
    // ~~MF=4 refused (the MF=4/MF=5 neutron-only representation was not
    // ported)~~ **CORRECTED 2026-11-13 (gh:#1369, deck 56):** translated in
    // `super::mf45`.

    // ── the nubar tabulation (`:178-205`) ──────────────────────────────────
    let mut fnubar: Option<Tab1> = None;
    for mt in [452, 456] {
        if let Some(s) = endf.section(mat, 1, mt) {
            let mut cur = SectionCursor::new(&s.rows);
            let h = cur.read_cont()?;
            if h.l2 == 1 {
                return Err(NjoyError::EndfParse(format!(
                    "acephn: mf=1/mt={mt} uses polynomial representation, which \
                     upstream does not support for photonuclear ACE files"
                )));
            }
            fnubar = Some(cur.read_tab1()?);
        }
    }

    // ── the energy grid from the PENDF's first MF=3 section (`:207-227`) ───
    let pendf3: Vec<&crate::endf::tape::Section> =
        pendf.sections().iter().filter(|s| s.key.mat == mat && s.key.mf == 3).collect();
    let first = *pendf3
        .first()
        .ok_or_else(|| NjoyError::EndfParse(format!("acephn: the PENDF has no MF=3 for MAT {mat}")))?;
    let mttot = first.key.mt;
    let t_tot = tab1_of(&first.rows)?;
    let mut g = Gety1::new(&t_tot);
    let mut grid: Vec<f64> = Vec::new();
    let mut enext = g.get(0.0).xnext;
    while enext < ETOP {
        let e = enext;
        let v = g.get(e);
        enext = v.xnext;
        if grid.is_empty() && v.y != 0.0 {
            grid.push(sigfig(e, 7, -1));
        }
        grid.push(e);
        if grid.len() > 5_000_000 {
            return Err(NjoyError::EndfParse("acephn: the MF=3 grid walk did not terminate".into()));
        }
    }
    let nes = grid.len();

    // ── locators (`:236-256`) ───────────────────────────────────────────────
    let esz = 1usize;
    let tot = esz + nes;
    let (non, els, thn) = (tot, 0usize, tot + nes);
    let mut x = Xss(vec![0.0; 8 * nes + 1024]);
    for (i, &e) in grid.iter().enumerate() {
        x.set(esz + i, e);
    }
    let mut nex = thn + nes;
    let mtr = nex;
    let lqr = mtr + ntr;
    let lsig = lqr + ntr;
    let sig = lsig + ntr;
    nex = sig;

    // ── the reaction cross sections, from the PENDF (`:258-327`) ───────────
    let start = if mttot == 5 {
        pendf3.iter().position(|s| s.key.mt == 5).unwrap_or(0)
    } else {
        1
    };
    let mut rxs: Vec<Rx> = Vec::new();
    for s in &pendf3[start..] {
        let mth = s.key.mt;
        if !(mth == 2 || mth > 4) {
            continue;
        }
        if (103..=107).contains(&mth) && mt10x[(mth - 103) as usize] {
            continue;
        }
        let t = tab1_of(&s.rows)?;
        let mut gg = Gety1::new(&t);
        let mut e = gg.get(0.0).xnext;
        if gg.get(e).y != 0.0 {
            e = sigfig(e, 7, -1);
        }
        let mut j = nes;
        while j >= 1 && e <= (1.0 + EPS) * x.get(esz + j - 1) {
            j -= 1;
        }
        j += 1;
        let ir = rxs.len();
        x.set(mtr + ir, f64::from(mth));
        x.set(lqr + ir, sigfig(t.head.c2 / EMEV, 7, 0));
        x.set(lsig + ir, (nex - sig + 1) as f64);
        x.set(nex, j as f64);
        let nnex = nex + 1;
        let k = nex;
        nex += 2;
        let mut n = 0usize;
        let mut gg = Gety1::new(&t);
        for i in j..=nes {
            let e = x.get(i);
            let s = sigfig(gg.get(e).y, 7, 0);
            *x.at(tot + i - 1) += s;
            x.set(nex, s);
            n += 1;
            nex += 1;
        }
        x.set(nnex, n as f64);
        rxs.push(Rx { mt: mth, q: x.get(lqr + ir), k, iaa: j });
    }
    if rxs.len() != ntr {
        return Err(NjoyError::EndfParse(format!(
            "acephn: the dictionary promised {ntr} MF=3 reactions and the PENDF has {}",
            rxs.len()
        )));
    }
    let find_rx = |mt: i32| -> Result<Rx, NjoyError> {
        rxs.iter()
            .copied()
            .find(|r| r.mt == mt)
            .ok_or_else(|| NjoyError::EndfParse(format!("acephn: MT={mt} has no MF=3 cross section")))
    };

    // The MF=6 sections, in tape order.
    let mut mf6s: Vec<Mf6> = Vec::new();
    for s in endf.sections().iter().filter(|s| s.key.mat == mat && s.key.mf == 6) {
        mf6s.push(read_mf6(s.key.mt, &s.rows)?);
    }
    let mfs5: Vec<i32> = dict.iter().filter(|d| d.0 == 5).map(|d| d.1).collect();

    // ── pass 1: count the particles, find the thresholds, recoil heating
    // (`:329-605`) ───────────────────────────────────────────────────────────
    let order = [1, 0, 1001, 1002, 1003, 2003, 2004];
    let mut count = [0usize; 7];
    let mut thr = [ETOP; 7];
    // An MF=4 section is one neutron-producing reaction (`:348-366`).
    let mf4_rxs: Vec<Reaction> = dict
        .iter()
        .filter(|d| d.0 == 4)
        .map(|d| find_rx(d.1).map(|r| Reaction { mt: r.mt, q: r.q, k: r.k, iaa: r.iaa }))
        .collect::<Result<_, _>>()?;
    for rx in &mf4_rxs {
        count[0] += 1;
        let thresh = x.get(esz + rx.iaa - 1);
        if thresh < thr[0] {
            thr[0] = thresh;
        }
    }
    for m in &mf6s {
        let mth = m.mt;
        for sub in &m.subs {
            let mut y = sub.y.clone();
            let izap = sub.izap();
            let law = sub.law();
            if let Some(slot) = order.iter().position(|&z| z == izap) {
                count[slot] += 1;
            }
            if mth == 18 && izap == 1 {
                if let Some(nu) = &fnubar {
                    y = copynubar(&y, nu);
                }
            }
            let rx = find_rx(mth)?;
            let thresh = x.get(esz + rx.iaa - 1);
            if izap <= 2004 {
                // The first energy whose *next* yield is positive (`:417-424`).
                let mut e = thresh;
                let mut found = false;
                for w in y.pairs.windows(2) {
                    if w[1].1 > 0.0 {
                        e = w[0].0;
                        found = true;
                        break;
                    }
                }
                if !found {
                    // **Deliberate divergence on undefined behaviour** (gh:#1369,
                    // upstream test 64, Ra-226 MT=11 photons): upstream's
                    // `do while (ynext.le.zero)` has no bound and, on an all-zero
                    // yield, reads past the TAB1 into stale `scr` words. Its `e`
                    // only enters the `min` below, so an all-zero yield is taken
                    // as giving no production threshold.
                    continue;
                }
                if e < thresh {
                    e = thresh;
                }
                if let Some(slot) = order.iter().position(|&z| z == izap) {
                    if e < thr[slot] {
                        thr[slot] = e;
                    }
                }
            } else if law == 1 {
                // Recoil heating from a LAW=1 recoil spectrum (`:441-504`).
                let (Some(t2), lists) = (&sub.tab2, &sub.lists) else { unreachable!() };
                let ne = t2.head.n2.max(0) as usize;
                let mut htab: Vec<(f64, f64)> = Vec::with_capacity(ne);
                for l in lists.iter().take(ne) {
                    let e = l.head.c2;
                    let na = l.head.l2.max(0) as usize;
                    let np = l.head.n2.max(0) as usize;
                    let yv = terp(&y, e);
                    let mut heat = 0.0f64;
                    let (mut epl, mut gl) = (0.0f64, 0.0f64);
                    for ip in 1..=np {
                        let ep = l.data[(na + 2) * (ip - 1)];
                        let gg = l.data[1 + (na + 2) * (ip - 1)];
                        if ip > 1 {
                            heat += (ep - epl) * gl * (ep + epl) / 2.0;
                        }
                        epl = ep;
                        gl = gg;
                    }
                    htab.push((e, yv * heat));
                }
                for ie in rx.iaa..=nes {
                    let e = x.get(esz + ie - 1);
                    let mut h = terp_lin(&htab, e);
                    h = (h / EMEV) * x.get(2 + rx.k + ie - rx.iaa);
                    if x.get(tot + ie - 1) != 0.0 {
                        h /= x.get(tot + ie - 1);
                    }
                    let v = sigfig(x.get(thn + ie - 1) + h, 7, 0);
                    x.set(thn + ie - 1, v);
                }
                for d in dict.iter_mut().filter(|d| d.0 == 6 && d.1 == mth) {
                    d.2 += 1;
                }
            } else if law == 4 {
                // Heating from a discrete recoil (`acepn.f90:507-584`, gh:#1369,
                // upstream test 64): upstream backs up to the section's
                // **first** subsection (`findf`, its TAB1 gives `awp`, its TAB2
                // and LISTs the incident energies, read as a LAW=2 two-body
                // distribution) and tabulates `awp*(e+q*emev)/(awr-awp)` at each
                // LIST's `e`; `q` is this reaction's LQR (MeV).
                let first = m.subs.first().ok_or_else(|| NjoyError::EndfParse(format!("acephn: MF=6/MT={mth} has no subsection")))?;
                let awp = first.y.head.c2;
                let ne = first.tab2.as_ref().map_or(0, |t| t.head.n2.max(0) as usize);
                let htab: Vec<(f64, f64)> = first
                    .lists
                    .iter()
                    .take(ne)
                    .map(|l| {
                        let e = l.head.c2;
                        (e, awp * (e + rx.q * EMEV) / (awr - awp))
                    })
                    .collect();
                for ie in rx.iaa..=nes {
                    let e = x.get(esz + ie - 1);
                    let mut h = terp_lin(&htab, e);
                    h = (h / EMEV) * x.get(2 + rx.k + ie - rx.iaa);
                    if x.get(tot + ie - 1) != 0.0 {
                        h /= x.get(tot + ie - 1);
                    }
                    let v = sigfig(x.get(thn + ie - 1) + h, 7, 0);
                    x.set(thn + ie - 1, v);
                }
                for d in dict.iter_mut().filter(|d| d.0 == 6 && d.1 == mth) {
                    d.2 += 1;
                }
            }
            // Law 0 and the rest: upstream only prints a message (`:586-595`).
        }
    }

    // ── e+q heating for capture with no recoil, and for MF=6 sections with no
    // recoil (`:607-641`) ────────────────────────────────────────────────────
    let mut icapt = false;
    for d in &dict {
        if d.0 == 3 && d.1 == 102 {
            icapt = true;
        }
        if d.0 == 6 && d.1 == 102 && d.2 > 0 {
            icapt = false;
        }
    }
    for rx in &rxs {
        if rx.mt == 102 && icapt {
            for ie in rx.iaa..=nes {
                let e = x.get(esz + ie - 1);
                if x.get(tot + ie - 1) != 0.0 {
                    let v = x.get(thn + ie - 1) + (e / EMEV) * x.get(2 + rx.k + ie - rx.iaa) / x.get(tot + ie - 1);
                    x.set(thn + ie - 1, v);
                }
            }
        }
    }
    for rx in &rxs {
        for d in dict.iter().filter(|d| d.0 == 6 && d.1 == rx.mt && d.2 == 0) {
            let _ = d;
            for ie in rx.iaa..=nes {
                let e = x.get(esz + ie - 1);
                if x.get(tot + ie - 1) != 0.0 {
                    let v = x.get(thn + ie - 1) + (e / EMEV + rx.q) * x.get(2 + rx.k + ie - rx.iaa) / x.get(tot + ie - 1);
                    x.set(thn + ie - 1, v);
                }
            }
        }
    }

    // ── the IXSA table (`:643-692`) ─────────────────────────────────────────
    let npixs = 2usize;
    let neixs = 12usize;
    let ipts = [1, 2, 9, 31, 32, 33, 34];
    let ixsa = nex;
    let mut ntype = 0usize;
    let mut types: Vec<usize> = Vec::new();
    for slot in 0..7 {
        if count[slot] > 0 {
            ntype += 1;
            types.push(slot);
            x.set(ixsa + neixs * (ntype - 1), f64::from(ipts[slot]));
            x.set(ixsa + neixs * (ntype - 1) + 1, count[slot] as f64);
        }
    }
    nex = ixsa + neixs * ntype;
    let ixs = nex;

    // ── one production block per particle (`:696-1813`) ─────────────────────
    for (itype0, &slot) in types.iter().enumerate() {
        let row = ixsa + neixs * itype0;
        let ip = order[slot];
        let ntrp = count[slot];
        let thresh = thr[slot];
        let mut it = 1usize;
        while it < nes && x.get(esz + it - 1) < thresh {
            it += 1;
        }
        if it > 1 {
            it -= 1;
        }
        let pxs = nex;
        x.set(row + 2, pxs as f64);
        x.set(pxs, it as f64);
        x.set(pxs + 1, (nes - it + 1) as f64);
        let phn = pxs + 2 + nes - it + 1;
        x.set(row + 3, phn as f64);
        x.set(phn, it as f64);
        x.set(phn + 1, (nes - it + 1) as f64);
        let mtrp = phn + 2 + nes - it + 1;
        x.set(row + 4, mtrp as f64);
        let tyrp = mtrp + ntrp;
        x.set(row + 5, tyrp as f64);
        let lsigp = tyrp + ntrp;
        x.set(row + 6, lsigp as f64);
        let sigp = lsigp + ntrp;
        x.set(row + 7, sigp as f64);
        nex = sigp;

        let blk = Block { esz, tot, thn, nes, it, pxs, phn, mtrp, tyrp, lsigp, sigp, ldlwp: 0, dlwp: 0 };
        let mut jp = 0usize;
        // MF=4/5 representations, neutrons only (`:742-833`).
        if mf4 && ip == 1 {
            mf4_yields(&mut x, &blk, &mf4_rxs, fnubar.as_ref(), &mut jp, &mut nex)?;
        }
        // MF=6 production cross sections and yields (`:830-916`).
        for m in &mf6s {
            let mth = m.mt;
            let rx = find_rx(mth)?;
            for sub in &m.subs {
                if sub.izap() != ip {
                    continue;
                }
                let mut y = sub.y.clone();
                if mth == 18 && ip == 1 {
                    if let Some(nu) = &fnubar {
                        y = copynubar(&y, nu);
                    }
                }
                jp += 1;
                x.set(mtrp + jp - 1, f64::from(mth));
                x.set(lsigp + jp - 1, (nex - sigp + 1) as f64);
                x.set(tyrp + jp - 1, if m.lct == 1 { 1.0 } else { -1.0 });
                for i in rx.iaa..=nes {
                    let e = x.get(esz + i - 1);
                    let yv = terp(&y, e);
                    let ss = x.get(2 + rx.k + i - rx.iaa);
                    let tt = x.get(pxs + 2 + i - it) + yv * ss;
                    x.set(pxs + 2 + i - it, sigfig(tt, 7, 0));
                }
                // `scr(7+2*i)`: upstream assumes one interpolation region here.
                let f = flat(&y);
                x.set(nex, 6.0);
                x.set(nex + 1, f64::from(mth));
                x.set(nex + 2, 0.0);
                let ne = y.pairs.len();
                x.set(nex + 3, ne as f64);
                for i in 1..=ne {
                    x.set(nex + 3 + i, sigfig(f[6 + 2 * i] / EMEV, 7, 0));
                    x.set(nex + 3 + i + ne, sigfig(f[7 + 2 * i], 7, 0));
                }
                nex += 4 + 2 * ne;
            }
        }

        // LANDP / ANDP: MF=6 LAW=2 angular distributions (`:918-1150`).
        let landp = nex;
        x.set(row + 8, landp as f64);
        let andp = landp + ntrp;
        x.set(row + 9, andp as f64);
        nex = andp;
        for i in 1..=ntrp {
            let mt = x.get(mtrp + i - 1).round() as i32;
            let rx = find_rx(mt)?;
            let Some(m) = mf6s.iter().find(|m| m.mt == mt) else { continue };
            for sub in &m.subs {
                if !(sub.izap() == ip && sub.law() == 2) {
                    continue;
                }
                let (Some(t2), lists) = (&sub.tab2, &sub.lists) else { unreachable!() };
                let jp = i;
                let awp = sub.y.head.c2;
                x.set(landp + jp - 1, (nex - andp + 1) as f64);
                let ne = t2.head.n2.max(0) as usize;
                x.set(nex, ne as f64);
                let ie0 = nex;
                let il = ie0 + ne;
                nex = il + ne + 1;
                let mut htab: Vec<(f64, f64)> = Vec::with_capacity(ne);
                for (iie, l) in lists.iter().enumerate().take(ne) {
                    let iie = iie + 1;
                    let lang = l.head.l1;
                    let (mu, pdf) = if lang == 0 {
                        let nl = l.head.n2.max(0) as usize;
                        crate::acer::acensd::ptleg2(&l.data[..nl.min(l.data.len())])?
                    } else {
                        let nn = l.head.n2.max(0) as usize;
                        let pairs: Vec<(f64, f64)> = (0..nn).map(|q| (l.data[2 * q], l.data[2 * q + 1])).collect();
                        crate::acer::acensd::pttab2(&pairs)
                    };
                    x.set(ie0 + iie, sigfig(l.head.c2 / EMEV, 7, 0));
                    let n = mu.len();
                    x.set(il + iie, -((nex - andp + 1) as f64));
                    x.set(nex, 2.0);
                    x.set(nex + 1, n as f64);
                    for ii in 1..=n {
                        x.set(nex + 1 + ii, sigfig(mu[ii - 1], 7, 0));
                        let mut p = sigfig(pdf[ii - 1], 7, 0);
                        if p < RMIN {
                            p = 0.0;
                        }
                        x.set(nex + 1 + n + ii, p);
                        if ii == 1 {
                            x.set(nex + 1 + 2 * n + ii, 0.0);
                        } else {
                            let sum = x.get(nex + 1 + 2 * n + ii - 1)
                                + (x.get(nex + 1 + n + ii) + x.get(nex + 1 + n + ii - 1))
                                    * (x.get(nex + 1 + ii) - x.get(nex + 1 + ii - 1))
                                    / 2.0;
                            x.set(nex + 1 + 2 * n + ii, sum);
                        }
                    }
                    let renorm = 1.0 / x.get(nex + 1 + 3 * n);
                    for ii in 1..=n {
                        let p = sigfig(renorm * x.get(nex + 1 + n + ii), 7, 0);
                        x.set(nex + 1 + n + ii, p);
                        let c = sigfig(renorm * x.get(nex + 1 + 2 * n + ii), 9, 0);
                        x.set(nex + 1 + 2 * n + ii, c);
                    }
                    nex += 2 + 3 * n;
                    let e = x.get(ie0 + iie);
                    htab.push((e, (awr - awp) * (e + rx.q) / awr));
                }
                // Heating from the two-body kinematics (`:1102-1137`).
                for ie in it..=nes {
                    let e = x.get(esz + ie - 1) / EMEV;
                    let h0 = terp_lin(&htab, e);
                    let yld = terp(&sub.y, e * EMEV);
                    let h = yld * h0;
                    let ss = if ie >= rx.iaa { x.get(2 + rx.k + ie - rx.iaa) } else { 0.0 };
                    let v = x.get(phn + 2 + ie - it) + h * ss;
                    x.set(phn + 2 + ie - it, v);
                    if x.get(tot + ie - 1) != 0.0 {
                        let v = x.get(thn + ie - 1) + h * ss / x.get(tot + ie - 1);
                        x.set(thn + ie - 1, v);
                    }
                }
            }
        }

        // LDLWP / DLWP (`:1152-1159`).
        let ldlwp = nex;
        x.set(row + 10, ldlwp as f64);
        let dlwp = ldlwp + ntrp;
        x.set(row + 11, dlwp as f64);
        nex = dlwp;
        let blk = Block { ldlwp, dlwp, ..blk };
        let reaction = |mt: i32| find_rx(mt).map(|r| Reaction { mt: r.mt, q: r.q, k: r.k, iaa: r.iaa });
        // ~~An MF=5 distribution refused~~ **CORRECTED 2026-11-13 (gh:#1369):**
        // MF=4 discrete levels as ACE law 33 (`:1166-1212`), then MF=5
        // (`:1214-1425`), in `super::mf45`.
        for i in 1..=ntrp {
            let mt = x.get(mtrp + i - 1).round() as i32;
            if dict.iter().any(|d| d.0 == 4 && d.1 == mt) && (50..91).contains(&mt) {
                law33_discrete(&mut x, &blk, reaction(mt)?, awr, i, &mut nex);
            }
        }
        for i in 1..=ntrp {
            let mt = x.get(mtrp + i - 1).round() as i32;
            if mfs5.contains(&mt) {
                let s = endf
                    .section(mat, 5, mt)
                    .ok_or_else(|| NjoyError::EndfParse(format!("acephn: no MF=5/MT={mt} for MAT {mat}")))?;
                mf5_distribution(&mut x, &blk, reaction(mt)?, &s.rows, fnubar.as_ref(), i, &mut nex)?;
            }
        }

        // MF=6 energy distributions (`:1427-1800`).
        let mut jp = 0usize;
        for i in 1..=ntrp {
            let mt = x.get(mtrp + i - 1).round() as i32;
            let Some(m) = mf6s.iter().find(|m| m.mt == mt) else { continue };
            jp = i;
            let mth = m.mt;
            let rx = find_rx(mth)?;
            for sub in &m.subs {
                if sub.izap() != ip {
                    continue;
                }
                let mut y = sub.y.clone();
                if mth == 18 && ip == 1 {
                    if let Some(nu) = &fnubar {
                        y = copynubar(&y, nu);
                    }
                }
                let awp = y.head.c2;
                let law = sub.law();
                x.set(ldlwp + jp - 1, (nex - dlwp + 1) as f64);
                let last = nex;
                x.set(last, 0.0);
                x.set(last + 1, 0.0);
                x.set(last + 2, 0.0);
                nex += 3;
                if law == 1 {
                    let (Some(t2), lists) = (&sub.tab2, &sub.lists) else { unreachable!() };
                    let lang = t2.head.l1;
                    let lep = t2.head.l2;
                    let ne = t2.head.n2.max(0) as usize;
                    match lang {
                        1 => x.set(last + 1, 61.0),
                        2 => x.set(last + 1, 44.0),
                        _ => {
                            return Err(NjoyError::EndfParse(format!(
                                "acephn: lang={lang} not supported for law=1"
                            )))
                        }
                    }
                    x.set(landp + jp - 1, -1.0);
                    x.set(nex, 0.0);
                    let lee = nex;
                    nex += 1;
                    let nle = 2usize;
                    x.set(nex, nle as f64);
                    nex += 1 + 2 * nle;
                    x.set(last + 2, (nex - dlwp + 1) as f64);
                    x.set(nex, 0.0);
                    nex += 1;
                    x.set(nex, ne as f64);
                    nex += 1;
                    let lle = nex;
                    nex = lle + 2 * ne;
                    let mut htab: Vec<(f64, f64)> = Vec::with_capacity(ne);
                    for (ie, l) in lists.iter().enumerate().take(ne) {
                        let ie = ie + 1;
                        let nd = l.head.l1.max(0) as usize;
                        let na = l.head.l2.max(0) as usize;
                        let ng = l.head.n2.max(0) as usize;
                        let ncyc = na + 2;
                        let d = |k: usize| l.data.get(k).copied().unwrap_or(0.0);
                        let e_mev = sigfig(l.head.c2 / EMEV, 7, 0);
                        if ie == 1 {
                            x.set(lee + 2, e_mev);
                            x.set(lee + 4, 1.0);
                        } else if ie == ne {
                            x.set(lee + 3, e_mev);
                            x.set(lee + 5, 1.0);
                        }
                        x.set(lle + ie - 1, e_mev);
                        let ee = x.get(lle + ie - 1);
                        x.set(lle + ne + ie - 1, (nex - dlwp + 1) as f64);
                        x.set(nex, f64::from(lep) + 10.0 * nd as f64);
                        x.set(nex + 1, ng as f64);
                        let mut nexc = nex + 2 + 4 * ng;
                        let amass = awp * emc2;
                        let avadd = ee / (awr * emc2);
                        let mut avlab = 0.0f64;
                        let mut avll = 0.0f64;
                        for ig in 1..=ng {
                            let epr = d(ncyc * (ig - 1));
                            let gr = d(1 + ncyc * (ig - 1));
                            x.set(nex + 1 + ig, sigfig(epr / EMEV, 7, 0));
                            if ig <= nd {
                                x.set(nex + 1 + ig + ng, sigfig(gr, 7, 0));
                            } else {
                                x.set(nex + 1 + ig + ng, sigfig(gr * EMEV, 7, 0));
                            }
                            let test = x.get(nex + 1 + ig + ng);
                            if test > 0.0 && test < SMALL {
                                x.set(nex + 1 + ig + ng, SMALL);
                            }
                            if ig == 1 {
                                x.set(nex + 1 + ig + 2 * ng, if nd == 0 { 0.0 } else { gr });
                            } else if ig <= nd {
                                let v = x.get(nex + ig + 2 * ng) + gr;
                                x.set(nex + 1 + ig + 2 * ng, v);
                            } else if ig == nd + 1 {
                                let v = x.get(nex + ig + 2 * ng);
                                x.set(nex + 1 + ig + 2 * ng, v);
                            }
                            if ig > nd + 1 {
                                let epp = d(ncyc * (ig - 2));
                                let gp = d(1 + ncyc * (ig - 2));
                                let v = if lep == 1 {
                                    x.get(nex + ig + 2 * ng) + gp * (epr - epp)
                                } else {
                                    x.get(nex + ig + 2 * ng) + ((gp + gr) / 2.0) * (epr - epp)
                                };
                                if lep == 1 || lep == 2 {
                                    x.set(nex + 1 + ig + 2 * ng, v);
                                }
                            }
                            let (mut rkal, mut akal) = (0.0f64, 0.0f64);
                            if lang == 2 {
                                rkal = d(2 + ncyc * (ig - 1));
                                let ep = x.get(nex + 1 + ig);
                                akal = bachaa(izai, sub.izap(), za, ee, ep)?;
                                x.set(nex + 1 + ig + 3 * ng, sigfig(rkal, 7, 0));
                                x.set(nex + 1 + ig + 4 * ng, sigfig(akal, 7, 0));
                            } else {
                                x.set(nex + 1 + ig + 3 * ng, (nexc - dlwp + 1) as f64);
                                let lll = 1 + ncyc * (ig - 1);
                                let coeffs: Vec<f64> = (1..=na)
                                    .map(|ib| if d(lll) != 0.0 { d(lll + ib) / d(lll) } else { 0.0 })
                                    .collect();
                                let (amu, apdf) = crate::acer::acensd::ptleg2(&coeffs)?;
                                let nmu = amu.len();
                                x.set(nexc, 2.0);
                                x.set(nexc + 1, nmu as f64);
                                for imu in 1..=nmu {
                                    x.set(nexc + 1 + imu, sigfig(amu[imu - 1], 7, 0));
                                    x.set(nexc + 1 + nmu + imu, sigfig(apdf[imu - 1], 7, 0));
                                    if imu == 1 {
                                        x.set(nexc + 1 + 2 * nmu + imu, 0.0);
                                    } else if imu == nmu {
                                        x.set(nexc + 1 + 2 * nmu + imu, 1.0);
                                    } else {
                                        let del = amu[imu - 1] - amu[imu - 2];
                                        let av = (apdf[imu - 1] + apdf[imu - 2]) / 2.0;
                                        let v = x.get(nexc + 1 + 2 * nmu + imu - 1) + del * av;
                                        x.set(nexc + 1 + 2 * nmu + imu, sigfig(v, 7, 0));
                                    }
                                }
                                nexc += 2 + 3 * nmu;
                            }
                            // The average laboratory energy (`:1671-1691`).
                            if ig != 1 {
                                let eavi = x.get(nex + 1 + ig);
                                let avl = if lang != 2 || na == 0 {
                                    eavi
                                } else {
                                    let avcm = (2.0 * eavi / amass).sqrt();
                                    eavl(akal, amass, avcm, avadd, rkal, 1.0)
                                };
                                let dele = x.get(nex + 1 + ig) - x.get(nex + ig);
                                let avav = if lep == 1 {
                                    x.get(nex + ig + ng) * (avll + avl) / 2.0
                                } else {
                                    (x.get(nex + ig + ng) * avll + x.get(nex + ig + 1 + ng) * avl) / 2.0
                                };
                                avlab += avav * dele;
                                avll = avl;
                            }
                        }
                        let renorm = 1.0 / x.get(nex + 1 + 3 * ng);
                        for ig in 1..=ng {
                            let p = sigfig(renorm * x.get(nex + 1 + ng + ig), 7, 0);
                            x.set(nex + 1 + ng + ig, p);
                            let c = sigfig(renorm * x.get(nex + 1 + 2 * ng + ig), 9, 0);
                            x.set(nex + 1 + 2 * ng + ig, c);
                        }
                        htab.push((ee, avlab));
                        if lang == 1 {
                            nex = nexc;
                        } else {
                            nex += 2 + (2 * na + 3) * ng;
                        }
                    }
                    // Heating for this subsection (`:1716-1738`).
                    let ys: Vec<f64> = (rx.iaa..=nes)
                        .map(|ie| terp(&y, x.get(esz + ie - 1)) * x.get(2 + rx.k + ie - rx.iaa))
                        .collect();
                    let it2 = x.get(pxs).round() as usize;
                    let no_recoil = dict.iter().filter(|d| d.0 == 6 && d.1 == mth && d.2 == 0).count();
                    for ie in rx.iaa..=nes {
                        let e = x.get(esz + ie - 1) / EMEV;
                        let h = terp_lin(&htab, e);
                        let hh = h * ys[ie - rx.iaa];
                        let v = x.get(phn + 2 + ie - it2) + hh;
                        x.set(phn + 2 + ie - it2, v);
                        for _ in 0..no_recoil {
                            if x.get(tot + ie - 1) != 0.0 {
                                let v = x.get(thn + ie - 1) - hh / x.get(tot + ie - 1);
                                x.set(thn + ie - 1, v);
                            }
                        }
                    }
                } else if law == 2 {
                    // Discrete two-body: ACE law 33 (`:1740-1773`).
                    let f = flat(&y);
                    let nnr = f[4].round() as usize;
                    let nnp = f[5].round() as usize;
                    x.set(last + 1, 33.0);
                    x.set(nex, 0.0);
                    x.set(nex + 1, 2.0);
                    x.set(nex + 2, sigfig(f[6 + 2 * nnr] / EMEV, 7, 0));
                    x.set(nex + 3, sigfig(f[4 + 2 * nnr + 2 * nnp] / EMEV, 7, 0));
                    x.set(nex + 4, 1.0);
                    x.set(nex + 5, 1.0);
                    nex += 6;
                    x.set(last + 2, (nex - dlwp + 1) as f64);
                    x.set(nex, sigfig(-rx.q, 7, 0));
                    x.set(nex + 1, sigfig((awr - awp) / awr, 7, 0));
                    nex += 2;
                } else if law == 4 {
                    return Err(NjoyError::NotPorted("acephn: MF=6 LAW=4 two-body recoil is not ported"));
                }
                // Other laws: upstream prints "can only do law=1, law=2, or law=4".
            }
        }
        let _ = jp;

        // Divide the heating by the total, fold it in (`:1802-1812`). `ip` is
        // the ZAP, so the neutron (1) and the photon (0) are both excluded.
        for ie in it..=nes {
            if x.get(tot + ie - 1) != 0.0 {
                let v = x.get(phn + 2 + ie - it) / x.get(tot + ie - 1);
                x.set(phn + 2 + ie - it, v);
            }
            if ip > 1 {
                let v = x.get(thn + ie - 1) + x.get(phn + 2 + ie - it);
                x.set(thn + ie - 1, v);
            }
            let v = sigfig(x.get(phn + 2 + ie - it), 7, 0);
            x.set(phn + 2 + ie - it, v);
        }
    }
    let _ = mf6;
    let lxs = nex - 1;

    // ── the grid to MeV, the heating to 7 figures (`:1817-1821`) ─────────────
    for i in 1..=nes {
        let e = sigfig(x.get(esz + i - 1) / EMEV, 7, 0);
        x.set(esz + i - 1, e);
        let h = sigfig(x.get(thn + i - 1), 7, 0);
        x.set(thn + i - 1, h);
    }
    let xss: Vec<f64> = (1..=lxs).map(|k| x.get(k)).collect();

    let mut nxs = [0i32; 16];
    nxs[0] = lxs as i32;
    nxs[1] = za;
    nxs[2] = nes as i32;
    nxs[3] = ntr as i32;
    nxs[4] = ntype as i32;
    nxs[5] = npixs as i32;
    nxs[6] = neixs as i32;
    nxs[8] = is;
    nxs[9] = za / 1000;
    nxs[10] = za % 1000;
    nxs[15] = 1; // tvn
    let mut jxs = [0i32; 32];
    jxs[0] = esz as i32;
    jxs[1] = tot as i32;
    jxs[2] = non as i32;
    jxs[3] = els as i32;
    jxs[4] = thn as i32;
    jxs[5] = mtr as i32;
    jxs[6] = lqr as i32;
    jxs[7] = lsig as i32;
    jxs[8] = sig as i32;
    jxs[9] = ixsa as i32;
    jxs[10] = ixs as i32;

    // The integer/real split comes from the **layout walk**, a port of
    // `phnout` already byte-verified on the read side, so the builder cannot
    // disagree with the reader about where a locator is. A table the walk
    // cannot traverse is one this function built wrong.
    let xss_is_int = crate::acer::photonuclear::layout::walk(&nxs, &jxs, &xss)
        .map_err(|e| {
            NjoyError::EndfParse(format!(
                "acephn: the table just built does not walk -- {e}. This is a defect in the builder, not in the input."
            ))
        })?
        .is_int;
    Ok(PhotonuclearAce { mat, za, awr, nxs, jxs, xss, xss_is_int })
}

/// Read a section's single TAB1 (after its HEAD).
fn tab1_of(rows: &[[f64; 6]]) -> Result<Tab1, NjoyError> {
    let mut c = SectionCursor::new(rows);
    c.read_cont()?;
    c.read_tab1()
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
