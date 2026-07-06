//! `UNRESR` — effective self-shielded cross sections in the unresolved range.
//!
//! Produces effective self-shielded cross sections in the unresolved resonance
//! range (URR), where ENDF gives only average widths/spacings plus distribution
//! functions rather than individual resonances. Uses the **Bondarenko** narrow-
//! resonance flux model to tabulate σ(E, T, σ₀) versus background cross section
//! σ₀ — the precursor data GROUPR needs and, historically, PURR's ancestor.
//!
//! **Upstream:** `unresr.f90` (~1.8k lines). **Manual:** LA-UR-17-20093 §UNRESR.
//! **Status:** not yet ported — [`crate::NjoyError::NotPorted`] placeholder.
//! See `README.md` in this directory for theory, plan, and caveats.

use crate::NjoyError;

/// Run UNRESR. Placeholder until ported (Phase 3; see `docs/porting-plan.md`).
pub fn run() -> Result<(), NjoyError> {
    Err(NjoyError::NotPorted("unresr"))
}
