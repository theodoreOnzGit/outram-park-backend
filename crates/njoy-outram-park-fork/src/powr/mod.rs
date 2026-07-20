//! `POWR` — libraries for the EPRI-CELL / EPRI-CPM reactor codes.
//!
//! Prepares multigroup libraries in the format required by the Electric Power
//! Research Institute (EPRI) reactor-analysis codes EPRI-CELL and EPRI-CPM, from
//! GROUPR GENDF output. These lattice-physics codes compute core performance for
//! operating and reloading power reactors.
//!
//! **Upstream:** `powr.f90`. **Manual:** LA-UR-17-20093 §POWR.
//! **Status:** not yet ported — [`crate::NjoyError::NotPorted`] placeholder.
//! See `README.md` in this directory for theory, plan, and caveats.

use crate::NjoyError;

/// Run POWR. Placeholder until ported (Phase 6; see `docs/porting-plan.md`).
pub fn run() -> Result<(), NjoyError> {
    Err(NjoyError::NotPorted("powr"))
}
