// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Ported from cfMesh (Creative Fields, Ltd.; Franjo Juretic), GPL-3.0:
//   meshLibrary/utilities/triSurfaceTools/triSurfaceChecks/
//     triSurfaceChecks{.C,.H}
//   meshLibrary/utilities/helperFunctions/helperFunctionsGeometryQueriesI.H
//     (doTrianglesOverlap, doTrianglesIntersect with a distance tolerance)
//   utilities/checkSurfaceMesh/checkSurfaceMesh.C
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

//! `triSurfaceChecks` and the `checkSurfaceMesh` utility (gh:#1277,
//! `docs/upstream_coverage.md` §4 item 23).
//!
//! Each check comes in upstream's two forms: one that returns the offending
//! facets or points (`const triSurf&`), and one that also records them as a
//! subset of the surface (`triSurf&`, default subset names as upstream).

use crate::math::Vec3;
use crate::mesh_octree::{help, mag_sqr, BoundBox, MeshOctree, TriSurf, SMALL, VSMALL};
use crate::mesh_octree_creator::{group_marking, MeshOctreeCreator};
use petir::mathf::RealMath;
use std::sync::Arc;

/// `createOctreeWithRefinedBoundary(20, 30)`: the search octree the
/// geometric checks build.
pub(crate) fn search_octree(surf: &TriSurf, max_level: u8, n_tris: usize) -> MeshOctree {
    let mut o = MeshOctree::new(Arc::new(surf.clone()));
    MeshOctreeCreator::new(&o, None).create_octree_with_refined_boundary(&mut o, max_level, n_tris);
    o
}

/// Replace (or create) the facet subset `name` with `facets`.
fn set_facet_subset(surf: &mut TriSurf, name: &str, facets: &[usize]) {
    if let Some(id) = surf.facet_subsets.index(name) {
        surf.facet_subsets.remove(id);
    }
    let id = surf.facet_subsets.add(name);
    for &f in facets {
        surf.facet_subsets.add_element(id, f);
    }
}

/// One subset `prefix + i` per group (upstream does this only when there
/// is more than one group).
fn set_group_subsets(surf: &mut TriSurf, prefix: &str, group: &[Option<usize>], n_groups: usize) {
    if n_groups <= 1 {
        return;
    }
    let mut ids = Vec::with_capacity(n_groups);
    for i in 0..n_groups {
        let name = format!("{prefix}{i}");
        if let Some(id) = surf.facet_subsets.index(&name) {
            surf.facet_subsets.remove(id);
        }
        ids.push(surf.facet_subsets.add(&name));
    }
    for (t, g) in group.iter().enumerate() {
        if let Some(g) = g {
            surf.facet_subsets.add_element(ids[*g], t);
        }
    }
}

/// `checkAngles(surf, badTriangles, angleTol)`: facets where two
/// consecutive edge directions are within `angle_tol` degrees of each other,
/// `(p_i − p_{i−1})·(p_{i+1} − p_i) ≥ cos(angle_tol)` after normalising.
///
/// Note (read from the code, 2026-11-05): that is a vertex whose **interior
/// angle is at least 180° − `angle_tol`**, a flattened "cap" triangle; a
/// needle triangle (one small angle, two near 90°) is not flagged despite
/// the check's name and the `smallAngles` subset. Ported as written.
pub fn check_angles(surf: &TriSurf, angle_tol: f64) -> Vec<usize> {
    let tol = (angle_tol * std::f64::consts::PI / 180.0).r_cos();
    let pts = &surf.points;
    let mut bad = Vec::new();
    for (t, tri) in surf.triangles.iter().enumerate() {
        for i in 0..3 {
            let vn = pts[tri[(i + 1) % 3]].sub(pts[tri[i]]);
            let vn = vn.scale(1.0 / (vn.length() + VSMALL));
            let vp = pts[tri[i]].sub(pts[tri[(i + 2) % 3]]);
            let vp = vp.scale(1.0 / (vp.length() + VSMALL));
            if vp.dot(vn) >= tol {
                bad.push(t);
                break;
            }
        }
    }
    bad
}

