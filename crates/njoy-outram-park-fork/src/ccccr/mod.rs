// SPDX-License-Identifier: AGPL-3.0-only
// Translated from NJOY2016.80 `src/ccccr.f90` (git commit ac5adf5f33d893e42f2eed7fb286b0d51c7580da):
//   - the run order of `subroutine ccccr`, l.226-275.
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under AGPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! `CCCCR` — standard CCCC interface files (ISOTXS, BRKOXS, DLAYXS).
//!
//! Produces the standard CCCC ("four cees") binary interface files developed by
//! the Committee for Computer Code Coordination for the US Fast Breeder Reactor
//! Program, from GROUPR GENDF output: **ISOTXS** (isotope-ordered multigroup
//! cross sections), **BRKOXS** (Bondarenko self-shielding factors), and
//! **DLAYXS** (delayed-neutron precursor data).
//!
//! **Upstream:** `ccccr.f90`. **Manual:** LA-UR-17-20093 §CCCCR.
//!
//! ~~**Status:** not yet ported — [`crate::NjoyError::NotPorted`] placeholder.~~
//! **Status (2026-11-13, gh:#1369):** translated line for line, run from a deck
//! by [`run_ccccr`] (`crate::deck` reads the cards). The three files are
//! byte-identical to NJOY2016.80's on its test decks 02 (ISOTXS, BRKOXS) and 76
//! (DLAYXS). Untranslated paths are refused by name: ISOTXS `ifopt = 2`,
//! `nsblok = ngroup` and a chi matrix (`ichix > 1`); BRKOXS factors too large
//! for one block. The listing (`lprint = 1`: `pisotx`, `pbrkxs`, `pdlyxs`) is
//! not written. See `README.md` in this directory.
//!
//! - [`core`]: the equivalenced container, scratch tapes, record writer.
//! - [`state`]: the cards and the module variables.
//! - [`isotxs`], [`brkoxs`], [`dlayxs`]: the three files.

pub mod brkoxs;
pub mod core;
pub mod dlayxs;
pub mod isotxs;
pub mod state;

pub use state::{BrkoxsCards, CcccrCards, IsotopeCard, IsotxsCards, IsotxsIsotope};

use crate::errorr::gendf::GendfTape;
use crate::NjoyError;

/// The files one CCCCR run writes (gfortran unformatted sequential bytes).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CcccrOutput {
    /// ISOTXS, when `nisot /= 0`.
    pub isotxs: Option<Vec<u8>>,
    /// BRKOXS, when `nbrks /= 0`.
    pub brkoxs: Option<Vec<u8>>,
    /// DLAYXS, when `ndlay /= 0` and the tape has delayed-neutron data.
    pub dlayxs: Option<Vec<u8>>,
    /// The messages upstream prints with `mess`.
    pub notes: Vec<String>,
}

/// Run CCCCR (`ccccr.f90:226-252`) on the GENDF `gendf` read from unit `nin`.
///
/// # Errors
/// The refusals of [`isotxs::cisotx`] and [`brkoxs::cbrkxs`]; a GENDF tape that
/// ends early or has a different group structure from card 4.
pub fn run_ccccr(cards: &CcccrCards, gendf: &GendfTape) -> Result<CcccrOutput, NjoyError> {
    let mut s = state::State::new(cards, core::Cursor::new(gendf));
    let mut out = CcccrOutput::default();
    let [_, nisot, nbrks, ndlay] = cards.units;
    if nisot != 0 {
        let c = cards.isotxs.as_ref().ok_or_else(|| NjoyError::InvalidInput("ccccr: no cisotx cards".into()))?;
        out.isotxs = Some(isotxs::cisotx(&mut s, c)?);
    }
    if nbrks != 0 {
        let c = cards.brkoxs.as_ref().ok_or_else(|| NjoyError::InvalidInput("ccccr: no cbrkxs cards".into()))?;
        out.brkoxs = Some(brkoxs::cbrkxs(&mut s, c)?);
    }
    if ndlay != 0 {
        out.dlayxs = dlayxs::cdlyxs(&mut s)?;
    }
    out.notes = s.notes;
    Ok(out)
}

/// Run CCCCR as an [`crate::modules::NjoyModule`] with no tape units bound.
/// CCCCR runs from a deck ([`run_ccccr`] through `crate::deck`); this
/// unit-less entry point stays a refusal, as for the other modules.
pub fn run() -> Result<(), NjoyError> {
    Err(NjoyError::NotPorted("ccccr"))
}
