//! `COVR` — post-process and plot multigroup covariances from ERRORR.
//!
//! An editing module that post-processes ERRORR output: it reads the multigroup
//! covariance matrices and either (a) writes them in publication/interchange
//! forms (e.g. BOXER-style, or spectra), or (b) generates PostScript plots of
//! relative standard deviations and correlation matrices via the VIEWR path.
//!
//! **Upstream:** `covr.f90` (~3k lines). **Manual:** LA-UR-17-20093 §COVR.
//! **Status:** not yet ported — [`crate::NjoyError::NotPorted`] placeholder.
//! See `README.md` in this directory for theory, plan, and caveats.

use crate::NjoyError;

/// Run COVR. Placeholder until ported (Phase 5/6; see `docs/porting-plan.md`).
pub fn run() -> Result<(), NjoyError> {
    Err(NjoyError::NotPorted("covr"))
}
