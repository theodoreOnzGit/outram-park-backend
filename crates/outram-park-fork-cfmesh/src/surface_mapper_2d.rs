// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Ported from cfMesh (Creative Fields, Ltd.; Franjo Juretic), GPL-3.0:
//   meshLibrary/utilities/surfaceTools/meshSurfaceMapper2D/meshSurfaceMapper2D.{C,H}
//   meshLibrary/utilities/surfaceTools/meshSurfaceMapper2D/meshSurfaceMapper2DMapVertices.C
//   meshLibrary/utilities/surfaceTools/meshSurfaceMapper2D/meshSurfaceMapper2DPremapVertices.C
//   meshLibrary/utilities/surfaceTools/meshSurfaceEdgeExtractor2D/meshSurfaceEdgeExtractor2D.{C,H}
//   meshLibrary/utilities/surfaceTools/meshSurfaceEdgeExtractor2D/meshSurfaceEdgeExtractor2DDistributeFaces.C
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

//! cfMesh's 2D surface mapping (gh:#1275): **`meshSurfaceMapper2D`** and
//! **`meshSurfaceEdgeExtractor2D`**. In a mesh one cell thick, the side of
//! the domain is a ring of **active** boundary edges, each running across
//! the thickness (both its faces are side faces, [`PolyMeshGen2DEngine`]).
//! Every move here moves an active edge's two points together in x and y,
//! each keeping its own z.
//!
//! - [`adjust_z_coordinates`]: the z-min points to the surface's lowest z,
//!   the z-max points to its highest.
//! - [`pre_map_vertices_2d`]: per iteration (~~default 3~~ **CORRECTED 2026-11-11 (gh:#1385): upstream's `meshSurfaceMapper2D::preMapVertices` default is 2**, the 3 was the 3D routine's), each active edge's
//!   points go **half way** to the nearest surface point of the average of
//!   its two faces' centres. Unlike the 3D `preMapVertices` there is no
//!   weighting and no untangling.
//! - [`map_vertices_onto_surface_2d`]: each active edge to the nearest
//!   surface point of its start point.
//! - [`map_vertices_onto_surface_patches_2d`]: each active edge in one
//!   patch to the nearest point of that patch; an edge between two patches
//!   (a corner of the 2D outline) by `mapCorners` (the average of the
//!   nearest points of both patches, iterated, against the nearest surface
//!   feature edge, whichever is nearer within 1.2x, capped at the mapping
//!   distance).
//! - [`distribute_boundary_faces`] (`meshSurfaceEdgeExtractor2D`): each side
//!   face to the surface region nearest its centre, the z-min and z-max
//!   faces to `bottomEmptyFaces` / `topEmptyFaces` (type `empty`).
//! - [`remap_boundary_points`]: [`adjust_z_coordinates`], then
//!   [`map_vertices_onto_surface_patches_2d`].
//!
//! Nearest-point searches use the octree with upstream's rule
//! ([`NearestSearch::Upstream`], [`TriangleProjection::Upstream`]). MPI
//! (`mapToSmallestDistance`, the edge exchanges) is not ported. The
//! no-argument `mapCorners()` is not ported: nothing calls it, and it looks
//! up mesh point labels in a set of boundary-point labels (an upstream
//! defect without effect).

use crate::correct_edges::foam_face_centre;
use crate::math::Vec3;
use crate::mesh_octree::{BoundBox, MeshOctree};
use crate::mesh_octree_queries::{NearestSearch, TriangleProjection};
use crate::poly_mesh_gen_2d_engine::PolyMeshGen2DEngine;
use crate::surface_engine::{SurfaceEngine, SurfacePartitioner};
use crate::volume_mesh::{BoundaryPatch, VolumeMesh};
use std::collections::BTreeMap;

/// `findNearestSurfacePoint` (region `None`) or
/// `findNearestSurfacePointInRegion`: (point, region), `None` where
/// upstream finds nothing.
fn nearest(o: &MeshOctree, p: Vec3, region: Option<usize>) -> Option<(Vec3, usize)> {
    o.find_nearest_surface_point_with(p, NearestSearch::Upstream, TriangleProjection::Upstream, region).map(|h| (h.point, h.region))
}

