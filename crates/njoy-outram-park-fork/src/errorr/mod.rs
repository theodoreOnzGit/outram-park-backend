//! `ERRORR` — multigroup cross-section and distribution covariance matrices.
//!
//! Produces multigroup covariance matrices from ENDF File-3x covariance data
//! (MF=31/33/34/35/40): the group-averaged relative covariances of cross
//! sections, ν̄, angular distributions, and spectra that quantify nuclear-data
//! uncertainty for sensitivity/uncertainty (S/U) analysis. Output is a GENDF-like
//! covariance tape post-processed by COVR.
//!
//! **Upstream:** `errorr.f90` (~11.2k lines). **Manual:** LA-UR-17-20093 §ERRORR.
//! **Status:** not yet ported — [`crate::NjoyError::NotPorted`] placeholder.
//! See `README.md` in this directory for theory, plan, and caveats.

use crate::NjoyError;

/// Run ERRORR. Placeholder until ported (Phase 5; see `docs/porting-plan.md`).
pub fn run() -> Result<(), NjoyError> {
    Err(NjoyError::NotPorted("errorr"))
}
