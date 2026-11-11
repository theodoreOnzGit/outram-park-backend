// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Ported from cfMesh (Creative Fields, Ltd.; Franjo Juretic), GPL-3.0:
//   meshLibrary/utilities/octrees/meshOctree/meshOctreeNeighbourSearches.C
//     (findLeafContainingVertex, findNeighbourOverNode,
//      findNeighboursOverEdge, findNeighboursInDirection,
//      findNeighboursForLeaf, findAllLeafNeighbours,
//      findLeavesForCubeVertex, findLeavesInSphere)
//   meshLibrary/utilities/octrees/meshOctree/meshOctreeInsideCalculations.C
//     (findCubeForPosition, findLeafLabelForPosition, isPointInside,
//      findEdgesInBox, findTrianglesInBox, findLeavesContainedInBox)
//   meshLibrary/utilities/octrees/meshOctree/meshOctreeFindNearestSurfacePoint.C
//     (findNearestSurfacePoint, findNearestSurfacePointInRegion,
//      findNearestEdgePoint, findNearestPointToEdge, findNearestCorner,
//      findNearestPointToPatches)
//   meshLibrary/utilities/helperFunctions/helperFunctionsGeometryQueriesI.H
//     (nearestEdgePointToTheLine)
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

//! Queries on a [`MeshOctree`] (gh:#1263): point location, neighbour
//! searches across levels, and nearest surface point, triangle, feature edge
//! and corner.
//!
//! Upstream returns `-1` for "no leaf" and `OTHERPROC` for a leaf on another
//! MPI rank; the port is serial and returns `Option<usize>` (leaf labels).
//! Where upstream appends a `-1` to a neighbour list (a neighbour outside the
//! root, or a refined neighbour's child that is itself refined), the list
//! holds `None` at that place, so the list keeps upstream's length and order.
//!
//! # Nearest-point search: upstream's and an exact variant
//!
//! Upstream's `findNearestSurfacePoint` searches a box of half-width
//! `0.75 x` the size of the leaf holding `p`, doubles it until **any**
//! triangle is found, and keeps the nearest triangle **in that box**. That
//! is not always the nearest triangle of the surface: a triangle found at
//! distance `d` greater than the half-width `s` may hide a nearer one just
//! outside the box. [`NearestSearch::Upstream`] is that rule, transcribed.
//! [`NearestSearch::Exact`] adds one guard (keep doubling until the nearest
//! found distance is `<= s`, so every nearer triangle must cross the box) and
//! breaks ties by the **lower triangle index**, the rule of the crate's
//! brute force ([`crate::patches::nearest_triangle`],
//! [`crate::snap::closest_point_on_surface`]). The guard is this port's, not
//! upstream's; the two variants are compared in the tests.
//!
//! The per-triangle projection is selectable too ([`TriangleProjection`]):
//! upstream's `help::nearestPointOnTheTriangle` or Ericson's routine, which
//! the crate's brute force uses. Bit-identity with the brute force needs the
//! same projection.

use crate::math::Vec3;
use petir::mathf::RealMath;
use crate::mesh_octree::{
    help, mag_sqr, BoundBox, CubeCoordinates, CubeType, MeshOctree, EDGE_NODES, FACE_NODES, NO_CUBE, ROOT,
    SMALL, VGREAT, VSMALL,
};

/// How [`MeshOctree::find_nearest_surface_point_with`] stops searching.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NearestSearch {
    /// Upstream's rule: stop at the first box that holds any triangle;
    /// strict `<` in tree order; at most 100 doublings.
    Upstream,
    /// Stop only once the nearest found distance is within the box
    /// half-width (then no nearer triangle can lie outside it); ties to the
    /// lower triangle index.
    Exact,
}

/// The closest-point-on-triangle routine a nearest search uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TriangleProjection {
    /// cfMesh's `help::nearestPointOnTheTriangle`.
    Upstream,
    /// Ericson's Voronoi-region routine ([`crate::snap`]'s).
    Ericson,
}

/// Which route a caller's nearest-point search takes (gh:#1263). The brute
/// force is the default everywhere; the octree route is opt-in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SurfaceSearch {
    /// Every triangle, every query (`snap::closest_point_on_surface`,
    /// `patches::nearest_triangle`).
    #[default]
    BruteForce,
    /// cfMesh's surface octree, [`NearestSearch::Exact`] with
    /// [`TriangleProjection::Ericson`]: bit-identical to the brute force.
    Octree,
}

/// Result of a nearest-surface-point query.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NearestHit {
    /// The nearest point found.
    pub point: Vec3,
    /// Its squared distance to the query point.
    pub dist_sq: f64,
    /// The triangle it lies on.
    pub triangle: usize,
    /// That triangle's region.
    pub region: usize,
}

fn project(proj: TriangleProjection, a: Vec3, b: Vec3, c: Vec3, p: Vec3) -> Vec3 {
    match proj {
        TriangleProjection::Upstream => help::nearest_point_on_the_triangle(a, b, c, p),
        TriangleProjection::Ericson => crate::snap::closest_point_on_triangle(p, a, b, c),
    }
}

