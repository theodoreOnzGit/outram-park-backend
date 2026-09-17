// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Connect Vertex Path / Pairs. Follows the published behaviour of Blender's
// connect operators (source/blender/bmesh/operators/bmo_connect.cc,
// bmo_connect_pair.cc, github.com/blender/blender, GPL-2.0-or-later): connect
// selected vertices lying on a common face with new edges, splitting that face.
// Concepts only — no upstream source copied; this composes the face-chord split
// in `knife`.
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

//! **Connect Vertex Path / Pairs** (`op-hzs.54.16`, GH issue #37 §B) —
//! Blender's `J`.
//!
//! - [`connect_vertex_path`] connects an ordered vertex list: each consecutive
//!   pair that shares a face splits that face along the chord.
//! - [`connect_vertex_pairs`] connects an explicit list of pairs.
//!
//! Both compose [`crate::knife::knife`]'s face-chord split, so they inherit its
//! "one chord per face, applied in sequence" behaviour. A pair that shares no
//! face, or is already an edge, is skipped.

use std::collections::BTreeSet;

use crate::knife::{Chord, KnifePoint};
use crate::mesh::{FaceId, Mesh, VertexId};

/// Connect the ordered `path` of vertices: for each consecutive pair sharing a
/// face, split that face along the chord between them. Returns the rebuilt
/// mesh.
pub fn connect_vertex_path(mesh: &Mesh, path: &[VertexId]) -> Mesh {
    let pairs: Vec<(VertexId, VertexId)> = path.windows(2).map(|w| (w[0], w[1])).collect();
    connect_vertex_pairs(mesh, &pairs)
}

/// Connect an explicit list of vertex `pairs`. Order matters when several pairs
/// touch one face (each acts on whichever sub-face contains it).
pub fn connect_vertex_pairs(mesh: &Mesh, pairs: &[(VertexId, VertexId)]) -> Mesh {
    let mut chords: Vec<Chord> = Vec::new();
    for &(a, b) in pairs {
        if a == b {
            continue;
        }
        // Already an edge? then there is nothing to cut.
        if edge_between(mesh, a, b).is_some() {
            continue;
        }
        if let Some(f) = common_face(mesh, a, b) {
            chords.push(Chord {
                face: f,
                from: KnifePoint::Vertex(a),
                to: KnifePoint::Vertex(b),
            });
        }
    }
    if chords.is_empty() {
        return mesh.clone();
    }
    crate::knife::knife(mesh, &chords).mesh
}

/// A face incident to **both** `a` and `b` (the first by id), or `None`.
pub fn common_face(mesh: &Mesh, a: VertexId, b: VertexId) -> Option<FaceId> {
    (0..mesh.face_count()).map(FaceId).find(|&f| {
        let vs: BTreeSet<VertexId> = mesh.face_vertices(f).into_iter().collect();
        vs.contains(&a) && vs.contains(&b)
    })
}

fn edge_between(mesh: &Mesh, a: VertexId, b: VertexId) -> Option<crate::mesh::EdgeId> {
    (0..mesh.edge_count()).map(crate::mesh::EdgeId).find(|&e| {
        mesh.edge(e).is_some_and(|ed| {
            (ed.verts[0] == a && ed.verts[1] == b) || (ed.verts[0] == b && ed.verts[1] == a)
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::primitives;

    #[test]
    fn connect_opposite_corners_of_a_quad() {
        let m = primitives::grid(1, 1, 2.0); // one quad
        let q = m.face_vertices(FaceId(0));
        let c = connect_vertex_path(&m, &[q[0], q[2]]); // the diagonal
        assert_eq!(c.face_count(), 2, "quad → 2 triangles");
        assert_eq!(c.vertex_count(), 4, "no new vertices, only a new edge");
    }

    #[test]
    fn connect_a_path_across_two_grid_quads() {
        let m = primitives::grid(2, 1, 2.0); // quads 0,1; verts 0..5
                                             // Diagonal of quad 0 then diagonal of quad 1.
        let q0 = m.face_vertices(FaceId(0));
        let q1 = m.face_vertices(FaceId(1));
        let c = connect_vertex_path(&m, &[q0[0], q0[2], q1[2]]);
        assert!(c.face_count() >= 3);
    }

    #[test]
    fn adjacent_corners_are_a_noop() {
        let m = primitives::grid(1, 1, 2.0);
        let c = connect_vertex_path(&m, &[VertexId(0), VertexId(1)]);
        assert_eq!(c.face_count(), 1, "0-1 is already an edge");
    }

    #[test]
    fn vertices_on_no_common_face_are_skipped() {
        let m = primitives::cube(2.0);
        // Pick a vertex and its true antipode (they share no face).
        let p0 = m.vertex(VertexId(0)).unwrap().position;
        let anti = (0..m.vertex_count())
            .map(VertexId)
            .max_by(|&a, &b| {
                let da = m.vertex(a).unwrap().position.sub(p0).length();
                let db = m.vertex(b).unwrap().position.sub(p0).length();
                da.partial_cmp(&db).unwrap()
            })
            .unwrap();
        assert!(common_face(&m, VertexId(0), anti).is_none());
        let c = connect_vertex_path(&m, &[VertexId(0), anti]);
        assert_eq!(c.face_count(), m.face_count());
    }
}
