// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Dissolve and Delete. Follows the published behaviour of Blender's
// dissolve/delete operators (source/blender/bmesh/operators/bmo_dissolve.cc and
// editmesh_tools.cc `MESH_OT_delete`, github.com/blender/blender,
// GPL-2.0-or-later): dissolve vertices / edges / faces (merge the surrounding
// geometry), limited dissolve (planar cleanup), and the delete matrix
// (vertices / edges / faces / only-faces / collapse). Concepts only — no
// upstream source copied; polygon-soup rebuilds.
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

//! **Dissolve / Delete** (`op-hzs.54.13`, GH issue #37 §B).
//!
//! - [`dissolve_faces`] merges a connected set of faces into one n-gon (the
//!   union boundary). Fails (returns the mesh unchanged) if the set has a hole
//!   or a non-simple boundary.
//! - [`dissolve_edges`] dissolves each interior edge by merging its two faces.
//! - [`dissolve_vertices`] removes each vertex, merging its incident faces and
//!   dropping the vertex from the merged ring.
//! - [`limited_dissolve`] dissolves every edge whose two faces are within
//!   `angle` of coplanar — the planar cleanup pass.
//! - [`delete`] is the delete/erase matrix ([`DeleteMode`]).

use std::collections::{BTreeSet, HashMap, HashSet};

use crate::math::Vec3;
use crate::mesh::{EdgeId, FaceId, Mesh, VertexId};
use crate::topology::MeshTopology;

/// Merge a connected face set into a single n-gon. Returns the mesh unchanged
/// if `faces` is empty, disconnected, or its union boundary is not one simple
/// loop (e.g. it encloses a hole).
pub fn dissolve_faces(mesh: &Mesh, faces: &[FaceId]) -> Mesh {
    let sel: BTreeSet<usize> = faces
        .iter()
        .map(|f| f.0)
        .filter(|&f| f < mesh.face_count())
        .collect();
    if sel.len() < 2 {
        return mesh.clone();
    }
    let polys = mesh.polygons();
    let Some(ring) = union_boundary_ring(&polys, &sel) else {
        return mesh.clone();
    };

    let mut out: Vec<Vec<usize>> = polys
        .iter()
        .enumerate()
        .filter(|(fi, _)| !sel.contains(fi))
        .map(|(_, p)| p.iter().map(|v| v.0).collect())
        .collect();
    out.push(ring);
    compact(&mesh.positions(), &out)
}

/// Dissolve each interior edge in `edges` (merge its two faces). Dissolves are
/// applied in id order; an edge whose faces were already merged is skipped.
pub fn dissolve_edges(mesh: &Mesh, edges: &[EdgeId]) -> Mesh {
    let topo = MeshTopology::new(mesh);
    let mut group: Vec<usize> = (0..mesh.face_count()).collect();
    fn find(g: &mut [usize], mut x: usize) -> usize {
        while g[x] != x {
            g[x] = g[g[x]];
            x = g[x];
        }
        x
    }
    for &e in edges {
        let f = topo.edge_faces(e);
        if f.len() == 2 {
            let (a, b) = (find(&mut group, f[0].0), find(&mut group, f[1].0));
            group[a] = b;
        }
    }
    dissolve_groups(mesh, &mut group)
}

/// Dissolve each vertex in `verts`: merge its incident faces and remove the
/// vertex from the merged boundary.
pub fn dissolve_vertices(mesh: &Mesh, verts: &[VertexId]) -> Mesh {
    let topo = MeshTopology::new(mesh);
    let _vset: HashSet<usize> = verts.iter().map(|v| v.0).collect();
    let mut group: Vec<usize> = (0..mesh.face_count()).collect();
    fn find(g: &mut [usize], mut x: usize) -> usize {
        while g[x] != x {
            g[x] = g[g[x]];
            x = g[x];
        }
        x
    }

    for &v in verts {
        let f = topo.vertex_faces(v);
        for w in f.windows(2) {
            let (a, b) = (find(&mut group, w[0].0), find(&mut group, w[1].0));
            group[a] = b;
        }
    }
    // `union_boundary_ring` walks the merged boundary; a dissolved interior
    // vertex simply does not appear on it. A dissolved *boundary* vertex is
    // left on the ring as a collinear point in v1 (Blender removes it) — noted
    // as follow-up.
    dissolve_groups(mesh, &mut group)
}

