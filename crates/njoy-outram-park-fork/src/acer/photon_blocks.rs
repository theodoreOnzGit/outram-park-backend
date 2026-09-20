//! The ACE **photon-production** blocks: `MTRP`, `LSIGP`, `SIGP`, `LANDP`,
//! `ANDP`, `LDLWP`, `DLWP` — `NXS(6) = NTRP` and `JXS(13..19)`.
//!
//! Ports the photon half of `acelpp` (NJOY2016 `acefc.f90` 8214-9014).
//!
//! # Relationship to [`crate::photon`]
//!
//! [`crate::photon::PhotonProduction`] walks the same ENDF files (MF=12 LO=1,
//! MF=13, MF=15) but keeps only what HEATR's energy balance needs: the
//! **first moment** of each continuum spectrum, `Ē_γ(E)`. An ACE table needs
//! the *whole* distribution, plus the discrete-line parameters and the
//! subsection structure, so it cannot be built from that model — and its types
//! are private to that module.
//!
//! This follows the split the crate already uses for MF=5: `acer/energy/mf5.rs`
//! has its own narrower parser beside `nuclear_data`'s. Cited here so the two
//! are not mistaken for duplicates and do not drift.
//!
//! # Block layout
//!
//! One entry per **photon subsection**, not per reaction — which is why U-235
//! has 583 of them against U-234's 6.
//!
//! ```text
//! MTRP[i]  = MT·1000 + k        (k = 1-based subsection index within that MT)
//! LSIGP[i] = 1-based offset into SIGP
//! SIGP     = per entry, one of
//!              MFTYPE=12: [12, MTMULT, NR, (NBT,INT)·NR, NE, E(NE), y(NE)]
//!              MFTYPE=13: [13, IE, NE, sigma(NE)]        (on the ESZ grid)
//! LANDP[i] = 0 ⇒ isotropic (ANDP carries nothing for that entry)
//! LDLWP[i] = 1-based offset into DLWP
//! DLWP     = [LNW=0, LAW, IDAT, NR=0, NE=2, E_lo, E_hi, 1.0, 1.0] + law data
//! ```
//!
//! `MFTYPE` says how to turn the entry into a production rate: `13` is already
//! a cross section, `12` is a **yield** that the reader multiplies by the
//! `MTMULT` reaction's own cross section.
//!
//! # What is covered, and what is not
//!
//! Covered: MF=12 `LO=1` yields, MF=13 cross sections, discrete lines
//! (`LF=2` → ACE Law 2) and continuum spectra (`LF=1` → ACE Law 4 via MF=15).
//!
//! **Not covered:** MF=12 `LO=2` transition-probability cascades, and
//! anisotropic photons (MF=14 with `LI=0`). Both are *detected* and the whole
//! photon block is then refused rather than written wrong — see
//! [`build`]'s return contract. A partial photon block is a malformed table,
//! so refusing is the only safe failure.

use crate::endf::records::SectionCursor;
use crate::endf::tape::Tape;

use super::energy::EnergyLaw;
use super::energy::parse_mf5_law4;

/// eV → MeV.
const EMEV: f64 = 1.0e6;

/// MT=460 — **delayed** photon data, which ACER excludes from the prompt
/// photon-production block entirely.
///
/// Upstream guards it in three separate places, and all three are needed:
/// `convr` leaves it out of the `gmt` list (`acefc.f90:400`,
/// `mfd.eq.12.and.mtd.ne.460`), `gamout`'s counting pass skips the section
/// before it can add `NK` to `ntrpp` (`:3642`), and `gamout`'s writing pass
/// skips both MF=12 and MF=14 for it (`:4030`).
///
/// It is not a small correction. U-235 ENDF/B-VII.0 carries **`NK = 3262`**
/// subsections under MF=12/MT=460, so a port without this guard writes 3295
/// photon entries where NJOY2016 writes 33 — a hundredfold over-production
/// that leaves every LSIGP/LDLWP locator pointing at the wrong data.
const MT_DELAYED_PHOTON: i32 = 460;

