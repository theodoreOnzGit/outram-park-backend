// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
// Derived from OpenFOAM (www.openfoam.com)
// Copyright (C) 2004-2023 OpenFOAM Foundation
// Copyright (C) 2016-2023 OpenCFD Ltd.
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

//! Finite-volume mesh layer: topology and geometry.
//!
//! This module holds the flat, cache-friendly mesh representation the FV
//! operators run on. It contains:
//!
//! - [`FvMesh`] — the mesh itself (cells, faces, owner/neighbour connectivity,
//!   cell volumes [m³], face-area vectors [m²], and cell/face centres [m]),
//!   plus [`FvMeshBuilder`] to assemble one incrementally.
//! - [`BoundaryPatch`] / [`PatchKind`] — boundary-patch descriptors.
//! - [`RegionInterface`] — a face-to-face coupling map between two regions'
//!   patches (used by conjugate-heat-transfer solvers).
//! - [`MeshError`] — the errors raised during mesh construction and validation.
//!
//! It stores only the data required by the operators; the OpenFOAM
//! `polyMesh → primitiveMesh → lduMesh` inheritance chain is not reproduced.

pub mod error;
pub mod fv_mesh;
pub mod region_interface;

pub use error::MeshError;
pub use fv_mesh::*;
pub use region_interface::RegionInterface;
