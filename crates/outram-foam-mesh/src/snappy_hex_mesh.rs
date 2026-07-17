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

//! `snappyHexMesh` — automatic split-hex meshing around triangulated (STL)
//! surfaces.
//!
//! > **Provenance.** The three-phase structure, the split-hex hanging-node
//! > handling, and the control-parameter names re-implemented here are derived
//! > from OpenFOAM's `snappyHexMesh` utility
//! > (`src/mesh/snappyHexMesh`, © OpenFOAM Foundation / OpenCFD Ltd., GPL-3.0).
//! > This is an independent OUTRAM PARK re-implementation in Rust, not the
//! > official OpenFOAM software (see the crate-level notice and `TRADEMARKS.md`).
//!
//! Starting from a background hex mesh (see [`background`]; typically the output
//! of [`crate::block_mesh`]), `snappyHexMesh` runs three phases:
//!
//! 1. **Castellation** ([`castellation`], **implemented**) — octree cell
//!    refinement to the surface level, then removal of cells on the far side of
//!    the surface from a `keep_point`. Produces a valid, conforming refined
//!    [`FvMesh`](outram_foam_basic_lib::mesh::FvMesh).
//! 2. **Snapping** ([`snapping`], **scaffolded**) — morph the castellated
//!    boundary points onto the STL. The nearest-point projection primitive is
//!    implemented and tested; the quality-constrained relaxation solve is not.
//! 3. **Layer addition** ([`layers`], **scaffolded**) — insert graded prismatic
//!    boundary layers. The geometric grading arithmetic is implemented and
//!    tested; the topological extrusion is not.
//!
//! ## Status (bead op-ax7.2)
//!
//! | Phase | State | What works |
//! |---|---|---|
//! | STL input | ✅ done | ASCII + binary reader, inside/outside, nearest point |
//! | Castellation | ✅ done | octree refinement + region removal → valid `FvMesh` |
//! | Snapping | 🚧 stub | projection primitive done; morph solve `NotImplemented` |
//! | Layer addition | 🚧 stub | grading arithmetic done; extrusion `NotImplemented` |
//!
//! ## Minimal example
//!
//! ```no_run
//! use outram_foam_mesh::snappy_hex_mesh::{
//!     background::{BackgroundMesh, Bounds},
//!     castellation::{castellate, CastellationControls},
//!     stl::read_stl,
//! };
//! use outram_foam_basic_lib::primitives::Vector3;
//!
//! let surface = read_stl("sphere.stl").unwrap();
//! let (lo, hi) = surface.bounding_box().unwrap();
//! let domain = Bounds::new(lo, hi).expanded(0.5);
//! let background = BackgroundMesh::uniform(domain, 10, 10, 10);
//!
//! // Keep the region OUTSIDE the sphere (external-flow domain).
//! let keep_point = domain.min; // a far corner, outside the closed surface
//! let controls = CastellationControls::new(background, 2, keep_point);
//!
//! let castellated = castellate(&surface, &controls).unwrap();
//! println!("refined mesh has {} cells", castellated.n_cells());
//! ```

pub mod background;
pub mod castellation;
pub mod layers;
pub mod snapping;
pub mod stl;

// Re-export the primary entry points at the module root for discoverability.
pub use background::{BackgroundMesh, Bounds};
pub use castellation::{castellate, CastellatedMesh, CastellationControls, SurfaceFace};
pub use layers::{add_layers, layer_thicknesses, total_layer_thickness, LayerControls};
pub use snapping::{raw_surface_displacements, snap, SnapControls};
pub use stl::{read_stl, read_stl_ascii_str, read_stl_binary, read_stl_bytes, Triangle, TriangleSoup};
