//! `MIXR` — linear combinations of cross sections onto a new PENDF tape.
//!
//! Constructs a new PENDF tape whose reactions are specified **linear
//! combinations** of cross sections from one or more input tapes — e.g. building
//! an elemental cross section from its isotopes by abundance weighting, or mixing
//! materials for plotting. Output contains ENDF File 1 and File 3 sections only,
//! with linear-linear interpolation assumed.
//!
//! **Upstream:** `mixr.f90`. **Manual:** LA-UR-17-20093 §MIXR.
//! **Status:** not yet ported — [`crate::NjoyError::NotPorted`] placeholder.
//! See `README.md` in this directory for theory, plan, and caveats.

use crate::NjoyError;

/// Run MIXR. Placeholder until ported (Phase 6; see `docs/porting-plan.md`).
pub fn run() -> Result<(), NjoyError> {
    Err(NjoyError::NotPorted("mixr"))
}
