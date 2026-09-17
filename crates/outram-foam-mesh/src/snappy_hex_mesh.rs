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
//!    [`FvMesh`](outram_foam_basic_lib::mesh::FvMesh) plus a point/face
//!    [`PolyPatchMesh`](poly_topology::PolyPatchMesh) the later phases move.
//! 2. **Snapping** ([`snapping`], **implemented**) — morph the castellated
//!    boundary onto the STL via nearest-point projection, Laplacian patch
//!    smoothing, and a quality-gated relaxation, then rebuild the `FvMesh`.
//!    Feature-edge snapping is a restricted (tested) addition.
//! 3. **Layer addition** ([`layers`], **implemented, restricted**) — insert
//!    graded prismatic boundary layers at the wall patch with expansion-ratio
//!    grading and quality-limited collapse. Two placements exist and the driver
//!    picks per case: the medial-axis **interior shrink-and-insert** (the real
//!    `snappyLayerDriver` behaviour, volume-conserving) wherever it stays
//!    watertight, and an **outward extrusion** fallback on octree hanging-node
//!    regions, which grows the domain. Which one you got is not something to
//!    assume — see the [`layers`] module docs, and prefer
//!    [`crate::driver::mesh_from_surface`], which measures it and reports a
//!    [`LayerOutcome`](crate::driver::LayerOutcome).
//!
//! Run all three together with [`generate`] (this module's top-level entry), or
//! call the phase functions individually. For a one-call path that also picks
//! the background mesh, converts to `PolyMesh` and grades the result, use
//! [`crate::driver::mesh_from_surface`] instead.
//!
//! ## Status (bead op-ax7.2)
//!
//! | Phase | State | What works |
//! |---|---|---|
//! | STL input | ✅ done | ASCII + binary reader, inside/outside, nearest point |
//! | Castellation | ✅ done | octree refinement + region removal → valid `FvMesh` + topology |
//! | Snapping | ✅ done | projection + smoothing + quality-gated morph + rebuild; feature-edge (restricted) |
//! | Layer addition | 🟡 restricted | graded prism insertion + collapse; medial-axis interior shrink-and-insert where watertight, outward-extrusion fallback elsewhere |
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
pub mod cyclic;
pub mod layers;
pub mod poly_topology;
pub mod snapping;
pub mod stl;

// Re-export the primary entry points at the module root for discoverability.
pub use background::{BackgroundMesh, Bounds};
pub use castellation::{castellate, CastellatedMesh, CastellationControls, SurfaceFace};
pub use cyclic::{
    check_conformity, resolve_pairs, CyclicError, CyclicPair, CyclicPointConstraints,
    DEFAULT_CYCLIC_TOL,
};
pub use layers::{add_layers, layer_thicknesses, total_layer_thickness, LayerControls};
pub use poly_topology::{face_area_and_centre, MeshQuality, PolyPatchMesh, QualityLimits};
pub use snapping::{raw_surface_displacements, snap, SnapControls};
pub use stl::{read_stl, read_stl_ascii_str, read_stl_binary, read_stl_bytes, Triangle, TriangleSoup};

use crate::MeshError;

/// Controls for the full three-phase `snappyHexMesh` pipeline.
///
/// Bundles the per-phase controls and lets the snapping and layer phases be
/// switched off individually (`None`), mirroring the `snap`/`addLayers` toggles
/// in a `snappyHexMeshDict`. Castellation always runs (it produces the mesh the
/// later phases refine).
#[derive(Debug, Clone)]
pub struct SnappyHexMeshControls {
    /// Phase 1 — octree refinement + region removal (always run).
    pub castellation: CastellationControls,
    /// Phase 2 — morph the boundary onto the surface. `None` skips snapping.
    pub snap: Option<SnapControls>,
    /// Phase 3 — insert graded boundary layers. `None` skips layer addition.
    pub layers: Option<LayerControls>,
}

impl SnappyHexMeshControls {
    /// Castellation-only controls (snapping and layers disabled).
    pub fn castellation_only(castellation: CastellationControls) -> Self {
        Self {
            castellation,
            snap: None,
            layers: None,
        }
    }

    /// Enable snapping with the given controls (builder style).
    pub fn with_snap(mut self, snap: SnapControls) -> Self {
        self.snap = Some(snap);
        self
    }

    /// Enable layer addition with the given controls (builder style).
    pub fn with_layers(mut self, layers: LayerControls) -> Self {
        self.layers = Some(layers);
        self
    }
}

/// Run the full `snappyHexMesh` pipeline: castellation → (snapping) → (layers).
///
/// Executes the enabled phases in order, threading the [`CastellatedMesh`]
/// (which carries the point/face [`PolyPatchMesh`] topology and the validated
/// [`FvMesh`](outram_foam_basic_lib::mesh::FvMesh)) from one phase to the next,
/// and returns the final mesh. Phases whose controls are `None` are skipped.
///
/// This is the single top-level entry point corresponding to running the
/// `snappyHexMesh` utility; the individual phase functions ([`castellate`],
/// [`snap`], [`add_layers`]) remain public for finer control.
///
/// # Errors
/// Propagates the first phase error — [`MeshError::Construction`] from
/// castellation (empty surface, all cells removed, invalid assembly) or from a
/// snapping/layer rebuild.
///
/// # Example
/// ```no_run
/// use outram_foam_mesh::snappy_hex_mesh::{
///     background::{BackgroundMesh, Bounds},
///     castellation::CastellationControls,
///     generate, SnappyHexMeshControls, SnapControls, LayerControls,
///     stl::read_stl,
/// };
///
/// let surface = read_stl("sphere.stl").unwrap();
/// let (lo, hi) = surface.bounding_box().unwrap();
/// let domain = Bounds::new(lo, hi).expanded(0.5);
/// let background = BackgroundMesh::uniform(domain, 10, 10, 10);
/// let controls = SnappyHexMeshControls::castellation_only(
///     CastellationControls::new(background, 2, domain.min),
/// )
/// .with_snap(SnapControls::default())
/// .with_layers(LayerControls::default());
///
/// let mesh = generate(&surface, &controls).unwrap();
/// println!("final mesh has {} cells", mesh.n_cells());
/// ```
pub fn generate(
    surface: &TriangleSoup,
    controls: &SnappyHexMeshControls,
) -> Result<CastellatedMesh, MeshError> {
    let mut mesh = castellate(surface, &controls.castellation)?;
    if let Some(snap_controls) = &controls.snap {
        mesh = snap(&mesh, surface, snap_controls)?;
    }
    if let Some(layer_controls) = &controls.layers {
        mesh = add_layers(&mesh, layer_controls)?;
    }
    Ok(mesh)
}
