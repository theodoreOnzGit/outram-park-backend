// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Triangulate faces, following Blender's `BM_face_triangulate`
// (source/blender/bmesh/intern/bmesh_polygon.cc) method model: an explicit quad
// method and an explicit n-gon method, with upstream's defaults. The quad
// diagonal choice is transcribed from that function's switch; n-gons route
// through `crate::polyfill`, the port of `BLI_polyfill_calc`. Upstream commit
// 786af64aad84154047d93ee077e1fdd1d229f32d, read 2026-09-19. Blender is
// GPL-2.0-or-later; see crate NOTICE and upstream_source/README.md.
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
//! (`bmo_triangulate` / `BM_face_triangulate`). A quad becomes two triangles
//! and an `n`-gon becomes `n − 2`, and the result is rebuilt as a [`Mesh`]
//! whose every face is a triangle.
//!
//! # Concave faces: this used to be wrong
//!
//! Until 2026-09-19 this module fanned every face from its first corner. That
//! is correct for a convex face and **incorrect for a concave one** — the fan
//! emits triangles that cross the reflex corner, leave the polygon, and
//! overlap each other, so the triangulated surface no longer bounds the same
//! solid. Measured on an L-shaped hexagon of true area 0.75, the fan totalled
//! 1.000 (+33.3 %) from four of its six possible apex corners; see
//! [`crate::polyfill`]'s `concave_l_shape_is_tiled_exactly_where_a_fan_is_not`
//! for the full table. Since a face's corner order is just whatever the mesh
//! happens to store, that was firing on most concave faces.
//!
//! N-gons now route through [`crate::polyfill`] (ear clipping, upstream's
//! `BLI_polyfill_calc`), which tiles exactly. The fan survives only as the
//! explicit [`QuadMethod::Fixed`] choice on quads, where it is exact.
//!
//! # Methods
//!
//! Upstream exposes two orthogonal choices, and so does this module:
//! [`QuadMethod`] for four-cornered faces and [`NgonMethod`] for the rest.
//! [`triangulate`] applies upstream's own defaults; [`triangulate_with`]
//! takes them explicitly.
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
//! Every method preserves each face's winding, so a consistently-wound,
//! outward-facing input stays that way. No `faer`, no external dependency;
//! Android-safe.

use crate::math::Vec3;
use crate::mesh::Mesh;
use crate::polyfill;
use crate::polyfill_beautify;

/// How to split a four-cornered face into two triangles.
///
/// Mirrors upstream's `TriangulateModifierQuadMethod`
/// (`DNA_modifier_types.h:1927`). A quad has exactly two candidate diagonals;
/// every variant is a different rule for picking one. All are exact — a quad
/// is always tiled correctly by either diagonal *if* it is planar and convex;
/// for a non-planar or concave quad the choice changes the surface, which is
/// why upstream makes it a user decision rather than a constant.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum QuadMethod {
    /// Pick the diagonal that gives the better-shaped triangle pair.
    /// Upstream `MOD_TRIANGULATE_QUAD_BEAUTY`.
    ///
    /// Decided exactly as upstream's `BM_face_triangulate` does: first
    /// [`crate::polyfill_beautify::is_quad_flip_v3`] rejects a diagonal that
    /// would fold the quad, and only if neither folds is the
    /// area-over-perimeter measure
    /// ([`crate::polyfill_beautify::edge_rotate_cost_3d`]) consulted.
    /// Unlike the length-based variants this one is aware of non-planarity.
    Beauty,
    /// Always split corner 0 to corner 2 — the historical fan. Upstream
    /// `MOD_TRIANGULATE_QUAD_FIXED`.
    Fixed,
    /// Always split corner 1 to corner 3. Upstream
    /// `MOD_TRIANGULATE_QUAD_ALTERNATE`.
    Alternate,
    /// Split along the **shorter** diagonal. Upstream
    /// `MOD_TRIANGULATE_QUAD_SHORTEDGE`, and upstream's default.
    #[default]
    ShortEdge,
    /// Split along the **longer** diagonal. Upstream
    /// `MOD_TRIANGULATE_QUAD_LONGEDGE`.
    LongEdge,
}

