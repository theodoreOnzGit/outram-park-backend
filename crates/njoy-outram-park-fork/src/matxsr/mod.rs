//! `MATXSR` — MATXS generalized material cross-section files (for TRANSX).
//!
//! Writes the MATXS material cross-section format — a generalized CCCC-type
//! interface for neutron, photon, and charged-particle data including cross
//! sections, group-to-group matrices, temperature variation, self-shielding, and
//! time dependence — from GROUPR/GAMINR GENDF output. MATXS libraries are the
//! input to the TRANSX code, which builds effective cross sections for transport.
//!
//! **Upstream:** `matxsr.f90`. **Manual:** LA-UR-17-20093 §MATXSR.
//! **Status:** not yet ported — [`crate::NjoyError::NotPorted`] placeholder.
//! See `README.md` in this directory for theory, plan, and caveats.

use crate::NjoyError;

/// Run MATXSR. Placeholder until ported (Phase 6; see `docs/porting-plan.md`).
pub fn run() -> Result<(), NjoyError> {
    Err(NjoyError::NotPorted("matxsr"))
}