/// `checkAngles(surf, subsetName = "smallAngles", angleTol)`.
pub fn check_angles_subset(surf: &mut TriSurf, subset_name: &str, angle_tol: f64) -> usize {
    let bad = check_angles(surf, angle_tol);
    if !bad.is_empty() {
        set_facet_subset(surf, subset_name, &bad);
    }
    bad.len()
}

/// The facets across each two-facet edge of `t`.
fn manifold_neighbours(surf: &TriSurf, t: usize) -> Vec<usize> {
    let mut out = Vec::new();
    for &e in &surf.facet_edges[t] {
        let ef = &surf.edge_facets[e];
        if ef.len() != 2 {
            continue;
        }
        out.push(if ef[0] == t { ef[1] } else { ef[0] });
    }
    out
}

/// `checkSurfaceManifolds(surf, triangleInManifold)`: the facet groups
/// connected over two-facet edges. Returns `(group of each facet, count)`.
pub fn check_surface_manifolds(surf: &TriSurf) -> (Vec<Option<usize>>, usize) {
    group_marking(surf.size(), |t| manifold_neighbours(surf, t), |_| true)
}

/// `checkSurfaceManifolds(surf, subsetPrefix = "manifold_")`.
pub fn check_surface_manifolds_subset(surf: &mut TriSurf, prefix: &str) -> usize {
    let (g, n) = check_surface_manifolds(surf);
    set_group_subsets(surf, prefix, &g, n);
    n
}

/// `checkForHoles`: the facet of every one-facet edge, once per such edge.
pub fn check_for_holes(surf: &TriSurf) -> Vec<usize> {
    surf.edge_facets.iter().filter(|ef| ef.len() == 1).map(|ef| ef[0]).collect()
}

/// `checkForHoles(surf, subsetName = "holes")`.
pub fn check_for_holes_subset(surf: &mut TriSurf, subset_name: &str) -> usize {
    let bad = check_for_holes(surf);
    if !bad.is_empty() {
        set_facet_subset(surf, subset_name, &bad);
    }
    bad.len()
}

/// `checkForNonManifoldEdges`: every facet of every edge with more than
/// two facets.
pub fn check_for_non_manifold_edges(surf: &TriSurf) -> Vec<usize> {
    surf.edge_facets.iter().filter(|ef| ef.len() > 2).flat_map(|ef| ef.iter().copied()).collect()
}

/// `checkForNonManifoldEdges(surf, subsetPrefix = "nonManifold_")`.
///
/// **Upstream defect, fixed in the port:** upstream's subset form calls
/// `checkForHoles`, so it records (and `checkSurfaceMesh` reports) open
/// edges under the non-manifold name. The port calls the non-manifold
/// check. As upstream, the subset is named by the prefix itself.
pub fn check_for_non_manifold_edges_subset(surf: &mut TriSurf, prefix: &str) -> usize {
    let bad = check_for_non_manifold_edges(surf);
    if !bad.is_empty() {
        set_facet_subset(surf, prefix, &bad);
    }
    bad.len()
}

/// `orientationOps::surfaceNeiOp`: the neighbours over two-facet edges
/// that are oriented consistently with `t`.
fn orientation_neighbours(surf: &TriSurf, t: usize) -> Vec<usize> {
    let tri = surf.triangles[t];
    let mut out = Vec::new();
    for (te, &e) in surf.facet_edges[t].iter().enumerate() {
        let ef = &surf.edge_facets[e];
        if ef.len() != 2 {
            continue;
        }
        let nei = if ef[0] == t { ef[1] } else { ef[0] };
        let nt = surf.triangles[nei];
        let Some(pos) = nt.iter().position(|&p| p == tri[te]) else { continue };
        if tri[(te + 1) % 3] == nt[(pos + 2) % 3] {
            out.push(nei);
        }
    }
    out
}

/// `checkOrientation`: the groups of consistently oriented facets.
pub fn check_orientation(surf: &TriSurf) -> (Vec<Option<usize>>, usize) {
    group_marking(surf.size(), |t| orientation_neighbours(surf, t), |_| true)
}

/// `checkOrientation(surf, subsetPrefix = "orientation_")`.
pub fn check_orientation_subset(surf: &mut TriSurf, prefix: &str) -> usize {
    let (g, n) = check_orientation(surf);
    set_group_subsets(surf, prefix, &g, n);
    n
}

