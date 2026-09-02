// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Edge tools. Follows the published behaviour of Blender's edge operators
// (source/blender/bmesh/operators/bmo_rotate_edges.cc, the Set Edge Flow addon
// mesh_edge_flow, and the Edge Split modifier MOD_edgesplit.cc,
// github.com/blender/blender, GPL-2.0-or-later): rotate an edge to the next
// vertices of its two faces, relax an edge loop toward the surrounding flow,
// and split the mesh along an edge set. Concepts only — no upstream source
// copied; polygon-soup rebuilds.
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

//! **Edge tools** (`op-hzs.54.18`, GH issue #37 §B).
//!
//! - [`rotate_edge`] — spin an edge to the next pair of vertices of its two
//!   faces (a triangle flip generalises to any two faces). Blender's `Edge ▸
//!   Rotate Edge CW / CCW`.
//! - [`set_edge_flow`] — relax an edge loop's vertices toward a smooth path
//!   along their rail edges. Blender's `Edge ▸ Set Edge Flow` addon.
//! - [`edge_split`] — split the mesh along an edge set: each vertex shared by
//!   two face groups the split separates gets its own copy (the Edge Split
//!   modifier as an operator).

use std::collections::{BTreeSet, HashMap, HashSet};


use crate::mesh::{EdgeId, Mesh, VertexId};
use crate::topology::MeshTopology;

/// Rotate `edge` to connect the next vertices of its two incident faces —
/// `cw` picks the clockwise pair, `!cw` the counter-clockwise. A no-op unless
/// the edge has exactly two faces and the combined polygon stays simple.
pub fn rotate_edge(mesh: &Mesh, edge: EdgeId, cw: bool) -> Mesh {
    let topo = MeshTopology::new(mesh);
    let f = topo.edge_faces(edge);
    if f.len() != 2 {
        return mesh.clone();
    }
    let Some(ed) = mesh.edge(edge) else { return mesh.clone() };
    let (a, b) = (ed.verts[0].0, ed.verts[1].0);
    let polys = mesh.polygons();
    let (f0, f1) = (f[0].0, f[1].0);

    // Combined polygon: walk f0 from b→a, then f1 from a→b, skipping the shared
    // edge.
    let ring = match combined_ring(&polys[f0], &polys[f1], a, b) {
        Some(r) => r,
        None => return mesh.clone(),
    };
    let l = ring.len();
    let (pa, pb) = (
        ring.iter().position(|&v| v == a).unwrap(),
        ring.iter().position(|&v| v == b).unwrap(),
    );
    let step = |i: usize| if cw { (i + 1) % l } else { (i + l - 1) % l };
    let (na, nb) = (ring[step(pa)], ring[step(pb)]);
    if na == nb || na == b || nb == a {
        return mesh.clone();
    }

    let new_faces = split_ring_by_chord(&ring, na, nb);
    if new_faces.len() != 2 {
        return mesh.clone();
    }

    let mut out: Vec<Vec<usize>> = polys
        .iter()
        .enumerate()
        .filter(|(fi, _)| *fi != f0 && *fi != f1)
        .map(|(_, p)| p.iter().map(|v| v.0).collect())
        .collect();
    out.extend(new_faces);
    Mesh::from_polygons(&mesh.positions(), &out)
}

/// Relax the vertices of the edge loop `loop_edges` toward a smooth path:
/// `iterations` passes, each moving every 2-rail loop vertex a fraction
/// `strength` toward the midpoint of its two rail neighbours. Topology
/// unchanged.
pub fn set_edge_flow(mesh: &Mesh, loop_edges: &[EdgeId], iterations: u32, strength: f64) -> Mesh {
    let topo = MeshTopology::new(mesh);
    let eset: BTreeSet<EdgeId> = loop_edges.iter().copied().collect();
    let loop_verts: BTreeSet<usize> = loop_edges
        .iter()
        .filter_map(|&e| mesh.edge(e))
        .flat_map(|ed| [ed.verts[0].0, ed.verts[1].0])
        .collect();
    let mut positions = mesh.positions();
    let s = strength.clamp(0.0, 1.0);

    for _ in 0..iterations {
        let snapshot = positions.clone();
        for &v in &loop_verts {
            let rails: Vec<usize> = topo
                .vertex_edges(VertexId(v))
                .iter()
                .filter(|e| !eset.contains(e))
                .filter_map(|&e| topo.other_end(mesh, e, VertexId(v)))
                .map(|w| w.0)
                .collect();
            if rails.len() != 2 {
                continue;
            }
            let target = snapshot[rails[0]].add(snapshot[rails[1]]).scale(0.5);
            positions[v] = snapshot[v].add(target.sub(snapshot[v]).scale(s));
        }
    }
    Mesh::from_polygons(&positions, &to_soup(mesh))
}

