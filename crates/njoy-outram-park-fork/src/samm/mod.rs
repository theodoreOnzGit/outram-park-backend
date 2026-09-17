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

/// **Dead entry point. The R-matrix-limited physics in this module IS ported and
/// IS live — do not read this function's error as evidence otherwise.**
///
/// This mirrors `samm.f90`'s standalone NJOY-module entry, which nothing in this
/// port calls. RECONR reaches the same physics directly:
/// `reconr/mod.rs` → [`crate::reconr::rml::add_rml_range`] → [`setup`] +
/// [`xsformula::cssammy`], and that path has run end to end throughout.
///
/// # Why the wording is this emphatic
///
/// The old message — a bare `NotPorted("samm (R-matrix limited)")` under a
/// docstring reading *"Placeholder until ported"* — was taken at face value in
/// gh:#202 as proof that LRF=7 was unimplemented. It is not, and following that
/// inference would have cost a wasted re-port of a module that was already
/// there. A `NotPorted` marker on a function nobody calls is worse than no
/// marker: it is a false negative that reads like a finding.
///
/// LRF=7 is verified against NJOY2016 2016.79 on Sr-88 (MAT 3837, the only LRF=7
/// evaluation in `reference-data/endf/`) across all 44,326 points of NJOY's own
/// grid inside the resolved range — worst relative deviation MT=1 `9.80e-3`, and
/// `2.39e-3` at `err = 0.0001`. Record:
/// `verification_and_validation/reconr_sr88_lrf7_kbk_vs_njoy2016.md`; gate:
/// `tests/reconr_sr88_lrf7_kbk_njoy_golden.rs`.
///
/// What is genuinely absent is the **card-deck driver**, exactly as for
/// [`crate::reconr::run`]. Use the typed API.
///
/// # Errors
///
/// Always [`NjoyError::NotPorted`], naming the card-deck driver specifically so
/// the message cannot be mistaken for a statement about the physics.
pub fn run() -> Result<(), NjoyError> {
    Err(NjoyError::NotPorted(
        "samm card-deck driver (R-matrix-limited PHYSICS is ported and live via \
         reconr::rml::add_rml_range -- use the typed API)",
    ))
}
