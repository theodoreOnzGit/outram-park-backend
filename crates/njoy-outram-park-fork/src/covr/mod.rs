// Ported from NJOY2016 `src/covr.f90` (git commit ac5adf5f33d893e42f2eed7fb286b0d51c7580da).
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under GPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! `COVR` — post-process ERRORR covariance output (correlations + report).
//!
//! COVR is an *editing* module that consumes the multigroup covariance tape
//! produced by ERRORR and performs two largely independent functions
//! (`covr.f90:49-66`):
//!
//! 1. **Report / library** — per-group relative standard deviations, the
//!    correlation matrix, and a condensed BOXER-format covariance library.
//! 2. **Plotting** — VIEWR PostScript figures of the correlation matrix
//!    (shaded contour) and the standard-deviation vectors.
//!
//! **Upstream:** `covr.f90` (2250 lines, NJOY2016 commit
//! `ac5adf5f33d893e42f2eed7fb286b0d51c7580da`). **Manual:** LA-UR-17-20093
//! §COVR.
//!
//! # Port status — library option PORTED and oracle-verified; plotting out of scope
//!
//! ## Ported
//!
//! - [`input`] — the full COVR **card deck** ([`CovrInput`], [`CovrMode`], and
//!   the selector enums), the shade-level array expansion
//!   ([`PlotOptions::shade_levels`], `covr.f90:289-305`) and the MT-strip
//!   predicate ([`is_mt_stripped`], `covr.f90:546-553`).
//! - [`tape`] — the **ERRORR-tape reader** half of `covard`
//!   (`covr.f90:740-886`: tape classification, group structure, the MF=3
//!   vectors, the MF=33/34/35/40 subsection search) and the `expndo` scan of
//!   the MTs present (`covr.f90:526-556`).
//! - [`covard`] — the numeric half of `covard` (`covr.f90:895-935`): scatter
//!   the sparse rows into a dense matrix, zero spurious covariances where a
//!   cross section is zero, absolute -> relative; plus `expndo`'s pair
//!   enumeration ([`expand_mt_pairs`]) and the auto/cross rsd sourcing
//!   ([`correlation_from_auto_and_cross`]).
//! - [`correlation`] — the numeric heart of `subroutine corr`
//!   (`covr.f90:578-718`): [`CovarianceMatrix`], relative standard
//!   deviations, covariance -> correlation, the plot-stage clamp and shade
//!   levels.
//! - [`boxer`] / [`boxer_text`] — the **BOXER writer** (`press`/`setfor`,
//!   `covr.f90:1991-2247`): the run-length codec, the format table, and the
//!   byte-exact record layout, with a reader for verification.
//! - [`library`] — the **library-option driver** ([`run_library`]:
//!   `covr.f90:338-474` for `nout > 0` with `corr` underneath): ERRORR tape
//!   in, BOXER text out, byte-identical to NJOY2016 on the nine
//!   `reference-data/errorr/` tapes for both `matype = 3` and `4`
//!   (`tests/covr_boxer_golden.rs`).
//!
//! ## NOT ported (out of scope)
//!
//! - **All PostScript plotting** (`plotit`, `matshd`, `patlev`, `smilab`,
//!   `matmes`, `elem`, `mtno`, `truncg`, `covr.f90:939-1599,1649-1890`) — VIEWR
//!   figure generation, which OUTRAM PARK does not target; the plot option
//!   of [`run_with_deck`] returns [`NjoyError::NotPorted`]. The `epmin`
//!   reset of `corr` (`covr.f90:628-643`) belongs to that path too.
//! - Binary (`nin < 0`) tapes: the crate's ENDF reader is ASCII-only.
//!
//! See `README.md` in this directory for theory, the full ported-vs-NotPorted
//! table, and the recorded test results.

pub mod boxer;
pub mod boxer_text;
pub mod correlation;
pub mod covard;
pub mod input;
pub mod library;
pub mod tape;

