//! `ERRORR` — multigroup cross-section and distribution covariance matrices.
//!
//! Produces multigroup covariance matrices from ENDF File-3x covariance data
//! (MF=31/33/34/35/40): the group-averaged relative covariances of cross
//! sections, ν̄, angular distributions, and spectra that quantify nuclear-data
//! uncertainty for sensitivity/uncertainty (S/U) analysis. Output is a GENDF-like
//! covariance tape post-processed by COVR.
//!
//! **Upstream:** `errorr.f90` (~11.2k lines, NJOY2016 commit
//! `ac5adf5f33d893e42f2eed7fb286b0d51c7580da`). **Manual:** LA-UR-17-20093 §ERRORR.
//!
//! # Port status — MF=33 path PORTED AND VALIDATED; the rest PARTIAL
//!
//! [`run_mf33`] is the end-to-end MF=33 cross-section-covariance pipeline
//! (`gridd` → `egngpn`/`uniong` → `grpav` → `covcal` → `sigc`/`covout`),
//! validated element-for-element against NJOY2016 output tapes for nine
//! runs over eight materials (`tests/errorr_mf33_golden.rs`, worst
//! deviation at the tape's printing precision). Modules:
//!
//! - [`gridd`] — union energy grid, `merge`, derivation coefficients
//!   `akxy`/`ek`, lumped reactions (`gridd`/`merge`/`uniong`/`lumpmt`).
//! - [`weight`] — `egnwtf`/`egtwtf`/`egtflx`: the `iwt` menu with its
//!   panel stops, plus `endf.f90`'s `terpa`.
//! - [`grpav`] — `grpav`/`epanel`/`egtsig` and `gety1`: union-group cross
//!   sections and flux from the PENDF.
//! - [`covcal`] — `covcal`/`lumpxs`: the `LB` 0–6/8 kernels on the union
//!   grid.
//! - [`covout`] — `sigc`/`covout`: collapse to the user groups and the
//!   output-tape layout ([`ErrorrResult::to_tape`]).
//! - [`mf33`] — the orchestration ([`run_mf33`], [`Mf33Config`]).
//!
//! - [`resprx`] — the MF=32 resonance-parameter chain (`resprx`/`rpxlc12`/
//!   `rpxunr`/`rescon`, ERRORJ method) that `covout` folds in when the
//!   material carries MF=32; validated on TENDL-2023 Ar-37
//!   (`tests/errorr_mf32_ar37_golden.rs`).
//!
//! Still here from the first pass: [`driver`] (the full card deck as
//! [`ErrorrInput`]; its `run` stays `NotPorted` because it is keyed on tape
//! unit numbers, not tapes), [`groups`] (built-in group structures),
//! [`math`] (the `efacts`/`efacphi`/`eunfac`/`egnrl` kernels `resprx` uses),
//! [`covariance`] (the structural MF=31/33 reader).
//!
//! **Not ported** (refused with `NotPorted`, never approximated): the
//! MF=32 branches listed in [`resprx`] (`LRF=7` SAMM, `LCOMP=0`, `LRF=1/3`
//! sensitivities, INTG correlations, `irespr=0`), MF=31/34/35/40,
//! `iread` 1/2, `nstan`/`nin`, GENDF input (`colaps`), `covadd`, ENDF/B-IV.
//! See `README.md` in this directory.

pub mod covadd;
pub mod covariance;
pub mod covcal;
pub mod covout;
pub mod driver;
pub mod gridd;
pub mod groups;
pub mod grpav;
pub mod math;
pub mod mf33;
pub mod resprx;
pub mod weight;

pub use covariance::{
    detect_endf_version, read_covariance_section, CovarianceSection, CovarianceSubsection,
    NcSubsection, NiSubsection, SubsectionFormat,
};
pub use covout::{CoarseCovariance, CoarseGroupXs, ErrorrResult};
pub use driver::ErrorrInput;
pub use mf33::{run_mf33, Mf33Config};
pub use resprx::{resprx, ResonanceCovariance};
pub use weight::ErrorrWeight;

use crate::NjoyError;

/// Module-dispatch entry point used by the [`crate::NjoyModule`] registry.
///
/// ERRORR needs tapes and a deck to run, so this no-argument form exists
/// only so the module registry can name ERRORR. The real entry point is
/// [`run_mf33`] with an ENDF [`crate::endf::tape::Tape`], its PENDF and an
/// [`Mf33Config`].
pub fn run() -> Result<(), NjoyError> {
    Err(NjoyError::NotPorted("errorr"))
}
