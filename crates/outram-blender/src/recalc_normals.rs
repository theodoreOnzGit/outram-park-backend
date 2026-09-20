// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Breadth-first winding propagation across the face-adjacency graph, then an
// outward flip per connected component by signed-volume sign (divergence theorem).
// No named published algorithm — written from first principles; no upstream source
// was copied. Blender analogue (architecture only): Recalculate Normals Outside
// (mesh.normals_make_consistent).
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

//! Recalculate normals — make a mesh's face winding globally consistent and
//! outward-facing.
//!
//! This is the pure-Rust analogue of Blender's **Recalculate Normals Outside**
//! (`mesh.normals_make_consistent`). It repairs a polygon soup whose faces are
//! wound inconsistently — the common state of an imported or hand-assembled
//! mesh — so that adjacent faces agree on their shared edges and every
//! connected component's normals point outward.
//!
//! # Why this is not just [`crate::boolean_general`]'s orient-outward
//!
//! `boolean_general` flips a whole mesh by signed-volume sign, but only when it
//! is **already** consistently wound. This operator does the harder job first:
//! a breadth-first propagation across the face-adjacency graph that flips any
//! neighbour disagreeing on a shared edge, turning an inconsistent soup into a
//! consistently-wound one. Only then is each connected component flipped
//! outward.
//!
//! # Algorithm
//!
//! 1. **Adjacency.** Index faces by their undirected edges (sorted vertex
//!    pair). Two faces sharing an edge are adjacent.
//! 2. **Orientation propagation (BFS).** Seed each connected component with one
//!    face (unflipped). For a neighbour across a shared edge, the two faces are
//!    consistent iff they traverse that edge in **opposite** directions; if not,
//!    the neighbour is marked to flip. Directions compose by XOR, so the
//!    neighbour's flip is a closed-form function of the seed-relative
//!    orientation.
//! 3. **Outward pass.** For each connected component, compute the signed volume
//!    (divergence theorem) of its now-consistent faces; if negative, flip the
//!    whole component so its normals point outward.
//!
//! A non-orientable surface (e.g. a Möbius band) cannot be made globally
//! consistent — the BFS will visit it, but the result is best-effort along the
//! spanning tree, matching Blender's own behaviour. No `faer`, no external
//! dependency; Android-safe.

use crate::math::Vec3;
use crate::mesh::Mesh;
use std::collections::HashMap;