/// `help::nearestEdgePointToTheLine`: the points of closest approach of the
/// segment `e0`–`e1` (clamped) and the infinite line through `l0`–`l1`.
/// `None` for a degenerate line or a parallel pair. The 2 x 2 system is
/// solved as upstream does, through OpenFOAM's 3 x 3 `inv` with
/// `zz = SMALL`.
pub fn nearest_edge_point_to_the_line(e0: Vec3, e1: Vec3, l0: Vec3, l1: Vec3) -> Option<(Vec3, Vec3)> {
    let v = l1.sub(l0);
    let d = l0.sub(e0);
    let e = e1.sub(e0);
    let v_mag = v.length();
    if v_mag < VSMALL {
        return None;
    }
    let e_mag = e.length();
    if e_mag < VSMALL {
        let on_edge = e0;
        return Some((on_edge, help::nearest_point_on_the_edge(l0, l1, on_edge)));
    }
    if (v.scale(1.0 / v_mag).dot(e.scale(1.0 / e_mag))).abs() > 1.0 - SMALL {
        return None;
    }
    let (xx, xy, yy, zz) = (v.dot(v), -v.dot(e), e.dot(e), SMALL);
    let (yx, xz, yz, zx, zy) = (xy, 0.0, 0.0, 0.0, 0.0);
    let det = xx * yy * zz + xy * yz * zx + xz * yx * zy - xx * yz * zy - xy * yx * zz - xz * yy * zx;
    let i00 = (yy * zz - zy * yz) / det;
    let i01 = (xz * zy - xy * zz) / det;
    let i10 = (zx * yz - yx * zz) / det;
    let i11 = (xx * zz - xz * zx) / det;
    let src = [-d.dot(v), d.dot(e)];
    let s0 = i00 * src[0] + i01 * src[1];
    let s1 = i10 * src[0] + i11 * src[1];
    let on_line = l0.add(v.scale(s0));
    let on_edge = if s1 > 1.0 {
        e1
    } else if s1 < 0.0 {
        e0
    } else {
        e0.add(e.scale(s1))
    };
    Some((on_edge, on_line))
}

/// A leaf holding more triangles than this is split by
/// [`MeshOctree::for_nearest_queries`].
pub const NEAREST_QUERY_MAX_TRIANGLES: usize = 8;
/// The deepest level [`MeshOctree::for_nearest_queries`] refines to.
pub const NEAREST_QUERY_MAX_LEVEL: u8 = 12;

fn out_of_range(cc: &CubeCoordinates, root_z: Option<i64>) -> bool {
    let lim = 1i64 << cc.level;
    let z_out = match root_z {
        // the quadtree: every cube keeps the root's posZ
        Some(rz) => cc.z != rz,
        None => cc.z >= lim || cc.z < 0,
    };
    cc.x >= lim || cc.x < 0 || cc.y >= lim || cc.y < 0 || z_out
}

impl MeshOctree {
    /// The root's `posZ` for a quadtree (`initialCubePtr_->posZ()`),
    /// `None` for an octree.
    fn quadtree_root_z(&self) -> Option<i64> {
        if self.is_quadtree {
            Some(self.cube(ROOT).coords.z)
        } else {
            None
        }
    }
}

impl MeshOctree {
    /// An octree for nearest-point queries on a bare triangle soup (every
    /// triangle in region 0): leaves holding more than
    /// [`NEAREST_QUERY_MAX_TRIANGLES`] are split, down to
    /// [`NEAREST_QUERY_MAX_LEVEL`]. This stopping rule is the port's, not
    /// upstream's (upstream's depth comes from `meshOctreeCreator`'s cell
    /// sizes); it changes speed, not answers, with [`NearestSearch::Exact`].
    pub fn for_nearest_queries(points: &[Vec3], tris: &[[usize; 3]]) -> Self {
        let surf = crate::mesh_octree::TriSurf::new(points.to_vec(), tris.to_vec(), None);
        let mut o = MeshOctree::new(std::sync::Arc::new(surf));
        loop {
            let split: Vec<usize> = o
                .leaves
                .iter()
                .copied()
                .filter(|&c| {
                    let cube = o.cube(c);
                    cube.coords.level < NEAREST_QUERY_MAX_LEVEL
                        && cube.contained_elements.is_some_and(|r| o.slot.contained_triangles[r].len() > NEAREST_QUERY_MAX_TRIANGLES)
                })
                .collect();
            if split.is_empty() {
                break;
            }
            for c in split {
                o.refine_cube(c);
            }
            o.create_list_of_leaves();
        }
        o
    }

    /// The exact nearest surface point and its triangle
    /// ([`NearestSearch::Exact`], [`TriangleProjection::Ericson`]); `p`
    /// itself and triangle 0 for an empty surface.
    pub fn closest_point_exact(&self, p: Vec3) -> (Vec3, usize) {
        self.find_nearest_surface_point_with(p, NearestSearch::Exact, TriangleProjection::Ericson, None)
            .map_or((p, 0), |h| (h.point, h.triangle))
    }

    /// `findCubeForPosition`: walk from the root toward `cc`; returns the
    /// cube at `cc`'s level, or the **coarser leaf** covering it if the walk
    /// reaches a leaf first. `None` outside the root.
    pub fn find_cube_for_position(&self, cc: &CubeCoordinates) -> Option<usize> {
        if out_of_range(cc, self.quadtree_root_z()) {
            return None;
        }
        let mut ci = ROOT;
        for i in (0..cc.level as i64).rev() {
            match self.sub_cubes(ci) {
                Some(subs) => {
                    let lim = 1i64 << i;
                    let mut sc = 0;
                    if cc.x & lim != 0 {
                        sc |= 1;
                    }
                    if cc.y & lim != 0 {
                        sc |= 2;
                    }
                    if !self.is_quadtree && cc.z & lim != 0 {
                        sc |= 4;
                    }
                    ci = subs[sc];
                }
                None => break,
            }
        }
        Some(ci)
    }

