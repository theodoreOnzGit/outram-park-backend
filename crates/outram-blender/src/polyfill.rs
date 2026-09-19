// SPDX-License-Identifier: GPL-3.0-only
//
// ─── Ported from Blender ──────────────────────────────────────────────────
//  Upstream project : Blender <https://github.com/blender/blender>
//  Upstream file    : source/blender/blenlib/intern/polyfill_2d.cc
//                     (+ ortho_basis_v3v3_v3 / axis_dominant_v3_to_m3 from
//                      source/blender/blenlib/intern/math_vector.cc and
//                      source/blender/blenlib/intern/math_geom.cc)
//  Upstream commit  : 786af64aad84154047d93ee077e1fdd1d229f32d (sparse clone,
//                     read 2026-09-19; see upstream_source/README.md)
//  Upstream © text  : SPDX-FileCopyrightText: 2023 Blender Authors
//  Upstream licence : SPDX-License-Identifier: GPL-2.0-or-later
//
//  This port © 2026 OUTRAM PARK contributors, distributed as GPL-3.0-only.
//  Blender's "or later" clause is what makes that relicensing permitted; the
//  flow is ONE-WAY — this file cannot travel back upstream under GPL-3.0-only.
//  Do not strip this block during refactors (workspace
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

//! Ear-clipping triangulation of a single-boundary polygon — a port of
//! Blender's `BLI_polyfill_calc` (`polyfill_2d.cc`).
//!
//! # What this computes
//!
//! Given the `n` corners of one simple polygon, in order, this emits exactly
//! `n - 2` triangles as index triples into that corner list. The triangles
//! tile the polygon's interior and do not overlap. This is a *geometric*
//! routine — it produces no physical quantity and carries no units; the
//! inputs are plain positions in whatever length unit the caller's mesh uses,
//! and the outputs are corner indices.
//!
//! # Why this exists — fan triangulation is wrong on concave faces
//!
//! [`crate::triangulate`] historically fanned every face from its first
//! corner. For a **convex** polygon a fan is correct. For a **concave** one it
//! is not: the fan emits triangles that stick out past the boundary and
//! overlap each other, so the triangulated surface no longer bounds the same
//! solid. That matters here because this crate's meshes are handed to a CFD
//! volume mesher and a Monte Carlo CSG bridge, both of which take the triangle
//! soup as the truth about the geometry.
//!
//! Upstream does not fan. `bmo_triangulate` routes n-gons through
//! `BLI_polyfill_calc`, which is the ear-clipping algorithm ported here.
//!
//! # Fidelity to upstream, and the three deliberate deviations
//!
//! The control flow, the two-pass ear search, the "desperate mode" fallback
//! and the sign conventions are transcribed from `polyfill_2d.cc` rather than
//! re-derived. Three things differ, each for a stated reason:
//!
//! 1. **No k-d tree.** Upstream compiles `USE_KDTREE` to accelerate the
//!    point-in-candidate-ear test. This port takes upstream's *own*
//!    `#else` branch — the linear scan over concave corners, which is in
//!    `pf_ear_tip_check` verbatim — so the result is identical and only the
//!    complexity differs (`O(n * concave)` rather than `O(n log n)`). A k-d
//!    tree is an acceleration structure, not a semantic: porting it would add
//!    ~330 lines that cannot change an answer.
//! 2. **`f64`, not `f32`.** This crate is `f64` throughout. Upstream's own
//!    comments note the `TANGENTIAL` test compares exactly against `0.0`, so
//!    the *classification* of a near-degenerate corner can differ between the
//!    two precisions. Neither is "right"; ours is the stricter one.
//! 3. **Indices, not pointers.** Upstream's circular doubly-linked list is
//!    raw `PolyIndex *`. The workspace forbids lifetime parameters and `Box`,
//!    so the list is an index-linked `Vec` — same structure, same operations.
//!
//! # Winding — read this before using [`polyfill_2d`] directly
//!
//! Upstream normalises the working polygon to **clockwise** in 2-D and assumes
//! that orientation everywhere downstream ("Because the polygon has clockwise
//! winding order, the area sign will be positive if the point is strictly
//! inside"). A consequence, which `BLI_polyfill_2d.hh` states as part of the
//! contract, is that **the emitted triples are clockwise whatever the input
//! ring's winding was**. That is ported as-is: [`polyfill_2d`] emits clockwise
//! triangles from a counter-clockwise ring.
//!
//! Upstream's own mesh callers deal with this by projecting through the
//! *negated* face normal — `axis_dominant_v3_to_m3_negate` in
//! `bmesh_mesh_tessellate.cc:116`, immediately before
//! `BLI_polyfill_calc_arena(projverts, len, 1, ...)` — so the 2-D ring is
//! already clockwise and the clockwise output maps back to the face's own
//! winding in 3-D. [`polyfill_3d`] does exactly that, and is therefore the
//! entry point a mesh operator should call: it preserves the caller's winding
//! and hence its outward normal.

