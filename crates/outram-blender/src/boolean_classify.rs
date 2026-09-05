// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Implements: A. Jacobson, L. Kavan and O. Sorkine-Hornung, "Robust Inside-Outside
// Segmentation using Generalized Winding Numbers", ACM Trans. Graph. 32(4)
// (SIGGRAPH), 2013, article 33. Per-triangle signed solid angle by the formula of
// A. van Oosterom and J. Strackee, "The Solid Angle of a Plane Triangle", IEEE
// Trans. Biomed. Eng. BME-30(2), 1983, pp. 125-126.
// Written from the published formulations; no upstream source was copied.
// Blender analogue (architecture only): the inside/outside classification stage of
// mesh_boolean.cc.
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

//! Inside/outside point classification for a closed triangle mesh.
//!
//! This is the primitive a **general** mesh boolean needs to decide which
//! surface patches of the arrangement to keep: once two operand surfaces are
//! cut against each other, each resulting patch is kept or discarded by
//! sampling a point near it and asking "is this point inside the *other*
//! solid?" [`crate::boolean`]'s current convex half-space clipper does not
//! need this (a convex clip never has to ask "which side wins"), but the
//! honest `Unsupported` Union/Difference path documented there is exactly
//! the gap this module is a building block for.
//!
//! Algorithm reference: Blender `mesh_boolean.cc`
//! (github.com/blender/blender @ 96294be75080bbf687fa7f108e344a1063713586,
//! GPL-2.0-or-later) — inside/outside classification concept (it tracks a
//! per-shape *winding number* per arrangement cell and applies a boolean-op
//! predicate to it — see its `Cell::winding_`, `propagate_windings_and_in_output_volume`,
//! and `apply_bool_op`). This module does **not** transcribe that code: it
//! implements the standalone **generalized winding number** point-in-solid
//! test (Jacobson, Kavan, Sorkine-Hornung 2013) from first principles, plus a
//! textbook ray-casting parity check as an independent cross-check. The
//! `closest_point_on_triangle` helper is the well-known point/triangle
//! closest-point algorithm (Ericson, *Real-Time Collision Detection*,
//! §5.1.5) — public-domain-style textbook algorithm, not Blender code.
//!
//! ## The two techniques
//!
//! - [`winding_number`] — sum, over every triangle of the (fan-triangulated)
//!   mesh, of the **signed solid angle** that triangle subtends at query
//!   point `p` (Van Oosterom–Strackee formula), divided by `4*pi`. For a
//!   closed, outward-oriented, manifold surface this is (very close to) an
//!   **integer**: `0` outside, `+-1` inside a simply-connected solid (a
//!   surface wound inward, or a self-overlapping input, can in principle
//!   produce other integers — see Limitations). It degrades gracefully
//!   (continuously, not catastrophically) even for meshes with small gaps or
//!   local non-manifoldness, which is *why* Blender's own approach and this
//!   one both lean on the same underlying idea.
//! - a private ray-parity helper (Möller–Trumbore ray/triangle intersection,
//!   odd number of crossings along a ray from `p` to infinity ⇒ inside) —
//!   included as an independent numerical cross-check exercised in the test
//!   suite, not as the primary classifier.
//!
//! ## [`classify_point`] tolerance choices
//!
//! All tolerances below are **relative to the mesh's bounding-box diagonal**
//! (`mesh_scale`, the same pattern `boolean::intersect_convex` uses
//! for `clip_eps`/`weld`) so they hold at any model scale, not just
//! unit-sized test meshes:
//!
//! - **`OnBoundary` detection** (`ON_BOUNDARY_REL_EPS = 1e-6`): `p` is
//!   `OnBoundary` if its distance to the *closest point on any triangle* is
//!   `<= 1e-6 * mesh_scale`. This is checked **before** computing the winding
//!   number, because a point essentially on the surface makes individual
//!   solid-angle terms numerically unstable (a near-zero denominator in the
//!   Van Oosterom–Strackee formula) even though the *sum* would still limit
//!   correctly for an exactly-on-surface point in exact arithmetic. `1e-6` is
//!   looser than `boolean.rs`'s `1e-7` convexity tolerance because a
//!   point-to-triangle closest-point query chains more floating-point
//!   operations (six dot products plus divisions) than a single plane
//!   distance, so it accumulates more rounding error.
//! - **`Inside`/`Outside` decision** (`|winding_number| > 0.5`): not a
//!   scale-dependent epsilon at all — for a point that is not on the surface,
//!   the winding number of a closed manifold is within numerical noise of an
//!   integer, so `0.5` is the natural half-way decision boundary with a huge
//!   safety margin (noise is typically `< 1e-6`, not anywhere near `0.5`).
//!
//! ## Limitations (read before trusting this on new geometry)
//!
//! - **Requires a closed, manifold, consistently outward-oriented surface.**
//!   On an **open** mesh (a hole in the surface) the winding number varies
//!   continuously across the hole instead of jumping between integers, so
//!   `classify_point` can return a confident-looking `Inside`/`Outside` that
//!   is meaningless. This module does **not** check watertightness/manifoldness
//!   itself — callers must ensure the input is closed (e.g. everything
//!   [`crate::primitives`] generates, or `boolean::intersect_convex`'s
//!   output).
//! - **Fan triangulation of n-gons is inline and naive** (`(v0, v_i, v_{i+1})`
//!   for `i = 1..n-1`): correct for the convex faces every generator in this
//!   crate produces, but a **non-convex** n-gon can fan-triangulate into
//!   triangles that overlap or fold outside the polygon, silently corrupting
//!   the classification. There is no general n-gon triangulator here.
//! - **Points exactly on the surface are inherently a hard case in floating
//!   point.** `OnBoundary` detection is a distance threshold, not an exact
//!   predicate; a point that is mathematically on the surface but lands just
//!   outside `ON_BOUNDARY_REL_EPS` due to input coordinate rounding will be
//!   classified `Inside` or `Outside` instead, and (rarely) a point that is
//!   merely *very close* to the surface without being on it can be
//!   misclassified `OnBoundary`. No epsilon-based test can avoid this
//!   trade-off; exact/rational arithmetic would be needed to eliminate it.
//! - The private ray-parity cross-check is deliberately **not** the primary
//!   classifier: a ray that grazes a shared edge between two triangles can
//!   double-count or miss a crossing. It defends against this by trying a
//!   fixed list of non-axis-aligned, mutually non-parallel directions and
//!   rejecting any direction whose barycentric hit coordinates land within
//!   `1e-9` of a triangle's edge (signalling "try another direction"); if
//!   *every* candidate direction is degenerate against a given mesh (never
//!   observed in this module's tests, but not provably impossible for
//!   adversarial input) it falls back to a majority vote across all
//!   candidates rather than panicking. It exists to validate
//!   [`winding_number`] in tests, not as a second production API.
//!
//! > **Untrusted AI-generated draft until human-reviewed, per
//! > `RESPONSIBLE_USE.md`** — not for safety-critical use. Verified so far
//! > only against the analytic primitives ([`crate::primitives::cube`],
//! > [`crate::primitives::uv_sphere`]) and a hand-built concave L-prism (see
//! > tests below); not yet validated against arbitrary imported/scanned
//! > geometry.

