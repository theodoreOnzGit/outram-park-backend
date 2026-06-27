//! `ACER` — prepare ACE libraries for continuous-energy Monte Carlo.
//!
//! The terminal module of the OpenMC pipeline: it consumes the broadened
//! pointwise tape (plus optional probability tables from `PURR` and thermal
//! scattering from `THERMR`) and writes an **ACE** file — the format OpenMC and
//! MCNP read directly. This is the whole reason `njoy-outram-park-fork` exists
//! inside OUTRAM PARK.
//!
//! Upstream this is the largest area of NJOY: `src/acer.f90` plus the `ace*`
//! helpers (`acefc.f90` ~19,700 lines, `acepn`, `acepa`, `aceth`, `acedo`,
//! `acecm`). It is the last phase of the port.
//!
//! **Status:** not yet ported — [`crate::NjoyError::NotPorted`] placeholder.

use crate::NjoyError;

/// Run ACER. Placeholder until Phase 4 (see `docs/porting-plan.md`).
pub fn run() -> Result<(), NjoyError> {
    Err(NjoyError::NotPorted("acer"))
}