use crate::math::Vec3;

/// Corner classification, matching upstream's `eSign`.
///
/// The discriminants match upstream's `CONCAVE = -1, TANGENTIAL = 0,
/// CONVEX = 1` so the two-pass ear search reads the same way.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Sign {
    Concave,
    Tangential,
    Convex,
}

/// Upstream `signum_enum` (`polyfill_2d.cc`).
///
/// Note the exact `== 0.0` comparison: that is upstream's, and it is what
/// makes `TANGENTIAL` precision-sensitive (see the module docs).
fn signum_enum(a: f64) -> Sign {
    if a > 0.0 {
        Sign::Convex
    } else if a == 0.0 {
        Sign::Tangential
    } else {
        Sign::Concave
    }
}

/// Upstream `area_tri_signed_v2_alt_2x` — twice the signed area, the `/ 2`
/// dropped because only the sign is ever used.
fn area_tri_signed_v2_alt_2x(v1: [f64; 2], v2: [f64; 2], v3: [f64; 2]) -> f64 {
    let d2 = [v2[0] - v1[0], v2[1] - v1[1]];
    let d3 = [v3[0] - v1[0], v3[1] - v1[1]];
    d2[0] * d3[1] - d3[0] * d2[1]
}

/// Upstream `span_tri_v2_sign`. The argument reversal (`v3, v2, v1`) is
/// upstream's and is load-bearing — it is what makes `CONVEX` positive under
/// the clockwise normalisation.
fn span_tri_v2_sign(v1: [f64; 2], v2: [f64; 2], v3: [f64; 2]) -> Sign {
    signum_enum(area_tri_signed_v2_alt_2x(v3, v2, v1))
}

/// Upstream `cross_poly_v2` — the trapezium area rule, twice the signed area.
///
/// Positive for a counter-clockwise ring. Used only to decide which way to
/// walk the corner list.
fn cross_poly_v2(coords: &[[f64; 2]]) -> f64 {
    let n = coords.len();
    if n == 0 {
        return 0.0;
    }
    let mut cross = 0.0;
    let mut prev = coords[n - 1];
    for &curr in coords.iter() {
        cross += (prev[0] - curr[0]) * (curr[1] + prev[1]);
        prev = curr;
    }
    cross
}

/// One corner of the working polygon: a node in upstream's circular
/// doubly-linked `PolyIndex` list, with `usize` slots instead of pointers.
#[derive(Clone, Copy)]
struct PolyIndex {
    next: usize,
    prev: usize,
    /// Index into the caller's `coords`, which is *not* the slot number when
    /// the polygon was reversed for the clockwise normalisation.
    index: usize,
    sign: Sign,
}

