//! `PLOTR` — generate VIEWR input to plot ENDF/PENDF/GENDF data.
//!
//! A general-purpose plotting front-end: it reads ENDF, PENDF, or GENDF files and
//! writes VIEWR input describing 2-D plots (e.g. cross section vs energy, with
//! lin/log axes, ranges, labels) and 3-D plots, which VIEWR then renders to
//! PostScript. PLOTR chooses *what* to plot; VIEWR draws it.
//!
//! **Upstream:** `plotr.f90`. **Manual:** LA-UR-17-20093 §PLOTR.
//! ~~**Status:** not yet ported — [`crate::NjoyError::NotPorted`] placeholder.~~
//! **CORRECTED 2026-11-13 (gh:#1369):** translated line for line from
//! `plotr.f90` and run from a deck (`crate::deck::plotr`):
//!
//! - [`cards`] — PLOTR's cards, read in upstream's order with its defaults;
//! - [`stream`] — the input tapes read card by card (`contio` ... `findf`);
//! - [`run`] — `subroutine plotr`: the curve loop, ENDF/PENDF 2-d curves,
//!   input data and the 2-d plot commands; [`mf7`], [`gendf`] and
//!   [`plot2d`] (data, the 2-d writer) and
//!   [`plot3d`] (`ad3d`, `ed3d`, `gg3d`) its other branches; [`state`] its
//!   variables, edit descriptors and record calls;
//! - [`getters`] — `gety1` on a tape, `getz`, `gety4`, `gety6`, `fixl7`,
//!   `ascale`; [`rname`] — the reaction names.
//!
//! Upstream deck 06 and 12 plot files (and VIEWR's PostScript of them) are
//! NJOY2016's line for line (`tests/deck_driver_plotr_vs_njoy2016.rs`).
//! [`run()`] below is the module-enum entry point, which has no cards; it
//! still returns [`crate::NjoyError::NotPorted`]. See `README.md`.

use crate::NjoyError;

pub mod cards;
pub mod gendf;
pub mod getters;
pub mod mf7;
pub mod plot2d;
pub mod plot3d;
pub mod rname;
pub mod run;
pub mod state;
pub mod stream;

pub use cards::{read_plotr, PlotrCards};
pub use run::Plotr;

/// Run PLOTR. ~~Placeholder until ported (Phase 6; see `docs/porting-plan.md`).~~
/// **CORRECTED 2026-11-13 (gh:#1369):** PLOTR is ported and runs from a deck
/// ([`Plotr::run`] on [`read_plotr`]'s cards); this card-less entry point
/// still returns `NotPorted`.
pub fn run() -> Result<(), NjoyError> {
    Err(NjoyError::NotPorted("plotr"))
}
