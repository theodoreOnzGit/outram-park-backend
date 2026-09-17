// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Loop Cut and Slide. Follows the published behaviour of Blender's interactive
// loop-cut tool (source/blender/editors/mesh/editmesh_loopcut.cc and the
// bmo_subdivide_edgering operator, github.com/blender/blender,
// GPL-2.0-or-later): insert one or more edge loops around the ring of a seed
// edge, splitting each quad the ring crosses, with a slide factor positioning
// the loops between the rails. Concepts only — no upstream source copied; this
// is a polygon-soup rebuild.
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

//! **Loop Cut and Slide** (`op-hzs.54.5`, GH issue #37 §B) — insert `cuts`
//! parallel edge loops around the [ring](crate::topology::edge_ring) of a seed
//! edge.
//!
//! Blender's `Ctrl+R` tool. Each quad the ring crosses is cut into `cuts + 1`
//! quads by new edges perpendicular to the ring direction; `factor` in
//! `[-1, 1]` slides the whole set of loops between the two rails
//! (`0` = evenly spaced, `±1` = the outermost loop pressed against a rail).
//!
//! Only **quad** faces are cut. The walk stops at a triangle / n-gon / pole /
//! non-manifold edge or the mesh boundary; a terminal ring edge whose far face
//! is not part of the loop still gets the new vertices spliced into that face's
//! boundary (it gains sides) so the result stays watertight — the same
//! T-junction-free behaviour as Blender when a loop cut ends at an n-gon.
//!
//! [`loop_cut`] returns the rebuilt [`Mesh`] plus, per cut, the ordered vertex
//! chain of the new loop, so a caller can select it (mirroring the "and Slide"
//! tool leaving the new loop selected).

use std::collections::{BTreeSet, HashMap};

use crate::mesh::{EdgeId, FaceId, Mesh, VertexId};
use crate::topology::{self, MeshTopology};

/// The result of [`loop_cut`]: the rebuilt mesh and the new loops it added.
#[derive(Debug, Clone)]
pub struct LoopCutResult {
    /// The mesh with the new loops inserted. Rebuilt from a polygon soup, so
    /// every id from the source mesh may have moved — remap selections against
    /// [`LoopCutResult::new_loops`].
    pub mesh: Mesh,
    /// One entry per cut (in slide order), each the ordered [`VertexId`] chain
    /// of that new edge loop in the returned [`LoopCutResult::mesh`].
    pub new_loops: Vec<Vec<VertexId>>,
    /// `true` if the ring closed on itself (a band all the way around).
    pub closed: bool,
}

/// Insert `cuts` edge loops across the ring of `seed`. `cuts == 0` returns a
/// clone with no new loops. `factor` is clamped to `[-1, 1]`.
pub fn loop_cut(mesh: &Mesh, seed: EdgeId, cuts: usize, factor: f64) -> LoopCutResult {
    if cuts == 0 || mesh.edge(seed).is_none() {
        return LoopCutResult {
            mesh: mesh.clone(),
            new_loops: Vec::new(),
            closed: false,
        };
    }
    let topo = MeshTopology::new(mesh);
    let ring = topology::edge_ring(&topo, mesh, seed);
    if ring.len() < 2 {
        return LoopCutResult {
            mesh: mesh.clone(),
            new_loops: Vec::new(),
            closed: false,
        };
    }
    let ring_set: BTreeSet<EdgeId> = ring.iter().copied().collect();
    let closed = ring_closes(&topo, &ring_set);

    let ts = cut_parameters(cuts, factor.clamp(-1.0, 1.0));

    // New vertex per (ring edge, t), placed along ed.verts[0] → ed.verts[1].
    let mut positions = mesh.positions();
    let mut split: HashMap<EdgeId, Vec<usize>> = HashMap::new();
    for &e in &ring {
        let ed = mesh.edge(e).unwrap();
        let a = positions[ed.verts[0].0];
        let b = positions[ed.verts[1].0];
        let ids: Vec<usize> = ts
            .iter()
            .map(|&t| {
                let p = a.add(b.sub(a).scale(t));
                positions.push(p);
                positions.len() - 1
            })
            .collect();
        split.insert(e, ids);
    }

    // A face is a loop quad iff it is a quad with (at least) two ring edges.
    let mut faces: Vec<Vec<usize>> = Vec::new();
    let mut loop_quads: Vec<FaceId> = Vec::new();
    for f in 0..mesh.face_count() {
        let fe = topo.face_edges(mesh, FaceId(f));
        let ring_hits = fe.iter().filter(|e| ring_set.contains(e)).count();
        if fe.len() == 4 && ring_hits >= 2 {
            loop_quads.push(FaceId(f));
        } else {
            faces.push(splice(&mesh.polygons()[f], &split, mesh));
        }
    }

    for &qf in &loop_quads {
        emit_strip(&mut faces, qf, &ring_set, &split, mesh);
    }

    // Order each new loop's vertices by walking the ring from `seed`, taking
    // each edge's split vertex from the end it shares with the previous edge so
    // the chain does not zig-zag.
    let ordered = order_ring(&topo, mesh, seed, &ring_set);
    let new_loops: Vec<Vec<VertexId>> = (0..ts.len())
        .map(|j| {
            let mut chain = Vec::with_capacity(ordered.len());
            for (i, &e) in ordered.iter().enumerate() {
                let near = if i > 0 {
                    shared_vertex(mesh, ordered[i - 1], e)
                } else {
                    None
                };
                let ids = match near {
                    Some(v) => ids_from(&split, mesh, e, v),
                    None => split.get(&e).cloned().unwrap_or_default(),
                };
                if let Some(&vid) = ids.get(j) {
                    chain.push(VertexId(vid));
                }
            }
            chain
        })
        .collect();

    LoopCutResult {
        mesh: Mesh::from_polygons(&positions, &faces),
        new_loops,
        closed,
    }
}

