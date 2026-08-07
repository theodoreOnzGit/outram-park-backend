// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Implements: E. Catmull and J. Clark, "Recursively generated B-spline surfaces on
// arbitrary topological meshes", Computer-Aided Design 10(6), 1978, pp. 350-355;
// with the standard vertex-point weights of J. Stam, "Exact Evaluation of
// Catmull-Clark Subdivision Surfaces at Arbitrary Parameter Values", SIGGRAPH '98,
// pp. 395-404.
// Written from the published formulation; no upstream source was copied.
// Blender analogue (architecture only): the Subdivision-Surface modifier
// (OpenSubdiv-backed upstream).
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

//! Catmull-Clark subdivision surface via **local stencils** (no global solve).
//!
//! Catmull-Clark refinement takes any polygon mesh and produces a smoother,
//! denser, **all-quad** mesh that converges toward the Catmull-Clark limit
//! surface as the level increases. This module implements one round of
//! refinement with the classic three local point stencils and iterates it
//! `levels` times. Each new point is a fixed affine combination of a small
//! neighbourhood of the input mesh — there is no linear system to solve.
//!
//! # The three stencils (one refinement level)
//!
//! Working from the polygon soup ([`Mesh::positions`] + [`Mesh::polygons`]),
//! this builds its own undirected-edge adjacency (canonical key = the sorted
//! pair of endpoint vertex indices) recording, for each edge, its two endpoints
//! and the faces incident to it (1 incident face = a boundary/crease edge,
//! 2 = interior).
//!
//! - **Face point** `F_f` — the centroid (vertex average) of face `f`.
//! - **Edge point**:
//!   - *interior* edge (2 incident faces): `(v0 + v1 + F_a + F_b) / 4`, the
//!     average of the two endpoints and the two adjacent face points.
//!   - *boundary* edge (1 incident face): the endpoint midpoint `(v0 + v1) / 2`
//!     (crease rule — the boundary curve is refined but not pulled inward).
//! - **Vertex point** for an old vertex `P` of valence `n` (number of incident
//!   edges):
//!   - *interior* vertex (every incident edge interior):
//!     `(F_avg + 2*R_avg + (n - 3)*P) / n`, where `F_avg` is the average of the
//!     incident face points and `R_avg` is the average of the incident edge
//!     **midpoints** (the raw edge midpoints, not the edge points above).
//!   - *boundary* vertex (at least one incident boundary edge):
//!     `(m0 + 6*P + m1) / 8`, where `m0`, `m1` are the midpoints of the two
//!     incident boundary edges (crease rule). This keeps a straight boundary
//!     run exactly on its line and holds the boundary curve's shape fixed.
//!
//! # New topology
//!
//! Each old `n`-gon face emits exactly `n` quads. For every corner vertex `Pi`
//! of the face, with incoming edge `e_prev` and outgoing edge `e_next` (in the
//! old face's winding), one quad is emitted:
//! `[FacePoint_f, EdgePoint(e_prev), VertexPoint(Pi), EdgePoint(e_next)]`,
//! wound consistently with the old face. Every output face is therefore a quad.
//!
//! All new points are deduplicated (one face point per face, one edge point per
//! undirected edge, one vertex point per old vertex), assigned contiguous
//! indices `[vertex points | edge points | face points]`, and rebuilt through
//! [`Mesh::from_polygons`] (which recomputes edge dedup and loop wiring).
//!
//! # Valid inputs and boundary handling
//!
//! Accepts **any** polygon mesh: triangles, quads, or higher `n`-gons; closed
//! (e.g. a cube) or open patches with a boundary (e.g. a grid). Boundary edges
//! and boundary vertices use the crease stencils above, so an open patch stays
//! a disc (`chi` unchanged) and its boundary loop keeps its shape.
//!
//! # Limitations
//!
//! - **Non-manifold edges (>2 incident faces) are not a validated case.** They
//!   do not panic: an edge with `k >= 2` incident faces uses the generalised
//!   interior edge point `(v0 + v1 + sum of the k face points) / (2 + k)`, which
//!   reduces to the standard `/4` rule at `k = 2`. A boundary vertex that does
//!   not have exactly two incident boundary edges (a dangling or non-manifold
//!   boundary) is held fixed as a safe fallback. Neither case is covered by the
//!   V&V tests below.