/// How a photon-production entry states its production rate.
#[derive(Debug, Clone)]
pub enum SigP {
    /// A **yield** to be multiplied by reaction `mtmult`'s cross section.
    ///
    /// `mftype` is `12` when the yield came from MF=12, and `16` when it came
    /// from a photon subsection of MF=6 — ACE distinguishes the two even
    /// though a reader treats both as "multiply by MTMULT's cross section".
    Yield {
        /// `12` (from MF=12) or `16` (from MF=6).
        mftype: i32,
        /// The MT whose cross section this yield multiplies.
        mtmult: i32,
        /// Incident energies \[MeV\].
        e_mev: Vec<f64>,
        /// Yield (photons per reaction) aligned with `e_mev`.
        y: Vec<f64>,
    },
    /// `MFTYPE = 13` — a photon-production cross section \[barn\], kept as the
    /// evaluation's own tabulation.
    ///
    /// It is **not** resampled here: ACE stores it on the table's union energy
    /// grid, which is built inside `AceTable::build`, so tabulating it any
    /// earlier would index a different grid. The writer does it.
    Xs {
        /// ENDF interpolation regions `(NBT, INT)`.
        interp: Vec<(u32, u32)>,
        /// `(E \[eV\], sigma \[barn\])` pairs.
        pairs: Vec<(f64, f64)>,
    },
}

/// One photon-production entry: one subsection of one reaction.
#[derive(Debug, Clone)]
pub struct PhotonEntry {
    /// `MT·1000 + k`.
    pub mtrp: i32,
    /// The production rate and how to read it.
    pub sigp: SigP,
    /// The photon's energy distribution (ACE Law 2 or Law 4).
    pub law: EnergyLaw,
    /// Lower bound of the law's applicability \[MeV\].
    pub e_lo_mev: f64,
    /// Upper bound of the law's applicability \[MeV\].
    pub e_hi_mev: f64,
}

