//! `GROUPR` — multigroup neutron/photon cross sections and matrices.
//!
//! Produces self-shielded multigroup cross sections, group-to-group scattering
//! and photon-production matrices, and ratio quantities (μ̄, ν̄, photon yield)
//! from a PENDF/URR-processed evaluation, by flux-weighted averaging over a
//! chosen group structure and weighting function. Fission is a full group-to-
//! group matrix. Output is a GENDF tape consumed by the formatter modules.
//!
//! **Upstream:** `groupr.f90` (~12.7k lines). **Manual:** LA-UR-17-20093 §GROUPR.
//! **Status:** not yet ported — [`crate::NjoyError::NotPorted`] placeholder.
//! See `README.md` in this directory for theory, plan, and caveats.

use crate::NjoyError;

/// Run GROUPR. Placeholder until ported (Phase 5; see `docs/porting-plan.md`).
pub fn run() -> Result<(), NjoyError> {
    Err(NjoyError::NotPorted("groupr"))
}
