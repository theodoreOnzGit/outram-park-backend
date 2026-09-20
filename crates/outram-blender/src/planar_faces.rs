// SPDX-License-Identifier: GPL-3.0-only
//
// ─── Ported from Blender ──────────────────────────────────────────────────
//  Upstream project : Blender <https://github.com/blender/blender>
//  Upstream file    : source/blender/bmesh/operators/bmo_planar_faces.cc
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

//! Flatten faces by relaxing their vertices onto each face's own plane — a
//! port of Blender's **Make Planar Faces** (`bmo_planar_faces.cc`).
//!
//! # What this computes, and how it differs from splitting
//!
//! [`crate::connect_nonplanar`] makes a warped face planar by **cutting it**
//! — topology changes, positions do not. This operator does the opposite:
//! it **moves the vertices** until the faces are planar, leaving topology
//! alone. For a solver mesh both are useful and they are not
//! interchangeable: cutting preserves the surface exactly but multiplies
//! face count, while relaxing keeps the face count but perturbs the
//! geometry. Pick by which of the two you can afford to lose.
//!
//! # The algorithm
//!
//! One iteration, per upstream:
//!
//! 1. Each face defines a plane through its (area-weighted) centroid with
//!    its own normal. That centroid and normal are taken **once, from the
//!    input**, and reused — upstream's comment is "keep original face data
//!    (else we 'move' the face)". Recomputing them each sweep lets the face
//!    drift bodily through space instead of just flattening, which is why
//!    the recompute is `#if 0`-ed out upstream. That is reproduced here.
//! 2. Every vertex collects the closest point on each incident face's plane
//!    and averages them — upstream accumulates with a running mean
//!    (`interp_v3_v3v3(va.co, va.co, co, 1 / co_tot)`), which is the same
//!    value as a plain mean and is kept in that form.
//! 3. Each vertex moves a `factor` of the way toward its average target.
//!    Vertices that moved less than `1e-5` are considered settled; when no
//!    vertex moves, the sweep stops early.
//!
//! Triangles are skipped throughout — they are planar already, and pulling
//! their corners about would only distort the mesh.
//!
//! Inputs are positions in the caller's length unit; `factor` is
//! dimensionless in `[0, 1]` and `iterations` is a count. Nothing here
//! carries a physical dimension.
//!
//! # Fidelity to upstream, and the deviations
//!
//! The plane-per-face construction, the frozen centroid/normal, the running
//! mean, the `1e-5` settle threshold and the early-out are transcribed.
//! Differences:
//!
//! 1. **`f64`, not `f32`.** The `1e-5` threshold is kept at that value; it
//!    is a geometric tolerance in model units, not a mantissa bound.
//! 2. **No per-face dirty flag.** Upstream tracks `ELE_FACE_ADJUST` so a
//!    sweep only revisits faces touching a vertex that moved. This port
//!    recomputes every non-triangle face each sweep. Same fixed point, more
//!    work per sweep; the flag is an optimisation, not a semantic. Worth
//!    revisiting if this is ever run on a large mesh.
//! 3. **Returns a new [`Mesh`]** rather than mutating in place, like every
//!    other operator here.

use crate::math::Vec3;
use crate::mesh::Mesh;
use crate::polyfill;

/// Upstream's default relaxation factor for **Make Planar Faces**.
pub const DEFAULT_FACTOR: f64 = 1.0;

/// Upstream's default iteration count.
pub const DEFAULT_ITERATIONS: usize = 1;

/// Upstream's settle threshold: a vertex that would move less than this is
/// left where it is. Compared squared, as upstream does.
const EPS: f64 = 1e-5;

