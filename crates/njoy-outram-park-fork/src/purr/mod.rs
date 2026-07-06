//! `PURR` — unresolved-resonance probability tables for Monte Carlo.
//!
//! Produces probability tables for the unresolved resonance range (URR),
//! suitable for continuous-energy Monte Carlo self-shielding (MCNP/OpenMC),
//! where the Bondarenko method used by UNRESR/GROUPR is not applicable. Builds
//! the tables by sampling explicit **resonance ladders** from the ENDF average
//! parameters and binning the resulting cross sections into probability bins per
//! energy.
//!
//! **Upstream:** `purr.f90` (~2.9k lines). **Manual:** LA-UR-17-20093 §PURR.
//! **Status:** not yet ported — [`crate::NjoyError::NotPorted`] placeholder.
//! See `README.md` in this directory for theory, plan, and caveats.

use crate::NjoyError;

/// Run PURR. Placeholder until ported (Phase 3; see `docs/porting-plan.md`).
pub fn run() -> Result<(), NjoyError> {
    Err(NjoyError::NotPorted("purr"))
}
