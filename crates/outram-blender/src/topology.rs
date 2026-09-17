// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Precomputed mesh adjacency (the "radial" and "disk" queries a full BMesh
// keeps as pointer cycles). Follows the published architecture of Blender's
// bmesh adjacency walkers (source/blender/bmesh/intern/bmesh_queries.cc and
// bmesh_walkers_impl.cc, github.com/blender/blender, GPL-2.0-or-later) — the
// edge→faces radial cycle, the vertex→edges disk cycle, and the quad
// "opposite edge" step that edge-loop / edge-ring / loop-cut selection walk.
// Concepts only — no upstream source copied; this is a build-once index table.
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

//! Precomputed **mesh adjacency** — the queries [`crate::mesh::Mesh`] can only
//! answer by a scan, cached in flat `Vec`s so operators can walk topology in
//! `O(1)` per step.
//!
//! [`crate::mesh::Mesh`] deliberately omits BMesh's radial cycle (all faces on
//! an edge) and disk cycle (all edges at a vertex) — see its module docs.
//! [`MeshTopology`] builds them once from the public [`crate::mesh::Mesh`] API:
//!
//! - **edge → faces** ([`MeshTopology::edge_faces`]) — the radial cycle. 1
//!   face = boundary edge, 2 = manifold interior, >2 = non-manifold.
//! - **vertex → edges** ([`MeshTopology::vertex_edges`]) — the disk cycle
//!   (unordered here; ordering around the vertex is added when a consumer needs
//!   it).
//! - **vertex → faces** ([`MeshTopology::vertex_faces`]).
//! - the **quad opposite-edge** step ([`MeshTopology::opposite_edge_in_face`])
//!   and the loop/ring single steps ([`MeshTopology::edge_loop_step`],
//!   [`MeshTopology::edge_ring_step`]) that edge-loop / edge-ring / face-loop
//!   selection (`op-hzs.54.2`) and, later, loop cut (`op-hzs.54.5`) are built
//!   from.
//!
//! Rebuild a [`MeshTopology`] after any operator that changes topology — like
//! any index into the mesh, it goes stale.

use std::collections::HashMap;

use crate::mesh::{EdgeId, FaceId, Mesh, VertexId};

/// Precomputed adjacency for one [`Mesh`] snapshot. Build with
/// [`MeshTopology::new`]; discard and rebuild after a topology edit.
#[derive(Debug, Clone)]
pub struct MeshTopology {
    /// `edge_faces[e]` — every face incident to edge `e` (the radial cycle).
    edge_faces: Vec<Vec<FaceId>>,
    /// `vertex_edges[v]` — every edge incident to vertex `v` (the disk cycle,
    /// unordered).
    vertex_edges: Vec<Vec<EdgeId>>,
    /// `vertex_faces[v]` — every face incident to vertex `v`.
    vertex_faces: Vec<Vec<FaceId>>,
    /// `(min, max) VertexId` → `EdgeId`, for [`MeshTopology::edge_between`].
    edge_lookup: HashMap<(usize, usize), EdgeId>,
}

impl MeshTopology {
    /// Build the adjacency tables for `mesh` (one pass over its edges and
    /// faces).
    pub fn new(mesh: &Mesh) -> Self {
        let nv = mesh.vertex_count();
        let ne = mesh.edge_count();

        let mut edge_faces = vec![Vec::new(); ne];
        let mut vertex_edges = vec![Vec::new(); nv];
        let mut vertex_faces = vec![Vec::new(); nv];
        let mut edge_lookup = HashMap::with_capacity(ne);

        for e in 0..ne {
            if let Some(edge) = mesh.edge(EdgeId(e)) {
                let (a, b) = (edge.verts[0].0, edge.verts[1].0);
                if a < nv {
                    vertex_edges[a].push(EdgeId(e));
                }
                if b < nv {
                    vertex_edges[b].push(EdgeId(e));
                }
                edge_lookup.insert((a.min(b), a.max(b)), EdgeId(e));
            }
        }

        for f in 0..mesh.face_count() {
            let vs = mesh.face_vertices(FaceId(f));
            let n = vs.len();
            for i in 0..n {
                let v = vs[i].0;
                if v < nv {
                    vertex_faces[v].push(FaceId(f));
                }
                if let Some(e) = edge_lookup.get(&{
                    let (a, b) = (vs[i].0, vs[(i + 1) % n].0);
                    (a.min(b), a.max(b))
                }) {
                    edge_faces[e.0].push(FaceId(f));
                }
            }
        }

        MeshTopology {
            edge_faces,
            vertex_edges,
            vertex_faces,
            edge_lookup,
        }
    }

