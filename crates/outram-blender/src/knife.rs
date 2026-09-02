// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Knife tool. Follows the published behaviour of Blender's knife
// (source/blender/editors/mesh/editmesh_knife.cc, github.com/blender/blender,
// GPL-2.0-or-later): cut new edges through faces along a path of points on the
// mesh surface, inserting vertices where the path crosses existing edges and
// splitting each crossed face. Concepts only — no upstream source copied; this
// is a polygon-soup rebuild driven by explicit boundary points.
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

//! **Knife** (`op-hzs.54.6`, GH issue #37 §B) — cut new edges across faces
//! along a path of [boundary points](KnifePoint).
//!
//! Blender's interactive knife resolves a screen-space polyline into a chain of
//! vertex / edge-crossing / face-interior points; this module takes that chain
//! already resolved — as a list of [`Chord`]s, each a straight cut across one
//! face between two points on its boundary — and rebuilds the mesh with every
//! crossed edge split and every crossed face divided in two.
//!
//! Resolving a raw polyline (or another object's silhouette, for **Knife
//! Project**) into [`Chord`]s is the caller's job for now; a
//! `project_polyline` helper that walks the surface is tracked as follow-up
//! under this bead.
//!
//! Each [`knife`] chord splits exactly one face. Multiple chords on the same
//! face are applied in sequence, each acting on whichever sub-face contains it.

use crate::math::Vec3;
use crate::mesh::{EdgeId, Mesh, VertexId};

/// A point on the boundary of a face — where a [`Chord`] starts or ends.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum KnifePoint {
    /// An existing vertex of the face.
    Vertex(VertexId),
    /// A new vertex `t` of the way along `edge` (from its `verts[0]` to
    /// `verts[1]`), `0 < t < 1`.
    EdgeSplit { edge: EdgeId, t: f64 },
}

/// One straight knife cut across a single face, between two points on its
/// boundary. The two points must lie on *different* sides / vertices of the
/// face (a chord, not a degenerate zero-length cut).
#[derive(Debug, Clone, Copy)]
pub struct Chord {
    /// The face to cut, by its id in the **input** mesh.
    pub face: crate::mesh::FaceId,
    /// Where the cut enters.
    pub from: KnifePoint,
    /// Where the cut leaves.
    pub to: KnifePoint,
}

/// The result of [`knife`].
#[derive(Debug, Clone)]
pub struct KnifeResult {
    /// The rebuilt mesh. Ids may have moved; the new cut vertices are listed in
    /// [`KnifeResult::cut_vertices`].
    pub mesh: Mesh,
    /// Every vertex the knife introduced or cut along, in chord order (each
    /// chord contributes its `from` then `to` vertex). Duplicates are kept so a
    /// caller can see the per-chord pairing.
    pub cut_vertices: Vec<VertexId>,
}

/// Apply `chords` to `mesh`. Chords are grouped by face; within a face they are
/// applied in the given order. A chord whose endpoints resolve to the same
/// point, or whose face is not found, is skipped.
pub fn knife(mesh: &Mesh, chords: &[Chord]) -> KnifeResult {
    let mut positions = mesh.positions();

    // Resolve each KnifePoint to a concrete vertex index, creating edge-split
    // vertices (deduplicated per (edge, rounded-t)).
    let mut edge_split_cache: std::collections::HashMap<(usize, i64), usize> =
        std::collections::HashMap::new();
    let mut resolve = |kp: KnifePoint, positions: &mut Vec<Vec3>| -> Option<usize> {
        match kp {
            KnifePoint::Vertex(v) => (v.0 < positions.len()).then_some(v.0),
            KnifePoint::EdgeSplit { edge, t } => {
                let ed = mesh.edge(edge)?;
                let t = t.clamp(1e-6, 1.0 - 1e-6);
                let key = (edge.0, (t * 1e6).round() as i64);
                if let Some(&idx) = edge_split_cache.get(&key) {
                    return Some(idx);
                }
                let a = positions[ed.verts[0].0];
                let b = positions[ed.verts[1].0];
                let idx = positions.len();
                positions.push(a.add(b.sub(a).scale(t)));
                edge_split_cache.insert(key, idx);
                Some(idx)
            }
        }
    };

    // Group chords by face and pre-resolve their endpoints.
    let mut by_face: std::collections::BTreeMap<usize, Vec<(usize, usize)>> =
        std::collections::BTreeMap::new();
    let mut cut_vertices: Vec<VertexId> = Vec::new();
    for c in chords {
        let (Some(a), Some(b)) = (
            resolve(c.from, &mut positions),
            resolve(c.to, &mut positions),
        ) else {
            continue;
        };
        if a == b {
            continue;
        }
        by_face.entry(c.face.0).or_default().push((a, b));
        cut_vertices.push(VertexId(a));
        cut_vertices.push(VertexId(b));
    }

    // Rebuild faces.
    let mut out_faces: Vec<Vec<usize>> = Vec::new();
    for (fi, face) in mesh.polygons().iter().enumerate() {
        let ring: Vec<usize> = face.iter().map(|v| v.0).collect();
        match by_face.get(&fi) {
            None => {
                // A face not cut directly may still gain a vertex where a
                // neighbouring face's edge-split lands on a shared edge.
                out_faces.push(splice_edge_splits(&ring, mesh, &edge_split_cache));
            }
            Some(cuts) => {
                let mut pieces = vec![splice_edge_splits(&ring, mesh, &edge_split_cache)];
                for &(a, b) in cuts {
                    pieces = pieces
                        .into_iter()
                        .flat_map(|p| split_ring(&p, a, b))
                        .collect();
                }
                out_faces.extend(pieces);
            }
        }
    }

    KnifeResult { mesh: Mesh::from_polygons(&positions, &out_faces), cut_vertices }
}

