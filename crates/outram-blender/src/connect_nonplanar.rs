// SPDX-License-Identifier: GPL-3.0-only
//
// ─── Ported from Blender ──────────────────────────────────────────────────
//  Upstream project : Blender <https://github.com/blender/blender>
//  Upstream file    : source/blender/bmesh/operators/bmo_connect_nonplanar.cc
//                     (+ BM_face_calc_normal_subset from
//                      source/blender/bmesh/intern/bmesh_polygon.cc:1036)
//  Upstream commit  : 786af64aad84154047d93ee077e1fdd1d229f32d (sparse clone,
//                     read 2026-09-19; see upstream_source/README.md)
//  Upstream © text  : SPDX-FileCopyrightText: 2023 Blender Authors
//  Upstream licence : SPDX-License-Identifier: GPL-2.0-or-later
//
//  This port © 2026 OUTRAM PARK contributors, distributed as GPL-3.0-only.
//  Blender's "or later" clause is what makes that relicensing permitted; the
//  flow is ONE-WAY. Do not strip this block during refactors (workspace
//  RESEARCH_INTEGRITY_AND_PROVENANCE.md).
// ──────────────────────────────────────────────────────────────────────────
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

//! Split non-planar faces along their flattest diagonal — a port of
//! Blender's **Split Non-Planar Faces** (`bmo_connect_nonplanar.cc`).
//!
//! # What this computes and why a solver frontend needs it
//!
//! A face with four or more corners is only planar by accident. A warped
//! quad has no single well-defined plane, so its normal, its centroid and
//! its area all depend on how you choose to interpret it — and a CFD mesher
//! or a Monte Carlo surface test will each interpret it differently. This
//! operator finds, for each face, the chord that splits it into the two
//! *flattest* halves, and cuts along it when the two halves' normals differ
//! by more than a given angle. It then recurses, so a badly warped n-gon is
//! reduced until every piece is planar to tolerance.
//!
//! Inputs are positions in the caller's length unit; the tolerance is a true
//! angle in **radians** (upstream's `angle_limit` slot). Nothing here has a
//! physical dimension.
//!
//! # The measure: total height variation, not a plane fit
//!
//! Upstream scores a candidate half by `bm_face_subset_calc_planar`: project
//! every corner onto the half's own Newell normal and sum the **absolute
//! successive differences** in that height. This is total variation, not an
//! RMS residual. It punishes a face that zig-zags through the plane more
//! than one that bows smoothly away from it, which is the right bias — a
//! zig-zag is the shape a mesher chokes on. The two halves' scores are
//! summed and the lowest total wins.
//!
//! The *cut decision* is separate from the *cut choice*: having found the
//! best chord, the face is split only if the cosine of the angle between
//! the two halves' normals is below `cos(angle_limit)`. So a gently warped
//! face is left whole.
//!
//! # Fidelity to upstream, and the deviations
//!
//! The `O(N^2)` chord search, the planarity measure, the normal-angle gate
//! and the recursion via a work stack are transcribed from upstream.
//! Differences:
//!
//! 1. **`f64`, not `f32`.**
//! 2. **The legality test is our own.** Upstream calls
//!    `BM_face_splits_check_legal`, which works on live BMesh loops and
//!    knows about existing edges and face doubles. This crate rebuilds
//!    meshes from index rings, so the equivalent question is purely
//!    "is this chord a valid diagonal of this polygon?" — it must stay
//!    inside the ring and cross no edge. [`is_valid_diagonal`] answers that
//!    in the face's own projected plane. It is stricter than upstream's in
//!    one respect (it rejects a chord that merely touches an edge) and
//!    blind to one thing upstream catches (a pre-existing edge elsewhere in
//!    the mesh joining the same two vertices, which would create a double).
//!    Stated rather than papered over.
//! 3. **Rebuild, not in-place split.** Upstream mutates a BMesh; this
//!    returns a new [`Mesh`], like every other operator in this crate.

use crate::math::Vec3;
use crate::mesh::Mesh;
use crate::polyfill;

/// Upstream's default for the **Split Non-Planar Faces** operator: 5°,
/// expressed in radians.
///
/// Faces whose two best halves differ by less than this are left alone.
pub const DEFAULT_ANGLE_LIMIT: f64 = 5.0 * std::f64::consts::PI / 180.0;

