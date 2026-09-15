// Copyright (C) 2026 Theodore Ong and the outram-park contributors. GPL-3.0-only.
//
// Error codes mirror the GNU Scientific Library's `gsl_errno.h`
// (GPL-3.0-or-later, Copyright (C) 1996-2000 Gerard Jungman, Brian Gough),
// read at commit cf180cd7fbd06039a577f9c9ff0b428784765ac1. See NOTICE.

//! The crate's error type.
//!
//! # Why this exists rather than GSL's mechanism
//!
//! GSL reports errors by calling `gsl_error`, which invokes the installed
//! handler — and the **default handler aborts the process**
//! (`gsl_error.c`: `abort()`). That is unacceptable in a library, and doubly so
//! in `no_std`, where there may be no process to abort. PETIR returns
//! [`Result`] instead, and the variants below name the GSL code they stand in
//! for so a reader can follow a ported routine's error paths back to upstream.
//!
//! `thiserror` is not used: it requires `std`. [`Display`](core::fmt::Display)
//! is implemented by hand.
//!
//! # No `std` feature is needed for error interop
//!
//! [`core::error::Error`] has been stable since Rust 1.81 and is implemented
//! here unconditionally, so `?` and downstream `std`-based error handling work
//! without PETIR gaining a `std` feature at all. An earlier draft of this crate
//! carried one for exactly that purpose; it is gone because it would now be a
//! feature that does nothing, and a no-op feature is worse than no feature —
//! callers enable it expecting a behaviour change and get none.

use core::fmt;

/// The crate's result alias.
pub type Result<T> = core::result::Result<T, PetirError>;

/// What went wrong.
///
/// Variants correspond to the GSL codes named in each doc line
/// (`gsl_errno.h`); the mapping is deliberate so ported routines can keep
/// upstream's error paths visible rather than collapsing them.
///
/// `#[non_exhaustive]`: porting further GSL modules will add variants, and a
/// caller's `match` should not break when that happens.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum PetirError {
    /// `GSL_EDOM` — input domain error, e.g. `a >= b` for an interval, or an
    /// evaluation point outside the interval a fit was built on.
    Domain,
    /// `GSL_ERANGE` — the result is mathematically fine but not representable
    /// as an `f64`: the output overflowed or underflowed the exponent range.
    Range,
    /// `GSL_EINVAL` — invalid argument that is not a domain question, e.g. a
    /// requested order larger than the number of coefficients held.
    Invalid,
    /// `GSL_ENOMEM` — allocation failed.
    NoMemory,
    /// `GSL_EMAXITER` — an iteration limit was reached before convergence.
    ///
    /// Not necessarily a wrong answer: the current iterate may be perfectly
    /// serviceable, but the routine cannot vouch for the requested tolerance,
    /// so the caller decides.
    MaxIterations,
    /// `GSL_ETOL` — the requested tolerance could not be reached.
    Tolerance,
    /// `GSL_EZERODIV` — division by zero.
    ZeroDivide,
    /// `GSL_ESING` — a factorisation met an exactly vanishing pivot, so the
    /// matrix is singular.
    ///
    /// `col` is the zero-based column at which the pivot vanished.
    Singular {
        /// Zero-based column index of the vanishing pivot.
        col: usize,
    },
    /// `GSL_EBADLEN` — two arrays that must agree in length do not.
    ///
    /// Lengths are element counts, not byte counts.
    LengthMismatch {
        /// The length the routine required.
        expected: usize,
        /// The length it was given.
        found: usize,
    },
    /// `GSL_ENOTSQR` — a matrix that must be square is not.
    NotSquare {
        /// Row count of the offending matrix.
        rows: usize,
        /// Column count of the offending matrix.
        cols: usize,
    },
    /// `GSL_ELOSS` — loss of significance; cancellation left the result with no
    /// meaningful digits.
    LossOfAccuracy,
    /// `GSL_EROUND` — round-off error prevents the tolerance being reached,
    /// though the iteration is otherwise healthy.
    RoundOff,
    /// `GSL_EDIVERGE` — the iteration is diverging, or moving away from the
    /// solution.
    Diverged,
    /// `GSL_ENOPROG` — the iteration is no longer making progress toward a
    /// solution; the step has stagnated well short of the tolerance.
    NoProgress,
    /// `GSL_EUNIMPL` — the requested operation is not implemented in PETIR.
    ///
    /// Used where a ported routine deliberately covers a subset of upstream's
    /// generality; the calling routine's doc comment states what the subset is.
    Unimplemented,
}

