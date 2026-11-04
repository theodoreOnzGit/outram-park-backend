// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Ported from cfMesh (Creative Fields, Ltd.), commit 6cf1d211f9cf:
//   meshLibrary/utilities/surfaceTools/meshSurfaceMapper/
//     meshSurfaceMapperMapVertices.C   (mapVerticesOntoSurfacePatches)
//     meshSurfaceMapperCornersAndEdges.C (findMappingDistance, mapEdgeNodes,
//                                         mapCorners)
//   meshLibrary/utilities/surfaceTools/meshSurfaceEdgeExtractorNonTopo/
//     meshSurfaceEdgeExtractorNonTopo{,DistributeFaces}.C
//   meshLibrary/tetMesh/tetMeshGenerator/tetMeshGenerator.C (the order:
//     mapMeshToSurface, then mapEdgesAndCorners, then the boundary layers)
// Copyright (C) 2014-2017 Creative Fields, Ltd. Licence: GPL-3.0-only.
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

//! **Feature-edge capture**: map a mesh's boundary vertices onto the
//! surface **with respect to its patches** (gh:#1202), cfMesh's
//! `meshSurfaceEdgeExtractorNonTopo` / `meshSurfaceMapper::
//! mapVerticesOntoSurfacePatches`.
//!
//! Every boundary face belongs to the patch of its nearest surface triangle
//! (the crate's [`crate::patches`] rule; upstream's `edgeExtractor` refines
//! this assignment further, which is not ported). Then, per boundary point,
//! with `P` the set of patches of the boundary faces around it:
//!
//! - `|P| = 1` (cfMesh's patch node): moved to the nearest point of the
//!   surface **in that patch** (`findNearestSurfacePointInRegion`);
//! - `|P| = 2` (edge node): moved onto the feature edge between the two
//!   patches: up to 20 passes of "the average of the nearest points in each
//!   patch", then the nearest point on a surface edge shared by the two
//!   patches (`findNearestEdgePoint`), kept unless it is more than 1.2× as
//!   far (squared) as the iterated point;
//! - `|P| ≥ 3` (corner): moved to the nearest surface vertex touched by
//!   every one of those patches (`mapCorners` takes the nearest corner);
//!
//! every move limited to `√(4 max_f |c_f − p|²)` over the point's boundary
//! faces (`findMappingDistance`), and patch nodes first, then edge nodes,
//! then corners, as upstream. Not ported: upstream's
//! `correctEdgesBetweenPatches` (topological repair of faces whose every
//! vertex lands on a feature edge) and the surface untangler that follows;
//! the pipeline keeps the mapped mesh only if it is valid, and says so
//! otherwise.
//!
//! **Why the pipeline needs it.** The tet-dual's boundary vertices are
//! averages of the snapped tet boundary, not surface points, so without this
//! stage the dual's boundary is a polygon inside the surface and every sharp
//! edge is chamfered. On the Foam Studio pipe (R = 10 mm, h = 4 mm) the
//! inlet patch spanned 0.9396 πR² and the section 0.9796 πR²: the case ran
//! at 6 % less flow than it set (laminar_pipe_gh1155.md, gh:#1202).

use crate::math::Vec3;
use crate::patches::{nearest_triangle, SurfaceRegions};
use crate::snap::closest_point_on_triangle;
use crate::volume_mesh::VolumeMesh;
use std::collections::{BTreeMap, BTreeSet};

