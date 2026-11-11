// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Ported from cfMesh (Creative Fields, Ltd.; Franjo Juretic), GPL-3.0:
//   meshLibrary/utilities/smoothers/geometry/meshOptimizer/tetMeshOptimisation/
//     advancedSmoothers/meshUntangler/{meshUntangler.C, meshUntangler.H,
//       meshUntanglerCutRegion.C, meshUntanglerCutRegionPoints.C,
//       meshUntanglerCutRegionEdges.C, meshUntanglerCutRegionFaces.C,
//       meshUntanglerCutRegionTieBreak.C}
//   meshLibrary/utilities/helperClasses/sortEdgesIntoChains/
//     {sortEdgesIntoChains.C, sortEdgesIntoChains.H}
// Source: the cfMesh v1.1 line as carried by github.com/DAFoam/cfmesh, commit
// e944f6ba695fcefce73ad306074f6b3c0ea432ae (2026-02-17).
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

//! cfMesh's **LP mesh untangler** (gh:#1260): put a point inside the
//! *feasible region* of its simplex, the convex set where every tetrahedron
//! of the simplex has positive volume.
//!
//! The region is the bounding box of the opposite faces cut, one plane at a
//! time, by the plane of each opposite face (keep its positive side). The
//! cut polyhedron is held as points, edges and faces-as-edge-lists
//! ([`CutRegion`], upstream `meshUntangler::cutRegion`), and a cut is the
//! usual clip: keep the points on the positive side (within `tol`), put a
//! new point where an edge crosses, close every cut face with a new edge
//! and add the cap face. When round-off makes the cut-away part of a face
//! fall in two pieces, `tieBreak` keeps the deepest one. The point goes to
//! the mean of the region's corners. If the region is empty (the simplex
//! cannot be untangled by moving this point), the point stays.
//!
//! [`sort_edges_into_chains`] (`sortEdgesIntoChains`) orders a face's edges
//! into closed vertex chains; `tieBreak` uses it.
//!
//! **Differences from upstream, all bounded and stated:** upstream's
//! `tieBreak` region walk loops for ever when every vertex of the face is
//! cut away; here it stops after one lap (it cannot happen on the path that
//! calls it: such a face has no kept edge and is not tie-broken).
//! `createPolyMeshFromRegion` (debug output) is not ported.

use crate::math::Vec3;
use std::collections::BTreeMap;

/// OpenFOAM's `SMALL` for `double`.
const SMALL: f64 = 1e-15;
/// OpenFOAM's `VSMALL` for `double`.
const VSMALL: f64 = 1e-300;
/// OpenFOAM's `VGREAT` for `double`.
const VGREAT: f64 = 1e300;

/// `cutRegion::vTypes`.
const NONE: u8 = 0;
const KEEP: u8 = 1;
const INPLANE: u8 = 2;

/// An undirected edge, as OpenFOAM's `edge` (start, end).
type Edge = (usize, usize);

/// `edge::commonVertex`.
fn common_vertex(a: Edge, b: Edge) -> Option<usize> {
    if a.0 == b.0 || a.0 == b.1 {
        Some(a.0)
    } else if a.1 == b.0 || a.1 == b.1 {
        Some(a.1)
    } else {
        None
    }
}

/// `edge::otherVertex`.
fn other_vertex(e: Edge, v: usize) -> Option<usize> {
    if e.0 == v {
        Some(e.1)
    } else if e.1 == v {
        Some(e.0)
    } else {
        None
    }
}

