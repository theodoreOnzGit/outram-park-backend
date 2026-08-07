// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Surface-arrangement boolean with generalized-winding-number patch
// classification — see boolean_classify.rs for the winding-number references and
// boolean_predicates.rs for the exact orientation tests it relies on.
// Written from the published formulations; no upstream source was copied.
// Blender analogue (architecture only): the mesh_boolean.cc / mesh_intersect.cc
// arrangement pipeline.
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

//! General mesh boolean — Union / Difference / Intersect on arbitrary closed
//! meshes, by **surface arrangement + generalized-winding-number
//! classification**.
//!
//! This is the "robust route" the restricted convex clipper in
//! [`crate::boolean`] deferred to: it lifts the boolean out of the
//! convex-`Intersect`-only case and handles **non-convex** (and self-contained
//! genus-`g`) operands in **all three modes**. It is the pipeline the boolean
//! *foundation* modules were built for — [`crate::boolean_predicates`] (robust
//! Shewchuk orientation) supplies the exact-sign tests that make the
//! retriangulation stable, and [`crate::boolean_classify`] (generalized winding
//! number) supplies the inside/outside decision.
//!
//! ## Provenance / reference
//!
//! Architecture reference: Blender `mesh_boolean.cc` / `mesh_intersect.cc`
//! (github.com/blender/blender @ 96294be75080bbf687fa7f108e344a1063713586,
//! GPL-2.0-or-later; GPLv3-compatible per the workspace provenance rule). This
//! module does **not** transcribe Blender's code — Blender builds an exact
//! (rational-arithmetic) arrangement of the two surfaces, tracks a per-shape
//! *winding number* on each arrangement cell, and keeps a face when its two
//! adjacent cells disagree on membership in the output volume (see Blender's
//! `apply_bool_op` / `propagate_windings_and_in_output_volume`). We implement
//! the same *idea* with a self-contained, C-dependency-free pipeline suitable
//! for this crate (Android-buildable, no GMP): cut each operand's triangles
//! along the other surface, then classify each resulting patch by the winding
//! number of the *other* operand and keep/flip/drop it per the boolean op.
//!
//! The keep/flip/drop rules below are the two-operand specialization of
//! Blender's winding test (`winding[0]`/`winding[1]` = inside operand A / B):
//!
//! | Op | A-patch kept when | B-patch kept when | B-patch winding |
//! |---|---|---|---|
//! | Union      | outside B | outside A | preserved |
//! | Intersect  | inside B  | inside A  | preserved |
//! | Difference | outside B | inside A  | **flipped** |
//!
//! (Union = "in either" ⇒ each operand's surface survives only where it is not
//! already buried inside the other; Intersect = "in both"; Difference `A \ B` =
//! "in A and not in B" ⇒ A's surface survives outside B, and B's surface,
//! flipped to face outward from the removed cavity, survives inside A.)
//!
//! ## The pipeline (per call)
//!
//! 1. **Triangulate** both operands (fan triangulation of each face).
//! 2. **Intersect** every triangle of A against every triangle of B; each
//!    intersecting pair contributes one **segment** lying on both triangles.
//!    The *same* segment (identical `f64` endpoints) is handed to A's triangle
//!    *and* B's triangle, so the two operands' cut curves are numerically
//!    identical — the output welds watertight along the seam.
//! 3. **Retriangulate** each cut triangle, constrained to its segments, via a
//!    2D **constrained Delaunay triangulation** (`triangulate_constrained`,
//!    Bowyer–Watson + Anglada edge-insertion using the robust
//!    [`crate::boolean_predicates::orient2d`] / `incircle`). Uncut triangles
//!    pass through whole.
//! 4. **Classify** each resulting sub-triangle by the winding number of the
//!    *other* operand at its centroid, and **select** (keep / keep-flipped /
//!    drop) per the table above.
//! 5. **Weld** the kept triangles into one closed [`Mesh`].
//!
//! ## Why the constrained triangulation is well-conditioned here
//!
//! The intersection of two **closed** surfaces is a set of closed 1-manifold
//! loops, so *within a single triangle* the constraint segments never cross
//! transversally — they meet only end-to-end where the curve passes between
//! adjacent triangles. That non-crossing property is what makes the
//! Anglada flip-insertion terminate cleanly (no need for Steiner points at
//! interior crossings).
//!
//! ## Limitations (read before trusting on new geometry — honest, not fake-green)
//!
//! - **Generic position assumed.** Two operands that share a **coplanar
//!   overlapping face** are rejected with [`BooleanError::Unsupported`]
//!   ("coplanar faces") rather than guessed at — a coplanar overlap has no
//!   transverse intersection curve to cut along, so the winding classifier
//!   cannot resolve which coplanar patch wins. (Coplanarity *within* one
//!   operand is fine; only A-face-coplanar-with-B-face triggers this.)
//! - **Exactly-degenerate shared geometry** (a vertex of A exactly on a face of
//!   B, an edge of A exactly along an edge of B) can make a triangle pair's
//!   crossing ill-defined; such a pair is skipped (its cut is dropped) rather
//!   than forced. Nudging one operand by an irrational epsilon avoids it. This
//!   is the same class of hard case [`crate::boolean_classify`] documents for
//!   on-surface points.
//! - **Output is triangulated**, not merged back into co-planar n-gons. The
//!   Euler characteristic is still correct (triangulation-invariant), and
//!   downstream solvers triangulate anyway, but the face count is higher than a
//!   minimal polygonal result.
//! - **Requires closed, manifold, outward-oriented operands** — inherited from
//!   [`crate::boolean_classify::winding_number`]. Everything
//!   [`crate::primitives`] generates satisfies this; imported/scanned meshes
//!   must be repaired first.
//!
//! > **Untrusted AI-generated draft** until a human reviews it, per the
//! > workspace `RESPONSIBLE_USE.md`. Not for nuclear facility operation,
//! > reactor control, safety-critical, or licensing decisions.