    /// Faces incident to edge `e` (its radial cycle). Empty for an out-of-range
    /// or wire edge.
    pub fn edge_faces(&self, e: EdgeId) -> &[FaceId] {
        self.edge_faces
            .get(e.0)
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }

    /// Edges incident to vertex `v` (its disk cycle, unordered).
    pub fn vertex_edges(&self, v: VertexId) -> &[EdgeId] {
        self.vertex_edges
            .get(v.0)
            .map(|s| s.as_slice())
            .unwrap_or(&[])
    }

    /// Faces incident to vertex `v`.
    pub fn vertex_faces(&self, v: VertexId) -> &[FaceId] {
        self.vertex_faces
            .get(v.0)
            .map(|s| s.as_slice())
            .unwrap_or(&[])
    }

    /// `true` when `e` has exactly one incident face — a mesh-boundary (open)
    /// edge.
    pub fn is_boundary_edge(&self, e: EdgeId) -> bool {
        self.edge_faces(e).len() == 1
    }

    /// `true` when `e` has exactly two incident faces — a manifold interior
    /// edge.
    pub fn is_manifold_edge(&self, e: EdgeId) -> bool {
        self.edge_faces(e).len() == 2
    }

    /// The id of the undirected edge between `a` and `b`, or `None`.
    pub fn edge_between(&self, a: VertexId, b: VertexId) -> Option<EdgeId> {
        self.edge_lookup.get(&(a.0.min(b.0), a.0.max(b.0))).copied()
    }

    /// The other endpoint of `e` given one of them, or `None` if `v` is not on
    /// `e`.
    pub fn other_end(&self, mesh: &Mesh, e: EdgeId, v: VertexId) -> Option<VertexId> {
        let edge = mesh.edge(e)?;
        if edge.verts[0] == v {
            Some(edge.verts[1])
        } else if edge.verts[1] == v {
            Some(edge.verts[0])
        } else {
            None
        }
    }

    /// `true` when face `f` is a quadrilateral (four sides) — the case the
    /// edge-loop / edge-ring walk is defined for.
    pub fn is_quad(&self, mesh: &Mesh, f: FaceId) -> bool {
        mesh.face(f).map(|face| face.len == 4).unwrap_or(false)
    }

    /// The edges of face `f` in boundary order (one per consecutive vertex
    /// pair). Empty for an out-of-range face or if an expected edge is missing.
    pub fn face_edges(&self, mesh: &Mesh, f: FaceId) -> Vec<EdgeId> {
        let vs = mesh.face_vertices(f);
        let n = vs.len();
        if n < 3 {
            return Vec::new();
        }
        let mut out = Vec::with_capacity(n);
        for i in 0..n {
            match self.edge_between(vs[i], vs[(i + 1) % n]) {
                Some(e) => out.push(e),
                None => return Vec::new(),
            }
        }
        out
    }

    /// The edge of quad `f` opposite `e` — the one two steps around the ring.
    /// `None` if `f` is not a quad or `e` is not one of its edges.
    pub fn opposite_edge_in_face(&self, mesh: &Mesh, f: FaceId, e: EdgeId) -> Option<EdgeId> {
        let fe = self.face_edges(mesh, f);
        if fe.len() != 4 {
            return None;
        }
        let i = fe.iter().position(|&x| x == e)?;
        Some(fe[(i + 2) % 4])
    }

