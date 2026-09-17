//! Crate error type.
//!
//! Every fallible operation in this crate returns [`MpiResult`], whose error
//! variant is [`MpiError`]. The variants mirror the failure conditions the MPI
//! standard reports through error classes (`MPI_ERR_RANK`, `MPI_ERR_TRUNCATE`,
//! `MPI_ERR_TYPE`, `MPI_ERR_ARG`), adapted to Rust's `Result` idiom rather than
//! MPI's integer error codes + error handlers.

use crate::datatype::Datatype;

/// An error from an MPI-subset operation.
///
/// These correspond to the MPI standard's error classes but are surfaced as a
/// typed `Result` error rather than an integer code:
/// - [`MpiError::InvalidRank`] ↔ `MPI_ERR_RANK`
/// - [`MpiError::Truncated`] ↔ `MPI_ERR_TRUNCATE`
/// - [`MpiError::TypeMismatch`] ↔ `MPI_ERR_TYPE`
/// - [`MpiError::InvalidArgument`] ↔ `MPI_ERR_ARG` / `MPI_ERR_COUNT`
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum MpiError {
    /// A rank argument was outside `0..size` for the communicator it was used with.
    #[error("invalid rank {rank} for a communicator of size {size}")]
    InvalidRank {
        /// The offending rank value.
        rank: i32,
        /// The communicator size it was checked against.
        size: i32,
    },

    /// A receive buffer was smaller than the message that arrived (the message
    /// would be truncated). Mirrors `MPI_ERR_TRUNCATE`.
    #[error("message truncated: receive buffer holds {buffer} elements but {message} arrived")]
    Truncated {
        /// Capacity of the receive buffer, in elements.
        buffer: usize,
        /// Number of elements in the incoming message.
        message: usize,
    },

    /// The datatype of a received message did not match the datatype the receiver
    /// asked to decode it as. Mirrors `MPI_ERR_TYPE`.
    #[error("datatype mismatch: receiver expected {expected:?} but the message was {actual:?}")]
    TypeMismatch {
        /// The datatype the receiver requested.
        expected: Datatype,
        /// The datatype the sender used.
        actual: Datatype,
    },

    /// A generic invalid argument (bad count, mismatched root, empty operation, …).
    /// Mirrors `MPI_ERR_ARG` / `MPI_ERR_COUNT`.
    #[error("invalid argument: {0}")]
    InvalidArgument(String),

    /// The shared-memory transport failed (e.g. a rank thread panicked while a
    /// peer was blocked waiting on it).
    #[error("transport error: {0}")]
    Transport(String),
}

/// Convenience `Result` alias for this crate's fallible operations.
pub type MpiResult<T> = core::result::Result<T, MpiError>;
