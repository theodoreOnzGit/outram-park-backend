// SPDX-License-Identifier: AGPL-3.0-only
// Translated from NJOY2016.80 (commit ab27c6414a0f) `src/leapr.f90`, the unit
// handling of `subroutine leapr` (`:218-453`) and `endout` (`:2990-3170`).
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under AGPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! LEAPR in the deck driver (gh:#1369): the cards are read by the existing
//! [`LeaprDeck`] reader (no second parser), the run is [`run_deck`], and the
//! tape `endout` writes goes on card 1's unit `nout`.

use crate::deck::cards::CardReader;
use crate::deck::tapes::TapeUnits;
use crate::endf::tape::Tape;
use crate::leapr::deck::LeaprDeck;
use crate::leapr::run::run_deck;
use crate::leapr::vintage::PhysicalConstants;
use crate::NjoyError;

/// Read LEAPR's cards from the deck stream: cards 1-20 as `leapr.f90:216-372`
/// and `endout`'s card-20 loop read them, through [`LeaprDeck`]. The stream is
/// left at the record after the comment cards' terminator.
///
/// # Errors
/// Those of [`LeaprDeck::parse_with_records_left`].
pub fn read_leapr(r: &mut CardReader) -> Result<LeaprDeck, NjoyError> {
    // `LeaprDeck`'s cursor starts after a `leapr` line and stops at `stop`;
    // the module name was already read here, so give it one.
    let text = format!("leapr\n{}", r.rest());
    let before_stop = text.lines().take_while(|l| !l.trim().eq_ignore_ascii_case("stop")).count();
    let (deck, left) = LeaprDeck::parse_with_records_left(&text)?;
    r.skip(before_stop - 1 - left);
    Ok(deck)
}

/// Run LEAPR and put its tape on `nout` (`endout`, `leapr.f90:3049-3170`).
/// NJOY2016.80's `bk` is CODATA 2018 (`phys.f90:22`), whatever the deck's
/// evaluation date, so the run uses [`PhysicalConstants::Codata2018`].
///
/// # Errors
/// Those of [`run_deck`], a tape unit error, or the tape not reading back.
pub fn run_leapr(deck: &LeaprDeck, units: &mut TapeUnits) -> Result<(), NjoyError> {
    let nout = deck.nout;
    let run = run_deck(&deck.clone().with_constants(PhysicalConstants::Codata2018))?;
    let text = run.write_text()?;
    let tape = Tape::read(text.as_bytes())?;
    units.openz(nout, 1)?;
    units.store(nout, tape, None)?;
    if nout > 0 {
        // The coded file is `endout`'s text as written, not a re-write.
        units.put_text(nout, text);
    }
    Ok(())
}
