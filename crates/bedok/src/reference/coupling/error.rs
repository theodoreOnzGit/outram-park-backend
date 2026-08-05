//! Failures the coupled drivers can report.
//!
//! # Why this is not [`crate::BedokError`]
//!
//! The crate-level error enum does not yet carry the sparse-linear-algebra and
//! coupled-convergence cases the drivers need, and the reference translation is
//! being written by several hands at once, so this module keeps its additions
//! local rather than racing another author for `src/error.rs`. Merging
//! [`CouplingError`] into [`crate::BedokError`] is a one-way conversion away
//! (every variant here is either new or a wrapper) and is a deliberate
//! follow-up, not an oversight.
//!
//! # Provenance
//!
//! Support code for the translation of Than Yan Ren's (SNRSI) BEDOK MATLAB
//! snapshot (`BEDOKfiles.zip`, sha256 `e45cd6f57be2087c…`). The MATLAB has no
//! error type: it calls `error()` / `warning()` inline (`pauseonnan.m`,
//! `criticalboron_xyz:badeig`, `thdiffusion_solvertimexyz:diverged`). Each
//! variant below names the MATLAB identifier it stands in for.

use thiserror::Error;

use crate::error::BedokError;

/// Result alias for the coupled drivers.
pub type Result<T> = std::result::Result<T, CouplingError>;

/// Everything the coupled neutronics/thermal-hydraulics drivers can fail on.
#[derive(Debug, Error)]
pub enum CouplingError {
    /// A sparse operator could not be assembled — shape mismatch between terms,
    /// index overflow, or allocation failure.
    #[error("sparse assembly failed: {reason}")]
    SparseAssembly {
        /// What went wrong.
        reason: String,
    },

    /// The sparse LU factorisation failed: the operator is structurally or
    /// numerically singular.
    ///
    /// In BEDOK this normally means the diffusion operator has an empty row —
    /// an all-void plane, or a `whichsigma` map that disagrees with the
    /// geometry.
    #[error("sparse LU factorisation failed (singular operator): {reason}")]
    Singular {
        /// What the factorisation reported.
        reason: String,
    },

    /// A field held a NaN or a complex value where the MATLAB `pauseonnan.m`
    /// would have raised `'NaN occured'`.
    #[error("{field} contains NaN (MATLAB pauseonnan)")]
    NotANumber {
        /// Which field tripped the guard.
        field: &'static str,
    },

    /// An eigenvalue came back outside the physically sane band the critical
    /// boron search insists on — MATLAB `criticalboron_xyz:badeig` and
    /// `criticalboron_xyz:badboot`.
    #[error("eigenvalue out of sane range (k_eff = {k_eff} at {boron} ppm boron)")]
    EigenvalueOutOfRange {
        /// The offending eigenvalue \[-\].
        k_eff: f64,
        /// Boron concentration it was computed at \[ppm\].
        boron: f64,
    },

    /// The transient case supplied neither an end time nor a time grid —
    /// MATLAB `thdiffusion_solvertimexyz:notimedata`.
    #[error("params.tend and/or params.tgrid must be set by the geometry case")]
    NoTimeData,

    /// Something the case must supply was missing.
    #[error("missing case data: {what}")]
    MissingCaseData {
        /// Which field.
        what: &'static str,
    },

    /// A crate-level failure — grid indexing, fixture loading, I/O.
    #[error(transparent)]
    Bedok(#[from] BedokError),

    /// Writing a CSV output failed.
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}
