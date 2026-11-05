// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Ported from cfMesh (Creative Fields, Ltd.; Franjo Juretic), GPL-3.0:
//   meshLibrary/utilities/surfaceTools/meshSurfaceEdgeExtractorFUN/
//     meshSurfaceEdgeExtractorFUN.{C,H}
//     meshSurfaceEdgeExtractorFUNDistributeFaces.C
// Source: the cfMesh v1.1 line as carried by github.com/DAFoam/cfmesh, commit
// e944f6ba695fcefce73ad306074f6b3c0ea432ae.
// Copyright (C) Creative Fields, Ltd. Licence: GPL-3.0-or-later (cfMesh files
// say "either version 3 of the License, or (at your option) any later
// version"), conveyed here under GPL-3.0-only.
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

//! cfMesh's **`meshSurfaceEdgeExtractorFUN`** (gh:#1269): the
//! `edgeExtraction` step of the `pMesh` workflow
//! (`voronoiMeshGenerator::mapEdgesAndCorners`, `createWrapperSheet =
//! false`). Its constructor runs:
//!
//! 1. `createBasicFundamentalSheets`: `createFundamentalSheetsJFS`, a layer
//!    of cells along the patch borders. **Not ported**: it is built on
//!    `extrudeLayer` (1,915 lines, item 26 of `docs/upstream_coverage.md`),
//!    which is not ported. Reported in [`FunReport::sheets_not_created`].
//! 2. `smoothMeshSurface`: `meshSurfaceOptimizer(mse, octree)
//!    .optimizeSurface()` (5 iterations):
//!    [`crate::surface_optimizer::optimise_mesh_surface`].
//! 3. `remapBoundaryPoints`: `meshSurfaceMapper(mse, octree)
//!    .mapVerticesOntoSurfacePatches()`:
//!    [`crate::surface_mapper::map_vertices_onto_surface_patches_given`].
//!
//! Both use the mesh's own patches (`edgeExtractor`'s, in `pMesh`).
//! `distributeBoundaryFaces` (nearest patch per face centre),
//! `improveQualityOfFundamentalSheets`, `reviseCorners` and `reviseEdges`
//! have no caller (the last two are empty upstream); not ported.

use crate::math::Vec3;
use crate::patches::SurfaceRegions;
use crate::surface_mapper::map_vertices_onto_surface_patches_given;
use crate::surface_optimizer::{optimise_mesh_surface, UntangleReport};
use crate::volume_mesh::VolumeMesh;

/// What [`mesh_surface_edge_extractor_fun`] did.
#[derive(Debug, Clone, PartialEq)]
pub struct FunReport {
    /// Always `true`: `createFundamentalSheetsJFS` is not ported.
    pub sheets_not_created: bool,
    /// `smoothMeshSurface`'s `optimizeSurface`.
    pub smoothing: UntangleReport,
}

/// `meshSurfaceEdgeExtractorFUN(mesh, octree, false)` on `mesh`, whose
/// boundary face `f` is in surface region `face_region[f]` (`None` inside),
/// with the sheet step missing (module docs).
pub fn mesh_surface_edge_extractor_fun(mesh: &mut VolumeMesh, points: &[Vec3], tris: &[[usize; 3]], regions: &SurfaceRegions, face_region: &[Option<usize>]) -> FunReport {
    let smoothing = optimise_mesh_surface(mesh, points, tris, regions, face_region);
    *mesh = map_vertices_onto_surface_patches_given(mesh, points, tris, regions, face_region);
    FunReport { sheets_not_created: true, smoothing }
}