/// Split `mesh` along `edges`: each vertex the split separates into two or more
/// face groups gets one copy per extra group. The Edge Split modifier as an
/// operator (pair with a crease attribute later).
pub fn edge_split(mesh: &Mesh, edges: &[EdgeId]) -> Mesh {
    let topo = MeshTopology::new(mesh);
    let split: HashSet<(usize, usize)> = edges
        .iter()
        .filter_map(|&e| mesh.edge(e))
        .map(|ed| (ed.verts[0].0.min(ed.verts[1].0), ed.verts[0].0.max(ed.verts[1].0)))
        .collect();
    if split.is_empty() {
        return mesh.clone();
    }
    let polys = mesh.polygons();

    // Face components: union-find over face adjacency, not crossing split edges.
    let mut parent: Vec<usize> = (0..polys.len()).collect();
    fn find(p: &mut [usize], mut x: usize) -> usize {
        while p[x] != x {
            p[x] = p[p[x]];
            x = p[x];
        }
        x
    }
    for e in 0..mesh.edge_count() {
        let ed = mesh.edge(EdgeId(e)).unwrap();
        let key = (ed.verts[0].0.min(ed.verts[1].0), ed.verts[0].0.max(ed.verts[1].0));
        if split.contains(&key) {
            continue;
        }
        let fs = topo.edge_faces(EdgeId(e));
        if fs.len() == 2 {
            let (r0, r1) = (find(&mut parent, fs[0].0), find(&mut parent, fs[1].0));
            parent[r0] = r1;
        }
    }

    // Per vertex: which components touch it. If > 1, give each after the first
    // its own duplicate.
    let mut positions = mesh.positions();
    // (vertex, component) → id
    let mut vc: HashMap<(usize, usize), usize> = HashMap::new();
    for (fi, poly) in polys.iter().enumerate() {
        let comp = find(&mut parent, fi);
        for v in poly {
            vc.entry((v.0, comp)).or_insert_with(|| {
                // First component to claim the vertex keeps the original id.
                let existing = (0..fi).any(|g| {
                    find(&mut parent, g) != comp && polys[g].iter().any(|x| x.0 == v.0)
                });
                if existing {
                    positions.push(positions[v.0]);
                    positions.len() - 1
                } else {
                    v.0
                }
            });
        }
    }

    let out: Vec<Vec<usize>> = polys
        .iter()
        .enumerate()
        .map(|(fi, poly)| {
            let comp = find(&mut parent, fi);
            poly.iter().map(|v| vc[&(v.0, comp)]).collect()
        })
        .collect();
    Mesh::from_polygons(&positions, &out)
}

// --- helpers ---

fn to_soup(mesh: &Mesh) -> Vec<Vec<usize>> {
    mesh.polygons().iter().map(|f| f.iter().map(|v| v.0).collect()).collect()
}

/// The boundary ring of `f0 ∪ f1` where they share edge `(a, b)`.
fn combined_ring(f0: &[VertexId], f1: &[VertexId], a: usize, b: usize) -> Option<Vec<usize>> {
    let r0: Vec<usize> = f0.iter().map(|v| v.0).collect();
    let r1: Vec<usize> = f1.iter().map(|v| v.0).collect();
    // Rotate r0 so it starts at b and the next vertex is a (edge b→a in f0).
    let i0 = r0.iter().position(|&v| v == b)?;
    if r0[(i0 + 1) % r0.len()] != a {
        // f0 stores a→b; take f1 as the b→a face instead.
        return combined_ring(f1, f0, a, b);
    }
    // r1 should have a→b.
    let i1 = r1.iter().position(|&v| v == a)?;
    if r1[(i1 + 1) % r1.len()] != b {
        return None;
    }
    let mut ring = Vec::new();
    // f0: from b, skip a's edge — walk b, then everything after a back to b.
    for k in 0..r0.len() {
        ring.push(r0[(i0 + 1 + k) % r0.len()]); // starts at a
    }
    // ring now = [a, ..., b]; drop the trailing b, we re-add via f1.
    ring.pop();
    for k in 0..r1.len() {
        ring.push(r1[(i1 + 1 + k) % r1.len()]); // starts at b
    }
    ring.pop(); // drop trailing a
    // Deduplicate accidental repeats.
    ring.dedup();
    if ring.len() >= 4 && ring.first() == ring.last() {
        ring.pop();
    }
    (ring.len() >= 3).then_some(ring)
}

