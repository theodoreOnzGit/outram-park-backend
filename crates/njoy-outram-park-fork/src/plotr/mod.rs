//! `PLOTR` — generate VIEWR input to plot ENDF/PENDF/GENDF data.
//!
//! A general-purpose plotting front-end: it reads ENDF, PENDF, or GENDF files and
//! writes VIEWR input describing 2-D plots (e.g. cross section vs energy, with
//! lin/log axes, ranges, labels) and 3-D plots, which VIEWR then renders to
//! PostScript. PLOTR chooses *what* to plot; VIEWR draws it.
//!
//! **Upstream:** `plotr.f90`. **Manual:** LA-UR-17-20093 §PLOTR.
//! **Status:** not yet ported — [`crate::NjoyError::NotPorted`] placeholder.
//! See `README.md` in this directory for theory, plan, and caveats.

use crate::NjoyError;

/// Run PLOTR. Placeholder until ported (Phase 6; see `docs/porting-plan.md`).
pub fn run() -> Result<(), NjoyError> {
    Err(NjoyError::NotPorted("plotr"))
}
