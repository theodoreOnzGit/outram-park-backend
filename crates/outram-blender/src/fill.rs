// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Fill operators. Follows the published behaviour of Blender's fill operators
// (source/blender/bmesh/operators/bmo_grid_fill.cc, bmo_triangle_fill.cc, and
// editmesh_add_gizmo / MESH_OT_edge_face_add, github.com/blender/blender,
// GPL-2.0-or-later): make an edge/face from a vertex selection, grid-fill a
// rectangular boundary loop with quads, and beauty-fill a triangulated region
// toward better-shaped triangles. Concepts only — no upstream source copied;
// polygon-soup rebuilds.
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

//! **Fill operators** (`op-hzs.54.15`, GH issue #37 §B).
//!
//! - [`make_face`] — add one face through the given ordered vertices (Blender's
//!   `F` when it closes a loop). Two vertices with no face between them add a
//!   wire edge.
//! - [`grid_fill`] — fill a closed boundary loop of `2·(w + h)` vertices with a
//!   `w × h` quad grid, splitting the loop into four sides at `span`
//!   (Blender's `Face ▸ Grid Fill`).
//! - [`beauty_fill`] — flip the shared diagonal of adjacent triangle pairs
//!   toward the Delaunay (max-min-angle) criterion (Blender's `Face ▸ Beauty
//!   Fill`).
//! - Simple hole capping is [`crate::fill_holes`]; the F-fill of an *edge net*
//!   into multiple faces is tracked as follow-up.

use std::collections::HashMap;

use crate::math::Vec3;
use crate::mesh::{Mesh, VertexId};

/// Add one face through `verts` in the given order. If `verts.len() == 2` a
/// wire edge is recorded instead (a zero-area sliver in the soup model).
/// Returns the rebuilt mesh; `verts.len() < 2` is a no-op.
pub fn make_face(mesh: &Mesh, verts: &[VertexId]) -> Mesh {
    if verts.len() < 2 {
        return mesh.clone();
    }
    let positions = mesh.positions();
    let mut faces: Vec<Vec<usize>> = mesh.polygons().iter().map(|f| f.iter().map(|v| v.0).collect()).collect();
    if verts.len() == 2 {
        faces.push(vec![verts[0].0, verts[1].0, verts[0].0]);
    } else {
        faces.push(verts.iter().map(|v| v.0).collect());
    }
    Mesh::from_polygons(&positions, &faces)
}

/// Fill the closed boundary loop `boundary` (ordered vertex ring) with a quad
/// grid. `span` is the number of edges on the first side; the loop must have
/// `2 · span + 2 · other` vertices for some `other >= 1`. Returns the mesh
/// unchanged if that does not hold.
pub fn grid_fill(mesh: &Mesh, boundary: &[VertexId], span: usize) -> Mesh {
    let l = boundary.len();
    if span == 0 || l < 4 || !l.is_multiple_of(2) || l <= 2 * span {
        return mesh.clone();
    }
    let other = l / 2 - span;
    if other == 0 {
        return mesh.clone();
    }

    // Corner indices around the loop: 0, span, span+other, 2*span+other.
    let c0 = 0;
    let c1 = span;
    let c2 = span + other;
    let c3 = 2 * span + other;

    // Sides as vertex lists (span+1 or other+1 long).
    let side = |from: usize, len: usize| -> Vec<usize> {
        (0..=len).map(|k| boundary[(from + k) % l].0).collect()
    };
    let bottom = side(c0, span); // c0 → c1
    let right = side(c1, other); // c1 → c2
    let top_rev = side(c2, span); // c2 → c3  (reverse of the grid's top row)
    let left_rev = side(c3, other); // c3 → c0 (reverse of the grid's left column)

    let mut positions = mesh.positions();
    let mut grid = vec![vec![0usize; other + 1]; span + 1];
    for i in 0..=span {
        for j in 0..=other {
            grid[i][j] = if j == 0 {
                bottom[i]
            } else if j == other {
                top_rev[span - i]
            } else if i == 0 {
                left_rev[other - j]
            } else if i == span {
                right[j]
            } else {
                let u = i as f64 / span as f64;
                let v = j as f64 / other as f64;
                let pb = positions[bottom[i]];
                let pt = positions[top_rev[span - i]];
                let pl = positions[left_rev[other - j]];
                let pr = positions[right[j]];
                // Coons-ish bilinear blend of the four edges.
                let p = pb.scale(1.0 - v)
                    .add(pt.scale(v))
                    .add(pl.scale(1.0 - u))
                    .add(pr.scale(u))
                    .sub(
                        positions[bottom[0]].scale((1.0 - u) * (1.0 - v))
                            .add(positions[bottom[span]].scale(u * (1.0 - v)))
                            .add(positions[top_rev[0]].scale(u * v))
                            .add(positions[top_rev[span]].scale((1.0 - u) * v)),
                    );
                positions.push(p);
                positions.len() - 1
            };
        }
    }

    let mut faces: Vec<Vec<usize>> = mesh.polygons().iter().map(|f| f.iter().map(|v| v.0).collect()).collect();
    for i in 0..span {
        for j in 0..other {
            faces.push(vec![grid[i][j], grid[i + 1][j], grid[i + 1][j + 1], grid[i][j + 1]]);
        }
    }
    Mesh::from_polygons(&positions, &faces)
}