/// `checkDisconnectedParts`: facet groups connected over any shared edge.
pub fn check_disconnected_parts(surf: &TriSurf) -> (Vec<Option<usize>>, usize) {
    let nei = |t: usize| {
        let mut out = Vec::new();
        for &e in &surf.facet_edges[t] {
            for &f in &surf.edge_facets[e] {
                if f != t {
                    out.push(f);
                }
            }
        }
        out
    };
    group_marking(surf.size(), nei, |_| true)
}

/// `checkDisconnectedParts(surf, subsetPrefix = "region_")`.
pub fn check_disconnected_parts_subset(surf: &mut TriSurf, prefix: &str) -> usize {
    let (g, n) = check_disconnected_parts(surf);
    set_group_subsets(surf, prefix, &g, n);
    n
}

/// `calculateBoundingBox`.
pub fn calculate_bounding_box(surf: &TriSurf) -> BoundBox {
    BoundBox::of_points(&surf.points)
}

/// `checkCollocatedPoints`: points closer than `dist_tol` to another point
/// of a facet in a nearby octree leaf, ascending.
pub fn check_collocated_points(surf: &TriSurf, dist_tol: f64) -> Vec<usize> {
    let o = search_octree(surf, 20, 30);
    let pts = &surf.points;
    let mut coll = vec![false; pts.len()];
    let d = Vec3::new(dist_tol, dist_tol, dist_tol);
    for (pi, &p) in pts.iter().enumerate() {
        let bb = BoundBox::new(p.sub(d), p.add(d));
        for l in o.find_leaves_contained_in_box(&bb) {
            for t in o.contained_triangles(l) {
                for &q in &surf.triangles[t] {
                    if q == pi {
                        continue;
                    }
                    if mag_sqr(pts[q].sub(p)) < dist_tol * dist_tol {
                        coll[pi] = true;
                        coll[q] = true;
                    }
                }
            }
        }
    }
    (0..pts.len()).filter(|&p| coll[p]).collect()
}

/// `checkCollocatedPoints(surf, subsetName = "collocatedPoints", distTol)`:
/// a **point** subset.
pub fn check_collocated_points_subset(surf: &mut TriSurf, subset_name: &str, dist_tol: f64) -> usize {
    let bad = check_collocated_points(surf, dist_tol);
    if !bad.is_empty() {
        if let Some(id) = surf.point_subsets.index(subset_name) {
            surf.point_subsets.remove(id);
        }
        let id = surf.point_subsets.add(subset_name);
        for &p in &bad {
            surf.point_subsets.add_element(id, p);
        }
    }
    bad.len()
}

/// The triangles' corner points.
fn corners(surf: &TriSurf, t: usize) -> [Vec3; 3] {
    let tr = surf.triangles[t];
    [surf.points[tr[0]], surf.points[tr[1]], surf.points[tr[2]]]
}

/// The candidate pairs `(t, j)`, `t < j`, from the octree leaves overlapping
/// `t`'s box grown by `tol` (with repeats, in upstream's visiting order).
fn candidate_pairs(o: &MeshOctree, surf: &TriSurf, t: usize, tol: f64) -> Vec<usize> {
    let c = corners(surf, t);
    let mut bb = BoundBox::new(c[0], c[0]);
    for p in &c[1..] {
        bb.min = Vec3::new(bb.min.x.min(p.x), bb.min.y.min(p.y), bb.min.z.min(p.z));
        bb.max = Vec3::new(bb.max.x.max(p.x), bb.max.y.max(p.y), bb.max.z.max(p.z));
    }
    let d = Vec3::new(tol, tol, tol);
    let bb = BoundBox::new(bb.min.sub(d), bb.max.add(d));
    let mut out = Vec::new();
    for l in o.find_leaves_contained_in_box(&bb) {
        for j in o.contained_triangles(l) {
            if t < j {
                out.push(j);
            }
        }
    }
    out
}

/// Facets that self-intersect, and the pairs of vertex-disjoint facets
/// found to intersect (upstream records each pair as a subset
/// `noCommonIntersect_<t>_<j>`, even through its `const` form).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct IntersectionReport {
    /// The offending facets, ascending.
    pub facets: Vec<usize>,
    /// The vertex-disjoint pairs, in the order found (with repeats when a
    /// pair shares several leaves, as upstream adds it again).
    pub disjoint_pairs: Vec<(usize, usize)>,
}