/// `sortEdgesIntoChains(bEdges).sortedChains()`: the closed chains the
/// edges form, each as its vertices in order. Empty when any vertex has an
/// odd number of edges (`openEdges_`). A chain is grown from a vertex with
/// exactly two edges and kept only if it closes or both ends stop at a
/// vertex of other degree; its edges are then removed (swap-remove, as
/// `DynList::removeElement`) and the next vertex is tried.
pub fn sort_edges_into_chains(edges: &[Edge]) -> Vec<Vec<usize>> {
    // createNodeLabels
    let mut new_node: BTreeMap<usize, usize> = BTreeMap::new();
    let mut n_points = 0usize;
    for &(s, e) in edges {
        for v in [s, e] {
            new_node.entry(v).or_insert_with(|| {
                n_points += 1;
                n_points - 1
            });
        }
    }
    let mut at_point: Vec<Vec<usize>> = vec![Vec::new(); n_points];
    for (ei, &(s, e)) in edges.iter().enumerate() {
        at_point[new_node[&s]].push(ei);
        at_point[new_node[&e]].push(ei);
    }
    let mut chains = Vec::new();
    if at_point.iter().any(|v| v.len() % 2 == 1) {
        return chains;
    }
    for p in 0..n_points {
        let Some(chain_edges) = chain_through(edges, &new_node, &at_point, p) else { continue };
        // createChainFromEdges
        if let Some(first) = chain_edges.iter().position(|&c| c) {
            let mut pts = vec![edges[first].0, edges[first].1];
            loop {
                let mut found = false;
                let last = *pts.last().unwrap();
                for &pe in &at_point[new_node[&last]] {
                    if !chain_edges[pe] {
                        continue;
                    }
                    let Some(other) = other_vertex(edges[pe], *pts.last().unwrap()) else { continue };
                    if pts[pts.len() - 2] == other || pts[0] == other {
                        continue;
                    }
                    found = true;
                    pts.push(other);
                }
                if !found {
                    break;
                }
            }
            chains.push(pts);
        }
        // shrinkEdges
        for (ei, &c) in chain_edges.iter().enumerate() {
            if c {
                for v in [edges[ei].0, edges[ei].1] {
                    let list = &mut at_point[new_node[&v]];
                    if let Some(i) = list.iter().position(|&x| x == ei) {
                        list.swap_remove(i);
                    }
                }
            }
        }
    }
    chains
}

/// `findPointsBelongingToTheChain`: the edges of the chain through the
/// node `cur` (which must have exactly two edges), or `None`.
fn chain_through(edges: &[Edge], new_node: &BTreeMap<usize, usize>, at_point: &[Vec<usize>], cur: usize) -> Option<Vec<bool>> {
    if at_point[cur].len() != 2 {
        return None;
    }
    let mut chain = vec![false; edges.len()];
    let (e0, e1) = (edges[at_point[cur][0]], edges[at_point[cur][1]]);
    let common = common_vertex(e0, e1)?;
    let mut prev = other_vertex(e0, common)?;
    let mut next = other_vertex(e1, common)?;
    for &e in &at_point[cur] {
        chain[e] = true;
    }
    for end in [&mut prev, &mut next] {
        loop {
            let mut found = false;
            let v_edges = &at_point[new_node[&*end]];
            if v_edges.len() == 2 {
                for &e in v_edges {
                    if !chain[e] {
                        found = true;
                        chain[e] = true;
                        *end = other_vertex(edges[e], *end).unwrap_or(*end);
                    }
                }
            }
            if !found {
                break;
            }
        }
    }
    if at_point[new_node[&next]].len() != 2 && at_point[new_node[&prev]].len() != 2 && prev != next {
        return None;
    }
    Some(chain)
}

/// A plane through `ref_point` with unit `normal` (OpenFOAM's `plane`
/// normalises the vector it is given).
#[derive(Debug, Clone, Copy)]
pub struct Plane {
    /// A point on the plane.
    pub ref_point: Vec3,
    /// The unit normal; the kept side is where `(p − ref) · n > 0`.
    pub normal: Vec3,
}

impl Plane {
    /// `plane(point, normal)`.
    pub fn new(ref_point: Vec3, normal: Vec3) -> Self {
        let m = normal.length();
        Plane { ref_point, normal: if m > VSMALL { normal.div(m) } else { Vec3::ZERO } }
    }
}

