// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Edge chamfering by per-face corner inset plus edge/vertex gap filling. No named
// published algorithm — written from first principles; no upstream source was
// copied. Blender analogue (architecture only): the Bevel tool in edge mode.
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

//! Edge bevel — chamfer every edge of a closed mesh.
//!
//! This is the pure-Rust analogue of Blender's **Bevel** on edges (as opposed
//! to the vertex-truncation [`crate::ops::bevel_vertices`]). Each face is cut
//! back from its edges by `width`, the gap left along every original **edge**
//! is filled by a bevel face, and the gap at every original **vertex** is
//! capped — turning each sharp edge into a flat chamfer.
//!
//! # Construction
//!
//! For each face and each of its corners, a new **face-corner vertex** is placed
//! by moving the corner inward along its two incident in-face edge directions by
//! `width` (exact for right-angle corners, a good approximation otherwise). The
//! output is then three families of faces:
//!
//! - **shrunk faces** — each original face, rebuilt on its face-corner vertices;
//! - **edge chamfers** — one quad per original edge, bridging the two shrunk
//!   faces that met there;
//! - **vertex caps** — one polygon per original vertex, filling the truncated
//!   corner (its incident face-corner vertices in umbrella order).
//!
//! Only the **connectivity** is built here; the final consistent, outward
//! winding is delegated to [`crate::recalc_normals::recalculate_normals`], so
//! the construction never has to reason about per-face orientation.
//!
//! # Scope (v1)
//!
//! Flat chamfer (a single bevel face per edge; rounded multi-segment bevels are
//! future work). Intended for **closed manifold** meshes; boundary edges/vertices
//! are left uncapped. `width` must be smaller than half the shortest edge, or a
//! face can invert. No `faer`, no external dependency; Android-safe.

use crate::math::Vec3;
use crate::mesh::Mesh;
use std::collections::{HashMap, HashSet};