use std::collections::HashMap;

use crate::math::Vec3;
use crate::mesh::Mesh;

/// Apply `levels` rounds of Catmull-Clark subdivision to `mesh`.
///
/// Returns a new, denser, all-quad [`Mesh`] converging toward the Catmull-Clark
/// limit surface. Each level applies the face-point / edge-point / vertex-point
/// stencils documented at the module level ([`crate::subdivision`]) once.
///
/// - `levels == 0` returns a plain clone of the input (identical topology and
///   positions).
/// - `levels == 1` on an `n`-gon mesh yields one all-quad mesh where every old
///   face has become `n` quads.
///
/// Accepts any polygon mesh (tris, quads, or higher `n`-gons; closed or open
/// with a boundary). Boundary edges and vertices are treated with the crease
/// stencils, so an open patch keeps its boundary shape and its Euler
/// characteristic. The output is always an all-quad mesh.
pub fn catmull_clark(mesh: &Mesh, levels: u32) -> Mesh {
    let mut current = mesh.clone();
    for _ in 0..levels {
        current = subdivide_once(&current);
    }
    current
}

/// Canonical undirected-edge key: the sorted pair of endpoint vertex indices.
fn edge_key(a: usize, b: usize) -> (usize, usize) {
    if a < b {
        (a, b)
    } else {
        (b, a)
    }
}

