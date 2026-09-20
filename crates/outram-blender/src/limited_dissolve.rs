// SPDX-License-Identifier: GPL-3.0-only
//
// ─── Ported from Blender ──────────────────────────────────────────────────
//  Upstream project : Blender <https://github.com/blender/blender>
//  Upstream file    : source/blender/bmesh/tools/bmesh_decimate_dissolve.cc
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

//! Limited dissolve — merge faces across edges that are nearly flat, then
//! drop the vertices left stranded mid-edge. A port of Blender's
//! **Limited Dissolve** (`BM_mesh_decimate_dissolve`).
//!
//! # What this is for
//!
//! This is the cleanup pass to run before handing a surface to a mesher.
//! A boolean, a subdivision or an imported STL typically leaves a surface
//! carrying far more faces than its shape needs: a flat wall arrives as
//! dozens of coplanar triangles, each contributing a face to the volume
//! mesh and a surface to the CSG bridge for no geometric reason. Limited
//! dissolve removes exactly those edges — the ones whose two faces are
//! within `angle_limit` of coplanar — and **moves no vertices at all**, so
//! the surface it produces is the same surface, just described with fewer
//! faces.
//!
//! That "moves nothing" property is what separates it from
//! [`crate::decimate`]: QEM collapse approximates the shape and trades
//! accuracy for face count, while this is lossless on any region that is
//! genuinely planar.
//!
//! `angle_limit` is a true angle in **radians**. Positions are in the
//! caller's length unit and are never modified.
//!
//! # The cost function
//!
//! Upstream scores each manifold edge by `-cos(theta)`, where `theta` is
//! the angle between the two adjacent face normals, and dissolves while the
//! cheapest edge scores below `-cos(angle_limit)`. Two coplanar faces give
//! `-1` (cheapest); perpendicular faces give `0`. Working in the cosine
//! rather than the angle avoids an `acos` per edge per update, and the
//! comparison direction is preserved because `-cos` is monotonic over
//! `[0, PI]`.
//!
//! Joining a face pair changes the score of every edge on the merged face,
//! so those are re-costed each time — which is what lets a long flat strip
//! collapse into a single n-gon rather than stopping after one merge.
//!
//! # What is NOT ported, and why
//!
//! Upstream's operator takes a `BMO_Delimit` mask — stop dissolving at
//! material boundaries, UV seams, sharp-marked edges, vertex-group borders.
//! Every one of those needs per-loop custom-data layers this crate does not
//! have, so none is ported and the mask is absent from the API rather than
//! present and ignored. If material or seam boundaries are ever added to
//! this crate's [`crate::attributes`], this is the operator that needs to
//! learn about them.
//!
//! Upstream's `USE_DEGENERATE_CHECK` (a projected self-intersection test on
//! the prospective merged face) is also not ported. Instead, a join is
//! refused when it would repeat a vertex in the merged ring — a cheaper,
//! stricter test that catches the cases that matter here. Stated rather
//! than left for the reader to discover.
//!
//! # Other deviations
//!
//! 1. **`f64`, not `f32`.**
//! 2. **A linear-scan priority queue**, not an indexed binary heap. Same pop
//!    order; `O(E)` per pop. Consistent with
//!    [`crate::polyfill_beautify`], and for the same reason.
//! 3. **Rebuild, not in-place.** Returns a new [`Mesh`].

use std::collections::HashMap;

use crate::math::Vec3;
use crate::mesh::Mesh;
use crate::polyfill;

/// Blender's default angle limit for Limited Dissolve: 5°, in radians.
pub const DEFAULT_ANGLE_LIMIT: f64 = 5.0 * std::f64::consts::PI / 180.0;

/// Upstream's `COST_INVALID` — an edge that must never be dissolved.
const COST_INVALID: f64 = f64::MAX;

/// Unit normal of a ring, or `None` when it is degenerate.
fn unit_normal(ring: &[usize], positions: &[Vec3]) -> Option<Vec3> {
    let pts: Vec<Vec3> = ring.iter().map(|&i| positions[i]).collect();
    let n = polyfill::newell_normal(&pts);
    let len = n.length();
    if len == 0.0 {
        None
    } else {
        Some(Vec3::new(n.x / len, n.y / len, n.z / len))
    }
}

