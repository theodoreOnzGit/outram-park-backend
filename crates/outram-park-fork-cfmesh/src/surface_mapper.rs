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
//! this assignment further, ~~which is not ported~~ **ported 2026-11-05,
//! gh:#1269, as [`crate::edge_extractor`]; the `_given` entry points take
//! its assignment**). Then, per boundary point,
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
//! then corners, as upstream. ~~Not ported: upstream's
//! `correctEdgesBetweenPatches` (topological repair of faces whose every
//! vertex lands on a feature edge)~~ **UPDATED 2026-11-05 (gh:#1241):**
//! `correctEdgesBetweenPatches` is ported in [`crate::correct_edges`] and
//! runs before this stage when `TetDualOptions::correct_edges` is on
//! (off by default). ~~Not ported: the surface untangler that follows;~~
//! **UPDATED 2026-11-05 (gh:#1259):** the untangler is
//! [`crate::surface_optimizer::MeshSurfaceOptimizer`], used instead of the
//! revert when `TetDualOptions::untangle_surface` is on (off by default);
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
use crate::surface_engine::{SurfaceEngine, SurfacePartitioner};
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
    map_impl(mesh, points, tris, regions, None, None, None)
}

/// [`map_vertices_onto_surface_patches`] with the patch of every boundary
/// face given (`face_region[f]`, `None` for internal faces), with nothing
/// undone afterwards: upstream's `mapVerticesOntoSurfacePatches` as
/// `meshSurfaceEdgeExtractorNonTopo::remapBoundaryPoints` runs it.
pub fn map_vertices_onto_surface_patches_given(
    mesh: &VolumeMesh,
    points: &[Vec3],
    tris: &[[usize; 3]],
    regions: &SurfaceRegions,
    face_region: &[Option<usize>],
) -> VolumeMesh {
    map_impl(mesh, points, tris, regions, Some(face_region), None, None)
}

/// [`map_vertices_onto_surface_patches_given`] with upstream's octree for the
/// nearest-point searches (gh:#1385): `meshSurfaceEdgeExtractorNonTopo::
/// remapBoundaryPoints` as the generators run it.
pub fn map_vertices_onto_surface_patches_given_octree(
    mesh: &VolumeMesh,
    o: &crate::mesh_octree::MeshOctree,
    points: &[Vec3],
    tris: &[[usize; 3]],
    regions: &SurfaceRegions,
    face_region: &[Option<usize>],
) -> VolumeMesh {
    map_impl(mesh, points, tris, regions, Some(face_region), None, Some(o))
}

/// [`map_vertices_onto_surface_patches_given`] for the boundary points with
/// `only[v]` set; every other point stays where it is (gh:#1283). The
/// patch, edge and corner classes and the mapping-distance bound are those
/// of the whole boundary, as upstream's `meshSurfaceMapper` computes them;
/// only the moves are restricted. Used to put back onto the surface the
/// points `untangleMeshFV`'s boundary loop moved
/// ([`crate::final_mesh::BoundaryProjection`]).
pub fn map_points_onto_surface_patches_given(
    mesh: &VolumeMesh,
    points: &[Vec3],
    tris: &[[usize; 3]],
    regions: &SurfaceRegions,
    face_region: &[Option<usize>],
    only: &[bool],
) -> VolumeMesh {
    map_impl(mesh, points, tris, regions, Some(face_region), Some(only), None)
}

/// The patch of every boundary face by its nearest surface triangle (the
/// rule [`map_vertices_onto_surface_patches`] uses), `None` for internal
/// faces: ~~the assignment cfMesh's `edgeExtractor` would hand~~ the
/// stand-in for `edgeExtractor`'s assignment (ported since gh:#1269 as
/// [`crate::edge_extractor`], whose result differs at the rims) handed to
/// [`crate::correct_edges::correct_edges_between_patches`].
pub fn boundary_face_regions(mesh: &VolumeMesh, points: &[Vec3], tris: &[[usize; 3]], regions: &SurfaceRegions) -> Vec<Option<usize>> {
    (0..mesh.face_count())
        .map(|f| mesh.neighbour[f].is_none().then(|| regions.region_of_tri[nearest_triangle(mesh.face_centroid(f), points, tris)]))
        .collect()
}