/// One Catmull-Clark refinement level, expressed as a pure function over the
/// polygon-soup view of `mesh`.
fn subdivide_once(mesh: &Mesh) -> Mesh {
    let positions = mesh.positions();
    let polygons = mesh.polygons();
    let v_count = positions.len();

    // ---- Build undirected-edge adjacency from the polygon soup. -----------
    // edge_index maps a canonical key to a dense edge index; edge_verts stores
    // each edge's endpoints; edge_faces lists the faces incident to each edge
    // (len 1 = boundary, len 2 = interior, len > 2 = non-manifold).
    let mut edge_index: HashMap<(usize, usize), usize> = HashMap::new();
    let mut edge_verts: Vec<(usize, usize)> = Vec::new();
    let mut edge_faces: Vec<Vec<usize>> = Vec::new();

    for (f, poly) in polygons.iter().enumerate() {
        let n = poly.len();
        for i in 0..n {
            let a = poly[i].0;
            let b = poly[(i + 1) % n].0;
            let key = edge_key(a, b);
            let idx = *edge_index.entry(key).or_insert_with(|| {
                let e = edge_verts.len();
                edge_verts.push(key);
                edge_faces.push(Vec::new());
                e
            });
            edge_faces[idx].push(f);
        }
    }
    let e_count = edge_verts.len();

    // ---- Face points: centroid of each face's vertices. -------------------
    let face_point: Vec<Vec3> = polygons
        .iter()
        .map(|poly| {
            let mut acc = Vec3::ZERO;
            for v in poly {
                acc = acc.add(positions[v.0]);
            }
            acc.scale(1.0 / poly.len() as f64)
        })
        .collect();

    // ---- Edge points. -----------------------------------------------------
    let mut edge_point: Vec<Vec3> = Vec::with_capacity(e_count);
    for e in 0..e_count {
        let (a, b) = edge_verts[e];
        let va = positions[a];
        let vb = positions[b];
        let faces = &edge_faces[e];
        let ep = if faces.len() == 1 {
            // Boundary/crease edge: plain endpoint midpoint.
            va.add(vb).scale(0.5)
        } else {
            // Interior edge: (v0 + v1 + F_a + F_b) / 4 at 2 faces; the same
            // averaging generalises to k faces as (v0 + v1 + sum F) / (2 + k).
            let mut acc = va.add(vb);
            for &f in faces {
                acc = acc.add(face_point[f]);
            }
            acc.scale(1.0 / (2 + faces.len()) as f64)
        };
        edge_point.push(ep);
    }

    // ---- Per-vertex incidence (edges and faces touching each old vertex). --
    let mut vert_edges: Vec<Vec<usize>> = vec![Vec::new(); v_count];
    let mut vert_faces: Vec<Vec<usize>> = vec![Vec::new(); v_count];
    for (e, &(a, b)) in edge_verts.iter().enumerate() {
        vert_edges[a].push(e);
        vert_edges[b].push(e);
    }
    for (f, poly) in polygons.iter().enumerate() {
        for v in poly {
            vert_faces[v.0].push(f);
        }
    }

    // ---- Vertex points. ---------------------------------------------------
    let mut vertex_point: Vec<Vec3> = Vec::with_capacity(v_count);
    for v in 0..v_count {
        let p = positions[v];
        let edges = &vert_edges[v];
        let boundary_edges: Vec<usize> = edges
            .iter()
            .copied()
            .filter(|&e| edge_faces[e].len() == 1)
            .collect();

        let vp = if boundary_edges.is_empty() {
            // Interior vertex: (F_avg + 2*R_avg + (n - 3)*P) / n.
            let n = edges.len() as f64;

            let faces = &vert_faces[v];
            let mut f_acc = Vec3::ZERO;
            for &f in faces {
                f_acc = f_acc.add(face_point[f]);
            }
            let f_avg = f_acc.scale(1.0 / faces.len() as f64);

            // R_avg = average of the incident edge *midpoints* (not the edge
            // points): each edge contributes (v0 + v1) / 2.
            let mut r_acc = Vec3::ZERO;
            for &e in edges {
                let (a, b) = edge_verts[e];
                r_acc = r_acc.add(positions[a].add(positions[b]).scale(0.5));
            }
            let r_avg = r_acc.scale(1.0 / edges.len() as f64);

            f_avg
                .add(r_avg.scale(2.0))
                .add(p.scale(n - 3.0))
                .scale(1.0 / n)
        } else if boundary_edges.len() == 2 {
            // Boundary/crease vertex: (m0 + 6*P + m1) / 8.
            let midpoint = |e: usize| {
                let (a, b) = edge_verts[e];
                positions[a].add(positions[b]).scale(0.5)
            };
            let m0 = midpoint(boundary_edges[0]);
            let m1 = midpoint(boundary_edges[1]);
            m0.add(p.scale(6.0)).add(m1).scale(1.0 / 8.0)
        } else {
            // Dangling/non-manifold boundary vertex: hold fixed (see module
            // Limitations). Not a validated case.
            p
        };
        vertex_point.push(vp);
    }

    // ---- Assemble deduplicated new vertices with contiguous indices. -------
    // Layout: [ vertex points (v_count) | edge points (e_count) | face points ].
    let mut new_positions: Vec<Vec3> =
        Vec::with_capacity(v_count + e_count + polygons.len());
    new_positions.extend_from_slice(&vertex_point);
    new_positions.extend_from_slice(&edge_point);
    new_positions.extend_from_slice(&face_point);

    let vp_index = |v: usize| v;
    let ep_index = |e: usize| v_count + e;
    let fp_index = |f: usize| v_count + e_count + f;

    // ---- Emit one quad per corner of each old face. -----------------------
    let mut new_faces: Vec<Vec<usize>> = Vec::with_capacity(new_positions.len());
    for (f, poly) in polygons.iter().enumerate() {
        let n = poly.len();
        for i in 0..n {
            let prev = poly[(i + n - 1) % n].0;
            let cur = poly[i].0;
            let next = poly[(i + 1) % n].0;
            let e_prev = edge_index[&edge_key(prev, cur)];
            let e_next = edge_index[&edge_key(cur, next)];
            new_faces.push(vec![
                fp_index(f),
                ep_index(e_prev),
                vp_index(cur),
                ep_index(e_next),
            ]);
        }
    }

    Mesh::from_polygons(&new_positions, &new_faces)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a cube of the given `size`, centred at the origin, as a polygon
    /// soup (8 corner vertices, 6 quad faces). Corner index 6 is the
    /// `(+a, +a, +a)` corner with `a = size / 2`.
    fn cube(size: f64) -> Mesh {
        let a = size / 2.0;
        let positions = [
            Vec3::new(-a, -a, -a), // 0
            Vec3::new(a, -a, -a),  // 1
            Vec3::new(a, a, -a),   // 2
            Vec3::new(-a, a, -a),  // 3
            Vec3::new(-a, -a, a),  // 4
            Vec3::new(a, -a, a),   // 5
            Vec3::new(a, a, a),    // 6
            Vec3::new(-a, a, a),   // 7
        ];
        let faces = vec![
            vec![0, 1, 2, 3],
            vec![4, 5, 6, 7],
            vec![0, 1, 5, 4],
            vec![1, 2, 6, 5],
            vec![2, 3, 7, 6],
            vec![3, 0, 4, 7],
        ];
        Mesh::from_polygons(&positions, &faces)
    }

    /// Build a 2x2-quad grid patch (3x3 vertices) in the z = 0 plane, unit
    /// spacing, corner at the origin. Vertex index `j*3 + i` sits at `(i, j, 0)`
    /// for `i, j` in `0..=2`. This is an open disc (`chi = 1`) with a boundary.
    fn grid_2x2() -> Mesh {
        let mut positions = Vec::new();
        for j in 0..3 {
            for i in 0..3 {
                positions.push(Vec3::new(i as f64, j as f64, 0.0));
            }
        }
        let gi = |i: usize, j: usize| j * 3 + i;
        let mut faces = Vec::new();
        for cj in 0..2 {
            for ci in 0..2 {
                faces.push(vec![
                    gi(ci, cj),
                    gi(ci + 1, cj),
                    gi(ci + 1, cj + 1),
                    gi(ci, cj + 1),
                ]);
            }
        }
        Mesh::from_polygons(&positions, &faces)
    }

    /// Largest absolute coordinate over all vertices — half-extent of the
    /// axis-aligned bounding box for an origin-centred mesh.
    fn half_extent(mesh: &Mesh) -> f64 {
        mesh.positions()
            .iter()
            .map(|p| p.x.abs().max(p.y.abs()).max(p.z.abs()))
            .fold(0.0_f64, f64::max)
    }

    /// **Methodology.** Apply one Catmull-Clark level to a cube (8 verts, 12
    /// edges, 6 quads, `chi = 2`) and check the refined element counts. One
    /// level must create 8 vertex points (one per old corner) + 12 edge points
    /// (one per old edge) + 6 face points (one per old face) = 26 vertices, and
    /// 6 faces x 4 corners = 24 quad faces. For the resulting closed genus-0
    /// all-quad mesh, `chi = V - E + F = 2` forces `E = 26 + 24 - 2 = 48`.
    ///
    /// **Results (2026-07-17, deterministic geometry).** Measured V = 26,
    /// E = 48, F = 24, `chi = 2` — matching the derivation exactly. Confirms the
    /// stencil dedup (face point once per face, edge point once per edge, vertex
    /// point once per vertex) and the all-quad topology.
    #[test]
    fn cube_one_level_counts() {
        let m = catmull_clark(&cube(2.0), 1);
        assert_eq!(m.vertex_count(), 26, "8 vertex + 12 edge + 6 face points");
        assert_eq!(m.face_count(), 24, "6 faces x 4 quads each");
        assert_eq!(m.edge_count(), 48, "closed all-quad chi=2 => E=48");
        assert_eq!(m.euler_characteristic(), 2, "closed genus-0 surface");
    }

    /// **Methodology.** Limit-surface smoothness sanity on a convex solid. A
    /// cube of `size = 2` has corners at distance `r0 = sqrt(3)/2 * size =
    /// sqrt(3) ~= 1.7320508` from the origin. The `(+1, +1, +1)` corner (index
    /// 6) has valence `n = 3` with all edges interior, so its vertex point is
    /// `(F_avg + 2*R_avg) / 3`. Because the cube is convex, this must move the
    /// corner strictly inward: the new distance must be `< r0`. The vertex-point
    /// layout puts old vertex `i` at output index `i`, so index 6 is the moved
    /// corner.
    ///
    /// **Results (2026-07-17, deterministic geometry).** The corner moves to
    /// exactly `(5/9, 5/9, 5/9) = (0.5555556, 0.5555556, 0.5555556)`, new
    /// distance `sqrt(3) * 5/9 ~= 0.9622504`, i.e. `5/9 ~= 0.5556` of `r0`.
    /// `0.9622504 < 1.7320508`, so the corner moves strictly inward as required
    /// for a convex solid converging to its Catmull-Clark limit surface.
    #[test]
    fn cube_corner_moves_inward() {
        let size = 2.0;
        let r0 = 3.0_f64.sqrt() / 2.0 * size; // = sqrt(3) ~= 1.7320508
        let m = catmull_clark(&cube(size), 1);
        let corner = m.positions()[6];
        let new_dist = corner.length();
        assert!(
            new_dist < r0,
            "corner must move inward: new_dist={new_dist} >= r0={r0}"
        );
        // Exact expected corner is (5/9, 5/9, 5/9); new_dist = sqrt(3)*5/9.
        let expected = 3.0_f64.sqrt() * 5.0 / 9.0;
        assert!(
            (new_dist - expected).abs() < 1e-9,
            "new_dist {new_dist} != expected {expected}"
        );
    }

    /// **Methodology.** Convergence of the bounding box toward the (smaller)
    /// Catmull-Clark limit surface of a convex cube. Starting from a `size = 2`
    /// cube (half-extent 1.0), the mesh is refined 3 levels and the AABB
    /// half-extent (largest absolute coordinate) is compared against the
    /// original. It must shrink, because every corner and, after the first
    /// level, every face-centroid vertex is pulled inward. Note: at level 1 the
    /// half-extent is still exactly 1.0, because each face centroid keeps the
    /// face-plane coordinate (e.g. the `+x` face point stays at `x = 1`); the
    /// shrink first appears at level 2, so the test uses >= 2 levels.
    ///
    /// **Results (2026-07-17, deterministic geometry).** Half-extents:
    /// L0 = 1.000000000, L1 = 1.000000000, L2 = 0.878472222, L3 = 0.849175347.
    /// Shrink factor L0 -> L2 = 0.8784722, L0 -> L3 = 0.8491753. The box
    /// contracts monotonically after level 1, consistent with convergence to a
    /// rounded limit surface strictly inside the original cube.
    #[test]
    fn cube_bounding_box_shrinks() {
        let base = cube(2.0);
        let e0 = half_extent(&base);
        assert!((e0 - 1.0).abs() < 1e-12, "cube half-extent should be 1.0");

        let m3 = catmull_clark(&base, 3);
        let e3 = half_extent(&m3);
        assert!(
            e3 < e0,
            "bounding box must shrink after 3 levels: e3={e3} >= e0={e0}"
        );
        // Recorded measured factor 0.8491753 (L0 -> L3).
        let factor = e3 / e0;
        assert!(
            (factor - 0.849_175_347).abs() < 1e-6,
            "shrink factor {factor} != measured 0.849175347"
        );
    }

    /// **Methodology.** Open-patch (boundary) behaviour on a 2x2-quad grid
    /// (9 verts, 12 edges, 4 quads, `chi = 1` — a disc). One Catmull-Clark level
    /// must keep it a disc (`chi = 1`) and preserve the boundary. Expected
    /// counts: 9 vertex + 12 edge + 4 face points = 25 vertices; 4 faces x 4 =
    /// 16 quads; for an all-quad disc, `E = V + F - chi = 25 + 16 - 1 = 40`.
    /// Boundary preservation is checked two ways using the crease stencils:
    /// (1) the corner `(0,0)` (index 0) stays a corner — it moves inward but
    /// remains strictly inside the original `[0,2]^2` bounding box; and (2) a
    /// straight-boundary-edge vertex `(1,0)` (index 1), whose two boundary edges
    /// are collinear along `y = 0`, stays exactly on that line (indeed fixed at
    /// `(1,0,0)`), so the boundary line is preserved.
    ///
    /// **Results (2026-07-17, deterministic geometry).** Measured V = 25,
    /// E = 40, F = 16, `chi = 1`. Corner `(0,0)` moves to exactly
    /// `(0.0625, 0.0625, 0)` — inside the box, still the corner of the boundary
    /// loop. Straight-boundary vertex `(1,0)` maps to exactly `(1, 0, 0)`,
    /// unmoved and on the line `y = 0`. Confirms the crease rule keeps the
    /// boundary curve's shape and the patch topology (still a disc).
    #[test]
    fn grid_one_level_preserves_boundary() {
        let g = grid_2x2();
        assert_eq!(g.euler_characteristic(), 1, "grid patch is a disc");

        let m = catmull_clark(&g, 1);
        assert_eq!(m.vertex_count(), 25, "9 vertex + 12 edge + 4 face points");
        assert_eq!(m.face_count(), 16, "4 faces x 4 quads each");
        assert_eq!(m.edge_count(), 40, "all-quad disc chi=1 => E=40");
        assert_eq!(m.euler_characteristic(), 1, "still a disc after refinement");

        // Corner (0,0) -> (0.0625, 0.0625, 0): inside original [0,2]^2 box.
        let corner = m.positions()[0];
        assert!(
            (corner.x - 0.0625).abs() < 1e-9
                && (corner.y - 0.0625).abs() < 1e-9
                && corner.z.abs() < 1e-9,
            "corner moved to {corner:?}, expected (0.0625, 0.0625, 0)"
        );
        assert!(
            corner.x > 0.0 && corner.x < 2.0 && corner.y > 0.0 && corner.y < 2.0,
            "corner {corner:?} must stay inside the original bounding box"
        );

        // Straight-boundary vertex (1,0) stays exactly on the line y = 0.
        let edge_mid_vert = m.positions()[1];
        assert!(
            (edge_mid_vert.x - 1.0).abs() < 1e-9
                && edge_mid_vert.y.abs() < 1e-9
                && edge_mid_vert.z.abs() < 1e-9,
            "straight-boundary vertex moved to {edge_mid_vert:?}, expected (1, 0, 0)"
        );
    }

    /// **Methodology.** `levels == 0` is the identity: the contract says it
    /// returns a clone of the input. Checked on the cube by comparing element
    /// counts and the Euler characteristic against the untouched mesh.
    ///
    /// **Results (2026-07-17).** V, E, F, and `chi` all match the input cube
    /// (8, 12, 6, 2) exactly — `levels == 0` performs no refinement.
    #[test]
    fn zero_levels_is_identity() {
        let base = cube(2.0);
        let m = catmull_clark(&base, 0);
        assert_eq!(m.vertex_count(), base.vertex_count());
        assert_eq!(m.edge_count(), base.edge_count());
        assert_eq!(m.face_count(), base.face_count());
        assert_eq!(m.euler_characteristic(), base.euler_characteristic());
    }
}