    /// One step of an **edge loop** walk: from `edge`, pivoting about its
    /// endpoint `pivot`, the next edge of the loop — or `None` at a pole, a
    /// non-manifold vertex, or the end of an open boundary.
    ///
    /// - **Boundary edge** — follows the mesh boundary: the other boundary edge
    ///   at `pivot`, if there is exactly one.
    /// - **Interior edge** — requires `pivot` to be a regular valence-4 vertex;
    ///   the next edge is the one at `pivot` sharing **no** face with `edge`
    ///   (its "opposite" across the vertex).
    pub fn edge_loop_step(&self, edge: EdgeId, pivot: VertexId) -> Option<EdgeId> {
        let at_pivot = self.vertex_edges(pivot);
        if self.is_boundary_edge(edge) {
            let mut it = at_pivot
                .iter()
                .copied()
                .filter(|&e| e != edge && self.is_boundary_edge(e));
            let first = it.next()?;
            return it.next().is_none().then_some(first);
        }
        if at_pivot.len() != 4 {
            return None;
        }
        let here: std::collections::BTreeSet<FaceId> =
            self.edge_faces(edge).iter().copied().collect();
        at_pivot
            .iter()
            .copied()
            .find(|&e| e != edge && self.edge_faces(e).iter().all(|f| !here.contains(f)))
    }

    /// One step of an **edge ring** walk: from `edge` across quad `face`, the
    /// opposite edge of `face` plus the quad on its far side (the next `face`),
    /// or `None` when `face` is not a quad. The returned face is `None` when the
    /// opposite edge is on the boundary (the ring ends, but the opposite edge
    /// is still part of it).
    pub fn edge_ring_step(
        &self,
        mesh: &Mesh,
        edge: EdgeId,
        face: FaceId,
    ) -> Option<(EdgeId, Option<FaceId>)> {
        let opp = self.opposite_edge_in_face(mesh, face, edge)?;
        let next_face = self.edge_faces(opp).iter().copied().find(|&f| f != face);
        Some((opp, next_face))
    }
}

/// The full **edge loop** through `seed` — the chain of edges that runs
/// "straight" across regular valence-4 vertices, or follows the mesh boundary
/// when `seed` is a boundary edge. Blender's `Alt`-click edge select.
///
/// The result always contains `seed`; it is unordered. A closed loop (around a
/// cylinder, say) terminates when the walk returns to `seed`; an open loop
/// terminates at the first pole / non-manifold vertex on each side.
pub fn edge_loop(topo: &MeshTopology, mesh: &Mesh, seed: EdgeId) -> Vec<EdgeId> {
    let mut out = vec![seed];
    let Some(edge) = mesh.edge(seed) else {
        return out;
    };
    for &start in &edge.verts {
        let mut cur = seed;
        let mut pivot = start;
        while let Some(next) = topo.edge_loop_step(cur, pivot) {
            if next == seed || out.contains(&next) {
                break;
            }
            out.push(next);
            let Some(far) = topo.other_end(mesh, next, pivot) else {
                break;
            };
            cur = next;
            pivot = far;
        }
    }
    out
}

/// The full **edge ring** through `seed` — the edges "parallel" to `seed`, one
/// per quad crossed as the walk steps to each quad's opposite edge. Blender's
/// `Ctrl+Alt`-click edge select. Always contains `seed`; unordered.
pub fn edge_ring(topo: &MeshTopology, mesh: &Mesh, seed: EdgeId) -> Vec<EdgeId> {
    let mut out = vec![seed];
    for &start_face in topo.edge_faces(seed) {
        let mut cur_edge = seed;
        let mut cur_face = start_face;
        while let Some((opp, next_face)) = topo.edge_ring_step(mesh, cur_edge, cur_face) {
            if opp == seed || out.contains(&opp) {
                break;
            }
            out.push(opp);
            match next_face {
                Some(nf) => {
                    cur_edge = opp;
                    cur_face = nf;
                }
                None => break,
            }
        }
    }
    out
}