/// `checkSelfIntersections(surf, badFaces, tol)`. Facets sharing an edge
/// are skipped; facets sharing one point are tested with the opposite edge
/// against the other facet (`triLineIntersection`); disjoint facets with
/// [`do_triangles_intersect`].
pub fn check_self_intersections(surf: &TriSurf, tol: f64) -> IntersectionReport {
    let o = search_octree(surf, 20, 30);
    let mut hit = vec![false; surf.size()];
    let mut pairs = Vec::new();
    for t in 0..surf.size() {
        let tri = surf.triangles[t];
        let ct = corners(surf, t);
        for j in candidate_pairs(&o, surf, t, tol) {
            let nt = surf.triangles[j];
            let mut shared = Vec::new();
            for (pj, &q) in nt.iter().enumerate() {
                for &p in &tri {
                    if p == q {
                        shared.push(pj);
                    }
                }
            }
            if shared.len() >= 2 {
                continue;
            } else if shared.len() == 1 {
                let s = surf.points[nt[(shared[0] + 1) % 3]];
                let e = surf.points[nt[(shared[0] + 2) % 3]];
                if help::tri_line_intersection(ct[0], ct[1], ct[2], s, e).is_some() {
                    hit[t] = true;
                    hit[j] = true;
                }
            } else if do_triangles_intersect(ct, corners(surf, j), tol) {
                hit[t] = true;
                hit[j] = true;
                pairs.push((t, j));
            }
        }
    }
    IntersectionReport { facets: (0..surf.size()).filter(|&t| hit[t]).collect(), disjoint_pairs: pairs }
}

/// Add upstream's per-pair subsets `<prefix><t>_<j>`.
fn add_pair_subsets(surf: &mut TriSurf, prefix: &str, pairs: &[(usize, usize)]) {
    for &(t, j) in pairs {
        let id = surf.facet_subsets.add(&format!("{prefix}{t}_{j}"));
        surf.facet_subsets.add_element(id, t);
        surf.facet_subsets.add_element(id, j);
    }
}

/// `checkSelfIntersections(surf, subsetName = "selfIntersect", tol)`, with
/// the `noCommonIntersect_*` pair subsets.
pub fn check_self_intersections_subset(surf: &mut TriSurf, subset_name: &str, tol: f64) -> usize {
    let r = check_self_intersections(surf, tol);
    add_pair_subsets(surf, "noCommonIntersect_", &r.disjoint_pairs);
    if !r.facets.is_empty() {
        set_facet_subset(surf, subset_name, &r.facets);
    }
    r.facets.len()
}

/// `checkOverlaps(surf, badFaces, tol, angleTol)`: pairs of facets (any
/// connectivity) that [`do_triangles_overlap`] at `tol` and within
/// `angle_tol` degrees of parallel. Upstream's debug write of each overlap
/// polygon to an `.fms` file is not ported.
pub fn check_overlaps(surf: &TriSurf, tol: f64, angle_tol: f64) -> IntersectionReport {
    let o = search_octree(surf, 20, 30);
    let cos_val = (angle_tol * std::f64::consts::PI / 180.0).r_cos();
    let mut hit = vec![false; surf.size()];
    let mut pairs = Vec::new();
    for t in 0..surf.size() {
        let ct = corners(surf, t);
        for j in candidate_pairs(&o, surf, t, tol) {
            if do_triangles_overlap(ct, corners(surf, j), tol, cos_val).is_some() {
                hit[t] = true;
                hit[j] = true;
                pairs.push((t, j));
            }
        }
    }
    IntersectionReport { facets: (0..surf.size()).filter(|&t| hit[t]).collect(), disjoint_pairs: pairs }
}

/// `checkOverlaps(surf, subsetName = "overlappingTriangles", tol,
/// angleTol)`, with the `noCommonOverlap_*` pair subsets.
pub fn check_overlaps_subset(surf: &mut TriSurf, subset_name: &str, tol: f64, angle_tol: f64) -> usize {
    let r = check_overlaps(surf, tol, angle_tol);
    add_pair_subsets(surf, "noCommonOverlap_", &r.disjoint_pairs);
    if !r.facets.is_empty() {
        set_facet_subset(surf, subset_name, &r.facets);
    }
    r.facets.len()
}

