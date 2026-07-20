//! `WIMSR` — libraries for the WIMS reactor-physics code.
//!
//! Prepares multigroup libraries for the WIMS ("Winfrith Improved Multigroup
//! Scheme") lattice-physics code (WIMS-D / WIMS-E) from GROUPR GENDF output. WIMS
//! uses collision-probability methods for pin-cell and assembly flux solutions
//! and needs cross sections, scattering matrices, and resonance-integral /
//! self-shielding data in its own library format.
//!
//! **Upstream:** `wimsr.f90`. **Manual:** LA-UR-17-20093 §WIMSR.
//! **Status:** not yet ported — [`crate::NjoyError::NotPorted`] placeholder.
//! See `README.md` in this directory for theory, plan, and caveats.

use crate::NjoyError;

/// Run WIMSR. Placeholder until ported (Phase 6; see `docs/porting-plan.md`).
pub fn run() -> Result<(), NjoyError> {
    Err(NjoyError::NotPorted("wimsr"))
}