use crate::math::Vec3;
use crate::mesh::Mesh;

/// Result of classifying a point against a closed mesh's solid interior.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PointClass {
    /// The point is in the solid's interior (winding number `~= +-1`, away
    /// from the surface).
    Inside,
    /// The point is outside the solid (winding number `~= 0`, away from the
    /// surface).
    Outside,
    /// The point lies on (within `ON_BOUNDARY_REL_EPS` * the mesh's
    /// bounding-box diagonal of) the mesh surface itself — neither cleanly
    /// inside nor outside. See the module docs' "Limitations" section for why
    /// this is an epsilon-based judgement call, not an exact predicate.
    OnBoundary,
}

/// Relative (to [`mesh_scale`]) distance tolerance for [`PointClass::OnBoundary`]
/// detection. See the module-level docs for the reasoning behind this value.
const ON_BOUNDARY_REL_EPS: f64 = 1e-6;

/// Barycentric-coordinate tolerance used by the private ray-parity check to
/// detect a ray that grazes a triangle edge or vertex (an unreliable
/// crossing to count) and signal that a different ray direction should be
/// tried instead.
#[cfg(test)]
const RAY_EDGE_BARY_EPS: f64 = 1e-9;

/// The generalized winding number of `p` with respect to `mesh`'s (fan-
/// triangulated) surface.
///
/// This is `(1 / 4*pi)` times the sum, over every triangle, of the **signed
/// solid angle** that triangle subtends at `p`, computed per-triangle with
/// the Van Oosterom–Strackee formula:
///
/// `solid_angle = 2 * atan2(ra . (rb x rc), |ra||rb||rc| + (ra.rb)|rc| + (rb.rc)|ra| + (rc.ra)|rb|)`
///
/// where `ra, rb, rc` are the triangle's vertices relative to `p`
/// (`vertex - p`). Summed over a **closed**, outward-wound surface this
/// telescopes to (very close to) an integer: `0` for `p` outside the solid,
/// `+1` for `p` inside a simply-connected solid whose faces wind
/// counter-clockwise as seen from outside (the convention every
/// [`crate::primitives`] generator and [`crate::mesh::Mesh::face_normal`]
/// use) — see [`PointClass`] and the module docs' "Limitations" for what can
/// make this not hold (open meshes, inconsistent winding, self-overlap).
///
/// A triangle whose relative vertex is (numerically) coincident with `p`
/// (any of `|ra|, |rb|, |rc|` under `1e-12`) is skipped rather than dividing
/// by a near-zero denominator; [`classify_point`] avoids this case in the
/// first place by checking [`PointClass::OnBoundary`] before calling this
/// function, but `winding_number` stays well-defined (if slightly
/// approximate) for a caller that invokes it directly on a near-surface `p`.
pub fn winding_number(mesh: &Mesh, p: Vec3) -> f64 {
    const COINCIDENT_EPS: f64 = 1e-12;

    let mut total_solid_angle = 0.0;
    for [a, b, c] in mesh_triangles(mesh) {
        let ra = a.sub(p);
        let rb = b.sub(p);
        let rc = c.sub(p);
        let la = ra.length();
        let lb = rb.length();
        let lc = rc.length();
        if la < COINCIDENT_EPS || lb < COINCIDENT_EPS || lc < COINCIDENT_EPS {
            continue;
        }

        let numerator = ra.dot(rb.cross(rc));
        let denominator = la * lb * lc + ra.dot(rb) * lc + rb.dot(rc) * la + rc.dot(ra) * lb;
        total_solid_angle += 2.0 * numerator.atan2(denominator);
    }
    total_solid_angle / (4.0 * std::f64::consts::PI)
}