/// `meshUntangler::cutRegion`: a convex polyhedron as points, edges, and
/// faces given by their edges, cut by planes.
#[derive(Debug, Clone)]
pub struct CutRegion {
    points: Vec<Vec3>,
    edges: Vec<Edge>,
    faces: Vec<Vec<usize>>,
    tol: f64,
    valid: bool,
}

/// The scratch state of one cut (upstream's `c*Ptr_` and per-vertex lists).
struct Cut {
    c_pts: Vec<Vec3>,
    c_edges: Vec<Edge>,
    new_vertex: Vec<Option<usize>>,
    distance: Vec<f64>,
    types: Vec<u8>,
    new_edge: Vec<Option<usize>>,
}

impl CutRegion {
    /// `cutRegion(bb)` / `createInitialConfiguration`: the box `lo`–`hi`,
    /// `tol = SMALL |hi − lo|`.
    pub fn new(lo: Vec3, hi: Vec3) -> Self {
        let c = hi.add(lo).scale(0.5);
        let v = hi.sub(lo).scale(0.5);
        let pt = |sx: f64, sy: f64, sz: f64| Vec3::new(c.x + sx * v.x, c.y + sy * v.y, c.z + sz * v.z);
        let points = vec![
            pt(-1.0, -1.0, -1.0),
            pt(1.0, -1.0, -1.0),
            pt(1.0, 1.0, -1.0),
            pt(-1.0, 1.0, -1.0),
            pt(-1.0, -1.0, 1.0),
            pt(1.0, -1.0, 1.0),
            pt(1.0, 1.0, 1.0),
            pt(-1.0, 1.0, 1.0),
        ];
        let edges = vec![(0, 1), (3, 2), (7, 6), (4, 5), (1, 2), (0, 3), (4, 7), (5, 6), (0, 4), (1, 5), (2, 6), (3, 7)];
        let faces = vec![vec![5, 11, 6, 8], vec![4, 10, 7, 9], vec![0, 8, 3, 9], vec![1, 11, 2, 10], vec![0, 4, 1, 5], vec![3, 7, 2, 6]];
        CutRegion { points, edges, faces, tol: SMALL * hi.sub(lo).length(), valid: true }
    }

    /// The region's corners.
    pub fn points(&self) -> &[Vec3] {
        &self.points
    }

    /// The region's faces, each as edge indices.
    pub fn faces(&self) -> &[Vec<usize>] {
        &self.faces
    }

    /// The region's edges.
    pub fn edges(&self) -> &[Edge] {
        &self.edges
    }

    /// False once a tie could not be broken (upstream `valid_`); later cuts
    /// are ignored.
    pub fn is_valid(&self) -> bool {
        self.valid
    }

    /// `planeCut`: keep the part on the positive side of `plane`.
    pub fn plane_cut(&mut self, plane: &Plane) {
        if !self.valid {
            return;
        }
        let Some(mut cut) = self.find_new_vertices(plane) else { return };
        self.find_new_edges(&mut cut);
        let Some(c_faces) = self.find_new_faces(&mut cut) else { return };
        if !self.valid {
            return;
        }
        self.points = cut.c_pts;
        self.edges = cut.c_edges;
        self.faces = c_faces;
    }

    /// `findNewVertices`: classify the points; `None` when every point is
    /// kept (the plane does not cut the region).
    fn find_new_vertices(&self, plane: &Plane) -> Option<Cut> {
        let n = self.points.len();
        let mut cut = Cut { c_pts: Vec::new(), c_edges: Vec::new(), new_vertex: vec![None; n], distance: vec![0.0; n], types: vec![NONE; n], new_edge: Vec::new() };
        for (pi, &p) in self.points.iter().enumerate() {
            let d = p.sub(plane.ref_point).dot(plane.normal);
            cut.distance[pi] = d;
            if d > self.tol {
                cut.new_vertex[pi] = Some(cut.c_pts.len());
                cut.c_pts.push(p);
                cut.types[pi] |= KEEP;
            } else if d >= -self.tol {
                cut.new_vertex[pi] = Some(cut.c_pts.len());
                cut.c_pts.push(p);
                cut.types[pi] |= INPLANE;
                cut.distance[pi] = 0.0;
            }
        }
        (cut.c_pts.len() < n).then_some(cut)
    }