/// The vertex shared by two edges, if any.
fn shared_vertex(mesh: &Mesh, a: EdgeId, b: EdgeId) -> Option<VertexId> {
    let (ea, eb) = (mesh.edge(a)?, mesh.edge(b)?);
    ea.verts.iter().copied().find(|v| eb.verts.contains(v))
}

/// The `t` positions of the cuts, ascending, with the slide applied.
fn cut_parameters(cuts: usize, factor: f64) -> Vec<f64> {
    let base: Vec<f64> = (1..=cuts).map(|j| j as f64 / (cuts as f64 + 1.0)).collect();
    let room = base
        .first()
        .copied()
        .unwrap_or(0.5)
        .min(1.0 - base.last().copied().unwrap_or(0.5));
    let offset = factor * room;
    base.iter()
        .map(|&t| (t + offset).clamp(1e-4, 1.0 - 1e-4))
        .collect()
}

/// Emit the `cuts + 1` strip quads for one loop quad.
fn emit_strip(
    faces: &mut Vec<Vec<usize>>,
    qf: FaceId,
    ring_set: &BTreeSet<EdgeId>,
    split: &HashMap<EdgeId, Vec<usize>>,
    mesh: &Mesh,
) {
    let vs = mesh.face_vertices(qf);
    if vs.len() != 4 {
        return;
    }
    // Rotate so the two split (ring) edges are at boundary positions (0,1) and
    // (2,3): edges are (v0,v1),(v1,v2),(v2,v3),(v3,v0).
    let edge_at = |i: usize| topology_edge(mesh, vs[i], vs[(i + 1) % 4]);
    let split01 = edge_at(0).is_some_and(|e| ring_set.contains(&e))
        && edge_at(2).is_some_and(|e| ring_set.contains(&e));
    let vs = if split01 {
        vs.clone()
    } else {
        vec![vs[1], vs[2], vs[3], vs[0]]
    };
    let (v0, v1, v2, v3) = (vs[0], vs[1], vs[2], vs[3]);

    let ea = topology_edge(mesh, v0, v1);
    let eb = topology_edge(mesh, v2, v3);
    let (Some(ea), Some(eb)) = (ea, eb) else {
        return;
    };
    if !ring_set.contains(&ea) || !ring_set.contains(&eb) {
        return;
    }

    // P_j along v0 → v1; Q_j along v3 → v2 (same rail side as P: v0-v3).
    let ps = ids_from(split, mesh, ea, v0);
    let qs = ids_from(split, mesh, eb, v3);
    let k = ps.len().min(qs.len());
    if k == 0 {
        faces.push(vec![v0.0, v1.0, v2.0, v3.0]);
        return;
    }

    faces.push(vec![v0.0, ps[0], qs[0], v3.0]);
    for j in 0..k - 1 {
        faces.push(vec![ps[j], ps[j + 1], qs[j + 1], qs[j]]);
    }
    faces.push(vec![ps[k - 1], v1.0, v2.0, qs[k - 1]]);
}

/// The split-vertex ids for `edge`, ordered so index 0 is nearest `near_end`.
fn ids_from(
    split: &HashMap<EdgeId, Vec<usize>>,
    mesh: &Mesh,
    edge: EdgeId,
    near_end: VertexId,
) -> Vec<usize> {
    let Some(ids) = split.get(&edge) else {
        return Vec::new();
    };
    let ed = mesh.edge(edge).unwrap();
    if ed.verts[0] == near_end {
        ids.clone()
    } else {
        ids.iter().rev().copied().collect()
    }
}

/// Splice any split verts on `face`'s edges into its boundary, preserving
/// winding — for non-loop faces touched by a terminal ring edge.
fn splice(face: &[VertexId], split: &HashMap<EdgeId, Vec<usize>>, mesh: &Mesh) -> Vec<usize> {
    let n = face.len();
    let mut out = Vec::with_capacity(n + 2);
    for i in 0..n {
        let a = face[i];
        let b = face[(i + 1) % n];
        out.push(a.0);
        if let Some(e) = topology_edge(mesh, a, b) {
            for vid in ids_from(split, mesh, e, a) {
                out.push(vid);
            }
        }
    }
    out
}

