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
//! Ported so far (Phases 1–5 of 6, see `README.md` for the full phased
//! plan and per-phase caveats):
//! - [`mf2`] — the ENDF LRF=7 (KRM=3) parameter reader. See its module doc
//!   for a flagged, unresolved verification question on the eliminated-
//!   channel reorder step.
//! - [`penetrability`] — hard-sphere penetrability/shift/phase-shift, for
//!   uncharged (non-Coulomb) channels.
//! - [`context`] — particle-pair defaults, quantum-number validation, and
//!   per-channel kinematic scale factors.
//! - [`betset`] — energy-independent per-resonance channel amplitudes.
//! - [`coulomb`] — the Coulomb wave-function library, for charged channels.
//! - [`linpack`] / [`rmatrix_invert`] — the R-matrix (level-matrix)
//!   inversion, closed-form for 1–3 channels and via a general LINPACK-
//!   style complex-symmetric solver for 4+.
//! - [`setup`] — one-time per-section setup tying the above together
//!   (`ppsammy`'s non-derivative, non-angular core).
//! - [`xsformula`] — the cross-section formula itself:
//!   [`xsformula::cross_sections`] (per-particle-pair) and
//!   [`xsformula::cssammy`] (the RECONR-facing MT-slot wrapper — **the
//!   actual entry point**, `samm.f90`'s own `cssammy`). This is where an
//!   actual cross section, in barns, comes out end-to-end.
//!
//! - [`derivs`] — the resonance-parameter derivatives
//!   (`Want_Partial_Derivs`: `babb`, `abpart`'s derivative half, `setqri`,
//!   `settri`, `derres`), driven by [`xsformula::cross_sections_with_derivs`]
//!   / [`xsformula::cssammy_with_derivs`] for ERRORR's `LRF=7` MF=32 path.
//!
//! **Verified** against NJOY2016 on ENDF/B-VII.1 Cl-35 (2026-09-11): the
//! cross sections at every node of NJOY's RECONR grid to the 7-figure
//! printing floor (`tests/reconr_cl35_rml_njoy_golden.rs`), and the
//! derivatives through ERRORR's group covariances to 4.8e-7
//! (`tests/errorr_mf32_cl35_rml_golden.rs`).
//!
//! **Not ported:** the angular-distribution routines (`angle`, `lmaxxx`,
//! `kclbsch`, `clbsch`, `setleg`) — both NJOY2016 callers hard-code
//! `Want_Angular_Dist = .false.` (`reconr.f90:149-150`, `errorr.f90:393`),
//! so they are dead code upstream as shipped; and `derext` (background
//! R-matrix parameters, `KBK > 0`, which [`mf2`] does not carry).
//! `run()` remains [`crate::NjoyError::NotPorted`] — it was never `samm`'s
//! real entry point; RECONR calls [`setup::setup`] + [`xsformula::cssammy`]
//! and ERRORR [`setup::setup_with_derivs`] + [`xsformula::cssammy_with_derivs`].

pub mod betset;
pub mod context;
pub mod coulomb;
pub mod derivs;
pub mod linpack;
pub mod mf2;
pub mod penetrability;
pub mod rmatrix_invert;
pub mod setup;
pub mod xsformula;

use crate::NjoyError;

/// Evaluate an RML/Reich–Moore resonance section. Placeholder until ported;
/// RECONR (`crate::reconr`) currently handles SLBW/MLBW + Reich–Moore only.
pub fn run() -> Result<(), NjoyError> {
    Err(NjoyError::NotPorted("samm (R-matrix limited)"))
}
