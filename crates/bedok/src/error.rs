//! Error type for BEDOK.

use thiserror::Error;

/// Result alias used throughout the crate.
pub type Result<T> = std::result::Result<T, BedokError>;

/// Everything that can go wrong in a BEDOK solve or fixture load.
#[derive(Debug, Error)]
pub enum BedokError {
    /// A grid dimension was zero.
    #[error("grid has a zero dimension: {nx}x{ny}x{nz}, {ngroups} groups")]
    EmptyGrid {
        /// Nodes in x.
        nx: usize,
        /// Nodes in y.
        ny: usize,
        /// Nodes in z.
        nz: usize,
        /// Energy groups.
        ngroups: usize,
    },

    /// A flat state-vector index was outside the grid.
    #[error("state index {idx} out of range (length {len})")]
    IndexOutOfRange {
        /// The offending index.
        idx: usize,
        /// Valid length.
        len: usize,
    },

    /// A reference fixture could not be read or did not have the expected shape.
    #[error("fixture {path}: {reason}")]
    Fixture {
        /// Fixture path.
        path: String,
        /// What was wrong with it.
        reason: String,
    },

    /// An iterative solve failed to converge.
    #[error("{what} did not converge in {iterations} iterations (residual {residual:e})")]
    NotConverged {
        /// Which solve.
        what: &'static str,
        /// Iterations taken.
        iterations: usize,
        /// Residual reached.
        residual: f64,
    },

    /// Underlying I/O failure.
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}
