//! `SAMM` — Reich–Moore / R-matrix-limited (RML) resonance kernel.
//!
//! Not a standalone NJOY *module* but a shared physics library of R-matrix
//! subroutines used by RECONR and UNRESR to evaluate cross sections for the
//! **R-matrix-limited (RML, LRF=7)** and Reich–Moore resonance formalisms. It
//! evaluates the resonance cross sections via the full R-matrix (channel matrix
//! inversion) rather than the SLBW/MLBW pole approximations, giving the correct
//! treatment for light nuclides and strongly overlapping resonances.
//!
//! **Upstream:** `samm.f90` (7169 lines — the SAMMY method, ported from
//! coding provided by Nancy Larson, ORNL). **Manual:** no standalone
//! chapter — theory is in §RECONR and the ENDF-102 LRF=7 spec.
//!
//! **Scope, matching what `samm.f90` itself supports:** upstream's own
//! `rdsammy` hard-errors on `IFG≠0` and `KRM≠3` — NJOY *itself* never
//! exercises the fully general R-matrix (KRM=1/2/4) or reduced-width
//! (IFG=1) cases, only Reich-Moore-limited (KRM=3, IFG=0). This port matches
//! that restriction.
//!
//! Ported so far:
//! - [`mf2`] — the ENDF LRF=7 (KRM=3) parameter reader (`rdsammy`'s
//!   `mode==7` branch). See its module doc for a flagged, unresolved
//!   verification question on the eliminated-channel reorder step.
//! - [`penetrability`] — hard-sphere penetrability/shift/phase-shift
//!   (`pf`, `genpsf`, `pgh`), for uncharged (non-Coulomb) channels.
//! - [`context`] — particle-pair defaults, quantum-number validation, and
//!   per-channel kinematic scale factors (`ppdefs`, `checkqn`, `fxradi`).
//!
//! **Scope (2026-07-07):** since RECONR — the only current caller — always
//! disables `Want_Partial_Derivs`/`Want_Angular_Dist` (`reconr.f90:149-150`),
//! the derivative routines (`babb`, `abpart`, `derres`, `derext`) and the
//! angular-distribution routines (`angle`, `lmaxxx`, `kclbsch`, `clbsch`,
//! `setleg`) are deferred until `ERRORR` (the only caller that enables them)
//! is actually being built, rather than ported now against no reachable
//! caller. See `README.md`.
//!
//! **Status:** the R-matrix inversion, Coulomb wave-function library, and
//! cross-section formula (`crosss`/`setr`) are not yet ported — see
//! `README.md` for the phased plan. `run()` remains
//! [`crate::NjoyError::NotPorted`].

pub mod context;
pub mod mf2;
pub mod penetrability;

use crate::NjoyError;

/// Evaluate an RML/Reich–Moore resonance section. Placeholder until ported;
/// RECONR (`crate::reconr`) currently handles SLBW/MLBW + Reich–Moore only.
pub fn run() -> Result<(), NjoyError> {
    Err(NjoyError::NotPorted("samm (R-matrix limited)"))
}