/// `findActiveBoundaryEdges`: boundary edges with two boundary faces, both
/// active (engine edge labels).
pub fn active_boundary_edges(engine: &SurfaceEngine, e2d: &PolyMeshGen2DEngine) -> Vec<usize> {
    (0..engine.edges.len())
        .filter(|&e| {
            let ef = &engine.edge_faces[e];
            ef.len() == 2 && e2d.active_face[engine.boundary_faces[ef[0]]] && e2d.active_face[engine.boundary_faces[ef[1]]]
        })
        .collect()
}

/// `adjustZCoordinates`: every z-min point of `mesh` to the lowest z of the
/// octree's surface, every z-max point to the highest. `Err` for a point
/// that is neither ("This mesh is not in the x - y plane!").
pub fn adjust_z_coordinates(mesh: &mut VolumeMesh, o: &MeshOctree) -> Result<(), String> {
    let sbb = BoundBox::of_points(&o.surface.points);
    let e2d = PolyMeshGen2DEngine::new(mesh)?;
    for p in 0..mesh.points.len() {
        if e2d.z_min_point[p] {
            mesh.points[p].z = sbb.min.z;
        } else if e2d.z_max_point[p] {
            mesh.points[p].z = sbb.max.z;
        } else {
            return Err("This mesh is not in the x - y plane!".into());
        }
    }
    Ok(())
}

/// Move both points of active edge `[a, b]` to `m` in x and y.
fn move_edge(points: &mut [Vec3], [a, b]: [usize; 2], m: Vec3) {
    points[a] = Vec3::new(m.x, m.y, points[a].z);
    points[b] = Vec3::new(m.x, m.y, points[b].z);
}

/// `meshSurfaceMapper2D::preMapVertices(nIterations)` (module docs).
/// Returns the edges whose search found nothing (left in place).
pub fn pre_map_vertices_2d(mesh: &mut VolumeMesh, o: &MeshOctree, n_iterations: usize) -> Result<usize, String> {
    let e2d = PolyMeshGen2DEngine::new(mesh)?;
    let mut n_missed = 0;
    for _ in 0..n_iterations {
        let engine = SurfaceEngine::new(mesh);
        let active = active_boundary_edges(&engine, &e2d);
        let targets: Vec<Option<Vec3>> = active
            .iter()
            .map(|&e| {
                let ef = &engine.edge_faces[e];
                if ef.is_empty() {
                    return None;
                }
                let s = ef.iter().fold(Vec3::ZERO, |s, &bf| s.add(engine.face_centres[bf]));
                Some(s.div(ef.len() as f64))
            })
            .collect();
        for (i, &e) in active.iter().enumerate() {
            let [a, b] = engine.edges[e];
            let p = mesh.points[a];
            let Some(t) = targets[i] else { continue };
            match nearest(o, t, None) {
                Some((m, _)) => {
                    let m = Vec3::new(m.x, m.y, p.z);
                    move_edge(&mut mesh.points, [a, b], p.add(m).scale(0.5));
                }
                None => n_missed += 1,
            }
        }
    }
    Ok(n_missed)
}

/// `meshSurfaceMapper2D::mapVerticesOntoSurface()`: every active edge to
/// the nearest surface point of its start point.
pub fn map_vertices_onto_surface_2d(mesh: &mut VolumeMesh, o: &MeshOctree) -> Result<(), String> {
    let e2d = PolyMeshGen2DEngine::new(mesh)?;
    let engine = SurfaceEngine::new(mesh);
    for e in active_boundary_edges(&engine, &e2d) {
        let [a, b] = engine.edges[e];
        if let Some((m, _)) = nearest(o, mesh.points[a], None) {
            move_edge(&mut mesh.points, [a, b], m);
        }
    }
    Ok(())
}