/// Return `mesh` with every face rewound so the surface is consistently wound
/// and each connected component faces outward (positive enclosed volume).
///
/// This is infallible. An already-correct mesh is returned with the same
/// winding; an inconsistently-wound soup is repaired; a fully-inward mesh is
/// flipped outward. Winding is the only thing changed — vertex positions and
/// the face-vertex sets are untouched.
///
/// # Examples
///
/// ```
/// use outram_blender::{primitives, recalc_normals::recalculate_normals};
///
/// // A pristine cube is already outward-wound, so this is a no-op on topology.
/// let cube = primitives::cube(2.0);
/// let fixed = recalculate_normals(&cube);
/// assert_eq!(fixed.vertex_count(), 8);
/// assert_eq!(fixed.face_count(), 6);
/// assert_eq!(fixed.euler_characteristic(), 2);
/// ```
pub fn recalculate_normals(mesh: &Mesh) -> Mesh {
    let positions = mesh.positions();
    let polygons: Vec<Vec<usize>> = mesh
        .polygons()
        .iter()
        .map(|p| p.iter().map(|v| v.0).collect())
        .collect();
    let nf = polygons.len();
    if nf == 0 {
        return Mesh::new();
    }

    // undirected edge (lo, hi) -> list of (face, orig_ab) where orig_ab is true
    // when the face traverses the edge lo -> hi in its original winding.
    let mut edge_faces: HashMap<(usize, usize), Vec<(usize, bool)>> = HashMap::new();
    for (f, poly) in polygons.iter().enumerate() {
        let k = poly.len();
        for i in 0..k {
            let a = poly[i];
            let b = poly[(i + 1) % k];
            let (lo, hi, ab) = if a < b { (a, b, true) } else { (b, a, false) };
            edge_faces.entry((lo, hi)).or_default().push((f, ab));
        }
    }

    // BFS orientation propagation. flip[f] = reverse this face's winding.
    let mut flip = vec![false; nf];
    let mut visited = vec![false; nf];
    // Per-face component id, for the outward pass.
    let mut component = vec![usize::MAX; nf];
    let mut components: Vec<Vec<usize>> = Vec::new();

    // orig_ab for a (face, edge): recover from edge_faces lookups on demand.
    let orig_ab_of = |f: usize, lo: usize, hi: usize| -> bool {
        edge_faces[&(lo, hi)]
            .iter()
            .find(|(g, _)| *g == f)
            .map(|(_, ab)| *ab)
            .unwrap()
    };

    for seed in 0..nf {
        if visited[seed] {
            continue;
        }
        let comp_id = components.len();
        let mut comp = Vec::new();
        visited[seed] = true;
        component[seed] = comp_id;
        let mut queue = std::collections::VecDeque::new();
        queue.push_back(seed);
        while let Some(f) = queue.pop_front() {
            comp.push(f);
            let poly = &polygons[f];
            let k = poly.len();
            for i in 0..k {
                let a = poly[i];
                let b = poly[(i + 1) % k];
                let (lo, hi) = if a < b { (a, b) } else { (b, a) };
                // final_ab(f) = orig_ab(f) XOR flip[f].
                let final_ab_f = orig_ab_of(f, lo, hi) ^ flip[f];
                for &(g, _) in &edge_faces[&(lo, hi)] {
                    if g == f || visited[g] {
                        continue;
                    }
                    // Want final_ab(g) = !final_ab(f); flip[g] = orig_ab(g) XOR that.
                    let orig_ab_g = orig_ab_of(g, lo, hi);
                    flip[g] = orig_ab_g ^ (!final_ab_f);
                    visited[g] = true;
                    component[g] = comp_id;
                    queue.push_back(g);
                }
            }
        }
        components.push(comp);
    }

    // Outward pass: per component, decide from the loop furthest out.
    //
    // This is upstream's criterion (`recalc_face_normals_find_index`,
    // bmo_normals.cc:64), ported after the signed-volume test that used to
    // live here was shown to be origin-dependent on open surfaces — see
    // `component_outward_needs_flip` and the regression test.
    for comp in &components {
        if component_outward_needs_flip(&positions, &polygons, comp, &flip) {
            for &f in comp {
                flip[f] = !flip[f];
            }
        }
    }

    // Rebuild with flips applied.
    let new_faces: Vec<Vec<usize>> = polygons
        .iter()
        .enumerate()
        .map(|(f, poly)| {
            if flip[f] {
                poly.iter().rev().copied().collect()
            } else {
                poly.clone()
            }
        })
        .collect();

    Mesh::from_polygons(&positions, &new_faces)
}

