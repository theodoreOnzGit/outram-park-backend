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
    build_impl(tape, mat, None)
}

/// [`build`] with the PENDF the ACE table is made from, which is what
/// upstream converts MF=12 `LO = 2` sections against (`convr`,
/// `acefc.f90:4173-4350`).
///
/// Each level energy comes from the PENDF's MF=3 threshold
/// (`ee = eeth*awr/(awr+1)`). Each photon becomes a yield TAB1 from that
/// threshold to `elim`, written through a formatted scratch tape, so every
/// value is 7-figure ENDF text. `acelpp` then rounds energies to
/// `sigfig(E/1e6, 7)`.
///
/// ~~[`build`]'s own cascade expansion (`expand_cascade`)~~ is replaced here
/// (2026-09-26, GitHub #340). It left photon energies and yields unrounded:
/// U-238's first LO=2 photon read 4.51063173566e-2 MeV where NJOY writes
/// 4.510633e-2. It also took level energies from the tape rather than from
/// the PENDF, and BROADR moves a broadened level's threshold: NJOY's U-235
/// yields differ between its 0 K and 293.6 K tables for that reason.
pub fn build_with_pendf(tape: &Tape, mat: i32, pendf: &crate::reconr::ReconrResult) -> Option<Vec<PhotonEntry>> {
    build_impl(tape, mat, Some(pendf))
}