/// Build every photon-production entry for `mat`.
///
/// Returns `None` when the evaluation uses a form this port does not write —
/// MF=12 `LO=2` cascades, or anisotropic photons (MF=14 with `LI=0`). In that
/// case the caller must leave `NXS(6) = 0`, which is a **legal** ACE table
/// meaning "no photon production", rather than emitting a partial block, which
/// would be a malformed one.
///
/// Returns `Some(vec![])` for a material with no photon data at all — also a
/// legal table, and not an error.
pub fn build(tape: &Tape, mat: i32) -> Option<Vec<PhotonEntry>> {
    // Anisotropic photons are not written. MF=14 with LI=1 is "all isotropic"
    // and needs nothing; LI=0 carries real distributions we cannot serialise.
    // From MT=1: MT=0 is a section TERMINATOR (SEND), not a section. Its
    // record carries zeros, so reading it as an MF=14 HEAD sees LI=0 and
    // wrongly concludes the photons are anisotropic — which silently
    // suppressed the whole block on the first run of this code.
    for mt in 1..1000 {
        if mt == MT_DELAYED_PHOTON {
            continue; // `acefc.f90:4030` — MF=14/MT=460 is skipped outright
        }
        if let Some(sec) = tape.section(mat, 14, mt) {
            let mut cur = SectionCursor::new(&sec.rows);
            if let Ok(h) = cur.read_cont() {
                if h.l1 == 0 {
                    return None; // LI=0 ⇒ anisotropic
                }
            }
        }
    }

    // ORDER MATTERS: NJOY emits every MF=12 (yield) entry first, then every
    // MF=13 (cross section) one, because `gamout` walks the two files in turn.
    // The MTRP *set* is the same either way, but the LSIGP/LDLWP locators are
    // positional, so a table in MT order would pair each entry with the wrong
    // SIGP and DLWP. Verified against NJOY's U-234 table, whose MTRP reads
    // [18001, 102001, 3001..3004] — MF=12's MT=18 and MT=102 ahead of MF=13's
    // MT=3, not ascending by MT.
    // The whole level scheme, read once: every MT whose MF=12 is LO=2
    // contributes the transitions OUT of one level, and a cascade started at
    // any level walks through the others.
    let levels = read_levels(tape, mat);

    // Upper bound of the synthesised yield tables — the evaluation's own top
    // energy, taken from MF=3 MT=1 rather than assumed to be 20 or 30 MeV.
    let e_max_ev = tape
        .section(mat, 3, 1)
        .and_then(|sec| {
            let mut c = SectionCursor::new(&sec.rows);
            c.read_cont().ok()?;
            let t = c.read_tab1().ok()?;
            t.pairs.last().map(|&(e, _)| e)
        })
        .unwrap_or(2.0e7);

    let mut out = Vec::new();
    for (file, is_yield) in [(12i32, true), (13i32, false)] {
    for mt in 1..1000 {
        if file == 12 && mt == MT_DELAYED_PHOTON {
            continue; // `acefc.f90:3642` — MF=12/MT=460 never reaches MTRP
        }
        let Some(sec) = tape.section(mat, file, mt) else { continue };
        let mut cur = SectionCursor::new(&sec.rows);
        let Ok(head) = cur.read_cont() else { continue };
        if is_yield && head.l1 == 2 {
            // LO=2 transition-probability cascade — expanded below, from the
            // level data gathered once before this loop.
            let awr = head.c2;
            for e in expand_cascade(&levels, mt, awr, e_max_ev) {
                out.push(e);
            }
            continue;
        }
        if is_yield && head.l1 != 1 {
            return None;
        }
        let nk = head.n1;
        if nk > 1 {
            // A leading total (yield or cross section) precedes the
            // subsections when there is more than one; it is redundant with
            // their sum and ACE does not store it.
            if cur.read_tab1().is_err() {
                return None;
            }
        }
        // MF=15 continuum spectra for this MT, consumed in order by the
        // LF=1 subsections.
        let mut mf15_used = false;

        for k in 1..=nk {
            let Ok(tab) = cur.read_tab1() else { return None };
            let eg_ev = tab.head.c1;
            let lp = tab.head.l1;
            let lf = tab.head.l2;
            if tab.pairs.is_empty() {
                continue;
            }
            let e_first = tab.pairs.first().map(|&(e, _)| e).unwrap_or(0.0);
            let e_last = tab.pairs.last().map(|&(e, _)| e).unwrap_or(0.0);

            let law = if lf == 2 || lf == 0 {
                EnergyLaw::Law2 { lp, eg_mev: eg_ev / EMEV }
            } else {
                // LF=1 ⇒ the spectrum lives in MF=15 for this MT.
                if mf15_used {
                    // More than one continuum subsection for one MT: the
                    // mapping to MF=15's own subsections is not established
                    // here, so refuse rather than guess.
                    return None;
                }
                mf15_used = true;
                let sec15 = tape.section(mat, 15, mt)?;
                let l4 = parse_mf5_law4(sec15).ok()?;
                EnergyLaw::Law4(l4)
            };

            let sigp = if is_yield {
                SigP::Yield {
                    mftype: 12,
                    mtmult: mt,
                    e_mev: tab.pairs.iter().map(|&(e, _)| e / EMEV).collect(),
                    y: tab.pairs.iter().map(|&(_, y)| y).collect(),
                }
            } else {
                SigP::Xs { interp: tab.interp.clone(), pairs: tab.pairs.clone() }
            };

            out.push(PhotonEntry {
                mtrp: mt * 1000 + k,
                sigp,
                law,
                e_lo_mev: e_first / EMEV,
                e_hi_mev: e_last / EMEV,
            });
        }
    }
    }

    // THIRD PASS: photon production given in MF=6 as a ZAP=0 secondary.
    //
    // Several evaluations put the continuum reactions' photons here instead of
    // in MF=12/13 — U-238's MT=5, 16, 17, 91, 102 and 649 all do — which is
    // upstream's "move any MF=6 photon production" (`convr`). These entries go
    // LAST and in ascending MT: verified against NJOY's U-238 table, where
    // they occupy positions 352-357 of 358, after every MF=12 and MF=13 entry.
    // The order matters because LSIGP and LDLWP are positional.
    for mt in 1..1000 {
        let Some(sec) = tape.section(mat, 6, mt) else { continue };
        let Ok(products) = super::energy::parse_mf6_law1_products(sec, 0) else { continue };
        for (k, prod) in products.into_iter().enumerate() {
            if prod.yield_pairs.is_empty() {
                continue;
            }
            let e_first = prod.yield_pairs.first().map(|&(e, _)| e).unwrap_or(0.0);
            let e_last = prod.yield_pairs.last().map(|&(e, _)| e).unwrap_or(0.0);
            out.push(PhotonEntry {
                mtrp: mt * 1000 + k as i32 + 1,
                sigp: SigP::Yield {
                    mftype: 16,
                    mtmult: mt,
                    e_mev: prod.yield_pairs.iter().map(|&(e, _)| e / EMEV).collect(),
                    y: prod.yield_pairs.iter().map(|&(_, y)| y).collect(),
                },
                law: EnergyLaw::Law4(prod.law4),
                e_lo_mev: e_first / EMEV,
                e_hi_mev: e_last / EMEV,
            });
        }
    }

    Some(out)
}

/// One excited level's decay data, from its MF=12 `LO=2` section.
#[derive(Debug, Clone)]
struct Level {
    /// The MT whose MF=12 section declared this level (51 → first level, …).
    mt: i32,
    /// Level energy `ES` \[eV\].
    es_ev: f64,
    /// `LG`: 2 when a photon fraction `GP` accompanies each transition
    /// probability, 1 when every transition emits a photon.
    lg: i32,
    /// `(E_lower [eV], TP, GP)` for each transition out of this level.
    trans: Vec<(f64, f64, f64)>,
}