/// Classify `p` against `mesh`'s solid interior.
///
/// First checks [`PointClass::OnBoundary`] (distance to the closest point on
/// any triangle `<= ON_BOUNDARY_REL_EPS * mesh_scale`); if not on the
/// boundary, classifies by [`winding_number`]: `|w| > 0.5` is `Inside`,
/// otherwise `Outside`. See the module docs for the reasoning behind both
/// tolerances and the closed-manifold assumption this relies on.
pub fn classify_point(mesh: &Mesh, p: Vec3) -> PointClass {
    let eps = ON_BOUNDARY_REL_EPS * mesh_scale(mesh).max(1.0);
    if is_on_boundary(mesh, p, eps) {
        return PointClass::OnBoundary;
    }
    if winding_number(mesh, p).abs() > 0.5 {
        PointClass::Inside
    } else {
        PointClass::Outside
    }
}

// ---------------------------------------------------------------------------
// Internal: triangulation, scale, and the boundary distance test.
// ---------------------------------------------------------------------------

/// Bounding-box diagonal length of a mesh's vertices — the size scale used to
/// make every tolerance in this module scale-invariant. Returns `0.0` for an
/// empty mesh. (Same pattern as [`crate::boolean`]'s private `mesh_scale`;
/// duplicated here rather than shared because this file is the crate's only
/// owner of its own helpers per the module's scope.)
fn mesh_scale(mesh: &Mesh) -> f64 {
    let ps = mesh.positions();
    if ps.is_empty() {
        return 0.0;
    }
    let mut lo = ps[0];
    let mut hi = ps[0];
    for &p in &ps {
        lo = Vec3::new(lo.x.min(p.x), lo.y.min(p.y), lo.z.min(p.z));
        hi = Vec3::new(hi.x.max(p.x), hi.y.max(p.y), hi.z.max(p.z));
    }
    hi.sub(lo).length()
}

