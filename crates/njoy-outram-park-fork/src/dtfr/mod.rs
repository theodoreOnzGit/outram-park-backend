//! `DTFR` — DTF-IV format libraries for discrete-ordinates (Sₙ) codes.
//!
//! Prepares multigroup transport tables in the DTF-IV card-image format accepted
//! by many discrete-ordinates (Sₙ) and diffusion codes, from GROUPR GENDF output.
//! Also contains a simple plotting capability. DTFR was NJOY's first output
//! module and is largely superseded by MATXS/TRANSX.
//!
//! **Upstream:** `dtfr.f90`. **Manual:** LA-UR-17-20093 §DTFR.
//! **Status:** not yet ported — [`crate::NjoyError::NotPorted`] placeholder.
//! See `README.md` in this directory for theory, plan, and caveats.

use crate::NjoyError;

/// Run DTFR. Placeholder until ported (Phase 6; see `docs/porting-plan.md`).
pub fn run() -> Result<(), NjoyError> {
    Err(NjoyError::NotPorted("dtfr"))
}