/// `findMappingDistance`: 16 x the largest squared distance from the
/// edge's centre to the centres of its faces.
fn mapping_distance(engine: &SurfaceEngine, points: &[Vec3], e: usize) -> f64 {
    let [a, b] = engine.edges[e];
    let c = points[a].add(points[b]).scale(0.5);
    let d = engine.edge_faces[e].iter().fold(0.0f64, |m, &bf| {
        let v = engine.face_centres[bf].sub(c);
        m.max(v.dot(v))
    });
    16.0 * d
}

/// `meshSurfaceMapper2D::mapCorners(edgesToMap)`: map active edges whose
/// two points are corners of the mesh surface onto the corner of the
/// outline between their common patches (module docs). Returns the edges
/// mapped. `Err` where upstream aborts ("Trying to map a point that is not
/// a corner").
fn map_corners(mesh: &mut VolumeMesh, o: &MeshOctree, engine: &SurfaceEngine, part: &SurfacePartitioner, edges: &[usize]) -> Result<usize, String> {
    let dist: BTreeMap<usize, f64> = edges.iter().map(|&e| (e, mapping_distance(engine, &mesh.points, e))).collect();
    for &e in edges {
        let [a, b] = engine.edges[e];
        let (bps, bpe) = (engine.bp[a], engine.bp[b]);
        let e_patches: Vec<usize> = part.point_patches[bps].iter().copied().filter(|q| part.point_patches[bpe].contains(q)).collect();
        if !part.corners.contains(&bps) || !part.corners.contains(&bpe) {
            return Err(format!("Trying to map a point that is not a corner (edge {e})"));
        }
        let p = mesh.points[a];
        let max_dist = dist[&e];
        let mut approx = p;
        for _ in 0..40 {
            let mut np = Vec3::ZERO;
            for &r in &e_patches {
                if let Some((q, _)) = nearest(o, approx, Some(r)) {
                    np = np.add(q);
                }
            }
            let np = np.div(e_patches.len() as f64);
            let d = np.sub(approx);
            if d.dot(d) < 1e-8 * max_dist {
                break;
            }
            approx = np;
        }
        let dsq_approx = {
            let d = approx.sub(p);
            d.dot(d)
        };
        let mut map_point = approx;
        if let Some((mp, dsq, _)) = o.find_nearest_edge_point(p, &e_patches) {
            let (mut mp, mut dsq) = (mp, dsq);
            if dsq > max_dist {
                mp = p.add(mp.sub(p).scale((max_dist / dsq).sqrt()));
                dsq = max_dist;
            }
            map_point = if dsq > 1.2 * dsq_approx { approx } else { mp };
        }
        move_edge(&mut mesh.points, [a, b], map_point);
    }
    Ok(edges.len())
}

/// `meshSurfaceMapper2D::mapVerticesOntoSurfacePatches()` (module docs).
/// The mesh's patch `i` must be the surface's region `i` (as
/// [`distribute_boundary_faces`] leaves it). Returns the corner edges.
pub fn map_vertices_onto_surface_patches_2d(mesh: &mut VolumeMesh, o: &MeshOctree) -> Result<usize, String> {
    let e2d = PolyMeshGen2DEngine::new(mesh)?;
    let engine = SurfaceEngine::new(mesh);
    let part = SurfacePartitioner::new(&engine);
    let active = active_boundary_edges(&engine, &e2d);
    map_edges_onto_surface_patches_2d(mesh, o, &engine, &part, &active)
}

/// `meshSurfaceMapper2D::mapVerticesOntoSurfacePatches(edgesToMap)`: the
/// given boundary edges of `engine` (built on `mesh`), as
/// [`map_vertices_onto_surface_patches_2d`] maps every active edge (the
/// one copy: that function calls this one). Upstream then updates the
/// engine's geometry at the moved points; here the caller does
/// (`optimizeSurface2D`, gh:#1275 remainder). Returns the corner edges.
pub fn map_edges_onto_surface_patches_2d(mesh: &mut VolumeMesh, o: &MeshOctree, engine: &SurfaceEngine, part: &SurfacePartitioner, edges: &[usize]) -> Result<usize, String> {
    let mut corners = Vec::new();
    for &e in edges {
        if engine.edge_patches[e].len() == 2 {
            corners.push(e);
            continue;
        }
        let [a, b] = engine.edges[e];
        if let Some((m, _)) = nearest(o, mesh.points[a], Some(engine.edge_patches[e][0])) {
            move_edge(&mut mesh.points, [a, b], m);
        }
    }
    map_corners(mesh, o, engine, part, &corners)
}