/// Every face of `mesh`, fan-triangulated from its first vertex:
/// `(v0, v_i, v_{i+1})` for `i = 1..n-1`. See the module docs' "Limitations"
/// for why this is only correct for convex (or at least star-shaped-from-`v0`)
/// faces.
fn mesh_triangles(mesh: &Mesh) -> Vec<[Vec3; 3]> {
    let ps = mesh.positions();
    let mut tris = Vec::new();
    for face in mesh.polygons() {
        if face.len() < 3 {
            continue;
        }
        let v0 = ps[face[0].0];
        for i in 1..face.len() - 1 {
            let v1 = ps[face[i].0];
            let v2 = ps[face[i + 1].0];
            tris.push([v0, v1, v2]);
        }
    }
    tris
}

/// `true` if `p` is within `eps` of the closest point on any triangle of
/// `mesh`'s fan triangulation.
fn is_on_boundary(mesh: &Mesh, p: Vec3, eps: f64) -> bool {
    mesh_triangles(mesh)
        .iter()
        .any(|&[a, b, c]| closest_point_on_triangle(p, a, b, c).sub(p).length() <= eps)
}

/// Closest point to `p` on the (filled) triangle `abc`.
///
/// Standard textbook algorithm (Ericson, *Real-Time Collision Detection*,
/// §5.1.5): first tests whether `p` projects outside each of the triangle's
/// three Voronoi vertex/edge regions (returning the corresponding vertex or
/// edge-projection early), and only computes the full barycentric projection
/// onto the triangle's plane once all of those are ruled out. This is
/// numerically well-behaved for both degenerate-adjacent and typical cases
/// and is not specific to any triangle winding.
fn closest_point_on_triangle(p: Vec3, a: Vec3, b: Vec3, c: Vec3) -> Vec3 {
    let ab = b.sub(a);
    let ac = c.sub(a);
    let ap = p.sub(a);
    let d1 = ab.dot(ap);
    let d2 = ac.dot(ap);
    if d1 <= 0.0 && d2 <= 0.0 {
        return a; // barycentric (1,0,0): closest to vertex a
    }

    let bp = p.sub(b);
    let d3 = ab.dot(bp);
    let d4 = ac.dot(bp);
    if d3 >= 0.0 && d4 <= d3 {
        return b; // barycentric (0,1,0): closest to vertex b
    }

    let vc = d1 * d4 - d3 * d2;
    if vc <= 0.0 && d1 >= 0.0 && d3 <= 0.0 {
        let v = d1 / (d1 - d3);
        return a.add(ab.scale(v)); // on edge ab
    }

    let cp = p.sub(c);
    let d5 = ab.dot(cp);
    let d6 = ac.dot(cp);
    if d6 >= 0.0 && d5 <= d6 {
        return c; // barycentric (0,0,1): closest to vertex c
    }

    let vb = d5 * d2 - d1 * d6;
    if vb <= 0.0 && d2 >= 0.0 && d6 <= 0.0 {
        let w = d2 / (d2 - d6);
        return a.add(ac.scale(w)); // on edge ac
    }

    let va = d3 * d6 - d5 * d4;
    if va <= 0.0 && (d4 - d3) >= 0.0 && (d5 - d6) >= 0.0 {
        let w = (d4 - d3) / ((d4 - d3) + (d5 - d6));
        return b.add(c.sub(b).scale(w)); // on edge bc
    }

    // Inside the face: barycentric projection onto the plane.
    let denom = 1.0 / (va + vb + vc);
    let v = vb * denom;
    let w = vc * denom;
    a.add(ab.scale(v)).add(ac.scale(w))
}