/// Map the boundary of `mesh` onto the surface `(points, tris)` patch by
/// patch, sharpening the feature edges between `regions` (see the module
/// documentation). The topology is unchanged; only boundary points move.
pub fn map_vertices_onto_surface_patches(
    mesh: &VolumeMesh,
    points: &[Vec3],
    tris: &[[usize; 3]],
    regions: &SurfaceRegions,
) -> VolumeMesh {
    // Patch (region) of every boundary face, by its nearest triangle.
    let mut face_region: BTreeMap<usize, usize> = BTreeMap::new();
    for f in 0..mesh.face_count() {
        if mesh.neighbour[f].is_none() {
            let t = nearest_triangle(mesh.face_centroid(f), points, tris);
            face_region.insert(f, regions.region_of_tri[t]);
        }
    }
    // Point -> its boundary faces and their patches.
    let mut point_faces: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    for &f in face_region.keys() {
        for &v in &mesh.faces[f] {
            point_faces.entry(v).or_default().push(f);
        }
    }
    let patches_of = |v: usize| -> Vec<usize> {
        let s: BTreeSet<usize> = point_faces[&v].iter().map(|f| face_region[f]).collect();
        s.into_iter().collect()
    };
    // Triangles of each region; feature edges (surface edges whose two
    // triangles are in different regions) keyed by their region pair; the
    // regions touching each surface vertex.
    let n_regions = regions.names.len();
    let mut region_tris: Vec<Vec<usize>> = vec![Vec::new(); n_regions];
    let mut edge_tris: BTreeMap<(usize, usize), Vec<usize>> = BTreeMap::new();
    let mut vertex_regions: BTreeMap<usize, BTreeSet<usize>> = BTreeMap::new();
    for (i, t) in tris.iter().enumerate() {
        let r = regions.region_of_tri[i];
        region_tris[r].push(i);
        for k in 0..3 {
            let (a, b) = (t[k], t[(k + 1) % 3]);
            edge_tris.entry((a.min(b), a.max(b))).or_default().push(i);
            vertex_regions.entry(t[k]).or_default().insert(r);
        }
    }
    let mut feature_edges: BTreeMap<(usize, usize), Vec<(usize, usize)>> = BTreeMap::new();
    for (&e, ts) in &edge_tris {
        let rs: BTreeSet<usize> = ts.iter().map(|&t| regions.region_of_tri[t]).collect();
        if rs.len() == 2 {
            let v: Vec<usize> = rs.into_iter().collect();
            feature_edges.entry((v[0], v[1])).or_default().push(e);
        }
    }
    let nearest_in_region = |p: Vec3, r: usize| -> Vec3 {
        let mut best = p;
        let mut best_d2 = f64::MAX;
        for &i in &region_tris[r] {
            let t = tris[i];
            let q = closest_point_on_triangle(p, points[t[0]], points[t[1]], points[t[2]]);
            let d2 = q.sub(p).dot(q.sub(p));
            if d2 < best_d2 {
                best_d2 = d2;
                best = q;
            }
        }
        best
    };

    let mut new_points = mesh.points.clone();
    // findMappingDistance: 4 x the largest squared distance from the point
    // to the centres of its boundary faces, on the current geometry.
    let max_dist2 = |pts: &[Vec3], v: usize| -> f64 {
        let p = pts[v];
        let d = point_faces[&v]
            .iter()
            .map(|&f| {
                let ring = &mesh.faces[f];
                let c = ring.iter().fold(Vec3::ZERO, |a, &q| a.add(pts[q])).scale(1.0 / ring.len() as f64);
                c.sub(p).dot(c.sub(p))
            })
            .fold(0.0_f64, f64::max);
        4.0 * d
    };
    let limit = |p: Vec3, q: Vec3, max2: f64| -> Vec3 {
        let d2 = q.sub(p).dot(q.sub(p));
        if d2 > max2 && d2 > 0.0 {
            p.add(q.sub(p).scale((max2 / d2).sqrt()))
        } else {
            q
        }
    };
    let (mut edge_nodes, mut corners) = (Vec::new(), Vec::new());
    // Patch nodes (upstream maps these without a distance limit).
    for &v in point_faces.keys() {
        let ps = patches_of(v);
        match ps.len() {
            1 => new_points[v] = nearest_in_region(mesh.points[v], ps[0]),
            2 => edge_nodes.push((v, ps)),
            _ => corners.push((v, ps)),
        }
    }
    // Edge nodes.
    let mut mapped_edges: Vec<(usize, Vec3)> = Vec::new();
    for (v, ps) in &edge_nodes {
        let p = new_points[*v];
        let max2 = max_dist2(&new_points, *v);
        let mut approx = p;
        for _ in 0..20 {
            let np = ps.iter().fold(Vec3::ZERO, |a, &r| a.add(nearest_in_region(approx, r))).scale(1.0 / ps.len() as f64);
            let moved = np.sub(approx).dot(np.sub(approx));
            approx = np;
            if moved < 1e-8 * max2 {
                break;
            }
        }
        let d2_approx = approx.sub(p).dot(approx.sub(p));
        let mut map = approx;
        let mut d2 = d2_approx;
        if let Some(es) = feature_edges.get(&(ps[0], ps[1])) {
            let mut best = (f64::MAX, p);
            for &(a, b) in es {
                let q = closest_on_segment(p, points[a], points[b]);
                let dq = q.sub(p).dot(q.sub(p));
                if dq < best.0 {
                    best = (dq, q);
                }
            }
            if best.0 <= 1.2 * d2_approx {
                map = best.1;
                d2 = best.0;
            }
        }
        let _ = d2;
        mapped_edges.push((*v, limit(p, map, max2)));
    }
    for (v, q) in mapped_edges {
        new_points[v] = q;
    }
    // Corners: the nearest surface vertex touched by all the point's patches.
    let mut mapped_corners: Vec<(usize, Vec3)> = Vec::new();
    for (v, ps) in &corners {
        let p = new_points[*v];
        let max2 = max_dist2(&new_points, *v);
        let mut best = (f64::MAX, p);
        for (&sv, rs) in &vertex_regions {
            if ps.iter().all(|r| rs.contains(r)) {
                let d = points[sv].sub(p).dot(points[sv].sub(p));
                if d < best.0 {
                    best = (d, points[sv]);
                }
            }
        }
        mapped_corners.push((*v, limit(p, best.1, max2)));
    }
    for (v, q) in mapped_corners {
        new_points[v] = q;
    }
    VolumeMesh {
        points: new_points,
        faces: mesh.faces.clone(),
        owner: mesh.owner.clone(),
        neighbour: mesh.neighbour.clone(),
        n_cells: mesh.n_cells,
        patches: mesh.patches.clone(),
    }
}

/// The point of segment `[a, b]` nearest to `p`.
fn closest_on_segment(p: Vec3, a: Vec3, b: Vec3) -> Vec3 {
    let e = b.sub(a);
    let l2 = e.dot(e);
    let t = if l2 > 0.0 { (p.sub(a).dot(e) / l2).clamp(0.0, 1.0) } else { 0.0 };
    a.add(e.scale(t))
}