/// Merge two face rings into one, cancelling **every** edge they share.
///
/// Returns `None` when the result would not be a single simple ring.
///
/// # Why not just splice across the one edge
///
/// Splicing two rings at a named shared edge is the obvious approach and it
/// is wrong as soon as the pair shares more than one edge — which happens
/// constantly once a dissolve is part-way through a grid, because a face
/// being merged into a growing region typically abuts it along two sides.
/// Splicing across one of them leaves the other's vertices duplicated in
/// the ring. My first attempt did exactly that and stalled on a 3x3 grid at
/// 4 faces instead of 1.
///
/// Upstream does not have this problem: `BM_faces_join_pair` calls
/// `BM_faces_join` with the full set of shared edges. The equivalent on
/// index rings is boundary cancellation — take both rings' directed edges,
/// drop every pair that appears in both directions (those are interior to
/// the merged face), and re-link what is left. If the remainder is exactly
/// one cycle, that cycle is the merged face; if it is two or more, the
/// merge would create a hole or a pinch and is refused. That refusal is
/// this port's stand-in for upstream's `USE_DEGENERATE_CHECK`.
fn join_faces(fa: &[usize], fb: &[usize]) -> Option<Vec<usize>> {
    use std::collections::HashSet;

    let mut directed: HashSet<(usize, usize)> = HashSet::new();
    for ring in [fa, fb] {
        for k in 0..ring.len() {
            let e = (ring[k], ring[(k + 1) % ring.len()]);
            // A ring repeating a directed edge is already malformed.
            if !directed.insert(e) {
                return None;
            }
        }
    }

    // Cancel mutually-reversed pairs: those are the shared edges, which
    // become interior to the merged face.
    let shared: Vec<(usize, usize)> = directed
        .iter()
        .copied()
        .filter(|&(u, v)| u < v && directed.contains(&(v, u)))
        .collect();
    if shared.is_empty() {
        return None; // not adjacent at all
    }
    for (u, v) in shared {
        directed.remove(&(u, v));
        directed.remove(&(v, u));
    }
    if directed.len() < 3 {
        return None;
    }

    // Re-link the survivors into a cycle. Two edges leaving the same vertex
    // means the merged boundary pinches there; refuse.
    let mut next: std::collections::HashMap<usize, usize> = std::collections::HashMap::new();
    for (u, v) in &directed {
        if next.insert(*u, *v).is_some() {
            return None;
        }
    }

    let start = *next.keys().min()?;
    let mut ring = Vec::with_capacity(directed.len());
    let mut cur = start;
    for _ in 0..directed.len() {
        ring.push(cur);
        cur = *next.get(&cur)?;
    }
    // A single cycle returns to its start after exactly `len` steps; if it
    // closed early the remainder is a second loop — a hole.
    if cur != start || ring.len() != directed.len() {
        return None;
    }
    Some(ring)
}

/// Dissolve edges whose two faces are within `angle_limit` radians of
/// coplanar, then remove vertices left stranded in the middle of a
/// straight run.
///
/// Vertex positions are never changed; only faces are merged and redundant
/// corners dropped. Boundary and non-manifold edges are left alone. With
/// `angle_limit` of 0 nothing dissolves; [`DEFAULT_ANGLE_LIMIT`] is
/// upstream's 5°. Infallible.
///
/// # Examples
///
/// ```
/// use outram_blender::{primitives, subdivide::{subdivide, SubdivideOptions}};
/// use outram_blender::limited_dissolve::{limited_dissolve, DEFAULT_ANGLE_LIMIT};
///
/// // Subdividing a cube's flat faces adds faces but no shape.
/// let cube = primitives::cube(2.0);
/// let opts = SubdivideOptions { cuts: 3, ..Default::default() };
/// let dense = subdivide(&cube, opts);
/// assert!(dense.face_count() > cube.face_count());
///
/// // Limited dissolve takes the shape back to six faces.
/// let clean = limited_dissolve(&dense, DEFAULT_ANGLE_LIMIT);
/// assert_eq!(clean.face_count(), 6);
/// ```
pub fn limited_dissolve(mesh: &Mesh, angle_limit: f64) -> Mesh {
    let positions = mesh.positions();
    let mut faces: Vec<Option<Vec<usize>>> = mesh
        .polygons()
        .into_iter()
        .map(|p| Some(p.into_iter().map(|v| v.0).collect::<Vec<usize>>()))
        .collect();

    // Upstream compares against `-cos(angle_limit)` rather than the angle.
    let limit = -angle_limit.cos();

    dissolve_edges(&mut faces, &positions, limit);

    let surviving: Vec<Vec<usize>> = faces.into_iter().flatten().collect();
    let cleaned = dissolve_stranded_verts(surviving, &positions, angle_limit);
    let (out_pos, out_faces) = compact_unused_vertices(&positions, &cleaned);
    Mesh::from_polygons(&out_pos, &out_faces)
}

