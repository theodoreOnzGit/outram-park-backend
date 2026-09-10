// Ported from NJOY2016 `src/leapr.f90` (git commit ac5adf5f33d893e42f2eed7fb286b0d51c7580da):
//   - `subroutine leapr`, l.218-453 — the driver: card reading (via `LeaprDeck`), the
//     scatterer/temperature loops, the Bragg-edge call, and the `isym` bookkeeping.
//   - `subroutine endout`, l.3007-3009 (`sb`), 3043 (`iel = -1` for a solid with no
//     translational term), 3049-3156 (the MF=1/MT=451 header, Hollerith comment cards
//     and dictionary), 3160-3170 (`SB = sb*npr` of the incoherent-elastic TAB1).
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under GPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! The LEAPR `run` driver — a whole card deck to a whole ENDF tape, the
//! in-memory equivalent of `njoy < deck` with `leapr` on it.
//!
//! [`run_deck`] runs every temperature block of a [`LeaprDeck`] through the
//! Fortran temperature loop ([`build_law_at_temperature`]), decides the
//! elastic channel the way `subroutine leapr`/`endout` do (a built-in lattice
//! for `iel > 0`; incoherent elastic for `iel < 0` *and* for `iel = 0` when the
//! deck has no translational term, `leapr.f90:3043`; nothing otherwise),
//! builds the multi-temperature MF=7 through [`endout`], and records what the
//! MF=1/MT=451 header needs. [`LeaprRun::write_text`] then lays the tape out
//! as `endout` writes `nout`: the blank TPID record, MF=1/MT=451 with the
//! card-20 Hollerith comments and the dictionary (whose `NC` entries are the
//! Fortran's *estimates*, `leapr.f90:3128-3150`, not counted lines), then
//! MF=7.
//!
//! **Not reproduced:** the `nsyso` listing, and the Fortran unit numbers —
//! card 1's `nout` is recorded on the deck and otherwise ignored. The
//! cold-hydrogen (`ncold`) path is refused by [`LeaprDeck::unsupported_features`]
//! as before.
//!
//! Validated against three NJOY2016 runs (`tests/leapr_run_driver_njoy_oracle.rs`).

use std::fmt::Write as _;

use crate::endf::parse::format_endf_float;
use crate::endf::tape::Tape;
use crate::leapr::coher::coher_with_constants;
use crate::leapr::deck::LeaprDeck;
use crate::leapr::endout::{endout, ElasticOutput, LeaprOutput};
use crate::leapr::generate::{build_law_at_temperature, COHERENT_ELASTIC_EMAX_EV};
use crate::mixr::mix::sigfig;
use crate::NjoyError;

/// `therm` — the thermal energy `endout` scales `beta(nbeta)` by for the
/// MF=1 `EMAX` field (`leapr.f90:2997`, `:3075`).
const THERM_EV: f64 = 0.0253;

/// What the MF=1/MT=451 header of a LEAPR tape carries
/// (`endout`, `leapr.f90:3049-3156`).
#[derive(Debug, Clone, PartialEq)]
pub struct Mf1Header {
    /// `MAT`, `ZA`, `AWR`.
    pub mat: i32,
    pub za: f64,
    pub awr: f64,
    /// `EMAX = sigfig(0.0253 * beta(nbeta), 7)` \[eV\] — the third CONT's `C2`.
    pub emax: f64,
    /// The card-20 comment cards, one 66-column Hollerith line each.
    pub comments: Vec<String>,
    /// The effective `iel` after the `leapr.f90:3043` rule (`< 0` incoherent
    /// elastic, `> 0` coherent, `0` none): decides whether MF=7/MT=2 is in
    /// the dictionary and how its card count is estimated.
    pub iel: i32,
    /// `ntempr`, `nedge` (Bragg edges, `0` without coherent elastic),
    /// `nalpha`, `nbeta` — the dictionary's card-count inputs.
    pub ntempr: usize,
    pub nedge: usize,
    pub nalpha: usize,
    pub nbeta: usize,
}

impl Mf1Header {
    /// `NXC` — the number of dictionary entries (`leapr.f90:3092-3093`).
    pub fn nxc(&self) -> i32 {
        if self.iel != 0 {
            3
        } else {
            2
        }
    }