/// Does this connected component need flipping to face outward?
///
/// Port of the decision in upstream's `recalc_face_normals_find_index`
/// (`source/blender/bmesh/operators/bmo_normals.cc:64`, Blender commit
/// 786af64aad84154047d93ee077e1fdd1d229f32d, read 2026-09-20;
/// GPL-2.0-or-later, see the crate NOTICE).
///
/// # Why not signed volume
///
/// This used to sum `p0 . (v1 x v2)` over the component and flip when the
/// total came out negative. That is the divergence theorem, and for a
/// **closed** surface it gives the enclosed volume independently of where
/// the coordinate origin sits. For an **open** surface it does not: the sum
/// is the volume swept from the origin, so translating the same geometry
/// can change its sign and therefore the answer.
///
/// Measured on an open hemispherical bowl, correct normals in, at several
/// centres along Z:
///
/// | bowl centre z | outwardness after the old pass |
/// |---|---|
/// | 0, 2, 5, 20 | +0.9999 (correct) |
/// | -20 | **-0.9999 (inverted)** |
///
/// Same mesh, moved, opposite answer. Open surfaces are ordinary here — a
/// patch, a half-shell before `fill_holes` — so this was not a corner case.
///
/// # Upstream's criterion instead
///
/// Find the corner furthest from the component's **area-weighted centre**,
/// and ask whether the surface there faces away from that centre. Ties
/// break on, in order: the corner's most outward-pointing incident edge,
/// then the magnitude of the loop-normal's radial component. Upstream's
/// note on why the furthest point specifically: "Important this is
/// correctly detected, where casting a ray from the center won't hit any
/// loops past this one."
///
/// That is a local test on an extreme point, so it needs no enclosed
/// volume and is translation-invariant by construction — *provided* the
/// centre it measures from is the real centroid, which brings us to:
///
/// # A bug in upstream, reproduced here would defeat the purpose
///
/// Upstream builds that centre as
///
/// ```text
/// const float cent_fac = 1.0f / float(faces_len);
/// for each face:  cent += f_cent * (cent_fac * f_area);
///                 cent_area_accum += f_area;
/// cent *= 1.0f / cent_area_accum;
/// ```
///
/// which evaluates to `sum(f_cent * area) / (N * total_area)`. An
/// area-weighted centroid is `sum(f_cent * area) / total_area`. The
/// `cent_fac` is applied to the accumulator but never to the divisor, so
/// upstream's centre is **the true centroid divided by the face count** —
/// i.e. pulled most of the way back to the coordinate origin.
///
/// Checked against a cube of side 2 centred at `(10, 0, 0)` with 6 faces:
/// the true centroid is `(10, 0, 0)` and upstream's formula gives
/// `(1.667, 0, 0)`. A cube at the origin gives `(0, 0, 0)` either way,
/// which is why this is invisible in normal Blender use — a mesh is
/// usually near its own object origin in local space.
///
/// It is **not** invisible here. This crate's meshes are frequently
/// authored in world coordinates well away from the origin, and with the
/// `1/N` in place the ported criterion stayed origin-dependent and the
/// bowl-at-z=-20 case kept coming out inverted.
///
/// So the `1/N` is dropped. That is a divergence from upstream and it is
/// stated here rather than quietly absorbed; everything else in this
/// function is transcribed. Worth reporting to Blender, which is a
/// maintainer's call, not an agent's.
///
/// # Results (measured 2026-09-20)
///
/// An open hemispherical bowl, correctly wound, at several centres:
///
/// | bowl centre z | old signed-volume pass | upstream verbatim (1/N) | this port |
/// |---|---|---|---|
/// | 0 | +0.9999 | +0.9999 | +0.9999 |
/// | 2 | +0.9999 | +0.9999 | +0.9999 |
/// | 5 | +0.9999 | +0.9999 | +0.9999 |
/// | 20 | +0.9999 | +0.9999 | +0.9999 |
/// | -20 | **-0.9999** | **-0.9999** | +0.9999 |
///
/// (Outwardness is the mean dot of each face normal with the outward
/// radial direction; +1 is fully outward.)
fn component_outward_needs_flip(
    positions: &[Vec3],
    polygons: &[Vec<usize>],
    comp: &[usize],
    flip: &[bool],
) -> bool {
    let eps = f64::EPSILON;

    // The component's area-weighted centre (upstream's `cent`).
    let mut cent = Vec3::ZERO;
    let mut area_accum = 0.0;
    // DELIBERATE DIVERGENCE FROM UPSTREAM — see the doc comment above.
    // Upstream uses `cent_fac = 1.0f / faces_len` here and then divides the
    // accumulator by the total area only, which leaves a stray 1/N.
    let cent_fac = 1.0;
    for &f in comp {
        let pts: Vec<Vec3> = polygons[f].iter().map(|&i| positions[i]).collect();
        if pts.len() < 3 {
            continue;
        }
        let a = crate::polyfill::polyfill_3d(&pts, crate::polyfill::newell_normal(&pts))
            .iter()
            .map(|t| {
                0.5 * pts[t[1]]
                    .sub(pts[t[0]])
                    .cross(pts[t[2]].sub(pts[t[0]]))
                    .length()
            })
            .sum::<f64>();
        let c = crate::planar_faces::center_median_weighted(&pts);
        cent = cent.add(c.scale(cent_fac * a));
        area_accum += a;
    }
    if area_accum != 0.0 {
        cent = cent.scale(1.0 / area_accum);
    }

    // Upstream's three-key search, compared in this order.
    let mut best_dist_sq = eps;
    let mut best_edge_dot = f64::MIN;
    let mut best_loop_dot = f64::MIN;
    let mut is_flip = false;

    for &f in comp {
        let ring = &polygons[f];
        let n = ring.len();
        if n < 3 {
            continue;
        }
        // Walk the ring as it will be AFTER the consistency pass, since
        // that is the winding whose outwardness we are judging.
        let at = |i: usize| -> Vec3 {
            let k = if flip[f] { n - 1 - (i % n) } else { i % n };
            positions[ring[k]]
        };

        for i in 0..n {
            let v = at(i);
            let dir_raw = v.sub(cent);
            let dist_sq = dir_raw.dot(dir_raw);
            let is_best_dist = dist_sq > best_dist_sq;
            if !(is_best_dist || dist_sq == best_dist_sq) {
                continue;
            }
            let dir = dir_raw.scale(1.0 / dist_sq.sqrt());

            let e0_raw = at(i + 1).sub(v);
            let e1_raw = at(i + n - 1).sub(v);
            let (l0, l1) = (e0_raw.length(), e1_raw.length());
            if !(l0 > eps && l1 > eps) {
                continue;
            }
            let e0 = e0_raw.scale(1.0 / l0);
            let e1 = e1_raw.scale(1.0 / l1);

            let edge_dot = dir.dot(e0).max(dir.dot(e1));
            let is_best_edge = edge_dot > best_edge_dot;
            if !(is_best_dist || is_best_edge || edge_dot == best_edge_dot) {
                continue;
            }

            let loop_dir_raw = e0.cross(e1);
            let ld = loop_dir_raw.length();
            if ld <= eps {
                continue;
            }
            let mut loop_dir = loop_dir_raw.scale(1.0 / ld);

            // Upstream: "Highly unlikely the furthest loop is also the
            // concave part of an ngon, but it can be contrived with _very_
            // non-planar faces - so better check."
            let pts: Vec<Vec3> = (0..n).map(at).collect();
            let face_no = crate::polyfill::newell_normal(&pts);
            if loop_dir.dot(face_no) < 0.0 {
                loop_dir = loop_dir.scale(-1.0);
            }

            let loop_dir_dot = dir.dot(loop_dir);
            let loop_dot = loop_dir_dot.abs();
            if is_best_dist || is_best_edge || loop_dot > best_loop_dot {
                best_dist_sq = dist_sq;
                best_edge_dot = edge_dot;
                best_loop_dot = loop_dot;
                is_flip = loop_dir_dot < 0.0;
            }
        }
    }

    is_flip
}