/// The **face loop** perpendicular to `seed` — the strip of quads crossed by
/// the [`edge_ring`] walk (the faces, rather than their shared edges). Blender's
/// `Alt`-click face select. Unordered; contains every face incident to `seed`.
pub fn face_loop(topo: &MeshTopology, mesh: &Mesh, seed: EdgeId) -> Vec<FaceId> {
    let mut out: Vec<FaceId> = Vec::new();
    for &start_face in topo.edge_faces(seed) {
        if !out.contains(&start_face) {
            out.push(start_face);
        }
        let mut cur_edge = seed;
        let mut cur_face = start_face;
        while let Some((opp, Some(nf))) = topo.edge_ring_step(mesh, cur_edge, cur_face) {
            if out.contains(&nf) {
                break;
            }
            out.push(nf);
            cur_edge = opp;
            cur_face = nf;
        }
    }
    out
}

/// The **boundary loop** containing `seed` — the ring of open (one-face) edges
/// around a hole or the outer border. Empty if `seed` is not a boundary edge.
/// (This is just [`edge_loop`] restricted to a boundary start, exposed
/// separately for intent.)
pub fn boundary_loop(topo: &MeshTopology, mesh: &Mesh, seed: EdgeId) -> Vec<EdgeId> {
    if !topo.is_boundary_edge(seed) {
        return Vec::new();
    }
    edge_loop(topo, mesh, seed)
}

/// A shortest **vertex path** from `from` to `to` along mesh edges, weighted by
/// edge length (Dijkstra). Returns the ordered vertex chain including both
/// ends, or an empty `Vec` if they are not connected. Blender's `Ctrl`-click
/// "Select Shortest Path" in vertex mode (geometry-distance flavour).
pub fn shortest_vertex_path(
    topo: &MeshTopology,
    mesh: &Mesh,
    from: VertexId,
    to: VertexId,
) -> Vec<VertexId> {
    use std::collections::BinaryHeap;

    if from == to {
        return vec![from];
    }
    let nv = mesh.vertex_count();
    let mut dist = vec![f64::INFINITY; nv];
    let mut prev: Vec<Option<VertexId>> = vec![None; nv];
    let mut heap: BinaryHeap<(std::cmp::Reverse<OrdF64>, VertexId)> = BinaryHeap::new();
    if from.0 >= nv {
        return Vec::new();
    }
    dist[from.0] = 0.0;
    heap.push((std::cmp::Reverse(OrdF64(0.0)), from));

    while let Some((std::cmp::Reverse(OrdF64(d)), v)) = heap.pop() {
        if d > dist[v.0] {
            continue;
        }
        if v == to {
            break;
        }
        let vp = mesh.vertex(v).map(|x| x.position);
        for &e in topo.vertex_edges(v) {
            let Some(n) = topo.other_end(mesh, e, v) else {
                continue;
            };
            if n.0 >= nv {
                continue;
            }
            let w = match (vp, mesh.vertex(n).map(|x| x.position)) {
                (Some(a), Some(b)) => b.sub(a).length(),
                _ => 1.0,
            };
            let nd = d + w;
            if nd < dist[n.0] {
                dist[n.0] = nd;
                prev[n.0] = Some(v);
                heap.push((std::cmp::Reverse(OrdF64(nd)), n));
            }
        }
    }

    if dist[to.0].is_infinite() {
        return Vec::new();
    }
    let mut path = vec![to];
    let mut cur = to;
    while let Some(p) = prev[cur.0] {
        path.push(p);
        cur = p;
    }
    path.reverse();
    path
}

/// A shortest path (fewest hops) between two elements over a simple adjacency
/// graph — used for the edge-mode and face-mode "Select Shortest Path".
/// `adjacent(x)` yields the neighbours of `x`. Returns the ordered chain
/// including both ends, or empty if disconnected.
pub fn shortest_hop_path<T, F, I>(from: T, to: T, mut adjacent: F) -> Vec<T>
where
    T: Copy + Eq + std::hash::Hash,
    F: FnMut(T) -> I,
    I: IntoIterator<Item = T>,
{
    use std::collections::{HashMap, VecDeque};

    if from == to {
        return vec![from];
    }
    let mut prev: HashMap<T, T> = HashMap::new();
    let mut queue: VecDeque<T> = VecDeque::from([from]);
    let mut done = false;
    while let Some(cur) = queue.pop_front() {
        if cur == to {
            done = true;
            break;
        }
        for n in adjacent(cur) {
            if n != from && !prev.contains_key(&n) {
                prev.insert(n, cur);
                queue.push_back(n);
            }
        }
    }
    if !done && !prev.contains_key(&to) {
        return Vec::new();
    }
    let mut path = vec![to];
    let mut cur = to;
    while let Some(&p) = prev.get(&cur) {
        path.push(p);
        cur = p;
    }
    path.reverse();
    path
}

