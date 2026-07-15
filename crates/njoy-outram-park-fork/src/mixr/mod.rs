// Ported from NJOY2016 `src/mixr.f90` (git commit ac5adf5f33d893e42f2eed7fb286b0d51c7580da).
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under GPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! `MIXR` — linear combinations of cross sections onto a new PENDF tape.
//!
//! Constructs a new PENDF tape whose reactions are specified **linear
//! combinations** of cross sections from one or more input tapes — e.g. building
//! an elemental cross section from its isotopes by abundance weighting, or mixing
//! materials for plotting. Output contains ENDF File 1 (MT=451) and File 3
//! sections only, with linear-linear interpolation assumed (`mixr.f90:22-29`).
//!
//! # Module map
//!
//! - [`input`] — the six-card input deck as a typed [`MixrInput`] /
//!   [`MixComponent`] (`mixr.f90:31-56,99-121`).
//! - [`mix`] — the mixing engine: union-grid weighted sums producing a new
//!   [`crate::endf::tape::Tape`] (`mixr.f90:196-390`). The reusable core is
//!   [`mix::mix_reaction`], which returns the exact `(E, sigma)` points for one
//!   reaction.
//!
//! # What is ported vs deferred
//!
//! - **Ported (complete):** the mixing physics — [`mix::mix_reaction`],
//!   [`mix::gety_value`] (`gety`'s value retrieval), [`MixrInput::mix`] /
//!   [`mix::mix`] (full tape assembly with MF=1/451 + MF=3), and the
//!   [`MixrInput`] card model.
//! - **Ported (driver):** [`driver::run_mix`] — the file-level driver that reads
//!   each input tape from a byte source, mixes, and writes the output tape
//!   (`Tape::read -> mix -> Tape::write`, `mixr.f90:144-389`). It takes the deck
//!   as a typed [`MixrInput`] rather than re-parsing free-format cards from
//!   `nsysi` — the one deliberate divergence.
//! - **Deferred:** [`run`] — the no-argument module-registry shim. It cannot run
//!   without input tapes, so it returns [`crate::NjoyError::NotPorted`] and points
//!   at [`driver::run_mix`]; the free-format `nsysi` card *text* reader and the
//!   physical-unit `nout` handle remain unported (`mixr.f90:84-121`).
//! - **Documented divergence — MF=1/451 comment text.** The crate's `[f64; 6]`
//!   section-row model cannot store Hollerith characters, so the card-6
//!   description becomes a blank comment record. AWI/EMAX/NSUB **are** numeric and
//!   are now read back from the inputs' headers ([`mix::nsub_from_awi`]); only the
//!   comment *text* is still not representable.
//!
//! **Upstream:** `mixr.f90` (git `ac5adf5f`). **Manual:** LA-UR-17-20093 §MIXR.
//! See `README.md` in this directory for theory, fidelity notes, and V&V status.

pub mod driver;
pub mod input;
pub mod mix;

pub use driver::run_mix;
pub use input::{MixComponent, MixrInput};
pub use mix::{gety_value, mix, mix_reaction, nsub_from_awi, sigfig};

use crate::NjoyError;

/// Module-registry shim for MIXR (the no-argument entry point).
///
/// **Not ported** in this form — MIXR needs input tapes plus a deck, so this
/// zero-argument entry only names the module for the [`crate::NjoyModule`]
/// registry. Use [`driver::run_mix`] for the real, functional file-level driver
/// (`Tape::read -> mix -> Tape::write`), or [`MixrInput::mix`] for the in-memory
/// engine. What remains unported is only the free-format `nsysi` card-text reader
/// and the physical `nout` unit handle (`mixr.f90:84-121,380-389`).
pub fn run() -> Result<(), NjoyError> {
    Err(NjoyError::NotPorted(
        "mixr card-deck driver (functional driver: crate::mixr::driver::run_mix)",
    ))
}
