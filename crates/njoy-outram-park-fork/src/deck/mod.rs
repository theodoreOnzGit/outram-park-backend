// SPDX-License-Identifier: AGPL-3.0-only
// Translated from NJOY2016.80 `src/main.f90` and `src/util.f90`; see each file.

//! **NJOY input-deck driver** (gh:#1369): run an NJOY deck as written.
//!
//! - [`cards`] — the free-format card reader, Fortran list-directed
//!   `read(nsysi,*)`: `/` terminators, `n*v` repeats, quoted strings, `--`
//!   comment cards, defaults kept across nulls. Checked against gfortran
//!   (`verification_and_validation/deck_driver/`).
//! - [`tapes`] — `openz`/`repoz`/`closz`: unit numbers, ASCII (positive) and
//!   blocked-binary (negative) mode, units up to 999.
//! - [`modules`] — each module's card reader, translated from that module's
//!   own input block; they build the crate's existing typed inputs.
//! - [`run`] — `program njoy`'s loop: [`run_deck`].
//!
//! **Status (part 1):** MODER, RECONR, BROADR and HEATR run from cards; the
//! cards of THERMR, GROUPR and GASPR are read, but those modules have no
//! tape-level driver here yet, so a deck reaching them stops with
//! [`DeckRun::blocked`] set. UNRESR, PURR, ACER and the rest are not yet read.
//! The PENDF a deck passes between modules is the pointwise content
//! ([`crate::heatr::pendf_for_heatr`]), not a byte copy of NJOY's tape.

pub mod cards;
pub mod modules;
pub mod run;
pub mod tapes;

pub use cards::{CardReader, Slot};
pub use run::{run_deck, DeckOptions, DeckRun};
pub use tapes::{TapeUnits, Unit};