    /// The dictionary `(MF, MT, NC, MOD)` rows (`leapr.f90:3118-3150`).
    /// `NC` for MF=7 is the Fortran's card-count *estimate*, reproduced
    /// verbatim (it does not always equal the number of lines written).
    pub fn dictionary(&self) -> Vec<(i32, i32, i32, i32)> {
        let nc = self.comments.len() as i32;
        let mut rows = Vec::with_capacity(3);
        let mut nc451 = 5 + nc + 1;
        if self.iel != 0 {
            nc451 += 1;
        }
        rows.push((1, 451, nc451, 0));
        let ntempr = self.ntempr as i32;
        if self.iel != 0 {
            let nedge = self.nedge as i32;
            let mut ncards = if self.iel < 0 {
                3 + (2 * ntempr + 4) / 6
            } else {
                3 + (2 * nedge + 4) / 6
            };
            if self.iel > 0 && ntempr > 1 {
                ncards += (ntempr - 1) * (1 + (nedge + 5) / 6);
            }
            rows.push((7, 2, ncards, 0));
        }
        let nalpha = self.nalpha as i32;
        let nbeta = self.nbeta as i32;
        let mut ncards = 2 + (2 * nalpha + 4) / 6;
        if ntempr > 1 {
            ncards += (ntempr - 1) * (1 + (nalpha + 5) / 6);
        }
        ncards = 5 + nbeta * ncards;
        rows.push((7, 4, ncards, 0));
        rows
    }

    /// The MF=1/MT=451 records exactly as `endout` writes them — 80-column
    /// lines from the HEAD through the dictionary, then the SEND
    /// (`seq = 99999`) and the FEND. The TPID record is not included.
    pub fn lines(&self) -> Vec<String> {
        let mat = self.mat;
        let mut out = Vec::with_capacity(8 + self.comments.len());
        let mut seq = 0i32;
        let mut cont =
            |out: &mut Vec<String>, c1: f64, c2: f64, l1: i32, l2: i32, n1: i32, n2: i32| {
                seq += 1;
                out.push(format!(
                    "{}{}{l1:11}{l2:11}{n1:11}{n2:11}{mat:4} 1451{seq:5}",
                    format_endf_float(c1),
                    format_endf_float(c2)
                ));
            };
        // HEAD, then the two CONTs (leapr.f90:3060-3080)
        cont(&mut out, self.za, self.awr, -1, 0, 0, 0);
        cont(&mut out, 0.0, 0.0, 0, 0, 0, 6);
        cont(&mut out, 1.0, self.emax, 0, 0, 12, 6);
        // the Hollerith header (hdatio, l.3081-3093, 3111-3116): the Fortran
        // stages 17*nc words, and hdatio writes N1 as the number of cards.
        let nc = self.comments.len() as i32;
        cont(&mut out, 0.0, 0.0, 0, 0, nc, self.nxc());
        for c in &self.comments {
            seq += 1;
            let mut text: String = c.chars().take(66).collect();
            while text.chars().count() < 66 {
                text.push(' ');
            }
            out.push(format!("{text}{mat:4} 1451{seq:5}"));
        }
        // the dictionary (dictio: 22 blanks then 4i11)
        for (mf, mt, ncards, modn) in self.dictionary() {
            seq += 1;
            out.push(format!(
                "{:22}{mf:11}{mt:11}{ncards:11}{modn:11}{mat:4} 1451{seq:5}",
                ""
            ));
        }
        // asend / afend: blank data columns
        out.push(format!("{:66}{mat:4} 1  099999", ""));
        out.push(format!("{:66}{mat:4} 0  0    0", ""));
        out
    }
}

/// A completed LEAPR run: every temperature of the deck, the MF=7 tape and
/// the MF=1 header.
#[derive(Debug)]
pub struct LeaprRun {
    /// Card 2.
    pub title: String,
    /// What `endout` was given.
    pub output: LeaprOutput,
    /// The MF=7 sections (`endout`).
    pub tape: Tape,
    /// The MF=1/MT=451 header.
    pub header: Mf1Header,
}