/// The input surface prepared for the mapper's queries: the triangles of
/// each region, the feature edges (surface edges whose two triangles are in
/// different regions) keyed by their region pair, and the regions touching
/// each surface vertex. Upstream asks its `meshOctree` for these
/// (`findNearestSurfacePointInRegion`, `findNearestEdgePoint`); here they
/// are brute force, as before.
#[derive(Debug, Clone)]
pub struct SurfaceQueries {
    points: Vec<Vec3>,
    tris: Vec<[usize; 3]>,
    region_tris: Vec<Vec<usize>>,
    feature_edges: BTreeMap<(usize, usize), Vec<(usize, usize)>>,
    vertex_regions: BTreeMap<usize, BTreeSet<usize>>,
    /// gh:#1385: with an octree, `nearest` and `nearest_in_region` are
    /// `meshOctree::findNearestSurfacePoint[InRegion]` (upstream's search and
    /// `nearestPointOnTheTriangle`), not the brute-force stand-ins.
    octree: Option<std::sync::Arc<crate::mesh_octree::MeshOctree>>,
}

impl SurfaceQueries {
    /// The octree these queries go through, if any (gh:#1385).
    pub(crate) fn octree(&self) -> Option<&std::sync::Arc<crate::mesh_octree::MeshOctree>> {
        self.octree.as_ref()
    }

    /// These queries answered by upstream's octree search (see the field).
    pub fn with_octree(mut self, o: std::sync::Arc<crate::mesh_octree::MeshOctree>) -> Self {
        self.octree = Some(o);
        self
    }

    pub fn new(points: &[Vec3], tris: &[[usize; 3]], regions: &SurfaceRegions) -> Self {
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
        SurfaceQueries { points: points.to_vec(), tris: tris.to_vec(), region_tris, feature_edges, vertex_regions, octree: None }
    }

    /// `findNearestSurfacePointInRegion`: the nearest point of region `r`
    /// (`p` itself when the region has no triangle).
    pub(crate) fn nearest_in_region(&self, p: Vec3, r: usize) -> Vec3 {
        if let Some(o) = &self.octree {
            use crate::mesh_octree_queries::{NearestSearch, TriangleProjection};
            if let Some(h) = o.find_nearest_surface_point_with(p, NearestSearch::Upstream, TriangleProjection::Upstream, Some(r)) {
                return h.point;
            }
        }
        let mut best = p;
        let mut best_d2 = f64::MAX;
        for &i in &self.region_tris[r] {
            let t = self.tris[i];
            let q = closest_point_on_triangle(p, self.points[t[0]], self.points[t[1]], self.points[t[2]]);
            let d2 = q.sub(p).dot(q.sub(p));
            if d2 < best_d2 {
                best_d2 = d2;
                best = q;
            }
        }
        best
    }

    /// `findNearestSurfacePoint`: the nearest point of the whole surface.
    pub(crate) fn nearest(&self, p: Vec3) -> Vec3 {
        if let Some(h) = self.octree.as_ref().and_then(|o| o.find_nearest_surface_point(p)) {
            return h.point;
        }
        crate::snap::closest_point_on_surface(p, &self.points, &self.tris)
    }

    /// `findNearestEdgePoint` for the edge between two regions (`ps`
    /// sorted): the nearest point on such a feature edge and its squared
    /// distance, `None` when the regions share no edge.
    pub(crate) fn nearest_edge_point(&self, p: Vec3, ps: (usize, usize)) -> Option<(f64, Vec3)> {
        let es = self.feature_edges.get(&ps)?;
        let mut best = (f64::MAX, p);
        for &(a, b) in es {
            let q = closest_on_segment(p, self.points[a], self.points[b]);
            let dq = q.sub(p).dot(q.sub(p));
            if dq < best.0 {
                best = (dq, q);
            }
        }
        Some(best)
    }

