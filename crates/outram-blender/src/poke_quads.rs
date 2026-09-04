// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Poke Faces / Tris <-> Quads. Follows the published behaviour of Blender's
// operators (source/blender/bmesh/operators/bmo_poke.cc, bmo_triangulate.cc and
// bmo_join_triangles.cc, github.com/blender/blender, GPL-2.0-or-later): poke a
// face into a centroid fan, triangulate quads by a chosen diagonal, and join
// adjacent triangle pairs back into quads under a shape/angle threshold.
// Concepts only — no upstream source copied; polygon-soup rebuilds.
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

//! **Poke Faces / Tris ↔ Quads** (`op-hzs.54.17`, GH issue #37 §B).
//!
//! - [`poke_faces`] replaces each face with a fan of triangles from a new
//!   centre vertex, offset by `offset` along the face normal (Blender's
//!   `Face ▸ Poke Faces`).
//! - [`triangulate_quads`] triangulates each quad by the diagonal chosen per
//!   [`QuadMethod`]; n-gons are centroid-fanned (Blender's `Face ▸
//!   Triangulate` with a quad method). The plain fan is
//!   [`crate::triangulate::triangulate`].
//! - [`tris_to_quads`] greedily merges adjacent coplanar-ish triangle pairs
//!   into quads whose corner angles stay within `max_angle` of 90° (Blender's
//!   `Face ▸ Tris to Quads`). Attribute comparisons (material / UV / sharp /
//!   seam) arrive with the attribute layers in `op-hzs.54.28`.

use std::collections::{HashMap, HashSet};

use crate::math::Vec3;
use crate::mesh::{FaceId, Mesh};

/// Which diagonal [`triangulate_quads`] cuts a quad along.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuadMethod {
    /// The shorter of the two diagonals.
    ShortestDiagonal,
    /// Always `v0–v2`.
    Fixed,
    /// Always `v1–v3`.
    FixedAlternate,
    /// The diagonal that maximises the smaller of the four resulting triangle
    /// angles (a local "beauty" choice).
    Beauty,
}

/// Poke every face into a centroid fan. The centre vertex is
/// `centroid + normal · offset`.
pub fn poke_faces(mesh: &Mesh, offset: f64) -> Mesh {
    let mut positions = mesh.positions();
    let mut faces: Vec<Vec<usize>> = Vec::new();
    for f in 0..mesh.face_count() {
        let vs: Vec<usize> = mesh.face_vertices(FaceId(f)).iter().map(|v| v.0).collect();
        if vs.len() < 3 {
            faces.push(vs);
            continue;
        }
        let c = mesh
            .face_centroid(FaceId(f))
            .add(mesh.face_normal(FaceId(f)).scale(offset));
        let ci = positions.len();
        positions.push(c);
        let n = vs.len();
        for i in 0..n {
            faces.push(vec![ci, vs[i], vs[(i + 1) % n]]);
        }
    }
    Mesh::from_polygons(&positions, &faces)
}

/// Triangulate every quad by `method`; n-gons are centroid-fanned; triangles
/// are kept.
pub fn triangulate_quads(mesh: &Mesh, method: QuadMethod) -> Mesh {
    let pos = mesh.positions();
    let mut positions = pos.clone();
    let mut faces: Vec<Vec<usize>> = Vec::new();
    for f in 0..mesh.face_count() {
        let vs: Vec<usize> = mesh.face_vertices(FaceId(f)).iter().map(|v| v.0).collect();
        match vs.len() {
            3 => faces.push(vs),
            4 => {
                let diag_02 = match method {
                    QuadMethod::Fixed => true,
                    QuadMethod::FixedAlternate => false,
                    QuadMethod::ShortestDiagonal => {
                        pos[vs[0]].sub(pos[vs[2]]).length() <= pos[vs[1]].sub(pos[vs[3]]).length()
                    }
                    QuadMethod::Beauty => {
                        let a = min_angle4(&pos, vs[0], vs[1], vs[2], vs[3], true);
                        let b = min_angle4(&pos, vs[0], vs[1], vs[2], vs[3], false);
                        a >= b
                    }
                };
                if diag_02 {
                    faces.push(vec![vs[0], vs[1], vs[2]]);
                    faces.push(vec![vs[0], vs[2], vs[3]]);
                } else {
                    faces.push(vec![vs[1], vs[2], vs[3]]);
                    faces.push(vec![vs[1], vs[3], vs[0]]);
                }
            }
            n if n > 4 => {
                let c = mesh.face_centroid(FaceId(f));
                let ci = positions.len();
                positions.push(c);
                for i in 0..n {
                    faces.push(vec![ci, vs[i], vs[(i + 1) % n]]);
                }
            }
            _ => faces.push(vs),
        }
    }
    Mesh::from_polygons(&positions, &faces)
}