impl LeaprRun {
    /// The TPID record `endout` writes: a blank text record with `MAT = 1`
    /// (`leapr.f90:3049-3057`).
    pub fn tpid_line() -> String {
        format!("{:66}   1 0  0    0", "")
    }

    /// The whole tape as text: TPID, MF=1/MT=451, then the MF=7 sections
    /// (with this crate's MF=7 line formatting; see [`Tape::write`] for how
    /// its CONT integer columns and sequence numbers differ cosmetically
    /// from NJOY's).
    ///
    /// # Errors
    /// Only the [`Tape::write`] I/O error, which a `String` sink never raises.
    pub fn write_text(&self) -> Result<String, NjoyError> {
        let mut s = String::new();
        s.push_str(&Self::tpid_line());
        s.push('\n');
        for l in self.header.lines() {
            s.push_str(&l);
            s.push('\n');
        }
        let mut mf7 = Vec::new();
        self.tape.write(&mut mf7)?;
        let mf7 = String::from_utf8(mf7)
            .map_err(|e| NjoyError::EndfParse(format!("leapr tape is not UTF-8: {e}")))?;
        // drop Tape::write's own TPID line
        for l in mf7.lines().skip(1) {
            let _ = writeln!(s, "{l}");
        }
        Ok(s)
    }
}

/// Run a whole LEAPR deck (`subroutine leapr`, `leapr.f90:218-453`).
///
/// Every temperature block is built in deck order (a second pass for the
/// secondary scatterer of a mixed moderator is folded in per block by
/// [`build_law_at_temperature`]), the elastic channel follows the deck's
/// `iel` as `endout` interprets it, and the result is one multi-temperature
/// MF=7 tape plus the MF=1 header.
///
/// The physical constants are the deck's ([`LeaprDeck::constants`]), so a
/// deck whose `EVAL` date predates NJOY's CODATA change regenerates with the
/// legacy `k_B` unless overridden with [`LeaprDeck::with_constants`].
///
/// # Errors
/// - [`NjoyError::NotPorted`] for a deck feature the port lacks
///   ([`LeaprDeck::unsupported_features`]) or a built-in lattice code
///   without a lattice.
/// - Whatever [`build_law_at_temperature`] reports for a block.
pub fn run_deck(deck: &LeaprDeck) -> Result<LeaprRun, NjoyError> {
    let unsupported = deck.unsupported_features();
    if !unsupported.is_empty() {
        log::warn!(
            "LEAPR deck '{}' uses features this port does not implement: {:?}",
            deck.title,
            unsupported
        );
        return Err(NjoyError::NotPorted(
            "LEAPR deck uses features this port does not implement \
             (see LeaprDeck::unsupported_features)",
        ));
    }
    let temps = deck.temperatures_k();
    let ntempr = temps.len();
    if ntempr == 0 {
        return Err(NjoyError::EndfParse(
            "leapr: deck declares no temperature block".into(),
        ));
    }

    // the scatterer/temperature loops (leapr.f90:323-397)
    let mut laws = Vec::with_capacity(ntempr);
    for (block, &t) in temps.iter().enumerate() {
        laws.push(build_law_at_temperature(deck, block, t)?);
    }
    let constants = laws[0].constants;
    let mixed = deck.is_mixed_moderator();

    // `iel = -1` for a solid with no translational term (leapr.f90:3043):
    // `twt` is the module global, i.e. the last card 11 read.
    let twt_last = deck.input_at(ntempr - 1)?.continuous.twt;
    let mut iel = deck.iel.code();
    if iel == 0 && twt_last == 0.0 {
        iel = -1;
    }

    // Bragg edges for a built-in lattice (leapr.f90:411-419, emax = 5 eV);
    // sb*npr for incoherent elastic (:3007, :3169).
    let elastic = if iel > 0 {
        let lattice = deck.iel.coherent_lattice().ok_or(NjoyError::NotPorted(
            "leapr: iel > 0 without a built-in lattice for this code",
        ))?;
        ElasticOutput::Coherent(coher_with_constants(
            lattice,
            deck.npr as usize,
            COHERENT_ELASTIC_EMAX_EV,
            constants,
        ))
    } else if iel < 0 {
        let sb = deck.spr * ((1.0 + deck.awr) / deck.awr).powi(2);
        ElasticOutput::Incoherent {
            sb_npr: sb * deck.npr as f64,
        }
    } else {
        ElasticOutput::None
    };
    let nedge = match &elastic {
        ElasticOutput::Coherent(b) | ElasticOutput::CoherentPreWeighted(b) => b.edges.len(),
        _ => 0,
    };

    let output = LeaprOutput {
        mat: deck.mat,
        za: deck.za,
        awr: deck.awr,
        lat: if deck.lat { 1 } else { 0 },
        isym: deck.isabt,
        ilog: deck.ilog != 0,
        smin: deck.smin,
        alpha: deck.alpha.clone(),
        beta: deck.beta.clone(),
        temperatures_k: temps.clone(),
        dwpix: laws.iter().map(|l| l.dwpix).collect(),
        tempf: laws.iter().map(|l| l.tempf).collect(),
        tempf_secondary: if mixed {
            Some(
                laws.iter()
                    .map(|l| l.tempf_secondary.unwrap_or(0.0))
                    .collect(),
            )
        } else {
            None
        },
        dwpix_secondary: if mixed {
            Some(
                laws.iter()
                    .map(|l| l.dwpix_secondary.unwrap_or(0.0))
                    .collect(),
            )
        } else {
            None
        },
        ssm: laws.into_iter().map(|l| l.ssm).collect(),
        ssp: None,
        npr: deck.npr,
        spr: deck.spr,
        elastic,
        secondary: deck.secondary_scatterer(),
        constants,
    };
    let tape = endout(&output);
    let header = Mf1Header {
        mat: deck.mat,
        za: deck.za,
        awr: deck.awr,
        emax: sigfig(THERM_EV * deck.beta[deck.beta.len() - 1], 7, 0),
        comments: deck.comments.clone(),
        iel,
        ntempr,
        nedge,
        nalpha: deck.alpha.len(),
        nbeta: deck.beta.len(),
    };
    Ok(LeaprRun {
        title: deck.title.clone(),
        output,
        tape,
        header,
    })
}

