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

//! Finite-volume mesh topology and geometry ([`fv_mesh::FvMesh`]), plus
//! multi-region coupling ([`region_interface::RegionInterface`], e.g. for
//! conjugate heat transfer between a fluid and a solid region).

pub(crate) mod error;
/// Mesh topology (owner/neighbour, boundary patches) and geometry (cell
/// volumes, centres, face areas/normals) — see [`fv_mesh::FvMesh`].
pub mod fv_mesh;
/// Region-to-region coupling (e.g. CHT fluid/solid interfaces) — see
/// [`region_interface::RegionInterface`].
pub mod region_interface;

pub use error::MeshError;
pub use fv_mesh::*;
pub use region_interface::RegionInterface;
