//! `SAMM` — Reich–Moore / R-matrix-limited (RML) resonance kernel.
//!
//! Not a standalone NJOY *module* but a shared physics library of R-matrix
//! subroutines used by RECONR and UNRESR to evaluate cross sections for the
//! **R-matrix-limited (RML, LRF=7)** and Reich–Moore resonance formalisms. It
//! evaluates the resonance cross sections via the full R-matrix (channel matrix
//! inversion) rather than the SLBW/MLBW pole approximations, giving the correct
//! treatment for light nuclides and strongly overlapping resonances.
//!
//! **Upstream:** `samm.f90` (~7.2k lines). **Manual:** no standalone chapter —
//! theory is in §RECONR and the ENDF-102 LRF=7 spec.
//! **Status:** not yet ported — [`crate::NjoyError::NotPorted`] placeholder.
//! See `README.md` in this directory for theory, plan, and caveats.

use crate::NjoyError;

/// Evaluate an RML/Reich–Moore resonance section. Placeholder until ported;
/// RECONR (`crate::reconr`) currently handles SLBW/MLBW + Reich–Moore only.
pub fn run() -> Result<(), NjoyError> {
    Err(NjoyError::NotPorted("samm (R-matrix limited)"))
}