/// [`run_deck`] on deck text (`LeaprDeck::parse`), the closest thing to
/// `njoy < deck`.
///
/// # Errors
/// Those of [`LeaprDeck::parse`] and [`run_deck`].
pub fn run_deck_text(text: &str) -> Result<LeaprRun, NjoyError> {
    run_deck(&LeaprDeck::parse(text)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The dictionary card counts against the values NJOY wrote on the two
    /// oracle tapes: H-in-H2O (222 x 317, one temperature, no elastic) has
    /// `NC = 24097` for MT=4 and `60` for MT=451 with 54 comment cards;
    /// SiO2 (72 x 124, five temperatures, incoherent elastic) has `9677`,
    /// `5` and `78` with 71 cards.
    ///
    /// **Result (2026-09-10).** Both reproduced exactly.
    #[test]
    fn dictionary_card_counts_match_njoy() {
        let h2o = Mf1Header {
            mat: 1,
            za: 1001.0,
            awr: 0.9991673,
            emax: 10.00008,
            comments: vec![String::new(); 54],
            iel: 0,
            ntempr: 1,
            nedge: 0,
            nalpha: 222,
            nbeta: 317,
        };
        assert_eq!(h2o.dictionary(), vec![(1, 451, 60, 0), (7, 4, 24097, 0)]);
        let sio2 = Mf1Header {
            mat: 47,
            za: 147.0,
            awr: 27.84423,
            emax: 2.46675,
            comments: vec![String::new(); 71],
            iel: -1,
            ntempr: 5,
            nedge: 0,
            nalpha: 72,
            nbeta: 124,
        };
        assert_eq!(
            sio2.dictionary(),
            vec![(1, 451, 78, 0), (7, 2, 5, 0), (7, 4, 9677, 0)]
        );
        assert_eq!(sio2.lines().len(), 4 + 71 + 3 + 2);
        assert!(sio2.lines()[0].starts_with(
            " 1.470000+2 2.784423+1         -1          0          0          0  47 1451    1"
        ));
    }
}
