//! Crate-wide error type.
//!
//! NJOY's Fortran code aborts via the `error` subroutine (in `util.f90`) printing
//! a message and stopping. The Rust port replaces that with a recoverable
//! [`NjoyError`] returned through `Result`, so a caller (e.g. an ACE-generation
//! driver) can handle failures instead of taking down the process.

use thiserror::Error;

/// Errors raised while parsing ENDF data or running an NJOY module.
#[derive(Debug, Error)]
pub enum NjoyError {
    /// An ENDF tape was malformed or a record could not be parsed.
    #[error("ENDF parse error: {0}")]
    EndfParse(String),

    /// A requested material (MAT), file (MF), or section (MT) was not present.
    #[error("ENDF section not found: MAT={mat} MF={mf} MT={mt}")]
    SectionNotFound { mat: i32, mf: i32, mt: i32 },

    /// A numerical routine failed to converge within its iteration budget.
    #[error("numerical routine '{routine}' did not converge")]
    NotConvergent { routine: &'static str },

    /// A feature exists in upstream NJOY2016 but has not been ported yet.
    #[error("not yet ported from NJOY2016: {0}")]
    NotPorted(&'static str),

    /// A Windowed-Multipole data file (MIT CRPG `WMP_Library` HDF5, or an
    /// embedded blob) was missing a dataset, had an unexpected shape, or could
    /// not be decoded.
    #[error("WMP data error: {0}")]
    WmpData(String),

    /// Underlying I/O failure reading or writing a tape file.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// A HIGH-tier data download or its integrity check failed: HTTP error,
    /// network failure, checksum mismatch, or a corrupt cached artifact. Only
    /// produced by the `net-fetch` feature ([`crate::acquire`]).
    #[error("data download error: {0}")]
    Download(String),
}