    /// `findLeafLabelForPosition`: the leaf label of
    /// [`MeshOctree::find_cube_for_position`] when that cube is a leaf.
    pub fn find_leaf_label_for_position(&self, cc: &CubeCoordinates) -> Option<usize> {
        self.find_cube_for_position(cc).and_then(|c| if self.cube(c).is_leaf() { self.cube(c).cube_label } else { None })
    }

    /// `findLeafContainingVertex`: descend by comparing `p` with the cube
    /// centres (`>=` goes to the upper child). `None` if `p` is outside the
    /// root cube (upstream aborts).
    pub fn find_leaf_containing_vertex(&self, p: Vec3) -> Option<usize> {
        if !self.cube(ROOT).coords.is_vertex_inside(&self.root_box, p) {
            return None;
        }
        let mut ci = ROOT;
        while let Some(subs) = self.sub_cubes(ci) {
            let c = self.cube(ci).coords.centre(&self.root_box);
            let mut sc = 0;
            if p.x >= c.x {
                sc |= 1;
            }
            if p.y >= c.y {
                sc |= 2;
            }
            if !self.is_quadtree && p.z >= c.z {
                sc |= 4;
            }
            ci = subs[sc];
        }
        self.cube(ci).cube_label
    }

    /// Labels of the leaf children of refined cube `ci` (`None` where the
    /// child is itself refined, as upstream's `-1`).
    fn child_labels(&self, ci: usize) -> [Option<usize>; 8] {
        let mut sc = [None; 8];
        if let Some(subs) = self.sub_cubes(ci) {
            for (i, s) in subs.iter().enumerate() {
                if *s != NO_CUBE {
                    sc[i] = self.cube(*s).cube_label;
                }
            }
        }
        sc
    }

    /// `findNeighbourOverNode`: the leaf across node `node` (0–7) of the cube
    /// at `cc`: the same-level or coarser leaf there, or, if that cube is
    /// refined, its child touching the node.
    pub fn find_neighbour_over_node(&self, cc: &CubeCoordinates, node: usize) -> Option<usize> {
        if self.is_quadtree {
            return None;
        }
        let nc = cc.plus(&self.regularity_positions[18 + node]);
        let nei = self.find_cube_for_position(&nc)?;
        if self.cube(nei).is_leaf() {
            self.cube(nei).cube_label
        } else {
            self.child_labels(nei)[7 - node]
        }
    }

    /// `findNeighboursOverEdge`: append the leaves across edge `e` (0–11):
    /// one same-level-or-coarser leaf, or the two children of a refined
    /// neighbour along that edge. `None` for "outside the root".
    pub fn find_neighbours_over_edge(&self, cc: &CubeCoordinates, e: usize, out: &mut Vec<Option<usize>>) {
        if self.is_quadtree && e < 8 {
            out.push(None);
            return;
        }
        let nc = cc.plus(&self.regularity_positions[6 + e]);
        match self.find_cube_for_position(&nc) {
            None => {
                if out_of_range(&nc, self.quadtree_root_z()) {
                    out.push(None);
                }
            }
            Some(nei) if self.cube(nei).is_leaf() => out.push(self.cube(nei).cube_label),
            Some(nei) => {
                let sc = self.child_labels(nei);
                if !self.is_quadtree {
                    out.push(sc[7 - EDGE_NODES[e][1]]);
                    out.push(sc[7 - EDGE_NODES[e][0]]);
                } else {
                    // upstream appends only children that exist, and the
                    // second only when it differs from the first
                    let (a, b) = (sc[7 - EDGE_NODES[e][1]], sc[7 - EDGE_NODES[e][0]]);
                    if a.is_some() {
                        out.push(a);
                    }
                    if b.is_some() && b != a {
                        out.push(b);
                    }
                }
            }
        }
    }

    /// `findNeighboursInDirection`: append the leaves across face `dir`
    /// (`-x, +x, -y, +y, -z, +z`): one leaf, or the four children of a
    /// refined neighbour on that face.
    pub fn find_neighbours_in_direction(&self, cc: &CubeCoordinates, dir: usize, out: &mut Vec<Option<usize>>) {
        if self.is_quadtree && dir >= 4 {
            out.push(None);
            return;
        }
        let mut n = *cc;
        match dir {
            0 => n.x -= 1,
            1 => n.x += 1,
            2 => n.y -= 1,
            3 => n.y += 1,
            4 => n.z -= 1,
            _ => n.z += 1,
        }
        match self.find_cube_for_position(&n) {
            None => {
                if out_of_range(&n, self.quadtree_root_z()) {
                    out.push(None);
                }
            }
            Some(nei) if self.cube(nei).is_leaf() => out.push(self.cube(nei).cube_label),
            Some(nei) => {
                let sc = self.child_labels(nei);
                for i in 0..4 {
                    if self.is_quadtree && sc[7 - FACE_NODES[dir][i]].is_none() {
                        continue;
                    }
                    out.push(sc[7 - FACE_NODES[dir][i]]);
                }
            }
        }
    }

    /// `findNeighboursForLeaf`: neighbours over the 6 faces.
    pub fn find_neighbours_for_leaf(&self, cc: &CubeCoordinates) -> Vec<Option<usize>> {
        let mut out = Vec::new();
        let n_faces = if self.is_quadtree { 4 } else { 6 };
        for d in 0..n_faces {
            self.find_neighbours_in_direction(cc, d, &mut out);
        }
        out
    }