/// Insert any cached edge-split vertices that lie on `ring`'s edges, in order.
fn splice_edge_splits(
    ring: &[usize],
    mesh: &Mesh,
    cache: &std::collections::HashMap<(usize, i64), usize>,
) -> Vec<usize> {
    if cache.is_empty() {
        return ring.to_vec();
    }
    let n = ring.len();
    let mut out = Vec::with_capacity(n + cache.len());
    for i in 0..n {
        let a = ring[i];
        let b = ring[(i + 1) % n];
        out.push(a);
        // Find the edge (a,b) and any splits on it.
        let Some(e) = (0..mesh.edge_count()).map(EdgeId).find(|&e| {
            mesh.edge(e).is_some_and(|ed| {
                (ed.verts[0].0 == a && ed.verts[1].0 == b)
                    || (ed.verts[0].0 == b && ed.verts[1].0 == a)
            })
        }) else {
            continue;
        };
        let ed = mesh.edge(e).unwrap();
        let mut splits: Vec<(f64, usize)> = cache
            .iter()
            .filter(|((eid, _), _)| *eid == e.0)
            .map(|((_, ti), &idx)| (*ti as f64 / 1e6, idx))
            .collect();
        // `t` is measured verts[0] → verts[1]; flip if this ring edge runs b→a.
        if ed.verts[0].0 == b {
            for s in &mut splits {
                s.0 = 1.0 - s.0;
            }
        }
        splits.sort_by(|x, y| x.0.partial_cmp(&y.0).unwrap());
        for (_, idx) in splits {
            out.push(idx);
        }
    }
    out
}

/// Split a face `ring` into two along the chord `a … b` (both must appear in
/// `ring`). Returns the two sub-rings, or the original if the endpoints are not
/// both present / are adjacent-degenerate.
fn split_ring(ring: &[usize], a: usize, b: usize) -> Vec<Vec<usize>> {
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
    use crate::mesh::FaceId;
    use crate::primitives;

    #[test]
    fn chord_between_two_edge_midpoints_splits_a_quad() {
        let m = primitives::grid(1, 1, 2.0); // one quad, 4 verts, 4 edges
        // Cut from the midpoint of edge 0 to the midpoint of edge 2 (opposite).
        let r = knife(
            &m,
            &[Chord {
                face: FaceId(0),
                from: KnifePoint::EdgeSplit { edge: EdgeId(0), t: 0.5 },
                to: KnifePoint::EdgeSplit { edge: EdgeId(2), t: 0.5 },
            }],
        );
        assert_eq!(r.mesh.face_count(), 2, "quad → 2 faces");
        assert_eq!(r.mesh.vertex_count(), 6, "two new edge-mid vertices");
        assert_eq!(r.mesh.euler_characteristic(), 1);
    }

    #[test]
    fn chord_from_a_vertex_to_an_opposite_edge() {
        let m = primitives::grid(1, 1, 2.0);
        let r = knife(
            &m,
            &[Chord {
                face: FaceId(0),
                from: KnifePoint::Vertex(VertexId(0)),
                to: KnifePoint::EdgeSplit { edge: EdgeId(1), t: 0.5 },
            }],
        );
        assert_eq!(r.mesh.face_count(), 2);
        assert_eq!(r.mesh.vertex_count(), 5);
    }

    #[test]
    fn two_parallel_chords_on_one_face_make_three_pieces() {
        // grid(1,1,2): edge 0 is the bottom (v0→v1), edge 2 the top (v2→v3),
        // wound the opposite way — so a geometrically-parallel pair of cuts is
        // (edge0 @ t) ↔ (edge2 @ 1-t).
        let m = primitives::grid(1, 1, 2.0);
        let r = knife(
            &m,
            &[
                Chord {
                    face: FaceId(0),
                    from: KnifePoint::EdgeSplit { edge: EdgeId(0), t: 0.25 },
                    to: KnifePoint::EdgeSplit { edge: EdgeId(2), t: 0.75 },
                },
                Chord {
                    face: FaceId(0),
                    from: KnifePoint::EdgeSplit { edge: EdgeId(0), t: 0.75 },
                    to: KnifePoint::EdgeSplit { edge: EdgeId(2), t: 0.25 },
                },
            ],
        );
        assert_eq!(r.mesh.face_count(), 3);
    }

    #[test]
    fn diagonal_cut_across_a_cube_face_keeps_it_closed() {
        let m = primitives::cube(2.0);
        let e = crate::mesh::EdgeId(0);
        let ed = m.edge(e).unwrap();
        let r = knife(
            &m,
            &[Chord {
                face: FaceId(0),
                from: KnifePoint::Vertex(ed.verts[0]),
                to: KnifePoint::Vertex(m.face_vertices(FaceId(0))[2]),
            }],
        );
        assert_eq!(r.mesh.face_count(), 7, "one cube face → 2 triangles");
        assert_eq!(r.mesh.euler_characteristic(), 2, "still closed");
    }
}