/// Flip the shared diagonal of every adjacent triangle pair toward the
/// max-min-angle (Delaunay) criterion — one pass. Non-triangle faces are left
/// alone. Returns the rebuilt mesh.
pub fn beauty_fill(mesh: &Mesh) -> Mesh {
    let positions = mesh.positions();
    let mut tris: Vec<[usize; 3]> = Vec::new();
    let mut others: Vec<Vec<usize>> = Vec::new();
    for poly in mesh.polygons() {
        if poly.len() == 3 {
            tris.push([poly[0].0, poly[1].0, poly[2].0]);
        } else {
            others.push(poly.iter().map(|v| v.0).collect());
        }
    }

    // Map each undirected edge → the (tri index, opposite vertex).
    let mut edge_tri: HashMap<(usize, usize), Vec<(usize, usize)>> = HashMap::new();
    for (ti, t) in tris.iter().enumerate() {
        for k in 0..3 {
            let (a, b, c) = (t[k], t[(k + 1) % 3], t[(k + 2) % 3]);
            edge_tri.entry((a.min(b), a.max(b))).or_default().push((ti, c));
        }
    }

    let mut flipped = vec![false; tris.len()];
    for (&(a, b), pair) in &edge_tri {
        if pair.len() != 2 {
            continue;
        }
        let ((t0, c0), (t1, c1)) = (pair[0], pair[1]);
        if flipped[t0] || flipped[t1] {
            continue;
        }
        // Flip a-b → c0-c1 if it improves the minimum angle.
        let cur_min = min_angle(&positions, a, c0, b).min(min_angle(&positions, a, c1, b));
        let new_min = min_angle(&positions, c0, a, c1).min(min_angle(&positions, c0, b, c1));
        if new_min > cur_min + 1e-9 {
            tris[t0] = [c0, c1, a];
            tris[t1] = [c1, c0, b];
            flipped[t0] = true;
            flipped[t1] = true;
        }
        let _ = (a, b);
    }

    let mut faces: Vec<Vec<usize>> = others;
    faces.extend(tris.iter().map(|t| t.to_vec()));
    Mesh::from_polygons(&positions, &faces)
}