    /// `findAllLeafNeighbours`: neighbours over the 8 nodes, 12 edges and 6
    /// faces, in that order.
    pub fn find_all_leaf_neighbours(&self, cc: &CubeCoordinates) -> Vec<Option<usize>> {
        let mut out = Vec::new();
        if self.is_quadtree {
            // a quadtree leaf: neighbours over the 4 z edges, then 4 faces
            for e in 8..12 {
                self.find_neighbours_over_edge(cc, e, &mut out);
            }
            for d in 0..4 {
                self.find_neighbours_in_direction(cc, d, &mut out);
            }
            return out;
        }
        for n in 0..8 {
            out.push(self.find_neighbour_over_node(cc, n));
        }
        for e in 0..12 {
            self.find_neighbours_over_edge(cc, e, &mut out);
        }
        for d in 0..6 {
            self.find_neighbours_in_direction(cc, d, &mut out);
        }
        out
    }

    /// `findLeavesForCubeVertex`: the (up to 8) leaves sharing node `vrt` of
    /// leaf `leaf`, located one level finer than the leaf.
    pub fn find_leaves_for_cube_vertex(&self, leaf: usize, vrt: usize) -> [Option<usize>; 8] {
        let cc = self.return_leaf(leaf).coords.refine_for_position(vrt);
        let mut out = [None; 8];
        for (i, o) in out.iter_mut().enumerate() {
            let pos = cc.plus(&self.vrt_leaves_pos[vrt][i]);
            *o = self.find_leaf_label_for_position(&pos);
        }
        out
    }

    /// `findLeavesContainedInBox` (label form): leaf labels of the leaves
    /// whose boxes overlap `bb`, in tree order.
    pub fn find_leaves_contained_in_box(&self, bb: &BoundBox) -> Vec<usize> {
        self.leaves_in_box(bb).into_iter().filter_map(|c| self.cube(c).cube_label).collect()
    }

    /// `findLeavesInSphere`.
    pub fn find_leaves_in_sphere(&self, c: Vec3, r: f64) -> Vec<usize> {
        self.leaves_in_sphere(c, r)
    }

    /// `isPointInside`: the leaf holding `p` is flagged `INSIDE`.
    pub fn is_point_inside(&self, p: Vec3) -> bool {
        self.find_leaf_containing_vertex(p)
            .is_some_and(|l| self.return_leaf(l).cube_type & CubeType::INSIDE != 0)
    }

    /// `findTrianglesInBox`: triangles of the leaves overlapping `bb` whose
    /// nearest point to the box centre is within half the box's x extent.
    /// A triangle in several leaves is listed once per leaf, as upstream.
    pub fn find_triangles_in_box(&self, bb: &BoundBox) -> Vec<usize> {
        let c = bb.min.add(bb.max).scale(0.5);
        let d_sq = (0.5 * (bb.max.x - bb.min.x)).r_powi(2);
        let mut out = Vec::new();
        for ci in self.leaves_in_box(bb) {
            if let Some(r) = self.cube(ci).contained_elements {
                for &t in &self.slot.contained_triangles[r] {
                    let tr = self.surface.triangles[t];
                    let pts = &self.surface.points;
                    let p = help::nearest_point_on_the_triangle(pts[tr[0]], pts[tr[1]], pts[tr[2]], c);
                    if mag_sqr(p.sub(c)) < d_sq {
                        out.push(t);
                    }
                }
            }
        }
        out
    }

    /// `findEdgesInBox`: the feature edges of the leaves overlapping `bb`
    /// within half the box's x extent of its centre.
    pub fn find_edges_in_box(&self, bb: &BoundBox) -> Vec<usize> {
        let c = bb.min.add(bb.max).scale(0.5);
        let d_sq = (0.5 * (bb.max.x - bb.min.x)).r_powi(2);
        let mut out = Vec::new();
        for ci in self.leaves_in_box(bb) {
            if let Some(r) = self.cube(ci).contained_edges {
                for &e in &self.slot.contained_edges[r] {
                    let [a, b] = self.surface.edges[e];
                    let p = help::nearest_point_on_the_edge_exact(self.surface.points[a], self.surface.points[b], c);
                    if mag_sqr(p.sub(c)) < d_sq {
                        out.push(e);
                    }
                }
            }
        }
        out
    }

    /// The starting search half-width of the nearest-point queries:
    /// `0.75 x` the size of the leaf holding `p`, or `searchRange_`.
    fn initial_search_size(&self, p: Vec3) -> f64 {
        match self.find_leaf_containing_vertex(p) {
            Some(l) => 0.75 * self.return_leaf(l).coords.size(&self.root_box),
            None => self.search_range,
        }
    }

    /// `findNearestSurfacePoint`, transcribed: [`NearestSearch::Upstream`]
    /// with upstream's projection. `None` where upstream warns "Could not
    /// find a boundary region".
    pub fn find_nearest_surface_point(&self, p: Vec3) -> Option<NearestHit> {
        self.find_nearest_surface_point_with(p, NearestSearch::Upstream, TriangleProjection::Upstream, None)
    }

