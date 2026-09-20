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

/// How a photon-production entry states its production rate.
#[derive(Debug, Clone)]
pub enum SigP {
    /// `MFTYPE = 12` — a yield to be multiplied by reaction `mtmult`'s cross
    /// section.
    Yield {
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
    let mut out = Vec::new();
    for (file, is_yield) in [(12i32, true), (13i32, false)] {
    for mt in 1..1000 {
        let Some(sec) = tape.section(mat, file, mt) else { continue };
        let mut cur = SectionCursor::new(&sec.rows);
        let Ok(head) = cur.read_cont() else { continue };
        if is_yield && head.l1 != 1 {
            // LO=2 transition-probability cascade — not ported.
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
    Some(out)
}