    /// `mapCorners`' target: the nearest surface vertex touched by every
    /// region of `ps` (`p` itself when there is none).
    pub(crate) fn nearest_corner(&self, p: Vec3, ps: &[usize]) -> Vec3 {
        let mut best = (f64::MAX, p);
        for (&sv, rs) in &self.vertex_regions {
            if ps.iter().all(|r| rs.contains(r)) {
                let d = self.points[sv].sub(p).dot(self.points[sv].sub(p));
                if d < best.0 {
                    best = (d, self.points[sv]);
                }
            }
        }
        best.1
    }
}

/// `mapEdgeNodes` for one point at `p` between patches `ps` (sorted, two):
/// up to 20 passes of the average of the nearest points in each patch, then
/// the nearest feature-edge point unless more than 1.2× as far (squared);
/// the move is limited to `max2` (squared). Shared by the patch mapper and
/// [`crate::surface_optimizer`]'s untangler.
pub(crate) fn map_edge_node(q: &SurfaceQueries, p: Vec3, ps: &[usize], max2: f64) -> Vec3 {
    let mut approx = p;
    for _ in 0..20 {
        let np = ps.iter().fold(Vec3::ZERO, |a, &r| a.add(q.nearest_in_region(approx, r))).scale(1.0 / ps.len() as f64);
        let moved = np.sub(approx).dot(np.sub(approx));
        approx = np;
        if moved < 1e-8 * max2 {
            break;
        }
    }
    let d2_approx = approx.sub(p).dot(approx.sub(p));
    let mut map = approx;
    if ps.len() == 2 {
        if let Some(best) = q.nearest_edge_point(p, (ps[0], ps[1])) {
            if best.0 <= 1.2 * d2_approx {
                map = best.1;
            }
        }
    }
    limit_move(p, map, max2)
}

/// Limit the move `p -> q` to squared length `max2` (`findMappingDistance`'s
/// bound, as `mapEdgeNodes` and `mapCorners` apply it).
pub(crate) fn limit_move(p: Vec3, q: Vec3, max2: f64) -> Vec3 {
    let d2 = q.sub(p).dot(q.sub(p));
    if d2 > max2 && d2 > 0.0 {
        p.add(q.sub(p).scale((max2 / d2).sqrt()))
    } else {
        q
    }
}

fn map_impl(
    mesh: &VolumeMesh,
    points: &[Vec3],
    tris: &[[usize; 3]],
    regions: &SurfaceRegions,
    given: Option<&[Option<usize>]>,
    only: Option<&[bool]>,
    octree: Option<&crate::mesh_octree::MeshOctree>,
) -> VolumeMesh {
    // The boundary's addressing from the shared engine (gh:#1257): patch
    // of every boundary face as given (upstream's mesh patches), or by its
    // nearest triangle; then each point's faces and sorted patches.
    let engine = SurfaceEngine::new(mesh);
    let face_region: Vec<usize> = engine
        .boundary_faces
        .iter()
        .map(|&f| match given.and_then(|g| g[f]) {
            Some(r) => r,
            None => regions.region_of_tri[nearest_triangle(mesh.face_centroid(f), points, tris)],
        })
        .collect();
    let part = SurfacePartitioner::with_face_patches(&engine, face_region);
    let mut q = SurfaceQueries::new(points, tris, regions);
    if let Some(o) = octree {
        q = q.with_octree(std::sync::Arc::new(o.clone()));
    }
    let mut new_points = mesh.points.clone();
    // gh:#1283's `only` (mesh point labels) as the boundary-point selection
    // of `map_points_onto_patches` (gh:#1269): the same rule, one copy.
    let selected: Option<Vec<bool>> = only.map(|o| engine.boundary_points.iter().map(|&v| o[v]).collect());
    map_points_onto_patches(&engine, &part, &q, &mut new_points, selected.as_deref());
    VolumeMesh {
        points: new_points,
        faces: mesh.faces.clone(),
        owner: mesh.owner.clone(),
        neighbour: mesh.neighbour.clone(),
        n_cells: mesh.n_cells,
        patches: mesh.patches.clone(),
        cells: mesh.cells.clone(),
    }
}

