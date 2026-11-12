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

//! cfMesh's **`meshSurfaceEdgeExtractorFUN`** (gh:#1269, completed gh:#1281):
//! the `edgeExtraction` step of the `pMesh` workflow
//! (`voronoiMeshGenerator::mapEdgesAndCorners`, `createWrapperSheet =
//! false`). Its constructor runs:
//!
//! 1. `createBasicFundamentalSheets`: `createFundamentalSheetsJFS`, layers
//!    of cells that wrap each patch's cells
//!    ([`crate::fundamental_sheets`], on `extrudeLayer`,
//!    [`crate::layers::extrude_layer`]). ~~**Not ported**~~ **PORTED
//!    2026-11-05 (gh:#1280, #1281)**, and run by default;
//!    [`FunSheets::Without`] is the ablation (the gh:#1269 behaviour, kept
//!    for the differential). Upstream aborts on an `extrudeLayer`
//!    `FatalError`; here the error is reported in [`FunReport::sheets`] and
//!    the step goes on without the sheets.
//! 2. `smoothMeshSurface`: `meshSurfaceOptimizer(mse, octree)
//!    .optimizeSurface()` (5 iterations):
//!    [`crate::surface_optimizer::optimise_mesh_surface`].
//! 3. `remapBoundaryPoints`: `meshSurfaceMapper(mse, octree)
//!    .mapVerticesOntoSurfacePatches()`:
//!    [`crate::surface_mapper::map_vertices_onto_surface_patches_given`].
//!
//! All three use the mesh's own patches (`edgeExtractor`'s, in `pMesh`).
//! The sheets add boundary faces, so the per-face patches change: the new
//! list is returned. `distributeBoundaryFaces` (nearest patch per face
//! centre), `improveQualityOfFundamentalSheets`, `reviseCorners` and
//! `reviseEdges` have no caller (the last two are empty upstream); not
//! ported.

use crate::fundamental_sheets::{create_fundamental_sheets, SheetsKind, SheetsReport};
use crate::layers::extrude_layer::ExtrudeLayerError;
use crate::math::Vec3;
use crate::patches::SurfaceRegions;
use crate::surface_mapper::map_vertices_onto_surface_patches_given;
use crate::surface_optimizer::{optimise_mesh_surface, UntangleReport};
use crate::topology::TopoMesh;
use crate::volume_mesh::VolumeMesh;

/// Whether `createBasicFundamentalSheets` runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FunSheets {
    /// Upstream: `createFundamentalSheetsJFS(mesh, createWrapperSheet)`.
    #[default]
    Jfs,
    /// Ablation: no sheets (the gh:#1269 behaviour).
    Without,
}

/// What [`mesh_surface_edge_extractor_fun`] did.
#[derive(Debug, Clone, PartialEq)]
pub struct FunReport {
    /// The sheets: `None` when ablated, the error when `extrudeLayer`
    /// stopped (the step then went on without them).
    pub sheets: Option<Result<SheetsReport, ExtrudeLayerError>>,
    /// `smoothMeshSurface`'s `optimizeSurface`.
    pub smoothing: UntangleReport,
}

/// `meshSurfaceEdgeExtractorFUN(mesh, octree, createWrapperSheet)` on
/// `mesh`, whose boundary face `f` is in surface region `face_region[f]`
/// (`None` inside). Returns the report and the per-face regions of the new
/// mesh.
pub fn mesh_surface_edge_extractor_fun(
    mesh: &mut VolumeMesh,
    octree: Option<&crate::mesh_octree::MeshOctree>,
    points: &[Vec3],
    tris: &[[usize; 3]],
    regions: &SurfaceRegions,
    face_region: &[Option<usize>],
    sheets: FunSheets,
    create_wrapper_sheet: bool,
) -> (FunReport, Vec<Option<usize>>) {
    trace_dump("F0", mesh);
    let mut given = face_region.to_vec();
    let sheets_result = match sheets {
        FunSheets::Without => None,
        FunSheets::Jfs => {
            let mut w = TopoMesh::from_volume_mesh(mesh);
            w.region = face_region.to_vec();
            w.patch_names = regions.names.clone();
            Some(match create_fundamental_sheets(&w, SheetsKind::Jfs, create_wrapper_sheet) {
                Ok((out, r)) => {
                    let (m, g) = out.into_volume_mesh();
                    *mesh = m;
                    given = g.into_iter().map(|r| r.filter(|&r| r < regions.names.len())).collect();
                    Ok(r)
                }
                Err(e) => Err(e),
            })
        }
    };
    trace_dump("FA", mesh);
    let smoothing = match octree {
        Some(o) => crate::surface_optimizer::optimise_mesh_surface_octree(mesh, o, points, tris, regions, &given),
        None => optimise_mesh_surface(mesh, points, tris, regions, &given),
    };
    trace_dump("FB", mesh);
    *mesh = match octree {
        Some(o) => crate::surface_mapper::map_vertices_onto_surface_patches_given_octree(mesh, o, points, tris, regions, &given),
        None => map_vertices_onto_surface_patches_given(mesh, points, tris, regions, &given),
    };
    trace_dump("FC", mesh);
    (FunReport { sheets: sheets_result, smoothing }, given)
}

/// gh:#1385: with `GH1385_TRACE` set, print the mesh the way the instrumented
/// upstream `meshSurfaceEdgeExtractorFUN` does after each of its three steps
/// (`<tag>P` points, `<tag>F` faces with owner and neighbour, `<tag>C` cell lists).
fn trace_dump(tag: &str, mesh: &VolumeMesh) {
    if std::env::var("GH1385_TRACE").is_err() {
        return;
    }
    for (i, p) in mesh.points.iter().enumerate() {
        eprintln!("{tag}P {i} {:?} {:?} {:?}", p.x, p.y, p.z);
    }
    for (i, f) in mesh.faces.iter().enumerate() {
        let verts: Vec<String> = f.iter().map(|v| v.to_string()).collect();
        eprintln!("{tag}F {i} {} {} {}", mesh.owner[i], mesh.neighbour[i].map_or(-1, |n| n as i64), verts.join(" "));
    }
    for (i, c) in mesh.cell_face_lists().iter().enumerate() {
        let fs: Vec<String> = c.iter().map(|v| v.to_string()).collect();
        eprintln!("{tag}C {i} {}", fs.join(" "));
    }
}
