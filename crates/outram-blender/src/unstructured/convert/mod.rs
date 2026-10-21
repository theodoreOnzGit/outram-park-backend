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

//! **Converters and mesher orchestration**: one module per solver family,
//! each behind the cargo feature that pulls that solver crate.
//!
//! | module | feature | to | from | meshers driven |
//! |---|---|---|---|---|
//! | `foam` | `foam-export` | `outram-foam-basic-lib` `PolyMesh`, `FvMesh` | `PolyMesh` | — |
//! | `block_mesh` | `block-mesh` | — | `outram-foam-mesh` `blockMesh`, `snappyHexMesh` driver | `block_mesh`, `snappy_from_surface` |
//! | `cfmesh` | `foam-mesh` | — | `outram-park-fork-cfmesh` `VolumeMesh` | `tet_dual` (tet -> dual -> layers) |
//! | `fem` | `fem-export` | `farrer-park` `Mesh` | `farrer-park` `Mesh` | `FemGenerator` (farrer-park's own generators, which stay there) |
//!
//! The 1-D mesher is in the core ([`super::one_d`]), because its points are
//! not recoverable from the `FvMesh` the FV crate's version returns.
//!
//! **Direction rule (GitHub #486, #492).** The solver crates never depend on
//! this one; this crate depends on them, optionally. No converter targets
//! `outram-mc-libs`: Monte Carlo takes the neutral mesh directly through
//! `spatial_mesh::MeshKind::Unstructured`.
//!
//! There is no `FvMesh -> UnstructuredMesh` converter, deliberately: an
//! `FvMesh` holds no points, so there is nothing to build vertices from. Go
//! through the `PolyMesh` it was built from.

use super::mesh::UnstructuredMeshError;

#[cfg(feature = "foam-export")]
pub mod foam;

#[cfg(feature = "block-mesh")]
pub mod block_mesh;

#[cfg(feature = "foam-mesh")]
pub mod cfmesh;

#[cfg(feature = "fem-export")]
pub mod fem;

/// Errors from a converter or a mesher it drives.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum ConvertError {
    /// The target needs a mesh of another dimension.
    #[error("a {dim}-D mesh cannot be converted to {target}")]
    Dimension {
        /// The mesh's dimension.
        dim: usize,
        /// The target type.
        target: &'static str,
    },
    /// The target needs every cell to be one typed element kind.
    #[error("{target} needs one typed element kind for every cell; cell {cell} is {found}, cell 0 is {first}")]
    MixedElements {
        /// The target type.
        target: &'static str,
        /// The offending cell.
        cell: usize,
        /// Its kind.
        found: &'static str,
        /// The first cell's kind.
        first: &'static str,
    },
    /// A neutral-mesh construction error on the way in.
    #[error("neutral mesh: {0}")]
    Mesh(#[from] UnstructuredMeshError),
    /// The solver crate refused the result or the mesher failed; its message.
    #[error("{0}")]
    Solver(String),
}
