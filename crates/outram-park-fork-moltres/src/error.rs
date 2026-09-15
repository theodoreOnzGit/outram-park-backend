// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of OUTRAM PARK.
//
// OUTRAM PARK is free software: you can redistribute it and/or modify it
// under the terms of the GNU General Public License as published by the
// Free Software Foundation, either version 3 of the License, or (at your
// option) any later version.
//
// OUTRAM PARK is distributed in the hope that it will be useful, but
// WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the GNU
// General Public License for more details.
//
// You should have received a copy of the GNU General Public License along
// with OUTRAM PARK.  If not, see <https://www.gnu.org/licenses/>.

//! Error type shared by every solver in this crate.

use outram_foam_basic_lib::prelude::MeshError;
use thiserror::Error;

/// Everything that can go wrong while building or running an MSR model.
///
/// Construction errors (`InvalidMaterial`, `SizeMismatch`, `InvalidMesh`)
/// indicate a caller mistake and are raised before any physics runs;
/// runtime errors (`NoFissionSource`, `NotConverged`, `LinearSolveFailed`)
/// indicate the configured problem has no computable answer within the
/// requested tolerances.
#[derive(Debug, Error)]
pub enum MoltresError {
    /// A material record is internally inconsistent (wrong vector length,
    /// negative cross section, fission spectrum not normalised, ...).
    #[error("invalid material data: {0}")]
    InvalidMaterial(String),

    /// Two coupled arrays that must agree in length do not.
    #[error("size mismatch for {what}: expected {expected}, got {got}")]
    SizeMismatch {
        /// What was being checked (e.g. `"zone_of_cell"`).
        what: &'static str,
        /// The length the mesh / group structure requires.
        expected: usize,
        /// The length actually supplied.
        got: usize,
    },

    /// The finite-volume mesh failed validation (from `outram-foam-basic-lib`).
    #[error("mesh error: {0}")]
    InvalidMesh(#[from] MeshError),

    /// The initial flux produces zero fission neutrons, so a k-eigenvalue is
    /// undefined (non-multiplying configuration).
    #[error("no fission source: the configuration is non-multiplying, k_eff undefined")]
    NoFissionSource,

    /// The outer (power) iteration exhausted its iteration budget.
    #[error(
        "outer iteration not converged after {outer_iterations} iterations \
         (k residual {k_residual:.3e}, flux residual {flux_residual:.3e})"
    )]
    NotConverged {
        /// Outer iterations performed before giving up.
        outer_iterations: usize,
        /// Last relative change in `k_eff` (dimensionless).
        k_residual: f64,
        /// Last relative L2 change in the flux (dimensionless).
        flux_residual: f64,
    },

    /// An inner linear solve failed to reach its tolerance.
    #[error("linear solve for {field} not converged: residual {residual:.3e} after {iterations} iterations")]
    LinearSolveFailed {
        /// Name of the field being solved (e.g. `"precursor2"`).
        field: String,
        /// Final normalised residual (dimensionless).
        residual: f64,
        /// Iterations performed.
        iterations: usize,
    },
}