pub use boxer::{
    compress, decompress, setfor, BoxerData, BoxerDataType, BoxerFormat, BoxerHeader, BoxerPage,
    BoxerShape,
};
pub use boxer_text::{parse_boxer_text, press_text, BoxerRecord};
pub use library::{run_library, CovrLibraryOutput, PairReport};
pub use tape::{
    present_mts, read_covariance_rows, read_covariance_section, read_group_structure, read_vector,
    ErrorrTapeKind, GroupStructure,
};
pub use correlation::{
    correlation_cross, shade_level, Correlation, CorrelationMatrix, CovarianceMatrix,
    CovarianceValue, RelativeStdDev,
};
pub use covard::{
    correlation_from_auto_and_cross, expand_mt_pairs, CovarianceElement, CovarianceRowBlock,
    CovardResult, CrossSectionBarn, ErrorrCovarianceSection, GroupBoundaryEv,
};
pub use input::{
    default_tlev, is_mt_stripped, ColorStyle, CovarianceForm, CovrInput, CovrMode, LegendOption,
    LibraryOptions, MatrixOutputType, PlotOptions, ReactionSelector, EPMIN_READ_FACTOR, NCASE_MAX,
};

use crate::NjoyError;

/// Module-dispatch entry point used by the [`crate::NjoyModule`] registry.
///
/// COVR needs a full input deck ([`CovrInput`]) to run, so this no-argument
/// form exists only so the module registry can name COVR; use
/// [`run_with_deck`] (NJOY's `tape<n>` convention) or [`run_library`] (a
/// parsed tape in, BOXER text out) for the real entry points.
///
/// # Errors
/// Always returns [`NjoyError::NotPorted`] with `"covr"`.
pub fn run() -> Result<(), NjoyError> {
    Err(NjoyError::NotPorted("covr"))
}

/// Run the COVR module from a parsed input deck (`subroutine covr`,
/// `covr.f90:49-506`), with NJOY's unit convention: the ERRORR tape is read
/// from `tape<nin>` and the BOXER library written to `tape<nout>` in the
/// current directory.
///
/// Only the **library option** (`nout > 0`) is ported — see [`run_library`]
/// for the tape-in / text-out form that needs no files. The **plot option**
/// (`nout <= 0`, VIEWR PostScript) is out of scope and returns
/// [`NjoyError::NotPorted`] rather than fabricating a result.
///
/// # Errors
/// [`NjoyError::EndfParse`] if the deck fails [`CovrInput::validate`];
/// [`NjoyError::NotPorted`] (`"covr::plot"`) for the plot option; otherwise
/// whatever [`run_library`], the tape reader, or the file I/O reports.
pub fn run_with_deck(input: &CovrInput) -> Result<(), NjoyError> {
    input.validate()?;
    match &input.mode {
        CovrMode::Plot(_) => Err(NjoyError::NotPorted("covr::plot")),
        CovrMode::Library(opts) => {
            let tape = crate::endf::tape::Tape::read_file(std::path::Path::new(&format!(
                "tape{}",
                input.nin.abs()
            )))?;
            let out = run_library(&tape, opts, &input.cases)?;
            std::fs::write(format!("tape{}", input.nout), out.text).map_err(NjoyError::Io)?;
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Methodology: the plot option is out of scope and must report
    /// `NotPorted`, never a fabricated figure. Feed a minimal valid
    /// plot-option deck and assert the exact tag. Result (2026-09-10):
    /// `NotPorted("covr::plot")`.
    #[test]
    fn run_reports_not_ported() {
        let input = CovrInput {
            nin: 20,
            nout: 0,
            nplot: 0,
            mode: CovrMode::Plot(PlotOptions::default()),
            cases: vec![ReactionSelector::auto(9228, 102)],
        };
        match run_with_deck(&input) {
            Err(NjoyError::NotPorted(tag)) => assert_eq!(tag, "covr::plot"),
            other => panic!("expected NotPorted(\"covr::plot\"), got {other:?}"),
        }
        // The registry no-arg form reports the module-level NotPorted tag.
        assert!(matches!(run(), Err(NjoyError::NotPorted("covr"))));
    }

    /// Methodology: an invalid deck (no card-4 selectors) must fail validation
    /// *before* reaching the NotPorted skeleton, surfacing the input error.
    /// Result (2026-07-15): returns an EndfParse error, not NotPorted.
    #[test]
    fn run_rejects_empty_case_list() {
        let input = CovrInput {
            nin: 20,
            nout: 0,
            nplot: 0,
            mode: CovrMode::Plot(PlotOptions::default()),
            cases: vec![],
        };
        assert!(matches!(
            run_with_deck(&input),
            Err(NjoyError::EndfParse(_))
        ));
    }
}