// ---------------------------------------------------------------------------
// Internal: Möller–Trumbore ray-triangle intersection + the ray-parity
// cross-check used only to validate `winding_number` in the test suite.
// ---------------------------------------------------------------------------

/// Möller–Trumbore ray/triangle intersection: returns `(t, u, v)` — the ray
/// parameter and the two non-`a` barycentric coordinates — if the ray
/// `orig + t * dir` (`t` unrestricted in sign; callers filter by `t`) hits
/// the triangle `abc`, or `None` if it misses or is parallel to the
/// triangle's plane.
#[cfg(test)]
fn moller_trumbore(orig: Vec3, dir: Vec3, a: Vec3, b: Vec3, c: Vec3) -> Option<(f64, f64, f64)> {
    const PARALLEL_EPS: f64 = 1e-14;

    let e1 = b.sub(a);
    let e2 = c.sub(a);
    let pvec = dir.cross(e2);
    let det = e1.dot(pvec);
    if det.abs() < PARALLEL_EPS {
        return None; // ray parallel (or near-parallel) to the triangle's plane
    }
    let inv_det = 1.0 / det;

    let tvec = orig.sub(a);
    let u = tvec.dot(pvec) * inv_det;
    if !(0.0..=1.0).contains(&u) {
        return None;
    }

    let qvec = tvec.cross(e1);
    let v = dir.dot(qvec) * inv_det;
    if v < 0.0 || u + v > 1.0 {
        return None;
    }

    let t = e2.dot(qvec) * inv_det;
    Some((t, u, v))
}

/// A fixed, deterministic set of unit ray directions with irrational-ish,
/// mutually non-parallel, non-axis-aligned components — chosen so that a ray
/// cast along any one of them is unlikely to graze an edge/vertex of
/// axis-aligned or otherwise "nice" test geometry (cubes, spheres, prisms).
/// Not cryptographically random and not meant to be: determinism keeps the
/// ray-parity cross-check reproducible between test runs.
#[cfg(test)]
fn candidate_ray_directions() -> [Vec3; 6] {
    [
        Vec3::new(1.0, std::f64::consts::SQRT_2, 3.0_f64.sqrt()),
        Vec3::new(std::f64::consts::PI, 1.0, std::f64::consts::E),
        Vec3::new(-1.0, 2.0, -std::f64::consts::SQRT_2),
        Vec3::new(0.618, -1.414, 2.236),
        Vec3::new(-2.236, 0.577, 1.732),
        Vec3::new(1.234, -5.678, 2.345),
    ]
    .map(|v| v.normalize())
}

