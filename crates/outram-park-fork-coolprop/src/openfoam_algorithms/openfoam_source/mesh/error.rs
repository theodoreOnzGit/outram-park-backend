// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors

/// Errors produced by the mesh layer (mesh construction and validation).
#[derive(Debug, Clone, PartialEq)]
pub enum MeshError {
    /// An array field in the mesh has the wrong length.
    ///
    /// For example, `owner` must have length `n_faces`; `neighbour` must have
    /// length `n_internal_faces`; `cell_volumes` must have length `n_cells`.
    ArrayLengthMismatch {
        /// Name of the offending array (e.g. `"owner"`, `"cell_volumes"`).
        array: &'static str,
        expected: usize,
        got: usize,
    },

    /// A boundary patch does not start immediately after the previous one,
    /// leaving a gap or overlap in face coverage.
    PatchStartMismatch {
        /// Name of the offending patch.
        name: String,
        expected: usize,
        got: usize,
    },

    /// The sum of all patch sizes does not equal the number of boundary faces.
    PatchCoverageMismatch {
        /// Total face count covered by all patches.
        covered: usize,
        /// Total face count in the mesh.
        n_faces: usize,
    },

    /// `number_of_cells` was zero or negative when building a 1-D mesh.
    NonPositiveCellCount { got: i64 },
}

impl std::fmt::Display for MeshError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MeshError::ArrayLengthMismatch {
                array,
                expected,
                got,
            } => write!(
                f,
                "mesh array '{array}': expected length {expected}, got {got}"
            ),
            MeshError::PatchStartMismatch {
                name,
                expected,
                got,
            } => write!(
                f,
                "patch '{name}': expected to start at face {expected}, starts at {got}"
            ),
            MeshError::PatchCoverageMismatch { covered, n_faces } => write!(
                f,
                "boundary patches cover {covered} faces but n_faces = {n_faces}"
            ),
            MeshError::NonPositiveCellCount { got } => {
                write!(f, "number_of_cells must be ≥ 1, got {got}")
            }
        }
    }
}

impl std::error::Error for MeshError {}