/// Upstream `BM_face_calc_normal_subset` (`bmesh_polygon.cc:1036`) — the
/// Newell normal of the corner run `ring[a..=b]`, walking forward with
/// wrap-around, normalised.
///
/// Returns `None` when the run is degenerate (zero-length normal), which is
/// upstream's `== 0.0f` early-out.
fn normal_subset(ring: &[Vec3], a: usize, b: usize) -> Option<Vec3> {
    let n = ring.len();
    let mut no = Vec3::new(0.0, 0.0, 0.0);
    // Newell over the closed run: v_prev starts at the *last* corner of the
    // run, so the run is treated as its own closed loop.
    let mut v_prev = ring[b];
    let mut i = a;
    loop {
        let v_curr = ring[i];
        no = no.add(Vec3::new(
            (v_prev.y - v_curr.y) * (v_prev.z + v_curr.z),
            (v_prev.z - v_curr.z) * (v_prev.x + v_curr.x),
            (v_prev.x - v_curr.x) * (v_prev.y + v_curr.y),
        ));
        v_prev = v_curr;
        if i == b {
            break;
        }
        i = (i + 1) % n;
    }
    let len = no.length();
    if len == 0.0 {
        None
    } else {
        Some(Vec3::new(no.x / len, no.y / len, no.z / len))
    }
}

/// Upstream `bm_face_subset_calc_planar` — how non-planar the corner run
/// `ring[a..=b]` is, measured against `no`.
///
/// The sum of absolute successive changes in height above the plane, walked
/// as a closed loop. Zero for a perfectly planar run.
fn subset_planarity_error(ring: &[Vec3], a: usize, b: usize, no: Vec3) -> f64 {
    let n = ring.len();
    // `dot_m3_v3_row_z(axis_mat, co)` with `axis_dominant_v3_to_m3(no)` is
    // just `dot(no, co)`: the matrix's third column is the normal.
    let mut delta_z = 0.0;
    let mut z_prev = no.dot(ring[b]);
    let mut i = a;
    loop {
        let z_curr = no.dot(ring[i]);
        delta_z += (z_curr - z_prev).abs();
        z_prev = z_curr;
        if i == b {
            break;
        }
        i = (i + 1) % n;
    }
    delta_z
}

/// Is the chord `a`-`b` a usable diagonal of the ring?
///
/// Stands in for upstream's `BM_face_splits_check_legal` — see the module
/// docs for exactly how the two differ. Works in the face's own projected
/// plane: the chord must not be an edge or touch one, must not cross any
/// edge, and must lie inside the polygon.
fn is_valid_diagonal(ring2d: &[[f64; 2]], a: usize, b: usize) -> bool {
    let n = ring2d.len();
    if n < 4 {
        return false;
    }
    // Adjacent corners share an edge, not a diagonal.
    if (a + 1) % n == b || (b + 1) % n == a || a == b {
        return false;
    }

    let pa = ring2d[a];
    let pb = ring2d[b];

    // No proper crossing with any edge that does not share an endpoint.
    for i in 0..n {
        let j = (i + 1) % n;
        if i == a || i == b || j == a || j == b {
            continue;
        }
        if segments_properly_intersect(pa, pb, ring2d[i], ring2d[j]) {
            return false;
        }
    }

    // The chord must leave `a` into the interior, not out through the
    // exterior wedge. Standard "in cone" test.
    in_cone(ring2d, a, b) && in_cone(ring2d, b, a)
}

fn cross2(o: [f64; 2], p: [f64; 2], q: [f64; 2]) -> f64 {
    (p[0] - o[0]) * (q[1] - o[1]) - (p[1] - o[1]) * (q[0] - o[0])
}

/// Strict crossing — collinear or endpoint-touching does not count, which is
/// what makes the diagonal test reject chords that merely graze an edge.
fn segments_properly_intersect(p1: [f64; 2], p2: [f64; 2], q1: [f64; 2], q2: [f64; 2]) -> bool {
    let d1 = cross2(q1, q2, p1);
    let d2 = cross2(q1, q2, p2);
    let d3 = cross2(p1, p2, q1);
    let d4 = cross2(p1, p2, q2);
    ((d1 > 0.0) != (d2 > 0.0)) && ((d3 > 0.0) != (d4 > 0.0))
}