/// Upstream `ortho_basis_v3v3_v3` — a right-handed orthonormal pair spanning
/// the plane whose normal is `n`.
///
/// Returns `(n1, n2)` with `n1 × n2 = n`, so a ring wound counter-clockwise
/// about `n` in 3-D projects to a counter-clockwise ring in 2-D. `n` must be
/// unit length.
fn ortho_basis(n: Vec3) -> (Vec3, Vec3) {
    // Upstream's `eps` is FLT_EPSILON against an f32 magnitude; the f64
    // equivalent of "the normal is essentially +/-Z" is the same test taken
    // at this crate's precision.
    let eps = f64::EPSILON;
    let f = n.x * n.x + n.y * n.y;
    if f > eps {
        let d = 1.0 / f.sqrt();
        let n1 = Vec3::new(n.y * d, -n.x * d, 0.0);
        let n2 = Vec3::new(-n.z * n1.y, n.z * n1.x, n.x * n1.y - n.y * n1.x);
        (n1, n2)
    } else {
        // Degenerate case: the normal is along Z, so any X/Y pair will do.
        let n1 = Vec3::new(if n.z < 0.0 { -1.0 } else { 1.0 }, 0.0, 0.0);
        let n2 = Vec3::new(0.0, 1.0, 0.0);
        (n1, n2)
    }
}

/// Project 3-D polygon corners onto the plane of `normal`, as upstream's
/// `axis_dominant_v3_to_m3` + `mul_v2_m3v3` pair does.
///
/// `normal` need not be unit length; it is normalised here. A zero-length
/// normal (a fully degenerate face) falls back to the X/Y plane, which keeps
/// the routine total rather than panicking.
pub fn project_to_plane(points: &[Vec3], normal: Vec3) -> Vec<[f64; 2]> {
    let len = normal.length();
    let n = if len > 0.0 {
        Vec3::new(normal.x / len, normal.y / len, normal.z / len)
    } else {
        Vec3::new(0.0, 0.0, 1.0)
    };
    let (n1, n2) = ortho_basis(n);
    points.iter().map(|&p| [p.dot(n1), p.dot(n2)]).collect()
}

