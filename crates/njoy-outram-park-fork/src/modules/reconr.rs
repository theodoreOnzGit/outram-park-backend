//! `RECONR` — reconstruct pointwise cross sections from resonance parameters.
//!
//! Reconstructs energy-dependent cross sections from ENDF resonance parameters
//! (File 2) and interpolation schemes, to a user-specified tolerance, writing a
//! pointwise (File 3) ENDF tape. First module in the ACE pipeline.
//!
//! Ported from NJOY2016 `src/reconr.f90` (~5,700 lines).
//!
//! **Status:** not yet ported — [`crate::NjoyError::NotPorted`] placeholder.

use crate::NjoyError;

/// Run RECONR. Placeholder until Phase 2 (see `docs/porting-plan.md`).
pub fn run() -> Result<(), NjoyError> {
    Err(NjoyError::NotPorted("reconr"))
}