/// The mapping of [`map_vertices_onto_surface_patches`] on points held by
/// the caller, in place: `engine` is the mesh's surface, `part` its
/// partition into the surface's regions, and only boundary points `b` with
/// `selected[b]` move when `selected` is given (upstream's
/// `mapVerticesOntoSurfacePatches(nodesToMap)`, which `edgeExtractor::
/// checkFacePatchesGeometry` calls, gh:#1269). With `None` it is the
/// whole-surface mapping operation for operation: each patch node reads its
/// own position before writing it, so reading `new_points` in place equals
/// reading the original points (pinned by the dual-kind fingerprints).
pub(crate) fn map_points_onto_patches(engine: &SurfaceEngine, part: &SurfacePartitioner, q: &SurfaceQueries, new_points: &mut [Vec3], selected: Option<&[bool]>) {
    if let Some(o) = q.octree() {
        // gh:#1385: with the octree, upstream's own routines
        map_points_onto_patches_upstream(engine, part, q, o, new_points, selected);
        return;
    }
    let patches_of = |b: usize| -> Vec<usize> {
        let mut s = part.point_patches[b].clone();
        s.sort_unstable();
        s
    };
    // findMappingDistance: 4 x the largest squared distance from the point
    // to the centres of its boundary faces, on the current geometry (the
    // vertex average here; upstream's faceCentres are OpenFOAM centres:
    // kept as before, a difference recorded in docs/upstream_coverage.md).
    let max_dist2 = |pts: &[Vec3], b: usize| -> f64 {
        let p = pts[engine.boundary_points[b]];
        let d = engine.point_faces[b]
            .iter()
            .map(|&bf| {
                let ring = &engine.face_rings[bf];
                let c = ring.iter().fold(Vec3::ZERO, |a, &v| a.add(pts[v])).scale(1.0 / ring.len() as f64);
                c.sub(p).dot(c.sub(p))
            })
            .fold(0.0_f64, f64::max);
        4.0 * d
    };
    let (mut edge_nodes, mut corners) = (Vec::new(), Vec::new());
    // Patch nodes (upstream maps these without a distance limit).
    for b in 0..engine.n_boundary_points() {
        if selected.is_some_and(|s| !s[b]) {
            continue;
        }
        let v = engine.boundary_points[b];
        let ps = patches_of(b);
        match ps.len() {
            1 => new_points[v] = q.nearest_in_region(new_points[v], ps[0]),
            2 => edge_nodes.push((b, ps)),
            _ => corners.push((b, ps)),
        }
    }
    // Edge nodes.
    let mut mapped_edges: Vec<(usize, Vec3)> = Vec::new();
    for (b, ps) in &edge_nodes {
        let v = engine.boundary_points[*b];
        let max2 = max_dist2(&new_points, *b);
        mapped_edges.push((v, map_edge_node(&q, new_points[v], ps, max2)));
    }
    for (v, p) in mapped_edges {
        new_points[v] = p;
    }
    // Corners: the nearest surface vertex touched by all the point's patches.
    let mut mapped_corners: Vec<(usize, Vec3)> = Vec::new();
    for (b, ps) in &corners {
        let v = engine.boundary_points[*b];
        let p = new_points[v];
        let max2 = max_dist2(&new_points, *b);
        mapped_corners.push((v, limit_move(p, q.nearest_corner(p, ps), max2)));
    }
    for (v, p) in mapped_corners {
        new_points[v] = p;
    }
}

/// `meshSurfaceMapper::findMappingDistance` for boundary point `b`: 4 x the
/// largest squared distance from the point to the centres (`face::centre`,
/// the engine's `faceCentres`) of its boundary faces, on `pts`.
fn mapping_distance(engine: &SurfaceEngine, pts: &[Vec3], b: usize) -> f64 {
    let p = pts[engine.boundary_points[b]];
    let mut d = 0.0_f64;
    for &bf in &engine.point_faces[b] {
        let c = crate::correct_edges::foam_face_centre(pts, &engine.face_rings[bf]);
        let v = c.sub(p);
        d = d.max(v.dot(v));
    }
    d * 4.0
}