/// `help::doTrianglesIntersect(tri0, tri1, distTol)`: any vertex of one
/// within `dist_tol` of the other triangle, or any edge of one crossing the
/// other. `false` (upstream warns) for a negative tolerance.
pub fn do_triangles_intersect(t0: [Vec3; 3], t1: [Vec3; 3], dist_tol: f64) -> bool {
    if dist_tol < 0.0 {
        return false;
    }
    let d2 = dist_tol * dist_tol;
    for p in t0 {
        if mag_sqr(help::nearest_point_on_the_triangle(t1[0], t1[1], t1[2], p).sub(p)) < d2 {
            return true;
        }
    }
    for p in t1 {
        if mag_sqr(help::nearest_point_on_the_triangle(t0[0], t0[1], t0[2], p).sub(p)) < d2 {
            return true;
        }
    }
    for i in 0..3 {
        if help::tri_line_intersection(t0[0], t0[1], t0[2], t1[i], t1[(i + 1) % 3]).is_some() {
            return true;
        }
    }
    for i in 0..3 {
        if help::tri_line_intersection(t1[0], t1[1], t1[2], t0[i], t0[(i + 1) % 3]).is_some() {
            return true;
        }
    }
    false
}

/// `help::doTrianglesOverlap(tri0, tri1, overlappingPolygon, distTol,
/// cosTol)`: for nearly parallel triangles (`|n0·n1| ≥ cos_tol`) with a
/// vertex within `dist_tol` of the other's plane, the overlap polygon (tri0
/// clipped by tri1's edges in a common plane). `None` for no overlap.
pub fn do_triangles_overlap(t0: [Vec3; 3], t1: [Vec3; 3], dist_tol: f64, cos_tol: f64) -> Option<Vec<Vec3>> {
    if dist_tol < 0.0 {
        return None;
    }
    let area = |t: [Vec3; 3]| t[1].sub(t[0]).cross(t[2].sub(t[0])).scale(0.5);
    let n0 = area(t0);
    let dn0 = n0.length();
    let n0 = n0.scale(1.0 / (dn0 + VSMALL));
    if dn0 < VSMALL {
        return None;
    }
    let n1 = area(t1);
    let dn1 = n1.length();
    let n1 = n1.scale(1.0 / (dn1 + VSMALL));
    if dn1 < VSMALL {
        return None;
    }
    if n0.dot(n1).abs() < cos_tol {
        return None;
    }
    let near = t1.iter().any(|p| p.sub(t0[0]).dot(n0).abs() < dist_tol) || t0.iter().any(|p| p.sub(t1[0]).dot(n1).abs() < dist_tol);
    if !near {
        return None;
    }
    let vec = n0.add(if n0.dot(n1) >= 0.0 { n1 } else { n1.scale(-1.0) });
    let vec = vec.scale(1.0 / (vec.length() + VSMALL));
    let c0 = t0[0].add(t0[1]).add(t0[2]).scale(1.0 / 3.0);
    let c1 = t1[0].add(t1[1]).add(t1[2]).scale(1.0 / 3.0);
    let origin = c0.add(c1).scale(0.5);
    // The vertex farthest from the origin (strictly greater, first wins).
    let mut best = t0[0];
    let mut dist_sq = mag_sqr(t0[0].sub(origin));
    for p in [t0[1], t0[2], t1[0], t1[1], t1[2]] {
        let d = mag_sqr(p.sub(origin));
        if d > dist_sq {
            dist_sq = d;
            best = p;
        }
    }
    if dist_sq < VSMALL {
        return None;
    }
    let bo = best.sub(origin);
    let x = bo.sub(vec.scale(bo.dot(vec)));
    let x = x.scale(1.0 / (x.length() + VSMALL));
    let y = vec.cross(x);
    let proj = |p: Vec3| (p.sub(origin).dot(x), p.sub(origin).dot(y));
    let mut poly: Vec<(f64, f64)> = t0.iter().map(|&p| proj(p)).collect();
    let t1p: Vec<(f64, f64)> = t1.iter().map(|&p| proj(p)).collect();
    for e in 0..3 {
        let a = t1p[e];
        let b = t1p[(e + 1) % 3];
        let v = (b.0 - a.0, b.1 - a.1);
        let dist: Vec<f64> = poly.iter().map(|p| v.1 * (p.0 - a.0) - v.0 * (p.1 - a.1)).collect();
        let n = poly.len();
        let mut np = Vec::new();
        for i in 0..n {
            let (di, dn) = (dist[i], dist[(i + 1) % n]);
            let (pi, pn) = (poly[i], poly[(i + 1) % n]);
            let cut = || {
                let (a, b) = (di.abs(), dn.abs());
                ((a * pn.0 + b * pi.0) / (a + b + VSMALL), (a * pn.1 + b * pi.1) / (a + b + VSMALL))
            };
            if di >= 0.0 {
                np.push(pi);
                if dn < 0.0 {
                    np.push(cut());
                }
            } else if dn >= 0.0 {
                np.push(cut());
            }
        }
        poly = np;
    }
    if poly.is_empty() {
        return None;
    }
    Some(poly.iter().map(|&(u, v)| origin.add(x.scale(u)).add(y.scale(v))).collect())
}