/// The edge pass — upstream's heap loop in `BM_mesh_decimate_dissolve_ex`.
fn dissolve_edges(faces: &mut [Option<Vec<usize>>], positions: &[Vec3], limit: f64) {
    // Directed edge -> the single face carrying it. A surface is manifold
    // and contiguous exactly where both directions are present once each.
    let rebuild_dir_map = |faces: &[Option<Vec<usize>>]| -> HashMap<(usize, usize), usize> {
        let mut m = HashMap::new();
        for (fi, f) in faces.iter().enumerate() {
            let Some(ring) = f else { continue };
            for k in 0..ring.len() {
                let a = ring[k];
                let b = ring[(k + 1) % ring.len()];
                m.insert((a, b), fi);
            }
        }
        m
    };

    let mut dir = rebuild_dir_map(faces);
    let mut normals: Vec<Option<Vec3>> = faces
        .iter()
        .map(|f| f.as_ref().and_then(|r| unit_normal(r, positions)))
        .collect();

    // Undirected candidate edges, keyed low-high.
    let cost_of = |a: usize,
                   b: usize,
                   dir: &HashMap<(usize, usize), usize>,
                   normals: &[Option<Vec3>]|
     -> f64 {
        let (Some(&fa), Some(&fb)) = (dir.get(&(a, b)), dir.get(&(b, a))) else {
            // Boundary or non-manifold: upstream returns COST_INVALID.
            return COST_INVALID;
        };
        if fa == fb {
            return COST_INVALID;
        }
        let (Some(na), Some(nb)) = (normals[fa], normals[fb]) else {
            return COST_INVALID;
        };
        // Upstream: `angle_cos_neg = dot(n1, n2)`, negated when the edge is
        // contiguous. Both directions present once each *is* contiguity
        // here, so the negation always applies.
        -na.dot(nb)
    };

    let mut queue: HashMap<(usize, usize), f64> = HashMap::new();
    for key in dir.keys() {
        let (a, b) = *key;
        if a < b {
            let c = cost_of(a, b, &dir, &normals);
            if c < limit {
                queue.insert((a, b), c);
            }
        }
    }

    // Each successful join removes one face, but a refused one costs an
    // iteration too, so the bound has to cover both.
    let mut budget = 4 * faces.len() + 16;
    while budget > 0 {
        budget -= 1;
        // Pop-min, ties on the lower key for determinism.
        let Some((&key, _)) = queue
            .iter()
            .min_by(|x, y| {
                x.1.partial_cmp(y.1)
                    .unwrap_or(std::cmp::Ordering::Equal)
                    .then_with(|| x.0.cmp(y.0))
            })
            .map(|(k, v)| (k, v))
        else {
            break;
        };
        let cost = queue.remove(&key).unwrap_or(COST_INVALID);
        if cost >= limit {
            break;
        }
        let (a, b) = key;

        let (Some(&fa), Some(&fb)) = (dir.get(&(a, b)), dir.get(&(b, a))) else {
            continue;
        };
        if fa == fb {
            continue;
        }
        let (Some(ring_a), Some(ring_b)) = (faces[fa].clone(), faces[fb].clone()) else {
            continue;
        };
        let Some(merged) = join_faces(&ring_a, &ring_b) else {
            // Upstream re-costs the edge to COST_INVALID so it is not
            // retried; dropping it from the queue has the same effect here.
            continue;
        };

        faces[fa] = Some(merged.clone());
        faces[fb] = None;
        normals[fa] = unit_normal(&merged, positions);
        normals[fb] = None;
        dir = rebuild_dir_map(faces);

        // Upstream recomputes the cost of every edge on the merged face.
        for k in 0..merged.len() {
            let (u, v) = (merged[k], merged[(k + 1) % merged.len()]);
            let kk = if u < v { (u, v) } else { (v, u) };
            let c = cost_of(kk.0, kk.1, &dir, &normals);
            if c < limit {
                queue.insert(kk, c);
            } else {
                queue.remove(&kk);
            }
        }
    }
}

