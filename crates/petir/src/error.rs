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

use core::fmt;

/// The crate's result alias.
pub type Result<T> = core::result::Result<T, PetirError>;

/// What went wrong.
///
/// Variants correspond to the GSL codes named in each doc line
/// (`gsl_errno.h`); the mapping is deliberate so ported routines can keep
/// upstream's error paths visible rather than collapsing them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum PetirError {
    /// `GSL_EDOM` — input domain error, e.g. `a >= b` for an interval, or an
    /// evaluation point outside the interval a fit was built on.
    Domain,
    /// `GSL_EINVAL` — invalid argument that is not a domain question, e.g. a
    /// requested order larger than the number of coefficients held.
    Invalid,
    /// `GSL_ENOMEM` — allocation failed.
    NoMemory,
    /// `GSL_EMAXITER` — an iteration limit was reached before convergence.
    MaxIterations,
    /// `GSL_ETOL` — the requested tolerance could not be reached.
    Tolerance,
    /// `GSL_EZERODIV` — division by zero.
    ZeroDivide,
}

impl PetirError {
    /// The upstream GSL symbol this variant stands in for, for error messages
    /// and for tracing a ported routine back to its source.
    pub const fn gsl_code(self) -> &'static str {
        match self {
            PetirError::Domain => "GSL_EDOM",
            PetirError::Invalid => "GSL_EINVAL",
            PetirError::NoMemory => "GSL_ENOMEM",
            PetirError::MaxIterations => "GSL_EMAXITER",
            PetirError::Tolerance => "GSL_ETOL",
            PetirError::ZeroDivide => "GSL_EZERODIV",
        }
    }
}

impl fmt::Display for PetirError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let what = match self {
            PetirError::Domain => "input domain error",
            PetirError::Invalid => "invalid argument",
            PetirError::NoMemory => "allocation failed",
            PetirError::MaxIterations => "iteration limit reached before convergence",
            PetirError::Tolerance => "requested tolerance could not be reached",
            PetirError::ZeroDivide => "division by zero",
        };
        write!(f, "{what} ({})", self.gsl_code())
    }
}

#[cfg(feature = "std")]
impl std::error::Error for PetirError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_variant_names_its_gsl_code() {
        for e in [
            PetirError::Domain,
            PetirError::Invalid,
            PetirError::NoMemory,
            PetirError::MaxIterations,
            PetirError::Tolerance,
            PetirError::ZeroDivide,
        ] {
            assert!(e.gsl_code().starts_with("GSL_E"), "{e:?}");
        }
    }
}
