// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Per-face inset by centroid-directed corner shrink plus a bridging quad ring. No
// named published algorithm — written from first principles; no upstream source
// was copied. Blender analogue (architecture only): the bmo_inset operator,
// Individual mode.
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

//! Inset faces — replace each face with a smaller inner copy plus a bridging
//! ring of quads.
//!
//! This is the pure-Rust analogue of Blender's **Inset Faces**
//! (`bmo_inset`, *Individual* mode): every face is shrunk toward its own
//! centroid by a fraction, the shrunk copy becomes an **inner face**, and a
//! ring of quads bridges the original boundary to the inner face. It is the
//! standard modelling operation for adding a border loop around each face
//! (panelling, framing, controlled bevelling of the interior).
//!
//! # Individual mode
//!
//! Each face gets its **own** inner vertices — they are not shared between
//! adjacent faces — which is Blender's *Individual* inset. This keeps the
//! construction purely local and always valid: the original corner vertices
//! stay shared between neighbours (so the surface stays manifold and closed),
//! while the inset ring is independent per face.
//!
//! # Winding
//!
//! The inner face keeps the original winding (outward normal preserved). Each
//! ring quad `[vi, v(i+1), v(i+1)', vi']` traverses the inner edge opposite to
//! the inner face, so the whole result stays consistently wound — the same
//! adjacent-faces-oppose rule used elsewhere in the crate. No `faer`, no
//! external dependency; Android-safe.

use crate::math::Vec3;
use crate::mesh::Mesh;

/// Inset every face of `mesh` by `amount`, returning the new mesh.
///
/// `amount` is the fraction each corner moves **toward its face centroid**:
/// `0.0` leaves the face unchanged (a no-op), `0.5` halves the face, and values
/// approaching `1.0` collapse the inner face onto the centroid. Values `<= 0`
/// are treated as a no-op; values `>= 1` are clamped just below `1` to avoid a
/// degenerate zero-area inner face.
///
/// Each face becomes `1 + k` faces (one inner `k`-gon plus `k` ring quads) and
/// gains `k` new inner vertices. This is infallible.
///
/// # Examples
///
/// ```
/// use outram_blender::{primitives, inset::inset_faces};
///
/// // Inset every face of a cube by 30%: 6 faces → 6 inner quads + 24 ring
/// // quads = 30 faces, still a closed genus-0 surface (χ = 2).
/// let cube = primitives::cube(2.0);
/// let inset = inset_faces(&cube, 0.3);
/// assert_eq!(inset.face_count(), 30);
/// assert_eq!(inset.euler_characteristic(), 2);
/// ```
pub fn inset_faces(mesh: &Mesh, amount: f64) -> Mesh {
    let positions = mesh.positions();
    let polygons = mesh.polygons();

    // A no-op below zero: return an identical rebuild.
    if amount <= 0.0 {
        let faces: Vec<Vec<usize>> =
            polygons.iter().map(|p| p.iter().map(|v| v.0).collect()).collect();
        return Mesh::from_polygons(&positions, &faces);
    }
    // Clamp just under 1 so the inner face keeps positive area.
    let t = amount.min(1.0 - 1e-9);

    let mut new_positions = positions.clone();
    let mut new_faces: Vec<Vec<usize>> = Vec::new();

    for poly in &polygons {
        let k = poly.len();
        if k < 3 {
            continue;
        }
        // Face centroid.
        let mut c = Vec3::ZERO;
        for v in poly {
            c = c.add(positions[v.0]);
        }
        c = c.scale(1.0 / k as f64);

        // Inner vertices: lerp each corner toward the centroid by t.
        let mut inner: Vec<usize> = Vec::with_capacity(k);
        for v in poly {
            let p = positions[v.0];
            let pi = p.add(c.sub(p).scale(t));
            new_positions.push(pi);
            inner.push(new_positions.len() - 1);
        }

        // Inner face keeps the original winding.
        new_faces.push(inner.clone());

        // Ring quads bridge the boundary to the inner face.
        for i in 0..k {
            let a = poly[i].0;
            let b = poly[(i + 1) % k].0;
            let a_in = inner[i];
            let b_in = inner[(i + 1) % k];
            // [a, b, b', a'] — traverses the inner edge b'->a', opposite the
            // inner face's a'->b', keeping the winding consistent.
            new_faces.push(vec![a, b, b_in, a_in]);
        }
    }

    Mesh::from_polygons(&new_positions, &new_faces)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::primitives;
    use std::collections::HashSet;

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

    /// V&V — headline. Methodology: inset every face of `cube(2.0)` by 0.3.
    /// Pass criterion: each of the 6 quads becomes 1 inner quad + 4 ring quads
    /// and the surface stays a closed genus-0 manifold. Result: F 6→30
    /// (6·5), V 8→32 (8 corners + 6·4 inner), χ = 2, watertight & consistently
    /// wound.
    #[test]
    fn cube_inset_counts_and_watertight() {
        let inset = inset_faces(&primitives::cube(2.0), 0.3);
        assert_eq!(inset.face_count(), 30, "6 faces × (1 inner + 4 ring)");
        assert_eq!(inset.vertex_count(), 32, "8 corners + 6×4 inner");
        assert_eq!(inset.euler_characteristic(), 2, "closed genus-0 surface");
        assert!(is_watertight_consistent(&inset), "consistent outward winding");
    }

    /// V&V — the inner vertices lie strictly inside their faces. For a cube of
    /// side 2 inset by 0.3, each inner corner is 30% of the way to the face
    /// centre, so its distance from the origin is smaller than a corner's.
    #[test]
    fn inner_vertices_move_toward_centroid() {
        let cube = primitives::cube(2.0);
        let corner_r = cube.positions()[0].length();
        let inset = inset_faces(&cube, 0.3);
        // The first 8 positions are the untouched corners; the rest are inner.
        let inner_max_r = inset.positions()[8..]
            .iter()
            .map(|p| p.length())
            .fold(f64::MIN, f64::max);
        assert!(inner_max_r < corner_r, "inner ring is pulled inside the corners");
    }

    /// V&V — `amount = 0` is a no-op: topology unchanged.
    #[test]
    fn zero_amount_is_noop() {
        let cube = primitives::cube(2.0);
        let inset = inset_faces(&cube, 0.0);
        assert_eq!(inset.vertex_count(), 8);
        assert_eq!(inset.face_count(), 6);
        assert_eq!(inset.euler_characteristic(), 2);
    }

    /// V&V — the `MeshOp::Inset` dispatch matches the free function.
    #[test]
    fn meshop_inset_matches_free_function() {
        use crate::ops::MeshOp;
        let cube = primitives::cube(2.0);
        let direct = inset_faces(&cube, 0.25);
        let via_op = MeshOp::Inset { amount: 0.25 }.apply(cube).expect("inset infallible");
        assert_eq!(via_op.vertex_count(), direct.vertex_count());
        assert_eq!(via_op.face_count(), direct.face_count());
        assert_eq!(via_op.euler_characteristic(), direct.euler_characteristic());
    }
}