/// Area-weighted median centre of a face — upstream's
/// `BM_face_calc_center_median_weighted`.
///
/// Each corner is weighted by the length of the two edges meeting there,
/// which keeps the centre from being dragged toward a cluster of closely
/// spaced corners the way a plain vertex mean would.
fn center_median_weighted(pts: &[Vec3]) -> Vec3 {
    let n = pts.len();
    if n == 0 {
        return Vec3::new(0.0, 0.0, 0.0);
    }
    let mut total_w = 0.0;
    let mut acc = Vec3::new(0.0, 0.0, 0.0);
    for i in 0..n {
        let prev = pts[(i + n - 1) % n];
        let next = pts[(i + 1) % n];
        let w = prev.sub(pts[i]).length() + next.sub(pts[i]).length();
        acc = acc.add(pts[i].scale(w));
        total_w += w;
    }
    if total_w > 0.0 {
        acc.scale(1.0 / total_w)
    } else {
        // Every corner coincident: fall back to the plain mean.
        pts.iter()
            .fold(Vec3::new(0.0, 0.0, 0.0), |a, &b| a.add(b))
            .scale(1.0 / n as f64)
    }
}

/// Closest point on the plane `(point_on_plane, unit_normal)` to `co` —
/// upstream's `closest_to_plane_normalized_v3`.
fn closest_to_plane(point_on_plane: Vec3, unit_normal: Vec3, co: Vec3) -> Vec3 {
    let d = unit_normal.dot(co.sub(point_on_plane));
    co.sub(unit_normal.scale(d))
}

/// Relax the vertices of `mesh` so its faces become planar.
///
/// `factor` is how far toward the target each vertex moves per sweep
/// (upstream's "factor" slot, [`DEFAULT_FACTOR`] = 1.0 for the full step);
/// `iterations` bounds the number of sweeps ([`DEFAULT_ITERATIONS`] = 1).
/// Topology is untouched — only positions change — so the returned mesh has
/// the same vertex, edge and face counts. Infallible.
///
/// Triangles are left alone. A mesh of only triangles is returned unchanged.
///
/// # Examples
///
/// ```
/// use outram_blender::{mesh::Mesh, math::Vec3};
/// use outram_blender::planar_faces::{planar_faces, DEFAULT_FACTOR};
///
/// let pts = vec![
///     Vec3::new(0.0, 0.0, 0.0),
///     Vec3::new(1.0, 0.0, 0.0),
///     Vec3::new(1.0, 1.0, 0.5),
///     Vec3::new(0.0, 1.0, 0.0),
/// ];
/// let warped = Mesh::from_polygons(&pts, &[vec![0, 1, 2, 3]]);
/// let flat = planar_faces(&warped, DEFAULT_FACTOR, 20);
/// assert_eq!(flat.face_count(), warped.face_count());
/// assert_eq!(flat.vertex_count(), warped.vertex_count());
/// ```
pub fn planar_faces(mesh: &Mesh, factor: f64, iterations: usize) -> Mesh {
    let mut positions = mesh.positions();
    let polys: Vec<Vec<usize>> = mesh
        .polygons()
        .into_iter()
        .map(|p| p.into_iter().map(|v| v.0).collect())
        .collect();

    // Upstream freezes the plane per face from the INPUT and reuses it — the
    // `#if 0` around the recompute. Without this the faces translate rather
    // than flatten.
    let planes: Vec<Option<(Vec3, Vec3)>> = (0..mesh.face_count())
        .map(|i| {
            let ring: Vec<Vec3> = polys[i].iter().map(|&v| positions[v]).collect();
            if ring.len() == 3 {
                return None;
            }
            let no = polyfill::newell_normal(&ring);
            let len = no.length();
            if len == 0.0 {
                return None;
            }
            let unit = Vec3::new(no.x / len, no.y / len, no.z / len);
            Some((center_median_weighted(&ring), unit))
        })
        .collect();

    let eps_sq = EPS * EPS;

    for _ in 0..iterations {
        // Running mean per vertex, as upstream's `VertAccum`.
        let mut acc = vec![Vec3::new(0.0, 0.0, 0.0); positions.len()];
        let mut acc_tot = vec![0usize; positions.len()];

        for (fi, ring) in polys.iter().enumerate() {
            let Some((center, no)) = planes[fi] else {
                continue;
            };
            for &v in ring {
                let co = closest_to_plane(center, no, positions[v]);
                acc_tot[v] += 1;
                // Upstream: `interp_v3_v3v3(va.co, va.co, co, 1 / co_tot)`.
                let t = 1.0 / acc_tot[v] as f64;
                acc[v] = acc[v].add(co.sub(acc[v]).scale(t));
            }
        }

        let mut changed = false;
        for v in 0..positions.len() {
            if acc_tot[v] == 0 {
                continue;
            }
            let target = acc[v];
            let d = target.sub(positions[v]);
            if d.dot(d) > eps_sq {
                positions[v] = positions[v].add(d.scale(factor));
                changed = true;
            }
        }

        // Upstream's early-out: nothing moved, so nothing will.
        if !changed {
            break;
        }
    }

    Mesh::from_polygons(&positions, &polys)
}