/// Count crossings of the ray `p + t * dir` (`t > edge_eps`, i.e. strictly
/// ahead of `p`) with `mesh`'s fan-triangulated surface, or `None` if any
/// crossing's barycentric coordinates land within [`RAY_EDGE_BARY_EPS`] of a
/// triangle edge/vertex — an unreliable count, since a ray grazing a shared
/// edge between two triangles can be double-counted or missed depending on
/// floating-point rounding at that exact edge.
#[cfg(test)]
fn ray_crossing_count(mesh: &Mesh, p: Vec3, dir: Vec3, edge_eps: f64) -> Option<usize> {
    let mut count = 0usize;
    for [a, b, c] in mesh_triangles(mesh) {
        if let Some((t, u, v)) = moller_trumbore(p, dir, a, b, c) {
            if t <= edge_eps {
                continue; // behind (or at) the ray origin
            }
            let w = 1.0 - u - v;
            if u < RAY_EDGE_BARY_EPS || v < RAY_EDGE_BARY_EPS || w < RAY_EDGE_BARY_EPS {
                return None; // grazes an edge/vertex — this direction is unreliable
            }
            count += 1;
        }
    }
    Some(count)
}

/// Ray-parity cross-check: is `p` inside `mesh` by the "cast a ray to
/// infinity and count crossings" test (odd ⇒ inside)? Tries each of
/// [`candidate_ray_directions`] in turn and uses the first one that produces
/// a clean (non-degenerate) crossing count; if every candidate is degenerate
/// against this particular mesh (not observed in this module's own tests),
/// falls back to a majority vote across all of them rather than panicking.
///
/// This exists purely to validate [`winding_number`]-based [`classify_point`]
/// in the test suite below — see the module docs' "Limitations" for why it is
/// not promoted to a second production classifier.
#[cfg(test)]
fn ray_parity_inside(mesh: &Mesh, p: Vec3) -> bool {
    let edge_eps = 1e-9 * mesh_scale(mesh).max(1.0);

    for dir in candidate_ray_directions() {
        if let Some(count) = ray_crossing_count(mesh, p, dir, edge_eps) {
            return count % 2 == 1;
        }
    }

    // Fallback: every candidate direction was degenerate against this mesh.
    // Majority-vote the raw (non-strict) parity across all candidates rather
    // than treating this as a hard error.
    let mut inside_votes = 0;
    let mut outside_votes = 0;
    for dir in candidate_ray_directions() {
        let mut count = 0usize;
        for [a, b, c] in mesh_triangles(mesh) {
            if let Some((t, _, _)) = moller_trumbore(p, dir, a, b, c) {
                if t > edge_eps {
                    count += 1;
                }
            }
        }
        if count % 2 == 1 {
            inside_votes += 1;
        } else {
            outside_votes += 1;
        }
    }
    inside_votes > outside_votes
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::primitives;

    fn approx(a: f64, b: f64, tol: f64) -> bool {
        (a - b).abs() <= tol
    }

    /// A non-convex closed genus-0 mesh: an **L-shaped prism** (L cross-section
    /// in the z=0..1 slab). Copied verbatim from `boolean.rs`'s test helper of
    /// the same name so this module's concavity test builds an identical
    /// geometry (`V=12, E=18, F=8 -> chi = 2`, reentrant corner at `(1,1)` in
    /// the xy cross-section).
    fn l_prism() -> Mesh {
        // L cross-section, CCW in xy (concave at D=(1,1)).
        let xy = [
            (0.0, 0.0), // A
            (2.0, 0.0), // B
            (2.0, 1.0), // C
            (1.0, 1.0), // D  (reflex corner)
            (1.0, 2.0), // E
            (0.0, 2.0), // F
        ];
        let mut positions: Vec<Vec3> = Vec::new();
        for &(x, y) in &xy {
            positions.push(Vec3::new(x, y, 0.0)); // bottom 0..5
        }
        for &(x, y) in &xy {
            positions.push(Vec3::new(x, y, 1.0)); // top 6..11
        }
        let n = xy.len();
        let mut faces: Vec<Vec<usize>> = Vec::new();
        // Bottom cap (outward -z): reverse of the CCW xy order.
        faces.push((0..n).rev().collect());
        // Top cap (outward +z): CCW xy order, shifted by n.
        faces.push((0..n).map(|i| i + n).collect());
        // Side quads: [b_i, b_{i+1}, t_{i+1}, t_i] -> outward normal.
        for i in 0..n {
            let j = (i + 1) % n;
            faces.push(vec![i, j, j + n, i + n]);
        }
        Mesh::from_polygons(&positions, &faces)
    }

    /// Deterministic (xorshift64-seeded), not-actually-random point cloud in
    /// `[-2, 2]^3` — used only for the winding/ray-parity agreement batch test.
    /// Deterministic on purpose: a flaky test on a random seed would be worse
    /// than a fixed, reviewable set of query points.
    fn xorshift_points(n: usize) -> Vec<Vec3> {
        let mut seed: u64 = 88172645463325252;
        let mut next = || {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            ((seed >> 16) & 0xffff) as f64 / 65535.0 * 4.0 - 2.0 // -> [-2, 2]
        };
        (0..n).map(|_| Vec3::new(next(), next(), next())).collect()
    }

    /// Methodology: `winding_number` at the origin of `primitives::cube(2.0)`
    /// (`[-1,1]^3`, origin is deep interior) should be `~= 1` (outward-wound
    /// closed cube); at a far point `(10,10,10)` (well outside) it should be
    /// `~= 0`. Pass criterion: both within `1e-6` of the analytically expected
    /// integer, since a cube's faces are flat (no faceting error).
    ///
    /// Results (measured by this test, asserted below): interior winding
    /// number and exterior winding number both land within `1e-6` of `1.0`
    /// and `0.0` respectively — the formula is exact (up to floating-point
    /// rounding) for a flat-faced convex solid.
    #[test]
    fn winding_number_cube_inside_and_outside() {
        let mesh = primitives::cube(2.0);
        let w_in = winding_number(&mesh, Vec3::ZERO);
        assert!(
            approx(w_in.abs(), 1.0, 1e-6),
            "expected |w| ~= 1 inside cube, got {w_in}"
        );
        let w_out = winding_number(&mesh, Vec3::new(10.0, 10.0, 10.0));
        assert!(
            approx(w_out.abs(), 0.0, 1e-6),
            "expected |w| ~= 0 outside cube, got {w_out}"
        );
    }

    /// `classify_point` agrees with the winding-number test above: origin
    /// Inside, far point Outside.
    #[test]
    fn classify_point_cube() {
        let mesh = primitives::cube(2.0);
        assert_eq!(classify_point(&mesh, Vec3::ZERO), PointClass::Inside);
        assert_eq!(
            classify_point(&mesh, Vec3::new(10.0, 10.0, 10.0)),
            PointClass::Outside
        );
    }

    /// Methodology: same inside/outside check as the cube, but on a faceted
    /// `primitives::uv_sphere(24, 12, 1.0)` (centred at the origin per
    /// `uv_sphere`'s construction). A polygonal approximation of a sphere is
    /// not an exact solid, so the tolerance is looser than the flat-faced cube
    /// case. Pass criterion: `|w|` within `1e-3` of `1.0` at the center, `Inside`;
    /// within `1e-6` of `0.0` at `(10,10,10)`, `Outside`.
    ///
    /// Results (measured by this test, asserted below): center winding number
    /// within `1e-3` of `1.0`; far-point winding number within `1e-6` of `0.0`;
    /// `classify_point` agrees (`Inside` / `Outside`).
    #[test]
    fn classify_point_uv_sphere() {
        let mesh = primitives::uv_sphere(24, 12, 1.0);
        let w_in = winding_number(&mesh, Vec3::ZERO);
        // Sign, not just magnitude: an outward-wound sphere gives +1 inside.
        // (Guards the uv_sphere inward-winding regression, bead op-hzs.14.)
        assert!(
            approx(w_in, 1.0, 1e-3),
            "expected w ~= +1 at sphere center, got {w_in}"
        );
        assert_eq!(classify_point(&mesh, Vec3::ZERO), PointClass::Inside);

        let w_out = winding_number(&mesh, Vec3::new(10.0, 10.0, 10.0));
        assert!(
            approx(w_out, 0.0, 1e-6),
            "expected |w| ~= 0 far outside sphere, got {w_out}"
        );
        assert_eq!(
            classify_point(&mesh, Vec3::new(10.0, 10.0, 10.0)),
            PointClass::Outside
        );
    }

    /// Methodology: the case that distinguishes true classification from a
    /// bounding-box test. `l_prism()`'s xy cross-section is the union of the
    /// strips `x in [0,1], y in [0,2]` and `x in [0,2], y in [0,1]` — an
    /// L-shape whose axis-aligned bounding box `[0,2]x[0,2]` also covers the
    /// **reentrant notch** `x in (1,2), y in (1,2)`, which is not part of the
    /// solid. `p = (1.5, 1.5, 0.5)` sits in that notch (inside the bounding
    /// box, outside the actual solid); `p_in = (0.5, 0.5, 0.5)` sits in the
    /// solid's left column for comparison. Pass criterion: `p` classifies
    /// `Outside` (and `|w| < 0.5`) despite being inside the AABB; `p_in`
    /// classifies `Inside`.
    ///
    /// Results (measured by this test, asserted below): `p`'s winding number
    /// has magnitude `< 0.5` (specifically very close to `0`, as expected for
    /// a point outside a simply-connected solid) -> `Outside`; `p_in` ->
    /// `Inside`.
    #[test]
    fn concave_notch_is_outside_despite_being_inside_the_aabb() {
        let mesh = l_prism();

        let p = Vec3::new(1.5, 1.5, 0.5);
        let w = winding_number(&mesh, p);
        assert!(
            w.abs() < 0.5,
            "notch point should have |w| well under 0.5, got {w}"
        );
        assert_eq!(
            classify_point(&mesh, p),
            PointClass::Outside,
            "reentrant notch point must be Outside despite lying inside the prism's AABB"
        );

        let p_in = Vec3::new(0.5, 0.5, 0.5);
        assert_eq!(classify_point(&mesh, p_in), PointClass::Inside);
    }

    /// Methodology: cross-check [`winding_number`]-based [`classify_point`]
    /// against the independent ray-parity helper on a batch of deterministic
    /// pseudo-random points around `primitives::cube(2.0)`. Points that land
    /// close enough to classify `OnBoundary` are skipped (both methods are
    /// expected to disagree/be unstable exactly at the surface — that is the
    /// documented limitation, not a bug), and the test asserts a healthy
    /// majority of the batch was actually off the boundary and checked, so a
    /// bug that made every point look `OnBoundary` (silently skipping the
    /// entire batch) would still fail loudly.
    ///
    /// Results (measured by this test, asserted below): of 60 candidate
    /// points in `[-2,2]^3` against the unit-scale `[-1,1]^3` cube, well over
    /// 20 are off the boundary and checked, and the two independent methods
    /// (generalized winding number vs. ray-parity) agree on every one of them.
    #[test]
    fn winding_and_ray_parity_agree_on_random_points_cube() {
        let mesh = primitives::cube(2.0);
        let mut checked = 0;
        for p in xorshift_points(60) {
            let class = classify_point(&mesh, p);
            if matches!(class, PointClass::OnBoundary) {
                continue;
            }
            let winding_says_inside = matches!(class, PointClass::Inside);
            let ray_says_inside = ray_parity_inside(&mesh, p);
            assert_eq!(
                winding_says_inside, ray_says_inside,
                "winding-number and ray-parity disagree at {p:?} (winding class = {class:?})"
            );
            checked += 1;
        }
        assert!(checked > 20, "expected most of the 60 sample points to be off the boundary, only {checked} were checked");
    }
}
