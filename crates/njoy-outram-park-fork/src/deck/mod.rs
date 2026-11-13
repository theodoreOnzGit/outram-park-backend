// SPDX-License-Identifier: AGPL-3.0-only
// Translated from NJOY2016.80 `src/main.f90` and `src/util.f90`; see each file.

//! **NJOY input-deck driver** (gh:#1369): run an NJOY deck as written.
//!
//! - [`cards`] — the free-format card reader, Fortran list-directed
//!   `read(nsysi,*)`: `/` terminators, `n*v` repeats, quoted strings, `--`
//!   comment cards, defaults kept across nulls. Checked against gfortran
//!   (`verification_and_validation/deck_driver/`).
//! - [`acer_thermal`] — ACER `iopt = 2`: the thermal ACE file and its directory
//!   line, from THERMR's PENDF and the run recorded on it (gh:#1369).
//! - [`tapes`] — `openz`/`repoz`/`closz`: unit numbers, ASCII (positive) and
//!   blocked-binary (negative) mode, units up to 999.
//! - [`modules`] — each module's card reader, translated from that module's
//!   own input block; they build the crate's existing typed inputs.
//! - [`run`] — `program njoy`'s loop: [`run_deck`].
//! - [`leapr`] — LEAPR's cards and run, its tape on `nout` (gh:#1369).
//! - [`acer_photon`] — ACER `iopt = 4` and `5` and their directory line.
//!
//! - [`pendf`] — the PENDF tape as NJOY writes it (MF=1 text and dictionary,
//!   MF=2, MF=3 heads, MF=10/12/13/23), so tapes compare line for line.
//!
//! **Status (part 2):** MODER, RECONR, BROADR (several temperatures,
//! bootstrap), HEATR and ACER `iopt = 1` (`newfor = 1` and, for MF=4 angular
//! distributions, `newfor = 0`), `iopt = 2` ([`acer_thermal`]) and `iopt = 7` run
//! from cards; UNRESR
//! and PURR read their cards and pass the tape through (their MT=152/153 are
//! not written; see [`DeckRun::notes`]); THERMR runs the free-gas case
//! (`iinc = 1`, [`thermr`]; **CORRECTED 2026-11-11**, was "no tape driver"),
//! GROUPR and GASPR read their cards but have no tape driver, so a deck
//! reaching them (or THERMR with `iinc = 2`, an elastic option or `iform = 1`)
//! stops with [`DeckRun::blocked`] set. VIEWR runs from its card on the plot
//! file the previous module left ([`crate::viewr`], gh:#1369). ~~The other
//! modules' cards are not yet read.~~ **CORRECTED 2026-11-13 (gh:#1369):**
//! LEAPR runs from its cards ([`leapr`], through `leapr::deck::LeaprDeck`), and
//! ACER `iopt = 4/5` (photo-atomic, photo-nuclear) runs ([`acer_photon`]). The
//! cards of GAMINR, ERRORR, COVR, DTFR, CCCCR, MATXSR, RESXSR, POWR, WIMSR, PLOTR
//! and MIXR are still not read; a deck reaching one stops there.

pub mod acer_photon;
pub mod acer_thermal;
pub mod cards;
pub mod groupr;
pub mod leapr;
pub mod modules;
pub mod pendf;
pub mod run;
pub mod tapes;
pub mod thermr;

pub use cards::{CardReader, Slot};
pub use run::{run_deck, DeckOptions, DeckRun};
pub use tapes::{PendfMeta, TapeUnits, Unit};