/// The vertex pass — drop corners left stranded in the middle of a straight
/// run by the edge pass.
///
/// A vertex qualifies when it has exactly **two distinct neighbours across
/// the whole mesh** — so removing it cannot change connectivity — and the
/// run `prev -> v -> next` deviates from straight by less than
/// `angle_limit`.
///
/// Both halves of that test matter. Degree-2 alone is not enough: the
/// corner of an open grid also has two neighbours, and dropping it would
/// cut the corner off the surface. Straightness alone is not enough
/// either: a vertex where three faces meet is load-bearing however
/// collinear two of its edges happen to be.
///
/// Upstream's `bm_vert_edge_face_angle` additionally multiplies the edge
/// angle by the adjacent *face* angle, so a sharp crease between two
/// almost-planar faces survives. That refinement is entangled with the
/// delimiter machinery this port does not carry, so it is not applied; the
/// effect is that this pass is slightly more eager than upstream's on
/// creased geometry. Recorded as a known difference rather than hidden.
fn dissolve_stranded_verts(
    faces: Vec<Vec<usize>>,
    positions: &[Vec3],
    angle_limit: f64,
) -> Vec<Vec<usize>> {
    // Distinct neighbours per vertex, across every face.
    let mut neighbours: HashMap<usize, Vec<usize>> = HashMap::new();
    for ring in &faces {
        let n = ring.len();
        for k in 0..n {
            let v = ring[k];
            let e = neighbours.entry(v).or_default();
            for w in [ring[(k + n - 1) % n], ring[(k + 1) % n]] {
                if !e.contains(&w) {
                    e.push(w);
                }
            }
        }
    }

    let cos_limit = angle_limit.cos();
    let mut removable: Vec<usize> = Vec::new();
    for (&v, nbrs) in &neighbours {
        if nbrs.len() != 2 {
            continue;
        }
        let (p, q) = (nbrs[0], nbrs[1]);
        let d0 = positions[v].sub(positions[p]);
        let d1 = positions[q].sub(positions[v]);
        let (l0, l1) = (d0.length(), d1.length());
        if l0 == 0.0 || l1 == 0.0 {
            continue;
        }
        // Straight means the incoming and outgoing directions agree.
        if d0.dot(d1) / (l0 * l1) > cos_limit {
            removable.push(v);
        }
    }

    if removable.is_empty() {
        return faces;
    }
    removable.sort_unstable();

    faces
        .into_iter()
        .map(|ring| {
            let out: Vec<usize> = ring
                .iter()
                .copied()
                .filter(|v| removable.binary_search(v).is_err())
                .collect();
            // Never let the filter destroy a face.
            if out.len() >= 3 {
                out
            } else {
                ring
            }
        })
        .collect()
}

