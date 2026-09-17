//! `VIEWR` — render 2-D/3-D plots to PostScript.
//!
//! A general-purpose plotting back-end: it reads plot-command input (typically
//! from PLOTR, COVR, or DTFR) defining 2-D and 3-D graphs and writes a
//! high-quality PostScript file. Supports curves with line patterns, legends,
//! tags/arrows, lin/log axes, and colour.
//!
//! **Upstream:** `viewr.f90` (+ shared `graph.f90`). **Manual:** LA-UR-17-20093
//! §VIEWR. **Status:** not yet ported — [`crate::NjoyError::NotPorted`]
//! placeholder. See `README.md` in this directory for theory, plan, and caveats.

use crate::NjoyError;

/// Run VIEWR. Placeholder until ported (Phase 6; see `docs/porting-plan.md`).
pub fn run() -> Result<(), NjoyError> {
    Err(NjoyError::NotPorted("viewr"))
}
