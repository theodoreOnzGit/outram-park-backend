// PROVENANCE
//   Upstream project: CYCLUS <https://github.com/cyclus/cyclus>
//   Upstream file:    src/error.h, src/error.cc
//   Upstream commit:  d4faab7ce0566ccb8febfcf50915cdeedee59db0
//   Upstream licence: BSD-3-Clause
//   Copyright (c) 2010-2016, University of Wisconsin Computational Nuclear
//   Engineering Research Group. All rights reserved.
//
//   This file is an independent Rust translation, (C) 2026 Theodore Ong and the
//   outram-park contributors, distributed under GPL-3.0-only. BSD-3-Clause is
//   GPLv3-compatible; the relicensing is ONE-WAY.

//! Error taxonomy, translated from Cyclus's exception hierarchy.
//!
//! # Why an enum and not an exception hierarchy
//!
//! Upstream defines `cyclus::Error` and derives `ValueError`, `KeyError`,
//! `StateError`, `IOError` and `ValidationError` from it, then throws them.
//! Rust has no exceptions and this workspace forbids trait objects, so the
//! hierarchy collapses into one enum whose variants carry the same meanings.
//! The mapping is exact and one-to-one, so a reader can open `error.h` next to
//! this file:
//!
//! | Upstream C++ | Here |
//! |---|---|
//! | `cyclus::Error` | [`CyclusError::Generic`] |
//! | `cyclus::ValueError` | [`CyclusError::Value`] |
//! | `cyclus::KeyError` | [`CyclusError::Key`] |
//! | `cyclus::StateError` | [`CyclusError::State`] |
//! | `cyclus::IOError` | [`CyclusError::Io`] |
//! | `cyclus::ValidationError` | [`CyclusError::Validation`] |
//!
//! One variant has no upstream counterpart: [`CyclusError::Numeric`], which
//! carries a [`petir::PetirError`] out of a numerical kernel. Upstream has
//! nowhere for this to come from because its linear algebra throws
//! `cyclus::Error` directly; here the numerics live in a separate crate with
//! its own error type, and swallowing it would lose the diagnosis.

use core::fmt;

use petir::PetirError;

/// The result type used throughout this crate.
pub type Result<T> = core::result::Result<T, CyclusError>;

/// Every way a Cyclus kernel operation can fail.
///
/// Each variant carries a `&'static str` context rather than a formatted
/// `String`. That is a deliberate `no_std` choice: an allocation on the error
/// path is the one allocation a constrained target can least afford, and the
/// variant plus the static context has in practice been enough to locate the
/// fault. Where a number genuinely identifies the fault — an invalid nuclide
/// id, say — the variant carries it as a field instead of formatting it.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum CyclusError {
    /// Upstream `cyclus::Error`: a generic failure with no better category.
    Generic(&'static str),

    /// Upstream `cyclus::ValueError`: a value was outside its allowed range.
    ///
    /// This is by far the most common failure in the resource layer — a
    /// negative quantity, an extraction larger than the inventory, an
    /// enrichment assay outside `(0, 1)`.
    Value(&'static str),

    /// Upstream `cyclus::KeyError`: a lookup by name or id found nothing.
    Key(&'static str),

    /// Upstream `cyclus::StateError`: an object was used in a state that does
    /// not permit the operation (an exchange node with no group, a facility
    /// asked to trade before it entered the simulation).
    State(&'static str),

    /// Upstream `cyclus::IOError`. Retained for translation fidelity; nothing
    /// in this `no_std` kernel performs I/O, so it is produced only by
    /// downstream crates that add a persistence layer.
    Io(&'static str),

    /// Upstream `cyclus::ValidationError`: input failed schema validation.
    Validation(&'static str),

    /// A nuclide id was not a well-formed `zzzaaammmm` identifier.
    ///
    /// Carries the offending id, because the id *is* the diagnosis here and a
    /// static string could not convey it.
    InvalidNuclide(i32),

    /// A numerical kernel in [`petir`] failed.
    Numeric(PetirError),
}

impl fmt::Display for CyclusError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Generic(m) => write!(f, "cyclus error: {m}"),
            Self::Value(m) => write!(f, "value error: {m}"),
            Self::Key(m) => write!(f, "key error: {m}"),
            Self::State(m) => write!(f, "state error: {m}"),
            Self::Io(m) => write!(f, "io error: {m}"),
            Self::Validation(m) => write!(f, "validation error: {m}"),
            Self::InvalidNuclide(id) => write!(f, "invalid nuclide id: {id}"),
            Self::Numeric(e) => write!(f, "numerics error: {e}"),
        }
    }
}

impl From<PetirError> for CyclusError {
    fn from(e: PetirError) -> Self {
        Self::Numeric(e)
    }
}

#[cfg(feature = "std")]
extern crate std;

#[cfg(feature = "std")]
impl std::error::Error for CyclusError {}