use crate::boolean::BooleanError;
use crate::boolean_classify::winding_number;
use crate::boolean_predicates::{incircle, orient2d, Vec2};
use crate::math::Vec3;
use crate::mesh::{FaceId, Mesh};
use crate::ops::BooleanMode;

/// A triangle as three model-space corners (winding = outward normal side).
type Tri = [Vec3; 3];

/// A cut segment as its two model-space endpoints.
type Seg = [Vec3; 2];

/// Compute the boolean `A op B` of two **general closed** meshes by surface
/// arrangement + winding classification.
///
/// Handles [`BooleanMode::Union`], [`BooleanMode::Difference`], and
/// [`BooleanMode::Intersect`] on non-convex (but closed, manifold,
/// outward-oriented) operands — see the module docs for the full contract, the
/// keep/flip/drop rules, and the limitations (coplanar overlap, exact
/// degeneracy). Returns [`BooleanError::Unsupported`] for a coplanar-overlap
/// input or a result that welds to fewer than four faces (empty / degenerate).
///
/// Both meshes are dimensionless model-space geometry; `mode` selects the CSG
/// operation. Operands are taken by shared reference and are not modified.
pub fn boolean_general(a: &Mesh, b: &Mesh, mode: BooleanMode) -> Result<Mesh, BooleanError> {
    let mut tris_a = triangulate(a);
    let mut tris_b = triangulate(b);
    if tris_a.len() < 4 || tris_b.len() < 4 {
        return Err(BooleanError::Unsupported(
            "operand does not triangulate to a closed volume (need >= 4 triangles)",
        ));
    }

    // Canonicalize each operand to **outward** winding (positive enclosed
    // volume). The winding classifier ([`winding_number`]) and the keep/flip
    // rules assume outward-oriented operands; a consistently-but-inward-wound
    // input (which some generators produce) would otherwise emit an inside-out
    // result. This is a no-op for already-outward meshes.
    orient_outward(&mut tris_a);
    orient_outward(&mut tris_b);

    let scale = tri_scale(&tris_a).max(tri_scale(&tris_b)).max(1.0);
    let weld = 1e-7 * scale;
    let plane_eps = 1e-9 * scale;

    // Per-triangle constraint segments, filled from every intersecting pair so
    // A's and B's cut curves use identical endpoint coordinates (watertight).
    let mut cons_a: Vec<Vec<Seg>> = vec![Vec::new(); tris_a.len()];
    let mut cons_b: Vec<Vec<Seg>> = vec![Vec::new(); tris_b.len()];

    for (i, &ta) in tris_a.iter().enumerate() {
        for (j, &tb) in tris_b.iter().enumerate() {
            match tri_tri_segment(ta, tb, plane_eps, weld) {
                TriTri::Segment(seg) => {
                    cons_a[i].push(seg);
                    cons_b[j].push(seg);
                }
                TriTri::CoplanarOverlap => {
                    return Err(BooleanError::Unsupported(
                        "coplanar overlapping faces between operands not supported",
                    ));
                }
                TriTri::None => {}
            }
        }
    }

    let mut out: Vec<Tri> = Vec::new();
    emit_operand(&tris_a, &cons_a, b, mode, Operand::A, weld, &mut out);
    emit_operand(&tris_b, &cons_b, a, mode, Operand::B, weld, &mut out);

    build_mesh(&out, weld)
}

/// Which operand a surface patch came from (selects the keep/flip/drop rule).
#[derive(Clone, Copy)]
enum Operand {
    A,
    B,
}

/// Result of one keep/flip/drop decision for a classified sub-triangle.
#[derive(Clone, Copy, PartialEq)]
enum Keep {
    Keep,
    Flip,
    Drop,
}

/// Retriangulate every triangle of one operand along its constraints, classify
/// each sub-triangle against `other`, and push the kept (possibly flipped)
/// triangles into `out`.
fn emit_operand(
    tris: &[Tri],
    cons: &[Vec<Seg>],
    other: &Mesh,
    mode: BooleanMode,
    operand: Operand,
    weld: f64,
    out: &mut Vec<Tri>,
) {
    for (i, &t) in tris.iter().enumerate() {
        let n = tri_normal(t);
        if n == Vec3::ZERO {
            continue; // degenerate source triangle — nothing to emit
        }
        let subs = if cons[i].is_empty() {
            vec![t]
        } else {
            triangulate_constrained(t, n, &cons[i], weld)
        };
        for sub in subs {
            let centroid = tri_centroid(sub);
            // Classify by the *other* operand's winding number at the centroid.
            // We use the winding magnitude directly (not `classify_point`'s
            // OnBoundary gate): after cutting along the seam, a sub-triangle's
            // interior is on one definite side, and a thin sub-triangle's
            // centroid could sit inside the OnBoundary epsilon while still
            // belonging cleanly to one side.
            let inside_other = winding_number(other, centroid).abs() > 0.5;
            match select(mode, operand, inside_other) {
                Keep::Keep => out.push(sub),
                Keep::Flip => out.push([sub[0], sub[2], sub[1]]),
                Keep::Drop => {}
            }
        }
    }
}