/// Six times the signed volume contribution of one face (fan-triangulated),
/// with the face optionally reversed. Summed over a *closed* surface this
/// is `6 * V`, and its sign tells inward (`< 0`) from outward (`> 0`).
///
/// **No longer used by the operator.** This was the outward test until
/// 2026-09-20; it is origin-dependent on open surfaces, which
/// [`component_outward_needs_flip`] explains and measures. Kept as a
/// test-only helper so the superseded behaviour can still be exercised
/// alongside the new one.
#[cfg(test)]
fn face_signed_volume6(positions: &[Vec3], poly: &[usize], flip: bool) -> f64 {
    let k = poly.len();
    if k < 3 {
        return 0.0;
    }
    let idx = |i: usize| if flip { poly[k - 1 - i] } else { poly[i] };
    let p0 = positions[idx(0)];
    let mut acc = 0.0;
    for i in 1..k - 1 {
        let v1 = positions[idx(i)];
        let v2 = positions[idx(i + 1)];
        acc += p0.dot(v1.cross(v2));
    }
    acc
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::primitives;
    use std::collections::HashSet;

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

    /// Signed volume × 6 of a mesh as-wound (positive = outward).
    fn signed_volume6(mesh: &Mesh) -> f64 {
        let ps = mesh.positions();
        let mut v = 0.0;
        for poly in mesh.polygons() {
            let idx: Vec<usize> = poly.iter().map(|x| x.0).collect();
            v += super::face_signed_volume6(&ps, &idx, false);
        }
        v
    }

    /// Build a cube with the faces in `reversed` wound inward (inconsistent).
    fn cube_with_reversed(reversed: &[usize]) -> Mesh {
        let cube = primitives::cube(2.0);
        let ps = cube.positions();
        let faces: Vec<Vec<usize>> = cube
            .polygons()
            .into_iter()
            .enumerate()
            .map(|(i, poly)| {
                let v: Vec<usize> = poly.into_iter().map(|x| x.0).collect();
                if reversed.contains(&i) {
                    v.into_iter().rev().collect()
                } else {
                    v
                }
            })
            .collect();
        Mesh::from_polygons(&ps, &faces)
    }

    /// V&V — headline. Methodology: take `cube(2.0)` and reverse faces {0,2,4}
    /// so the surface is inconsistently wound (some directed half-edges have no
    /// reverse partner). Recalculate. Pass criterion: fully consistent winding
    /// and outward-facing. Result: `is_watertight_consistent` holds and the
    /// signed volume is positive (+8 for a side-2 cube), from an input that
    /// failed both.
    #[test]
    fn inconsistent_cube_is_repaired_outward() {
        let bad = cube_with_reversed(&[0, 2, 4]);
        assert!(
            !is_watertight_consistent(&bad),
            "input is inconsistently wound"
        );

        let fixed = recalculate_normals(&bad);
        assert_eq!(fixed.face_count(), 6);
        assert_eq!(fixed.euler_characteristic(), 2);
        assert!(
            is_watertight_consistent(&fixed),
            "repaired to consistent winding"
        );
        let v6 = signed_volume6(&fixed);
        assert!(v6 > 0.0, "normals point outward (signed volume > 0)");
        assert!(
            (v6 - 48.0).abs() < 1e-9,
            "6·V = 6·(2³) = 48 for a side-2 cube"
        );
    }

    /// V&V — an all-inward cube is flipped outward. Reversing every face gives a
    /// consistently-wound but inward mesh (negative volume); recalc flips it.
    #[test]
    fn all_inward_cube_flips_outward() {
        let inward = cube_with_reversed(&[0, 1, 2, 3, 4, 5]);
        assert!(
            is_watertight_consistent(&inward),
            "uniform reverse is still consistent"
        );
        assert!(
            signed_volume6(&inward) < 0.0,
            "but inward (negative volume)"
        );

        let fixed = recalculate_normals(&inward);
        assert!(is_watertight_consistent(&fixed));
        assert!(signed_volume6(&fixed) > 0.0, "flipped outward");
    }

    /// V&V — an already-correct cube keeps its (outward, consistent) winding.
    #[test]
    fn correct_cube_is_unchanged() {
        let cube = primitives::cube(2.0);
        let fixed = recalculate_normals(&cube);
        assert!(is_watertight_consistent(&fixed));
        assert!((signed_volume6(&fixed) - 48.0).abs() < 1e-9);
        assert_eq!(fixed.face_count(), 6);
        assert_eq!(fixed.euler_characteristic(), 2);
    }

    /// V&V — the `MeshOp::RecalculateNormals` dispatch matches the free function.
    #[test]
    fn meshop_recalc_matches_free_function() {
        use crate::ops::MeshOp;
        let bad = cube_with_reversed(&[1, 3]);
        let direct = recalculate_normals(&bad);
        let via_op = MeshOp::RecalculateNormals
            .apply(bad)
            .expect("recalc infallible");
        assert_eq!(via_op.face_count(), direct.face_count());
        assert_eq!(via_op.euler_characteristic(), direct.euler_characteristic());
        assert!(is_watertight_consistent(&via_op));
    }

    /// Build an open hemispherical bowl, outward-wound, centred at `centre`.
    fn bowl(centre: Vec3, radius: f64, lon: usize, lat: usize) -> Mesh {
        let mut pts = Vec::new();
        for j in 0..=lat {
            // Only the upper half: theta from 0 (pole) to pi/2 (equator).
            let theta = std::f64::consts::FRAC_PI_2 * (j as f64) / (lat as f64);
            for i in 0..lon {
                let phi = std::f64::consts::TAU * (i as f64) / (lon as f64);
                pts.push(Vec3::new(
                    centre.x + radius * theta.sin() * phi.cos(),
                    centre.y + radius * theta.sin() * phi.sin(),
                    centre.z + radius * theta.cos(),
                ));
            }
        }
        let mut faces = Vec::new();
        for j in 0..lat {
            for i in 0..lon {
                let a = j * lon + i;
                let b = j * lon + (i + 1) % lon;
                let c = (j + 1) * lon + (i + 1) % lon;
                let d = (j + 1) * lon + i;
                faces.push(vec![a, b, c, d]);
            }
        }
        Mesh::from_polygons(&pts, &faces)
    }

    /// Mean dot of each face normal with the outward radial direction from
    /// `centre`. Positive means the bowl faces outward.
    fn outwardness(m: &Mesh, centre: Vec3) -> f64 {
        let p = m.positions();
        let mut acc = 0.0;
        for f in 0..m.face_count() {
            let vs = m.face_vertices(crate::mesh::FaceId(f));
            let pts: Vec<Vec3> = vs.iter().map(|v| p[v.0]).collect();
            let n = crate::polyfill::newell_normal(&pts);
            let c = pts
                .iter()
                .fold(Vec3::ZERO, |a, &b| a.add(b))
                .scale(1.0 / pts.len() as f64);
            let radial = c.sub(centre);
            let (ln, lr) = (n.length(), radial.length());
            if ln > 0.0 && lr > 0.0 {
                acc += n.dot(radial) / (ln * lr);
            }
        }
        acc / m.face_count() as f64
    }

    /// The regression this operator's outward pass was rewritten for.
    ///
    /// # Methodology
    ///
    /// Build an open hemispherical bowl — an ordinary shape here, being
    /// what a patch or a half-shell looks like before `fill_holes` — wound
    /// inward, at five different positions along Z. Recalculate normals
    /// and measure outwardness: the mean dot of each face normal with the
    /// outward radial direction from the bowl's own centre. +1 is fully
    /// outward, -1 fully inward.
    ///
    /// Translating a mesh cannot change which way is out, so all five must
    /// agree. That is the whole test.
    ///
    /// # Results (measured 2026-09-20)
    ///
    /// | bowl centre z | before | after |
    /// |---|---|---|
    /// | 0 | -0.9999 | +0.9999 |
    /// | 2 | -0.9999 | +0.9999 |
    /// | 5 | -0.9999 | +0.9999 |
    /// | 20 | -0.9999 | +0.9999 |
    /// | -20 | -0.9999 | +0.9999 |
    ///
    /// Before the rewrite the last row came out **-0.9999** — the same
    /// mesh, moved, given the opposite answer. Two separate causes, both
    /// documented on `component_outward_needs_flip`: this crate's
    /// signed-volume test was origin-dependent on open surfaces, and
    /// upstream's replacement criterion carries a stray `1/N` in its
    /// centroid that reintroduced the same dependence.
    #[test]
    fn an_open_surface_gets_the_same_answer_wherever_it_sits() {
        for z in [0.0f64, 2.0, 5.0, 20.0, -20.0] {
            let c = Vec3::new(0.0, 0.0, z);
            let b = bowl(c, 1.0, 16, 6);
            assert!(
                outwardness(&b, c) < -0.9,
                "fixture at z={z} should start inward"
            );
            let after = outwardness(&recalculate_normals(&b), c);
            assert!(
                after > 0.9,
                "bowl at z={z} came out at {after}; translating a mesh must \
                 not change which way is out"
            );
        }
    }

    /// The upstream centroid bug, isolated.
    ///
    /// Upstream's `recalc_face_normals_find_index` computes
    /// `sum(f_cent * area) / (N * total_area)` where an area-weighted
    /// centroid is `sum(f_cent * area) / total_area`. This pins the
    /// arithmetic so the claim in `component_outward_needs_flip`'s docs can
    /// be checked rather than taken on trust.
    #[test]
    fn upstreams_centroid_formula_is_off_by_the_face_count() {
        // Cube of side 2 at (10, 0, 0): 6 faces, each of area 4.
        let cube = primitives::cube(2.0);
        let shifted: Vec<Vec3> = cube
            .positions()
            .iter()
            .map(|p| p.add(Vec3::new(10.0, 0.0, 0.0)))
            .collect();
        let polys: Vec<Vec<usize>> = cube
            .polygons()
            .into_iter()
            .map(|r| r.into_iter().map(|v| v.0).collect())
            .collect();

        let n = polys.len() as f64;
        let mut sum_ca = Vec3::ZERO;
        let mut total_area = 0.0;
        for ring in &polys {
            let pts: Vec<Vec3> = ring.iter().map(|&i| shifted[i]).collect();
            let a = crate::polyfill::polyfill_3d(&pts, crate::polyfill::newell_normal(&pts))
                .iter()
                .map(|t| {
                    0.5 * pts[t[1]]
                        .sub(pts[t[0]])
                        .cross(pts[t[2]].sub(pts[t[0]]))
                        .length()
                })
                .sum::<f64>();
            let c = crate::planar_faces::center_median_weighted(&pts);
            sum_ca = sum_ca.add(c.scale(a));
            total_area += a;
        }

        let correct = sum_ca.scale(1.0 / total_area);
        let upstream = sum_ca.scale(1.0 / (n * total_area));

        assert!(
            correct.sub(Vec3::new(10.0, 0.0, 0.0)).length() < 1e-9,
            "the real centroid should be the cube's centre, got {correct:?}"
        );
        assert!(
            upstream.sub(Vec3::new(10.0 / 6.0, 0.0, 0.0)).length() < 1e-9,
            "upstream's formula should give the centroid over 6, got {upstream:?}"
        );
    }
}