    /// `findNewEdges`: keep the edges with both ends kept, cut the edges
    /// with one end `KEEP` at the plane.
    fn find_new_edges(&self, cut: &mut Cut) {
        cut.c_edges.clear();
        cut.new_edge = vec![None; self.edges.len()];
        for (ei, &(start, end)) in self.edges.iter().enumerate() {
            let (ns, ne) = (cut.new_vertex[start], cut.new_vertex[end]);
            if let (Some(ns), Some(ne)) = (ns, ne) {
                cut.new_edge[ei] = Some(cut.c_edges.len());
                cut.c_edges.push((ns, ne));
            } else if let (Some(ne), true) = (ne, cut.types[end] & KEEP != 0) {
                cut.new_edge[ei] = Some(cut.c_edges.len());
                cut.c_edges.push((ne, cut.c_pts.len()));
                let t = -cut.distance[start] / (cut.distance[end] - cut.distance[start]);
                cut.c_pts.push(self.points[start].scale(1.0 - t).add(self.points[end].scale(t)));
            } else if let (Some(ns), true) = (ns, cut.types[start] & KEEP != 0) {
                cut.new_edge[ei] = Some(cut.c_edges.len());
                cut.c_edges.push((ns, cut.c_pts.len()));
                let t = -cut.distance[end] / (cut.distance[start] - cut.distance[end]);
                cut.c_pts.push(self.points[end].scale(1.0 - t).add(self.points[start].scale(t)));
            }
        }
    }

    /// `findNewFaces`: rebuild every face from its kept edges, closing it
    /// with the edge between its two singly used points, then add the cap
    /// face (the edges used once). On a face whose cut leaves more than two
    /// open points, `tieBreak` and start again.
    fn find_new_faces(&mut self, cut: &mut Cut) -> Option<Vec<Vec<usize>>> {
        loop {
            let mut finished = true;
            let mut c_faces: Vec<Vec<usize>> = Vec::new();
            for fi in 0..self.faces.len() {
                let mut usage = vec![0usize; cut.c_pts.len()];
                let mut new_face = Vec::new();
                for &e in &self.faces[fi] {
                    if let Some(el) = cut.new_edge[e] {
                        let (a, b) = cut.c_edges[el];
                        usage[a] += 1;
                        usage[b] += 1;
                        new_face.push(el);
                    }
                }
                if new_face.len() > 1 {
                    let open: Vec<usize> = (0..usage.len()).filter(|&p| usage[p] == 1).collect();
                    if open.len() == 2 {
                        new_face.push(cut.c_edges.len());
                        cut.c_edges.push((open[0], open[1]));
                    } else if open.len() > 2 {
                        let f = self.faces[fi].clone();
                        self.tie_break(&f, cut);
                        if !self.valid {
                            return None;
                        }
                        finished = false;
                        break;
                    }
                    c_faces.push(new_face);
                }
            }
            if !finished {
                continue;
            }
            let mut usage = vec![0usize; cut.c_edges.len()];
            for f in &c_faces {
                for &e in f {
                    usage[e] += 1;
                }
            }
            let in_plane: Vec<usize> = (0..usage.len()).filter(|&e| usage[e] == 1).collect();
            if in_plane.len() > 2 {
                c_faces.push(in_plane);
            }
            return Some(c_faces);
        }
    }

