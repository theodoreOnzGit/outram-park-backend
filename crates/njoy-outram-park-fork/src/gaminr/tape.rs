// Translated from NJOY2016.80 (commit ac5adf5) `src/gaminr.f90`, `subroutine gaminr`
// (`:133-536`) and `ruing` (`:538-583`).
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under AGPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! **GAMINR's tape control flow** (gh:#1369): the ENDF-format probe, the
//! GAM-out MF=1/MT=451 head of each material, the loop over the requested
//! (or standard) reactions, the `akn` cache and the total-heating edit, written
//! as the GENDF text NJOY writes ([`crate::deck::groupr::GendfText`]).
//!
//! Upstream quirks kept:
//! - the head's title word is `rt(1)` after `tt(1) = title(1)`: `tt` is
//!   `character(4)` equivalenced to the 8-byte `rt`, so the word is four
//!   blanks followed by columns 5-8 of the PENDF's first description card
//!   (`:203-206`, `:228-233`; `ntw = 1`, `title(1) = ''`, `:574-577`). Deck 03
//!   prints it as `1.7514+190`, deck 30 as `6.0135-154`.
//! - `iverf` comes from `nendf` and decides how many records of the **PENDF**'s
//!   MF=1 are read before `hdatio` (`:152-164`, `:196-198`).
//! - `gpanel`'s `save`d state lives for the whole run ([`PanelState`]).
//!
//! Refused by name: an input GAM-out tape (`ngam1 /= 0`, the merge of
//! `:166-186`), a read-in weight function (`iwt = 1`), and `ngam2 = 0`.

use crate::deck::groupr::GendfText;
use crate::endf::records::SectionCursor;
use crate::endf::tape::Tape;
use crate::endf::EndfKey;
use crate::gaminr::gtff::PhotonTab1;
use crate::gaminr::matrix::{reaction_groups, AknCache, PanelState, PhotonFlux, PhotonReaction, ReactionRun, TotalHeating};
use crate::groupr::gendf::GendfSection;
use crate::NjoyError;

/// GAMINR's cards (`ruing`, card 6 and card 7).
#[derive(Debug, Clone, PartialEq)]
pub struct GaminrCards {
    /// `nendf npend ngam1 ngam2`.
    pub units: [i32; 4],
    /// `igg`.
    pub igg: i32,
    /// `iwt`.
    pub iwt: i32,
    /// `lord`.
    pub lord: i32,
    /// `iprint` (listing only).
    pub iprint: i32,
    /// Card 3, the run title (listing only; the tape gets the blank word).
    pub title: String,
    /// Card 4 (`igg = 1`): the group bounds \[eV\].
    pub egg_read: Option<Vec<f64>>,
    /// Materials in deck order; `None` reactions is `mfd = -1` (all).
    pub materials: Vec<(i32, Option<Vec<(i32, i32)>>)>,
}

/// The standard reaction list (`mflst`, `mtlst`, `mtlst6`, `:111-116`).
fn standard(iverf: i32) -> Vec<(i32, i32)> {
    const MF: [i32; 9] = [23, 23, 26, 23, 26, 23, 26, 23, 23];
    const MT: [i32; 9] = [501, 502, 502, 504, 504, 516, 516, 602, 621];
    const MT6: [i32; 9] = [501, 502, 502, 504, 504, 516, 516, 522, 525];
    (0..9).map(|i| (MF[i], if iverf >= 6 { MT6[i] } else { MT[i] })).collect()
}

fn tab1_of(tape: &Tape, mat: i32, mf: i32, mt: i32) -> Result<crate::endf::records::Tab1, NjoyError> {
    let s = tape.section(mat, mf, mt).ok_or(NjoyError::SectionNotFound { mat, mf, mt })?;
    let mut cur = SectionCursor::new(&s.rows);
    cur.read_cont()?;
    cur.read_tab1()
}

/// `rt(1)` after `tt(1) = title(1)` (see the module doc).
fn title_word(pendf: &Tape, mat: i32, iverf: i32) -> Result<f64, NjoyError> {
    let s = pendf.section(mat, 1, 451).ok_or(NjoyError::SectionNotFound { mat, mf: 1, mt: 451 })?;
    let nc = 1 + usize::from(iverf >= 5) + usize::from(iverf >= 6);
    let ncards = s.rows.get(nc).map_or(0.0, |r| r[4]);
    if ncards < 1.0 {
        return Err(NjoyError::NotPorted("gaminr: a PENDF with no description card (rt(1) is then a stale scratch word)"));
    }
    let line = pendf
        .raw_text(EndfKey { mat, mf: 1, mt: 451 })
        .and_then(|r| r.get(nc + 1))
        .cloned()
        .unwrap_or_default();
    let mut c: Vec<u8> = line.bytes().collect();
    c.resize(66, b' ');
    Ok(f64::from_le_bytes([b' ', b' ', b' ', b' ', c[4], c[5], c[6], c[7]]))
}

fn write_section(g: &mut GendfText, mat: i32, s: &GendfSection) {
    g.begin(mat, s.mf, s.mt);
    g.cont([s.za, s.zam], [s.nl, s.nz, 0, s.num_groups]);
    for r in &s.records {
        g.list([0.0, 0.0], r.ng2, r.ig2lo, r.ig, &r.data);
    }
    g.send();
}