/// Ear-clip a single-boundary 2-D polygon into `coords.len() - 2` triangles.
///
/// Returns index triples into `coords`, **wound clockwise regardless of the
/// input ring's winding** — upstream's documented contract
/// (`BLI_polyfill_2d.hh`: "This array is filled in with triangle indices in
/// clockwise order"). If you need the caller's winding preserved, use
/// [`polyfill_3d`], which applies upstream's negated-normal projection.
///
/// Fewer than three corners yields no triangles. Self-intersecting input is
/// degenerate — upstream's contract is that the triangle *count* and the
/// non-repetition of indices still hold, but the triangles may overlap.
///
/// Upstream takes a `coords_sign` hint to skip the winding test; this port
/// always measures it, which is semantically the same and one shoelace pass
/// slower.
///
/// This is the port of `BLI_polyfill_calc`; see the module docs for the three
/// deviations from upstream.
pub fn polyfill_2d(coords: &[[f64; 2]]) -> Vec<[usize; 3]> {
    let coords_num = coords.len();
    if coords_num < 3 {
        return Vec::new();
    }

    // ---- upstream `polyfill_prepare` -----------------------------------
    // Normalise to clockwise: everything below assumes it.
    let coords_sign = if cross_poly_v2(coords) <= 0.0 { 1 } else { -1 };

    let mut indices: Vec<PolyIndex> = (0..coords_num)
        .map(|i| PolyIndex {
            next: (i + 1) % coords_num,
            prev: (i + coords_num - 1) % coords_num,
            index: if coords_sign == 1 {
                i
            } else {
                coords_num - 1 - i
            },
            sign: Sign::Tangential, // filled in below
        })
        .collect();

    let mut coords_num_concave = 0usize;
    for i in 0..coords_num {
        let s = coord_sign_calc(coords, &indices, i);
        indices[i].sign = s;
        if s != Sign::Convex {
            coords_num_concave += 1;
        }
    }

    // ---- upstream `pf_triangulate` -------------------------------------
    let mut tris: Vec<[usize; 3]> = Vec::with_capacity(coords_num - 2);
    let mut head = 0usize;
    let mut live = coords_num;
    // USE_CLIP_EVEN: advance the start of the ear search each iteration, so a
    // convex shape does not degenerate back into a fan.
    let mut ear_init = head;
    // USE_CLIP_SWEEP: alternate search direction about convex ears, which
    // stops the fill going lop-sided.
    let mut reverse = false;

    while live > 3 {
        let ear = ear_tip_find(
            coords,
            &indices,
            head,
            live,
            coords_num_concave,
            ear_init,
            reverse,
        );

        // USE_CONVEX_SKIP bookkeeping: this corner leaves the ring.
        if indices[ear].sign != Sign::Convex {
            coords_num_concave -= 1;
        }

        let prev = indices[ear].prev;
        let next = indices[ear].next;

        // upstream `pf_ear_tip_cut`
        tris.push([indices[prev].index, indices[ear].index, indices[next].index]);
        coord_remove(&mut indices, &mut head, ear);
        live -= 1;

        // Clipping the ear can make either neighbour convex; upstream
        // recomputes only the ones that were not already convex.
        if indices[prev].sign != Sign::Convex {
            let s = coord_sign_calc(coords, &indices, prev);
            indices[prev].sign = s;
            if s == Sign::Convex {
                coords_num_concave -= 1;
            }
        }
        if indices[next].sign != Sign::Convex {
            let s = coord_sign_calc(coords, &indices, next);
            indices[next].sign = s;
            if s == Sign::Convex {
                coords_num_concave -= 1;
            }
        }

        // USE_CLIP_EVEN + USE_CLIP_SWEEP: pick where the next search starts,
        // and flip direction when that start is not a promising ear.
        ear_init = if reverse {
            indices[prev].prev
        } else {
            indices[next].next
        };
        if indices[ear_init].sign != Sign::Convex {
            ear_init = if reverse {
                indices[ear_init].prev
            } else {
                indices[ear_init].next
            };
            reverse = !reverse;
        }
    }

    if live == 3 {
        let a = head;
        let b = indices[a].next;
        let c = indices[b].next;
        tris.push([indices[a].index, indices[b].index, indices[c].index]);
    }

    tris
}

/// Upstream `pf_coord_sign_calc` — classify corner `i` from its neighbours.
fn coord_sign_calc(coords: &[[f64; 2]], indices: &[PolyIndex], i: usize) -> Sign {
    let pi = indices[i];
    span_tri_v2_sign(
        coords[indices[pi.prev].index],
        coords[pi.index],
        coords[indices[pi.next].index],
    )
}

/// Upstream `pf_coord_remove` — unlink a corner, advancing `head` if it was
/// the head.
fn coord_remove(indices: &mut [PolyIndex], head: &mut usize, i: usize) {
    let (prev, next) = (indices[i].prev, indices[i].next);
    indices[next].prev = prev;
    indices[prev].next = next;
    if *head == i {
        *head = next;
    }
}

/// Upstream `pf_ear_tip_find` — two accepting passes, then "desperate mode".
#[allow(clippy::too_many_arguments)]
fn ear_tip_find(
    coords: &[[f64; 2]],
    indices: &[PolyIndex],
    head: usize,
    live: usize,
    coords_num_concave: usize,
    ear_init: usize,
    reverse: bool,
) -> usize {
    // Pass 1 accepts only CONVEX corners, which avoids emitting zero-area
    // triangles. Pass 2 (TANGENTIAL) is reached only for degenerate polygons,
    // where a zero-area triangle beats no triangle. Upstream ref: #103913.
    for &sign_accept in &[Sign::Convex, Sign::Tangential] {
        let mut ear = ear_init;
        for _ in 0..live {
            if ear_tip_check(coords, indices, ear, sign_accept, coords_num_concave) {
                return ear;
            }
            ear = if reverse {
                indices[ear].prev
            } else {
                indices[ear].next
            };
        }
    }

    // Desperate mode (Held, "FIST: Fast industrial-strength triangulation of
    // polygons", Algorithmica 1998): no corner is a valid ear, so the polygon
    // has become degenerate — take any non-concave corner.
    let mut ear = ear_init;
    for _ in 0..live {
        if indices[ear].sign != Sign::Concave {
            return ear;
        }
        ear = indices[ear].next;
    }

    // Every corner concave: upstream returns the last one walked. `head` is
    // only reached when `live` is 0, which the caller prevents.
    let _ = head;
    ear
}

