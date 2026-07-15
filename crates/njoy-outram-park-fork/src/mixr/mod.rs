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
//! - **Deferred:** [`run`] — the file-level card-deck driver that reads the six
//!   cards from a physical `nsysi` unit and writes to a physical `nout` unit.
//!   Returns [`crate::NjoyError::NotPorted`], matching every other module in
//!   this crate (`crate::moder`, `crate::reconr`, …): the physics lives in a
//!   typed API, `run()` is the deferred shim. Callers drive MIXR through
//!   [`crate::endf::tape::Tape::read`] -> [`MixrInput::mix`] ->
//!   [`crate::endf::tape::Tape::write`].
//!
//! **Upstream:** `mixr.f90` (git `ac5adf5f`). **Manual:** LA-UR-17-20093 §MIXR.
//! See `README.md` in this directory for theory, fidelity notes, and V&V status.

pub mod input;
pub mod mix;

pub use input::{MixComponent, MixrInput};
pub use mix::{gety_value, mix, mix_reaction, sigfig};

use crate::NjoyError;

/// Run MIXR from a physical card deck (the file-level driver).
///
/// **Not ported** — deferred exactly as `crate::moder::run` is. The mixing
/// engine is reached directly through [`MixrInput::mix`] / [`mix::mix`]; tape
/// (de)serialisation through [`crate::endf::tape::Tape::read`] /
/// [`crate::endf::tape::Tape::write`]. What remains unported is only the
/// `nsysi`/`nout` card-reader shell of `mixr.f90:84-121,380-389`.
pub fn run() -> Result<(), NjoyError> {
    Err(NjoyError::NotPorted(
        "mixr card-deck driver (mixing engine: crate::mixr::MixrInput::mix)",
    ))
}