/// Greedily merge adjacent triangle pairs into quads. A pair is merged when the
/// shared edge's two opposite vertices form a convex quad whose four corner
/// angles are all within `max_angle` (radians) of 90°, and the two triangle
/// normals agree.
pub fn tris_to_quads(mesh: &Mesh, max_angle: f64) -> Mesh {
    let pos = mesh.positions();
    let polys = mesh.polygons();
    let mut tri: Vec<Option<[usize; 3]>> = polys
        .iter()
        .map(|p| {
            if p.len() == 3 {
                Some([p[0].0, p[1].0, p[2].0])
            } else {
                None
            }
        })
        .collect();
    let others: Vec<Vec<usize>> = polys
        .iter()
        .filter(|p| p.len() != 3)
        .map(|p| p.iter().map(|v| v.0).collect())
        .collect();

    // edge → (tri idx, opposite vertex)
    let mut edge_tri: HashMap<(usize, usize), Vec<(usize, usize)>> = HashMap::new();
    for (ti, t) in tri.iter().enumerate() {
        let Some(t) = t else { continue };
        for k in 0..3 {
            let (a, b, c) = (t[k], t[(k + 1) % 3], t[(k + 2) % 3]);
            edge_tri
                .entry((a.min(b), a.max(b)))
                .or_default()
                .push((ti, c));
        }
    }

    // Score candidate merges; take best-first, non-overlapping.
    let mut candidates: Vec<(f64, usize, usize, [usize; 4])> = Vec::new();
    for (&(a, b), pair) in &edge_tri {
        if pair.len() != 2 {
            continue;
        }
        let ((t0, c0), (t1, c1)) = (pair[0], pair[1]);
        // Quad a, c0, b, c1 (winding may need the tri order; use c0,a,c1,b).
        let quad = [c0, a, c1, b];
        if !convex_planar(&pos, &quad) {
            continue;
        }
        let dev = quad_angle_deviation(&pos, &quad);
        if dev <= max_angle {
            candidates.push((dev, t0, t1, quad));
        }
    }
    candidates.sort_by(|x, y| x.0.partial_cmp(&y.0).unwrap());

    let mut used: HashSet<usize> = HashSet::new();
    let mut quads: Vec<Vec<usize>> = Vec::new();
    for (_, t0, t1, quad) in candidates {
        if used.contains(&t0) || used.contains(&t1) {
            continue;
        }
        used.insert(t0);
        used.insert(t1);
        quads.push(quad.to_vec());
        tri[t0] = None;
        tri[t1] = None;
    }

    let mut faces = others;
    faces.extend(quads);
    faces.extend(tri.into_iter().flatten().map(|t| t.to_vec()));
    Mesh::from_polygons(&pos, &faces)
}

fn min_angle4(pos: &[Vec3], a: usize, b: usize, c: usize, d: usize, diag_ac: bool) -> f64 {
    let ang = |o: usize, p: usize, q: usize| {
        let u = pos[p].sub(pos[o]);
        let v = pos[q].sub(pos[o]);
        (u.dot(v) / (u.length() * v.length() + 1e-12))
            .clamp(-1.0, 1.0)
            .acos()
    };
    if diag_ac {
        [
            ang(a, b, c),
            ang(b, c, a),
            ang(c, a, b),
            ang(a, c, d),
            ang(c, d, a),
            ang(d, a, c),
        ]
    } else {
        [
            ang(b, c, d),
            ang(c, d, b),
            ang(d, b, c),
            ang(b, d, a),
            ang(d, a, b),
            ang(a, b, d),
        ]
    }
    .into_iter()
    .fold(f64::MAX, f64::min)
}

