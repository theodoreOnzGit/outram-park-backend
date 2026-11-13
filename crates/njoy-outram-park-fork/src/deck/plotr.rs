// SPDX-License-Identifier: AGPL-3.0-only
// Translated from NJOY2016.80 `src/plotr.f90` (git commit ac5adf5f33d893e42f2eed7fb286b0d51c7580da).
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under AGPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! **PLOTR from a deck** (gh:#1369): its cards ([`crate::plotr::read_plotr`])
//! and its run ([`crate::plotr::Plotr::run`]) on the deck's tape units. The
//! plot-command file goes on unit `nplt` as text, where VIEWR reads it.

use crate::deck::cards::CardReader;
use crate::deck::tapes::TapeUnits;
use crate::NjoyError;

/// Read PLOTR's cards and, unless `parse_only`, run it.
///
/// # Errors
/// A malformed card; PLOTR's own errors.
pub fn run_plotr(
    r: &mut CardReader,
    units: &mut TapeUnits,
    parse_only: bool,
) -> Result<(), NjoyError> {
    let cards = crate::plotr::read_plotr(r)?;
    if parse_only {
        return Ok(());
    }
    crate::plotr::Plotr::run(&cards, units)
}
