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

//! Snapping — Phase 2 of `snappyHexMesh` (**scaffolded, not yet implemented**).
//!
//! Snapping morphs the castellated staircase boundary onto the STL surface so
//! the mesh becomes body-fitted. OpenFOAM's algorithm
//! (`snappyHexMeshDriver::doSnap`) is, per iteration:
//!
//! 1. **Find patch points.** Collect the points of the carved surface patch
//!    (here [`CastellatedMesh::surface_faces`] gives the faces and their corner
//!    point indices into [`CastellatedMesh::points`]).
//! 2. **Compute displacements.** For each surface point, find the nearest point
//!    on the STL — [`Triangle::closest_point`] / [`TriangleSoup::nearest_point`]
//!    already do this exactly — giving a target position and thus a
//!    displacement vector.
//! 3. **Constrain + relax.** Feature points snap to feature edges/points; the
//!    raw displacement is smoothed and scaled back (`snapControls::nSmoothPatch`,
//!    `nSolveIter`) so interior mesh quality is preserved — this is the hard
//!    part, an iterative mesh-motion solve.
//! 4. **Mesh-quality gate.** Apply the displacement only where the resulting
//!    faces stay within the `meshQualityControls` limits (non-orthogonality,
//!    skewness, min volume); otherwise reduce it and retry.
//!
//! Because [`FvMesh`](outram_foam_basic_lib::mesh::FvMesh) stores only geometry
//! (centres, area vectors, volumes) and no point/face-vertex topology, snapping
//! needs the point-based view carried in [`CastellatedMesh`]. A full
//! implementation must (a) move points, (b) recompute face areas/centres and
//! cell centres/volumes from the moved points, and (c) rebuild the `FvMesh`.
//! Steps (2) — the projection primitive — is done; (1), (3), (4), and the
//! geometry rebuild are the remaining work.
//!
//! The geometric building block (nearest surface point) is exercised by the
//! test suite via the STL primitives; the driver below is an honest stub.

use outram_foam_basic_lib::primitives::Vector3;

use crate::snappy_hex_mesh::castellation::CastellatedMesh;
use crate::snappy_hex_mesh::stl::TriangleSoup;
use crate::MeshError;

/// Controls for the snapping phase (subset of `snapControls`).
#[derive(Debug, Clone)]
pub struct SnapControls {
    /// Number of point-displacement relaxation iterations.
    pub n_solve_iter: usize,
    /// Number of patch-smoothing iterations per solve step.
    pub n_smooth_patch: usize,
    /// Relative distance (in cell sizes) a point may travel to the surface.
    pub tolerance: f64,
}

impl Default for SnapControls {
    fn default() -> Self {
        Self {
            n_solve_iter: 30,
            n_smooth_patch: 3,
            tolerance: 2.0,
        }
    }
}

/// The displacement each surface point *would* move under one unconstrained
/// projection to the surface — the raw input to the (unimplemented) relaxation
/// solve. Provided so callers/tests can inspect the projection even though the
/// full morph is not yet applied.
///
/// `result[p]` is the vector from `mesh.points[p]` to its nearest surface point,
/// for every point that appears in a surface face; other points map to the zero
/// vector.
pub fn raw_surface_displacements(mesh: &CastellatedMesh, surface: &TriangleSoup) -> Vec<Vector3> {
    let mut disp = vec![Vector3::ZERO; mesh.points.len()];
    for face in &mesh.surface_faces {
        for &pid in &face.corners {
            if disp[pid] == Vector3::ZERO {
                if let Some(target) = surface.nearest_point(mesh.points[pid]) {
                    disp[pid] = target - mesh.points[pid];
                }
            }
        }
    }
    disp
}

/// Morph the castellated boundary onto the surface (**not yet implemented**).
///
/// Returns [`MeshError::NotImplemented`]. See the module docs for the algorithm;
/// [`raw_surface_displacements`] provides the projection step, but the
/// quality-constrained relaxation solve and the `FvMesh` geometry rebuild are
/// still to do.
pub fn snap(
    _mesh: &CastellatedMesh,
    _surface: &TriangleSoup,
    _controls: &SnapControls,
) -> Result<CastellatedMesh, MeshError> {
    Err(MeshError::NotImplemented(
        "snappyHexMesh snapping phase: point-displacement relaxation solve and \
         FvMesh geometry rebuild are scaffolded but not implemented (see \
         snapping module docs; raw_surface_displacements provides the projection step)"
            .to_string(),
    ))
}