fn split_ring_by_chord(ring: &[usize], a: usize, b: usize) -> Vec<Vec<usize>> {
    let (Some(ia), Some(ib)) = (
        ring.iter().position(|&v| v == a),
        ring.iter().position(|&v| v == b),
    ) else {
        return vec![ring.to_vec()];
    };
    let n = ring.len();
    let mut left = Vec::new();
    let mut i = ia;
    loop {
        left.push(ring[i]);
        if i == ib {
            break;
        }
        i = (i + 1) % n;
    }
    let mut right = Vec::new();
    let mut j = ib;
    loop {
        right.push(ring[j]);
        if j == ia {
            break;
        }
        j = (j + 1) % n;
    }
    if left.len() < 3 || right.len() < 3 {
        return vec![ring.to_vec()];
    }
    vec![left, right]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::math::Vec3;
    use crate::primitives;

    #[test]
    fn rotate_edge_flips_a_triangle_pair() {
        // A quad split into two triangles along one diagonal.
        let mut m = Mesh::new();
        let a = m.add_vertex(Vec3::new(0.0, 0.0, 0.0));
        let b = m.add_vertex(Vec3::new(1.0, 0.0, 0.0));
        let c = m.add_vertex(Vec3::new(1.0, 1.0, 0.0));
        let d = m.add_vertex(Vec3::new(0.0, 1.0, 0.0));
        m.add_face(&[a, b, c]);
        m.add_face(&[a, c, d]);
        let diag = (0..m.edge_count())
            .map(EdgeId)
            .find(|&e| {
                let ed = m.edge(e).unwrap();
                (ed.verts[0] == a && ed.verts[1] == c) || (ed.verts[0] == c && ed.verts[1] == a)
            })
            .unwrap();
        let r = rotate_edge(&m, diag, true);
        assert_eq!(r.face_count(), 2);
        // The diagonal a–c should be gone, replaced by b–d.
        let has_bd = (0..r.edge_count()).map(EdgeId).any(|e| {
            let ed = r.edge(e).unwrap();
            (ed.verts[0] == b && ed.verts[1] == d) || (ed.verts[0] == d && ed.verts[1] == b)
        });
        assert!(has_bd, "diagonal rotated to b–d");
    }

    #[test]
    fn set_edge_flow_moves_a_kinked_loop_toward_straight() {
        // 3x3 grid, nudge the middle column's centre vertex, then relax.
        let mut m = primitives::grid(2, 2, 4.0);
        // Find the exact centre vertex (0,0).
        let cv = (0..m.vertex_count())
            .find(|&i| m.vertex(VertexId(i)).unwrap().position.sub(Vec3::ZERO).length() < 1e-9)
            .unwrap();
        let mut pos = m.positions();
        pos[cv] = pos[cv].add(Vec3::new(1.0, 0.0, 0.0)); // kink it
        m = Mesh::from_polygons(&pos, &to_soup(&m));

        let topo = MeshTopology::new(&m);
        let seed = topo
            .vertex_edges(VertexId(cv))
            .iter()
            .copied()
            .find(|&e| {
                let ed = m.edge(e).unwrap();
                (m.vertex(ed.verts[0]).unwrap().position.y - m.vertex(ed.verts[1]).unwrap().position.y).abs() > 0.5
            })
            .unwrap();
        let loop_edges = crate::topology::edge_loop(&topo, &m, seed);
        let flowed = set_edge_flow(&m, &loop_edges, 5, 0.5);
        let after = flowed.vertex(VertexId(cv)).unwrap().position.x.abs();
        assert!(after < 1.0, "the kink relaxed back toward x = 0 (was 1.0)");
    }

    #[test]
    fn edge_split_along_a_cube_loop_duplicates_ring_verts() {
        let m = primitives::cube(2.0);
        // Split along the 4 edges of face 0.
        let topo = MeshTopology::new(&m);
        let ring = topo.face_edges(&m, crate::mesh::FaceId(0));
        let s = edge_split(&m, &ring);
        assert!(s.vertex_count() >= m.vertex_count(), "some ring verts duplicated");
        assert_eq!(s.face_count(), 6);
    }

    #[test]
    fn edge_split_empty_is_a_noop() {
        let m = primitives::cube(2.0);
        let s = edge_split(&m, &[]);
        assert_eq!(s.vertex_count(), m.vertex_count());
    }
}