/// `f64` wrapper with a total order, for the Dijkstra heap (mesh edge lengths
/// are always finite and non-negative here).
#[derive(Debug, Clone, Copy, PartialEq)]
struct OrdF64(f64);
impl Eq for OrdF64 {}
impl PartialOrd for OrdF64 {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for OrdF64 {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0
            .partial_cmp(&other.0)
            .unwrap_or(std::cmp::Ordering::Equal)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::primitives;

    #[test]
    fn cube_edge_has_two_faces_and_vertex_has_three_edges() {
        let m = primitives::cube(2.0);
        let t = MeshTopology::new(&m);
        for e in 0..m.edge_count() {
            assert_eq!(
                t.edge_faces(EdgeId(e)).len(),
                2,
                "closed cube: every edge is manifold"
            );
            assert!(!t.is_boundary_edge(EdgeId(e)));
        }
        for v in 0..m.vertex_count() {
            assert_eq!(
                t.vertex_edges(VertexId(v)).len(),
                3,
                "cube corner valence 3"
            );
            assert_eq!(t.vertex_faces(VertexId(v)).len(), 3);
        }
    }

    #[test]
    fn grid_boundary_edges_are_detected() {
        let m = primitives::grid(4, 4, 4.0);
        let t = MeshTopology::new(&m);
        let boundary = (0..m.edge_count())
            .filter(|&e| t.is_boundary_edge(EdgeId(e)))
            .count();
        // A 4x4 quad grid has 16 boundary edges (4 per side).
        assert_eq!(boundary, 16);
    }

    #[test]
    fn edge_loop_step_crosses_a_grid_interior_vertex() {
        // 4x4 grid: interior vertices are valence 4, so an edge loop runs
        // straight across.
        let m = primitives::grid(4, 4, 4.0);
        let t = MeshTopology::new(&m);

        // Find an interior vertex (valence 4).
        let interior = (0..m.vertex_count())
            .map(VertexId)
            .find(|&v| t.vertex_edges(v).len() == 4)
            .unwrap();
        // Any interior edge at it.
        let e = t
            .vertex_edges(interior)
            .iter()
            .copied()
            .find(|&e| t.is_manifold_edge(e))
            .unwrap();
        let next = t.edge_loop_step(e, interior);
        assert!(next.is_some(), "loop continues across a regular vertex");
        let n = next.unwrap();
        assert_ne!(n, e);
        // The next edge shares no face with e.
        let ef: std::collections::BTreeSet<_> = t.edge_faces(e).iter().copied().collect();
        assert!(t.edge_faces(n).iter().all(|f| !ef.contains(f)));
    }

    #[test]
    fn edge_ring_step_walks_a_quad_row() {
        let m = primitives::grid(5, 5, 5.0);
        let t = MeshTopology::new(&m);
        // Take face 0 and one of its edges; the ring step must land on the
        // opposite edge and the neighbouring quad.
        let f0 = FaceId(0);
        let e0 = t.face_edges(&m, f0)[0];
        let (opp, next_face) = t.edge_ring_step(&m, e0, f0).unwrap();
        assert_ne!(opp, e0);
        assert!(t.face_edges(&m, f0).contains(&opp));
        assert!(next_face.is_some(), "interior row continues");
    }