/// Dissolve every interior edge whose two faces are within `angle` radians of
/// coplanar — Blender's Limited Dissolve (planar cleanup).
pub fn limited_dissolve(mesh: &Mesh, angle: f64) -> Mesh {
    let topo = MeshTopology::new(mesh);
    let cos_tol = angle.cos();
    let mut to_dissolve: Vec<EdgeId> = Vec::new();
    for e in 0..mesh.edge_count() {
        let f = topo.edge_faces(EdgeId(e));
        if f.len() == 2 {
            let n0 = mesh.face_normal(f[0]);
            let n1 = mesh.face_normal(f[1]);
            if n0.dot(n1) >= cos_tol {
                to_dissolve.push(EdgeId(e));
            }
        }
    }
    dissolve_edges(mesh, &to_dissolve)
}

/// The delete/erase matrix.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeleteMode {
    /// Remove the vertices and every edge/face using them.
    Vertices,
    /// Remove the edges and every face using them; keep the vertices.
    Edges,
    /// Remove the faces; keep their edges and vertices (leaves a hole).
    Faces,
    /// Remove the faces only (same as [`DeleteMode::Faces`] in a soup model).
    OnlyFaces,
    /// Collapse the given edges (see [`crate::merge::merge_edges`]).
    Collapse,
}

/// Apply `mode` to the given elements, returning the rebuilt mesh.
pub fn delete(
    mesh: &Mesh,
    mode: DeleteMode,
    verts: &[VertexId],
    edges: &[EdgeId],
    faces: &[FaceId],
) -> Mesh {
    let polys = mesh.polygons();
    match mode {
        DeleteMode::Vertices => {
            let dead: HashSet<usize> = verts.iter().map(|v| v.0).collect();
            let kept: Vec<Vec<usize>> = polys
                .iter()
                .filter(|p| !p.iter().any(|v| dead.contains(&v.0)))
                .map(|p| p.iter().map(|v| v.0).collect())
                .collect();
            compact(&mesh.positions(), &kept)
        }
        DeleteMode::Edges => {
            let dead_e: HashSet<(usize, usize)> = edges
                .iter()
                .filter_map(|&e| mesh.edge(e))
                .map(|ed| {
                    (
                        ed.verts[0].0.min(ed.verts[1].0),
                        ed.verts[0].0.max(ed.verts[1].0),
                    )
                })
                .collect();
            let kept: Vec<Vec<usize>> = polys
                .iter()
                .filter(|p| {
                    let n = p.len();
                    !(0..n).any(|i| {
                        let (a, b) = (p[i].0, p[(i + 1) % n].0);
                        dead_e.contains(&(a.min(b), a.max(b)))
                    })
                })
                .map(|p| p.iter().map(|v| v.0).collect())
                .collect();
            compact(&mesh.positions(), &kept)
        }
        DeleteMode::Faces | DeleteMode::OnlyFaces => {
            let dead: HashSet<usize> = faces.iter().map(|f| f.0).collect();
            let kept: Vec<Vec<usize>> = polys
                .iter()
                .enumerate()
                .filter(|(fi, _)| !dead.contains(fi))
                .map(|(_, p)| p.iter().map(|v| v.0).collect())
                .collect();
            compact(&mesh.positions(), &kept)
        }
        DeleteMode::Collapse => crate::merge::merge_edges(mesh, edges),
    }
}

// --- helpers ---

/// Merge faces sharing a union-find group into one n-gon each.
fn dissolve_groups(mesh: &Mesh, group: &mut [usize]) -> Mesh {
    fn find(g: &mut [usize], mut x: usize) -> usize {
        while g[x] != x {
            g[x] = g[g[x]];
            x = g[x];
        }
        x
    }
    let polys = mesh.polygons();
    let mut buckets: HashMap<usize, BTreeSet<usize>> = HashMap::new();
    for fi in 0..polys.len() {
        let r = find(group, fi);
        buckets.entry(r).or_default().insert(fi);
    }
    let mut out: Vec<Vec<usize>> = Vec::new();
    for members in buckets.values() {
        if members.len() == 1 {
            let fi = *members.iter().next().unwrap();
            out.push(polys[fi].iter().map(|v| v.0).collect());
        } else if let Some(ring) = union_boundary_ring(&polys, members) {
            out.push(ring);
        } else {
            for &fi in members {
                out.push(polys[fi].iter().map(|v| v.0).collect());
            }
        }
    }
    compact(&mesh.positions(), &out)
}