    /// The nearest-surface-point search with the stopping rule, projection
    /// and an optional region filter (`findNearestSurfacePointInRegion`)
    /// chosen by the caller. See the module docs.
    pub fn find_nearest_surface_point_with(
        &self,
        p: Vec3,
        search: NearestSearch,
        proj: TriangleProjection,
        region: Option<usize>,
    ) -> Option<NearestHit> {
        let mut s = self.initial_search_size(p);
        if search == NearestSearch::Exact && s <= 0.0 {
            // Outside the root with no search range: start from the
            // distance to the root box (upstream would loop at zero width).
            let q = vmax3(self.root_box.min, vmin3(self.root_box.max, p));
            s = q.sub(p).length() + self.root_box.span().x;
        }
        // Upstream gives up after 100 doublings (5 for the region search).
        let max_iter = if region.is_some() { 5 } else { 100 };
        let root_extent = self.root_box.span().length();
        let mut iter = 0;
        loop {
            let half = Vec3::new(s, s, s);
            let bb = BoundBox::new(p.sub(half), p.add(half));
            let mut best: Option<NearestHit> = None;
            for ci in self.leaves_in_box(&bb) {
                let Some(r) = self.cube(ci).contained_elements else {
                    continue;
                };
                for &t in &self.slot.contained_triangles[r] {
                    if region.is_some_and(|rg| self.surface.regions[t] != rg) {
                        continue;
                    }
                    let tr = self.surface.triangles[t];
                    let pts = &self.surface.points;
                    let q = project(proj, pts[tr[0]], pts[tr[1]], pts[tr[2]], p);
                    let d = mag_sqr(q.sub(p));
                    let better = match (&best, search) {
                        (None, _) => d < VGREAT,
                        (Some(b), NearestSearch::Upstream) => d < b.dist_sq,
                        (Some(b), NearestSearch::Exact) => d < b.dist_sq || (d == b.dist_sq && t < b.triangle),
                    };
                    if better {
                        best = Some(NearestHit { point: q, dist_sq: d, triangle: t, region: self.surface.regions[t] });
                    }
                }
            }
            match search {
                NearestSearch::Upstream => {
                    if best.is_some() {
                        return best;
                    }
                    iter += 1;
                    if iter > max_iter {
                        return None;
                    }
                }
                NearestSearch::Exact => {
                    if let Some(b) = best {
                        // Any triangle nearer than s crosses the box.
                        if b.dist_sq <= s * s || 2.0 * s > 4.0 * root_extent {
                            return best;
                        }
                    } else if 2.0 * s > 4.0 * root_extent {
                        return None;
                    }
                }
            }
            s *= 2.0;
        }
    }

    /// `findNearestEdgePoint`: the nearest point on a feature edge whose
    /// facets are all in `regions`. Up to 3 searches, doubling the box.
    pub fn find_nearest_edge_point(&self, p: Vec3, regions: &[usize]) -> Option<(Vec3, f64, usize)> {
        let mut s = self.initial_search_size(p);
        let mut best: Option<(Vec3, f64, usize)> = None;
        for _ in 0..3 {
            let half = Vec3::new(s, s, s);
            let bb = BoundBox::new(p.sub(half), p.add(half));
            for ci in self.leaves_in_box(&bb) {
                let Some(r) = self.cube(ci).contained_edges else {
                    continue;
                };
                for &e in &self.slot.contained_edges[r] {
                    if !self.surface.edge_facets[e].iter().all(|&f| regions.contains(&self.surface.regions[f])) {
                        continue;
                    }
                    let [a, b] = self.surface.edges[e];
                    let np = help::nearest_point_on_the_edge_exact(self.surface.points[a], self.surface.points[b], p);
                    let d = mag_sqr(np.sub(p));
                    if d < best.map_or(VGREAT, |x| x.1) {
                        best = Some((np, d, e));
                    }
                }
            }
            if best.is_some() {
                break;
            }
            s *= 2.0;
        }
        best
    }

    /// `findNearestPointToEdge`: the feature edge between regions
    /// `regions[0]` and `regions[1]` nearest to the line through
    /// `edge_points`, searched in a box of `0.75 x` the edge length about its
    /// midpoint. Returns (point on the surface edge, squared distance, edge).
    pub fn find_nearest_point_to_edge(&self, edge_points: [Vec3; 2], regions: [usize; 2]) -> Option<(Vec3, f64, usize)> {
        let c = edge_points[0].add(edge_points[1]).scale(0.5);
        let dst = edge_points[0].sub(edge_points[1]).length();
        let half = Vec3::new(dst, dst, dst).scale(0.75);
        let bb = BoundBox::new(c.sub(half), c.add(half));
        let mut best: Option<(Vec3, f64, usize)> = None;
        for ci in self.leaves_in_box(&bb) {
            let Some(r) = self.cube(ci).contained_edges else {
                continue;
            };
            for &e in &self.slot.contained_edges[r] {
                let ef = &self.surface.edge_facets[e];
                if ef.len() != 2 {
                    continue;
                }
                let (r0, r1) = (self.surface.regions[ef[0]], self.surface.regions[ef[1]]);
                if !((r0 == regions[0] && r1 == regions[1]) || (r1 == regions[0] && r0 == regions[1])) {
                    continue;
                }
                let [a, b] = self.surface.edges[e];
                if let Some((on_edge, on_line)) = nearest_edge_point_to_the_line(
                    self.surface.points[a],
                    self.surface.points[b],
                    edge_points[0],
                    edge_points[1],
                ) {
                    let d = mag_sqr(on_edge.sub(on_line));
                    if d < best.map_or(VGREAT, |x| x.1) {
                        best = Some((on_edge, d, e));
                    }
                }
            }
        }
        best
    }