/// Upstream `pf_ear_tip_check`, `#else` (non-k-d-tree) branch.
///
/// True when the candidate corner's triangle contains no other concave
/// corner, i.e. it can be clipped off.
fn ear_tip_check(
    coords: &[[f64; 2]],
    indices: &[PolyIndex],
    ear: usize,
    sign_accept: Sign,
    coords_num_concave: usize,
) -> bool {
    // USE_CONVEX_SKIP fast path: with no concave corners left the polygon is
    // convex, so every corner is an ear.
    if coords_num_concave == 0 {
        return true;
    }
    if indices[ear].sign != sign_accept {
        return false;
    }

    let v1 = coords[indices[indices[ear].prev].index];
    let v2 = coords[indices[ear].index];
    let v3 = coords[indices[indices[ear].next].index];

    // Walk the corners strictly between next and prev — the three corners of
    // the candidate triangle itself are skipped, or the test always trips.
    let mut checked = 0usize;
    let stop = indices[ear].prev;
    let mut curr = indices[indices[ear].next].next;
    while curr != stop {
        // Only concave (and tangential) corners can lie inside a candidate
        // ear; convex ones cannot, which is what makes the skip sound.
        if indices[curr].sign != Sign::Convex {
            let v = coords[indices[curr].index];
            // Clockwise winding means "strictly inside" reads as a positive
            // area against all three edges; 0 (on the edge) counts as inside
            // too, hence `!= CONCAVE`. Upstream tests (v3,v1) first because
            // it rejects far more often than the other two.
            if span_tri_v2_sign(v3, v1, v) != Sign::Concave
                && span_tri_v2_sign(v1, v2, v) != Sign::Concave
                && span_tri_v2_sign(v2, v3, v) != Sign::Concave
            {
                return false;
            }
            checked += 1;
            if checked == coords_num_concave {
                break;
            }
        }
        curr = indices[curr].next;
    }

    true
}

/// Ear-clip a 3-D polygon given its corner positions and face normal.
///
/// Returns index triples into `points`, **wound the same way the input ring
/// is** — so a face's outward normal survives triangulation. This is the
/// entry point a mesh operator wants.
///
/// The winding is preserved by projecting through the *negated* normal, which
/// is what upstream's tessellation path does
/// (`axis_dominant_v3_to_m3_negate`, `bmesh_mesh_tessellate.cc:116`): it makes
/// the 2-D ring clockwise, so [`polyfill_2d`]'s clockwise output maps back to
/// the face's own orientation. Getting this backwards inverts every normal on
/// the triangulated mesh, which is why it is spelled out rather than left to
/// the reader.
pub fn polyfill_3d(points: &[Vec3], normal: Vec3) -> Vec<[usize; 3]> {
    if points.len() < 3 {
        return Vec::new();
    }
    let coords = project_to_plane(points, normal.scale(-1.0));
    polyfill_2d(&coords)
}