/// What `checkSurfaceMesh` found. Each count is the length of the list the
/// check returned (upstream's return value).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct SurfaceMeshCheck {
    /// Manifolds (connected over two-facet edges).
    pub n_manifolds: usize,
    /// Facets at open edges (one per open edge).
    pub n_hole_facets: usize,
    /// Facets at non-manifold edges.
    pub n_non_manifold_facets: usize,
    /// Disconnected parts.
    pub n_disconnected_parts: usize,
    /// Facets failing [`check_angles`] at 1°.
    pub n_small_angle_facets: usize,
    /// Self-intersecting facets.
    pub n_self_intersecting_facets: usize,
    /// Overlapping facets (5°).
    pub n_overlapping_facets: usize,
    /// Collocated points.
    pub n_collocated_points: usize,
    /// Consistently oriented groups. **Not upstream's utility:** it does not
    /// run `checkOrientation`; reported here because the utility's checks
    /// cannot see a flipped facet.
    pub n_orientation_groups: usize,
    /// Upstream's failure count (manifolds > 1, holes, non-manifold edges,
    /// parts > 1, small angles, self-intersections, overlaps, collocated
    /// points); 0 means "Surface passes all checks".
    pub n_failed: usize,
}

/// The `checkSurfaceMesh` utility (file I/O not ported): run upstream's
/// checks in upstream's order with its tolerances (`distTol = SMALL ×
/// |bounding-box diagonal|`, 1° angles, 5° overlaps), recording the
/// subsets on `surf`. The subset form of the non-manifold check is the
/// fixed one ([`check_for_non_manifold_edges_subset`]); orientation is
/// checked in addition (see [`SurfaceMeshCheck::n_orientation_groups`]).
pub fn check_surface_mesh(surf: &mut TriSurf) -> SurfaceMeshCheck {
    let bb = calculate_bounding_box(surf);
    let dist_tol = SMALL * bb.max.sub(bb.min).length();
    let mut r = SurfaceMeshCheck { n_manifolds: check_surface_manifolds_subset(surf, "manifold_"), ..Default::default() };
    r.n_hole_facets = check_for_holes_subset(surf, "holes");
    r.n_non_manifold_facets = check_for_non_manifold_edges_subset(surf, "nonManifold_");
    r.n_disconnected_parts = check_disconnected_parts_subset(surf, "region_");
    r.n_small_angle_facets = check_angles_subset(surf, "smallAngles", 1.0);
    r.n_self_intersecting_facets = check_self_intersections_subset(surf, "selfIntersect", dist_tol);
    r.n_overlapping_facets = check_overlaps_subset(surf, "overlaps", dist_tol, 5.0);
    r.n_collocated_points = check_collocated_points_subset(surf, "collocatedPoints", dist_tol);
    r.n_orientation_groups = check_orientation(surf).1;
    r.n_failed = [
        r.n_manifolds > 1,
        r.n_hole_facets > 0,
        r.n_non_manifold_facets > 0,
        r.n_disconnected_parts > 1,
        r.n_small_angle_facets > 0,
        r.n_self_intersecting_facets > 0,
        r.n_overlapping_facets > 0,
        r.n_collocated_points > 0,
    ]
    .iter()
    .filter(|&&b| b)
    .count();
    r
}

