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
    /// The shorter of the two diagonals. Same rule as
    /// [`crate::triangulate::QuadMethod::ShortEdge`].
    ShortestDiagonal,
    /// Always `v0–v2`. Same as [`crate::triangulate::QuadMethod::Fixed`].
    Fixed,
    /// Always `v1–v3`. Same as [`crate::triangulate::QuadMethod::Alternate`].
    FixedAlternate,
    /// The diagonal that gives the better-shaped triangle pair, decided by
    /// Blender's own rule — see [`crate::triangulate::QuadMethod::Beauty`].
    ///
    /// **Changed 2026-09-19.** This variant previously used a hand-rolled
    /// "maximise the smallest of the four resulting angles" measure, written
    /// before Blender's own rule was ported. It now delegates to the ported
    /// rule (`is_quad_flip_v3`, then area-over-perimeter), so the crate has
    /// one definition of "beauty" rather than two that drift. The two agree
    /// on planar convex quads and differ on warped or concave ones, where
    /// the upstream rule is the one that avoids folding the pair.
    Beauty,
}

impl From<QuadMethod> for crate::triangulate::QuadMethod {
    fn from(m: QuadMethod) -> Self {
        match m {
            QuadMethod::ShortestDiagonal => crate::triangulate::QuadMethod::ShortEdge,
            QuadMethod::Fixed => crate::triangulate::QuadMethod::Fixed,
            QuadMethod::FixedAlternate => crate::triangulate::QuadMethod::Alternate,
            QuadMethod::Beauty => crate::triangulate::QuadMethod::Beauty,
        }
    }
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
                // One definition of the diagonal choice, in `triangulate`,
                // which is the port of upstream's `BM_face_triangulate`
                // switch. Keeping a second copy here is how the two silently
                // drift apart (workspace CLAUDE.md, "Search the workspace
                // before building anything").
                let quad = [pos[vs[0]], pos[vs[1]], pos[vs[2]], pos[vs[3]]];
                for t in crate::triangulate::split_quad(&quad, method.into()) {
                    faces.push(vec![vs[t[0]], vs[t[1]], vs[t[2]]]);
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

/// The superseded hand-rolled "beauty" measure: the smallest of the four
/// angles produced by a given diagonal.
///
/// Retained **only as a test fixture**, so the 2026-09-19 switch to
/// Blender's own rule can be shown rather than asserted. Not part of the
/// operator any more — see [`QuadMethod::Beauty`].
#[cfg(test)]
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

    /// Evidence for replacing this module's hand-rolled beauty rule with
    /// Blender's.
    ///
    /// # Methodology
    ///
    /// Sweep the same 625-quad family used in `triangulate` — three corners
    /// moved over a 5-value grid, spanning non-planar quads and planar
    /// darts. For each, compare the diagonal the old min-angle rule picks
    /// against the one the ported upstream rule picks, and count how often
    /// the old rule chooses a diagonal that FOLDS the triangle pair.
    ///
    /// # Results (measured 2026-09-19)
    ///
    /// The two rules disagree on **246 of 625** quads. On the 576 where
    /// exactly one diagonal folds, the old min-angle rule picked the
    /// folding one **242 times (42 %)**; the ported upstream rule picked it
    /// **zero** times. Maximising the smallest angle says nothing about
    /// whether the two triangles point the same way, which is exactly why
    /// upstream consults `is_quad_flip_v3` before measuring anything.
    ///
    /// That is the justification for the switch: not that the old rule was
    /// arbitrary, but that it was blind to folding.
    #[test]
    fn the_ported_beauty_rule_avoids_folds_the_old_one_did_not() {
        let vals = [-1.0f64, -0.4, 0.0, 0.4, 1.0];
        let mut disagreements = 0;
        let mut old_folds = 0;
        let mut new_folds = 0;
        let mut single_fold_cases = 0;

        for &z0 in &vals {
            for &z1 in &vals {
                for &a in &vals {
                    for &b in &vals {
                        let pts = [
                            Vec3::new(0.0, 0.0, z0),
                            Vec3::new(1.0, 0.0, z1),
                            Vec3::new(0.3 + 0.2 * a, 0.3 + 0.2 * b, 0.0),
                            Vec3::new(0.0, 1.0, 0.0),
                        ];
                        let idx = [0usize, 1, 2, 3];

                        // Old rule.
                        let old_02 = min_angle4(&pts, idx[0], idx[1], idx[2], idx[3], true)
                            >= min_angle4(&pts, idx[0], idx[1], idx[2], idx[3], false);
                        // Ported rule.
                        let new_split = crate::triangulate::split_quad(
                            &pts,
                            crate::triangulate::QuadMethod::Beauty,
                        );
                        let new_02 = new_split == [[0, 1, 2], [0, 2, 3]];
                        if old_02 != new_02 {
                            disagreements += 1;
                        }

                        let folds = |diag_02: bool| {
                            let t = if diag_02 {
                                [[0usize, 1, 2], [0, 2, 3]]
                            } else {
                                [[1usize, 2, 3], [1, 3, 0]]
                            };
                            let nrm = |q: [usize; 3]| {
                                let (x, y, z) = (pts[q[0]], pts[q[1]], pts[q[2]]);
                                y.sub(x).cross(z.sub(x))
                            };
                            nrm(t[0]).dot(nrm(t[1])) < 0.0
                        };
                        // Only meaningful where exactly one diagonal folds.
                        if folds(true) != folds(false) {
                            single_fold_cases += 1;
                            if folds(old_02) {
                                old_folds += 1;
                            }
                            if folds(new_02) {
                                new_folds += 1;
                            }
                        }
                    }
                }
            }
        }

        assert_eq!(single_fold_cases, 576);
        assert_eq!(
            new_folds, 0,
            "the ported rule must never pick a folding diagonal"
        );
        assert!(
            old_folds > 0,
            "the old rule was supposed to be blind to folding; if this now \
             passes with 0 the fixture family has changed"
        );
        assert!(
            disagreements > 0,
            "the two rules should differ somewhere, or the switch was a no-op"
        );
    }
}