fn build_impl(tape: &Tape, mat: i32, pendf: Option<&crate::reconr::ReconrResult>) -> Option<Vec<PhotonEntry>> {
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

    let mut cascade = Lo2Cascade::new();
    let rt = |x: f64| {
        use crate::endf::parse::{format_endf_float, parse_endf_float};
        parse_endf_float(&format_endf_float(x)).unwrap_or(x)
    };
    let eeth = |mt: i32| -> Option<f64> {
        let p = pendf?;
        p.sections.iter().find(|s| s.mt.number() == mt).map(|s| {
            let np = s.pairs.len() as i32;
            let t = crate::endf::records::Tab1 {
                head: crate::endf::records::Cont { c1: 0.0, c2: 0.0, l1: 0, l2: 0, n1: 1, n2: np },
                interp: vec![(np as u32, 2)],
                pairs: s.pairs.clone(),
            };
            crate::endf::gety1::Gety1::new(&t).get(0.0).xnext
        })
    };
    let elim = {
        let emax = tape
            .section(mat, 1, 451)
            .and_then(|s| s.rows.get(2).map(|r| r[1]))
            .unwrap_or(0.0);
        if emax > 2.0e7 { emax } else { 2.0e7 }
    };

    let mut out = Vec::new();
    for (file, is_yield) in [(12i32, true), (13i32, false)] {
        for mt in 1..1000 {
            if file == 12 && mt == MT_DELAYED_PHOTON {
                continue; // `acefc.f90:3642` — MF=12/MT=460 never reaches MTRP
            }
            let Some(sec) = tape.section(mat, file, mt) else {
                continue;
            };
            let mut cur = SectionCursor::new(&sec.rows);
            let Ok(head) = cur.read_cont() else { continue };
            if is_yield && head.l1 == 2 && pendf.is_some() {
                use crate::mixr::mix::sigfig;
                let Ok(list) = cur.read_list() else { return None };
                let res = cascade.section(mt, head.c2, head.l2, &list, &eeth)?;
                let Some(res) = res else { continue };
                let elow = rt(eeth(mt).unwrap_or(0.0));
                let (lo, hi) = (sigfig(elow / EMEV, 7, 0), sigfig(rt(elim) / EMEV, 7, 0));
                for (k, &(eg, _es, yy)) in res.photons.iter().enumerate() {
                    let y = rt(yy);
                    out.push(PhotonEntry {
                        mtrp: mt * 1000 + k as i32 + 1,
                        sigp: SigP::Yield { mftype: 12, mtmult: mt, e_mev: vec![lo, hi], y: vec![y, y] },
                        law: EnergyLaw::Law2 { lp: 0, eg_mev: sigfig(rt(eg) / EMEV, 7, 0) },
                        e_lo_mev: lo,
                        e_hi_mev: hi,
                    });
                }
                continue;
            }
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
                let Ok(tab) = cur.read_tab1() else {
                    return None;
                };
                let eg_ev = tab.head.c1;
                let lp = tab.head.l1;
                let lf = tab.head.l2;
                if tab.pairs.is_empty() {
                    continue;
                }
                let e_first = tab.pairs.first().map(|&(e, _)| e).unwrap_or(0.0);
                let e_last = tab.pairs.last().map(|&(e, _)| e).unwrap_or(0.0);

                let law = if lf == 2 || lf == 0 {
                    // `xss(nex+10)=sigfig(eg/emev,7,0)` (`acefc.f90:8509`)
                    EnergyLaw::Law2 {
                        lp,
                        eg_mev: crate::mixr::mix::sigfig(eg_ev / EMEV, 7, 0),
                    }
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
                    let l4 = acelpp_law4(sec15, k).ok()?;
                    EnergyLaw::Law4(l4)
                };

                let sigp = if is_yield {
                    SigP::Yield {
                        mftype: 12,
                        mtmult: mt,
                        // `xss(nex+i)=sigfig(E/emev,7,0)` (`acefc.f90:8326`)
                        e_mev: tab
                            .pairs
                            .iter()
                            .map(|&(e, _)| crate::mixr::mix::sigfig(e / EMEV, 7, 0))
                            .collect(),
                        y: tab.pairs.iter().map(|&(_, y)| y).collect(),
                    }
                } else {
                    SigP::Xs {
                        interp: tab.interp.clone(),
                        pairs: tab.pairs.clone(),
                    }
                };

                out.push(PhotonEntry {
                    mtrp: mt * 1000 + k,
                    sigp,
                    law,
                    // `sigfig(ef/emev,7,0)`, `sigfig(el/emev,7,0)` (`:8504-8505`)
                    e_lo_mev: crate::mixr::mix::sigfig(e_first / EMEV, 7, 0),
                    e_hi_mev: crate::mixr::mix::sigfig(e_last / EMEV, 7, 0),
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
        let Some(sec) = tape.section(mat, 6, mt) else {
            continue;
        };
        // `convr` never moves MF=6/MT=18 photons when `jpp != 0`
        // (`acefc.f90:4393-4396`).
        if mt == 18 {
            let jp = sec.rows.first().map_or(0, |r| r[2] as i32);
            if jp - 10 * (jp % 10) != 0 {
                continue;
            }
        }
        let Ok(products) = super::energy::parse_mf6_law1_products(sec, 0) else {
            continue;
        };
        let lib = tape
            .section(mat, 1, 451)
            .map(|s| {
                let r = |i: usize, j: usize| s.rows.get(i).map_or(0, |r| r[j] as i32);
                (r(0, 4), r(2, 5), r(2, 2))
            })
            .unwrap_or((0, 0, 0));
        for (k, mut prod) in products.into_iter().enumerate() {
            if prod.yield_pairs.is_empty() {
                continue;
            }
            // `acelpp`'s own MF=16 rounding (see `acelpp_mf16_law4`).
            if let Some(l4) = acelpp_mf16_law4(sec, k, lib) {
                prod.law4 = l4;
            }
            let e_first = crate::mixr::mix::sigfig(prod.yield_pairs.first().map(|&(e, _)| e).unwrap_or(0.0), 7, 0);
            let e_last = crate::mixr::mix::sigfig(prod.yield_pairs.last().map(|&(e, _)| e).unwrap_or(0.0), 7, 0);
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
        let Some(sec) = tape.section(mat, 12, mt) else {
            continue;
        };
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
        out.push(Level {
            mt,
            es_ev,
            lg,
            trans,
        });
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
            law: EnergyLaw::Law2 {
                lp: 0,
                eg_mev: eg_ev / EMEV,
            },
            e_lo_mev: e_lo / EMEV,
            e_hi_mev: e_max_ev / EMEV,
        })
        .collect()
}

/// An MF=15 continuum photon spectrum as `acelpp` writes it into DLWP
/// (`acefc.f90:8514-8610`, the `lff.eq.1` branch).
///
/// Subsection `mtdnc` (the photon's own subsection number, or 1 when it
/// exceeds `NC`) is used. At each incident energy:
///
/// - `E_in = sigfig(E/1e6, 7)`;
/// - each `E_out = sigfig(E'/1e6, 7)`;
/// - `pdf = sigfig(g*1e6, 7)`, zeroed below `rmin = 1e-30`;
/// - the cdf is accumulated from the **raw** tape values, histogram or
///   trapezoid by the TAB1's last `INT`;
/// - both are renormalized to a unit cdf and rounded to 9 figures.
///
/// ~~`parse_mf5_law4`, the neutron MF=5 reader~~, **CHANGED 2026-09-26**
/// (GitHub #340). It left pdf and cdf unrounded: 16 241 of U-234's 25 126
/// DLWP words differed from NJOY's in the 10th figure.
fn acelpp_law4(sec15: &crate::endf::tape::Section, mtdnc: i32) -> Result<super::energy::Law4, crate::NjoyError> {
    use crate::mixr::mix::sigfig;
    const RMIN: f64 = 1.0e-30;
    let mut cur = SectionCursor::new(&sec15.rows);
    let head = cur.read_cont()?;
    let nc = head.n1;
    let want = if mtdnc > nc { 1 } else { mtdnc };
    for ic in 1..=nc {
        let _prob = cur.read_tab1()?;
        let tab2 = cur.read_tab2()?;
        let ne = tab2.head.n2;
        if ic != want {
            for _ in 0..ne {
                cur.read_tab1()?;
            }
            continue;
        }
        let mut incident = Vec::with_capacity(ne as usize);
        for _ in 0..ne {
            let g = cur.read_tab1()?;
            let jnt = g.interp.last().map_or(2, |&(_, i)| i);
            let n = g.pairs.len();
            let e_out: Vec<f64> = g.pairs.iter().map(|&(e, _)| sigfig(e / EMEV, 7, 0)).collect();
            let pdf0: Vec<f64> = g
                .pairs
                .iter()
                .map(|&(_, p)| {
                    let v = sigfig(p * EMEV, 7, 0);
                    if v < RMIN {
                        0.0
                    } else {
                        v
                    }
                })
                .collect();
            let mut cdf0 = vec![0.0f64; n];
            for k in 1..n {
                let (e0, p0) = g.pairs[k - 1];
                let (e1, p1) = g.pairs[k];
                cdf0[k] = match jnt {
                    1 => cdf0[k - 1] + p0 * (e1 - e0),
                    2 => cdf0[k - 1] + ((p0 + p1) / 2.0) * (e1 - e0),
                    _ => cdf0[k],
                };
            }
            let renorm = 1.0 / cdf0[n - 1];
            incident.push(super::energy::OutgoingEnergy {
                e_in_mev: sigfig(g.head.c2 / EMEV, 7, 0),
                intt: jnt,
                e_out_mev: e_out,
                pdf: pdf0.iter().map(|&v| sigfig(v * renorm, 9, 0)).collect(),
                cdf: cdf0.iter().map(|&v| sigfig(v * renorm, 9, 0)).collect(),
            });
        }
        // `if (m.ne.1.or.jnt.ne.2)`: a single lin-lin region is written as
        // NR = 0.
        let simple = tab2.interp.len() == 1 && tab2.interp[0].1 == 2;
        return Ok(super::energy::Law4 {
            e_in_interp: if simple { Vec::new() } else { tab2.interp.clone() },
            incident,
        });
    }
    Err(crate::NjoyError::EndfParse("MF=15 subsection not found".into()))
}

/// GPD, the total photon-production cross section on the ACE grid:
/// `gamsum` (`acefc.f90:3572-3866`) as `acelod` stores it
/// (`acefc.f90:6255-6271`).
///
/// At every grid energy `gamsum` accumulates:
///
/// 1. each MF=12 section in tape order (MT=460 skipped): its total yield
///    times the PENDF's MF=3 cross section of the same MT. For MT=3 that is
///    MT=1 minus MT=2, each term evaluated separately;
/// 2. each MF=13 section: its subsections, or its single TAB1 when `NK = 1`.
///
/// Each term starts at the first grid point at or above its own threshold,
/// `gety2`'s leading-zero scan of the MF=3 section or MF=13 subsection. The
/// first grid point is read at `E(1+eps)` and the last at `E(1-eps)`, and
/// the threshold point itself contributes zero. The sum is rounded once,
/// `sigfig(c, 7, 0)`, and `acelod` reads it back on the same grid.
///
/// MF=12 `LO = 2` sections are converted first, as `convr` does: a flat
/// total yield from the PENDF threshold to `elim` ([`Lo2Cascade`]). MF=6
/// photon products count as MF=16. Returns `None` for what is not ported,
/// rather than a wrong block: an `LO = 1` section with `NK > 1`, whose total
/// `convr` rebuilds from the subsections.
pub fn gpd(tape: &Tape, mat: i32, pendf: &crate::reconr::ReconrResult) -> Option<Vec<f64>> {
    use crate::endf::gety1::Gety1;
    use crate::endf::records::{Cont, Tab1};
    use crate::mixr::mix::sigfig;
    const EPS: f64 = 1.0e-10;

    let mt1 = pendf.sections.iter().find(|s| s.mt.number() == 1)?;
    let grid: Vec<f64> = mt1.pairs.iter().map(|p| p.0).collect();
    let nen = grid.len();
    let mut c = vec![0.0f64; nen];
    let as_tab = |pairs: &[(f64, f64)]| Tab1 {
        head: Cont { c1: 0.0, c2: 0.0, l1: 0, l2: 0, n1: 1, n2: pairs.len() as i32 },
        interp: vec![(pairs.len() as u32, 2)],
        pairs: pairs.to_vec(),
    };
    // One pass of `gamsum`'s inner loop over the grid: `yv` is the
    // thresholded table (MF=3 section or MF=13 subsection), `xv` the yield
    // multiplying it (MF=12), `sign` +1 or -1.
    let mut pass = |xv: Option<&Tab1>, yv: &Tab1, sign: f64| {
        let mut gy = Gety1::new(yv);
        let thresh = gy.get(0.0).xnext;
        let mut gx = xv.map(Gety1::new);
        if let Some(g) = gx.as_mut() {
            let _ = g.get(0.0);
        }
        for i in 0..nen {
            if grid[i] < (1.0 - EPS) * thresh {
                continue;
            }
            let mut e = grid[i];
            if i == 0 {
                e *= 1.0 + EPS;
            }
            if i == nen - 1 {
                e *= 1.0 - EPS;
            }
            let x = gx.as_mut().map_or(1.0, |g| g.get(e).y);
            let mut y = gy.get(e).y;
            if i > 0 && e < thresh * (1.0 + EPS) {
                y = 0.0;
            }
            c[i] += sign * x * y;
        }
    };

    let mut any = false;
    // MF=12 (after `convr`), then MF=16, then MF=13 -- `gamsum`'s file order.
    let elim = {
        // `elim = ehi`, or EMAX when larger (`acefc.f90:347-348`).
        let emax = tape
            .section(mat, 1, 451)
            .and_then(|s| s.rows.get(2).map(|r| r[1]))
            .unwrap_or(0.0);
        if emax > 2.0e7 { emax } else { 2.0e7 }
    };
    let eeth = |mt: i32| -> Option<f64> {
        pendf.sections.iter().find(|s| s.mt.number() == mt).map(|s| {
            let mut g = Gety1::new(&as_tab(&s.pairs));
            g.get(0.0).xnext
        })
    };
    let mut cascade = Lo2Cascade::new();
    for sec in tape.sections().iter().filter(|s| s.key.mat == mat && s.key.mf == 12) {
        if sec.key.mt == 460 {
            continue;
        }
        let mut cur = SectionCursor::new(&sec.rows);
        let head = cur.read_cont().ok()?;
        let x = if head.l1 == 2 {
            // `convr`'s LO=2 conversion: a flat total yield from the PENDF
            // threshold to `elim` (`acefc.f90:4173-4350`).
            let list = cur.read_list().ok()?;
            let out = cascade.section(sec.key.mt, head.c2, head.l2, &list, &eeth)?;
            let Some(out) = out else { continue };
            let ysum = out.ysum;
            let elow = eeth(sec.key.mt).unwrap_or(0.0);
            Tab1 {
                head: Cont { c1: 0.0, c2: 0.0, l1: 0, l2: 0, n1: 1, n2: 2 },
                interp: vec![(2, 2)],
                pairs: vec![(elow, ysum), (elim, ysum)],
            }
        } else {
            if head.n1 > 1 {
                // `convr` rebuilds a multi-subsection LO=1 total as the sum of
                // its subsections (`:4078-4105`); not ported here yet.
                return None;
            }
            cur.read_tab1().ok()?
        };
        let mts: &[(i32, f64)] = if sec.key.mt == 3 { &[(1, 1.0), (2, -1.0)] } else { &[(0, 1.0)] };
        for &(mtd, sign) in mts {
            let mtd = if mtd == 0 { sec.key.mt } else { mtd };
            let Some(s3) = pendf.sections.iter().find(|s| s.mt.number() == mtd) else {
                continue;
            };
            pass(Some(&x), &as_tab(&s3.pairs), sign);
            any = true;
        }
    }
    // MF=16: MF=6 photon products (`ZAP = 0`, `LAW != 0`), each yield TAB1
    // as the tape has it (`convr`, `:4400-4430`), times the MT's MF=3.
    for sec in tape.sections().iter().filter(|s| s.key.mat == mat && s.key.mf == 6) {
        // `convr` skips MF=6/MT=18 when its `JP` flag's `jpp = jp - 10*jpn`
        // is non-zero (`acefc.f90:4393-4396`); U-238 (JP = 11) is that case.
        // Including those photon products put U-238's GPD one unit high in
        // the 7th figure at 88 % of its points.
        if sec.key.mt == 18 {
            let jp = sec.rows.first().map_or(0, |r| r[2] as i32);
            let jpn = jp % 10;
            if jp - 10 * jpn != 0 {
                continue;
            }
        }
        for x in mf6_photon_yields(sec) {
            let Some(s3) = pendf.sections.iter().find(|s| s.mt.number() == sec.key.mt) else {
                continue;
            };
            pass(Some(&x), &as_tab(&s3.pairs), 1.0);
            any = true;
        }
    }
    for sec in tape.sections().iter().filter(|s| s.key.mat == mat && s.key.mf == 13) {
        let mut cur = SectionCursor::new(&sec.rows);
        let head = cur.read_cont().ok()?;
        let nk = head.n1;
        let ntab = if nk > 1 { nk + 1 } else { 1 };
        for ik in 0..ntab {
            let t = cur.read_tab1().ok()?;
            if nk == 1 || ik != 0 {
                pass(None, &t, 1.0);
                any = true;
            }
        }
    }
    if !any {
        return None;
    }
    Some(c.into_iter().map(|v| sigfig(v, 7, 0)).collect())
}

/// The yield TAB1 of every MF=6 photon product (`ZAP = 0`, `LAW != 0`), in
/// subsection order: what `convr` moves to MF=16 (`acefc.f90:4400-4430`).
fn mf6_photon_yields(sec: &crate::endf::tape::Section) -> Vec<crate::endf::records::Tab1> {
    let mut out = Vec::new();
    let mut cur = SectionCursor::new(&sec.rows);
    let Ok(head) = cur.read_cont() else { return out };
    for _ in 0..head.n1 {
        let Ok(y) = cur.read_tab1() else { return out };
        let (zap, law) = (y.head.c1, y.head.l2);
        if zap == 0.0 && law != 0 {
            out.push(y.clone());
        }
        // Skip the law's body to reach the next subsection.
        let ok = match law {
            1 | 2 | 5 => (|| -> Result<(), crate::NjoyError> {
                let t2 = cur.read_tab2()?;
                for _ in 0..t2.head.n2 {
                    cur.read_list()?;
                }
                Ok(())
            })(),
            6 => cur.read_cont().map(|_| ()),
            7 => (|| -> Result<(), crate::NjoyError> {
                let t2 = cur.read_tab2()?;
                for _ in 0..t2.head.n2 {
                    let t2b = cur.read_tab2()?;
                    for _ in 0..t2b.head.n2 {
                        cur.read_tab1()?;
                    }
                }
                Ok(())
            })(),
            _ => Ok(()),
        };
        if ok.is_err() {
            return out;
        }
    }
    out
}

/// `convr`'s LO=2 state (`acefc.f90:4173-4300`): the level energies `ee`
/// and the transition matrices `aa`/`rr`. The matrices persist across
/// sections, and across level series, exactly as upstream's do.
struct Lo2Cascade {
    ee: Vec<f64>,
    aa: Vec<f64>,
    rr: Vec<f64>,
    mt0: i32,
    mt0old: i32,
}

const LO2_IMAX: usize = 50;

/// One LO=2 section after `convr`: the total yield and each photon's
/// `(E_gamma, E_level, yield)`, in descending `E_gamma`.
struct Lo2Out {
    ysum: f64,
    photons: Vec<(f64, f64, f64)>,
}

impl Lo2Cascade {
    fn new() -> Self {
        let n = LO2_IMAX;
        let mut rr = vec![0.0; n * n];
        for i in 0..n {
            rr[i * n + i] = 1.0;
        }
        Lo2Cascade { ee: vec![0.0; n + 1], aa: vec![0.0; n * n], rr, mt0: 0, mt0old: 0 }
    }
    /// `aa((k-1)*imax+j)` for 1-based `k`, `j`.
    fn ix(k: usize, j: usize) -> usize {
        (k - 1) * LO2_IMAX + (j - 1)
    }

    /// Process one LO=2 section (`mt`, AWR `awr`, `LG`, its LIST); returns
    /// `Some(Some(ysum))`, or `Some(None)` when it produces no photon.
    fn section(
        &mut self,
        mt: i32,
        awr: f64,
        lg: i32,
        list: &crate::endf::records::List,
        eeth: &impl Fn(i32) -> Option<f64>,
    ) -> Option<Option<Lo2Out>> {
        let mt0 = match mt {
            51..=90 => 49,
            601..=649 => 599,
            651..=699 => 649,
            701..=749 => 699,
            751..=799 => 749,
            801..=849 => 799,
            876..=891 => 874,
            _ => self.mt0,
        };
        self.mt0 = mt0;
        if mt0 != self.mt0old {
            self.ee.iter_mut().for_each(|v| *v = 0.0);
            self.mt0old = mt0;
            let m1 = mt0 + 2;
            let m2 = if mt0 == 49 {
                91
            } else if mt0 != 874 {
                m1 + 48
            } else {
                m1 + 15
            };
            for m in m1..=m2 {
                if let Some(e) = eeth(m) {
                    let k = (m - mt0) as usize;
                    if k < self.ee.len() {
                        self.ee[k] = e * awr / (awr + 1.0);
                    }
                }
            }
        }
        let j = (mt - mt0) as usize;
        if j == 0 || j >= LO2_IMAX {
            return None;
        }
        // LIST: C1 = ES, N2 = NT; body (LG+1) words per transition.
        self.ee[j] = list.head.c1;
        let n = list.head.n2 as usize;
        let w = (lg + 1) as usize;
        let d = &list.data;
        let jm1 = j - 1;
        for kk in 1..=jm1 {
            let k = jm1 - kk + 1;
            let mut found = None;
            for i in 1..=n {
                let ei = d[(i - 1) * w];
                if (ei == 0.0 && self.ee[k] == 0.0) || (ei != 0.0 && ((ei - self.ee[k]) / ei).abs() < 0.0001) {
                    found = Some(i);
                    break;
                }
            }
            match found {
                None => {
                    self.aa[Self::ix(k, j)] = 0.0;
                    self.rr[Self::ix(k, j)] = 0.0;
                }
                Some(i) => {
                    let p = d[(i - 1) * w + 1];
                    let g = if lg == 2 { d[(i - 1) * w + 2] } else { 1.0 };
                    self.aa[Self::ix(k, j)] = p * g;
                    self.rr[Self::ix(k, j)] = p;
                }
            }
            if k != jm1 {
                for ii in (k + 1)..=jm1 {
                    self.rr[Self::ix(k, j)] += self.rr[Self::ix(ii, j)] * self.aa[Self::ix(k, ii)];
                }
            }
        }
        let mut ysum = 0.0f64;
        let mut ph: Vec<(f64, f64, f64)> = Vec::new(); // (eg, es, yy)
        for i in 2..=j {
            let ja = j + 2 - i;
            let eja = self.ee[ja];
            for ii in 1..=jm1 {
                let jb = j - ii;
                let y = self.aa[Self::ix(jb, ja)] * self.rr[Self::ix(ja, j)];
                if y != 0.0 {
                    ph.push((eja - self.ee[jb], eja, y));
                    ysum += y;
                }
            }
        }
        self.ee[1] = 0.0;
        // "arrange gamma energies in descending order" (`:4298-4316`),
        // upstream's own exchange sort.
        let l = ph.len();
        for i in 0..l.saturating_sub(1) {
            for ii in (i + 1)..l {
                if ph[i].0 < ph[ii].0 {
                    ph.swap(i, ii);
                }
            }
        }
        Some(if l == 0 { None } else { Some(Lo2Out { ysum, photons: ph }) })
    }
}

/// The law-4 DLWP data `acelpp` writes for an MF=16 photon (an MF=6
/// `ZAP = 0`, `LAW = 1` product moved by `convr`): `acefc.f90:8631-8960`.
///
/// Two passes over the incident energies, as upstream:
///
/// 1. Build the union list `dise` of discrete photon energies, highest
///    first. A repeated energy is nudged by `1+2*eps`, primary photons are
///    kept in JENDL form, and `LAW = 2` photons are shifted by
///    `-awr*E/(awr+1)`.
/// 2. At each incident energy, insert the missing discrete lines with zero
///    probability, convert primaries to photon energies, and write
///    `[LEP+10*ND, N, E'(N), pdf(N), cdf(N)]`:
///    - `E' = sigfig(E'/1e6, 7)`;
///    - discrete probabilities as they are, continuum densities times 1e6
///      (zero below `rmin`);
///    - the cdf accumulated from the raw values;
///    - pdf and cdf renormalized and rounded to **9** figures. Unlike MF=15,
///      the pdf is **not** pre-rounded to 7.
///
/// `photon` selects the photon (`ZAP = 0`) subsection by its order among
/// them. `lib = (NLIB, NVER, LREL)` from MF=1 selects the ENDF-8.1
/// primary-photon convention.
fn acelpp_mf16_law4(
    sec: &crate::endf::tape::Section,
    photon: usize,
    lib: (i32, i32, i32),
) -> Option<super::energy::Law4> {
    use crate::mixr::mix::sigfig;
    const EPS: f64 = 4.0e-6;
    const RMIN: f64 = 1.0e-30;
    let endf81 = lib == (0, 8, 1);
    let mut cur = SectionCursor::new(&sec.rows);
    let head = cur.read_cont().ok()?;
    let awr = head.c2;
    let mut seen = 0usize;
    for _ in 0..head.n1 {
        let y = cur.read_tab1().ok()?;
        let (zap, law) = (y.head.c1, y.head.l2);
        if !(zap == 0.0 && law != 0) {
            super::energy::mf6::skip_mf6_subsection(&mut cur, law).ok()?;
            continue;
        }
        if seen != photon || law != 1 {
            seen += 1;
            super::energy::mf6::skip_mf6_subsection(&mut cur, law).ok()?;
            continue;
        }
        let t2 = cur.read_tab2().ok()?;
        let lep = t2.head.l2;
        let ne = t2.head.n2.max(0) as usize;
        let mut lists = Vec::with_capacity(ne);
        for _ in 0..ne {
            lists.push(cur.read_list().ok()?);
        }
        // pass 1: the union list of discrete photons
        let mut dise: Vec<f64> = Vec::new();
        for l in &lists {
            let ei = l.head.c2;
            let nd = l.head.l1.max(0) as usize;
            if nd == 0 {
                continue;
            }
            let d = &l.data;
            if dise.is_empty() {
                for ki in 0..nd {
                    let mut ep = d[2 * ki];
                    if ep == 0.0 {
                        ep = 1.0e-5;
                    }
                    if law == 1 && ep < 0.0 && endf81 {
                        ep += ei * awr / (awr + 1.0);
                    }
                    if law == 2 {
                        ep -= awr * ei / (awr + 1.0);
                    }
                    if ki > 0 && dise[ki - 1] == ep {
                        ep *= 1.0 + 2.0 * EPS;
                    }
                    dise.push(ep);
                }
            } else {
                for ki in 0..nd {
                    let mut ep = d[2 * ki];
                    if ep == 0.0 {
                        ep = 1.0e-5;
                    }
                    if law == 1 && ep < 0.0 && endf81 {
                        ep += ei * awr / (awr + 1.0);
                    }
                    if law == 2 {
                        ep -= awr * ei / (awr + 1.0);
                    }
                    if ki > 0 && ep == d[2 * (ki - 1)] {
                        ep *= 1.0 + 2.0 * EPS;
                    }
                    if dise.iter().any(|&x| (ep / x - 1.0).abs() <= EPS) {
                        continue;
                    }
                    let n0 = dise.len();
                    if ep.abs() > dise[0].abs() {
                        dise.insert(0, ep);
                    } else if ep.abs() < dise[n0 - 1].abs() {
                        dise.push(ep);
                    } else {
                        let pos = (0..n0 - 1).find(|&m| ep.abs() < dise[m].abs() && ep.abs() > dise[m + 1].abs())?;
                        dise.insert(pos + 1, ep);
                    }
                }
            }
        }
        let nd0 = dise.len();
        // pass 2
        let mut incident = Vec::with_capacity(ne);
        for l in &lists {
            let ei = l.head.c2;
            let mut nd = l.head.l1.max(0) as usize;
            let na = l.head.l2.max(0) as usize;
            let ncyc = na + 2;
            let mut n = l.head.n2.max(0) as usize;
            // scr(7..) as a 0-based vector with room for inserted lines
            let mut s: Vec<f64> = l.data.clone();
            s.resize(s.len().max(ncyc * (n + nd0) + 2), 0.0);
            let mut tdise: Vec<(f64, f64)> = Vec::new();
            for nn in 0..nd {
                if s[2 * nn] == 0.0 {
                    s[2 * nn] = 1.0e-5;
                }
                let mut e = s[2 * nn];
                if nn > 0 && e == tdise[nn - 1].0 {
                    e *= 1.0 + 2.0 * EPS;
                }
                tdise.push((e, s[2 * nn + 1]));
            }
            if nd0 != 0 && nd == nd0 {
                for nn in 0..nd {
                    if law == 1 && s[2 * nn] < 0.0 {
                        s[2 * nn] = if endf81 { -s[2 * nn] } else { -s[2 * nn] + ei * awr / (awr + 1.0) };
                    }
                }
            } else if nd0 != 0 {
                if nd == 0 {
                    // move continuous data, then insert discrete data
                    for ki in (1..=2 * n).rev() {
                        s[2 * nd0 + ki - 1] = s[ki - 1];
                    }
                    for (ki, &d0) in dise.iter().enumerate() {
                        let mut epu = d0;
                        if law == 1 && epu < 0.0 {
                            epu = -epu + ei * awr / (awr + 1.0);
                        }
                        if law == 2 {
                            epu += ei * awr / (awr + 1.0);
                        }
                        s[2 * ki] = epu;
                        s[2 * ki + 1] = 0.0;
                    }
                    n += nd0;
                } else {
                    for ki in ((2 * nd + 1)..=(2 * n)).rev() {
                        s[2 * nd0 + ki - 2 * nd - 1] = s[ki - 1];
                    }
                    for m in (1..=nd0).rev() {
                        let mut ep = dise[m - 1];
                        if law == 1 && ep < 0.0 {
                            ep = if endf81 { -ep } else { -ep + ei * awr / (awr + 1.0) };
                        }
                        if law == 2 {
                            ep += ei * awr / (awr + 1.0);
                        }
                        s[2 * (m - 1)] = ep;
                        s[2 * (m - 1) + 1] = 0.0;
                        let mut nn = nd + 1;
                        while nn > 1 {
                            nn -= 1;
                            let r = (tdise[nn - 1].0.abs() / dise[m - 1].abs()) - 1.0;
                            if r.abs() < 0.001 * EPS {
                                s[2 * (m - 1) + 1] = tdise[nn - 1].1;
                                break;
                            } else if r > 0.0 {
                                break;
                            }
                        }
                    }
                    n = n - nd + nd0;
                }
                nd = nd0;
            }
            let e = |ki: usize| s[ncyc * ki];
            let p = |ki: usize| s[ncyc * ki + 1];
            let mut eo = vec![0.0; n];
            let mut pd = vec![0.0; n];
            let mut cd = vec![0.0; n];
            for ki in 0..n {
                eo[ki] = sigfig(e(ki) / EMEV, 7, 0);
                pd[ki] = if ki < nd { p(ki) } else { p(ki) * EMEV };
                if pd[ki] < RMIN {
                    pd[ki] = 0.0;
                }
                if nd > 0 && ki == 0 {
                    cd[ki] = pd[ki];
                } else if ki < nd {
                    cd[ki] = cd[ki - 1] + p(ki);
                }
                if nd > 0 && ki == nd {
                    cd[ki] = cd[ki - 1];
                }
                if ki > nd && lep == 1 {
                    cd[ki] = cd[ki - 1] + p(ki - 1) * (e(ki) - e(ki - 1));
                }
                if ki > nd && lep == 2 {
                    cd[ki] = cd[ki - 1] + ((p(ki - 1) + p(ki)) / 2.0) * (e(ki) - e(ki - 1));
                }
            }
            let renorm = if n > 0 && cd[n - 1] != 0.0 { 1.0 / cd[n - 1] } else { 1.0 };
            incident.push(super::energy::OutgoingEnergy {
                e_in_mev: sigfig(ei / EMEV, 7, 0),
                intt: (lep + 10 * nd as i32) as u32,
                e_out_mev: eo,
                pdf: pd.iter().map(|&v| sigfig(v * renorm, 9, 0)).collect(),
                cdf: cd.iter().map(|&v| sigfig(v * renorm, 9, 0)).collect(),
            });
        }
        let simple = t2.interp.len() == 1 && t2.interp[0].1 == 2;
        return Some(super::energy::Law4 {
            e_in_interp: if simple { Vec::new() } else { t2.interp.clone() },
            incident,
        });
    }
    None
}