/// Newell's method for a polygon normal — robust for non-planar rings, where
/// a single cross product of three corners is not.
///
/// Returns a non-normalised vector whose direction is the face's outward
/// normal under the ring's own winding, and whose length is twice the
/// projected area.
pub fn newell_normal(points: &[Vec3]) -> Vec3 {
    let n = points.len();
    let mut nx = 0.0;
    let mut ny = 0.0;
    let mut nz = 0.0;
    for i in 0..n {
        let a = points[i];
        let b = points[(i + 1) % n];
        nx += (a.y - b.y) * (a.z + b.z);
        ny += (a.z - b.z) * (a.x + b.x);
        nz += (a.x - b.x) * (a.y + b.y);
    }
    Vec3::new(nx, ny, nz)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Twice the signed area of a 2-D triangle.
    fn tri_area2(a: [f64; 2], b: [f64; 2], c: [f64; 2]) -> f64 {
        (b[0] - a[0]) * (c[1] - a[1]) - (c[0] - a[0]) * (b[1] - a[1])
    }

    /// Sum of |area| over the emitted triangles.
    fn tris_area(coords: &[[f64; 2]], tris: &[[usize; 3]]) -> f64 {
        tris.iter()
            .map(|t| 0.5 * tri_area2(coords[t[0]], coords[t[1]], coords[t[2]]).abs())
            .sum()
    }

    /// Shoelace area of the polygon itself.
    fn poly_area(coords: &[[f64; 2]]) -> f64 {
        0.5 * cross_poly_v2(coords).abs()
    }

    #[test]
    fn square_yields_two_triangles() {
        let sq = [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]];
        let tris = polyfill_2d(&sq);
        assert_eq!(tris.len(), 2);
        assert!((tris_area(&sq, &tris) - 1.0).abs() < 1e-12);
    }

    #[test]
    fn triangle_count_is_always_n_minus_two() {
        for n in 3..24usize {
            // A convex regular n-gon.
            let coords: Vec<[f64; 2]> = (0..n)
                .map(|i| {
                    let t = std::f64::consts::TAU * (i as f64) / (n as f64);
                    [t.cos(), t.sin()]
                })
                .collect();
            assert_eq!(polyfill_2d(&coords).len(), n - 2, "n = {n}");
        }
    }

    /// The defect this module exists to fix.
    ///
    /// # Methodology
    ///
    /// An L-shaped hexagon (a unit square with its top-right quadrant
    /// removed) has area 0.75. It is not star-shaped from every corner, so a
    /// fan rooted at the wrong corner emits triangles that cross the reflex
    /// notch and leave the polygon; their |area| then overshoots the truth.
    /// Ear clipping must land on 0.75 from *every* rotation of the ring.
    ///
    /// # Results (measured 2026-09-19)
    ///
    /// Ear clipping: 0.75 to within 1e-12 for **all six** rotations.
    ///
    /// Fan, per apex corner:
    ///
    /// | apex | fan total \|area\| | error |
    /// |---|---|---|
    /// | (0, 0) | 0.750 | exact |
    /// | (1, 0) | 1.000 | +33.3 % |
    /// | (1, 0.5) | 1.000 | +33.3 % |
    /// | (0.5, 0.5) | 0.750 | exact |
    /// | (0.5, 1) | 1.000 | +33.3 % |
    /// | (0, 1) | 1.000 | +33.3 % |
    ///
    /// So the fan is correct from only **two of six** corners, and wrong by a
    /// third of the area from the other four. That is not a tolerance
    /// question — it is triangles lying outside the boundary. A face's corner
    /// order is whatever the mesh happens to store, so the old fan was
    /// landing on a broken rotation for most concave faces.
    #[test]
    fn concave_l_shape_is_tiled_exactly_where_a_fan_is_not() {
        // (0,0) (1,0) (1,0.5) (0.5,0.5) (0.5,1) (0,1)  — CCW, area 0.75.
        let base = [
            [0.0, 0.0],
            [1.0, 0.0],
            [1.0, 0.5],
            [0.5, 0.5],
            [0.5, 1.0],
            [0.0, 1.0],
        ];
        let truth = poly_area(&base);
        assert!((truth - 0.75).abs() < 1e-12, "fixture area {truth}");

        let mut fan_overshot = 0;
        for rot in 0..base.len() {
            let l: Vec<[f64; 2]> = (0..base.len())
                .map(|i| base[(i + rot) % base.len()])
                .collect();

            let tris = polyfill_2d(&l);
            assert_eq!(tris.len(), 4, "rotation {rot}");
            let ear = tris_area(&l, &tris);
            assert!(
                (ear - truth).abs() < 1e-12,
                "rotation {rot}: ear clipping should tile exactly, got {ear}"
            );

            // The fan the old `triangulate` used, rooted at this rotation's
            // first corner.
            let fan: Vec<[usize; 3]> = (1..l.len() - 1).map(|i| [0, i, i + 1]).collect();
            if tris_area(&l, &fan) > truth + 1e-9 {
                fan_overshot += 1;
            }
        }
        assert_eq!(
            fan_overshot, 4,
            "the fan is measured to fail on exactly four of six rotations here"
        );
    }

    /// [`polyfill_2d`] emits clockwise triangles whatever the input winding —
    /// upstream's documented contract, and the reason [`polyfill_3d`] has to
    /// negate the normal.
    #[test]
    fn polyfill_2d_always_emits_clockwise_triangles() {
        let ccw = [
            [0.0, 0.0],
            [1.0, 0.0],
            [1.0, 0.5],
            [0.5, 0.5],
            [0.5, 1.0],
            [0.0, 1.0],
        ];
        let mut cw = ccw;
        cw.reverse();
        for ring in [&ccw[..], &cw[..]] {
            for t in polyfill_2d(ring) {
                let a2 = tri_area2(ring[t[0]], ring[t[1]], ring[t[2]]);
                assert!(a2 < 0.0, "triangle {t:?} should be clockwise, area2 = {a2}");
            }
        }
    }

    /// [`polyfill_3d`], by contrast, preserves the caller's winding — every
    /// emitted triangle's normal agrees with the face normal.
    #[test]
    fn polyfill_3d_preserves_face_winding() {
        let pts: Vec<Vec3> = [
            (0.0, 0.0),
            (1.0, 0.0),
            (1.0, 0.5),
            (0.5, 0.5),
            (0.5, 1.0),
            (0.0, 1.0),
        ]
        .iter()
        .map(|&(x, y)| Vec3::new(x, y, 0.0))
        .collect();
        let nrm = newell_normal(&pts);
        assert!(nrm.z > 0.0, "fixture should be CCW about +Z");
        for t in polyfill_3d(&pts, nrm) {
            let (a, b, c) = (pts[t[0]], pts[t[1]], pts[t[2]]);
            let tn = b.sub(a).cross(c.sub(a));
            assert!(
                tn.dot(nrm) > 0.0,
                "triangle {t:?} is flipped against the face normal"
            );
        }
    }

    /// Reversing the input ring must reverse every output triangle, not
    /// change which triangles are chosen in an unrelated way.
    #[test]
    fn clockwise_input_is_handled_as_well_as_counter_clockwise() {
        let ccw = [
            [0.0, 0.0],
            [1.0, 0.0],
            [1.0, 0.5],
            [0.5, 0.5],
            [0.5, 1.0],
            [0.0, 1.0],
        ];
        let mut cw = ccw;
        cw.reverse();
        let a = tris_area(&ccw, &polyfill_2d(&ccw));
        let b = tris_area(&cw, &polyfill_2d(&cw));
        assert!((a - b).abs() < 1e-12, "{a} vs {b}");
        assert!((a - 0.75).abs() < 1e-12);
    }

    /// A deeply concave comb — the case a fan mangles worst.
    #[test]
    fn comb_polygon_tiles_exactly() {
        // A rectangle with three square notches cut into its top edge.
        let mut p: Vec<[f64; 2]> = vec![[0.0, 0.0], [6.0, 0.0], [6.0, 2.0]];
        for i in 0..3 {
            let x = 5.0 - 2.0 * i as f64;
            p.push([x, 2.0]);
            p.push([x, 1.0]);
            p.push([x - 1.0, 1.0]);
            p.push([x - 1.0, 2.0]);
        }
        p.push([0.0, 2.0]);
        let truth = poly_area(&p);
        let tris = polyfill_2d(&p);
        assert_eq!(tris.len(), p.len() - 2);
        let got = tris_area(&p, &tris);
        assert!((got - truth).abs() < 1e-10, "got {got}, want {truth}");
    }

    /// Indices must be used consistently — upstream guarantees the triangles
    /// reference only real corners and each triangle is non-degenerate in
    /// index terms.
    #[test]
    fn emitted_indices_are_in_range_and_distinct_within_a_triangle() {
        let n = 17;
        let coords: Vec<[f64; 2]> = (0..n)
            .map(|i| {
                let t = std::f64::consts::TAU * (i as f64) / (n as f64);
                let r = if i % 3 == 0 { 0.4 } else { 1.0 };
                [r * t.cos(), r * t.sin()]
            })
            .collect();
        for t in polyfill_2d(&coords) {
            assert!(t.iter().all(|&i| i < n));
            assert!(t[0] != t[1] && t[1] != t[2] && t[0] != t[2], "{t:?}");
        }
    }

    #[test]
    fn degenerate_inputs_do_not_panic() {
        assert!(polyfill_2d(&[]).is_empty());
        assert!(polyfill_2d(&[[0.0, 0.0]]).is_empty());
        assert!(polyfill_2d(&[[0.0, 0.0], [1.0, 1.0]]).is_empty());
        // All corners collinear — degenerate, but must still emit n - 2.
        let line: Vec<[f64; 2]> = (0..6).map(|i| [i as f64, 0.0]).collect();
        assert_eq!(polyfill_2d(&line).len(), 4);
    }

    #[test]
    fn ortho_basis_is_right_handed_for_many_normals() {
        let dirs = [
            Vec3::new(0.0, 0.0, 1.0),
            Vec3::new(0.0, 0.0, -1.0),
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::new(0.0, 1.0, 0.0),
            Vec3::new(1.0, 2.0, 3.0).normalize(),
            Vec3::new(-3.0, 0.5, -2.0).normalize(),
        ];
        for n in dirs {
            let (a, b) = ortho_basis(n);
            let c = a.cross(b);
            assert!(c.sub(n).length() < 1e-12, "n = {n:?} gave {c:?}");
            assert!((a.length() - 1.0).abs() < 1e-12);
            assert!((b.length() - 1.0).abs() < 1e-12);
        }
    }

    /// A 3-D concave face tiles to its true area under the projection too.
    #[test]
    fn polyfill_3d_tiles_a_tilted_concave_face() {
        // The same L, lifted onto the plane z = x + y so it is not axis-aligned.
        let pts: Vec<Vec3> = [
            (0.0, 0.0),
            (1.0, 0.0),
            (1.0, 0.5),
            (0.5, 0.5),
            (0.5, 1.0),
            (0.0, 1.0),
        ]
        .iter()
        .map(|&(x, y)| Vec3::new(x, y, x + y))
        .collect();
        let nrm = newell_normal(&pts);
        let tris = polyfill_3d(&pts, nrm);
        assert_eq!(tris.len(), 4);

        // Area of the tilted L: the plane z = x + y scales area by
        // sqrt(1 + 1 + 1) = sqrt(3) relative to its XY shadow.
        let truth = 0.75 * 3.0_f64.sqrt();
        let got: f64 = tris
            .iter()
            .map(|t| {
                let (a, b, c) = (pts[t[0]], pts[t[1]], pts[t[2]]);
                0.5 * b.sub(a).cross(c.sub(a)).length()
            })
            .sum();
        assert!((got - truth).abs() < 1e-12, "got {got}, want {truth}");
    }
}
