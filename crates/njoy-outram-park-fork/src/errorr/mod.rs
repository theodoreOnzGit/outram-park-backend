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
//! # Port status — PARTIAL (AI draft, untrusted until reviewed)
//!
//! This module is a work-in-progress port. What exists so far:
//!
//! - [`driver`] — the user-input **card deck** ([`ErrorrInput`] and its
//!   selector enums) plus a [`driver::run`] orchestration skeleton that
//!   documents the pipeline stages but returns
//!   [`crate::NjoyError::NotPorted`] for the numeric path.
//! - [`groups`] — the built-in **neutron group structures** (`egngpn`,
//!   `ign` selector): group-boundary tables.
//! - [`math`] — self-contained **numerical kernels** used by the
//!   resonance-parameter covariance path (`efacphi`, `efacts`, `eunfac`,
//!   `egnrl`, `cleb`, and small 3×3 matrix helpers).
//!
//! What is **not** yet ported: the covariance calculation kernels (`covcal`),
//! group averaging (`grpav`/`grpav4`), covariance collapse/output
//! (`colaps`/`covout`), and the resonance-parameter covariance chain
//! (`resprx`/`rpxsamm`/…). The end-to-end pipeline therefore returns
//! `NotPorted`. See `README.md` in this directory and the review manifest
//! under `docs/ai-fleet-review/op-cjw/` for the full gap list.

pub mod driver;
pub mod groups;
pub mod math;

pub use driver::ErrorrInput;

use crate::NjoyError;

/// Module-dispatch entry point used by the [`crate::NjoyModule`] registry.
///
/// ERRORR needs a full input deck ([`ErrorrInput`]) to run, so this
/// no-argument form exists only so the module registry can name ERRORR; it
/// reports that the end-to-end pipeline is not yet ported. For the real entry
/// point (once the numeric kernels land) use [`driver::run`] with an
/// [`ErrorrInput`].
pub fn run() -> Result<(), NjoyError> {
    Err(NjoyError::NotPorted("errorr"))
}