/// Linear-scan edge lookup — only ever called for the handful of loop / touched
/// faces, so a `MeshTopology` build is not worth it here.
fn topology_edge(mesh: &Mesh, a: VertexId, b: VertexId) -> Option<EdgeId> {
    (0..mesh.edge_count()).map(EdgeId).find(|&e| {
        mesh.edge(e).is_some_and(|ed| {
            (ed.verts[0] == a && ed.verts[1] == b) || (ed.verts[0] == b && ed.verts[1] == a)
        })
    })
}

/// Whether the ring returns to `seed` (a closed band) rather than terminating
/// at a boundary or a non-quad — true iff every ring edge is manifold.
fn ring_closes(topo: &MeshTopology, ring: &BTreeSet<EdgeId>) -> bool {
    ring.iter().all(|&e| topo.edge_faces(e).len() == 2)
}

/// The ring edges ordered by an [`edge_ring_step`](MeshTopology::edge_ring_step)
/// walk from `seed`.
fn order_ring(
    topo: &MeshTopology,
    mesh: &Mesh,
    seed: EdgeId,
    ring: &BTreeSet<EdgeId>,
) -> Vec<EdgeId> {
    let mut out = vec![seed];
    let mut seen: BTreeSet<EdgeId> = BTreeSet::from([seed]);
    for &start_face in topo.edge_faces(seed) {
        let mut cur_edge = seed;
        let mut cur_face = start_face;
        while let Some((opp, next_face)) = topo.edge_ring_step(mesh, cur_edge, cur_face) {
            if !ring.contains(&opp) || !seen.insert(opp) {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::primitives;

    fn vertical_seed(m: &Mesh) -> EdgeId {
        let topo = MeshTopology::new(m);
        (0..m.edge_count())
            .map(EdgeId)
            .find(|&e| {
                let ed = m.edge(e).unwrap();
                let (a, b) = (
                    m.vertex(ed.verts[0]).unwrap().position,
                    m.vertex(ed.verts[1]).unwrap().position,
                );
                topo.is_manifold_edge(e) && (a.x - b.x).abs() < 1e-9 && (a.z - b.z).abs() < 1e-9
            })
            .unwrap()
    }

    #[test]
    fn single_cut_on_a_grid_row_adds_one_loop() {
        let m = primitives::grid(3, 1, 3.0);
        let seed = vertical_seed(&m);
        let r = loop_cut(&m, seed, 1, 0.0);
        assert!(!r.closed);
        assert_eq!(r.mesh.face_count(), 6, "3 quads → 6");
        assert_eq!(r.mesh.euler_characteristic(), 1);
        assert_eq!(r.new_loops.len(), 1);
        assert_eq!(r.new_loops[0].len(), 4, "one vertex per ring edge");
    }

    #[test]
    fn two_cuts_on_a_cylinder_close_the_loop() {
        let m = primitives::cylinder(8, 1.0, 2.0);
        let topo = MeshTopology::new(&m);
        let seed = (0..m.edge_count())
            .map(EdgeId)
            .find(|&e| {
                let ed = m.edge(e).unwrap();
                let (a, b) = (
                    m.vertex(ed.verts[0]).unwrap().position,
                    m.vertex(ed.verts[1]).unwrap().position,
                );
                topo.is_manifold_edge(e)
                    && (a.x - b.x).abs() < 1e-9
                    && (a.y - b.y).abs() < 1e-9
                    && (a.z - b.z).abs() > 0.5
            })
            .unwrap();
        let r = loop_cut(&m, seed, 2, 0.0);
        assert!(r.closed);
        assert_eq!(r.new_loops.len(), 2);
        assert_eq!(r.new_loops[0].len(), 8);
        assert_eq!(r.mesh.face_count(), 8 * 3 + 2);
        assert_eq!(r.mesh.euler_characteristic(), 2);
    }

    #[test]
    fn slide_factor_moves_the_loop() {
        // grid(2,1): the ring is the 3 vertical edges (constant x); the slide
        // moves the new loop *along* those edges, i.e. in y.
        let m = primitives::grid(2, 1, 2.0);
        let seed = vertical_seed(&m);
        let centered = loop_cut(&m, seed, 1, 0.0);
        let slid = loop_cut(&m, seed, 1, 0.8);
        let cy = |r: &LoopCutResult| {
            let v = r.new_loops[0][0];
            r.mesh.vertex(v).unwrap().position.y
        };
        assert!((cy(&centered)).abs() < 1e-6, "centred cut sits at y = 0");
        assert!(cy(&slid) > 0.5, "slid cut moves toward the +y rail");
    }

    #[test]
    fn zero_cuts_is_a_clone() {
        let m = primitives::grid(3, 3, 3.0);
        let seed = vertical_seed(&m);
        let r = loop_cut(&m, seed, 0, 0.0);
        assert_eq!(r.mesh.face_count(), m.face_count());
        assert!(r.new_loops.is_empty());
    }
}