/// The loop shared by `mapEdgeNodes` and `mapCorners`: up to 20 passes of
/// the average of the nearest points of `patches` to the running estimate
/// (`newP /= patches.size()`), stopping *before* the estimate is replaced
/// when the pass moved it by less than `1e-8 * max_dist` (gh:#1385: the port
/// replaced it first).
fn approximate_on_patches(q: &SurfaceQueries, p: Vec3, patches: &[usize], max_dist: f64) -> Vec3 {
    let mut approx = p;
    let mut iter = 0;
    while iter < 20 {
        iter += 1;
        let mut new_p = Vec3::ZERO;
        for &pt in patches {
            new_p = new_p.add(q.nearest_in_region(approx, pt));
        }
        new_p = new_p.div(patches.len() as f64);
        let d = new_p.sub(approx);
        if d.dot(d) < 1e-8 * max_dist {
            break;
        }
        approx = new_p;
    }
    approx
}

/// `meshSurfaceMapper::mapEdgeNodes` for one point `p` at the edge of
/// `patches` (in `pointFaces` order), with the octree (gh:#1385): the
/// approximation on the patches, `findNearestEdgePoint` (kept only within
/// 1.2 x the squared distance of the approximation) and the `maxDist` cut.
pub(crate) fn map_edge_node_upstream(q: &SurfaceQueries, o: &crate::mesh_octree::MeshOctree, p: Vec3, patches: &[usize], max_dist: f64) -> Vec3 {
    let approx = approximate_on_patches(q, p, patches, max_dist);
    let d_approx = approx.sub(p);
    let dist_sq_approx = d_approx.dot(d_approx);
    let (mut map_point, mut dist_sq) = match o.find_nearest_edge_point(p, patches) {
        Some((pt, d, _)) => (pt, d),
        None => (p, 1e300),
    };
    if dist_sq > 1.2 * dist_sq_approx {
        map_point = approx;
        dist_sq = dist_sq_approx;
    }
    if dist_sq > max_dist {
        let f = (max_dist / dist_sq).sqrt();
        map_point = map_point.sub(p).scale(f).add(p);
    }
    map_point
}

/// `meshSurfaceMapper::mapVerticesOntoSurfacePatches(nodesToMap)` as
/// upstream writes it, with the octree (gh:#1385): the nodes are split by
/// the partitioner's `corners` and `edgePoints`; the others go to the
/// nearest point of their first patch; then `mapEdgeNodes` (approximation on
/// the patches, `findNearestEdgePoint`, the 1.2 rule, the `maxDist` cut) and
/// `mapCorners` (the same approximation, the nearest corner of the surface
/// whose patches include the point's). The patches of a point stay in
/// `pointFaces` order, unsorted.
fn map_points_onto_patches_upstream(engine: &SurfaceEngine, part: &SurfacePartitioner, q: &SurfaceQueries, o: &crate::mesh_octree::MeshOctree, new_points: &mut [Vec3], selected: Option<&[bool]>) {
    let (mut edges, mut corners) = (Vec::new(), Vec::new());
    for b in 0..engine.n_boundary_points() {
        if selected.is_some_and(|s| !s[b]) {
            continue;
        }
        let v = engine.boundary_points[b];
        if part.corners.contains(&b) {
            corners.push(b);
        } else if part.edge_points.contains(&b) {
            edges.push(b);
        } else {
            new_points[v] = q.nearest_in_region(new_points[v], part.point_patches[b][0]);
        }
    }
    // mapEdgeNodes: every distance is found before any of them moves
    let max_dist: Vec<f64> = edges.iter().map(|&b| mapping_distance(engine, new_points, b)).collect();
    let mut moved: Vec<(usize, Vec3)> = Vec::with_capacity(edges.len());
    for (i, &b) in edges.iter().enumerate() {
        let v = engine.boundary_points[b];
        moved.push((v, map_edge_node_upstream(q, o, new_points[v], &part.point_patches[b], max_dist[i])));
    }
    for (v, p) in moved {
        new_points[v] = p;
    }
    // mapCorners, on the geometry the edge nodes left
    let surf = &o.surface;
    let sp = crate::surface::features::TriSurfacePartitioner::new(surf);
    let max_dist: Vec<f64> = corners.iter().map(|&b| mapping_distance(engine, new_points, b)).collect();
    let mut moved: Vec<(usize, Vec3)> = Vec::with_capacity(corners.len());
    for (i, &b) in corners.iter().enumerate() {
        let v = engine.boundary_points[b];
        let p = new_points[v];
        let patches = &part.point_patches[b];
        let approx = approximate_on_patches(q, p, patches, max_dist[i]);
        let d_approx = approx.sub(p);
        let dist_sq_approx = d_approx.dot(d_approx);
        let mut dist_sq = max_dist[i];
        let mut map_point = p;
        for (sc, &corner_id) in sp.corners.iter().enumerate() {
            let spt = surf.points[corner_id];
            let d = spt.sub(p);
            if d.dot(d) < dist_sq && patches.iter().all(|pt| sp.corner_patches[sc].contains(pt)) {
                map_point = spt;
                dist_sq = d.dot(d);
            }
        }
        if dist_sq > 1.2 * dist_sq_approx {
            map_point = approx;
        }
        moved.push((v, map_point));
    }
    for (v, p) in moved {
        new_points[v] = p;
    }
}