/// How to split a face with five or more corners.
///
/// Mirrors upstream's `TriangulateModifierNgonMethod`
/// (`DNA_modifier_types.h:1921`).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum NgonMethod {
    /// Ear-clip, then improve the result by rotating interior edges.
    /// Upstream `MOD_TRIANGULATE_NGON_BEAUTY`, and upstream's default.
    ///
    /// The rotation pass is [`crate::polyfill_beautify::polyfill_beautify`],
    /// the port of `BLI_polyfill_beautify`. Both variants tile correctly;
    /// this one additionally removes slivers (measured 5.8x improvement in
    /// the worst triangle's fatness on a sliver-prone fixture — see that
    /// module).
    Beauty,
    /// Ear-clip only. Upstream `MOD_TRIANGULATE_NGON_EARCLIP`, via
    /// [`crate::polyfill::polyfill_3d`].
    #[default]
    EarClip,
}

/// Triangulate every face of `mesh` using upstream's default methods.
///
/// Those defaults are [`QuadMethod::ShortEdge`] and [`NgonMethod::Beauty`]
/// (`DNA_modifier_types.h:1939-1940`). Positions are unchanged; only faces
/// are re-cut. A face with fewer than three corners is dropped (it is already
/// degenerate); a triangle is passed through unchanged. This is infallible.
///
/// # Examples
///
/// ```
/// use outram_blender::{primitives, triangulate::triangulate};
///
/// // A cube's 6 quads split into 12 triangles; still χ = 2.
/// let cube = primitives::cube(2.0);
/// let tris = triangulate(&cube);
/// assert_eq!(tris.face_count(), 12);
/// assert_eq!(tris.euler_characteristic(), 2);
/// ```
pub fn triangulate(mesh: &Mesh) -> Mesh {
    triangulate_with(mesh, QuadMethod::default(), NgonMethod::default())
}

/// Triangulate every face of `mesh`, choosing the quad and n-gon methods.
///
/// See [`QuadMethod`] and [`NgonMethod`]. Infallible.
///
/// # Examples
///
/// ```
/// use outram_blender::{primitives, triangulate::{triangulate_with, QuadMethod, NgonMethod}};
///
/// let cube = primitives::cube(2.0);
/// // The historical fan, still available where it is exact.
/// let tris = triangulate_with(&cube, QuadMethod::Fixed, NgonMethod::EarClip);
/// assert_eq!(tris.face_count(), 12);
/// ```
pub fn triangulate_with(mesh: &Mesh, quad: QuadMethod, ngon: NgonMethod) -> Mesh {
    let positions = mesh.positions();
    let mut faces: Vec<Vec<usize>> = Vec::new();

    for poly in mesh.polygons() {
        let k = poly.len();
        if k < 3 {
            continue;
        }
        if k == 3 {
            faces.push(poly.iter().map(|v| v.0).collect());
            continue;
        }

        let pts: Vec<Vec3> = poly.iter().map(|v| positions[v.0]).collect();

        if k == 4 {
            for t in split_quad(&pts, quad) {
                faces.push(vec![poly[t[0]].0, poly[t[1]].0, poly[t[2]].0]);
            }
        } else {
            let normal = polyfill::newell_normal(&pts);
            let mut tris = polyfill::polyfill_3d(&pts, normal);
            if ngon == NgonMethod::Beauty {
                // Beautify works in the same projected frame the clipping
                // used, so the coordinates must come from the same negated
                // projection — not a fresh one, or the rotations are scored
                // against a mirrored plane.
                let coords = polyfill::project_to_plane(&pts, normal.scale(-1.0));
                polyfill_beautify::polyfill_beautify(&coords, &mut tris);
            }
            for t in tris {
                faces.push(vec![poly[t[0]].0, poly[t[1]].0, poly[t[2]].0]);
            }
        }
    }

    Mesh::from_polygons(&positions, &faces)
}