fn convex_planar(pos: &[Vec3], q: &[usize; 4]) -> bool {
    let p: Vec<Vec3> = q.iter().map(|&i| pos[i]).collect();
    // Planar-ish: all cross products point the same way.
    let mut ref_n = Vec3::ZERO;
    for i in 0..4 {
        let a = p[(i + 1) % 4].sub(p[i]);
        let b = p[(i + 2) % 4].sub(p[(i + 1) % 4]);
        let n = a.cross(b);
        if ref_n.length() < 1e-12 {
            ref_n = n;
        } else if ref_n.dot(n) <= 0.0 {
            return false;
        }
    }
    ref_n.length() > 1e-12
}

fn quad_angle_deviation(pos: &[Vec3], q: &[usize; 4]) -> f64 {
    let ang = |o: usize, p: usize, r: usize| {
        let u = pos[p].sub(pos[o]);
        let v = pos[r].sub(pos[o]);
        (u.dot(v) / (u.length() * v.length() + 1e-12))
            .clamp(-1.0, 1.0)
            .acos()
    };
    let half_pi = std::f64::consts::FRAC_PI_2;
    (0..4)
        .map(|i| (ang(q[i], q[(i + 1) % 4], q[(i + 3) % 4]) - half_pi).abs())
        .fold(0.0, f64::max)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::primitives;

    #[test]
    fn poke_a_cube_makes_a_triangle_fan_per_face() {
        let m = primitives::cube(2.0);
        let p = poke_faces(&m, 0.0);
        assert_eq!(p.face_count(), 6 * 4, "each quad → 4 triangles");
        assert_eq!(p.vertex_count(), 8 + 6, "one centre vertex per face");
        assert_eq!(p.euler_characteristic(), 2);
    }

    #[test]
    fn poke_offset_lifts_the_centre() {
        let m = primitives::grid(1, 1, 2.0);
        let p = poke_faces(&m, 0.5);
        // The new vertex (last) is off the z = 0 plane.
        let c = p
            .vertex(crate::mesh::VertexId(p.vertex_count() - 1))
            .unwrap()
            .position;
        assert!((c.z.abs() - 0.5).abs() < 1e-9);
    }

    #[test]
    fn triangulate_quads_shortest_diagonal() {
        let m = primitives::cube(2.0);
        let t = triangulate_quads(&m, QuadMethod::ShortestDiagonal);
        assert_eq!(t.face_count(), 12, "6 quads → 12 triangles");
        assert_eq!(t.euler_characteristic(), 2);
    }

    #[test]
    fn tris_to_quads_rebuilds_a_grid() {
        // A grid triangulated, then joined back.
        let g = primitives::grid(3, 3, 3.0);
        let t = crate::triangulate::triangulate(&g);
        assert_eq!(t.face_count(), 18);
        let q = tris_to_quads(&t, 0.2);
        assert_eq!(q.face_count(), 9, "back to 9 quads");
    }

    #[test]
    fn tris_to_quads_leaves_non_convex_pairs_as_triangles() {
        // Two triangles whose union is a non-convex quad (corner `c` dents in).
        let mut m = Mesh::new();
        let a = m.add_vertex(Vec3::new(0.0, 0.0, 0.0));
        let b = m.add_vertex(Vec3::new(2.0, 0.0, 0.0));
        let c = m.add_vertex(Vec3::new(0.4, 0.4, 0.0));
        let d = m.add_vertex(Vec3::new(0.0, 2.0, 0.0));
        m.add_face(&[a, b, c]);
        m.add_face(&[a, c, d]);
        let q = tris_to_quads(&m, 1.0);
        assert_eq!(q.face_count(), 2, "non-convex union does not merge");
    }
}