    /// `tieBreak(f)`: the cut-away vertices of face `f` form more than one
    /// run along it; keep the run holding the most negative distance cut
    /// away and mark the others `INPLANE`, or (one run) turn `INPLANE`
    /// vertices with no cut-away neighbour into `KEEP`; then re-make the
    /// points and edges.
    fn tie_break(&mut self, f: &[usize], cut: &mut Cut) {
        let face_edges: Vec<Edge> = f.iter().map(|&e| self.edges[e]).collect();
        let chains = sort_edges_into_chains(&face_edges);
        if chains.len() != 1 {
            self.valid = false;
            return;
        }
        let fv = &chains[0];
        let n = fv.len();
        let fc = |i: usize| (i + 1) % n;
        let rc = |i: usize| (i + n - 1) % n;
        let mut region = vec![0usize; n];
        let mut r = 1usize;
        for vi in 0..n {
            if cut.types[fv[vi]] == NONE && region[vi] == 0 {
                region[vi] = r;
                let (mut f_i, mut r_i) = (fc(vi), rc(vi));
                // Upstream loops while either side is cut away; bounded
                // here to one lap (module docs).
                for _ in 0..n {
                    let mut found = false;
                    if cut.types[fv[f_i]] == NONE {
                        region[f_i] = r;
                        f_i = fc(f_i);
                        found = true;
                    }
                    if cut.types[fv[r_i]] == NONE {
                        region[r_i] = r;
                        r_i = rc(r_i);
                        found = true;
                    }
                    if !found {
                        break;
                    }
                }
                r += 1;
            }
        }
        if r > 2 {
            let (mut min_dist, mut min_region) = (VGREAT, 0usize);
            for vi in 0..n {
                if region[vi] != 0 && cut.distance[fv[vi]] < min_dist {
                    min_dist = cut.distance[fv[vi]];
                    min_region = region[vi];
                }
            }
            for vi in 0..n {
                if region[vi] != 0 && region[vi] != min_region {
                    cut.types[fv[vi]] |= INPLANE;
                }
            }
        } else {
            for vi in 0..n {
                if cut.types[fv[vi]] & INPLANE != 0 && region[rc(vi)] == 0 && region[fc(vi)] == 0 {
                    cut.types[fv[vi]] ^= INPLANE;
                    cut.types[fv[vi]] |= KEEP;
                }
            }
        }
        cut.c_pts.clear();
        for (pi, &p) in self.points.iter().enumerate() {
            if cut.types[pi] != NONE {
                cut.new_vertex[pi] = Some(cut.c_pts.len());
                cut.c_pts.push(p);
            } else {
                cut.new_vertex[pi] = None;
            }
        }
        self.find_new_edges(cut);
    }
}