/// [`map_vertices_onto_surface_patches`], then **undo the move locally**
/// wherever it left a cell with a non-positive face pyramid (a cell centre
/// on or outside one of its faces, including a face collapsed to zero area
/// because all its vertices landed on one feature edge): every boundary
/// point of such a cell goes back to where it was, and the check repeats,
/// at most `passes` times. Returns the mesh and the number of boundary
/// points left unmapped.
///
/// This stands in for ~~the two upstream steps that are not ported:
/// `correctEdgesBetweenPatches`, which re-splits a face lying on a feature
/// edge, and~~ `meshSurfaceOptimizer::untangleSurface`~~, which is not ported~~
/// (**UPDATED 2026-11-05, gh:#1241:** `correctEdgesBetweenPatches` is now
/// [`crate::correct_edges`], run before this when
/// `TetDualOptions::correct_edges` is on; **gh:#1259:** `untangleSurface`
/// is now [`crate::surface_optimizer`], the pipeline's alternative to this
/// function under `TetDualOptions::untangle_surface`). Where upstream repairs
/// the face, this leaves that bit of the boundary where the dual put it.
pub fn map_vertices_onto_surface_patches_untangled(
    mesh: &VolumeMesh,
    points: &[Vec3],
    tris: &[[usize; 3]],
    regions: &SurfaceRegions,
    passes: usize,
) -> (VolumeMesh, usize) {
    untangled_impl(mesh, points, tris, regions, passes, None)
}

/// [`map_vertices_onto_surface_patches_untangled`] with the patch of every
/// boundary face given (`face_region[f]`, `None` for internal faces), as
/// upstream's mapper reads the mesh's patches: after
/// [`crate::correct_edges::correct_edges_between_patches`] the children of a
/// split face keep their parent's patch, which their own centroids need not
/// be nearest to.
pub fn map_vertices_onto_surface_patches_untangled_given(
    mesh: &VolumeMesh,
    points: &[Vec3],
    tris: &[[usize; 3]],
    regions: &SurfaceRegions,
    passes: usize,
    face_region: &[Option<usize>],
) -> (VolumeMesh, usize) {
    untangled_impl(mesh, points, tris, regions, passes, Some(face_region))
}

fn untangled_impl(
    mesh: &VolumeMesh,
    points: &[Vec3],
    tris: &[[usize; 3]],
    regions: &SurfaceRegions,
    passes: usize,
    given: Option<&[Option<usize>]>,
) -> (VolumeMesh, usize) {
    let mapped = map_impl(mesh, points, tris, regions, given, None, None);
    // With given patches (after correctEdgesBetweenPatches) a tangled cell
    // may be a pyramid with no boundary face of its own: revert the
    // boundary points of all its faces. Without, the historical rule.
    revert_tangled(mesh, mapped, passes, given.is_some())
}