impl PetirError {
    /// The upstream GSL symbol this variant stands in for, for error messages
    /// and for tracing a ported routine back to its source.
    ///
    /// The *symbol* rather than the integer, deliberately: a reader diffing a
    /// port against `gsl_errno.h` is looking at `return GSL_EMAXITER;`, not at
    /// the number 11, and the numbers are an implementation detail upstream is
    /// free to renumber.
    pub const fn gsl_code(self) -> &'static str {
        match self {
            PetirError::Domain => "GSL_EDOM",
            PetirError::Range => "GSL_ERANGE",
            PetirError::Invalid => "GSL_EINVAL",
            PetirError::NoMemory => "GSL_ENOMEM",
            PetirError::MaxIterations => "GSL_EMAXITER",
            PetirError::Tolerance => "GSL_ETOL",
            PetirError::ZeroDivide => "GSL_EZERODIV",
            PetirError::Singular { .. } => "GSL_ESING",
            PetirError::LengthMismatch { .. } => "GSL_EBADLEN",
            PetirError::NotSquare { .. } => "GSL_ENOTSQR",
            PetirError::LossOfAccuracy => "GSL_ELOSS",
            PetirError::RoundOff => "GSL_EROUND",
            PetirError::Diverged => "GSL_EDIVERGE",
            PetirError::NoProgress => "GSL_ENOPROG",
            PetirError::Unimplemented => "GSL_EUNIMPL",
        }
    }
}

impl fmt::Display for PetirError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PetirError::Domain => write!(f, "input domain error (GSL_EDOM)"),
            PetirError::Range => write!(f, "output range error (GSL_ERANGE)"),
            PetirError::Invalid => write!(f, "invalid argument (GSL_EINVAL)"),
            PetirError::NoMemory => write!(f, "allocation failed (GSL_ENOMEM)"),
            PetirError::MaxIterations => write!(
                f,
                "iteration limit reached before convergence (GSL_EMAXITER)"
            ),
            PetirError::Tolerance => write!(
                f,
                "requested tolerance could not be reached (GSL_ETOL)"
            ),
            PetirError::ZeroDivide => write!(f, "division by zero (GSL_EZERODIV)"),
            PetirError::Singular { col } => write!(
                f,
                "matrix is singular: zero pivot at column {col} (GSL_ESING)"
            ),
            PetirError::LengthMismatch { expected, found } => write!(
                f,
                "length mismatch: expected {expected}, found {found} (GSL_EBADLEN)"
            ),
            PetirError::NotSquare { rows, cols } => {
                write!(f, "matrix is not square: {rows}x{cols} (GSL_ENOTSQR)")
            }
            PetirError::LossOfAccuracy => write!(f, "loss of accuracy (GSL_ELOSS)"),
            PetirError::RoundOff => write!(
                f,
                "round-off prevents the tolerance being reached (GSL_EROUND)"
            ),
            PetirError::Diverged => write!(f, "iteration is diverging (GSL_EDIVERGE)"),
            PetirError::NoProgress => {
                write!(f, "iteration is no longer making progress (GSL_ENOPROG)")
            }
            PetirError::Unimplemented => write!(f, "not implemented in PETIR (GSL_EUNIMPL)"),
        }
    }
}

impl core::error::Error for PetirError {}

#[cfg(test)]
mod tests {
    use super::*;

    /// Guards the GSL symbol mapping against a silent edit: these names are the
    /// contract with `gsl_errno.h`, and a reviewer diffing a ported routine
    /// relies on them.
    #[test]
    fn gsl_symbols_match_upstream_gsl_errno_h() {
        assert_eq!(PetirError::Domain.gsl_code(), "GSL_EDOM");
        assert_eq!(PetirError::Range.gsl_code(), "GSL_ERANGE");
        assert_eq!(PetirError::Invalid.gsl_code(), "GSL_EINVAL");
        assert_eq!(PetirError::NoMemory.gsl_code(), "GSL_ENOMEM");
        assert_eq!(PetirError::MaxIterations.gsl_code(), "GSL_EMAXITER");
        assert_eq!(PetirError::ZeroDivide.gsl_code(), "GSL_EZERODIV");
        assert_eq!(PetirError::Singular { col: 0 }.gsl_code(), "GSL_ESING");
        assert_eq!(
            PetirError::NotSquare { rows: 1, cols: 2 }.gsl_code(),
            "GSL_ENOTSQR"
        );
    }

    #[test]
    fn display_reports_the_offending_index() {
        let msg = std::format!("{}", PetirError::Singular { col: 3 });
        assert!(msg.contains('3'), "message should name the column: {msg}");
        let msg = std::format!("{}", PetirError::LengthMismatch { expected: 7, found: 2 });
        assert!(msg.contains('7') && msg.contains('2'), "got: {msg}");
    }

    /// The error must compose with `?` in a `std` program without PETIR
    /// linking `std` itself.
    #[test]
    fn implements_the_core_error_trait() {
        fn as_error(e: PetirError) -> &'static str {
            let _: &dyn core::error::Error = &e;
            e.gsl_code()
        }
        assert_eq!(as_error(PetirError::Domain), "GSL_EDOM");
    }
}