/// Flatten by calling [`planar_faces`] repeatedly until the mesh stops
/// moving — **this crate's addition, not an upstream operator.**
///
/// # Why this exists
///
/// [`planar_faces`] freezes each face's target plane from its input, which
/// is deliberate upstream (it stops faces drifting bodily through space)
/// but means its own `iterations` argument does not converge on geometry
/// where faces share vertices: measured on a corrugated 3x3 grid, peak warp
/// goes 0.150 -> 0.0666 after one sweep and then *back up* to 0.0674 by 200
/// sweeps. Re-invoking the operator recomputes the planes and does
/// converge. See the `iterations_within_one_call_do_not_converge_but_repeated_calls_do`
/// test for the full table.
///
/// So this is a loop around the upstream operator, not a different
/// algorithm. Each pass runs exactly one upstream sweep.
///
/// Stops when the largest vertex movement in a pass falls below
/// `tolerance` (in the caller's length unit), or after `max_passes`.
/// Topology is untouched. Infallible.
///
/// # It cannot flatten below upstream's absolute `eps`
///
/// `planar_faces` skips any vertex whose step is shorter than 1e-5 **model
/// units** (upstream's `eps`), so this loop plateaus there however many
/// passes it is given — measured 7.71e-5 peak warp on the corrugated grid,
/// unchanged from 200 passes to 5000. That floor is absolute, not relative,
/// so it scales with your model's units. Where exact planarity is required,
/// use [`crate::connect_nonplanar`], which cuts rather than moves.
///
/// # Examples
///
/// ```
/// use outram_blender::{mesh::Mesh, math::Vec3};
/// use outram_blender::planar_faces::{planar_faces_converge, DEFAULT_FACTOR};
///
/// let pts = vec![
///     Vec3::new(0.0, 0.0, 0.0),
///     Vec3::new(1.0, 0.0, 0.0),
///     Vec3::new(1.0, 1.0, 0.5),
///     Vec3::new(0.0, 1.0, 0.0),
/// ];
/// let warped = Mesh::from_polygons(&pts, &[vec![0, 1, 2, 3]]);
/// let flat = planar_faces_converge(&warped, DEFAULT_FACTOR, 1e-9, 100);
/// assert_eq!(flat.face_count(), 1);
/// ```
pub fn planar_faces_converge(mesh: &Mesh, factor: f64, tolerance: f64, max_passes: usize) -> Mesh {
    let mut cur = mesh.clone();
    for _ in 0..max_passes {
        let next = planar_faces(&cur, factor, 1);
        let moved = cur
            .positions()
            .iter()
            .zip(next.positions().iter())
            .map(|(a, b)| a.sub(*b).length())
            .fold(0.0, f64::max);
        cur = next;
        if moved < tolerance {
            break;
        }
    }
    cur
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mesh::FaceId;

    /// Peak absolute corner-to-plane deviation over all faces.
    fn max_warp(mesh: &Mesh) -> f64 {
        let p = mesh.positions();
        (0..mesh.face_count())
            .map(|f| {
                let vs = mesh.face_vertices(FaceId(f));
                let pts: Vec<Vec3> = vs.iter().map(|v| p[v.0]).collect();
                let n = polyfill::newell_normal(&pts);
                let len = n.length();
                if len == 0.0 {
                    return 0.0;
                }
                let nrm = Vec3::new(n.x / len, n.y / len, n.z / len);
                let c = center_median_weighted(&pts);
                pts.iter()
                    .map(|&q| nrm.dot(q.sub(c)).abs())
                    .fold(0.0, f64::max)
            })
            .fold(0.0, f64::max)
    }

    /// A 3x3 grid of quads with alternating interior lift — every quad
    /// warped, every interior vertex shared by four of them.
    fn corrugated_grid() -> Mesh {
        let n = 4; // 4x4 vertices -> 3x3 quads
        let mut pts = Vec::new();
        for j in 0..n {
            for i in 0..n {
                let interior = i > 0 && i < n - 1 && j > 0 && j < n - 1;
                let z = if interior {
                    if (i + j) % 2 == 0 {
                        0.15
                    } else {
                        -0.15
                    }
                } else {
                    0.0
                };
                pts.push(Vec3::new(i as f64, j as f64, z));
            }
        }
        let mut faces = Vec::new();
        for j in 0..n - 1 {
            for i in 0..n - 1 {
                faces.push(vec![
                    j * n + i,
                    j * n + i + 1,
                    (j + 1) * n + i + 1,
                    (j + 1) * n + i,
                ]);
            }
        }
        Mesh::from_polygons(&pts, &faces)
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
    /// Take the same warped quad `connect_nonplanar` is tested on (one
    /// corner lifted 0.5) and relax it with the default factor for 20
    /// sweeps. Measure peak corner-to-plane deviation before and after, and
    /// check nothing topological changed.
    ///
    /// # Results (measured 2026-09-19)
    ///
    /// Peak deviation drops below 1e-12 in a single effective sweep — a
    /// lone face's vertices all project onto one plane, so one step lands
    /// exactly and the early-out fires. Face count 1 and vertex count 4
    /// both unchanged. The interesting case is the shared-vertex one
    /// below, where one step is emphatically not enough.
    #[test]
    fn a_lone_warped_quad_flattens() {
        let m = warped_quad(0.5);
        let before = max_warp(&m);
        assert!(before > 0.05, "fixture warp {before}");

        let f = planar_faces(&m, DEFAULT_FACTOR, 20);
        assert_eq!(f.face_count(), 1);
        assert_eq!(f.vertex_count(), 4);
        let after = max_warp(&f);
        assert!(after < 1e-12, "should be flat, got {after}");
    }

    /// Topology must be exactly preserved — this operator moves vertices
    /// and nothing else.
    #[test]
    fn topology_is_never_changed() {
        let cube = crate::primitives::cube(2.0);
        let f = planar_faces(&cube, DEFAULT_FACTOR, 5);
        assert_eq!(f.vertex_count(), cube.vertex_count());
        assert_eq!(f.edge_count(), cube.edge_count());
        assert_eq!(f.face_count(), cube.face_count());
        assert_eq!(f.euler_characteristic(), cube.euler_characteristic());
    }

    /// An already-planar mesh must not be disturbed at all — the early-out
    /// should fire on the first sweep.
    #[test]
    fn a_planar_mesh_is_left_alone() {
        let cube = crate::primitives::cube(2.0);
        let f = planar_faces(&cube, DEFAULT_FACTOR, 10);
        for (a, b) in cube.positions().iter().zip(f.positions().iter()) {
            assert!(a.sub(*b).length() < 1e-12, "{a:?} moved to {b:?}");
        }
    }

    /// Triangles are skipped, so a triangle-only mesh is untouched even
    /// when its faces meet at sharp angles.
    #[test]
    fn triangle_meshes_are_untouched() {
        let tri = crate::triangulate::triangulate(&crate::primitives::cube(2.0));
        let f = planar_faces(&tri, DEFAULT_FACTOR, 10);
        for (a, b) in tri.positions().iter().zip(f.positions().iter()) {
            assert!(a.sub(*b).length() < 1e-12);
        }
    }

    /// The real case: warped faces sharing vertices — and the finding that
    /// upstream's `iterations` slot does not do what its name suggests.
    ///
    /// # Methodology
    ///
    /// A 3x3 grid of quads with every interior vertex pushed alternately up
    /// and down: a corrugated sheet where each quad is warped and each
    /// interior vertex is shared by four of them. Measure peak warp after
    /// 0, 1, 2, 3, 5, 10, 50 and 200 sweeps of a single [`planar_faces`]
    /// call, then separately after 1..7 *repeated calls* of one sweep each.
    ///
    /// # Results (measured 2026-09-19)
    ///
    /// Within one call, extra sweeps do **not** converge:
    ///
    /// | sweeps | peak warp |
    /// |---|---|
    /// | 0 | 0.150000 |
    /// | 1 | 0.066554 |
    /// | 2 | 0.065479 |
    /// | 3 | 0.065483 |
    /// | 10 | 0.065609 |
    /// | 50 | 0.066213 |
    /// | 200 | 0.067366 |
    ///
    /// It plateaus immediately and then **slowly gets worse**. Repeated
    /// *calls*, which recompute the frozen planes each time, do converge:
    ///
    /// | calls | peak warp |
    /// |---|---|
    /// | 1 | 0.066554 |
    /// | 2 | 0.052328 |
    /// | 3 | 0.042041 |
    /// | 5 | 0.028037 |
    /// | 7 | 0.019595 |
    ///
    /// # Why, and is this a port defect?
    ///
    /// Not a port defect — it is upstream's algorithm. Each face's target
    /// plane is computed **once, from the input**, and reused for every
    /// sweep; upstream's `BM_face_normal_update` + centroid recompute are
    /// `#if 0`-ed out with the comment "keep original face data (else we
    /// 'move' the face)". Freezing stops the face drifting bodily through
    /// space, but it also means that after the first sweep the vertices are
    /// being pulled toward planes that are no longer the faces' planes, so
    /// the iteration chases a stale target.
    ///
    /// The practical consequence for a caller: **`iterations` is not a
    /// convergence knob on shared geometry.** To actually flatten a mesh,
    /// call the operator repeatedly — or use
    /// [`planar_faces_converge`], which does that and is this crate's own
    /// addition, not upstream's.
    #[test]
    fn iterations_within_one_call_do_not_converge_but_repeated_calls_do() {
        let m = corrugated_grid();
        let w0 = max_warp(&m);
        assert!(w0 > 0.05, "fixture should be warped, got {w0}");

        let w1 = max_warp(&planar_faces(&m, DEFAULT_FACTOR, 1));
        let w50 = max_warp(&planar_faces(&m, DEFAULT_FACTOR, 50));
        let w200 = max_warp(&planar_faces(&m, DEFAULT_FACTOR, 200));

        assert!(w1 < w0 * 0.6, "one sweep should help a lot: {w0} -> {w1}");
        // The finding: more sweeps inside one call do not help, and in fact
        // drift back. Asserted so a future change to the freeze is noticed.
        assert!(
            w200 > w50 && w50 > w1 * 0.99,
            "extra sweeps are expected NOT to converge: {w1}, {w50}, {w200}"
        );

        // Repeated calls, which refresh the planes, do converge.
        let mut cur = m.clone();
        let mut prev = w0;
        for k in 1..=7 {
            cur = planar_faces(&cur, DEFAULT_FACTOR, 1);
            let w = max_warp(&cur);
            assert!(w < prev, "call {k} should improve: {prev} -> {w}");
            prev = w;
        }
        assert!(
            prev < 0.5 * w1,
            "seven calls should beat one: {w1} -> {prev}"
        );
    }

    /// [`planar_faces_converge`] gets far below what a single call can —
    /// and hits upstream's absolute settle floor, which is worth knowing.
    ///
    /// # Methodology
    ///
    /// Same corrugated grid. Compare `planar_faces(.., 200)` (one call,
    /// many sweeps) against `planar_faces_converge` at increasing pass
    /// counts, with the pass tolerance set low enough not to be the binding
    /// constraint.
    ///
    /// # Results (measured 2026-09-19)
    ///
    /// | passes | peak warp |
    /// |---|---|
    /// | one call, 200 sweeps | 6.74e-2 |
    /// | 10 | 1.25e-2 |
    /// | 50 | 6.42e-4 |
    /// | 200 | 7.71e-5 |
    /// | 1000 | 7.71e-5 |
    /// | 5000 | 7.71e-5 |
    ///
    /// Three orders of magnitude better than a single call, then a hard
    /// plateau at 7.71e-5 that more passes cannot beat.
    ///
    /// # The plateau is upstream's `eps`, and it is absolute
    ///
    /// `bmo_planar_faces.cc` sets `eps = 0.00001f` and skips any vertex
    /// whose step is shorter than that. Once every step is below 1e-5
    /// nothing moves and the result freezes — a peak warp of ~7.7e-5 is
    /// exactly what that floor produces here.
    ///
    /// **That threshold is in model units, not relative.** A mesh authored
    /// in metres therefore cannot be flattened below ~1e-5 m by this
    /// operator, while the same shape authored in millimetres flattens a
    /// thousand times finer. A caller who needs tighter planarity than the
    /// floor should scale up, flatten, and scale back — or use
    /// [`crate::connect_nonplanar`], which achieves exact planarity by
    /// cutting instead of moving.
    #[test]
    fn converge_beats_a_single_call_then_hits_the_absolute_eps_floor() {
        let m = corrugated_grid();
        let one_call = max_warp(&planar_faces(&m, DEFAULT_FACTOR, 200));
        let p50 = max_warp(&planar_faces_converge(&m, DEFAULT_FACTOR, 1e-15, 50));
        let p200 = max_warp(&planar_faces_converge(&m, DEFAULT_FACTOR, 1e-15, 200));
        let p5000 = max_warp(&planar_faces_converge(&m, DEFAULT_FACTOR, 1e-15, 5000));

        assert!(
            p200 < one_call / 100.0,
            "converge should be orders better than one call: {one_call} vs {p200}"
        );
        assert!(p50 > p200, "should still be improving at 50 passes");
        assert!(
            (p5000 - p200).abs() < 1e-12,
            "should have hit the eps floor by 200 passes: {p200} vs {p5000}"
        );
        // The floor itself, tied to upstream's eps = 1e-5.
        assert!(
            p5000 > 1e-6 && p5000 < 1e-3,
            "the plateau should sit near upstream's absolute 1e-5 eps, got {p5000}"
        );

        // Scaling the mesh up by 1000 should push the floor down by the same
        // factor, since eps is absolute — the direct demonstration.
        let big_pts: Vec<Vec3> = m.positions().iter().map(|p| p.scale(1000.0)).collect();
        let big = Mesh::from_polygons(
            &big_pts,
            &m.polygons()
                .into_iter()
                .map(|r| r.into_iter().map(|v| v.0).collect::<Vec<usize>>())
                .collect::<Vec<_>>(),
        );
        let big_warp = max_warp(&planar_faces_converge(&big, DEFAULT_FACTOR, 1e-15, 5000));
        // Compare like with like: scale the result back down.
        let relative = big_warp / 1000.0;
        assert!(
            relative < p5000 / 10.0,
            "a 1000x larger mesh should flatten relatively finer (eps is \
             absolute): {relative} vs {p5000}"
        );

        // Topology still untouched.
        let out = planar_faces_converge(&m, DEFAULT_FACTOR, 1e-9, 200);
        assert_eq!(out.vertex_count(), m.vertex_count());
        assert_eq!(out.face_count(), m.face_count());
    }

    /// A factor of zero must be a no-op, and the `changed` early-out must
    /// not mistake it for convergence in a way that hides a bug.
    #[test]
    fn zero_factor_moves_nothing() {
        let m = warped_quad(0.5);
        let f = planar_faces(&m, 0.0, 10);
        for (a, b) in m.positions().iter().zip(f.positions().iter()) {
            assert!(a.sub(*b).length() < 1e-15);
        }
    }

    /// The weighted centre must differ from the plain mean when corners are
    /// unevenly spaced — that is the whole reason upstream uses it.
    #[test]
    fn weighted_centre_is_not_the_plain_mean() {
        // Three corners bunched at one end of a long quad.
        let pts = [
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(0.1, 0.0, 0.0),
            Vec3::new(0.2, 0.1, 0.0),
            Vec3::new(10.0, 0.0, 0.0),
        ];
        let weighted = center_median_weighted(&pts);
        let plain = pts
            .iter()
            .fold(Vec3::new(0.0, 0.0, 0.0), |a, &b| a.add(b))
            .scale(0.25);
        assert!(
            weighted.sub(plain).length() > 0.5,
            "weighted {weighted:?} vs plain {plain:?}"
        );
    }
}
