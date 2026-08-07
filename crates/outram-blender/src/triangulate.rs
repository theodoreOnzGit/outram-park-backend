// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Fan triangulation of each polygon from its first corner (an n-gon becomes n - 2
// triangles), which preserves winding. Standard technique, exact for convex faces;
// no upstream source was copied. Blender analogue (architecture only): the
// bmo_triangulate operator in fan mode.
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

//! Triangulate — convert every polygon face into triangles, returning a
//! triangle-only [`Mesh`].
//!
//! This is the pure-Rust analogue of Blender's **Triangulate Faces**
//! (`bmo_triangulate`, fan mode). It fan-triangulates each face — a quad
//! becomes two triangles, an `n`-gon becomes `n − 2` — and rebuilds a mesh
//! whose every face is a triangle.
//!
//! # Why not [`crate::export::triangulate`]?
//!
//! [`crate::export::triangulate`] produces an
//! [`crate::export::IndexedTriangles`] — a flat positions + `u32` index buffer
//! for a GPU or a solver import, **not** a [`Mesh`]. This operator instead
//! returns a first-class [`Mesh`] with half-edge topology, so downstream
//! mesh operators that assume or prefer triangles — [`crate::loop_subdivision`]
//! (Loop subdivision is only defined on triangle meshes),
//! [`crate::decimate`] (QEM edge collapse), and the CSG/polyMesh bridges — can
//! consume the result directly.
//!
//! # Winding
//!
//! Fan triangulation from each face's first corner preserves the face's
//! winding, so a consistently-wound, outward-facing input stays that way. No
//! `faer`, no external dependency; Android-safe.

use crate::mesh::Mesh;

/// Fan-triangulate every face of `mesh`, returning a triangle-only mesh.
///
/// Positions are unchanged; only faces are re-cut. A face with fewer than three
/// corners is dropped (it is already degenerate); a triangle is passed through
/// unchanged. This is infallible.
///
/// # Examples
///
/// ```
/// use outram_blender::{primitives, triangulate::triangulate};
///
/// // A cube's 6 quads fan-triangulate into 12 triangles; still χ = 2.
/// let cube = primitives::cube(2.0);
/// let tris = triangulate(&cube);
/// assert_eq!(tris.face_count(), 12);
/// assert_eq!(tris.euler_characteristic(), 2);
/// ```
pub fn triangulate(mesh: &Mesh) -> Mesh {
    let positions = mesh.positions();
    let mut faces: Vec<Vec<usize>> = Vec::new();
    for poly in mesh.polygons() {
        let k = poly.len();
        if k < 3 {
            continue;
        }
        // Fan from the first corner: (v0, v1, v2), (v0, v2, v3), …
        for i in 1..k - 1 {
            faces.push(vec![poly[0].0, poly[i].0, poly[i + 1].0]);
        }
    }
    Mesh::from_polygons(&positions, &faces)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::primitives;
    use std::collections::HashSet;

    /// True when every face is a triangle.
    fn all_triangles(mesh: &Mesh) -> bool {
        mesh.polygons().iter().all(|p| p.len() == 3)
    }

    /// True when every directed half-edge is unique and its reverse is present.
    fn is_watertight_consistent(mesh: &Mesh) -> bool {
        let mut directed: HashSet<(usize, usize)> = HashSet::new();
        for poly in mesh.polygons() {
            let k = poly.len();
            for i in 0..k {
                let a = poly[i].0;
                let b = poly[(i + 1) % k].0;
                if !directed.insert((a, b)) {
                    return false;
                }
            }
        }
        directed.iter().all(|&(a, b)| directed.contains(&(b, a)))
    }

    /// V&V — headline. Methodology: triangulate `cube(2.0)` (6 quads). Pass
    /// criterion: every face is a triangle and the surface stays a closed
    /// genus-0 manifold. Result: V 8 (unchanged), F 6→12, E 12→18 (12 original
    /// + 6 quad diagonals), χ = 8 − 18 + 12 = 2, watertight & consistently wound.
    #[test]
    fn cube_triangulates_to_twelve_triangles() {
        let tris = triangulate(&primitives::cube(2.0));
        assert!(all_triangles(&tris), "every face is a triangle");
        assert_eq!(tris.vertex_count(), 8, "positions unchanged");
        assert_eq!(tris.face_count(), 12, "6 quads → 12 triangles");
        assert_eq!(tris.edge_count(), 18, "12 cube edges + 6 diagonals");
        assert_eq!(tris.euler_characteristic(), 2);
        assert!(is_watertight_consistent(&tris), "winding preserved");
    }

    /// V&V — an already-triangular mesh keeps its triangle count. The UV-sphere
    /// pole caps are triangles and its bands are quads; after triangulation
    /// every face is a triangle and χ stays 2.
    #[test]
    fn mixed_mesh_becomes_all_triangles() {
        let sphere = primitives::uv_sphere(8, 4, 1.0);
        let before_faces = sphere.face_count();
        let tris = triangulate(&sphere);
        assert!(all_triangles(&tris));
        assert!(tris.face_count() >= before_faces, "quads split, tris pass through");
        assert_eq!(tris.euler_characteristic(), 2);
        assert!(is_watertight_consistent(&tris));
    }

    /// V&V — idempotence: triangulating a triangle mesh changes nothing.
    #[test]
    fn triangulate_is_idempotent() {
        let once = triangulate(&primitives::cube(2.0));
        let twice = triangulate(&once);
        assert_eq!(twice.face_count(), once.face_count());
        assert_eq!(twice.vertex_count(), once.vertex_count());
        assert_eq!(twice.edge_count(), once.edge_count());
    }

    /// V&V — the `MeshOp::Triangulate` dispatch matches the free function.
    #[test]
    fn meshop_triangulate_matches_free_function() {
        use crate::ops::MeshOp;
        let cube = primitives::cube(2.0);
        let direct = triangulate(&cube);
        let via_op = MeshOp::Triangulate.apply(cube).expect("triangulate infallible");
        assert_eq!(via_op.face_count(), direct.face_count());
        assert_eq!(via_op.euler_characteristic(), direct.euler_characteristic());
    }
}
