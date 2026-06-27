//! `BROADR` — Doppler-broaden and thin pointwise cross sections.
//!
//! Takes the pointwise tape from [`super::reconr`] and Doppler-broadens the
//! cross sections to one or more temperatures (Cullen–Weisbin SIGMA1 kernel),
//! then thins the grid back to tolerance. Second module in the ACE pipeline.
//!
//! Ported from NJOY2016 `src/broadr.f90` (~2,000 lines).
//!
//! **Status:** not yet ported — [`crate::NjoyError::NotPorted`] placeholder.

use crate::NjoyError;

/// Run BROADR. Placeholder until Phase 2 (see `docs/porting-plan.md`).
pub fn run() -> Result<(), NjoyError> {
    Err(NjoyError::NotPorted("broadr"))
}