/// Run GAMINR (`gaminr.f90:133-536`) on ENDF tape `nendf` and PENDF `pend`:
/// the GAM-out tape as text.
///
/// # Errors
/// A section `findf` would not find, a malformed record, or a refusal named
/// in the module doc.
pub fn run_gaminr_tape(c: &GaminrCards, nendf: &Tape, pend: &Tape) -> Result<GendfText, NjoyError> {
    let [_, _, ngam1, ngam2] = c.units;
    if ngam1 != 0 {
        return Err(NjoyError::NotPorted("GAMINR with an input gamout tape (ngam1 /= 0)"));
    }
    if ngam2 == 0 {
        return Err(NjoyError::NotPorted("GAMINR with no output gamout tape (ngam2 = 0)"));
    }
    let flux = match c.iwt {
        2 => PhotonFlux::Constant,
        3 => PhotonFlux::one_over_e_rolloffs(),
        1 => return Err(NjoyError::NotPorted("GAMINR iwt = 1 (read-in weight function)")),
        _ => return Err(NjoyError::InvalidInput("gnwtf: illegal iwt".into())),
    };
    let egg: Vec<f64> = match &c.egg_read {
        Some(e) => e.clone(),
        None => crate::gaminr::photon_groups::PhotonGroupStructure::from_igg(c.igg)?.boundaries()?,
    };
    let ngg = egg.len().saturating_sub(1);
    let lord = usize::try_from(c.lord).map_err(|_| NjoyError::InvalidInput("gaminr: negative lord".into()))?;
    // `iverf` from the first material of `nendf` (`:152-164`).
    let first = nendf.sections().first().ok_or_else(|| NjoyError::InvalidInput("gaminr: empty endf tape".into()))?;
    let m1 = nendf.section(first.key.mat, 1, 451).ok_or(NjoyError::SectionNotFound { mat: first.key.mat, mf: 1, mt: 451 })?;
    let iverf = crate::moder::layout::iverf_from_mf1(&m1.rows);

    let mut g = GendfText::default();
    // `tpidio(0,ngam2)` writes the PENDF's label record as read.
    g.lines.push(pend.tpid.clone());
    let mut st = PanelState::new(lord + 1);
    let mut akn = AknCache::default();
    let mut zref = 101.0f64;
    // `ig2pp`: the last bound below `epair` (`:222-226`).
    let mut ig2pp = 0usize;
    for (i, &e) in egg.iter().enumerate() {
        if e < crate::gaminr::gtff::EPAIR {
            ig2pp = i + 1;
        }
    }
    for (matb, reactions) in &c.materials {
        let matb = *matb;
        let m = pend.section(matb, 1, 451).ok_or(NjoyError::SectionNotFound { mat: matb, mf: 1, mt: 451 })?;
        let (za, awr) = (m.rows[0][0], m.rows[0][1]);
        let rt1 = title_word(pend, matb, iverf)?;
        // MF=1/MT=451 head (`:208-248`).
        g.begin(matb, 1, 451);
        g.cont([za, awr], [0, 1, -1, 1]);
        let mut data = vec![rt1, 1.0e12];
        data.extend_from_slice(&egg);
        data.push(0.0);
        g.list([0.0, 0.0], ngg as i32, 0, 0, &data);
        g.fend();
        let list = match reactions {
            Some(l) => l.clone(),
            None => standard(iverf),
        };
        let mut heating = TotalHeating::new(ngg);
        let run = ReactionRun { egg: egg.clone(), lord, za, awr, ig2pp };
        for (mfd, mtd) in list {
            if mtd == 621 || mtd == 525 {
                if mfd != 23 {
                    return Err(NjoyError::NotPorted("GAMINR total-heating edit with mfd other than 23"));
                }
                write_section(&mut g, matb, &heating.section(mtd, za, awr));
                continue;
            }
            let reaction = match (mfd, mtd) {
                (23, _) => PhotonReaction::Vector { mt: mtd },
                (26, 502) => {
                    let t = tab1_of(nendf, matb, 27, 502)?;
                    PhotonReaction::Coherent { form_factor: PhotonTab1 { z: t.head.c2, interp: t.interp, pairs: t.pairs } }
                }
                (26, 504) => {
                    let t = tab1_of(nendf, matb, 27, 504)?;
                    PhotonReaction::Incoherent { scattering_function: PhotonTab1 { z: t.head.c2, interp: t.interp, pairs: t.pairs } }
                }
                (26, 516) => PhotonReaction::Pair,
                _ => return Err(NjoyError::NotPorted("GAMINR mfd/mtd other than MF=23 and MF=26 MT=502/504/516")),
            };
            let mut znow = 0.0;
            if mfd == 26 && mtd == 504 {
                znow = (za / 1000.0).round();
                if znow <= zref {
                    zref = znow;
                    znow = -znow;
                }
            }
            let sigma = tab1_of(pend, matb, 23, mtd)?;
            let (sec, computed) = reaction_groups(&run, &reaction, &sigma, &flux, &mut st, &mut heating, &mut akn, znow)?;
            if computed {
                write_section(&mut g, matb, &sec);
            }
        }
        g.mend();
    }
    g.tend();
    Ok(g)
}