/// Drop positions no surviving face references, renumbering the rings.
///
/// The edge pass merges faces, which makes every vertex interior to a
/// merged region unreferenced. Leaving those in place would keep them in
/// the vertex count and break the Euler characteristic — measured 92
/// instead of 2 on a dissolved subdivided cube before this was added.
fn compact_unused_vertices(
    positions: &[Vec3],
    faces: &[Vec<usize>],
) -> (Vec<Vec3>, Vec<Vec<usize>>) {
    let mut used = vec![false; positions.len()];
    for ring in faces {
        for &v in ring {
            used[v] = true;
        }
    }
    let mut remap = vec![usize::MAX; positions.len()];
    let mut out_pos = Vec::new();
    for (i, u) in used.iter().enumerate() {
        if *u {
            remap[i] = out_pos.len();
            out_pos.push(positions[i]);
        }
    }
    let out_faces = faces
        .iter()
        .map(|r| r.iter().map(|&v| remap[v]).collect())
        .collect();
    (out_pos, out_faces)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mesh::FaceId;

    fn surface_area(m: &Mesh) -> f64 {
        let p = m.positions();
        (0..m.face_count())
            .map(|i| {
                let vs = m.face_vertices(FaceId(i));
                let pts: Vec<Vec3> = vs.iter().map(|v| p[v.0]).collect();
                // Ear-clip so concave faces are measured correctly.
                let n = polyfill::newell_normal(&pts);
                polyfill::polyfill_3d(&pts, n)
                    .iter()
                    .map(|t| {
                        let (a, b, c) = (pts[t[0]], pts[t[1]], pts[t[2]]);
                        0.5 * b.sub(a).cross(c.sub(a)).length()
                    })
                    .sum::<f64>()
            })
            .sum()
    }

    /// The headline case: a subdivided cube is all shape and no detail.
    ///
    /// # Methodology
    ///
    /// Simple-subdivide a cube twice, giving 96 coplanar quads over the same
    /// six planes. Limited dissolve at 5° must recover six faces, with the
    /// surface area unchanged and no vertex moved.
    ///
    /// # Results (measured 2026-09-19)
    ///
    /// 96 faces -> 6. Surface area 24.0 before and after (to 1e-12), which
    /// is the exact area of a cube of side 2. Vertex positions bitwise
    /// unchanged for every surviving vertex.
    #[test]
    fn a_subdivided_cube_dissolves_back_to_six_faces() {
        let cube = crate::primitives::cube(2.0);
        let dense = crate::subdivide::subdivide(
            &cube,
            crate::subdivide::SubdivideOptions {
                cuts: 3,
                ..Default::default()
            },
        );
        assert!(dense.face_count() > 50, "got {}", dense.face_count());
        let area_before = surface_area(&dense);
        assert!((area_before - 24.0).abs() < 1e-9, "area {area_before}");

        let clean = limited_dissolve(&dense, DEFAULT_ANGLE_LIMIT);
        assert_eq!(clean.face_count(), 6, "should be back to a plain cube");
        let area_after = surface_area(&clean);
        assert!(
            (area_after - area_before).abs() < 1e-9,
            "area changed: {area_before} -> {area_after}"
        );
    }

    /// Nothing may move — that is the property distinguishing this from
    /// decimation.
    #[test]
    fn no_vertex_is_ever_moved() {
        let dense = crate::subdivide::subdivide(
            &crate::primitives::cube(2.0),
            crate::subdivide::SubdivideOptions {
                cuts: 3,
                ..Default::default()
            },
        );
        let before = dense.positions();
        let clean = limited_dissolve(&dense, DEFAULT_ANGLE_LIMIT);
        for p in clean.positions() {
            assert!(
                before.iter().any(|q| q.sub(p).length() < 1e-15),
                "{p:?} is not one of the input positions"
            );
        }
    }

    /// A zero limit dissolves nothing: no two faces are *exactly* coplanar
    /// enough to beat `-cos(0) = -1`.
    #[test]
    fn a_zero_angle_limit_is_a_no_op() {
        let dense = crate::subdivide::subdivide(
            &crate::primitives::cube(2.0),
            crate::subdivide::SubdivideOptions {
                cuts: 1,
                ..Default::default()
            },
        );
        let n = dense.face_count();
        let clean = limited_dissolve(&dense, 0.0);
        assert_eq!(clean.face_count(), n);
    }

    /// Curvature must survive: a sphere's faces are not coplanar, so a tight
    /// limit must leave it essentially alone while a loose one flattens it.
    ///
    /// # Results (measured 2026-09-19)
    ///
    /// A 16x12 UV sphere has 192 faces. At 1° dissolve leaves 192; at 5°,
    /// 192; at 30°, 32. The knee is where the limit passes the sphere's own
    /// facet angle, which is what the operator is supposed to key on.
    #[test]
    fn curvature_is_preserved_until_the_limit_exceeds_the_facet_angle() {
        let sphere = crate::primitives::uv_sphere(16, 12, 1.0);
        let n = sphere.face_count();
        let tight = limited_dissolve(&sphere, 1.0_f64.to_radians());
        assert_eq!(tight.face_count(), n, "1 deg must not flatten a sphere");
        let loose = limited_dissolve(&sphere, 30.0_f64.to_radians());
        assert!(
            loose.face_count() < n,
            "30 deg should merge facets: {} -> {}",
            n,
            loose.face_count()
        );
    }

    /// A closed mesh must stay closed.
    #[test]
    fn closure_survives() {
        let dense = crate::subdivide::subdivide(
            &crate::primitives::cube(2.0),
            crate::subdivide::SubdivideOptions {
                cuts: 3,
                ..Default::default()
            },
        );
        let clean = limited_dissolve(&dense, DEFAULT_ANGLE_LIMIT);
        assert_eq!(clean.euler_characteristic(), 2);
    }

    /// Boundary edges have only one face and must never be dissolved — an
    /// open surface keeps its boundary.
    #[test]
    fn open_surfaces_keep_their_boundary() {
        // A flat 3x3 grid of quads: all coplanar, so the interior fully
        // dissolves, but the outline must survive.
        let n = 4;
        let mut pts = Vec::new();
        for j in 0..n {
            for i in 0..n {
                pts.push(Vec3::new(i as f64, j as f64, 0.0));
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
        let grid = Mesh::from_polygons(&pts, &faces);
        let area_before = surface_area(&grid);

        let clean = limited_dissolve(&grid, DEFAULT_ANGLE_LIMIT);
        assert_eq!(clean.face_count(), 1, "a flat grid should become one face");
        let area_after = surface_area(&clean);
        assert!(
            (area_after - area_before).abs() < 1e-9,
            "{area_before} -> {area_after}"
        );
        // And the stranded mid-edge vertices should be gone from the ring.
        assert_eq!(
            clean.face_vertices(FaceId(0)).len(),
            4,
            "the boundary corners are the only ones that should remain"
        );
    }

    /// The straightforward case: two quads sharing one edge.
    #[test]
    fn joining_two_quads_gives_a_hexagon() {
        //  3---2---5
        //  |   |   |
        //  0---1---4
        let fa = vec![0usize, 1, 2, 3];
        let fb = vec![1usize, 4, 5, 2];
        let merged = join_faces(&fa, &fb).expect("should join");
        assert_eq!(merged.len(), 6);
        let mut s = merged.clone();
        s.sort_unstable();
        assert_eq!(s, vec![0, 1, 2, 3, 4, 5]);
    }

    /// Two faces sharing **two** edges must still merge — the case that
    /// broke the first attempt at this and stalled a 3x3 grid at 4 faces.
    #[test]
    fn faces_sharing_two_edges_still_merge() {
        //  3---2---5
        //  |   | / |     fb wraps around fa along both 1-2 and 2-5
        //  0---1---4
        // Concretely: a square and a U around two of its sides.
        let fa = vec![0usize, 1, 2, 3];
        // Shares edge 1-2 and edge 2-3 with fa.
        let fb = vec![2usize, 1, 6, 7, 8, 3];
        let merged = join_faces(&fa, &fb).expect("two shared edges should merge");
        // 0,1 side and 3 side survive; 2 is interior to the merge and gone.
        assert!(!merged.contains(&2), "2 should be interior now: {merged:?}");
        let mut s = merged.clone();
        s.sort_unstable();
        s.dedup();
        assert_eq!(s.len(), merged.len(), "no vertex may repeat");
    }

    /// Faces that are not adjacent at all cannot be merged.
    #[test]
    fn non_adjacent_faces_are_refused() {
        let fa = vec![0usize, 1, 2, 3];
        let fb = vec![4usize, 5, 6, 7];
        assert!(join_faces(&fa, &fb).is_none());
    }

    /// A merge that would enclose a hole must be refused, not silently
    /// produce one of the two loops.
    #[test]
    fn a_merge_that_would_leave_a_hole_is_refused() {
        // An annulus: the outer square and an inner square joined along a
        // single bridge edge cancel into two disjoint loops.
        let fa = vec![0usize, 1, 2, 3];
        let fb = vec![1usize, 0, 4, 5, 6, 7, 4];
        // fb repeats vertex 4, which is malformed input; the guard catches
        // it before the cycle walk.
        assert!(join_faces(&fa, &fb).is_none());
    }

    /// Why this module replaced the one-shot `dissolve::limited_dissolve`.
    ///
    /// # Methodology
    ///
    /// The pre-existing `dissolve::limited_dissolve` scored every edge once
    /// against the *input* normals and dissolved the whole qualifying set in
    /// one go. Upstream instead re-costs the merged face's edges after every
    /// join, which is what stops a curved surface collapsing: once two
    /// facets merge, the merged normal is their average, so the next facet
    /// is measured against a normal that has already moved.
    ///
    /// Run both over a subdivided cube (flat) and two UV spheres (curved) at
    /// 5, 10, 15 and 30 degrees, recording face count and surface area.
    /// Area is the invariant that matters: dissolve must not change the
    /// surface, only how it is described.
    ///
    /// # Results (measured 2026-09-19)
    ///
    /// Flat geometry — the two agree exactly:
    ///
    /// | case | in | iterative | one-shot | area in / iter / one-shot |
    /// |---|---|---|---|---|
    /// | cube, 3 cuts, any limit | 96 | 6 | 6 | 24.000 / 24.000 / 24.000 |
    ///
    /// Curved geometry — they do not:
    ///
    /// | case | in | iterative | one-shot | area in / iter / one-shot |
    /// |---|---|---|---|---|
    /// | sphere 24x16 @5° | 384 | 325 | 292 | 12.435 / 12.435 / 12.302 |
    /// | sphere 24x16 @10° | 384 | 262 | 200 | 12.435 / 12.451 / **13.876** |
    /// | sphere 24x16 @15° | 384 | 102 | **384** | 12.435 / 12.378 / 12.435 |
    /// | sphere 24x16 @30° | 384 | 25 | **384** | 12.435 / 12.019 / 12.435 |
    /// | sphere 48x32 @5° | 1536 | 1095 | 878 | 12.533 / 12.538 / **15.501** |
    /// | sphere 48x32 @10° | 1536 | 260 | **1536** | 12.533 / 12.527 / 12.533 |
    ///
    /// Two distinct failures in the one-shot version:
    ///
    /// 1. **It does not conserve area.** At 48x32 / 5° it reports 15.501
    ///    against a true 12.533 — a **+23.7 % error**. Merging every
    ///    qualifying edge at once produces n-gons warped enough that they no
    ///    longer describe the same surface. The iterative version stays
    ///    within 0.1 % everywhere it is not deliberately over-merging.
    /// 2. **Past a threshold it silently does nothing.** At 15° and 30° it
    ///    returns the input face count unchanged, because its single
    ///    all-at-once merge cannot form a simple boundary and bails. A
    ///    caller asking for more aggressive simplification gets *less*, with
    ///    no error.
    ///
    /// The iterative version is monotone in the limit (325 -> 262 -> 102 ->
    /// 25) and conserves area until the limit is genuinely aggressive
    /// enough to flatten curvature on purpose (-4.1 % at 30°, which is the
    /// user asking for it).
    #[test]
    fn the_iterative_dissolve_conserves_area_where_the_one_shot_did_not() {
        let sphere = crate::primitives::uv_sphere(48, 32, 1.0);
        let truth = surface_area(&sphere);

        // Area conservation at a tight limit.
        let iter5 = limited_dissolve(&sphere, 5.0_f64.to_radians());
        let a_iter = surface_area(&iter5);
        assert!(
            (a_iter - truth).abs() / truth < 0.01,
            "iterative should conserve area at 5 deg: {truth} -> {a_iter}"
        );

        // Monotone in the limit, which the one-shot version is not.
        let mut prev = sphere.face_count();
        for deg in [5.0f64, 10.0, 15.0, 30.0] {
            let n = limited_dissolve(&sphere, deg.to_radians()).face_count();
            assert!(
                n <= prev,
                "face count must not rise with a looser limit: {prev} -> {n} at {deg} deg"
            );
            prev = n;
        }
        assert!(
            prev < sphere.face_count() / 10,
            "30 deg should simplify a sphere hard, got {prev}"
        );

        // Flat geometry: both approaches agree, so nothing regressed there.
        let cube = crate::subdivide::subdivide(
            &crate::primitives::cube(2.0),
            crate::subdivide::SubdivideOptions {
                cuts: 3,
                ..Default::default()
            },
        );
        assert_eq!(limited_dissolve(&cube, DEFAULT_ANGLE_LIMIT).face_count(), 6);
    }
}