/// `meshSurfaceEdgeExtractor2D::distributeBoundaryFaces`: the surface's
/// regions (`names`) become the mesh's first patches, then
/// `bottomEmptyFaces` and `topEmptyFaces`. Every patch is kept, empty or
/// not, so patch `i` is region `i`. Returns the mesh and the patch types:
/// each surface patch's `geometricType()` (`surface_types`, gh:#1483;
/// upstream's `patchTypes[ptchI] = surfPatches[ptchI].geometricType()`, a
/// type missing from the slice reads `patch`), then `empty` twice.
pub fn distribute_boundary_faces(mesh: &VolumeMesh, o: &MeshOctree, names: &[String], surface_types: &[String]) -> Result<(VolumeMesh, Vec<String>), String> {
    let e2d = PolyMeshGen2DEngine::new(mesh)?;
    let n_int = mesh.n_internal_faces();
    let n_reg = names.len();
    let (bottom, top) = (n_reg, n_reg + 1);
    let mut groups: Vec<Vec<usize>> = vec![Vec::new(); n_reg + 2];
    for f in n_int..mesh.face_count() {
        let ring = &mesh.faces[f];
        if !e2d.active_face[f] {
            if e2d.z_min_point[ring[0]] {
                groups[bottom].push(f);
            } else if e2d.z_max_point[ring[0]] {
                groups[top].push(f);
            } else {
                // upstream leaves the patch label unset here
                return Err(format!("face {f} is neither active nor at z min / z max"));
            }
        } else {
            let c = foam_face_centre(&mesh.points, ring);
            let (_, r) = nearest(o, c, None).ok_or_else(|| format!("no surface point near face {f}"))?;
            groups[r].push(f);
        }
    }
    let mut out = mesh.clone();
    out.faces.truncate(n_int);
    out.owner.truncate(n_int);
    out.neighbour.truncate(n_int);
    out.patches.clear();
    let mut all_names: Vec<String> = names.to_vec();
    all_names.push("bottomEmptyFaces".into());
    all_names.push("topEmptyFaces".into());
    let mut new_label = vec![usize::MAX; mesh.face_count()];
    for (g, name) in groups.iter().zip(&all_names) {
        let start = out.faces.len();
        for &f in g {
            new_label[f] = out.faces.len();
            out.faces.push(mesh.faces[f].clone());
            out.owner.push(mesh.owner[f]);
            out.neighbour.push(None);
        }
        out.patches.push(BoundaryPatch { name: name.clone(), start_face: start, n_faces: g.len() });
    }
    // `replaceBoundary`: the internal faces stay in each cell's list, the new
    // boundary labels (running patch by patch) are appended to the owner's list
    // in the order of the old boundary faces (gh:#1385 round 4)
    let mut lists: Vec<Vec<usize>> = mesh.cell_face_lists().into_iter().map(|c| c.into_iter().filter(|&f| f < n_int).collect()).collect();
    for f in n_int..mesh.face_count() {
        lists[mesh.owner[f]].push(new_label[f]);
    }
    out.cells = Some(lists);
    let mut types: Vec<String> = (0..n_reg).map(|i| surface_types.get(i).cloned().unwrap_or_else(|| "patch".to_string())).collect();
    types.push("empty".into());
    types.push("empty".into());
    Ok((out, types))
}

/// `meshSurfaceEdgeExtractor2D::remapBoundaryPoints`:
/// [`adjust_z_coordinates`], then [`map_vertices_onto_surface_patches_2d`].
pub fn remap_boundary_points(mesh: &mut VolumeMesh, o: &MeshOctree) -> Result<usize, String> {
    adjust_z_coordinates(mesh, o)?;
    map_vertices_onto_surface_patches_2d(mesh, o)
}