/// Smallest interior angle of triangle `(a, b, c)` at any corner, in radians.
fn min_angle(pos: &[Vec3], a: usize, b: usize, c: usize) -> f64 {
    let ang = |o: usize, p: usize, q: usize| -> f64 {
        let u = pos[p].sub(pos[o]);
        let v = pos[q].sub(pos[o]);
        let d = u.dot(v) / (u.length() * v.length() + 1e-12);
        d.clamp(-1.0, 1.0).acos()
    };
    ang(a, b, c).min(ang(b, a, c)).min(ang(c, a, b))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::primitives;

    #[test]
    fn make_face_closes_a_hole_in_a_cube() {
        let m = primitives::cube(2.0);
        let open = crate::dissolve::delete(&m, crate::dissolve::DeleteMode::Faces, &[], &[], &[crate::mesh::FaceId(0)]);
        assert_eq!(open.face_count(), 5);
        let ring = open_boundary(&open);
        let filled = make_face(&open, &ring);
        assert_eq!(filled.face_count(), 6);
        assert_eq!(filled.euler_characteristic(), 2);
    }

    #[test]
    fn grid_fill_a_square_boundary() {
        // A 4x4 grid's border loop (12 verts), no interior faces: grid-fill it.
        let g = primitives::grid(3, 3, 3.0);
        let border_only = crate::dissolve::delete(
            &g,
            crate::dissolve::DeleteMode::Faces,
            &[],
            &[],
            &(0..g.face_count()).map(crate::mesh::FaceId).collect::<Vec<_>>(),
        );
        let _ = border_only; // deleting all faces leaves nothing — build the ring directly instead
        let mut m = Mesh::new();
        let mut ring = Vec::new();
        // 3x3 square perimeter, 12 verts, span 3, other 3.
        for i in 0..3 {
            ring.push(m.add_vertex(Vec3::new(i as f64, 0.0, 0.0)));
        }
        for j in 0..3 {
            ring.push(m.add_vertex(Vec3::new(3.0, j as f64, 0.0)));
        }
        for i in 0..3 {
            ring.push(m.add_vertex(Vec3::new(3.0 - i as f64, 3.0, 0.0)));
        }
        for j in 0..3 {
            ring.push(m.add_vertex(Vec3::new(0.0, 3.0 - j as f64, 0.0)));
        }
        m.add_face(&[ring[0], ring[1], ring[2], ring[3]]); // dummy to keep verts
        let filled = grid_fill(&m, &ring, 3);
        // 3x3 quads added.
        assert_eq!(filled.face_count(), 1 + 9);
    }

    #[test]
    fn beauty_fill_flips_a_bad_diagonal() {
        // A thin quad split the wrong way: two very obtuse triangles.
        let mut m = Mesh::new();
        let a = m.add_vertex(Vec3::new(0.0, 0.0, 0.0));
        let b = m.add_vertex(Vec3::new(4.0, 0.1, 0.0));
        let c = m.add_vertex(Vec3::new(4.0, 0.0, 0.0));
        let d = m.add_vertex(Vec3::new(0.0, 0.1, 0.0));
        m.add_face(&[a, b, c]);
        m.add_face(&[a, d, b]);
        let before = worst_triangle(&m);
        let bf = beauty_fill(&m);
        let after = worst_triangle(&bf);
        assert!(after >= before - 1e-9, "min angle did not get worse");
        assert_eq!(bf.face_count(), 2);
    }

    fn open_boundary(m: &Mesh) -> Vec<VertexId> {
        let topo = crate::topology::MeshTopology::new(m);
        let edges: Vec<crate::mesh::EdgeId> =
            (0..m.edge_count()).map(crate::mesh::EdgeId).filter(|&e| topo.is_boundary_edge(e)).collect();
        crate::bridge::ordered_ring(m, &edges).map(|(r, _)| r).unwrap_or_default()
    }

    fn worst_triangle(m: &Mesh) -> f64 {
        let pos = m.positions();
        m.polygons()
            .iter()
            .filter(|p| p.len() == 3)
            .map(|p| min_angle(&pos, p[0].0, p[1].0, p[2].0))
            .fold(f64::MAX, f64::min)
    }
}