/// The single simple boundary loop of a connected face set, or `None` if the
/// boundary is not one loop.
fn union_boundary_ring(polys: &[Vec<VertexId>], sel: &BTreeSet<usize>) -> Option<Vec<usize>> {
    // Directed boundary edges: a→b appears once (interior edges cancel).
    let mut dir: HashMap<usize, usize> = HashMap::new();
    let mut count: HashMap<(usize, usize), i32> = HashMap::new();
    for &fi in sel {
        let p = &polys[fi];
        let n = p.len();
        for i in 0..n {
            let (a, b) = (p[i].0, p[(i + 1) % n].0);
            *count.entry((a.min(b), a.max(b))).or_default() += 1;
        }
    }
    for &fi in sel {
        let p = &polys[fi];
        let n = p.len();
        for i in 0..n {
            let (a, b) = (p[i].0, p[(i + 1) % n].0);
            if count[&(a.min(b), a.max(b))] == 1 {
                dir.insert(a, b);
            }
        }
    }
    if dir.is_empty() {
        return None;
    }
    let start = *dir.keys().next().unwrap();
    let mut ring = vec![start];
    let mut cur = start;
    loop {
        let &next = dir.get(&cur)?;
        if next == start {
            break;
        }
        if ring.contains(&next) {
            return None;
        }
        ring.push(next);
        cur = next;
        if ring.len() > dir.len() + 1 {
            return None;
        }
    }
    (ring.len() >= 3).then_some(ring)
}

fn compact(positions: &[Vec3], faces: &[Vec<usize>]) -> Mesh {
    let mut used = vec![false; positions.len()];
    for f in faces {
        for &v in f {
            if v < used.len() {
                used[v] = true;
            }
        }
    }
    let mut idx = vec![usize::MAX; positions.len()];
    let mut pos = Vec::new();
    for (i, u) in used.iter().enumerate() {
        if *u {
            idx[i] = pos.len();
            pos.push(positions[i]);
        }
    }
    let f: Vec<Vec<usize>> = faces
        .iter()
        .map(|face| face.iter().map(|&v| idx[v]).collect())
        .filter(|f: &Vec<usize>| f.len() >= 3)
        .collect();
    Mesh::from_polygons(&pos, &f)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::primitives;

    #[test]
    fn dissolve_two_grid_quads_into_one() {
        let m = primitives::grid(2, 1, 2.0); // 2 quads, 6 verts
        let d = dissolve_faces(&m, &[FaceId(0), FaceId(1)]);
        assert_eq!(d.face_count(), 1);
        // The union boundary is a hexagon — the two mid-column verts stay on
        // the ring (dissolve *faces* does not remove boundary verts).
        assert_eq!(d.vertex_count(), 6);
    }

    #[test]
    fn dissolve_edge_merges_its_faces() {
        let m = primitives::grid(2, 1, 2.0);
        let topo = MeshTopology::new(&m);
        let shared = (0..m.edge_count())
            .map(EdgeId)
            .find(|&e| topo.is_manifold_edge(e))
            .unwrap();
        let d = dissolve_edges(&m, &[shared]);
        assert_eq!(d.face_count(), 1);
    }

    #[test]
    fn limited_dissolve_flattens_a_coplanar_grid() {
        let m = primitives::grid(3, 3, 3.0); // all coplanar
        let d = limited_dissolve(&m, 0.01);
        assert_eq!(d.face_count(), 1, "9 coplanar quads → 1 n-gon");
    }

    #[test]
    fn limited_dissolve_keeps_a_cube_sharp() {
        let m = primitives::cube(2.0);
        let d = limited_dissolve(&m, 0.01);
        assert_eq!(d.face_count(), 6, "cube edges are 90°, nothing dissolves");
    }

    #[test]
    fn delete_faces_leaves_a_hole() {
        let m = primitives::cube(2.0);
        let d = delete(&m, DeleteMode::Faces, &[], &[], &[FaceId(0)]);
        assert_eq!(d.face_count(), 5);
        assert_eq!(d.vertex_count(), 8, "vertices kept");
    }

    #[test]
    fn delete_vertices_removes_incident_faces() {
        let m = primitives::cube(2.0);
        let d = delete(&m, DeleteMode::Vertices, &[VertexId(0)], &[], &[]);
        assert_eq!(d.face_count(), 3, "vertex 0 is on 3 cube faces");
    }
}