/// The two-operand winding keep/flip/drop rule (see the module-doc table).
fn select(mode: BooleanMode, operand: Operand, inside_other: bool) -> Keep {
    match (mode, operand) {
        // Union: a surface survives only where it is NOT buried in the other.
        (BooleanMode::Union, _) => {
            if inside_other {
                Keep::Drop
            } else {
                Keep::Keep
            }
        }
        // Intersect: a surface survives only where it IS inside the other.
        (BooleanMode::Intersect, _) => {
            if inside_other {
                Keep::Keep
            } else {
                Keep::Drop
            }
        }
        // Difference A \ B: keep A's surface outside B ...
        (BooleanMode::Difference, Operand::A) => {
            if inside_other {
                Keep::Drop
            } else {
                Keep::Keep
            }
        }
        // ... and B's surface inside A, flipped to bound the cavity outward.
        (BooleanMode::Difference, Operand::B) => {
            if inside_other {
                Keep::Flip
            } else {
                Keep::Drop
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Triangulation of the operands + small geometry helpers.
// ---------------------------------------------------------------------------

/// Fan-triangulate every face of `m` into a flat list of triangles (`(v0, v_i,
/// v_{i+1})`). Correct for the convex faces every [`crate::primitives`]
/// generator and boolean output produces; a non-convex n-gon face would need a
/// general triangulator (same limitation as [`crate::boolean_classify`]).
fn triangulate(m: &Mesh) -> Vec<Tri> {
    let ps = m.positions();
    let mut tris = Vec::new();
    for f in 0..m.face_count() {
        let vs = m.face_vertices(FaceId(f));
        if vs.len() < 3 {
            continue;
        }
        let v0 = ps[vs[0].0];
        for k in 1..vs.len() - 1 {
            tris.push([v0, ps[vs[k].0], ps[vs[k + 1].0]]);
        }
    }
    tris
}

/// Flip every triangle's winding if the list's net signed volume is negative,
/// so the operand ends up **outward**-oriented (positive enclosed volume).
///
/// Signed volume is `(1/6) sum v0 . (v1 x v2)` over the triangles (divergence
/// theorem); it is positive for an outward-wound closed surface and negative
/// for an inward-wound one. A consistently-wound closed mesh has a
/// well-defined sign; an inconsistently-wound (non-orientable / broken) mesh
/// is outside this module's contract (see the module-doc limitations).
fn orient_outward(tris: &mut [Tri]) {
    let mut vol6 = 0.0;
    for t in tris.iter() {
        vol6 += t[0].dot(t[1].cross(t[2]));
    }
    if vol6 < 0.0 {
        for t in tris.iter_mut() {
            t.swap(1, 2);
        }
    }
}

/// Bounding-box diagonal of a triangle list — the size scale for tolerances.
fn tri_scale(tris: &[Tri]) -> f64 {
    let mut lo = tris[0][0];
    let mut hi = tris[0][0];
    for t in tris {
        for &p in t {
            lo = Vec3::new(lo.x.min(p.x), lo.y.min(p.y), lo.z.min(p.z));
            hi = Vec3::new(hi.x.max(p.x), hi.y.max(p.y), hi.z.max(p.z));
        }
    }
    hi.sub(lo).length()
}

/// Unnormalized-then-normalized outward normal of a triangle (`(b-a) x (c-a)`),
/// or [`Vec3::ZERO`] for a degenerate (zero-area) triangle.
fn tri_normal(t: Tri) -> Vec3 {
    t[1].sub(t[0]).cross(t[2].sub(t[0])).normalize()
}

/// Centroid (vertex average) of a triangle.
fn tri_centroid(t: Tri) -> Vec3 {
    t[0].add(t[1]).add(t[2]).scale(1.0 / 3.0)
}

// ---------------------------------------------------------------------------
// Triangle–triangle intersection: the exact segment two triangles share.
// ---------------------------------------------------------------------------

/// Outcome of intersecting two triangles.
enum TriTri {
    /// They cross transversally in a single segment (its two endpoints).
    Segment(Seg),
    /// They are coplanar and their in-plane footprints overlap (unsupported —
    /// no transverse curve to cut along).
    CoplanarOverlap,
    /// No intersection (disjoint, or coplanar but non-overlapping, or touching
    /// only at a point).
    None,
}

/// The segment where triangles `ta` and `tb` intersect, or a coplanar/none
/// verdict.
///
/// Method: each triangle's boundary crosses the *other* triangle's plane in a
/// sub-segment of the two planes' intersection line; the triangle–triangle
/// intersection is the overlap of those two sub-segments along that line
/// (Möller, *A Fast Triangle–Triangle Intersection Test*). Crossing points are
/// taken as **actual edge/plane crossings** (not re-interpolated) so the same
/// pair always yields identical coordinates for both operands.
fn tri_tri_segment(ta: Tri, tb: Tri, plane_eps: f64, weld: f64) -> TriTri {
    let na = tri_normal(ta);
    let nb = tri_normal(tb);
    if na == Vec3::ZERO || nb == Vec3::ZERO {
        return TriTri::None; // degenerate triangle
    }

    // Signed distances of each triangle's verts to the other's plane.
    let sd_a = [
        na_dist(tb, nb, ta[0]),
        na_dist(tb, nb, ta[1]),
        na_dist(tb, nb, ta[2]),
    ];
    let sd_b = [
        na_dist(ta, na, tb[0]),
        na_dist(ta, na, tb[1]),
        na_dist(ta, na, tb[2]),
    ];

    let a_all_on = sd_a.iter().all(|d| d.abs() <= plane_eps);
    if a_all_on {
        // ta lies in tb's plane -> coplanar. Overlap only matters if their
        // footprints actually intersect in-plane.
        return if coplanar_overlap(ta, tb, na, weld) {
            TriTri::CoplanarOverlap
        } else {
            TriTri::None
        };
    }

    // Reject if either triangle is entirely on one open side of the other's
    // plane (no crossing).
    if all_positive(&sd_a, plane_eps) || all_negative(&sd_a, plane_eps) {
        return TriTri::None;
    }
    if all_positive(&sd_b, plane_eps) || all_negative(&sd_b, plane_eps) {
        return TriTri::None;
    }

    // Crossing points of each triangle's boundary with the other's plane.
    let (a0, a1) = match plane_crossings(ta, sd_a, plane_eps) {
        Some(pair) => pair,
        None => return TriTri::None, // degenerate crossing (e.g. vertex on plane)
    };
    let (b0, b1) = match plane_crossings(tb, sd_b, plane_eps) {
        Some(pair) => pair,
        None => return TriTri::None,
    };

    // Parameterize all four crossing points along the intersection line
    // direction and take the overlap of the two sub-segments.
    let dir = na.cross(nb);
    let key = |p: Vec3| p.dot(dir);
    let (a_lo, a_hi) = order_by(a0, a1, &key);
    let (b_lo, b_hi) = order_by(b0, b1, &key);

    // Overlap endpoints: larger of the two lows, smaller of the two highs.
    let lo = if key(a_lo) >= key(b_lo) { a_lo } else { b_lo };
    let hi = if key(a_hi) <= key(b_hi) { a_hi } else { b_hi };

    if key(lo) > key(hi) || hi.sub(lo).length() <= weld {
        // Disjoint along the line, or the operands merely touch at a point.
        TriTri::None
    } else {
        TriTri::Segment([lo, hi])
    }
}

/// Signed distance of point `p` to the plane of triangle `t` with unit normal
/// `n`: `n . (p - t[0])`.
fn na_dist(t: Tri, n: Vec3, p: Vec3) -> f64 {
    n.dot(p.sub(t[0]))
}

/// `true` if every signed distance is strictly positive (beyond `eps`).
fn all_positive(sd: &[f64; 3], eps: f64) -> bool {
    sd.iter().all(|&d| d > eps)
}

/// `true` if every signed distance is strictly negative (beyond `eps`).
fn all_negative(sd: &[f64; 3], eps: f64) -> bool {
    sd.iter().all(|&d| d < -eps)
}

/// The (exactly two, for a transverse crossing) points where triangle `t`'s
/// boundary crosses the plane `sd = 0`. Returns `None` if the crossing is
/// degenerate (not exactly two distinct points — e.g. a vertex lies on the
/// plane), which the caller treats as a skipped/no-cut pair.
fn plane_crossings(t: Tri, sd: [f64; 3], eps: f64) -> Option<(Vec3, Vec3)> {
    let mut pts: Vec<Vec3> = Vec::new();
    for &(i, j) in &[(0usize, 1usize), (1, 2), (2, 0)] {
        let di = sd[i];
        let dj = sd[j];
        // Strict opposite sides -> one interior crossing point on this edge.
        if (di > eps && dj < -eps) || (di < -eps && dj > eps) {
            let s = di / (di - dj);
            pts.push(t[i].add(t[j].sub(t[i]).scale(s)));
        }
    }
    if pts.len() == 2 {
        Some((pts[0], pts[1]))
    } else {
        None
    }
}

/// Order two points by a scalar key (ascending); returns `(low, high)`.
fn order_by(p: Vec3, q: Vec3, key: &impl Fn(Vec3) -> f64) -> (Vec3, Vec3) {
    if key(p) <= key(q) {
        (p, q)
    } else {
        (q, p)
    }
}

/// Best-effort test of whether two **coplanar** triangles overlap in their
/// shared plane (positive area of intersection). Projects both to 2D and tests
/// for any edge crossing or vertex containment. Conservative: any real overlap
/// returns `true`; a shared edge/vertex only (measure-zero touch) returns
/// `false`.
fn coplanar_overlap(ta: Tri, tb: Tri, n: Vec3, weld: f64) -> bool {
    let drop = drop_axis(n);
    let a: Vec<Vec2> = ta.iter().map(|&p| project(p, drop)).collect();
    let b: Vec<Vec2> = tb.iter().map(|&p| project(p, drop)).collect();

    // Either triangle's centroid inside the other -> overlap. This is what
    // catches *coincident / identical* faces (all vertices shared and edges
    // collinear, so the vertex-in and edge-cross tests below both miss, but the
    // centroid is strictly interior to both).
    let ca = centroid2(&a);
    let cb = centroid2(&b);
    if point_in_tri(ca, &b, weld) || point_in_tri(cb, &a, weld) {
        return true;
    }
    // Any vertex of one triangle strictly inside the other -> overlap.
    if a.iter().any(|&p| point_in_tri(p, &b, weld)) {
        return true;
    }
    if b.iter().any(|&p| point_in_tri(p, &a, weld)) {
        return true;
    }
    // Any pair of edges crossing transversally -> overlap.
    for i in 0..3 {
        for j in 0..3 {
            if seg_cross(a[i], a[(i + 1) % 3], b[j], b[(j + 1) % 3]) {
                return true;
            }
        }
    }
    false
}

/// Arithmetic mean of three 2D points (a triangle's centroid).
fn centroid2(t: &[Vec2]) -> Vec2 {
    Vec2::new(
        (t[0].x + t[1].x + t[2].x) / 3.0,
        (t[0].y + t[1].y + t[2].y) / 3.0,
    )
}

/// `true` if 2D point `p` is inside (not merely on the boundary of) the
/// triangle `tri` (three CCW-or-CW vertices), using robust [`orient2d`] signs.
fn point_in_tri(p: Vec2, tri: &[Vec2], _weld: f64) -> bool {
    let s0 = orient2d(tri[0], tri[1], p);
    let s1 = orient2d(tri[1], tri[2], p);
    let s2 = orient2d(tri[2], tri[0], p);
    // Strictly inside: all three orientations share one nonzero sign.
    (s0 > 0 && s1 > 0 && s2 > 0) || (s0 < 0 && s1 < 0 && s2 < 0)
}

/// `true` if open segments `(a,b)` and `(c,d)` cross transversally (proper
/// intersection), by the robust four-orientation test.
fn seg_cross(a: Vec2, b: Vec2, c: Vec2, d: Vec2) -> bool {
    let d1 = orient2d(a, b, c);
    let d2 = orient2d(a, b, d);
    let d3 = orient2d(c, d, a);
    let d4 = orient2d(c, d, b);
    ((d1 > 0 && d2 < 0) || (d1 < 0 && d2 > 0)) && ((d3 > 0 && d4 < 0) || (d3 < 0 && d4 > 0))
}

// ---------------------------------------------------------------------------
// Constrained Delaunay retriangulation of one cut triangle.
// ---------------------------------------------------------------------------

/// Retriangulate triangle `t` so that every constraint segment appears as an
/// edge, returning sub-triangles wound to match the source normal `n`.
///
/// Projects to 2D (dropping the dominant normal axis), collects the corners
/// plus all constraint endpoints (welded to unique points), builds a
/// Bowyer–Watson Delaunay triangulation, and forces the constraint edges in by
/// Anglada flip-insertion. Because the constraints never cross transversally
/// (module docs), no Steiner points are needed. Sub-triangles are mapped back
/// to 3D and re-wound so each one's normal aligns with `n`.
fn triangulate_constrained(t: Tri, n: Vec3, constraints: &[Seg], weld: f64) -> Vec<Tri> {
    // 1. Collect unique 3D points: the three corners, then constraint endpoints.
    let mut pts3: Vec<Vec3> = t.to_vec();
    let index_of = |pts: &mut Vec<Vec3>, p: Vec3| -> usize {
        for (k, &q) in pts.iter().enumerate() {
            if q.sub(p).length() <= weld {
                return k;
            }
        }
        pts.push(p);
        pts.len() - 1
    };
    let mut edges: Vec<(usize, usize)> = Vec::new();
    for seg in constraints {
        let u = index_of(&mut pts3, seg[0]);
        let v = index_of(&mut pts3, seg[1]);
        if u != v {
            edges.push((u, v));
        }
    }

    // No genuinely new points -> the source triangle is already fine.
    if pts3.len() == 3 {
        return vec![t];
    }

    // 2. Project to 2D and Delaunay-triangulate.
    let drop = drop_axis(n);
    let pts2: Vec<Vec2> = pts3.iter().map(|&p| project(p, drop)).collect();
    let mut tris = bowyer_watson(&pts2);

    // 3. Force each constraint edge to be present.
    for &(u, v) in &edges {
        insert_constraint_edge(&mut tris, &pts2, u, v);
    }

    // 4. Map back to 3D, re-wind to match `n`, drop degenerate triangles.
    let mut out = Vec::with_capacity(tris.len());
    for [i, j, k] in tris {
        let mut tri = [pts3[i], pts3[j], pts3[k]];
        let tn = tri_normal(tri);
        if tn == Vec3::ZERO {
            continue;
        }
        if tn.dot(n) < 0.0 {
            tri.swap(1, 2);
        }
        out.push(tri);
    }
    out
}

/// Index of the normal's dominant axis (the coordinate to drop when projecting
/// the triangle to 2D): 0 = x, 1 = y, 2 = z.
fn drop_axis(n: Vec3) -> u8 {
    let (ax, ay, az) = (n.x.abs(), n.y.abs(), n.z.abs());
    if ax >= ay && ax >= az {
        0
    } else if ay >= az {
        1
    } else {
        2
    }
}

/// Project a 3D point to 2D by dropping the dominant axis. The kept-axis order
/// (`(y,z)`, `(z,x)`, `(x,y)`) preserves right-handedness, so a CCW loop about
/// `+axis` stays CCW in 2D (output winding is re-checked against the source
/// normal regardless).
fn project(p: Vec3, drop: u8) -> Vec2 {
    match drop {
        0 => Vec2::new(p.y, p.z),
        1 => Vec2::new(p.z, p.x),
        _ => Vec2::new(p.x, p.y),
    }
}

/// Bowyer–Watson Delaunay triangulation of a 2D point set, returning triangles
/// as index triples into `pts` (super-triangle removed). Each returned triangle
/// is wound counter-clockwise ([`orient2d`] `> 0`). Robust in-circle / orient
/// tests come from [`crate::boolean_predicates`].
fn bowyer_watson(pts: &[Vec2]) -> Vec<[usize; 3]> {
    let n = pts.len();
    // Super-triangle: a large triangle enclosing every point.
    let mut lo = pts[0];
    let mut hi = pts[0];
    for &p in pts {
        lo = Vec2::new(lo.x.min(p.x), lo.y.min(p.y));
        hi = Vec2::new(hi.x.max(p.x), hi.y.max(p.y));
    }
    let dx = (hi.x - lo.x).max(1.0);
    let dy = (hi.y - lo.y).max(1.0);
    let d = dx.max(dy) * 1000.0;
    let cx = (lo.x + hi.x) * 0.5;
    let cy = (lo.y + hi.y) * 0.5;
    let mut work: Vec<Vec2> = pts.to_vec();
    work.push(Vec2::new(cx - d, cy - d)); // n
    work.push(Vec2::new(cx + d, cy - d)); // n+1
    work.push(Vec2::new(cx, cy + d)); // n+2

    let mut tris: Vec<[usize; 3]> = vec![ccw([n, n + 1, n + 2], &work)];

    for i in 0..n {
        // Triangles whose circumcircle contains point i are "bad".
        let mut bad: Vec<usize> = Vec::new();
        for (ti, tri) in tris.iter().enumerate() {
            if incircle(work[tri[0]], work[tri[1]], work[tri[2]], work[i]) > 0 {
                bad.push(ti);
            }
        }
        // Boundary of the cavity: edges of bad triangles used by exactly one.
        let mut boundary: Vec<(usize, usize)> = Vec::new();
        for &ti in &bad {
            let tri = tris[ti];
            for &(a, b) in &[(tri[0], tri[1]), (tri[1], tri[2]), (tri[2], tri[0])] {
                let shared = bad.iter().any(|&oj| oj != ti && tri_has_edge(tris[oj], a, b));
                if !shared {
                    boundary.push((a, b));
                }
            }
        }
        // Remove bad triangles (high indices first) and re-fill the cavity.
        bad.sort_unstable();
        for &ti in bad.iter().rev() {
            tris.swap_remove(ti);
        }
        for (a, b) in boundary {
            tris.push(ccw([a, b, i], &work));
        }
    }

    // Drop any triangle still touching a super-corner.
    tris.retain(|t| t.iter().all(|&v| v < n));
    tris
}

/// Order a triangle's indices counter-clockwise (`orient2d > 0`) in `pts`.
fn ccw(t: [usize; 3], pts: &[Vec2]) -> [usize; 3] {
    if orient2d(pts[t[0]], pts[t[1]], pts[t[2]]) < 0 {
        [t[0], t[2], t[1]]
    } else {
        t
    }
}

/// `true` if triangle `t` uses the undirected edge `(a, b)`.
fn tri_has_edge(t: [usize; 3], a: usize, b: usize) -> bool {
    let has = |x: usize| t[0] == x || t[1] == x || t[2] == x;
    has(a) && has(b)
}

/// Force the undirected edge `(u, v)` into the triangulation `tris` by Anglada
/// flip-insertion: repeatedly flip a triangulation edge that the segment `u–v`
/// crosses, until `u–v` is present. Terminates because the constraint segments
/// never cross transversally (module docs); a bounded guard defends against a
/// pathological/degenerate input rather than looping forever.
fn insert_constraint_edge(tris: &mut Vec<[usize; 3]>, pts: &[Vec2], u: usize, v: usize) {
    if u == v || tris.iter().any(|t| tri_has_edge(*t, u, v)) {
        return;
    }
    let mut guard = 0usize;
    let guard_cap = 16 * tris.len() + 64;
    while guard < guard_cap {
        guard += 1;
        if tris.iter().any(|t| tri_has_edge(*t, u, v)) {
            return;
        }
        // Find two adjacent triangles sharing an edge (a,b) that the segment
        // u–v crosses, whose quad is convex, and flip that edge to its other
        // diagonal.
        let Some((t1, t2, a, b, p, q)) = find_flippable_crossing(tris, pts, u, v) else {
            return; // no progress possible (degenerate) — leave as-is
        };
        // Replace triangles (a,b,p) and (a,b,q) with (a,p,q) and (b,p,q).
        let (hi, loi) = if t1 > t2 { (t1, t2) } else { (t2, t1) };
        tris.swap_remove(hi);
        tris.swap_remove(loi);
        tris.push(ccw([a, p, q], pts));
        tris.push(ccw([b, p, q], pts));
    }
}

/// Locate a flippable triangulation edge crossed by segment `u–v`.
///
/// Returns `(t1, t2, a, b, p, q)`: the indices of the two triangles sharing
/// edge `(a, b)`, plus the apexes `p` (of `t1`) and `q` (of `t2`), when segment
/// `u–v` properly crosses `(a, b)` and the quad `a–p–b–q` is convex (so the
/// flip to diagonal `p–q` produces two valid triangles). `None` if no such edge
/// exists.
fn find_flippable_crossing(
    tris: &[[usize; 3]],
    pts: &[Vec2],
    u: usize,
    v: usize,
) -> Option<(usize, usize, usize, usize, usize, usize)> {
    for t1 in 0..tris.len() {
        for &(a, b) in &[
            (tris[t1][0], tris[t1][1]),
            (tris[t1][1], tris[t1][2]),
            (tris[t1][2], tris[t1][0]),
        ] {
            // The shared edge must not touch the constraint endpoints.
            if a == u || a == v || b == u || b == v {
                continue;
            }
            if !seg_cross(pts[u], pts[v], pts[a], pts[b]) {
                continue;
            }
            let p = apex(tris[t1], a, b)?;
            // Find the other triangle across edge (a,b).
            for t2 in 0..tris.len() {
                if t2 == t1 || !tri_has_edge(tris[t2], a, b) {
                    continue;
                }
                let Some(q) = apex(tris[t2], a, b) else {
                    continue;
                };
                // Convex quad a–p–b–q iff p and q lie on opposite sides of a–b
                // (guaranteed: shared edge) AND a, b lie on opposite sides of
                // p–q. Then the flip is legal.
                let sa = orient2d(pts[p], pts[q], pts[a]);
                let sb = orient2d(pts[p], pts[q], pts[b]);
                if (sa > 0 && sb < 0) || (sa < 0 && sb > 0) {
                    return Some((t1, t2, a, b, p, q));
                }
            }
        }
    }
    None
}

/// The third vertex of triangle `t` other than `a` and `b`, or `None` if `t`
/// does not contain both.
fn apex(t: [usize; 3], a: usize, b: usize) -> Option<usize> {
    t.iter().copied().find(|&x| x != a && x != b)
}

// ---------------------------------------------------------------------------
// Weld the kept triangles into a closed mesh.
// ---------------------------------------------------------------------------

/// Weld a triangle list into a [`Mesh`], merging vertices within `weld`.
/// Degenerate triangles (a repeated welded vertex) are dropped. Returns
/// [`BooleanError::Unsupported`] if the result is not a closed volume (fewer
/// than four faces or vertices).
fn build_mesh(tris: &[Tri], weld: f64) -> Result<Mesh, BooleanError> {
    let mut positions: Vec<Vec3> = Vec::new();
    let mut faces: Vec<Vec<usize>> = Vec::new();
    for t in tris {
        let idx: Vec<usize> = t.iter().map(|&p| find_or_add(&mut positions, p, weld)).collect();
        if idx[0] != idx[1] && idx[1] != idx[2] && idx[2] != idx[0] {
            faces.push(idx);
        }
    }
    if faces.len() < 4 || positions.len() < 4 {
        return Err(BooleanError::Unsupported(
            "boolean result is empty or degenerate (fewer than four faces)",
        ));
    }
    Ok(Mesh::from_polygons(&positions, &faces))
}

/// Find an existing welded vertex within `weld` of `p`, or append it.
fn find_or_add(positions: &mut Vec<Vec3>, p: Vec3, weld: f64) -> usize {
    for (i, &q) in positions.iter().enumerate() {
        if q.sub(p).length() <= weld {
            return i;
        }
    }
    positions.push(p);
    positions.len() - 1
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::primitives;

    /// Axis-aligned bounding box `(min, max)` of a mesh's vertices.
    fn bounds(m: &Mesh) -> (Vec3, Vec3) {
        let ps = m.positions();
        let mut lo = ps[0];
        let mut hi = ps[0];
        for &p in &ps {
            lo = Vec3::new(lo.x.min(p.x), lo.y.min(p.y), lo.z.min(p.z));
            hi = Vec3::new(hi.x.max(p.x), hi.y.max(p.y), hi.z.max(p.z));
        }
        (lo, hi)
    }

    /// Signed volume of a closed triangle-or-polygon mesh via the divergence
    /// theorem: `V = (1/6) sum over triangles v0 . (v1 x v2)` (fan-triangulated
    /// faces, outward winding). A closed, outward-oriented mesh gives a
    /// positive volume equal to the enclosed volume.
    fn signed_volume(m: &Mesh) -> f64 {
        let ps = m.positions();
        let mut vol = 0.0;
        for face in m.polygons() {
            let v0 = ps[face[0].0];
            for k in 1..face.len() - 1 {
                let v1 = ps[face[k].0];
                let v2 = ps[face[k + 1].0];
                vol += v0.dot(v1.cross(v2));
            }
        }
        vol / 6.0
    }

    /// `true` if every undirected edge of the mesh is shared by exactly two
    /// faces — the combinatorial test for a closed 2-manifold (watertight, no
    /// boundary edges, no non-manifold edges shared by 3+ faces).
    fn is_manifold(m: &Mesh) -> bool {
        use std::collections::HashMap;
        let mut edge_use: HashMap<(usize, usize), usize> = HashMap::new();
        for face in m.polygons() {
            let n = face.len();
            for i in 0..n {
                let (a, b) = (face[i].0, face[(i + 1) % n].0);
                let key = if a < b { (a, b) } else { (b, a) };
                *edge_use.entry(key).or_insert(0) += 1;
            }
        }
        edge_use.values().all(|&c| c == 2)
    }

    /// Rebuild a mesh with every vertex mapped through `f` (translate an
    /// operand while keeping its faces/winding).
    fn map_positions(m: &Mesh, f: impl Fn(Vec3) -> Vec3) -> Mesh {
        let ps: Vec<Vec3> = m.positions().iter().map(|&p| f(p)).collect();
        let faces: Vec<Vec<usize>> = m
            .polygons()
            .iter()
            .map(|face| face.iter().map(|v| v.0).collect())
            .collect();
        Mesh::from_polygons(&ps, &faces)
    }

    fn approx(a: f64, b: f64, tol: f64) -> bool {
        (a - b).abs() <= tol
    }

    /// Two axis-aligned boxes offset so they overlap in a smaller box, with
    /// **no coplanar faces and no shared vertices** (face planes `{0,2}` vs
    /// `{1,3}` are disjoint): `A = [0,2]^3`, `B = [1,3]^3`, overlap `[1,2]^3`.
    ///
    /// Analytic volumes: `|A| = |B| = 8`, `|A n B| = 1`, so
    /// `|A u B| = 8 + 8 - 1 = 15`, `|A n B| = 1`, `|A \ B| = 8 - 1 = 7`.
    fn boxes() -> (Mesh, Mesh) {
        let a = map_positions(&primitives::cube(2.0), |p| {
            Vec3::new(p.x + 1.0, p.y + 1.0, p.z + 1.0)
        }); // [0,2]^3
        let b = map_positions(&primitives::cube(2.0), |p| {
            Vec3::new(p.x + 2.0, p.y + 2.0, p.z + 2.0)
        }); // [1,3]^3
        (a, b)
    }

    /// Methodology: intersect `A = [0,2]^3` with `B = [1,3]^3`. Exact result is
    /// the box `[1,2]^3`: closed (`chi = 2`), bounds `[1,2]^3`, volume `1`.
    /// Pass criterion: `chi = 2`, bounds and volume within `1e-9`.
    ///
    /// Results (asserted below): `chi = 2`; bounds `min=(1,1,1)`,
    /// `max=(2,2,2)`; volume `1.0` (analytically exact — integer crossings).
    #[test]
    fn intersect_two_boxes_is_unit_overlap() {
        let (a, b) = boxes();
        let out = boolean_general(&a, &b, BooleanMode::Intersect).expect("intersect ok");
        assert_eq!(out.euler_characteristic(), 2, "closed result (chi=2)");
        let (lo, hi) = bounds(&out);
        let tol = 1e-9;
        assert!(approx(lo.x, 1.0, tol) && approx(lo.y, 1.0, tol) && approx(lo.z, 1.0, tol), "min={lo:?}");
        assert!(approx(hi.x, 2.0, tol) && approx(hi.y, 2.0, tol) && approx(hi.z, 2.0, tol), "max={hi:?}");
        assert!(approx(signed_volume(&out), 1.0, 1e-9), "volume={}", signed_volume(&out));
    }

    /// Methodology: union of `A = [0,2]^3` and `B = [1,3]^3`. Exact volume
    /// `8 + 8 - 1 = 15`; result is a closed genus-0 solid (`chi = 2`) spanning
    /// bounds `[0,3]^3`. Pass criterion: `chi = 2`, volume `15` within `1e-9`,
    /// bounds `[0,3]^3`.
    ///
    /// Results (asserted below): `chi = 2`; volume `15.0`; bounds `min=(0,0,0)`,
    /// `max=(3,3,3)`.
    #[test]
    fn union_two_boxes_has_volume_15() {
        let (a, b) = boxes();
        let out = boolean_general(&a, &b, BooleanMode::Union).expect("union ok");
        assert_eq!(out.euler_characteristic(), 2, "closed result (chi=2)");
        assert!(approx(signed_volume(&out), 15.0, 1e-9), "volume={}", signed_volume(&out));
        let (lo, hi) = bounds(&out);
        let tol = 1e-9;
        assert!(approx(lo.x, 0.0, tol) && approx(lo.y, 0.0, tol) && approx(lo.z, 0.0, tol), "min={lo:?}");
        assert!(approx(hi.x, 3.0, tol) && approx(hi.y, 3.0, tol) && approx(hi.z, 3.0, tol), "max={hi:?}");
    }

    /// Methodology: difference `A \ B` of `A = [0,2]^3` minus `B = [1,3]^3`.
    /// Exact volume `8 - 1 = 7`; result is a closed solid (an L/notched cube)
    /// with `chi = 2`, still spanning `A`'s bounds `[0,2]^3`. Pass criterion:
    /// `chi = 2`, volume `7` within `1e-9`.
    ///
    /// Results (asserted below): `chi = 2`; volume `7.0`; bounds `[0,2]^3`.
    #[test]
    fn difference_two_boxes_has_volume_7() {
        let (a, b) = boxes();
        let out = boolean_general(&a, &b, BooleanMode::Difference).expect("difference ok");
        assert_eq!(out.euler_characteristic(), 2, "closed result (chi=2)");
        assert!(approx(signed_volume(&out), 7.0, 1e-9), "volume={}", signed_volume(&out));
        let (lo, hi) = bounds(&out);
        let tol = 1e-9;
        assert!(approx(lo.x, 0.0, tol) && approx(hi.x, 2.0, tol), "x=({},{})", lo.x, hi.x);
    }

    /// The difference result must be genuinely **concave** — that is the whole
    /// point of the general path over the convex clipper. Methodology: subtract
    /// `B = [1,3]^3` from `A = [0,2]^3`, giving a notched (L-shaped) cube of
    /// volume 7, then use the winding number to confirm the removed notch
    /// centre `(1.5,1.5,1.5)` is now **outside** the result while the kept
    /// corner centre `(0.5,0.5,0.5)` is **inside** — a classification that only
    /// holds if the concavity was carved correctly (a convex hull would report
    /// the notch centre inside).
    ///
    /// Results (asserted below): notch centre `|w| < 0.5` (outside); corner
    /// centre `|w| > 0.5` (inside).
    #[test]
    fn difference_result_is_genuinely_concave() {
        let (a, b) = boxes();
        let out = boolean_general(&a, &b, BooleanMode::Difference).expect("difference ok");
        // The notch corner region [1,2]^3 was removed; its centre (1.5,1.5,1.5)
        // must now be OUTSIDE the result, while (0.5,0.5,0.5) stays inside.
        assert!(
            winding_number(&out, Vec3::new(1.5, 1.5, 1.5)).abs() < 0.5,
            "removed-notch centre must be outside the difference"
        );
        assert!(
            winding_number(&out, Vec3::new(0.5, 0.5, 0.5)).abs() > 0.5,
            "kept-corner centre must be inside the difference"
        );
    }

    /// Coplanar overlapping faces between the two operands are rejected
    /// honestly (no fake-green). Methodology: `A = [0,2]^3` and a copy `B`
    /// shifted by `(2,0,0)` share the coplanar face plane `x = 2` over the full
    /// `y,z in [0,2]` square — a coplanar overlap with no transverse cut curve.
    #[test]
    fn coplanar_shared_face_is_unsupported() {
        let a = map_positions(&primitives::cube(2.0), |p| Vec3::new(p.x + 1.0, p.y + 1.0, p.z + 1.0)); // [0,2]^3
        let b = map_positions(&primitives::cube(2.0), |p| Vec3::new(p.x + 3.0, p.y + 1.0, p.z + 1.0)); // [2,4]x[0,2]x[0,2]
        let r = boolean_general(&a, &b, BooleanMode::Union);
        assert!(
            matches!(r, Err(BooleanError::Unsupported(msg)) if msg.contains("coplanar")),
            "coplanar shared face must be reported, got {r:?}"
        );
    }

    /// Sphere ∩ box: a genuinely curved, faceted operand. Methodology:
    /// intersect a `uv_sphere(24, 12, 1.2)` (radius 1.2, centred at origin)
    /// with `cube(2.0)` (`[-1,1]^3`). The result must be closed (`chi = 2`) and
    /// its volume must lie strictly between the box-corner-clipped sphere and
    /// the full unit box — a loose sanity band, since the exact volume of a
    /// faceted-sphere ∩ box has no simple closed form. Pass criterion: `chi=2`
    /// and `0 < volume < 8` (box volume), and `> 4.0` (well above empty).
    ///
    /// Results (asserted below): closed (`chi = 2`); volume in the sane band.
    #[test]
    fn sphere_intersect_box_is_closed() {
        let sphere = primitives::uv_sphere(24, 12, 1.2);
        let cube = primitives::cube(2.0);
        let out = boolean_general(&sphere, &cube, BooleanMode::Intersect).expect("intersect ok");
        assert_eq!(out.euler_characteristic(), 2, "closed result (chi=2)");
        assert!(is_manifold(&out), "sphere∩box must be edge-manifold");
        let vol = signed_volume(&out);
        assert!(vol > 4.0 && vol < 8.0, "sphere∩box volume out of sane band: {vol}");
    }
}