    /// `findNearestCorner`: the nearest surface point with more than two
    /// feature edges whose regions include every one of `patches`. Up to 4
    /// searches, doubling the box.
    pub fn find_nearest_corner(&self, p: Vec3, patches: &[usize]) -> Option<(Vec3, f64, usize)> {
        let mut s = self.initial_search_size(p);
        let surf = &self.surface;
        let mut best: Option<(Vec3, f64, usize)> = None;
        for _ in 0..4 {
            let half = Vec3::new(s, s, s);
            let bb = BoundBox::new(p.sub(half), p.add(half));
            let mut checked = std::collections::BTreeSet::new();
            for ci in self.leaves_in_box(&bb) {
                let Some(r) = self.cube(ci).contained_elements else {
                    continue;
                };
                for &t in &self.slot.contained_triangles[r] {
                    for &sp in &surf.triangles[t] {
                        if !checked.insert(sp) {
                            continue;
                        }
                        let mut node_patches: Vec<usize> = Vec::new();
                        let mut n_edges = 0;
                        for &e in &surf.point_edges[sp] {
                            let ef = &surf.edge_facets[e];
                            if ef.len() != 2 {
                                break;
                            }
                            let (a, b) = (surf.regions[ef[0]], surf.regions[ef[1]]);
                            if a != b {
                                n_edges += 1;
                                for rg in [a, b] {
                                    if !node_patches.contains(&rg) {
                                        node_patches.push(rg);
                                    }
                                }
                            }
                        }
                        if n_edges > 2 && patches.iter().filter(|q| node_patches.contains(q)).count() >= patches.len() {
                            let d = mag_sqr(surf.points[sp].sub(p));
                            if d < best.map_or(VGREAT, |x| x.1) {
                                best = Some((surf.points[sp], d, sp));
                            }
                        }
                    }
                }
            }
            if best.is_some() {
                break;
            }
            s *= 2.0;
        }
        best
    }

    /// `findNearestPointToPatches`: iterate the average of the nearest
    /// points in each of `patches` (at most 40 times, stopping when the move
    /// is below `tol x` the squared distance). Returns (point, squared
    /// distance to `p`).
    pub fn find_nearest_point_to_patches(&self, p: Vec3, patches: &[usize], tol: f64) -> Option<(Vec3, f64)> {
        if patches.is_empty() {
            return None;
        }
        let mut nearest = p;
        let mut dist_sq = 0.0;
        for _ in 0..40 {
            let mut np = Vec3::ZERO;
            for &pt in patches {
                // Upstream adds whatever the region search left in `np`; a
                // failed search leaves it unset there. Here it adds `nearest`.
                let q = self
                    .find_nearest_surface_point_with(nearest, NearestSearch::Upstream, TriangleProjection::Upstream, Some(pt))
                    .map_or(nearest, |h| h.point);
                np = np.add(q);
            }
            np = np.div(patches.len() as f64);
            dist_sq = mag_sqr(np.sub(p));
            if mag_sqr(np.sub(nearest)) < tol * dist_sq {
                break;
            }
            nearest = np;
        }
        Some((nearest, dist_sq))
    }
}

fn vmin3(a: Vec3, b: Vec3) -> Vec3 {
    crate::mesh_octree::vmin(a, b)
}

