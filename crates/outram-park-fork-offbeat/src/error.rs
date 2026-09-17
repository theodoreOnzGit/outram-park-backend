// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from OFFBEAT (https://gitlab.com/foam-for-nuclear/offbeat),
// upstream commit 80e84450a115b0c411e1bfa5d166379f6bf6c084, GPL-3.0.
//
// This file is part of OUTRAM PARK. See `src/lib.rs` for the full licence
// notice.

//! Error type for the fuel-performance crate.

use thiserror::Error;

/// Everything that can go wrong in a fuel-performance calculation.
///
/// Fuel-performance runs fail in a small number of characteristic ways, and
/// each variant below names one of them so a caller can react rather than
/// merely log. Correlation ranges matter here: material correlations are fits
/// to experimental data over a stated range, and silently extrapolating one
/// (say, a UO2 conductivity fit far above its melting point) produces a number
/// that looks plausible and is meaningless. This crate reports that as
/// [`OffbeatError::OutOfRange`] rather than returning the extrapolated value.
#[derive(Debug, Error, Clone, PartialEq)]
pub enum OffbeatError {
    /// A material correlation was evaluated outside the range it was fitted
    /// over.
    ///
    /// `quantity` names what was being evaluated (e.g. `"UO2 conductivity"`),
    /// `value` is the offending input, `low`/`high` the validity bounds, and
    /// `unit` the SI unit the three numbers are expressed in.
    #[error("{quantity}: input {value} {unit} is outside the correlation's validity range [{low}, {high}] {unit}")]
    OutOfRange {
        /// What was being evaluated.
        quantity: &'static str,
        /// The offending input value.
        value: f64,
        /// Lower bound of validity.
        low: f64,
        /// Upper bound of validity.
        high: f64,
        /// SI unit of `value`, `low` and `high`.
        unit: &'static str,
    },

    /// A physically impossible input was supplied — a negative absolute
    /// temperature, a porosity outside `[0, 1]`, a negative density.
    #[error("unphysical input for {quantity}: {value} {unit} ({reason})")]
    Unphysical {
        /// What was being evaluated.
        quantity: &'static str,
        /// The offending input value.
        value: f64,
        /// SI unit of `value`.
        unit: &'static str,
        /// Why the value is impossible.
        reason: &'static str,
    },

    /// The mechanics solve did not converge within the iteration budget.
    ///
    /// `residual` is the final scaled residual and `tolerance` the target it
    /// failed to reach, after `iterations` outer iterations.
    #[error("mechanics solve failed to converge: residual {residual:.3e} > tolerance {tolerance:.3e} after {iterations} iterations")]
    MechanicsNotConverged {
        /// Final scaled residual.
        residual: f64,
        /// Convergence target.
        tolerance: f64,
        /// Outer iterations performed.
        iterations: usize,
    },

    /// The constitutive-law (stress) integration did not converge.
    ///
    /// Rate-dependent laws — creep especially — are integrated with a local
    /// Newton iteration per cell; this reports that iteration failing.
    #[error("constitutive integration failed to converge in cell {cell}: residual {residual:.3e} after {iterations} iterations")]
    ConstitutiveNotConverged {
        /// Index of the cell whose local iteration failed.
        cell: usize,
        /// Final local residual.
        residual: f64,
        /// Local iterations performed.
        iterations: usize,
    },

    /// A model was requested by name and no such model is registered.
    #[error("unknown {category} model: {name:?}")]
    UnknownModel {
        /// The family the model was looked up in (e.g. `"conductivity"`).
        category: &'static str,
        /// The name that was not found.
        name: String,
    },

    /// A mesh or field precondition was violated — mismatched sizes, a missing
    /// region, a zero-volume cell.
    #[error("mesh/field precondition violated: {0}")]
    Mesh(String),

    /// A model is declared but not yet implemented in this port.
    ///
    /// This is an honest placeholder, not a silent fallback: a caller reaching
    /// this has asked for physics the port does not yet contain.
    #[error("{0} is not yet implemented in this port")]
    NotImplemented(&'static str),
}

/// Convenience alias for fallible fuel-performance operations.
pub type Result<T> = core::result::Result<T, OffbeatError>;