/// Does the chord from `a` to `b` leave `a` into the polygon's interior?
fn in_cone(ring2d: &[[f64; 2]], a: usize, b: usize) -> bool {
    let n = ring2d.len();
    let p = ring2d[a];
    let prev = ring2d[(a + n - 1) % n];
    let next = ring2d[(a + 1) % n];
    let q = ring2d[b];

    if cross2(p, next, prev) >= 0.0 {
        // `a` is a convex corner: the chord must be left of both edges.
        cross2(p, q, prev) > 0.0 && cross2(q, p, next) > 0.0
    } else {
        // `a` is reflex: the chord must not be inside the reflex wedge.
        !(cross2(p, q, next) >= 0.0 && cross2(q, p, prev) >= 0.0)
    }
}

/// The best chord to split `ring` on, and the cosine of the angle between
/// the resulting halves' normals.
///
/// Port of upstream `bm_face_split_find`. `O(N^2)` as upstream is, with
/// upstream's own note that "faces normally aren't so large".
fn find_best_split(ring: &[Vec3], ring2d: &[[f64; 2]]) -> Option<((usize, usize), f64)> {
    let n = ring.len();
    let mut err_best = f64::MAX;
    let mut best: Option<((usize, usize), f64)> = None;

    for i_a in 0..n {
        // Upstream starts at `i_a + 2`: `i_a + 1` is an edge, not a chord.
        for i_b in (i_a + 2)..n {
            // Upstream's `BM_loop_is_adjacent` guard; with this index range
            // the only adjacency left is the wrap-around pair.
            if i_a == 0 && i_b == n - 1 {
                continue;
            }
            let (Some(no_a), Some(no_b)) =
                (normal_subset(ring, i_a, i_b), normal_subset(ring, i_b, i_a))
            else {
                continue;
            };

            let err_test = subset_planarity_error(ring, i_a, i_b, no_a)
                + subset_planarity_error(ring, i_b, i_a, no_b);

            if err_test < err_best && is_valid_diagonal(ring2d, i_a, i_b) {
                err_best = err_test;
                best = Some(((i_a, i_b), no_a.dot(no_b)));
            }
        }
    }
    best
}