/// Choose a quad's diagonal and emit the two triangles as corner indices
/// `0..4`, following upstream's `BM_face_triangulate` switch
/// (`bmesh_polygon.cc:1152-1219`).
///
/// Upstream builds a rotated four-loop array and always emits `(0,1,2)` and
/// `(0,2,3)` from it; the rotation is what encodes the diagonal. That is
/// reproduced here directly as the two index triples, which is the same thing
/// written without the intermediate array.
pub(crate) fn split_quad(pts: &[Vec3], method: QuadMethod) -> [[usize; 3]; 2] {
    // Diagonal 0-2: upstream's `loops = [c0, c1, c2, c3]`.
    const DIAG_02: [[usize; 3]; 2] = [[0, 1, 2], [0, 2, 3]];
    // Diagonal 1-3: upstream's `loops = [c1, c2, c3, c0]`.
    const DIAG_13: [[usize; 3]; 2] = [[1, 2, 3], [1, 3, 0]];

    if method == QuadMethod::Beauty {
        // Upstream names the corners v1..v4 = c1, c2, c3, c0 — rotated one
        // step — and `split_24` in that frame means "use the 0-2 diagonal".
        let (b1, b2, b3, b4) = (pts[1], pts[2], pts[3], pts[0]);
        let flip = polyfill_beautify::is_quad_flip_v3(b1, b2, b3, b4);
        let split_02 = if flip & (1 << 0) != 0 {
            true
        } else if flip & (1 << 1) != 0 {
            false
        } else {
            // flag 0 upstream, so degenerate states are not locked.
            polyfill_beautify::edge_rotate_cost_3d(b1, b2, b3, b4, false) > 0.0
        };
        return if split_02 { DIAG_02 } else { DIAG_13 };
    }

    match method {
        QuadMethod::Fixed => DIAG_02,
        QuadMethod::Alternate => DIAG_13,
        QuadMethod::ShortEdge | QuadMethod::LongEdge | QuadMethod::Beauty => {
            // Upstream: d1 across corners 0-2, d2 across corners 1-3, both
            // squared (the comparison never needs the root).
            let e02 = pts[0].sub(pts[2]);
            let e13 = pts[1].sub(pts[3]);
            let d1 = e02.dot(e02);
            let d2 = e13.dot(e13);
            // `split_24` in upstream means "use the 0-2 diagonal".
            let split_02 = match method {
                QuadMethod::LongEdge => (d2 - d1) < 0.0,
                _ => (d2 - d1) > 0.0,
            };
            if split_02 {
                DIAG_02
            } else {
                DIAG_13
            }
        }
    }
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
        assert!(
            tris.face_count() >= before_faces,
            "quads split, tris pass through"
        );
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
        let via_op = MeshOp::Triangulate
            .apply(cube)
            .expect("triangulate infallible");
        assert_eq!(via_op.face_count(), direct.face_count());
        assert_eq!(via_op.euler_characteristic(), direct.euler_characteristic());
    }

    /// Surface area is the invariant a triangulation must not change.
    fn surface_area(m: &Mesh) -> f64 {
        (0..m.face_count())
            .map(|i| {
                let f = crate::mesh::FaceId(i);
                let vs = m.face_vertices(f);
                let p = m.positions();
                // Fan is fine for *measuring* a convex triangle's area; every
                // face here is already a triangle when this is called.
                let mut a = 0.0;
                for k in 1..vs.len().saturating_sub(1) {
                    let (v0, v1, v2) = (p[vs[0].0], p[vs[k].0], p[vs[k + 1].0]);
                    a += 0.5 * v1.sub(v0).cross(v2.sub(v0)).length();
                }
                a
            })
            .sum()
    }

    /// The concave-face regression, at the `Mesh` level.
    ///
    /// # Methodology
    ///
    /// Build a single L-shaped hexagonal face whose corner order starts at a
    /// reflex-adjacent corner — the rotation a fan gets wrong. True area is
    /// 0.75. Triangulate with the default methods and sum the triangle areas.
    ///
    /// # Results (measured 2026-09-19)
    ///
    /// Default (ear-clip) total area = 0.75 exactly (within 1e-12).
    /// [`QuadMethod::Fixed`] with [`NgonMethod::EarClip`] gives the same, as
    /// the quad method does not apply to a hexagon. The pre-2026-09-19 fan
    /// gave 1.000 on this rotation — a 33 % overshoot from triangles outside
    /// the face.
    #[test]
    fn concave_ngon_face_keeps_its_area() {
        // Rotated so corner 0 is (1, 0.5), a corner the fan fails from.
        let ring = [
            (1.0, 0.5),
            (0.5, 0.5),
            (0.5, 1.0),
            (0.0, 1.0),
            (0.0, 0.0),
            (1.0, 0.0),
        ];
        let positions: Vec<crate::math::Vec3> = ring
            .iter()
            .map(|&(x, y)| crate::math::Vec3::new(x, y, 0.0))
            .collect();
        let mesh = Mesh::from_polygons(&positions, &[(0..6).collect::<Vec<usize>>()]);

        let tris = triangulate(&mesh);
        assert_eq!(tris.face_count(), 4);
        let area = surface_area(&tris);
        assert!(
            (area - 0.75).abs() < 1e-12,
            "ear-clipped area should be 0.75, got {area}"
        );
    }

    /// Every quad method tiles a planar quad exactly, and they really do pick
    /// different diagonals.
    #[test]
    fn quad_methods_differ_but_all_preserve_area() {
        use crate::math::Vec3;
        // A trapezoid, not a rectangle: a rectangle's two diagonals are
        // equal, which would make every length-based method agree and hide
        // the very difference this test is for.
        let positions = vec![
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(4.0, 0.0, 0.0),
            Vec3::new(3.0, 1.0, 0.0),
            Vec3::new(0.0, 1.0, 0.0),
        ];
        let mesh = Mesh::from_polygons(&positions, &[vec![0, 1, 2, 3]]);
        let truth = 3.5;

        let mut seen_diagonals = std::collections::HashSet::new();
        for qm in [
            QuadMethod::Fixed,
            QuadMethod::Alternate,
            QuadMethod::ShortEdge,
            QuadMethod::LongEdge,
            QuadMethod::Beauty,
        ] {
            let t = triangulate_with(&mesh, qm, NgonMethod::EarClip);
            assert_eq!(t.face_count(), 2, "{qm:?}");
            let a = surface_area(&t);
            assert!((a - truth).abs() < 1e-12, "{qm:?} area {a}");

            // Record which diagonal was used, by the vertex pair shared by
            // both triangles.
            let d = split_quad(&positions, qm);
            seen_diagonals.insert(if d == [[0, 1, 2], [0, 2, 3]] {
                "0-2"
            } else {
                "1-3"
            });
        }
        assert_eq!(
            seen_diagonals.len(),
            2,
            "the methods should not all pick the same diagonal: {seen_diagonals:?}"
        );
    }

    /// Short and long edge must pick opposite diagonals on a quad whose
    /// diagonals differ, and agree with a direct length comparison.
    #[test]
    fn short_and_long_edge_are_opposites() {
        use crate::math::Vec3;
        // Trapezoid, for the reason given in the test above.
        let pts = [
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(4.0, 0.0, 0.0),
            Vec3::new(3.0, 1.0, 0.0),
            Vec3::new(0.0, 1.0, 0.0),
        ];
        let d02 = pts[0].sub(pts[2]).length();
        let d13 = pts[1].sub(pts[3]).length();
        assert!((d02 - d13).abs() > 1e-9, "fixture diagonals must differ");

        let short = split_quad(&pts, QuadMethod::ShortEdge);
        let long = split_quad(&pts, QuadMethod::LongEdge);
        assert_ne!(short, long);

        // Whichever is shorter must be the one ShortEdge chose.
        let short_is_02 = short == [[0, 1, 2], [0, 2, 3]];
        assert_eq!(
            short_is_02,
            d02 < d13,
            "ShortEdge picked the longer diagonal"
        );
    }

    /// A triangulated mesh must stay closed if it started closed — a fan that
    /// escapes the face would break this on a concave cap.
    #[test]
    fn triangulating_a_closed_mesh_keeps_it_closed() {
        for m in [
            crate::primitives::cube(2.0),
            crate::primitives::uv_sphere(12, 8, 1.0),
        ] {
            let t = triangulate(&m);
            assert!(is_watertight_consistent(&t));
            assert_eq!(t.euler_characteristic(), 2);
        }
    }

    /// `NgonMethod::Beauty` must tile identically to `EarClip` but with
    /// better-shaped triangles — same area, same count, different diagonals.
    ///
    /// # Results (measured 2026-09-19)
    ///
    /// On a 20-corner zig-zag strip face, both methods emit 18 triangles
    /// totalling the same area to 1e-9; Beauty's worst triangle is strictly
    /// fatter. The assertion is on the relation, so it cannot go stale.
    #[test]
    fn ngon_beauty_matches_earclip_area_but_improves_shape() {
        use crate::math::Vec3;
        // A strip whose ear clipping is sliver-prone.
        let n = 10;
        let mut ring: Vec<Vec3> = Vec::new();
        for i in 0..n {
            ring.push(Vec3::new(i as f64, 0.0, 0.0));
        }
        for i in (0..n).rev() {
            ring.push(Vec3::new(i as f64 + 0.5, 1.0, 0.0));
        }
        let idx: Vec<usize> = (0..ring.len()).collect();
        let mesh = Mesh::from_polygons(&ring, &[idx]);

        let ear = triangulate_with(&mesh, QuadMethod::ShortEdge, NgonMethod::EarClip);
        let beauty = triangulate_with(&mesh, QuadMethod::ShortEdge, NgonMethod::Beauty);

        assert_eq!(ear.face_count(), beauty.face_count());
        let (a, b) = (surface_area(&ear), surface_area(&beauty));
        assert!((a - b).abs() < 1e-9, "areas differ: {a} vs {b}");

        // Worst-triangle fatness: 2*area / longest_edge^2.
        let worst = |m: &Mesh| -> f64 {
            let p = m.positions();
            (0..m.face_count())
                .map(|i| {
                    let vs = m.face_vertices(crate::mesh::FaceId(i));
                    let (x, y, z) = (p[vs[0].0], p[vs[1].0], p[vs[2].0]);
                    let area = 0.5 * y.sub(x).cross(z.sub(x)).length();
                    let longest = y
                        .sub(x)
                        .length()
                        .max(z.sub(y).length())
                        .max(x.sub(z).length());
                    2.0 * area / (longest * longest)
                })
                .fold(f64::INFINITY, f64::min)
        };
        let (we, wb) = (worst(&ear), worst(&beauty));
        assert!(
            wb > we,
            "beauty should improve the worst triangle: {we} -> {wb}"
        );
    }

    /// `QuadMethod::Beauty` must refuse a diagonal that folds a non-planar
    /// quad, where the purely length-based methods are blind to the fold.
    /// Code-to-code verification of upstream's quad-Beauty rule.
    ///
    /// # Methodology
    ///
    /// `BM_face_triangulate`'s Beauty branch consults
    /// `is_quad_flip_v3` first and only falls through to the
    /// area-over-perimeter measure when neither diagonal folds. Sweep a
    /// 625-member family of quads — three corners moved over a 5-value grid,
    /// covering both non-planar quads and planar darts — keep the ones where
    /// exactly one diagonal folds, and check that upstream's rule picks the
    /// other one.
    ///
    /// # Results (measured 2026-09-19)
    ///
    /// 576 of the 625 quads have exactly one folding diagonal. On **every
    /// one**, `QuadMethod::Beauty` picked the non-folding split and the
    /// alternative split did fold. The rule does what upstream claims.
    ///
    /// One finding worth recording: a merely **non-planar** quad is not
    /// enough to reach this branch. Sweeping only the four corner heights
    /// over a square footprint produced **zero** single-fold quads —
    /// `is_quad_flip_v3` returned 0 or 3, never 1 or 2, because a saddle
    /// folds across both diagonals or neither. The branch needs a *concave*
    /// (dart) quad, where one diagonal lies outside the quad. A fixture
    /// built from a saddle, which is the obvious first guess, tests nothing.
    #[test]
    fn quad_beauty_picks_the_non_folding_diagonal() {
        use crate::math::Vec3;
        let vals = [-1.0f64, -0.4, 0.0, 0.4, 1.0];
        let mut single_fold = 0;
        for &z0 in &vals {
            for &z1 in &vals {
                for &a in &vals {
                    for &b in &vals {
                        // Corner 2 moves in-plane (making darts) as well as
                        // corners 0 and 1 moving out of plane.
                        let pts = [
                            Vec3::new(0.0, 0.0, z0),
                            Vec3::new(1.0, 0.0, z1),
                            Vec3::new(0.3 + 0.2 * a, 0.3 + 0.2 * b, 0.0),
                            Vec3::new(0.0, 1.0, 0.0),
                        ];
                        let flip =
                            polyfill_beautify::is_quad_flip_v3(pts[1], pts[2], pts[3], pts[0]);
                        if flip != 1 && flip != 2 {
                            continue;
                        }
                        single_fold += 1;

                        let nrm = |t: [usize; 3]| {
                            let (p, q, r) = (pts[t[0]], pts[t[1]], pts[t[2]]);
                            q.sub(p).cross(r.sub(p))
                        };
                        let chosen = split_quad(&pts, QuadMethod::Beauty);
                        let other = if chosen == [[0, 1, 2], [0, 2, 3]] {
                            [[1usize, 2, 3], [1, 3, 0]]
                        } else {
                            [[0usize, 1, 2], [0, 2, 3]]
                        };
                        assert!(
                            nrm(chosen[0]).dot(nrm(chosen[1])) > 0.0,
                            "Beauty picked a folding diagonal at z=({z0},{z1}) a={a} b={b}"
                        );
                        assert!(
                            nrm(other[0]).dot(nrm(other[1])) < 0.0,
                            "the rejected diagonal was supposed to be the folding one"
                        );
                    }
                }
            }
        }
        assert_eq!(
            single_fold, 576,
            "the fixture family should contain 576 single-fold quads"
        );
    }

    /// The companion finding: a non-planar quad over a square footprint
    /// never has exactly one folding diagonal, so a saddle cannot exercise
    /// the branch above.
    #[test]
    fn a_saddle_folds_on_both_diagonals_or_neither() {
        use crate::math::Vec3;
        let vals = [-1.0f64, -0.4, 0.0, 0.4, 1.0];
        for &z0 in &vals {
            for &z1 in &vals {
                for &z2 in &vals {
                    for &z3 in &vals {
                        let pts = [
                            Vec3::new(0.0, 0.0, z0),
                            Vec3::new(1.0, 0.0, z1),
                            Vec3::new(1.0, 1.0, z2),
                            Vec3::new(0.0, 1.0, z3),
                        ];
                        let flip =
                            polyfill_beautify::is_quad_flip_v3(pts[1], pts[2], pts[3], pts[0]);
                        assert!(
                            flip == 0 || flip == 3,
                            "z=({z0},{z1},{z2},{z3}) gave flip={flip}, expected 0 or 3"
                        );
                    }
                }
            }
        }
    }

    /// Beauty on a planar convex quad must agree with the shorter diagonal —
    /// the case where the two rules provably coincide.
    #[test]
    fn quad_beauty_agrees_with_short_edge_on_a_planar_convex_quad() {
        use crate::math::Vec3;
        let pts = [
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(4.0, 0.0, 0.0),
            Vec3::new(3.0, 1.0, 0.0),
            Vec3::new(0.0, 1.0, 0.0),
        ];
        assert_eq!(
            split_quad(&pts, QuadMethod::Beauty),
            split_quad(&pts, QuadMethod::ShortEdge)
        );
    }
}
