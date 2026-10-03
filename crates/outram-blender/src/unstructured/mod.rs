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

//! # The neutral unstructured mesh — blender as the meshing nexus (GitHub #492)
//!
//! One mesh description, [`UnstructuredMesh`], that the three solver families
//! of this workspace are built from:
//!
//! - **finite volume** (`outram-foam-*`, GeN-Foam ports): cells are
//!   polyhedra bounded by faces with an owner and (internally) a neighbour,
//!   boundary faces grouped in typed patches — the OpenFOAM `polyMesh` model;
//! - **finite element** (`farrer-park`, later Moltres / OFFBEAT): cells are
//!   **typed** elements (`Tet4`, `Hex8`, `Tri3`, `Tri6`, `Quad4`) with nodes in
//!   a fixed local order;
//! - **Monte Carlo** (`outram-mc-libs`): the same cells are tally bins, through
//!   [`crate::spatial_mesh::MeshKind::Unstructured`], so MGXS bins and solver
//!   cells match one-to-one by construction (the #492 MGXS requirement).
//!
//! Every cell carries both views: its faces (always) and its element type and
//! ordered nodes (when it is a typed element; a cfMesh dual cell is a plain
//! [`ElementKind::Polyhedron`]). [`UnstructuredMesh::from_elements`] derives
//! the faces of an FE mesh; [`UnstructuredMesh::from_polyhedral`] recognises
//! tets and hexes in an FV mesh. Cell **zones** name material / solver regions
//! and patches name boundaries.
//!
//! ## What lives where
//!
//! | here (`outram_blender::unstructured`) | elsewhere |
//! |---|---|
//! | description: points, faces, cells, patches, zones, element types, units | — |
//! | geometry: face centres / area vectors, cell volumes / centroids (OpenFOAM `primitiveMesh` port), the centroid tetrahedral decomposition | — |
//! | a uniform-grid spatial index over cell bounding boxes ([`CellLocator`]) | — |
//! | the 1-D mesher ([`one_d::one_d_column`]) | — |
//! | converters and mesher orchestration ([`convert`], feature-gated) | the meshers themselves: `outram-foam-mesh` (blockMesh / snappy), `outram-park-fork-cfmesh`, `farrer-park`'s generators |
//! | drawing ([`plot`]) | — |
//! | — | **point location and track-length scoring**: `outram-mc-libs`' `UnstructuredMeshExt` (`tally::mesh`) |
//!
//! ## Units
//!
//! Coordinates are raw `f64` in the mesh's declared [`LengthUnit`] (the
//! documented raw-`f64` exception of `csg` / `spatial_mesh`), mirroring
//! OpenMC's unstructured-mesh `length_multiplier`. Converters scale to metres
//! for the FV / FE crates; Monte Carlo scales its cm positions into the mesh
//! unit.
//!
//! ## Status
//!
//! Untrusted AI-generated draft (2026-10-03), per the workspace
//! `RESPONSIBLE_USE.md`. Unit tests are written but **NOT YET RUN** (testing
//! deferred by the maintainer, 2026-10-03). For research, education and V&V
//! only.

pub mod build;
pub mod convert;
pub mod element;
pub mod geometry;
pub mod locator;
pub mod mesh;
pub mod one_d;
pub mod plot;

pub use build::DEFAULT_PATCH;
pub use convert::ConvertError;
pub use element::{BoundarySpec, Element, ElementKind, LengthUnit, Patch, PatchKind, PolyFace, Zone};
pub use locator::CellLocator;
pub use mesh::{UnstructuredMesh, UnstructuredMeshError};
pub use plot::{render_mesh_slice, render_mesh_slice_annotated, MeshColourBy, MeshSlice};
