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
//! # Port status — PARTIAL (AI draft, untrusted until reviewed)
//!
//! This port covers the **self-contained, testable math** and the **input
//! card model**, and honestly marks the tape-format reader and all plotting /
//! BOXER-output generation as not ported.
//!
//! ## Ported
//!
//! - [`input`] — the full COVR **card deck** ([`CovrInput`], [`CovrMode`], and
//!   the selector enums), plus two self-contained input-processing kernels:
//!   the shade-level array expansion ([`PlotOptions::shade_levels`],
//!   `covr.f90:289-305`) and the MT-strip predicate ([`is_mt_stripped`],
//!   `covr.f90:546-553`).
//! - [`correlation`] — the numeric heart of `subroutine corr`
//!   (`covr.f90:578-718`): [`CovarianceMatrix`] with its relative
//!   standard deviations ([`CovarianceMatrix::relative_std_dev`]) and the
//!   covariance -> correlation transform ([`CovarianceMatrix::to_correlation`],
//!   [`correlation_cross`]), plus the plot-stage clamp
//!   ([`CorrelationMatrix::clamped`]) and shade-level indexing ([`shade_level`]).
//! - [`covard`] — the **ERRORR-covariance reader transform** (`subroutine
//!   covard`, `covr.f90:815-935`) operating on an in-memory
//!   [`ErrorrCovarianceSection`]: scatter sparse row-blocks into a dense matrix,
//!   zero spurious covariances in zero-cross-section regions, and convert
//!   absolute to relative covariances. Also the `expndo` **MT-pair enumeration**
//!   ([`expand_mt_pairs`], `covr.f90:557-569`) and the auto/cross rsd sourcing
//!   ([`correlation_from_auto_and_cross`], `covr.f90:597-711`). The ENDF *tape
//!   I/O* half (decoding `contio`/`listio` records off a physical tape) is the
//!   remaining gap — see below.
//! - [`boxer`] — the **BOXER-format library writer** (`subroutine press`/
//!   `setfor`, `covr.f90:1991-2247`): the run-length [`compress`]/[`decompress`]
//!   codec, the [`setfor`] format selection, and the [`press_text`] record
//!   layout.
//!
//! ## NOT ported (honest gap list)
//!
//! - **ERRORR *tape I/O*** (the `repoz`/`finds`/`contio`/`listio`/`moreio` half
//!   of `covard`, `covr.f90:740-886`, and the `expndo` tape scan collecting the
//!   present MTs, `covr.f90:526-556`). This crate's ERRORR module produces no
//!   covariance *tape* yet (its `covout`/`colaps` kernels are unported), so
//!   there is no byte stream to decode: [`covard`] ports the numeric transform
//!   against the in-memory [`ErrorrCovarianceSection`] instead, and the
//!   record-decoding layer stays a documented gap. [`run`] returns
//!   [`NjoyError::NotPorted`].
//! - **All PostScript plotting** (`plotit`, `matshd`, `patlev`, `smilab`,
//!   `matmes`, `elem`, `mtno`, `truncg`, `covr.f90:939-1599,1649-1890`) — VIEWR
//!   figure generation, which OUTRAM PARK does not target. Only the
//!   self-contained numeric pieces used *by* the plot path (`level`, the shade
//!   array) are ported, in [`correlation`]/[`input`].
//!
//! See `README.md` in this directory for theory, the full ported-vs-NotPorted
//! table, and the recorded test results.

pub mod boxer;
pub mod correlation;
pub mod covard;
pub mod input;

pub use boxer::{
    compress, decompress, press_text, setfor, BoxerData, BoxerDataType, BoxerFormat, BoxerHeader,
    BoxerPage, BoxerShape,
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
/// form exists only so the module registry can name COVR; it reports that the
/// end-to-end pipeline is not yet ported (the tape reader, BOXER writer, and
/// plotting are unported). For the real entry point use [`run_with_deck`] with
/// a [`CovrInput`].
///
/// # Errors
/// Always returns [`NjoyError::NotPorted`] with `"covr"`.
pub fn run() -> Result<(), NjoyError> {
    Err(NjoyError::NotPorted("covr"))
}

/// Run the COVR module from a parsed input deck (`subroutine covr`,
/// `covr.f90:49-506`).
///
/// This is the **orchestration skeleton only**. The card deck is fully modelled
/// by [`CovrInput`], and the covariance -> correlation math is provided by
/// [`correlation`]; but the ERRORR tape reader (`covard`), the BOXER library
/// writer (`press`), and all VIEWR PostScript plotting are **not ported** (see
/// the module docs). This function therefore documents the pipeline and returns
/// [`NjoyError::NotPorted`] rather than fabricating a result.
///
/// To exercise the ported math directly, build a [`CovarianceMatrix`] and call
/// [`CovarianceMatrix::to_correlation`] / [`CovarianceMatrix::relative_std_dev`].
///
/// # Errors
/// Returns [`NjoyError::EndfParse`] if the input deck fails validation
/// ([`CovrInput::validate`]); otherwise always [`NjoyError::NotPorted`] with
/// `"covr::run"`.
pub fn run_with_deck(input: &CovrInput) -> Result<(), NjoyError> {
    // --- Stage 0: validate the card deck (covr.f90:183-260) --------------
    //   card 1 (nin,nout,nplot) -> input.{nin,nout,nplot};
    //   plot vs library cards -> input.mode; card 4 list -> input.cases.
    input.validate()?;

    // --- Stage 1: verify the ERRORR tape header (covr.f90:314-333) -------
    //   tpidio/contio, require MF1/MT451, set mf35 flag from mfflg
    //   (-11/-14 -> MF3, -12 -> MF5). NOT PORTED: ENDF tape I/O.

    // --- Stage 2: loop over cases (covr.f90:338-474) ---------------------
    //   per case: copy MAT (and MAT1) to scratch, expand the MT-MT1 list via
    //   expndo when mt<=0 (MT-strip logic is ported as `is_mt_stripped`),
    //   then per MT-MT1 pair call `corr` (covr.f90:384).

    // --- Stage 3: covariance -> correlation (covr.f90:578-718) -----------
    //   PORTED as `correlation`: read absolute covariances (covard, NOT
    //   PORTED), form per-group rsd = sqrt(diag), and
    //   corr(i,j) = cov(i,j)/(rsd_x(i)*rsd_y(j)).

    // --- Stage 4a: plot option (nout<=0, covr.f90:386-422) ---------------
    //   truncg + plotit + matshd -> VIEWR PostScript. NOT PORTED (plotting).

    // --- Stage 4b: library option (nout>0, covr.f90:425-459) -------------
    //   press() writes the BOXER-format covariance/correlation library.
    //   NOT PORTED (output formatting).

    // --- Stage 5: table of contents + cleanup (covr.f90:481-505) ---------
    //   plot ToC, copy statistics file, close tapes, timer.

    // Tape reader, BOXER writer, and plotting are not ported — never fabricate.
    Err(NjoyError::NotPorted("covr::run"))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Methodology: the driver skeleton must report `NotPorted` for a valid
    /// deck (the tape reader / plotting / BOXER writer are unported), never a
    /// fabricated covariance result. Feed a minimal valid plot-option deck and
    /// assert the exact tag. Result (2026-07-15): `NotPorted("covr::run")`.
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
            Err(NjoyError::NotPorted(tag)) => assert_eq!(tag, "covr::run"),
            other => panic!("expected NotPorted(\"covr::run\"), got {other:?}"),
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
