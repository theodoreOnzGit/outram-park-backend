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

//! Finite-volume mesh: the `FvMesh` topology and geometry (cell volumes, face
//! areas/normals, owner/neighbour addressing, boundary patches), the
//! `RegionInterface` linking coupled regions, and the `MeshError` type. Mirrors
//! OpenFOAM's `fvMesh`/`polyMesh` (built here in-memory rather than read from a
//! `polyMesh` directory).

// DEDUPED 2026-10-03 (GitHub #492): this module's `fv_mesh` and `error` modules were a copy of
// `outram-foam-basic-lib`'s, code-identical apart from doc comments (or, for
// `FvMesh`/`MeshError`, a strict subset of it: foam-basic-lib adds the
// cyclic/AMI fields and checks, which this crate never sets, so every mesh
// built here behaves identically). foam-basic-lib is the one copy; these are
// re-exports. Merging `FvMesh` alone was not possible: its fields are
// foam-basic-lib `Vector3`s, so the primitives had to come with it.
pub(crate) use outram_foam_basic_lib::mesh::error;
/// Mesh topology (owner/neighbour, boundary patches) and geometry (cell
/// volumes, centres, face areas/normals) — see [`fv_mesh::FvMesh`]. A
/// re-export of `outram_foam_basic_lib::mesh::fv_mesh`.
pub use outram_foam_basic_lib::mesh::fv_mesh;
pub mod region_interface;

pub use error::MeshError;
pub use fv_mesh::*;
pub use region_interface::RegionInterface;
