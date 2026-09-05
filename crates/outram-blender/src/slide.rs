// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Edge Slide and Vertex Slide. Follows the published behaviour of Blender's
// transform-slide operators (source/blender/editors/transform/transform_mode_
// edge_slide.cc and transform_mode_vert_slide.cc, github.com/blender/blender,
// GPL-2.0-or-later): move the vertices of an edge loop / a single vertex along
// their adjacent "rail" edges by a signed factor, with no change to topology.
// Concepts only — no upstream source copied; this is a position-only rewrite.
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

//! **Edge Slide** and **Vertex Slide** (`op-hzs.54.7`, GH issue #37 §B) — move
//! the vertices of an edge loop, or a single vertex, along their adjacent
//! **rail** edges. Topology is untouched; only positions change.
//!
//! - [`edge_slide`] takes a connected chain / loop of edges and a signed
//!   `factor` in `[-1, 1]`. Each vertex on the chain has two rail edges (the
//!   loop-perpendicular edges); `factor > 0` slides toward the rail on one
//!   consistent side of the loop, `factor < 0` toward the other. `factor = ±1`
//!   collapses the loop onto the neighbouring loop.
//! - [`vertex_slide`] moves one vertex a fraction `factor` of the way along a
//!   chosen incident edge toward its far end.
//!
//! The side used by [`edge_slide`] is fixed by propagating a "left face" along
//! the ordered chain, so the whole loop slides coherently. A vertex without
//! exactly two rails (a pole, a boundary end) is left where it is.

use std::collections::BTreeSet;

use crate::mesh::{EdgeId, Mesh, VertexId};
use crate::topology::MeshTopology;

/// Slide the vertices of the edge chain `edges` along their rails by `factor`
/// (clamped to `[-1, 1]`). Returns a new mesh with the same topology and moved
/// positions. `edges` should be edge-connected (a loop or an open run); a
/// disconnected set slides each component with its own side propagation.
pub fn edge_slide(mesh: &Mesh, edges: &[EdgeId], factor: f64) -> Mesh {
    let factor = factor.clamp(-1.0, 1.0);
    if edges.is_empty() || factor == 0.0 {
        return mesh.clone();
    }
    let topo = MeshTopology::new(mesh);
    let edge_set: BTreeSet<EdgeId> = edges.iter().copied().collect();
    let mut positions = mesh.positions();

    // Ordered vertex walk of the chain (handles a single component; a
    // disconnected set just yields several walks in turn).
    for chain in ordered_chains(mesh, &topo, &edge_set) {
        let left = propagate_left_faces(mesh, &topo, &chain);
        for (i, &v) in chain.verts.iter().enumerate() {
            let rails: Vec<EdgeId> = topo
                .vertex_edges(v)
                .iter()
                .copied()
                .filter(|e| !edge_set.contains(e))
                .collect();
            if rails.len() != 2 {
                continue;
            }
            // Side A = the rail that is an edge of this vertex's left face.
            let lf = left[i.min(left.len().saturating_sub(1))];
            let a_rail = lf
                .and_then(|f| {
                    rails
                        .iter()
                        .copied()
                        .find(|&r| topo.face_edges(mesh, f).contains(&r))
                })
                .unwrap_or(rails[0]);
            let b_rail = *rails.iter().find(|&&r| r != a_rail).unwrap_or(&rails[1]);

            let pv = positions[v.0];
            let target = if factor > 0.0 {
                topo.other_end(mesh, a_rail, v).map(|w| positions[w.0])
            } else {
                topo.other_end(mesh, b_rail, v).map(|w| positions[w.0])
            };
            if let Some(t) = target {
                positions[v.0] = pv.add(t.sub(pv).scale(factor.abs()));
            }
        }
    }

    Mesh::from_polygons(&positions, &to_soup(mesh))
}

/// Move `vert` a fraction `factor` (clamped to `[0, 1]`) of the way along
/// `along_edge` toward its far end. Returns a new mesh; topology unchanged.
pub fn vertex_slide(mesh: &Mesh, vert: VertexId, along_edge: EdgeId, factor: f64) -> Mesh {
    let factor = factor.clamp(0.0, 1.0);
    let mut positions = mesh.positions();
    let topo = MeshTopology::new(mesh);
    if let Some(far) = topo.other_end(mesh, along_edge, vert) {
        let pv = positions[vert.0];
        positions[vert.0] = pv.add(positions[far.0].sub(pv).scale(factor));
    }
    Mesh::from_polygons(&positions, &to_soup(mesh))
}

fn to_soup(mesh: &Mesh) -> Vec<Vec<usize>> {
    mesh.polygons()
        .iter()
        .map(|f| f.iter().map(|v| v.0).collect())
        .collect()
}

/// One ordered vertex walk of an edge chain.
struct Chain {
    verts: Vec<VertexId>,
    edges: Vec<EdgeId>,
}

