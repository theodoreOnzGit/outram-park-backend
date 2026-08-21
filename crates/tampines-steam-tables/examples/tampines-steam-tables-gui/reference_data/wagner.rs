//! Wagner / Kretzschmar reference states — re-exported from the library.
//!
//! **Relocated to `tampines_steam_tables::tabulated_data::wagner_kretzschmar_2019`
//! on 2026-08-21 (GitHub issue #26).** This module used to hold its own
//! ~3,100-line copy of the tables, mechanically extracted from the crate's
//! `#[cfg(test)]` fixtures — see the [module docs](super) for why the *other*
//! four reference-data submodules (`moody`, `zaloudek`, `marviken`, `edwards`)
//! still duplicate their source that way. Wagner is the exception: the
//! maintainer asked for it to become a first-class, queryable part of the
//! library itself (`tampines_steam_tables::tabulated_data::TabulatedData`),
//! reachable by any consumer, not just this GUI — so this file is now a thin
//! re-export rather than a second copy of the same ~2,554 rows.
//!
//! Prefer [`tampines_steam_tables::tabulated_data::TabulatedData`] directly
//! for new code (`.isobar(...)`, `.isotherm(...)`, `.saturation_curve()`);
//! this re-export exists only so the rest of this example's existing
//! `wagner::WAGNER_SATURATION_TABLE` / `wagner::SAT_COL_*` call sites did not
//! all need touching in the same change.
#[allow(unused_imports)]
// row-type aliases: not named at any call site in this example, kept for parity with the library's own public surface
pub use tampines_steam_tables::tabulated_data::{
    WagnerSaturationRow, WagnerSinglePhaseRow, SAT_COL_H_LIQ, SAT_COL_H_VAP, SAT_COL_P_BAR,
    SAT_COL_S_LIQ, SAT_COL_S_VAP, SAT_COL_T_DEGC, WAGNER_SATURATION_TABLE,
    WAGNER_SINGLE_PHASE_TABLE,
};