    #[test]
    fn edge_loop_spans_a_grid_row() {
        // 5x5 grid: an edge loop that runs across the grid should have exactly
        // 5 edges (one per column), terminating at the two border vertices.
        let m = primitives::grid(5, 5, 5.0);
        let t = MeshTopology::new(&m);
        // A boundary edge on the bottom row, then step inward one loop: pick an
        // interior horizontal edge. Heuristic: the first manifold edge whose
        // endpoints have equal y.
        let horizontal = (0..m.edge_count()).map(EdgeId).find(|&e| {
            let ed = m.edge(e).unwrap();
            let (a, b) = (
                m.vertex(ed.verts[0]).unwrap().position,
                m.vertex(ed.verts[1]).unwrap().position,
            );
            t.is_manifold_edge(e) && (a.y - b.y).abs() < 1e-9
        });
        let loop_edges = edge_loop(&t, &m, horizontal.unwrap());
        assert_eq!(
            loop_edges.len(),
            5,
            "a full row of 5 quads has a 5-edge loop"
        );
    }

    #[test]
    fn edge_ring_closes_around_a_cylinder() {
        // A cylinder side is a closed quad band: an edge ring around it has one
        // edge per segment and closes on itself.
        let m = primitives::cylinder(12, 1.0, 2.0);
        let t = MeshTopology::new(&m);
        // A vertical side edge: the cylinder axis is Z, so endpoints differ
        // only in z.
        let vertical = (0..m.edge_count()).map(EdgeId).find(|&e| {
            let ed = m.edge(e).unwrap();
            let (a, b) = (
                m.vertex(ed.verts[0]).unwrap().position,
                m.vertex(ed.verts[1]).unwrap().position,
            );
            t.is_manifold_edge(e)
                && (a.x - b.x).abs() < 1e-9
                && (a.y - b.y).abs() < 1e-9
                && (a.z - b.z).abs() > 0.5
        });
        let ring = edge_ring(&t, &m, vertical.unwrap());
        assert_eq!(ring.len(), 12, "one ring edge per cylinder segment");
    }

    #[test]
    fn face_loop_is_one_row_of_the_grid() {
        let m = primitives::grid(6, 6, 6.0);
        let t = MeshTopology::new(&m);
        let f0 = FaceId(0);
        let e = t.face_edges(&m, f0)[0];
        let faces = face_loop(&t, &m, e);
        assert_eq!(faces.len(), 6, "one strip of 6 quads");
    }

    #[test]
    fn shortest_vertex_path_along_a_grid_edge() {
        let m = primitives::grid(4, 4, 4.0);
        let t = MeshTopology::new(&m);
        // Corner to corner along one border: 4 edges, 5 vertices.
        let corner_a = (0..m.vertex_count())
            .map(VertexId)
            .min_by(|&a, &b| {
                let pa = m.vertex(a).unwrap().position;
                let pb = m.vertex(b).unwrap().position;
                (pa.x + pa.y).partial_cmp(&(pb.x + pb.y)).unwrap()
            })
            .unwrap();
        let corner_b = (0..m.vertex_count())
            .map(VertexId)
            .max_by(|&a, &b| {
                let pa = m.vertex(a).unwrap().position;
                let pb = m.vertex(b).unwrap().position;
                (pa.x + pa.y).partial_cmp(&(pb.x + pb.y)).unwrap()
            })
            .unwrap();
        let path = shortest_vertex_path(&t, &m, corner_a, corner_b);
        assert!(!path.is_empty());
        assert_eq!(path.first().copied(), Some(corner_a));
        assert_eq!(path.last().copied(), Some(corner_b));
        // Manhattan distance on a 4x4 grid: 8 steps → 9 vertices.
        assert_eq!(path.len(), 9);
    }

    #[test]
    fn shortest_hop_path_on_a_line_graph() {
        // 0-1-2-3-4 chain.
        let adj = |x: usize| -> Vec<usize> {
            let mut v = Vec::new();
            if x > 0 {
                v.push(x - 1);
            }
            if x < 4 {
                v.push(x + 1);
            }
            v
        };
        let p = shortest_hop_path(0usize, 4usize, adj);
        assert_eq!(p, vec![0, 1, 2, 3, 4]);
        assert!(shortest_hop_path(0usize, 9usize, |x: usize| if x < 4 {
            vec![x + 1]
        } else {
            vec![]
        })
        .is_empty());
    }
}