/// Split every face of `mesh` that is non-planar by more than
/// `angle_limit` radians, recursively, until no face can be improved.
///
/// Triangles are always planar and are passed through. Positions are never
/// moved — only faces are cut — so this is a topology change, not a
/// deformation. Use [`crate::planar_faces`] when you would rather move the
/// vertices than add edges.
///
/// `angle_limit` is in **radians**; [`DEFAULT_ANGLE_LIMIT`] is upstream's 5°.
/// A limit of 0 splits every face that is non-planar at all; a limit of
/// `PI` splits nothing. Infallible.
///
/// # Examples
///
/// ```
/// use outram_blender::{mesh::Mesh, math::Vec3};
/// use outram_blender::connect_nonplanar::{connect_nonplanar, DEFAULT_ANGLE_LIMIT};
///
/// // A badly warped quad: one corner lifted well out of the others' plane.
/// let pts = vec![
///     Vec3::new(0.0, 0.0, 0.0),
///     Vec3::new(1.0, 0.0, 0.0),
///     Vec3::new(1.0, 1.0, 1.0),
///     Vec3::new(0.0, 1.0, 0.0),
/// ];
/// let warped = Mesh::from_polygons(&pts, &[vec![0, 1, 2, 3]]);
/// let split = connect_nonplanar(&warped, DEFAULT_ANGLE_LIMIT);
/// assert_eq!(split.face_count(), 2);
/// ```
pub fn connect_nonplanar(mesh: &Mesh, angle_limit: f64) -> Mesh {
    let positions = mesh.positions();
    let angle_limit_cos = angle_limit.cos();

    let mut out: Vec<Vec<usize>> = Vec::new();
    // Upstream's `BLI_LINKSTACK`: split faces go back on the stack so an
    // n-gon is reduced repeatedly.
    let mut stack: Vec<Vec<usize>> = mesh
        .polygons()
        .into_iter()
        .map(|p| p.into_iter().map(|v| v.0).collect())
        .collect();

    // A face of `k` corners can be split at most `k - 3` times, so the total
    // work is bounded; this guard only catches a logic error, not a legal
    // input.
    let mut budget = 1 + 8 * stack.iter().map(|r| r.len()).sum::<usize>();

    while let Some(ring_idx) = stack.pop() {
        budget = budget.saturating_sub(1);
        if ring_idx.len() <= 3 || budget == 0 {
            out.push(ring_idx);
            continue;
        }

        let ring: Vec<Vec3> = ring_idx.iter().map(|&i| positions[i]).collect();
        let normal = polyfill::newell_normal(&ring);
        let ring2d = polyfill::project_to_plane(&ring, normal);

        match find_best_split(&ring, &ring2d) {
            // Upstream `bm_face_split_by_angle`: split only when the two
            // halves really do disagree by more than the limit.
            Some(((a, b), angle_cos)) if angle_cos < angle_limit_cos => {
                let mut half_a: Vec<usize> = Vec::new();
                let mut i = a;
                loop {
                    half_a.push(ring_idx[i]);
                    if i == b {
                        break;
                    }
                    i = (i + 1) % ring_idx.len();
                }
                let mut half_b: Vec<usize> = Vec::new();
                let mut j = b;
                loop {
                    half_b.push(ring_idx[j]);
                    if j == a {
                        break;
                    }
                    j = (j + 1) % ring_idx.len();
                }
                stack.push(half_a);
                stack.push(half_b);
            }
            _ => out.push(ring_idx),
        }
    }

    Mesh::from_polygons(&positions, &out)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Peak absolute deviation of a face's corners from the best plane
    /// through its centroid — a direct non-planarity readout.
    fn face_warp(mesh: &Mesh, f: usize) -> f64 {
        let p = mesh.positions();
        let vs = mesh.face_vertices(crate::mesh::FaceId(f));
        let pts: Vec<Vec3> = vs.iter().map(|v| p[v.0]).collect();
        let n = polyfill::newell_normal(&pts);
        let len = n.length();
        if len == 0.0 {
            return 0.0;
        }
        let nrm = Vec3::new(n.x / len, n.y / len, n.z / len);
        let c = pts.iter().fold(Vec3::new(0.0, 0.0, 0.0), |a, &b| a.add(b));
        let c = c.scale(1.0 / pts.len() as f64);
        pts.iter()
            .map(|&q| nrm.dot(q.sub(c)).abs())
            .fold(0.0, f64::max)
    }

    fn max_warp(mesh: &Mesh) -> f64 {
        (0..mesh.face_count())
            .map(|f| face_warp(mesh, f))
            .fold(0.0, f64::max)
    }

    fn warped_quad(lift: f64) -> Mesh {
        let pts = vec![
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::new(1.0, 1.0, lift),
            Vec3::new(0.0, 1.0, 0.0),
        ];
        Mesh::from_polygons(&pts, &[vec![0, 1, 2, 3]])
    }

    /// # Methodology
    ///
    /// A unit quad with one corner lifted by 1.0 out of the other three's
    /// plane is maximally warped. Split it at upstream's default 5° limit
    /// and measure the peak corner-to-plane deviation before and after.
    ///
    /// # Results (measured 2026-09-19)
    ///
    /// Before: one quad, peak corner-to-plane deviation **0.204124**.
    /// After: two triangles, peak deviation **0.0** (below 1e-12) —
    /// triangles are planar by construction, which is the whole point of
    /// the operator. Vertex count unchanged at 4, so no geometry was
    /// invented; only an edge was added.
    #[test]
    fn a_warped_quad_is_split_into_planar_triangles() {
        let m = warped_quad(1.0);
        assert_eq!(m.face_count(), 1);
        let before = max_warp(&m);
        assert!(before > 0.1, "fixture should be warped");

        let s = connect_nonplanar(&m, DEFAULT_ANGLE_LIMIT);
        assert_eq!(s.face_count(), 2);
        assert_eq!(s.vertex_count(), 4, "no vertices should be added");
        assert!(
            max_warp(&s) < 1e-12,
            "split faces should be planar, peak warp {}",
            max_warp(&s)
        );
    }

    /// A planar face must be left completely alone.
    #[test]
    fn planar_faces_are_untouched() {
        let flat = warped_quad(0.0);
        let s = connect_nonplanar(&flat, DEFAULT_ANGLE_LIMIT);
        assert_eq!(s.face_count(), 1);
        assert_eq!(s.vertex_count(), 4);
    }

    /// The angle gate must actually gate: a barely-warped face survives a
    /// loose limit and is split by a tight one.
    #[test]
    fn the_angle_limit_decides() {
        let barely = warped_quad(0.01);
        let loose = connect_nonplanar(&barely, 30.0_f64.to_radians());
        assert_eq!(loose.face_count(), 1, "30 deg should leave this whole");
        let tight = connect_nonplanar(&barely, 0.01_f64.to_radians());
        assert_eq!(tight.face_count(), 2, "0.01 deg should split it");
    }

    /// A closed mesh must stay closed and keep its area — splitting adds
    /// edges, never geometry.
    #[test]
    fn splitting_preserves_closure_and_area() {
        let cube = crate::primitives::cube(2.0);
        let area = |m: &Mesh| -> f64 {
            let p = m.positions();
            (0..m.face_count())
                .map(|i| {
                    let vs = m.face_vertices(crate::mesh::FaceId(i));
                    let mut a = 0.0;
                    for k in 1..vs.len().saturating_sub(1) {
                        let (x, y, z) = (p[vs[0].0], p[vs[k].0], p[vs[k + 1].0]);
                        a += 0.5 * y.sub(x).cross(z.sub(x)).length();
                    }
                    a
                })
                .sum()
        };
        let before = area(&cube);
        // A cube's faces are planar, so nothing should change at all.
        let s = connect_nonplanar(&cube, DEFAULT_ANGLE_LIMIT);
        assert_eq!(s.face_count(), 6);
        assert!((area(&s) - before).abs() < 1e-12);
        assert_eq!(s.euler_characteristic(), 2);
    }

    /// A warped hexagon needs more than one cut; the recursion must reach
    /// planarity rather than stopping after the first split.
    #[test]
    fn a_warped_ngon_is_reduced_until_planar() {
        let n = 6;
        let pts: Vec<Vec3> = (0..n)
            .map(|i| {
                let t = std::f64::consts::TAU * (i as f64) / (n as f64);
                // Alternating lift: a corrugated hexagon.
                Vec3::new(t.cos(), t.sin(), if i % 2 == 0 { 0.4 } else { -0.4 })
            })
            .collect();
        let m = Mesh::from_polygons(&pts, &[(0..n).collect::<Vec<usize>>()]);
        assert!(max_warp(&m) > 0.2);

        let s = connect_nonplanar(&m, DEFAULT_ANGLE_LIMIT);
        assert!(s.face_count() > 1, "should have been split");
        assert_eq!(s.vertex_count(), n);
        assert!(
            max_warp(&s) < 1e-9,
            "recursion should reach planarity, peak warp {}",
            max_warp(&s)
        );
    }

    /// The diagonal test must reject a chord that leaves a concave polygon.
    #[test]
    fn invalid_diagonals_are_rejected() {
        // The L-hexagon again. Corner 2 = (1, 0.5), corner 4 = (0.5, 1):
        // that chord passes through the removed quadrant.
        let l: Vec<[f64; 2]> = vec![
            [0.0, 0.0],
            [1.0, 0.0],
            [1.0, 0.5],
            [0.5, 0.5],
            [0.5, 1.0],
            [0.0, 1.0],
        ];
        assert!(!is_valid_diagonal(&l, 2, 4), "2-4 leaves the polygon");
        assert!(is_valid_diagonal(&l, 0, 3), "0-3 is a real diagonal");
        assert!(!is_valid_diagonal(&l, 0, 1), "adjacent corners are an edge");
        assert!(!is_valid_diagonal(&l, 0, 5), "wrap-around pair is an edge");
    }

    /// Non-planarity measure sanity: a flat run scores zero, a zig-zag does
    /// not, and a zig-zag scores worse than a smooth bow of the same
    /// amplitude — the bias the operator relies on.
    #[test]
    fn planarity_error_punishes_zig_zag_more_than_a_smooth_bow() {
        let flat: Vec<Vec3> = (0..6).map(|i| Vec3::new(i as f64, 0.0, 0.0)).collect();
        let n = Vec3::new(0.0, 0.0, 1.0);
        assert!(subset_planarity_error(&flat, 0, 5, n).abs() < 1e-15);

        let amp = 0.3;
        let zig: Vec<Vec3> = (0..6)
            .map(|i| Vec3::new(i as f64, 0.0, if i % 2 == 0 { amp } else { -amp }))
            .collect();
        let bow: Vec<Vec3> = (0..6)
            .map(|i| {
                let t = i as f64 / 5.0;
                Vec3::new(i as f64, 0.0, amp * (std::f64::consts::PI * t).sin())
            })
            .collect();
        let e_zig = subset_planarity_error(&zig, 0, 5, n);
        let e_bow = subset_planarity_error(&bow, 0, 5, n);
        assert!(e_zig > 0.0 && e_bow > 0.0);
        assert!(
            e_zig > e_bow,
            "total variation should punish the zig-zag more: {e_zig} vs {e_bow}"
        );
    }
}