/// `meshUntangler::optimizeNodePosition`: cut the bounding box of the
/// opposite faces (`simplexSmoother::bb_`) by the plane of every opposite
/// face with a normal larger than `VSMALL`, and move `pts[free]` to the mean
/// of the region's corners. Nothing moves when the region is empty or the
/// mean is not finite. Returns whether the point moved.
pub fn mesh_untangler(pts: &mut [Vec3], tets: &[[usize; 4]], free: usize) -> bool {
    let (mut lo, mut hi) = (Vec3::new(VGREAT, VGREAT, VGREAT), Vec3::new(-VGREAT, -VGREAT, -VGREAT));
    for t in tets {
        for &v in &t[..3] {
            let p = pts[v];
            lo = Vec3::new(lo.x.min(p.x), lo.y.min(p.y), lo.z.min(p.z));
            hi = Vec3::new(hi.x.max(p.x), hi.y.max(p.y), hi.z.max(p.z));
        }
    }
    let mut cr = CutRegion::new(lo, hi);
    for t in tets {
        let (a, b, c) = (pts[t[0]], pts[t[1]], pts[t[2]]);
        let n = b.sub(a).cross(c.sub(a));
        if n.length() < VSMALL {
            continue;
        }
        cr.plane_cut(&Plane::new(a, n));
    }
    if cr.points().is_empty() {
        return false;
    }
    let mut p = Vec3::ZERO;
    for &q in cr.points() {
        p = p.add(q);
    }
    let p = p.div(cr.points().len() as f64);
    if !(p.x.is_finite() && p.y.is_finite() && p.z.is_finite()) {
        return false;
    }
    pts[free] = p;
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The cube's 12 edges (upstream's initial configuration) form 6 faces
    /// of 4 edges; `sort_edges_into_chains` on one face gives one closed
    /// chain of its 4 vertices; on two disjoint squares, two chains; on an
    /// open path, none (an odd vertex). Predictions written 2026-11-05
    /// before the first run.
    #[test]
    fn sort_edges_into_chains_closes_faces() {
        let r = CutRegion::new(Vec3::ZERO, Vec3::new(1.0, 1.0, 1.0));
        for f in r.faces() {
            let edges: Vec<Edge> = f.iter().map(|&e| r.edges()[e]).collect();
            let ch = sort_edges_into_chains(&edges);
            assert_eq!(ch.len(), 1, "{edges:?}");
            assert_eq!(ch[0].len(), 4);
            // Consecutive chain vertices share an edge.
            for i in 0..4 {
                let (a, b) = (ch[0][i], ch[0][(i + 1) % 4]);
                assert!(edges.iter().any(|&e| e == (a, b) || e == (b, a)));
            }
        }
        let two = [(0, 1), (1, 2), (2, 3), (3, 0), (10, 11), (11, 12), (12, 10)];
        let ch = sort_edges_into_chains(&two);
        assert_eq!(ch.iter().map(|c| c.len()).collect::<Vec<_>>(), vec![4, 3]);
        assert!(sort_edges_into_chains(&[(0, 1), (1, 2)]).is_empty());
    }

    /// The feasible region of the corner simplex. Methodology: a box
    /// [−1, 2]³ cut by the planes x = 0, y = 0, z = 0 (normals +x, +y, +z)
    /// and x + y + z = 1 (normal −(1, 1, 1)). Prediction (written
    /// 2026-11-05, before the first run): the region is exactly the corner
    /// tetrahedron, 4 corners (0,0,0), (1,0,0), (0,1,0), (0,0,1) to 1e-12,
    /// 6 edges, 4 faces; its centroid (¼, ¼, ¼) satisfies all four strictly.
    #[test]
    fn cut_region_of_the_corner_simplex() {
        let mut r = CutRegion::new(Vec3::new(-1.0, -1.0, -1.0), Vec3::new(2.0, 2.0, 2.0));
        r.plane_cut(&Plane::new(Vec3::ZERO, Vec3::new(1.0, 0.0, 0.0)));
        r.plane_cut(&Plane::new(Vec3::ZERO, Vec3::new(0.0, 1.0, 0.0)));
        r.plane_cut(&Plane::new(Vec3::ZERO, Vec3::new(0.0, 0.0, 1.0)));
        r.plane_cut(&Plane::new(Vec3::new(1.0, 0.0, 0.0), Vec3::new(-1.0, -1.0, -1.0)));
        assert!(r.is_valid());
        let mut pts: Vec<Vec3> = r.points().to_vec();
        println!("corners {pts:?}, {} edges, {} faces", r.edges().len(), r.faces().len());
        // Coincident corners may survive as separate points; dedupe at 1e-12.
        pts.dedup_by(|a, b| a.sub(*b).length() < 1e-12);
        let want = [Vec3::ZERO, Vec3::new(1.0, 0.0, 0.0), Vec3::new(0.0, 1.0, 0.0), Vec3::new(0.0, 0.0, 1.0)];
        for w in want {
            assert!(r.points().iter().any(|p| p.sub(w).length() < 1e-12), "{w:?} missing");
        }
        for p in r.points() {
            assert!(want.iter().any(|w| p.sub(*w).length() < 1e-12), "extra corner {p:?}");
        }
        assert_eq!(r.faces().len(), 4);
    }

    fn star() -> (Vec<Vec3>, Vec<[usize; 4]>, usize) {
        let ax = [Vec3::new(1.0, 0.0, 0.0), Vec3::new(-1.0, 0.0, 0.0), Vec3::new(0.0, 1.0, 0.0), Vec3::new(0.0, -1.0, 0.0), Vec3::new(0.0, 0.0, 1.0), Vec3::new(0.0, 0.0, -1.0)];
        let mut pts: Vec<Vec3> = ax.to_vec();
        pts.push(Vec3::ZERO);
        let free = 6;
        let vol = |a: Vec3, b: Vec3, c: Vec3, d: Vec3| b.sub(a).cross(c.sub(a)).dot(d.sub(a)) / 6.0;
        let mut tets = Vec::new();
        for &(i, j, k) in &[(0, 2, 4), (2, 1, 4), (1, 3, 4), (3, 0, 4), (2, 0, 5), (1, 2, 5), (3, 1, 5), (0, 3, 5)] {
            let mut t = [i, j, k, free];
            if vol(pts[t[0]], pts[t[1]], pts[t[2]], pts[free]) < 0.0 {
                t.swap(0, 1);
            }
            tets.push(t);
        }
        (pts, tets, free)
    }

    fn vols(pts: &[Vec3], tets: &[[usize; 4]]) -> Vec<f64> {
        tets.iter().map(|t| pts[t[1]].sub(pts[t[0]]).cross(pts[t[2]].sub(pts[t[0]])).dot(pts[t[3]].sub(pts[t[0]])) / 6.0).collect()
    }

    /// V&V (gh:#1260) — **the LP guarantee and the infeasible case.**
    ///
    /// Methodology: the octahedral star of `mesh_optimizer`'s tests, the
    /// free point pushed out to (1.4, 0, 0) (four tetrahedra inverted), and
    /// a perturbed star (the six outer points jittered by up to 0.2, the
    /// free point at (−0.9, 0.7, 0.5)). Then an infeasible star: the +x
    /// point dragged to (−2, 0, 0) so the opposite faces fold over.
    ///
    /// Predictions (written 2026-11-05, before the first run):
    /// - Both feasible stars: after `mesh_untangler` every tetrahedron has
    ///   volume > 0 (the mean of a convex polytope's corners is inside it).
    ///   On the symmetric star the point lands on the origin to 1e-12 (the
    ///   region is the octahedron itself, its corners symmetric).
    /// - Infeasible star: the region is empty, the point is left
    ///   bit-identical and the call returns `false`.
    #[test]
    fn mesh_untangler_lp_guarantee() {
        let (mut pts, tets, free) = star();
        pts[free] = Vec3::new(1.4, 0.0, 0.0);
        assert_eq!(vols(&pts, &tets).iter().filter(|&&v| v <= 0.0).count(), 4);
        assert!(mesh_untangler(&mut pts, &tets, free));
        println!("symmetric star: point -> {:?}", pts[free]);
        assert!(vols(&pts, &tets).iter().all(|&v| v > 0.0));
        assert!(pts[free].length() < 1e-12, "{:?}", pts[free]);

        let (mut pts, tets, free) = star();
        let jitter = [[0.1, -0.2, 0.05], [0.15, 0.1, -0.1], [-0.05, 0.2, 0.1], [0.2, -0.1, 0.0], [-0.1, 0.05, -0.2], [0.0, 0.15, 0.1]];
        for (p, j) in pts.iter_mut().zip(jitter) {
            *p = p.add(Vec3::new(j[0], j[1], j[2]));
        }
        pts[free] = Vec3::new(-0.9, 0.7, 0.5);
        let before = vols(&pts, &tets).iter().filter(|&&v| v <= 0.0).count();
        assert!(mesh_untangler(&mut pts, &tets, free));
        println!("perturbed star: {before} inverted -> point {:?}, min volume {:.3e}", pts[free], vols(&pts, &tets).iter().copied().fold(f64::MAX, f64::min));
        assert!(before > 0);
        assert!(vols(&pts, &tets).iter().all(|&v| v > 0.0));

        let (mut pts, tets, free) = star();
        pts[0] = Vec3::new(-2.0, 0.0, 0.0);
        pts[free] = Vec3::new(0.3, 0.1, 0.2);
        let keep = pts[free];
        let moved = mesh_untangler(&mut pts, &tets, free);
        println!("infeasible star: moved {moved}, point {:?}", pts[free]);
        assert!(!moved);
        assert_eq!(pts[free], keep);
    }
}
