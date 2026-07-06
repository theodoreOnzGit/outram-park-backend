//! `RESXSR` — pointwise resonance cross-section files for near-epithermal codes.
//!
//! Prepares pointwise resonance cross-section libraries for the near-epithermal
//! range (e.g. 4–200 eV) where the simple Bondarenko narrow-resonance model used
//! by TRANSX is inadequate because many resonances are *not* narrow with respect
//! to the scattering energy loss. RESXSR supplies the pointwise data needed for
//! intermediate-resonance / detailed self-shielding treatments.
//!
//! **Upstream:** `resxsr.f90`. **Manual:** LA-UR-17-20093 §RESXSR.
//! **Status:** not yet ported — [`crate::NjoyError::NotPorted`] placeholder.
//! See `README.md` in this directory for theory, plan, and caveats.

use crate::NjoyError;

/// Run RESXSR. Placeholder until ported (Phase 6; see `docs/porting-plan.md`).
pub fn run() -> Result<(), NjoyError> {
    Err(NjoyError::NotPorted("resxsr"))
}
