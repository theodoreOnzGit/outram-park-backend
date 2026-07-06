//! `LEAPR` — generate the thermal scattering law S(α,β) (ENDF MF=7).
//!
//! Computes the thermal scattering law S(α,β) for bound moderators from a
//! phonon/vibrational model and writes it in ENDF-6 MF=7 form for THERMR to
//! consume. Combines a solid-type **phonon expansion** (from a phonon frequency
//! spectrum ρ(ω)), a **translational/diffusive** term, and **discrete
//! oscillators** for molecular vibrations. It is the upstream generator that
//! THERMR turns into cross sections.
//!
//! **Upstream:** `leapr.f90` (~3.6k lines). **Manual:** LA-UR-17-20093 §LEAPR.
//! **Status:** not yet ported — [`crate::NjoyError::NotPorted`] placeholder.
//! See `README.md` in this directory for theory, plan, and caveats.

use crate::NjoyError;

/// Run LEAPR. Placeholder until ported (Phase 5; see `docs/porting-plan.md`).
pub fn run() -> Result<(), NjoyError> {
    Err(NjoyError::NotPorted("leapr"))
}