/// Chamfer every edge of `mesh` by `width`, returning the beveled mesh.
///
/// `width` is the distance each face is cut back from its edges, in mesh units;
/// keep it below half the shortest edge length to avoid inverting a face. The
/// result is consistently wound and outward-facing. This is infallible.
///
/// # Examples
///
/// ```
/// use outram_blender::{primitives, edge_bevel::bevel_edges};
///
/// // Beveling a cube's 12 edges: 6 shrunk squares + 12 edge quads + 8 corner
/// // triangles = 26 faces, still a closed genus-0 surface (χ = 2).
/// let beveled = bevel_edges(&primitives::cube(2.0), 0.3);
/// assert_eq!(beveled.face_count(), 26);
/// assert_eq!(beveled.euler_characteristic(), 2);
/// ```
pub fn bevel_edges(mesh: &Mesh, width: f64) -> Mesh {
    let ps = mesh.positions();
    let polys: Vec<Vec<usize>> =
        mesh.polygons().iter().map(|p| p.iter().map(|v| v.0).collect()).collect();
    let nf = polys.len();

    // Face-corner vertices: one new vertex per (face, corner), inset inward.
    let mut new_positions: Vec<Vec3> = Vec::new();
    let mut fc: Vec<Vec<usize>> = Vec::with_capacity(nf);
    for poly in &polys {
        let k = poly.len();
        let mut row = Vec::with_capacity(k);
        for i in 0..k {
            let v = ps[poly[i]];
            let prev = ps[poly[(i + k - 1) % k]];
            let next = ps[poly[(i + 1) % k]];
            let pos = v.add(prev.sub(v).normalize().scale(width)).add(next.sub(v).normalize().scale(width));
            new_positions.push(pos);
            row.push(new_positions.len() - 1);
        }
        fc.push(row);
    }

    // Directed-edge owner: (a, b) -> (face, corner i) with a = poly[i], b = poly[i+1].
    let mut owner: HashMap<(usize, usize), (usize, usize)> = HashMap::new();
    for (f, poly) in polys.iter().enumerate() {
        let k = poly.len();
        for i in 0..k {
            owner.insert((poly[i], poly[(i + 1) % k]), (f, i));
        }
    }

    let mut new_faces: Vec<Vec<usize>> = Vec::new();

    // (a) Shrunk faces.
    for row in &fc {
        new_faces.push(row.clone());
    }

    // (b) Edge chamfers — one quad per undirected edge.
    let mut seen: HashSet<(usize, usize)> = HashSet::new();
    for (f, poly) in polys.iter().enumerate() {
        let k = poly.len();
        for i in 0..k {
            let a = poly[i];
            let b = poly[(i + 1) % k];
            let key = if a < b { (a, b) } else { (b, a) };
            if !seen.insert(key) {
                continue;
            }
            if let Some(&(g, gj)) = owner.get(&(b, a)) {
                let a_f = fc[f][i];
                let b_f = fc[f][(i + 1) % k];
                // g traverses b -> a: corner gj is b, gj+1 is a.
                let b_g = fc[g][gj];
                let a_g = fc[g][(gj + 1) % polys[g].len()];
                new_faces.push(vec![a_f, b_f, b_g, a_g]);
            }
            // Boundary edge (no reverse owner): left open in v1.
        }
    }

    // (c) Vertex caps — walk the umbrella of faces around each vertex.
    let mut vert_corners: HashMap<usize, Vec<(usize, usize)>> = HashMap::new();
    for (f, poly) in polys.iter().enumerate() {
        for (i, &v) in poly.iter().enumerate() {
            vert_corners.entry(v).or_default().push((f, i));
        }
    }
    for (_v, corners) in &vert_corners {
        if corners.len() < 3 {
            continue;
        }
        let start = corners[0];
        let mut cap = Vec::new();
        let mut cur = start;
        let mut open = false;
        loop {
            let (f, i) = cur;
            cap.push(fc[f][i]);
            let poly = &polys[f];
            let k = poly.len();
            let v = poly[i];
            let w = poly[(i + 1) % k]; // next edge v -> w
            match owner.get(&(w, v)) {
                // Neighbour g across that edge; v sits at corner (m+1) in g.
                Some(&(g, m)) => {
                    cur = (g, (m + 1) % polys[g].len());
                    if cur == start {
                        break;
                    }
                }
                None => {
                    open = true;
                    break;
                }
            }
            if cap.len() > corners.len() {
                break; // safety against a malformed umbrella
            }
        }
        // Only cap a fully-closed umbrella that visited every incident face once.
        if !open && cap.len() == corners.len() && cap.len() >= 3 {
            new_faces.push(cap);
        }
    }

    // Build connectivity, then let recalc_normals fix consistent outward winding.
    let built = Mesh::from_polygons(&new_positions, &new_faces);
    crate::recalc_normals::recalculate_normals(&built)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::primitives;

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

    fn signed_volume6(mesh: &Mesh) -> f64 {
        let ps = mesh.positions();
        let mut v = 0.0;
        for poly in mesh.polygons() {
            let p0 = ps[poly[0].0];
            for i in 1..poly.len() - 1 {
                v += p0.dot(ps[poly[i].0].cross(ps[poly[i + 1].0]));
            }
        }
        v
    }

    /// V&V — headline. Methodology: bevel the 12 edges of `cube(2.0)` by 0.3.
    /// Pass criterion: each face is cut back and the edge/corner gaps are filled
    /// into a closed manifold. Result: F = 6 shrunk squares + 12 edge quads + 8
    /// corner triangles = 26; V = 6 faces × 4 corners = 24; E = 24 − 2 + 26 =
    /// 48; χ = 2; watertight, consistently wound, positive (outward) volume.
    #[test]
    fn cube_edge_bevel_counts_and_watertight() {
        let beveled = bevel_edges(&primitives::cube(2.0), 0.3);
        assert_eq!(beveled.vertex_count(), 24, "6 faces × 4 face-corner vertices");
        assert_eq!(beveled.face_count(), 26, "6 shrunk + 12 edge + 8 corner");
        assert_eq!(beveled.edge_count(), 48);
        assert_eq!(beveled.euler_characteristic(), 2, "closed genus-0 surface");
        assert!(is_watertight_consistent(&beveled), "consistent winding");
        assert!(signed_volume6(&beveled) > 0.0, "outward-facing");
    }

    /// V&V — the chamfer shrinks the faces inward: no beveled vertex is farther
    /// from the origin than an original cube corner (which sat at √3 ≈ 1.732).
    #[test]
    fn beveled_vertices_move_inward() {
        let beveled = bevel_edges(&primitives::cube(2.0), 0.3);
        let corner_r = (3.0f64).sqrt(); // cube(2.0) corner at (±1,±1,±1)
        assert!(
            beveled.positions().iter().all(|p| p.length() <= corner_r + 1e-9),
            "every face-corner vertex is pulled inside the original corner radius",
        );
    }

    /// V&V — the `MeshOp::BevelEdges` dispatch matches the free function.
    #[test]
    fn meshop_bevel_edges_matches_free_function() {
        use crate::ops::MeshOp;
        let cube = primitives::cube(2.0);
        let direct = bevel_edges(&cube, 0.25);
        let via_op = MeshOp::BevelEdges { width: 0.25 }.apply(cube).expect("edge bevel infallible");
        assert_eq!(via_op.vertex_count(), direct.vertex_count());
        assert_eq!(via_op.face_count(), direct.face_count());
        assert_eq!(via_op.euler_characteristic(), direct.euler_characteristic());
    }
}
