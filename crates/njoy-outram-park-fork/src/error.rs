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

    /// An argument a caller passed is outside what the routine accepts: a
    /// negative or non-finite temperature, a tape holding no material, a
    /// setting of zero where a count is needed. Added 2026-10-14 with
    /// [`crate::process`] (GitHub #871); the message names the argument.
    #[error("invalid input: {0}")]
    InvalidInput(String),

    /// A numerical routine failed to converge within its iteration budget.
    #[error("numerical routine '{routine}' did not converge")]
    NotConvergent { routine: &'static str },

    /// A feature exists in upstream NJOY2016 but has not been ported yet.
    #[error("not yet ported from NJOY2016: {0}")]
    NotPorted(&'static str),

    /// A thermal-scattering evaluation was queried at a temperature outside its
    /// tabulated range (beyond the NJOY `rdelas` matching tolerance of
    /// `T/1000 + 5` K), which would require extrapolating the scattering law.
    /// All temperatures are in kelvin. The caller must either request a
    /// temperature inside `[min_k, max_k]` (interpolation is supported there)
    /// or use an evaluation that tabulates the desired temperature — this
    /// error is never silently downgraded to a nearest-temperature snap.
    #[error(
        "requested thermal-scattering temperature {requested_k} K is outside the \
         evaluation's tabulated range {min_k}–{max_k} K (extrapolation refused)"
    )]
    TemperatureOutOfRange {
        /// The temperature the caller asked for \[K\].
        requested_k: f64,
        /// Lowest tabulated temperature \[K\].
        min_k: f64,
        /// Highest tabulated temperature \[K\].
        max_k: f64,
    },

    /// THERMR's `calcem` found no tabulated S(α,β) temperature within
    /// `T/500` of the request, and stops: "desired temperature not found"
    /// (`thermr.f90:1720-1733`). NJOY2016 never interpolates S(α,β) in
    /// temperature, so the NJOY-deck path
    /// ([`crate::acer::AceTable::thermal_from_mf7_on_pendf`]) refuses where it
    /// does (GitHub #866). All temperatures in kelvin.
    #[error(
        "desired temperature not found: no S(alpha,beta) temperature on the \
         tape within T/500 of {requested_k} K (tabulated: {tabulated_k:?}); \
         NJOY2016's calcem stops here"
    )]
    TemperatureNotTabulated {
        /// The temperature the caller asked for \[K\].
        requested_k: f64,
        /// The tape's MF=7/MT=4 temperatures, in tape order \[K\].
        tabulated_k: Vec<f64>,
    },

    /// The S(α,β) handed to the NJOY-deck thermal path was resolved at a
    /// different temperature from the one `calcem` would read for the
    /// requested temperature: for example `parse_mf7` (base `T₀`) paired with
    /// a request for another tabulated temperature, or an interpolated table.
    /// Parse the tape with `parse_mf7_at_temperature(.., Some(requested_k))`.
    #[error(
        "S(alpha,beta) was resolved at {resolved_k} K (interpolated: \
         {interpolated}), but calcem reads the {calcem_k} K table for a \
         request of {requested_k} K"
    )]
    ThermalTemperatureMismatch {
        /// The temperature the caller asked for \[K\].
        requested_k: f64,
        /// The tabulated temperature `calcem` selects for it \[K\].
        calcem_k: f64,
        /// The temperature the S(α,β) it was given represents \[K\].
        resolved_k: f64,
        /// Whether that S(α,β) was interpolated between two temperatures.
        interpolated: bool,
    },

    /// A Windowed-Multipole data file (MIT CRPG `WMP_Library` HDF5, or an
    /// embedded blob) was missing a dataset, had an unexpected shape, or could
    /// not be decoded.
    #[error("WMP data error: {0}")]
    WmpData(String),

    /// Underlying I/O failure reading or writing a tape file.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// An HDF5 interchange file could not be written, or the data handed to
    /// the writer was inconsistent (a cross-section vector whose length does
    /// not match the group structure, an empty library).
    ///
    /// Added 2026-09-22 with the write path (GitHub #270). Reading failures
    /// keep their own variants -- [`Self::WmpData`] for the WMP library -- so a
    /// read fault and a write fault stay distinguishable.
    #[error("HDF5 error: {0}")]
    Hdf5(String),

    /// A HIGH-tier data download or its integrity check failed: HTTP error,
    /// network failure, checksum mismatch, or a corrupt cached artifact. Only
    /// produced by the `net-fetch` feature ([`crate::acquire`]).
    #[error("data download error: {0}")]
    Download(String),
}