/// Break `edge_set` into ordered chains (open runs and loops). Each chain's
/// `verts[i]`–`verts[i+1]` is `edges[i]`.
fn ordered_chains(mesh: &Mesh, topo: &MeshTopology, edge_set: &BTreeSet<EdgeId>) -> Vec<Chain> {
    let mut remaining: BTreeSet<EdgeId> = edge_set.clone();
    let mut chains = Vec::new();

    while let Some(&start) = remaining.iter().next() {
        // Prefer starting from an endpoint of degree 1 within the set.
        let start_edge = remaining
            .iter()
            .copied()
            .find(|&e| {
                mesh.edge(e).is_some_and(|ed| {
                    ed.verts.iter().any(|&v| {
                        topo.vertex_edges(v)
                            .iter()
                            .filter(|x| edge_set.contains(x))
                            .count()
                            == 1
                    })
                })
            })
            .unwrap_or(start);

        let ed = mesh.edge(start_edge).unwrap();
        let (mut verts, mut edges) = (vec![ed.verts[0], ed.verts[1]], vec![start_edge]);
        remaining.remove(&start_edge);

        // Extend forward from the last vertex.
        loop {
            let last = *verts.last().unwrap();
            let Some(next) = topo
                .vertex_edges(last)
                .iter()
                .copied()
                .find(|e| remaining.contains(e))
            else {
                break;
            };
            let far = topo.other_end(mesh, next, last).unwrap();
            verts.push(far);
            edges.push(next);
            remaining.remove(&next);
        }
        chains.push(Chain { verts, edges });
    }
    chains
}

/// For each vertex of `chain`, a "left" face fixed by propagation from the
/// first edge, so the slide direction is consistent along the whole chain.
fn propagate_left_faces(
    mesh: &Mesh,
    topo: &MeshTopology,
    chain: &Chain,
) -> Vec<Option<crate::mesh::FaceId>> {
    let mut out: Vec<Option<crate::mesh::FaceId>> = Vec::with_capacity(chain.verts.len());
    let mut prev_left = topo.edge_faces(chain.edges[0]).first().copied();
    out.push(prev_left);
    for w in 1..chain.verts.len() {
        let e = chain.edges[(w - 1).min(chain.edges.len() - 1)];
        let faces = topo.edge_faces(e);
        let left = faces
            .iter()
            .copied()
            .find(|&f| {
                prev_left.is_some_and(|pl| {
                    f == pl
                        || topo
                            .face_edges(mesh, f)
                            .iter()
                            .any(|fe| topo.face_edges(mesh, pl).contains(fe))
                })
            })
            .or_else(|| faces.first().copied());
        out.push(left);
        prev_left = left;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::primitives;
    use crate::topology;

    fn mid_vertical_loop(m: &Mesh) -> Vec<EdgeId> {
        let topo = MeshTopology::new(m);
        // The vertical edge on the centre column.
        let seed = (0..m.edge_count())
            .map(EdgeId)
            .find(|&e| {
                let ed = m.edge(e).unwrap();
                let (a, b) = (
                    m.vertex(ed.verts[0]).unwrap().position,
                    m.vertex(ed.verts[1]).unwrap().position,
                );
                topo.is_manifold_edge(e) && (a.x).abs() < 1e-9 && (b.x).abs() < 1e-9
            })
            .unwrap();
        topology::edge_loop(&topo, m, seed)
    }

    #[test]
    fn edge_slide_moves_a_grid_loop_coherently() {
        let m = primitives::grid(2, 2, 4.0); // columns at x = -2, 0, 2
        let loop_edges = mid_vertical_loop(&m);
        let slid = edge_slide(&m, &loop_edges, 0.5);

        // Every vertex that was at x = 0 should have moved the same direction
        // in x (toward one rail), and by the same amount (0.5 * 2.0 = 1.0).
        let moved: Vec<f64> = (0..m.vertex_count())
            .filter(|&i| m.vertex(VertexId(i)).unwrap().position.x.abs() < 1e-9)
            .map(|i| slid.vertex(VertexId(i)).unwrap().position.x)
            .collect();
        assert!(!moved.is_empty());
        assert!(
            moved.iter().all(|&x| (x - moved[0]).abs() < 1e-9),
            "loop slid coherently"
        );
        assert!((moved[0].abs() - 1.0).abs() < 1e-9, "0.5 of a 2.0 rail");
    }

    #[test]
    fn edge_slide_sign_flips_direction() {
        let m = primitives::grid(2, 2, 4.0);
        let loop_edges = mid_vertical_loop(&m);
        let pos = edge_slide(&m, &loop_edges, 0.5);
        let neg = edge_slide(&m, &loop_edges, -0.5);
        let x_of = |mm: &Mesh| {
            (0..m.vertex_count())
                .find(|&i| m.vertex(VertexId(i)).unwrap().position.x.abs() < 1e-9)
                .map(|i| mm.vertex(VertexId(i)).unwrap().position.x)
                .unwrap()
        };
        assert!(
            (x_of(&pos) + x_of(&neg)).abs() < 1e-9,
            "opposite factors → opposite slides"
        );
    }

    #[test]
    fn edge_slide_zero_is_identity() {
        let m = primitives::grid(2, 2, 4.0);
        let loop_edges = mid_vertical_loop(&m);
        let same = edge_slide(&m, &loop_edges, 0.0);
        for i in 0..m.vertex_count() {
            let a = m.vertex(VertexId(i)).unwrap().position;
            let b = same.vertex(VertexId(i)).unwrap().position;
            assert!(b.sub(a).length() < 1e-12);
        }
    }

    #[test]
    fn vertex_slide_moves_to_the_midpoint() {
        let m = primitives::grid(1, 1, 2.0);
        let e = EdgeId(0);
        let ed = m.edge(e).unwrap();
        let slid = vertex_slide(&m, ed.verts[0], e, 0.5);
        let mid = m
            .vertex(ed.verts[0])
            .unwrap()
            .position
            .add(m.vertex(ed.verts[1]).unwrap().position)
            .scale(0.5);
        assert!(slid.vertex(ed.verts[0]).unwrap().position.sub(mid).length() < 1e-12);
    }
}
