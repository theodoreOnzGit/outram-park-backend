//! `GAMINR` — multigroup photoatomic cross sections and matrices.
//!
//! Produces complete multigroup photoatomic (photon-interaction) data from ENDF
//! photoatomic evaluations: total, coherent, incoherent, pair-production, and
//! photoelectric cross sections averaged over group structures and weights, with
//! Legendre group-to-group coherent/incoherent scattering matrices computed from
//! atomic form factors and incoherent scattering functions.
//!
//! **Upstream:** `gaminr.f90` (~2k lines). **Manual:** LA-UR-17-20093 §GAMINR.
//! **Status:** not yet ported — [`crate::NjoyError::NotPorted`] placeholder.
//! See `README.md` in this directory for theory, plan, and caveats.

use crate::NjoyError;

/// Run GAMINR. Placeholder until ported (Phase 5; see `docs/porting-plan.md`).
pub fn run() -> Result<(), NjoyError> {
    Err(NjoyError::NotPorted("gaminr"))
}