/// Undo the boundary moves of `mapped` (against `orig`) at every cell with
/// a non-positive face pyramid, at most `passes` times: the points of the
/// cell's boundary faces, or with `whole_cell` every boundary point on any
/// of its faces. Returns the mesh and the number of points reverted.
fn revert_tangled(orig: &VolumeMesh, mut mapped: VolumeMesh, passes: usize, whole_cell: bool) -> (VolumeMesh, usize) {
    let boundary_pts: BTreeSet<usize> = if whole_cell {
        (0..orig.face_count()).filter(|&f| orig.neighbour[f].is_none()).flat_map(|f| orig.faces[f].iter().copied()).collect()
    } else {
        BTreeSet::new()
    };
    let mut reverted: BTreeSet<usize> = BTreeSet::new();
    for _ in 0..passes {
        let bad = cells_with_nonpositive_pyramids(&mapped);
        if bad.is_empty() {
            break;
        }
        let mut changed = false;
        for f in 0..mapped.face_count() {
            let hit = if whole_cell {
                bad.contains(&mapped.owner[f]) || mapped.neighbour[f].is_some_and(|n| bad.contains(&n))
            } else {
                mapped.neighbour[f].is_none() && bad.contains(&mapped.owner[f])
            };
            if hit {
                for &v in &mapped.faces[f] {
                    if (!whole_cell || boundary_pts.contains(&v)) && reverted.insert(v) {
                        mapped.points[v] = orig.points[v];
                        changed = true;
                    }
                }
            }
        }
        if !changed {
            break;
        }
    }
    (mapped, reverted.len())
}

/// cfMesh `meshSurfaceMapper::mapVerticesOntoSurface`
/// (`meshSurfaceMapperMapVertices.C`, v1.1 `e944f6ba695f`): every boundary
/// point to the nearest point of the surface, whatever its patch, with no
/// distance limit; `tetMeshGenerator::mapMeshToSurface` runs it before the
/// patches are extracted. Upstream then untangles the surface
/// (`meshSurfaceOptimizer::untangleSurface`, not ported); here the moves of
/// the points of any cell it tangles are undone (whole cells, at most
/// `passes` times). Returns the mesh and the number of points left where
/// they were.
pub fn map_vertices_onto_surface_untangled(mesh: &VolumeMesh, points: &[Vec3], tris: &[[usize; 3]], passes: usize) -> (VolumeMesh, usize) {
    let mut mapped = mesh.clone();
    let bnd: BTreeSet<usize> = (0..mesh.face_count()).filter(|&f| mesh.neighbour[f].is_none()).flat_map(|f| mesh.faces[f].iter().copied()).collect();
    for &v in &bnd {
        mapped.points[v] = crate::snap::closest_point_on_surface(mesh.points[v], points, tris);
    }
    revert_tangled(mesh, mapped, passes, true)
}

/// The cells of `m` with a non-positive face pyramid about their centre
/// (`(c_f − C)·S_f ≤ 0` for one of their faces, `S_f` outward from the
/// cell): what OpenFOAM's `checkMesh` fails as "face pyramids".
pub fn cells_with_nonpositive_pyramids(m: &VolumeMesh) -> BTreeSet<usize> {
    let (_, centres) = crate::checks::cell_geometry(m);
    let mut bad = BTreeSet::new();
    for f in 0..m.face_count() {
        let s = m.face_area_vector(f);
        let c = m.face_centroid(f);
        if c.sub(centres[m.owner[f]]).dot(s) <= 0.0 {
            bad.insert(m.owner[f]);
        }
        if let Some(nb) = m.neighbour[f] {
            if c.sub(centres[nb]).dot(s) >= 0.0 {
                bad.insert(nb);
            }
        }
    }
    bad
}

/// The point of segment `[a, b]` nearest to `p`.
fn closest_on_segment(p: Vec3, a: Vec3, b: Vec3) -> Vec3 {
    let e = b.sub(a);
    let l2 = e.dot(e);
    let t = if l2 > 0.0 { (p.sub(a).dot(e) / l2).clamp(0.0, 1.0) } else { 0.0 };
    a.add(e.scale(t))
}