/// Read every MF=12 `LO=2` level for this material, keyed by MT.
fn read_levels(tape: &Tape, mat: i32) -> Vec<Level> {
    let mut out = Vec::new();
    for mt in 1..1000 {
        if mt == MT_DELAYED_PHOTON {
            continue;
        }
        let Some(sec) = tape.section(mat, 12, mt) else { continue };
        let mut cur = SectionCursor::new(&sec.rows);
        let Ok(head) = cur.read_cont() else { continue };
        if head.l1 != 2 {
            continue;
        }
        let lg = head.l2;
        let Ok(list) = cur.read_list() else { continue };
        let es_ev = list.head.c1;
        let nt = list.head.n2 as usize;
        let stride = (lg + 1) as usize; // (E, TP) or (E, TP, GP)
        let mut trans = Vec::with_capacity(nt);
        for k in 0..nt {
            let b = k * stride;
            let Some(&e_j) = list.data.get(b) else { break };
            let tp = list.data.get(b + 1).copied().unwrap_or(0.0);
            let gp = if lg == 2 {
                list.data.get(b + 2).copied().unwrap_or(1.0)
            } else {
                1.0
            };
            trans.push((e_j, tp, gp));
        }
        out.push(Level { mt, es_ev, lg, trans });
    }
    out
}

/// Expand the γ cascade that follows populating one level, into discrete
/// photon lines.
///
/// This is `convr`'s LO=2 → LO=1 conversion (`acefc.f90` 3868-4514). A
/// reaction MT=51+n leaves the nucleus in level `n`; the nucleus then decays
/// down the level scheme, and **every** photon emitted on the way belongs to
/// that reaction. That is why MT=52 produces two lines in NJOY's table and
/// MT=54 produces four.
///
/// **`TP` and `GP` do different jobs, and conflating them is the easy error.**
/// `TP` is the probability the level decays *via that transition*, so it
/// carries the population downward. `GP` is the fraction of those decays that
/// emit a photon rather than an internal-conversion electron, so it scales the
/// *yield* only. Verified against U-238: its first level is almost entirely
/// converted (`GP = 1.639e-3`), which is exactly why NJOY's `52002` repeats
/// `51001`'s tiny yield while the population flowing through that level is 1.
fn expand_cascade(levels: &[Level], mt: i32, awr: f64, e_max_ev: f64) -> Vec<PhotonEntry> {
    // The level this reaction populates is the one whose own MF=12 section is
    // this MT.
    let Some(start) = levels.iter().find(|l| l.mt == mt) else {
        return Vec::new();
    };

    // (level energy, population). Walked highest-first so a level is decayed
    // only once everything above it has fed into it.
    let mut pop: Vec<(f64, f64)> = vec![(start.es_ev, 1.0)];
    let mut photons: Vec<(f64, f64)> = Vec::new();
    loop {
        let Some(i) = pop
            .iter()
            .enumerate()
            .filter(|(_, &(_, p))| p > 0.0)
            .max_by(|a, b| a.1 .0.partial_cmp(&b.1 .0).unwrap())
            .map(|(i, _)| i)
        else {
            break;
        };
        let (es_i, p_i) = pop[i];
        pop[i].1 = 0.0;
        let Some(lv) = levels
            .iter()
            .find(|l| (l.es_ev - es_i).abs() < 1.0e-6 * es_i.max(1.0))
        else {
            continue; // ground state, or a level with no decay data
        };
        for &(e_j, tp, gp) in &lv.trans {
            let yield_ = p_i * tp * if lv.lg == 2 { gp } else { 1.0 };
            if yield_ > 0.0 {
                photons.push((es_i - e_j, yield_));
            }
            let flow = p_i * tp;
            if flow > 0.0 && e_j > 0.0 {
                match pop
                    .iter_mut()
                    .find(|(e, _)| (*e - e_j).abs() < 1.0e-6 * e_j.max(1.0))
                {
                    Some(slot) => slot.1 += flow,
                    None => pop.push((e_j, flow)),
                }
            }
        }
    }

    // The reaction threshold in the laboratory: ES·(A+1)/A.
    let e_lo = start.es_ev * (awr + 1.0) / awr;
    photons
        .into_iter()
        .enumerate()
        .map(|(k, (eg_ev, y))| PhotonEntry {
            mtrp: mt * 1000 + k as i32 + 1,
            sigp: SigP::Yield {
                mftype: 12,
                mtmult: mt,
                e_mev: vec![e_lo / EMEV, e_max_ev / EMEV],
                y: vec![y, y],
            },
            law: EnergyLaw::Law2 { lp: 0, eg_mev: eg_ev / EMEV },
            e_lo_mev: e_lo / EMEV,
            e_hi_mev: e_max_ev / EMEV,
        })
        .collect()
}