fn vmax3(a: Vec3, b: Vec3) -> Vec3 {
    crate::mesh_octree::vmax(a, b)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::mesh_octree::tests::data_octree;
    use crate::mesh_octree::TriSurf;
    use crate::shapes::{cylinder_surface, sphere_surface};
    use std::sync::Arc;

    /// The 48-sided demo pipe (R 10 mm, L 60 mm), regions 0 `walls`, 1
    /// `inlet` (z = 0), 2 `outlet` (z = L), as `pipeline`'s tests label it.
    pub(crate) fn pipe(r: f64, l: f64, n: usize) -> Arc<TriSurf> {
        let (p, t) = cylinder_surface(Vec3::ZERO, r, l, n);
        let regions = t
            .iter()
            .map(|tri| {
                let z = [p[tri[0]].z, p[tri[1]].z, p[tri[2]].z];
                let flat = (z[0] - z[1]).abs() < 1e-12 && (z[1] - z[2]).abs() < 1e-12;
                match (flat, z[0] < 0.5 * l) {
                    (true, true) => 1,
                    (true, false) => 2,
                    _ => 0,
                }
            })
            .collect();
        Arc::new(TriSurf::new(p, t, Some(regions)))
    }

    /// Deterministic sample points (an LCG; `wrapping_*` on purpose) in the
    /// box `[lo, hi]`.
    pub(crate) fn samples(n: usize, lo: Vec3, hi: Vec3, seed: u64) -> Vec<Vec3> {
        let mut s = seed;
        let mut u = || {
            s = s.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            (s >> 11) as f64 / (1u64 << 53) as f64
        };
        (0..n)
            .map(|_| Vec3::new(lo.x + (hi.x - lo.x) * u(), lo.y + (hi.y - lo.y) * u(), lo.z + (hi.z - lo.z) * u()))
            .collect()
    }

    /// Two boxes touch (share at least a point) with a relative tolerance.
    fn touch(a: &BoundBox, b: &BoundBox, tol: f64) -> bool {
        a.min.x <= b.max.x + tol
            && b.min.x <= a.max.x + tol
            && a.min.y <= b.max.y + tol
            && b.min.y <= a.max.y + tol
            && a.min.z <= b.max.z + tol
            && b.min.z <= a.max.z + tol
    }

    /// V&V (gh:#1263): the octree nearest-point query against brute force on
    /// the **same points**.
    ///
    /// # Methodology
    ///
    /// Surfaces: the 48-sided pipe (R 10 mm, L 60 mm, 192 triangles) and the
    /// sphere R 3 m (24 x 48, 2 208 triangles). Octree: every leaf holding
    /// triangles refined to level 5. Points: 10 000 per surface, uniform in
    /// the surface's bounding box grown by 20 % (so inside and outside,
    /// near and far). Brute force: `snap::closest_point_on_surface` and
    /// `patches::nearest_triangle` (Ericson projection, ties to the lower
    /// index). Octree: [`NearestSearch::Exact`] with
    /// [`TriangleProjection::Ericson`], and upstream's own rule
    /// ([`MeshOctree::find_nearest_surface_point`]).
    ///
    /// **Prediction (written before the run):**
    /// - `Exact` + Ericson: nearest point and nearest triangle
    ///   **bit-identical** to the brute force for all 20 000 points (same
    ///   projection, same tie rule, and the guard makes the candidate set a
    ///   superset of every triangle that could win).
    /// - Upstream's rule: a nonzero number of misses (its stopping rule is
    ///   not exact, module docs), mostly for points far from the surface;
    ///   where it misses, its distance is larger, never smaller.
    /// - Time: the octree faster than brute force on the sphere (2 208
    ///   triangles); on the pipe (192) no prediction.
    ///
    /// # Results (2026-11-05, release build, one run)
    ///
    /// | surface | leaves | `Exact` vs brute: point / triangle diffs | upstream rule misses (worst excess distance) | brute | `Exact` | upstream rule |
    /// |---|---|---|---|---|---|---|
    /// | pipe 48 | 1 408 | 0 / 0 | 427 of 10 000 (+35.7 %) | 31 ms | 77 ms | 65 ms |
    /// | sphere | 4 936 | 0 / 0 | 734 of 10 000 (+41.4 %) | 285 ms | 382 ms | 116 ms |
    ///
    /// - Bit-identity: as predicted.
    /// - Upstream's rule: misses as predicted (4–7 % of points) and never
    ///   nearer than the exact answer (asserted). This is upstream's
    ///   behaviour, kept as the default of `find_nearest_surface_point`.
    /// - Time: **the prediction missed.** `Exact` is slower than the brute
    ///   force on the sphere too (382 against 285 ms). Not profiled; the
    ///   likely cause is that a point deep inside the sphere doubles its box
    ///   until it covers most of the surface, and a triangle stored in many
    ///   leaves is then projected once per leaf. Upstream's rule, which
    ///   stops at the first box with any triangle, is 2.5x faster than brute
    ///   force on the sphere. Times are single runs on a shared 4-core
    ///   machine.
    #[test]
    fn nearest_point_matches_brute_force_bit_for_bit() {
        for (name, surf) in [("pipe48", pipe(0.01, 0.06, 48)), ("sphere", Arc::new({
            let (p, t) = sphere_surface(Vec3::ZERO, 3.0, 24, 48);
            TriSurf::new(p, t, None)
        }))] {
            let o = data_octree(Arc::clone(&surf), 5);
            let bb = BoundBox::of_points(&surf.points);
            let g = bb.span().scale(0.2);
            let pts = samples(10_000, bb.min.sub(g), bb.max.add(g), 12345);

            let t0 = std::time::Instant::now();
            let brute: Vec<(Vec3, usize)> = pts
                .iter()
                .map(|&p| {
                    (
                        crate::snap::closest_point_on_surface(p, &surf.points, &surf.triangles),
                        crate::patches::nearest_triangle(p, &surf.points, &surf.triangles),
                    )
                })
                .collect();
            let t_brute = t0.elapsed();
            let t1 = std::time::Instant::now();
            let exact: Vec<NearestHit> = pts
                .iter()
                .map(|&p| o.find_nearest_surface_point_with(p, NearestSearch::Exact, TriangleProjection::Ericson, None).unwrap())
                .collect();
            let t_exact = t1.elapsed();
            let t2 = std::time::Instant::now();
            let up: Vec<NearestHit> = pts.iter().map(|&p| o.find_nearest_surface_point(p).unwrap()).collect();
            let t_up = t2.elapsed();

            let mut n_point_diff = 0;
            let mut n_tri_diff = 0;
            for (b, e) in brute.iter().zip(&exact) {
                if b.0.x.to_bits() != e.point.x.to_bits()
                    || b.0.y.to_bits() != e.point.y.to_bits()
                    || b.0.z.to_bits() != e.point.z.to_bits()
                {
                    n_point_diff += 1;
                }
                if b.1 != e.triangle {
                    n_tri_diff += 1;
                }
            }
            let mut n_up_miss = 0;
            let mut worst_rel: f64 = 0.0;
            for (b, u) in brute.iter().zip(&up).zip(&pts).map(|((b, u), p)| ((b.0.sub(*p).length(), u), p)).map(|(x, _)| x) {
                let du = u.dist_sq.sqrt();
                assert!(du >= b * (1.0 - 1e-12), "upstream never beats the exact nearest: {du} < {b}");
                if du > b * (1.0 + 1e-12) {
                    n_up_miss += 1;
                    worst_rel = worst_rel.max((du - b) / b);
                }
            }
            println!(
                "{name}: {} leaves; exact: {n_point_diff} point / {n_tri_diff} triangle differences; \
                 upstream rule misses {n_up_miss} of 10000 (worst +{:.3} %); time brute {:?}, exact {:?}, upstream {:?}",
                o.number_of_leaves(),
                100.0 * worst_rel,
                t_brute,
                t_exact,
                t_up
            );
            assert_eq!((n_point_diff, n_tri_diff), (0, 0), "{name}: exact octree query is bit-identical to brute force");
            let pinned = if name == "pipe48" { 427 } else { 734 };
            assert_eq!(n_up_miss, pinned, "{name}: upstream-rule misses");
        }
    }

    /// V&V (gh:#1263): the neighbour searches against geometry. After the
    /// modifier's 1-irregular refinement (every leaf within a sphere of the
    /// pipe refined once more), for every leaf the set of leaves
    /// `findAllLeafNeighbours` returns equals the set of leaves whose boxes
    /// touch it (brute force over all leaf pairs), and the face neighbours
    /// equal the leaves sharing a face of positive area. `None` entries
    /// appear only for neighbours outside the root.
    #[test]
    fn neighbour_searches_equal_geometric_adjacency() {
        let surf = pipe(0.01, 0.06, 48);
        let mut o = data_octree(surf, 4);
        let mut mark = vec![0i32; o.number_of_leaves()];
        let c = Vec3::new(0.01, 0.0, 0.03);
        for l in 0..o.number_of_leaves() {
            if o.return_leaf(l).coords.centre(&o.root_box).sub(c).length() < 0.012 {
                mark[l] = 1;
            }
        }
        o.refine_selected_boxes(&mut mark, false);
        let n = o.number_of_leaves();
        let boxes: Vec<BoundBox> = (0..n).map(|l| o.return_leaf(l).coords.cube_box(&o.root_box)).collect();
        let tol = 1e-12 * o.root_box.span().x;
        let mut max_jump = 0;
        for l in 0..n {
            let cc = o.return_leaf(l).coords;
            let all = o.find_all_leaf_neighbours(&cc);
            let got: std::collections::BTreeSet<usize> = all.iter().flatten().copied().collect();
            let brute: std::collections::BTreeSet<usize> =
                (0..n).filter(|&m| m != l && touch(&boxes[l], &boxes[m], tol)).collect();
            assert_eq!(got, brute, "leaf {l} {cc:?}");
            for m in &got {
                max_jump = max_jump.max((o.return_leaf(*m).coords.level as i32 - cc.level as i32).abs());
            }
            let faces: std::collections::BTreeSet<usize> = o.find_neighbours_for_leaf(&cc).into_iter().flatten().collect();
            let brute_faces: std::collections::BTreeSet<usize> = brute
                .iter()
                .copied()
                .filter(|&m| {
                    let (a, b) = (&boxes[l], &boxes[m]);
                    let ov = |lo1: f64, hi1: f64, lo2: f64, hi2: f64| hi1.min(hi2) - lo1.max(lo2);
                    let o3 = [ov(a.min.x, a.max.x, b.min.x, b.max.x), ov(a.min.y, a.max.y, b.min.y, b.max.y), ov(a.min.z, a.max.z, b.min.z, b.max.z)];
                    o3.iter().filter(|&&v| v > tol).count() == 2
                })
                .collect();
            assert_eq!(faces, brute_faces, "faces of leaf {l}");
            // Point location: the leaf centre is found in the leaf itself.
            assert_eq!(o.find_leaf_containing_vertex(cc.centre(&o.root_box)), Some(l));
        }
        assert!(max_jump <= 1, "1-irregular after refine_selected_boxes: jump {max_jump}");
    }

    /// V&V (gh:#1263): feature-edge, corner and region queries on the pipe.
    /// The nearest point on an `inlet`/`walls` feature edge to a point near
    /// the rim lies on the rim circle (z = 0, radius on the polygon);
    /// `findNearestPointToEdge` finds the same rim edge; the 48-sided pipe
    /// has no corners (no point has more than 2 feature edges); the region
    /// search returns a triangle of the requested region; and
    /// `findNearestPointToPatches` for {inlet, walls} converges onto the rim.
    #[test]
    fn feature_edge_corner_and_region_queries_on_the_pipe() {
        let surf = pipe(0.01, 0.06, 48);
        let o = data_octree(Arc::clone(&surf), 5);
        let p = Vec3::new(0.0095, 0.0003, 0.0007);
        let (q, _, e) = o.find_nearest_edge_point(p, &[0, 1]).expect("rim edge found");
        assert!(q.z.abs() < 1e-15 && surf.is_region_edge(e));
        let r = (q.x * q.x + q.y * q.y).sqrt();
        assert!(r > 0.01 * (std::f64::consts::PI / 48.0).cos() - 1e-12 && r <= 0.01 + 1e-12);
        let (_, _, e2) = o
            .find_nearest_point_to_edge([Vec3::new(0.0098, -0.0004, 0.0003), Vec3::new(0.0098, 0.0012, 0.0003)], [1, 0])
            .expect("edge near the line");
        assert!(surf.is_region_edge(e2));
        assert!(o.find_nearest_corner(p, &[0, 1]).is_none(), "a pipe has no corners");
        let h = o
            .find_nearest_surface_point_with(p, NearestSearch::Upstream, TriangleProjection::Upstream, Some(2))
            .expect("outlet found from near the inlet within 5 doublings");
        assert_eq!(surf.regions[h.triangle], 2);
        let (np, _) = o.find_nearest_point_to_patches(p, &[0, 1], 1e-4).unwrap();
        assert!(np.z.abs() < 2e-4 && ((np.x * np.x + np.y * np.y).sqrt() - 0.01).abs() < 2e-4, "{np:?}");
        // Triangles and edges in a box are within the box's half-width of its centre.
        let bb = BoundBox::new(Vec3::new(0.008, -0.002, -0.001), Vec3::new(0.012, 0.002, 0.003));
        assert!(!o.find_triangles_in_box(&bb).is_empty());
        assert!(!o.find_edges_in_box(&bb).is_empty());
        // The leaves at a node of a leaf contain that node.
        let l = o.find_leaf_containing_vertex(p).unwrap();
        let node = o.return_leaf(l).coords.vertices(&o.root_box)[7];
        for nl in o.find_leaves_for_cube_vertex(l, 7).into_iter().flatten() {
            assert!(o.return_leaf(nl).coords.is_vertex_inside(&o.root_box, node));
        }
        // Box search in label form equals the cube form.
        assert_eq!(o.find_leaves_contained_in_box(&bb).len(), o.leaves_in_box(&bb).len());
        assert!(!o.is_point_inside(p), "no cube is typed INSIDE before inside/outside marking");
        assert!(!o.find_leaves_in_sphere(p, 0.001).is_empty());
    }
}
