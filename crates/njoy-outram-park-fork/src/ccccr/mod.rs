//! `CCCCR` — standard CCCC interface files (ISOTXS, BRKOXS, DLAYXS).
//!
//! Produces the standard CCCC ("four cees") binary interface files developed by
//! the Committee for Computer Code Coordination for the US Fast Breeder Reactor
//! Program, from GROUPR GENDF output: **ISOTXS** (isotope-ordered multigroup
//! cross sections), **BRKOXS** (Bondarenko self-shielding factors), and
//! **DLAYXS** (delayed-neutron precursor data).
//!
//! **Upstream:** `ccccr.f90`. **Manual:** LA-UR-17-20093 §CCCCR.
//! **Status:** not yet ported — [`crate::NjoyError::NotPorted`] placeholder.
//! See `README.md` in this directory for theory, plan, and caveats.

use crate::NjoyError;

/// Run CCCCR. Placeholder until ported (Phase 6; see `docs/porting-plan.md`).
pub fn run() -> Result<(), NjoyError> {
    Err(NjoyError::NotPorted("ccccr"))
}
